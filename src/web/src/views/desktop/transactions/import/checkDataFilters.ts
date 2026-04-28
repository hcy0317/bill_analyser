import { DateRange, DateRangeScene, type WeekDayValue } from '@/core/datetime.ts';
import { TransactionType } from '@/core/transaction.ts';
import { getDateRangeByDateType, getDateTypeByDateRange } from '@/lib/datetime.ts';

import {
    matchesImportCheckAnnotationFilter,
    type ImportCheckAnnotationFilterValue
} from './checkDataAnnotation.ts';
import {
    matchesImportPreviewSignalFilter,
    type ImportPreviewSignalViewModel,
    type ImportPreviewVisibleSignalFilterValue
} from './checkDataMatching.ts';

export interface ImportCheckDataFilterLike {
    minDatetime: number | null;
    maxDatetime: number | null;
    transactionType: number | null;
    category: string | null | undefined;
    account: string | null | undefined;
    tag: string | null | undefined;
    signal: ImportPreviewVisibleSignalFilterValue | null;
    annotation: ImportCheckAnnotationFilterValue;
    description: string | null;
}

export interface ImportCheckVisibleTransactionLike {
    time: number;
    type: number;
    actualCategoryName?: string;
    categoryId?: string;
    actualSourceAccountName?: string;
    actualDestinationAccountName?: string;
    sourceAccountId?: string;
    destinationAccountId?: string;
    tagIds?: string[];
    originalTagNames?: string[];
    comment?: string;
    isManuallyAnnotated?: boolean;
}

export interface ImportCheckVisibleTransactionContext<T extends ImportCheckVisibleTransactionLike> {
    tagNameById?: Record<string, { name?: string } | undefined>;
    hasAnnotationIssues: (transaction: T) => boolean;
    isEditing: (transaction: T) => boolean;
    signalViewModelFor: (transaction: T) => ImportPreviewSignalViewModel;
}

export interface ImportCheckVisibleTransactionPageOptions {
    serverPaged: boolean;
    currentPage: number;
    countPerPage: number;
}

export type ImportCheckFilterMenuKey =
    | 'annotation'
    | 'signal'
    | 'dateRange'
    | 'type'
    | 'category'
    | 'account'
    | 'tag'
    | 'description';

export const IMPORT_CHECK_FILTER_MENU_ORDER: readonly ImportCheckFilterMenuKey[] = [
    'annotation',
    'signal',
    'dateRange',
    'type',
    'category',
    'account',
    'tag',
    'description'
] as const;

export const IMPORT_CHECK_DATE_PRESET_TYPES: readonly number[] = [
    DateRange.All.type,
    DateRange.ThisWeek.type,
    DateRange.ThisMonth.type,
    DateRange.ThisYear.type,
    DateRange.Custom.type
] as const;

export function resolveImportCheckDatePresetRange(
    dateType: number,
    firstDayOfWeek: WeekDayValue,
    fiscalYearStart: number
): { minDatetime: number | null; maxDatetime: number | null } {
    if (dateType === DateRange.All.type) {
        return {
            minDatetime: null,
            maxDatetime: null
        };
    }

    const range = getDateRangeByDateType(dateType, firstDayOfWeek, fiscalYearStart);
    if (!range) {
        return {
            minDatetime: null,
            maxDatetime: null
        };
    }

    return {
        minDatetime: range.minTime,
        maxDatetime: range.maxTime
    };
}

export function resolveImportCheckDatePresetType(
    minDatetime: number | null,
    maxDatetime: number | null,
    firstDayOfWeek: WeekDayValue,
    fiscalYearStart: number
): number {
    if (minDatetime === null || maxDatetime === null) {
        return DateRange.All.type;
    }

    return getDateTypeByDateRange(
        minDatetime,
        maxDatetime,
        firstDayOfWeek,
        fiscalYearStart,
        DateRangeScene.Normal
    );
}

