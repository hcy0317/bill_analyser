import { TransactionType } from '@/core/transaction.ts';

import { matchesImportTransactionCheckDataFilters } from '../checkDataFilters.ts';

import { buildImportPreviewSignalViewModelFromSnapshot } from '../checkDataMatching.ts';

import type { ImportPreviewSignalViewModel } from '../checkDataMatching.ts';
import type { ImportPreviewIndexFilterContext, ImportPreviewIndexItem, ImportPreviewIndexPageResult, ImportPreviewIndexResponseItem } from './types.ts';

import type { ImportCheckDataFilterLike } from '../checkDataFilters.ts';



// 响应映射和本地兜底分页仅用于预览索引模型，不直接承担 API 请求副作用。

export function mapImportPreviewIndexResponseItem(item: ImportPreviewIndexResponseItem): ImportPreviewIndexItem {
    const time = new Date(item.preview_date || '').getTime() / 1000;
    const learningStatusPresent = Object.prototype.hasOwnProperty.call(item, 'learning_status');
    const llmStatusPresent = Object.prototype.hasOwnProperty.call(item, 'llm_status');
    const historyStatusPresent = Object.prototype.hasOwnProperty.call(item, 'history_status');
    return {
        id: Number(item.id || 0),
        time: Number.isFinite(time) ? time : Date.now() / 1000,
        type: Number(item.type || TransactionType.ModifyBalance),
        actualCategoryName: item.actual_category_name || '',
        categoryId: item.category_id || '',
        actualSourceAccountName: item.actual_source_account_name || '',
        actualDestinationAccountName: item.actual_destination_account_name || '',
        sourceAccountId: item.source_account_id || '',
        destinationAccountId: item.destination_account_id || '',
        tagIds: [],
        originalTagNames: [],
        comment: item.comment || '',
        isManuallyAnnotated: !!item.is_manually_annotated,
        selected: !!item.selected,
        sourceAmountCents: Number(item.source_amount_cents || 0),
        counterparty: item.counterparty || '',
        paymentMethod: item.payment_method || '',
        parserId: item.parser_source || '',
        parserTags: item.parser_tags || [],
        dedupType: item.dedup_type || '',
        dedupSourceIds: item.dedup_source_ids || [],
        previewState: item.preview_state,
        transferStatus: item.transfer_status ?? null,
        transferTitle: item.transfer_title || '',
        learningStatus: learningStatusPresent ? (item.learning_status ?? null) : undefined,
        learningStatusAbsent: !learningStatusPresent,
        learningLifecycleStatus: item.learning_lifecycle_status || '',
        learningSignalState: item.learning_signal_state || '',
        learningTitle: item.learning_title || '',
        learningSummary: item.learning_summary || '',
        learningMode: item.learning_mode || '',
        learningAutoApplied: item.learning_auto_applied,
        learningSuppressed: item.learning_suppressed,
        learningRuleId: item.learning_rule_id ?? null,
        learningScore: item.learning_score ?? 0,
        learningConfidence: item.learning_confidence ?? 0,
        learningMargin: item.learning_margin ?? 0,
        learningAcceptedCount: item.learning_accepted_count ?? 0,
        learningRejectedCount: item.learning_rejected_count ?? 0,
        learningAutoAppliedCount: item.learning_auto_applied_count ?? 0,
        llmStatus: llmStatusPresent ? (item.llm_status ?? null) : undefined,
        llmStatusAbsent: !llmStatusPresent,
        llmLifecycleStatus: item.llm_lifecycle_status || '',
        llmSignalState: item.llm_signal_state || '',
        llmTitle: item.llm_title || '',
        llmConfidence: item.llm_confidence ?? 0,
        llmSuggestedCategoryId: item.llm_suggested_category_id ?? 0,
        llmCategoryPath: item.llm_category_path || '',
        llmSourceAccount: item.llm_source_account || '',
        llmDestinationAccount: item.llm_destination_account || '',
        llmSuppressed: item.llm_suppressed,
        historyStatus: historyStatusPresent ? (item.history_status ?? null) : undefined,
        historyTitle: item.history_title || '',
        historyPlannedOperation: item.history_planned_operation || '',
        historyBillId: item.history_bill_id ?? null,
        historyBillVersion: item.history_bill_version ?? null,
        historyOperationId: item.history_operation_id || '',
        historyAcknowledgementToken: item.history_acknowledgement_token || '',
        historyDestructiveAckRequired: !!item.history_destructive_ack_required,
        historySummary: item.history_summary ?? null,
        recurringTemplateId: item.recurring_template_id || '',
        recurringCandidateCount: Number(item.recurring_candidate_count || 0),
        recurringMatchReasons: item.recurring_match_reasons || '',
        recurringMatchedDate: item.recurring_matched_date || '',
    };
}

