import { describe, expect, test } from '@jest/globals';

import { DateRange } from '@/core/datetime.ts';
import { TransactionType } from '@/core/transaction.ts';
import type { ImportTransaction } from '@/models/imported_transaction.ts';
import { buildImportPreviewSignalViewModel } from '@/views/desktop/transactions/import/checkDataMatching.ts';
import {
    collectTrackedImportTransactionsForSelection,
    collectImportTransactionSelectionSummary
} from '@/views/desktop/transactions/import/checkDataSelection.ts';
import {
    getImportCheckVisibleTransactions,
    IMPORT_CHECK_FILTER_MENU_ORDER,
    matchesImportTransactionCheckDataFilters,
    resolveImportCheckDatePresetRange,
    resolveImportCheckDatePresetType,
    type ImportCheckDataFilterLike,
    type ImportCheckVisibleTransactionLike
} from '@/views/desktop/transactions/import/checkDataFilters.ts';

interface TestRow extends ImportCheckVisibleTransactionLike {
    id: number;
    signalState?: Parameters<typeof buildImportPreviewSignalViewModel>[0];
    hasAnnotationIssues?: boolean;
}

const defaultFilters: ImportCheckDataFilterLike = {
    minDatetime: null,
    maxDatetime: null,
    transactionType: null,
    category: null,
    account: null,
    tag: null,
    signal: null,
    annotation: null,
    description: null
};

function matches(row: TestRow, filters: Partial<ImportCheckDataFilterLike> = {}): boolean {
    return matchesImportTransactionCheckDataFilters(row, {
        ...defaultFilters,
        ...filters
    }, {
        tagNameById: {},
        hasAnnotationIssues: transaction => !!transaction.hasAnnotationIssues,
        isEditing: () => false,
        signalViewModelFor: transaction => buildImportPreviewSignalViewModel(transaction.signalState || {})
    });
}

function selectionRow(
    index: number,
    type: TransactionType,
    selected: boolean,
    options: { valid?: boolean; recurring?: boolean } = {}
): ImportTransaction {
    return {
        index,
        type,
        selected,
        valid: options.valid ?? true,
        hasRecurringMatch: () => !!options.recurring
    } as ImportTransaction;
}

function previewIdOf(transaction: ImportTransaction): number | null {
    const previewId = (transaction as { _previewId?: number })._previewId;
    return typeof previewId === 'number' ? previewId : null;
}