export function matchesImportTransactionCheckDataFilters<T extends ImportCheckVisibleTransactionLike>(
    transaction: T,
    filters: ImportCheckDataFilterLike,
    context: ImportCheckVisibleTransactionContext<T>
): boolean {
    if (
        typeof filters.minDatetime === 'number'
        && typeof filters.maxDatetime === 'number'
        && (transaction.time < filters.minDatetime || transaction.time > filters.maxDatetime)
    ) {
        return false;
    }

    if (typeof filters.transactionType === 'number' && transaction.type !== filters.transactionType) {
        return false;
    }

    const actualCategoryName = transaction.actualCategoryName || '';
    const categoryId = transaction.categoryId || '';
    if (typeof filters.category === 'string') {
        if (filters.category === '' && actualCategoryName !== '') {
            return false;
        } else if (filters.category !== '' && actualCategoryName !== filters.category) {
            return false;
        }
    } else if (filters.category === undefined) {
        if (transaction.type !== TransactionType.ModifyBalance && categoryId && categoryId !== '0') {
            return false;
        }
    }

    const actualSourceAccountName = transaction.actualSourceAccountName || '';
    const actualDestinationAccountName = transaction.actualDestinationAccountName || '';
    const sourceAccountId = transaction.sourceAccountId || '';
    const destinationAccountId = transaction.destinationAccountId || '';
    if (typeof filters.account === 'string') {
        if (filters.account === '' && (actualSourceAccountName !== '' || actualDestinationAccountName !== '')) {
            return false;
        } else if (
            filters.account !== ''
            && actualSourceAccountName !== filters.account
            && actualDestinationAccountName !== filters.account
        ) {
            return false;
        }
    } else if (filters.account === undefined) {
        if (transaction.type !== TransactionType.Transfer && sourceAccountId && sourceAccountId !== '0') {
            return false;
        } else if (
            transaction.type === TransactionType.Transfer
            && sourceAccountId && sourceAccountId !== '0'
            && destinationAccountId && destinationAccountId !== '0'
        ) {
            return false;
        }
    }

    if (typeof filters.tag === 'string') {
        if (filters.tag === '' && transaction.tagIds && transaction.tagIds.length > 0) {
            return false;
        } else if (filters.tag !== '') {
            let hasTagName = false;
            for (const [tagIndex, tagId] of (transaction.tagIds || []).entries()) {
                let tagName = transaction.originalTagNames?.[tagIndex] || '';
                if (tagId && tagId !== '0') {
                    tagName = context.tagNameById?.[tagId]?.name || tagName;
                }
                if (tagName === filters.tag) {
                    hasTagName = true;
                    break;
                }
            }

            if (!hasTagName) {
                return false;
            }
        }
    } else if (filters.tag === undefined) {
        if (transaction.tagIds && transaction.tagIds.length > 0) {
            const hasInvalidTag = transaction.tagIds.some(tagId => !tagId || tagId === '0');
            if (!hasInvalidTag) {
                return false;
            }
        } else {
            return false;
        }
    }

    if (
        !matchesImportCheckAnnotationFilter(filters.annotation, {
            hasAnnotationIssues: context.hasAnnotationIssues(transaction),
            isManuallyAnnotated: !!transaction.isManuallyAnnotated,
            isEditing: context.isEditing(transaction)
        })
    ) {
        return false;
    }

    if (!matchesImportPreviewSignalFilter(context.signalViewModelFor(transaction), filters.signal)) {
        return false;
    }

    if (typeof filters.description === 'string') {
        const comment = transaction.comment || '';
        if (filters.description === '' && comment !== '') {
            return false;
        } else if (filters.description !== '' && !comment.includes(filters.description)) {
            return false;
        }
    }

    return true;
}

export function getImportCheckVisibleTransactions<T>(
    transactions: T[],
    isDisplayed: (transaction: T) => boolean,
    options: ImportCheckVisibleTransactionPageOptions
): T[] {
    const visibleTransactions = transactions.filter(transaction => isDisplayed(transaction));

    if (options.serverPaged) {
        return visibleTransactions;
    }

    if (options.countPerPage < 0) {
        return visibleTransactions;
    }

    const normalizedCurrentPage = Math.max(options.currentPage || 1, 1);
    const normalizedCountPerPage = Math.max(options.countPerPage || 1, 1);
    const start = Math.max((normalizedCurrentPage - 1) * normalizedCountPerPage, 0);
    return visibleTransactions.slice(start, start + normalizedCountPerPage);
}
