import type { ImportCheckMatchingDedupTitleOptions, ImportPreviewHistoryRewriteAcknowledgement, ImportPreviewHistoryRewriteAcknowledgementOperation, ImportPreviewHistoryRewriteSelectionScope, ImportPreviewSignalReviewView, ImportPreviewSignalState } from './types.ts';

import { buildSignalTitle, dedupeTextItems, HISTORY_REWRITE_NOTICE, HISTORY_REWRITE_OPERATION_LABELS, normalizePositiveInteger, normalizeTextValue } from './shared.ts';
import { importPreviewSignalStatusIsUnknown } from './signalStatus.ts';



// 历史改写确认模型单独维护，确保危险合并/改写的 acknowledgement payload 不和普通信号 chip 混在一起。

export function isImportPreviewHistoryRewriteOperation(rawOperation: string | undefined): boolean {
    const normalizedOperation = normalizeTextValue(rawOperation).toLowerCase();
    return normalizedOperation === 'update_history' || normalizedOperation === 'merge_transfer_history';
}

export function getImportPreviewHistoryRewriteLabelKey(rawOperation: string | undefined): string {
    const normalizedOperation = normalizeTextValue(rawOperation).toLowerCase();
    return HISTORY_REWRITE_OPERATION_LABELS[normalizedOperation] || 'History Rewrite';
}

export function buildHistoryRewriteDetailLines(
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    const summary = state.reconciliationHistorySummary;
    if (!summary) {
        return ['历史账单详情暂不可用，可点击查看原账单'];
    }
    const currency = normalizeTextValue(summary.currency).toUpperCase() || 'CNY';
    const amount = options.formatAmountWithCurrency
        ? options.formatAmountWithCurrency(summary.amount_cents, currency)
        : String(summary.amount_cents);
    const identityLabel = (name: string | null | undefined, status: 'known' | 'deleted' | 'unknown' | undefined, kind: '分类' | '账户'): string => {
        if (status === 'deleted') {
            return `已删除${kind}`;
        }
        return normalizeTextValue(name) || `未知${kind}`;
    };
    const route = [
        identityLabel(summary.source_account_name, summary.source_account_status, '账户'),
        summary.destination_account_status === 'deleted' || normalizeTextValue(summary.destination_account_name)
            ? identityLabel(summary.destination_account_name, summary.destination_account_status, '账户')
            : ''
    ]
        .filter(Boolean)
        .join(' → ');
    return dedupeTextItems([
        `历史账单：${normalizeTextValue(summary.date_time)}`,
        `金额：${amount}`,
        `分类：${identityLabel(summary.category_name, summary.category_status, '分类')}`,
        `账户：${route}`,
        normalizeTextValue(summary.counterparty) ? `对方：${normalizeTextValue(summary.counterparty)}` : '',
        normalizeTextValue(summary.description) ? `备注：${normalizeTextValue(summary.description)}` : ''
    ]);
}

export function buildHistoryRewriteSignalView(
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): ImportPreviewSignalReviewView | null {
    if (importPreviewSignalStatusIsUnknown('history', state.reconciliationStatus)) {
        return null;
    }
    const plannedOperation = normalizeTextValue(state.reconciliationPlannedOperation);
    const hasHistoryRewriteMarker = isImportPreviewHistoryRewriteOperation(plannedOperation)
        || !!state.reconciliationDestructiveAckRequired;
    if (!hasHistoryRewriteMarker) {
        return null;
    }

    const detailLines = buildHistoryRewriteDetailLines(state, options);
    const labelKey = getImportPreviewHistoryRewriteLabelKey(plannedOperation);
    return {
        status: 'pending',
        labelKey,
        title: buildSignalTitle(detailLines, state.reconciliationTitle || HISTORY_REWRITE_NOTICE),
        color: 'warning',
        actions: [],
        detailLines,
        historyBillId: normalizePositiveInteger(state.reconciliationHistoryBillId)
    };
}

// 将单行预览的历史改写信号转换成 confirm API 需要的 acknowledgement operation，缺少 token 或历史账单身份时必须丢弃。
export function buildImportPreviewHistoryRewriteOperationAcknowledgement(
    previewId: number | string | null | undefined,
    state: ImportPreviewSignalState
): ImportPreviewHistoryRewriteAcknowledgementOperation | null {
    const normalizedPreviewId = normalizePositiveInteger(previewId);
    const plannedOperation = normalizeTextValue(state.reconciliationPlannedOperation);
    const operationId = normalizeTextValue(state.reconciliationOperationId);
    const acknowledgementToken = normalizeTextValue(state.reconciliationAcknowledgementToken);
    const historyBillId = normalizePositiveInteger(state.reconciliationHistoryBillId);
    const historyBillVersion = normalizePositiveInteger(state.reconciliationHistoryBillVersion) || 1;

    if (
        normalizedPreviewId <= 0
        || !isImportPreviewHistoryRewriteOperation(plannedOperation)
        || !operationId
        || !acknowledgementToken
        || historyBillId <= 0
    ) {
        return null;
    }

    return {
        preview_id: normalizedPreviewId,
        operation_id: operationId,
        planned_operation: plannedOperation,
        history_bill_id: historyBillId,
        history_bill_version: historyBillVersion,
        acknowledgement_token: acknowledgementToken
    };
}

// 汇总跨页选择范围内的历史改写 acknowledgement，confirm 前端 payload 只发送已选 preview id 对应的危险操作。
export function buildImportPreviewHistoryRewriteAcknowledgement({
    selectedPreviewIds,
    operations,
    selectionScope
}: {
    selectedPreviewIds: Array<number | string | null | undefined>;
    operations: ImportPreviewHistoryRewriteAcknowledgementOperation[];
    selectionScope: ImportPreviewHistoryRewriteSelectionScope;
}): ImportPreviewHistoryRewriteAcknowledgement | null {
    const normalizedSelectedIds = selectedPreviewIds
        .map(previewId => normalizePositiveInteger(previewId))
        .filter(previewId => previewId > 0)
        .sort((left, right) => left - right)
        .filter((previewId, index, ids) => index === 0 || ids[index - 1] !== previewId);
    const uniqueOperations = new Map<number, ImportPreviewHistoryRewriteAcknowledgementOperation>();
    for (const operation of operations) {
        if (operation.preview_id > 0) {
            uniqueOperations.set(operation.preview_id, operation);
        }
    }

    if (uniqueOperations.size < 1) {
        return null;
    }

    return {
        acknowledged: true,
        selected_preview_ids: normalizedSelectedIds,
        operations: Array.from(uniqueOperations.values()).sort((left, right) => left.preview_id - right.preview_id),
        selection_scope: {
            ...selectionScope,
            selected_count: normalizedSelectedIds.length,
            history_rewrite_count: uniqueOperations.size
        }
    };
}
