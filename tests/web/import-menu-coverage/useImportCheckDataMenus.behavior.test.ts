import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const { computed, ref } = jest.requireActual('vue') as any;

const mockDateRange = {
    All: { type: 0 },
    ThisWeek: { type: 1 },
    ThisMonth: { type: 2 },
    ThisYear: { type: 3 },
    Custom: { type: 4 }
} as const;
const mockTransactionType = { ModifyBalance: 1, Income: 2, Expense: 3, Transfer: 4, Investment: 5 } as const;
const mockResolveRange = jest.fn((type: number, firstDay: number, fiscalStart: number) => ({
    minDatetime: type * 100 + firstDay,
    maxDatetime: type * 100 + fiscalStart
}));

jest.mock('@/core/datetime.ts', () => ({ DateRange: mockDateRange }));
jest.mock('@/core/transaction.ts', () => ({ TransactionType: mockTransactionType }));
jest.mock('@mdi/js', () => ({
    mdiAutoFix: 'auto', mdiCheck: 'check', mdiFindReplace: 'replace', mdiShapePlusOutline: 'create', mdiTransfer: 'transfer'
}));
jest.mock('@/views/desktop/transactions/import/checkDataFilters.ts', () => ({
    resolveImportCheckDatePresetRange: (type: number, firstDay: number, fiscalStart: number) => mockResolveRange(type, firstDay, fiscalStart)
}));

const { useImportCheckDataMenus } = require('@/views/desktop/transactions/import/check-data-tab/useImportCheckDataMenus.ts') as {
    useImportCheckDataMenus: (options: any) => { filterMenus: any; toolMenus: any };
};

function createOptions(): any {
    const invalid = [{ name: 'Invalid', value: 'invalid' }];
    return {
        allInvalidAccountNames: ref([...invalid]),
        allInvalidExpenseCategoryNames: ref([...invalid]),
        allInvalidIncomeCategoryNames: ref([...invalid]),
        allInvalidTransactionTagNames: ref([...invalid]),
        allInvalidTransferCategoryNames: ref([...invalid]),
        allOriginalTransactionTagNames: ref([{ name: 'Original', value: 'original' }]),
        allUsedAccountFilterGroups: ref([{ title: 'Assets', labels: ['Wallet', 'Bank'] }]),
        allUsedCategoryFilterGroups: ref([{ title: 'Food', labels: ['Cafe', 'Market'] }]),
        allUsedTagNames: ref(['Daily', 'Work']),
        clearSelectedRecurringMatches: jest.fn(),
        convertTransactionType: jest.fn(),
        currentDateFilterType: computed(() => mockDateRange.ThisWeek.type),
        currentDescriptionFilterValue: ref(null as string | null),
        displayFilterCustomDateRange: computed(() => '2026-01-01 - 2026-01-31'),
        filters: ref({
            annotation: null,
            signal: null,
            minDatetime: 0,
            maxDatetime: 0,
            transactionType: null,
            category: null,
            account: null,
            tag: null,
            description: null
        }),
        firstDayOfWeek: computed(() => 1),
        fiscalYearStartValue: computed(() => 4),
        getAnnotationFilterTitle: () => 'Annotation',
        getNeedsReviewOrAnnotatedText: () => 'Needs review',
        getNoAnnotationIssuesText: () => 'No issues',
        isEditing: ref(false),
        selectedExpenseTransactionCount: ref(1),
        selectedImportTransactionCount: ref(1),
        selectedIncomeTransactionCount: ref(1),
        selectedRecurringMatchCount: ref(1),
        selectedTransferTransactionCount: ref(1),
        showBatchAddDialog: jest.fn(),
        showBatchCreateInvalidItemDialog: jest.fn(),
        showBatchReplaceDialog: jest.fn(),
        showCustomDateRangeDialog: ref(false),
        showCustomDescriptionDialog: ref(false),
        showReplaceAllTypesDialog: jest.fn(),
        showReplaceInvalidItemDialog: jest.fn(),
        tt: (key: string) => `tt:${key}`
    };
}

function findMenu(groups: any[], title: string): any {
    const menu = groups.find(group => group.title === title);
    if (!menu) throw new Error(`missing menu ${title}`);
    return menu;
}

function invokeMenuItems(items: any[]): void {
    for (const item of items) {
        item.onClick?.();
        if (item.items) invokeMenuItems(item.items);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
});

