import { describe, expect, test } from '@jest/globals';

import { DateRange } from '@/core/datetime.ts';
import { TransactionType } from '@/core/transaction.ts';
import type { ImportPreviewSignalViewModel } from '@/views/desktop/transactions/import/checkDataMatching.ts';
import {
    getImportCheckVisibleTransactions,
    matchesImportTransactionCheckDataFilters,
    resolveImportCheckDatePresetRange,
    resolveImportCheckDatePresetType,
    type ImportCheckDataFilterLike,
    type ImportCheckVisibleTransactionContext,
    type ImportCheckVisibleTransactionLike
} from '@/views/desktop/transactions/import/checkDataFilters.ts';

interface TestRow extends ImportCheckVisibleTransactionLike {
    id: number;
    hasIssues?: boolean;
    editing?: boolean;
    signalView?: ImportPreviewSignalViewModel;
}

const EMPTY_SIGNAL: ImportPreviewSignalViewModel = {
    parser: null,
    dedup: null,
    isManuallyAnnotated: false,
    historyRewrite: null,
    transferSuggestion: null,
    investment: null,
    learning: null,
    llm: null,
    recurring: null,
    hasAnySignal: false
};

const BASE_FILTERS: ImportCheckDataFilterLike = {
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

const CONTEXT: ImportCheckVisibleTransactionContext<TestRow> = {
    tagNameById: { valid: { name: 'Known tag' } },
    hasAnnotationIssues: row => !!row.hasIssues,
    isEditing: row => !!row.editing,
    signalViewModelFor: row => row.signalView ?? EMPTY_SIGNAL
};

function row(overrides: Partial<TestRow> = {}): TestRow {
    return {
        id: 1,
        time: 100,
        type: TransactionType.Expense,
        comment: '',
        ...overrides
    };
}

function matches(transaction: TestRow, overrides: Partial<ImportCheckDataFilterLike>): boolean {
    return matchesImportTransactionCheckDataFilters(transaction, { ...BASE_FILTERS, ...overrides }, CONTEXT);
}

describe('import check-data filter behavior', () => {
    test('falls back to all dates for partial or unsupported presets', () => {
        expect(resolveImportCheckDatePresetRange(DateRange.All.type, 1, 1)).toEqual({
            minDatetime: null,
            maxDatetime: null
        });
        expect(resolveImportCheckDatePresetRange(999_999, 1, 1)).toEqual({
            minDatetime: null,
            maxDatetime: null
        });
        expect(resolveImportCheckDatePresetType(null, 100, 1, 1)).toBe(DateRange.All.type);
        expect(resolveImportCheckDatePresetType(100, null, 1, 1)).toBe(DateRange.All.type);
    });

    test('rejects rows outside date, type, category, and account constraints', () => {
        expect(matches(row({ time: 9 }), { minDatetime: 10, maxDatetime: 20 })).toBe(false);
        expect(matches(row({ time: 21 }), { minDatetime: 10, maxDatetime: 20 })).toBe(false);
        expect(matches(row(), { transactionType: TransactionType.Income })).toBe(false);

        expect(matches(row({ actualCategoryName: 'Food' }), { category: '' })).toBe(false);
        expect(matches(row({ actualCategoryName: 'Food' }), { category: 'Travel' })).toBe(false);
        expect(matches(row({ categoryId: 'cat-1' }), { category: undefined })).toBe(false);
        expect(matches(row({ type: TransactionType.ModifyBalance, categoryId: 'cat-1' }), { category: undefined })).toBe(true);

        expect(matches(row({ actualSourceAccountName: 'Cash' }), { account: '' })).toBe(false);
        expect(matches(row({ actualDestinationAccountName: 'Bank' }), { account: '' })).toBe(false);
        expect(matches(row({ actualSourceAccountName: 'Cash' }), { account: 'Bank' })).toBe(false);
        expect(matches(row({ sourceAccountId: 'cash' }), { account: undefined })).toBe(false);
        expect(matches(row({
            type: TransactionType.Transfer,
            sourceAccountId: 'cash',
            destinationAccountId: 'bank'
        }), { account: undefined })).toBe(false);
        expect(matches(row({
            type: TransactionType.Transfer,
            sourceAccountId: 'cash',
            destinationAccountId: '0'
        }), { account: undefined })).toBe(true);
    });

    test('resolves tag names from canonical and original sources and detects invalid identities', () => {
        expect(matches(row({ tagIds: ['valid'] }), { tag: '' })).toBe(false);
        expect(matches(row({ tagIds: ['valid'] }), { tag: 'Known tag' })).toBe(true);
        expect(matches(row({ tagIds: ['legacy'], originalTagNames: ['Legacy tag'] }), { tag: 'Legacy tag' })).toBe(true);
        expect(matches(row({ tagIds: ['missing'], originalTagNames: ['Other'] }), { tag: 'Unknown' })).toBe(false);
        expect(matches(row({ tagIds: ['valid'] }), { tag: undefined })).toBe(false);
        expect(matches(row({ tagIds: ['0'] }), { tag: undefined })).toBe(true);
        expect(matches(row({ tagIds: [''] }), { tag: undefined })).toBe(true);
        expect(matches(row({ tagIds: [] }), { tag: undefined })).toBe(false);
        expect(matches(row(), { tag: 'Known tag' })).toBe(false);
    });

    test('combines annotation, signal, and description behavior without bypassing editing rows', () => {
        expect(matches(row({ hasIssues: false }), { annotation: 'needs-review' })).toBe(false);
        expect(matches(row({ hasIssues: true }), { annotation: 'no-issues' })).toBe(false);
        expect(matches(row({ hasIssues: true, editing: true }), { annotation: 'no-issues' })).toBe(true);

        expect(matches(row(), { signal: 'parser' })).toBe(false);
        expect(matches(row({
            signalView: {
                ...EMPTY_SIGNAL,
                parser: {
                    parserId: 'alipay',
                    label: 'Alipay',
                    color: 'primary',
                    title: 'Alipay',
                    detailLines: []
                },
                hasAnySignal: true
            }
        }), { signal: 'parser' })).toBe(true);

        expect(matches(row({ comment: 'memo' }), { description: '' })).toBe(false);
        expect(matches(row({ comment: 'monthly coffee' }), { description: 'tea' })).toBe(false);
        expect(matches(row({ comment: 'monthly coffee' }), { description: 'coffee' })).toBe(true);
    });

    test('paginates only client-owned collections and normalizes invalid page values', () => {
        const rows = [row({ id: 1 }), row({ id: 2 }), row({ id: 3 }), row({ id: 4 })];
        const displayed = (item: TestRow): boolean => item.id !== 2;

        expect(getImportCheckVisibleTransactions(rows, displayed, {
            serverPaged: true,
            currentPage: 99,
            countPerPage: 1
        }).map(item => item.id)).toEqual([1, 3, 4]);
        expect(getImportCheckVisibleTransactions(rows, displayed, {
            serverPaged: false,
            currentPage: 2,
            countPerPage: -1
        }).map(item => item.id)).toEqual([1, 3, 4]);
        expect(getImportCheckVisibleTransactions(rows, displayed, {
            serverPaged: false,
            currentPage: 0,
            countPerPage: 0
        }).map(item => item.id)).toEqual([1]);
        expect(getImportCheckVisibleTransactions(rows, displayed, {
            serverPaged: false,
            currentPage: 2,
            countPerPage: 2
        }).map(item => item.id)).toEqual([4]);
    });
});