export function getPrimaryRecurringReason(item: ImportPreviewIndexItem): string {
    if (!item.recurringMatchReasons) {
        return '';
    }

    return item.recurringMatchReasons
        .split('|')
        .map(text => text.trim())
        .filter(text => !!text)[0] || '';
}

export function collectImportPreviewIndexAnnotationIssues(item: ImportPreviewIndexItem): string[] {
    const reasons: string[] = [];

    if (item.type !== TransactionType.ModifyBalance && (!item.categoryId || item.categoryId === '0')) {
        reasons.push('Missing Category');
    }

    if (!item.sourceAccountId || item.sourceAccountId === '0') {
        reasons.push('Missing Source Account');
    }

    const requiresDestinationAccount = item.type === 4 || item.type === 5;
    if (requiresDestinationAccount && (!item.destinationAccountId || item.destinationAccountId === '0')) {
        reasons.push('Missing Destination Account');
    }

    if (
        requiresDestinationAccount
        && item.sourceAccountId
        && item.destinationAccountId
        && item.sourceAccountId !== '0'
        && item.destinationAccountId !== '0'
        && item.sourceAccountId === item.destinationAccountId
    ) {
        reasons.push('Review Transfer Accounts');
    }

    return reasons;
}

export function buildImportPreviewIndexSignalViewModel(item: ImportPreviewIndexItem): ImportPreviewSignalViewModel {
    return buildImportPreviewSignalViewModelFromSnapshot({
        parserId: item.parserId,
        parserTags: item.parserTags,
        dedupType: item.dedupType,
        dedupSourceIds: item.dedupSourceIds,
        isManuallyAnnotated: item.isManuallyAnnotated,
        transferStatus: item.transferStatus ?? null,
        transferTitle: item.transferTitle || '',
        learningStatus: item.learningStatus,
        learningTitle: item.learningTitle || '',
        learningSummary: item.learningSummary || '',
        learningMode: item.learningMode || '',
        learningLifecycleStatus: item.learningLifecycleStatus || '',
        learningSignalState: item.learningSignalState || '',
        learningAutoApplied: item.learningAutoApplied,
        learningSuppressed: item.learningSuppressed,
        learningStatusAuthoritative: !item.learningStatusAbsent && typeof item.learningStatus !== 'undefined',
        learningRuleId: item.learningRuleId ?? null,
        learningScore: item.learningScore ?? 0,
        learningConfidence: item.learningConfidence ?? 0,
        learningMargin: item.learningMargin ?? 0,
        learningAcceptedCount: item.learningAcceptedCount ?? 0,
        learningRejectedCount: item.learningRejectedCount ?? 0,
        learningAutoAppliedCount: item.learningAutoAppliedCount ?? 0,
        llmStatus: item.llmStatus,
        llmTitle: item.llmTitle || '',
        llmConfidence: item.llmConfidence ?? 0,
        llmSuggestedCategoryId: item.llmSuggestedCategoryId ?? 0,
        llmCategoryPath: item.llmCategoryPath || '',
        llmSourceAccount: item.llmSourceAccount || '',
        llmDestinationAccount: item.llmDestinationAccount || '',
        llmLifecycleStatus: item.llmLifecycleStatus || '',
        llmSignalState: item.llmSignalState || '',
        llmSuppressed: item.llmSuppressed,
        llmStatusAuthoritative: !item.llmStatusAbsent && typeof item.llmStatus !== 'undefined',
        reconciliationTitle: item.historyTitle || '',
        reconciliationPlannedOperation: item.historyPlannedOperation || '',
        reconciliationHistoryBillId: item.historyBillId ?? null,
        reconciliationHistoryBillVersion: item.historyBillVersion ?? null,
        reconciliationOperationId: item.historyOperationId || '',
        reconciliationAcknowledgementToken: item.historyAcknowledgementToken || '',
        reconciliationDestructiveAckRequired: !!item.historyDestructiveAckRequired,
        reconciliationHistorySummary: item.historySummary ?? null,
        hasRecurringMatch: !!item.recurringTemplateId,
        recurringTitle: item.recurringMatchReasons || '',
        recurringCandidateCount: Number(item.recurringCandidateCount || 0),
        recurringPrimaryReason: getPrimaryRecurringReason(item),
    }, item.previewState);
}