describe('useImportCheckDataMenus production behavior', () => {
    test('derives every filter summary and executes all nested filter actions', () => {
        const options = createOptions();
        let menus = useImportCheckDataMenus(options).filterMenus.value;
        expect(findMenu(menus, 'Annotation').summary).toBe('tt:All');
        expect(findMenu(menus, 'tt:Signals').summary).toBe('tt:All');
        expect(findMenu(menus, 'tt:Date Range').summary).toBe('tt:This week');
        expect(findMenu(menus, 'tt:Type').summary).toBe('tt:All');
        expect(findMenu(menus, 'tt:Category').summary).toBe('tt:All');
        expect(findMenu(menus, 'tt:Account').summary).toBe('tt:All');
        expect(findMenu(menus, 'tt:Tags').summary).toBe('tt:All');
        expect(findMenu(menus, 'tt:Description').summary).toBe('tt:All');

        invokeMenuItems(menus.flatMap((group: any) => group.items));
        expect(mockResolveRange).toHaveBeenCalledTimes(4);
        expect(options.filters.value.minDatetime).toBe(301);
        expect(options.filters.value.maxDatetime).toBe(304);
        expect(options.showCustomDateRangeDialog.value).toBe(true);
        expect(options.showCustomDescriptionDialog.value).toBe(true);
        expect(options.currentDescriptionFilterValue.value).toBe('');
        expect(options.filters.value).toEqual(expect.objectContaining({
            annotation: 'no-issues', signal: 'llm', transactionType: mockTransactionType.Investment,
            category: 'Market', account: 'Bank', tag: 'Work', description: ''
        }));
    });

    test.each([
        [mockDateRange.ThisWeek.type, 'tt:This week'],
        [mockDateRange.ThisMonth.type, 'tt:This month'],
        [mockDateRange.ThisYear.type, 'tt:This year'],
        [mockDateRange.Custom.type, '2026-01-01 - 2026-01-31'],
        [999, 'tt:All']
    ])('maps date preset %s summary', (dateType, summary) => {
        const options = createOptions();
        options.currentDateFilterType = computed(() => dateType);
        expect(findMenu(useImportCheckDataMenus(options).filterMenus.value, 'tt:Date Range').summary).toBe(summary);
    });

    test.each([
        [mockTransactionType.Income, 'tt:Income'],
        [mockTransactionType.Expense, 'tt:Expense'],
        [mockTransactionType.Transfer, 'tt:Transfer'],
        [mockTransactionType.Investment, 'tt:Investment'],
        [null, 'tt:All']
    ])('maps transaction type %s summary', (type, summary) => {
        const options = createOptions();
        options.filters.value.transactionType = type;
        expect(findMenu(useImportCheckDataMenus(options).filterMenus.value, 'tt:Type').summary).toBe(summary);
    });

    test.each([
        ['parser', 'tt:Parser'],
        ['platform_duplicate', 'tt:Platform Duplicate'],
        ['transfer', 'tt:Transfer Match'],
        ['history', 'tt:History Rewrite'],
        ['learning', 'tt:Learning Suggestion'],
        ['llm', 'tt:LLM Suggestion'],
        [null, 'tt:All']
    ])('maps signal %s summary', (signal, summary) => {
        const options = createOptions();
        options.filters.value.signal = signal;
        expect(findMenu(useImportCheckDataMenus(options).filterMenus.value, 'tt:Signals').summary).toBe(summary);
    });

    test('maps annotation, invalid, none, custom, and description summaries', () => {
        const options = createOptions();
        const menu = () => useImportCheckDataMenus(options).filterMenus.value;
        options.filters.value.annotation = 'needs-review';
        expect(findMenu(menu(), 'Annotation').summary).toBe('Needs review');
        options.filters.value.annotation = 'no-issues';
        expect(findMenu(menu(), 'Annotation').summary).toBe('No issues');
        options.filters.value.annotation = 'other';
        expect(findMenu(menu(), 'Annotation').summary).toBe('tt:All');

        options.filters.value.category = undefined;
        expect(findMenu(menu(), 'tt:Category').summary).toBe('tt:Invalid Category');
        options.filters.value.category = '';
        expect(findMenu(menu(), 'tt:Category').summary).toBe('tt:None');
        options.filters.value.category = 'Cafe';
        expect(findMenu(menu(), 'tt:Category').summary).toBe('Cafe');
        options.filters.value.description = '';
        expect(findMenu(menu(), 'tt:Description').summary).toBe('tt:None');
        options.filters.value.description = 'coffee';
        expect(findMenu(menu(), 'tt:Description').summary).toBe('coffee');
    });

    test('executes every batch tool with enabled inputs and preserves typed arguments', () => {
        const options = createOptions();
        const tools = useImportCheckDataMenus(options).toolMenus.value;
        expect(tools).toHaveLength(24);
        expect(tools.every((tool: any) => tool.disabled === false)).toBe(true);
        invokeMenuItems(tools);
        expect(options.showBatchReplaceDialog).toHaveBeenCalledWith('expenseCategory');
        expect(options.showBatchReplaceDialog).toHaveBeenCalledWith('tag', options.allOriginalTransactionTagNames.value);
        expect(options.showBatchAddDialog).toHaveBeenCalledWith('tag');
        expect(options.showReplaceInvalidItemDialog).toHaveBeenCalledTimes(5);
        expect(options.showReplaceAllTypesDialog).toHaveBeenCalledTimes(1);
        expect(options.showBatchCreateInvalidItemDialog).toHaveBeenCalledTimes(4);
        expect(options.convertTransactionType).toHaveBeenCalledTimes(6);
        expect(options.clearSelectedRecurringMatches).toHaveBeenCalledTimes(1);
    });

    test('disables every mutation while editing and on empty selections or invalid lists', () => {
        const options = createOptions();
        options.isEditing.value = true;
        options.selectedExpenseTransactionCount.value = 0;
        options.selectedImportTransactionCount.value = 0;
        options.selectedIncomeTransactionCount.value = 0;
        options.selectedRecurringMatchCount.value = 0;
        options.selectedTransferTransactionCount.value = 0;
        options.allInvalidAccountNames.value = undefined;
        options.allInvalidExpenseCategoryNames.value = [];
        options.allInvalidIncomeCategoryNames.value = undefined;
        options.allInvalidTransactionTagNames.value = [];
        options.allInvalidTransferCategoryNames.value = undefined;
        const tools = useImportCheckDataMenus(options).toolMenus.value;
        expect(tools.every((tool: any) => tool.disabled === true)).toBe(true);
    });
});
