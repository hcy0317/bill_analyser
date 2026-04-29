import { TransactionType } from '@/core/transaction.ts';

import {
    matchesImportTransactionCheckDataFilters,
    type ImportCheckDataFilterLike,
    type ImportCheckVisibleTransactionContext,
    type ImportCheckVisibleTransactionLike,
} from './checkDataFilters.ts';
import {
    buildImportPreviewSignalViewModel,
    type ImportPreviewSignalStatus,
    type ImportPreviewSignalViewModel,
} from './checkDataMatching.ts';

export interface ImportPreviewIndexItem extends ImportCheckVisibleTransactionLike {
    id: number;
    selected: boolean;
    sourceAmount: number;
    counterparty: string;
    paymentMethod: string;
    parserSource: string;
    parserTags: string[];
    dedupType: string;
    dedupSourceIds: Array<number | string>;
    transferStatus?: ImportPreviewSignalStatus | null;
    transferTitle?: string;
    learningStatus?: ImportPreviewSignalStatus | null;
    learningTitle?: string;
    learningSummary?: string;
    learningMode?: string;
    recurringTemplateId?: string;
    recurringCandidateCount?: number;
    recurringMatchReasons?: string;
    recurringMatchedDate?: string;
}

export type ImportPreviewIndexFilterContext =
    Pick<ImportCheckVisibleTransactionContext<ImportPreviewIndexItem>, 'tagNameById'>;

export interface ImportPreviewIndexPageResult {
    page: number;
    totalCount: number;
    totalPages: number;
    previewIds: number[];
}

function getPrimaryRecurringReason(item: ImportPreviewIndexItem): string {
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
        parserSource: item.parserSource,
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
            case 'sourceAmount':
                compareResult = compareNumber(Number(left.item.sourceAmount || 0), Number(right.item.sourceAmount || 0));
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