// 预览索引本地兜底筛选复用 check-data 可见筛选语义，保持 server-paged 与当前页草稿的信号/标注判断一致。
export function matchesImportPreviewIndexItemFilters(
    item: ImportPreviewIndexItem,
    filters: ImportCheckDataFilterLike,
    context: ImportPreviewIndexFilterContext = {},
): boolean {
    return matchesImportTransactionCheckDataFilters(item, filters, {
        tagNameById: context.tagNameById,
        hasAnnotationIssues: candidate => collectImportPreviewIndexAnnotationIssues(candidate).length > 0,
        isEditing: () => false,
        signalViewModelFor: candidate => buildImportPreviewIndexSignalViewModel(candidate),
    });
}

// 本地索引排序按 preview id 做同方向 tie-breaker，与 Rust/SQL 的稳定分页顺序保持一致。
export function sortImportPreviewIndexItems(
    items: ImportPreviewIndexItem[],
    sortBy: string | null | undefined,
    sortDirection: 'asc' | 'desc' | null | undefined,
): ImportPreviewIndexItem[] {
    const normalizedSortBy = String(sortBy || '').trim();
    const normalizedSortDirection = sortDirection === 'desc' ? 'desc' : 'asc';
    const directionMultiplier = normalizedSortDirection === 'desc' ? -1 : 1;
    const stabilizedItems = items.map((item, index) => ({ item, index }));

    const compareNumber = (left: number, right: number): number => {
        if (left === right) {
            return 0;
        }
        return left < right ? -1 : 1;
    };

    const compareString = (left: string, right: string): number => left.localeCompare(right, undefined, {
        sensitivity: 'base',
    });

    stabilizedItems.sort((left, right) => {
        let compareResult = 0;
        switch (normalizedSortBy) {
            case 'time':
                compareResult = compareNumber(Number(left.item.time || 0), Number(right.item.time || 0));
                break;
            case 'type':
                compareResult = compareNumber(Number(left.item.type || 0), Number(right.item.type || 0));
                break;
            case 'sourceAmountCents':
                compareResult = compareNumber(Number(left.item.sourceAmountCents || 0), Number(right.item.sourceAmountCents || 0));
                break;
            case 'counterparty':
                compareResult = compareString(left.item.counterparty || '', right.item.counterparty || '');
                break;
            case 'paymentMethod':
                compareResult = compareString(left.item.paymentMethod || '', right.item.paymentMethod || '');
                break;
            case 'comment':
                compareResult = compareString(left.item.comment || '', right.item.comment || '');
                break;
            default:
                compareResult = compareNumber(Number(left.item.time || 0), Number(right.item.time || 0));
                break;
        }

        if (compareResult !== 0) {
            return compareResult * directionMultiplier;
        }

        const idCompareResult = compareNumber(Number(left.item.id), Number(right.item.id));
        if (idCompareResult !== 0) {
            return idCompareResult * directionMultiplier;
        }

        return left.index - right.index;
    });

    return stabilizedItems.map(entry => entry.item);
}

// 本地索引分页只返回当前页 preview id 列表，供无服务端分页时复用同一 index result contract。
export function resolveImportPreviewIndexPage(
    items: ImportPreviewIndexItem[],
    page: number,
    pageSize: number,
): ImportPreviewIndexPageResult {
    const totalCount = items.length;
    const normalizedPageSize = Math.max(Number(pageSize || 10), 1);
    const totalPages = Math.max(Math.ceil(totalCount / normalizedPageSize), 1);
    const normalizedPage = Math.min(Math.max(Number(page || 1), 1), totalPages);
    const start = (normalizedPage - 1) * normalizedPageSize;
    const previewIds = items.slice(start, start + normalizedPageSize).map(item => item.id);

    return {
        page: normalizedPage,
        totalCount,
        totalPages,
        previewIds,
    };
}
