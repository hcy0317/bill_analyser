import { describe, expect, test } from '@jest/globals';

import {
    PREVIEW_FILTER_INVALID_VALUE,
    PREVIEW_FILTER_NONE_VALUE,
    buildImportPreviewServerQueryFilters,
    collectImportPreviewIndexAnnotationIssues,
    groupImportPreviewAccountFilterLabels,
    groupImportPreviewCategoryFilterLabels,
    isImportPreviewServerPagedSortableColumn,
    mapImportPreviewIndexResponseItem,
    matchesImportPreviewIndexItemFilters,
    normalizePreviewPage,
    normalizePreviewPageSize,
    normalizePreviewTableSortDirection,
    normalizePreviewTableSortItems,
    resolveServerPagedSelectionCount,
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

    test('maps server-paged index responses into the table filter model', () => {
        const item = mapImportPreviewIndexResponseItem({
            id: 42,
            preview_date: '2026-05-01T00:00:00Z',
            type: 3,
            source_amount: 1288,
            category_id: 'c-food',
            actual_category_name: '餐饮',
            source_account_id: 'acc-1',
            actual_source_account_name: '支付宝',
            comment: '早餐',
            selected: true,
            parser_source: 'alipay',
            parser_tags: ['parser:alipay'],
            transfer_status: 'accepted',
            recurring_candidate_count: 2
        });

        expect(item).toMatchObject({
            id: 42,
            time: new Date('2026-05-01T00:00:00Z').getTime() / 1000,
            type: 3,
            sourceAmount: 1288,
            categoryId: 'c-food',
            actualCategoryName: '餐饮',
            sourceAccountId: 'acc-1',
            actualSourceAccountName: '支付宝',
            comment: '早餐',
            selected: true,
            parserSource: 'alipay',
            parserTags: ['parser:alipay'],
            transferStatus: 'accepted',
            recurringCandidateCount: 2
        });
    });

    test('clamps server-paged selected count to the preview total', () => {
        expect(resolveServerPagedSelectionCount({
            total: 13195,
            selected: 13195,
            delta: 3
        })).toBe(13195);

        expect(resolveServerPagedSelectionCount({
            total: 13195,
            selected: 13195,
            delta: -1
        })).toBe(13194);

        expect(resolveServerPagedSelectionCount({
            total: 13195,
            selected: 0,
            delta: -5
        })).toBe(0);
    });

    test('groups category and account filters under their management parents', () => {
        expect(groupImportPreviewCategoryFilterLabels([
            '餐饮',
            '工资',
            '未知分类'
        ], {
            1: [
                {
                    name: '日常支出',
                    subCategories: [
                        { name: '餐饮' },
                        { name: '交通' }
                    ]
                }
            ],
            2: [
                {
                    name: '工作收入',
                    subCategories: [
                        { name: '工资' }
                    ]
                }
            ]
        }, 'Other')).toStrictEqual([
            { title: '日常支出', labels: ['餐饮'] },
            { title: '工作收入', labels: ['工资'] },
            { title: 'Other', labels: ['未知分类'] }
        ]);

        expect(groupImportPreviewAccountFilterLabels([
            '招商银行卡',
            '余额宝',
            '未知账户'
        ], [
            { name: '招商银行卡', category: 2 },
            {
                name: '支付宝',
                category: 4,
                subAccounts: [
                    { name: '余额宝' }
                ]
            }
        ], [
            { type: 2, name: 'Checking Account' },
            { type: 4, name: 'Virtual Account' }
        ], 'Other')).toStrictEqual([
            { title: 'Checking Account', labels: ['招商银行卡'] },
            { title: 'Virtual Account', labels: ['余额宝'] },
            { title: 'Other', labels: ['未知账户'] }
        ]);
    });

    test('normalizes server-paged table page and sort request state', () => {
        expect(normalizePreviewPage('2.9')).toBe(2);
        expect(normalizePreviewPage(0)).toBe(1);
        expect(normalizePreviewPageSize(-1, { serverPaged: false })).toBe(-1);
        expect(normalizePreviewPageSize(-1, { serverPaged: true })).toBe(1);
        expect(normalizePreviewTableSortDirection('DESC')).toBe('desc');
        expect(normalizePreviewTableSortDirection(true)).toBe('asc');
        expect(isImportPreviewServerPagedSortableColumn('time')).toBe(true);
        expect(isImportPreviewServerPagedSortableColumn('actualCategoryName')).toBe(false);

        expect(normalizePreviewTableSortItems([
            { key: 'actualCategoryName', order: 'desc' },
            { key: 'time', order: 'desc' }
        ], { serverPaged: true })).toStrictEqual([
            { key: 'time', order: 'desc' }
        ]);

        expect(normalizePreviewTableSortItems([
            { key: 'actualCategoryName', order: 'desc' },
            { value: 'time', order: 'desc' }
        ], { serverPaged: false })).toStrictEqual([
            { key: 'actualCategoryName', order: 'desc' },
            { key: 'time', order: 'desc' }
        ]);
    });

    test('maps check-data filters into backend preview query parameters', () => {
        const filters = buildImportPreviewServerQueryFilters({
            minDatetime: 1777636800,
            maxDatetime: null,
            transactionType: 3,
            category: undefined,
            account: '支付宝',
            tag: '',
            signal: 'learning',
            annotation: 'needs-review',
            description: '早餐',
        }, {
            accountIdByName: {
                支付宝: '200'
            }
        });

        expect(filters).toMatchObject({
            minDatetime: '2026-05-01 12:00:00',
            transactionType: '支出',
            category: PREVIEW_FILTER_INVALID_VALUE,
            account: '200',
            tag: PREVIEW_FILTER_NONE_VALUE,
            signal: 'learning',
            annotation: 'needs-review',
            description: '早餐'
        });
    });
});
