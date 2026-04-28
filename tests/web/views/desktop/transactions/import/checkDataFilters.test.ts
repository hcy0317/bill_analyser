import { describe, expect, test } from '@jest/globals';

import { DateRange } from '@/core/datetime.ts';
import { buildImportPreviewSignalViewModel } from '@/views/desktop/transactions/import/checkDataMatching.ts';
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

    test('applies existing annotation, signal, and date filters to visible-row calculation', () => {
        const rows: TestRow[] = [
            { id: 1, time: 100, type: 5, comment: 'parser-row', signalState: { parserSource: 'alipay' } },
            { id: 2, time: 200, type: 5, comment: 'learning-row', signalState: { learningStatus: 'pending', learningSummary: '支出 | 餐饮/咖啡' } },
            { id: 3, time: 300, type: 5, comment: 'manual-row', isManuallyAnnotated: true }
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
        expect(annotatedRows.map(row => row.id)).toStrictEqual([3]);
    });
});
