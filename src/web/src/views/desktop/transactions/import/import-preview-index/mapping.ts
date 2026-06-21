import { TransactionType } from '@/core/transaction.ts';

import { matchesImportTransactionCheckDataFilters } from '../checkDataFilters.ts';

import { buildImportPreviewSignalViewModel } from '../checkDataMatching.ts';

import type { ImportPreviewSignalViewModel } from '../checkDataMatching.ts';
import type { ImportPreviewIndexFilterContext, ImportPreviewIndexItem, ImportPreviewIndexPageResult, ImportPreviewIndexResponseItem } from './types.ts';

import type { ImportCheckDataFilterLike } from '../checkDataFilters.ts';



// 响应映射和本地兜底分页仅用于预览索引模型，不直接承担 API 请求副作用。

export function mapImportPreviewIndexResponseItem(item: ImportPreviewIndexResponseItem): ImportPreviewIndexItem {
    const time = new Date(item.preview_date || '').getTime() / 1000;
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
        transferStatus: item.transfer_status ?? null,
        transferTitle: item.transfer_title || '',
        learningStatus: item.learning_status ?? null,
        learningTitle: item.learning_title || '',
        learningSummary: item.learning_summary || '',
        learningMode: item.learning_mode || '',
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
    return buildImportPreviewSignalViewModel({
        parserId: item.parserId,
        parserTags: item.parserTags,
        dedupType: item.dedupType,
        dedupSourceIds: item.dedupSourceIds,
        isManuallyAnnotated: item.isManuallyAnnotated,
        transferStatus: item.transferStatus ?? null,
        transferTitle: item.transferTitle || '',
        learningStatus: item.learningStatus ?? null,
        learningTitle: item.learningTitle || '',
        learningSummary: item.learningSummary || '',
        learningMode: item.learningMode || '',
        hasRecurringMatch: !!item.recurringTemplateId,
        recurringTitle: item.recurringMatchReasons || '',
        recurringCandidateCount: Number(item.recurringCandidateCount || 0),
        recurringPrimaryReason: getPrimaryRecurringReason(item),
    });
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

// 本地索引排序必须稳定保留原始顺序作为 tie-breaker，避免前端 fallback 分页在相同字段值下抖动。
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
