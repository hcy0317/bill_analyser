import { describe, expect, test } from '@jest/globals';

import {
    collectImportPreviewIndexAnnotationIssues,
    matchesImportPreviewIndexItemFilters,
    resolveImportPreviewIndexPage,
    sortImportPreviewIndexItems,
    type ImportPreviewIndexItem,
} from '@/views/desktop/transactions/import/importPreviewIndex.ts';

const baseItem = (): ImportPreviewIndexItem => ({
    id: 0,
    time: 0,
    type: 3,
    actualCategoryName: '',
    categoryId: '',
    actualSourceAccountName: '',
    actualDestinationAccountName: '',
    sourceAccountId: '',
    destinationAccountId: '',
    tagIds: [],
    originalTagNames: [],
    comment: '',
    isManuallyAnnotated: false,
    selected: false,
    sourceAmount: 0,
    counterparty: '',
    paymentMethod: '',
    parserSource: '',
    parserTags: [],
    dedupType: '',
    dedupSourceIds: [],
    transferStatus: null,
    transferTitle: '',
    learningStatus: null,
    learningTitle: '',
    learningSummary: '',
    recurringTemplateId: '',
    recurringCandidateCount: 0,
    recurringMatchReasons: '',
    recurringMatchedDate: '',
});

describe('import preview index helpers', () => {
    test('counts annotation issues from rows that were never visited in the table', () => {
        const issues = collectImportPreviewIndexAnnotationIssues({
            ...baseItem(),
            id: 9,
            type: 4,
            sourceAccountId: '100',
            destinationAccountId: '100',
            actualSourceAccountName: '支付宝',
            actualDestinationAccountName: '支付宝',
        });

        expect(issues).toContain('Missing Category');
        expect(issues).toContain('Review Transfer Accounts');
    });

    test('filters globally across the full index instead of the current detail page only', () => {
        const items: ImportPreviewIndexItem[] = [
            {
                ...baseItem(),
                id: 1,
                time: 100,
                type: 3,
                categoryId: '10',
                actualCategoryName: '餐饮',
                sourceAccountId: '200',
                actualSourceAccountName: '支付宝',
                comment: '早餐',
            },
            {
                ...baseItem(),
                id: 2,
                time: 200,
                type: 2,
                categoryId: '20',
                actualCategoryName: '工资',
                sourceAccountId: '201',
                actualSourceAccountName: '招商银行卡',
                comment: '工资',
                parserSource: 'alipay',
            },
            {
                ...baseItem(),
                id: 3,
                time: 300,
                type: 3,
                categoryId: '',
                actualCategoryName: '',
                sourceAccountId: '',
                actualSourceAccountName: '',
                comment: '待处理',
                learningStatus: 'pending',
                learningTitle: '历史命中',
            },
        ];

        const filtered = items.filter(item => matchesImportPreviewIndexItemFilters(item, {
            minDatetime: null,
            maxDatetime: null,
            transactionType: null,
            category: null,
            account: null,
            tag: null,
            signal: 'learning',
            annotation: 'needs-review',
            description: null,
        }));

        expect(filtered.map(item => item.id)).toStrictEqual([3]);
    });

    test('sorts and slices ids for server-paged requests from the filtered global index', () => {
        const sorted = sortImportPreviewIndexItems([
            { ...baseItem(), id: 1, time: 200, sourceAmount: 2000, comment: 'b' },
            { ...baseItem(), id: 2, time: 100, sourceAmount: 3000, comment: 'c' },
            { ...baseItem(), id: 3, time: 300, sourceAmount: 1000, comment: 'a' },
        ], 'time', 'asc');

        expect(sorted.map(item => item.id)).toStrictEqual([2, 1, 3]);

        const page = resolveImportPreviewIndexPage(sorted, 2, 2);
        expect(page.totalCount).toBe(3);
        expect(page.totalPages).toBe(2);
        expect(page.page).toBe(2);
        expect(page.previewIds).toStrictEqual([3]);
    });
});