describe('checkDataFilters helpers', () => {
    test('keeps the filter-menu order aligned with the preview columns and row status order', () => {
        expect(IMPORT_CHECK_FILTER_MENU_ORDER).toStrictEqual([
            'annotation',
            'signal',
            'dateRange',
            'type',
            'category',
            'account',
            'tag',
            'description'
        ]);
    });

    test('maps 本周、本月、本年 presets back to stable min/max datetime ranges', () => {
        const firstDayOfWeek = 1;
        const fiscalYearStart = 1;

        const allRange = resolveImportCheckDatePresetRange(DateRange.All.type, firstDayOfWeek, fiscalYearStart);
        expect(allRange).toStrictEqual({
            minDatetime: null,
            maxDatetime: null
        });

        for (const dateType of [DateRange.ThisWeek.type, DateRange.ThisMonth.type, DateRange.ThisYear.type]) {
            const range = resolveImportCheckDatePresetRange(dateType, firstDayOfWeek, fiscalYearStart);
            expect(typeof range.minDatetime).toBe('number');
            expect(typeof range.maxDatetime).toBe('number');
            expect((range.minDatetime || 0) <= (range.maxDatetime || 0)).toBe(true);
            expect(resolveImportCheckDatePresetType(
                range.minDatetime,
                range.maxDatetime,
                firstDayOfWeek,
                fiscalYearStart
            )).toBe(dateType);
        }
    });

    test('keeps server-paged preview rows visible when no local filter is applied', () => {
        const rows: TestRow[] = [
            { id: 1, time: 100, type: 5, comment: 'row-1' },
            { id: 2, time: 200, type: 5, comment: 'row-2' }
        ];

        const visibleRows = getImportCheckVisibleTransactions(
            rows,
            row => matches(row),
            {
                serverPaged: true,
                currentPage: 1,
                countPerPage: 10
            }
        );

        expect(visibleRows.map(row => row.id)).toStrictEqual([1, 2]);
    });

    test('treats server-paged currentPage and countPerPage as request state instead of re-slicing fetched rows', () => {
        const rows: TestRow[] = [
            { id: 1, time: 100, type: 5, comment: 'row-1' },
            { id: 2, time: 200, type: 5, comment: 'row-2' },
            { id: 3, time: 300, type: 5, comment: 'row-3' }
        ];

        const visibleRows = getImportCheckVisibleTransactions(
            rows,
            row => matches(row),
            {
                serverPaged: true,
                currentPage: 4,
                countPerPage: 50
            }
        );

        expect(visibleRows.map(row => row.id)).toStrictEqual([1, 2, 3]);
    });

    test('applies existing annotation, signal, and date filters to visible-row calculation', () => {
        const rows: TestRow[] = [
            { id: 1, time: 100, type: 5, comment: 'parser-row', signalState: { parserSource: 'alipay' } },
            { id: 2, time: 200, type: 5, comment: 'learning-row', signalState: { learningStatus: 'pending', learningSummary: '支出 | 餐饮/咖啡' } },
            { id: 3, time: 300, type: 5, comment: 'manual-row', isManuallyAnnotated: true },
            { id: 4, time: 400, type: 5, comment: 'missing-row', hasAnnotationIssues: true }
        ];

        const learningRows = getImportCheckVisibleTransactions(
            rows,
            row => matches(row, {
                minDatetime: 150,
                maxDatetime: 250,
                signal: 'learning'
            }),
            {
                serverPaged: true,
                currentPage: 1,
                countPerPage: 10
            }
        );
        expect(learningRows.map(row => row.id)).toStrictEqual([2]);

        const annotatedRows = getImportCheckVisibleTransactions(
            rows,
            row => matches(row, {
                annotation: 'needs-review'
            }),
            {
                serverPaged: true,
                currentPage: 1,
                countPerPage: 10
            }
        );
        expect(annotatedRows.map(row => row.id)).toStrictEqual([4]);

        const resolvedManualRows = getImportCheckVisibleTransactions(
            rows,
            row => matches(row, {
                annotation: 'no-issues'
            }),
            {
                serverPaged: true,
                currentPage: 1,
                countPerPage: 10
            }
        );
        expect(resolvedManualRows.map(row => row.id)).toContain(3);
    });

    test('summarizes selected check-data rows and annotation reasons', () => {
        const rows = [
            selectionRow(1, TransactionType.Expense, true, { recurring: true }),
            selectionRow(2, TransactionType.Income, true, { valid: false }),
            selectionRow(3, TransactionType.Transfer, false)
        ];
        const issuesByIndex: Record<number, string[]> = {
            1: ['Missing Category'],
            2: ['Missing Category', 'Missing Source Account'],
            3: ['Missing Destination Account']
        };

        const summary = collectImportTransactionSelectionSummary(
            rows,
            transaction => issuesByIndex[transaction.index] || []
        );

        expect(summary.selectedCount).toBe(2);
        expect(summary.selectedExpenseCount).toBe(1);
        expect(summary.selectedIncomeCount).toBe(1);
        expect(summary.selectedTransferCount).toBe(0);
        expect(summary.selectedRecurringMatchCount).toBe(1);
        expect(summary.selectedInvalidCount).toBe(1);
        expect(summary.annotationCount).toBe(3);
        expect(summary.selectedAnnotationCount).toBe(2);
        expect(summary.selectedAnnotationTransactions.map(transaction => transaction.index)).toStrictEqual([1, 2]);
        expect(summary.annotationReasonSummaries).toStrictEqual([
            { key: 'Missing Category', label: 'Missing Category', count: 2 },
            { key: 'Missing Source Account', label: 'Missing Source Account', count: 1 }
        ]);
    });

    test('keeps current-page selection objects live over cached server-paged drafts', () => {
        const cachedDraft = selectionRow(1, TransactionType.Expense, true);
        (cachedDraft as { _previewId?: number })._previewId = 101;
        const currentPageTransaction = selectionRow(2, TransactionType.Expense, false);
        (currentPageTransaction as { _previewId?: number })._previewId = 101;

        const trackedTransactions = collectTrackedImportTransactionsForSelection(
            [[101, cachedDraft]],
            [currentPageTransaction],
            previewIdOf
        );

        expect(trackedTransactions).toHaveLength(1);
        expect(trackedTransactions[0]).toBe(currentPageTransaction);
        expect(collectImportTransactionSelectionSummary(trackedTransactions, () => []).selectedCount).toBe(0);

        currentPageTransaction.selected = true;

        expect(collectImportTransactionSelectionSummary(trackedTransactions, () => []).selectedCount).toBe(1);
    });
});
