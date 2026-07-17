import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockPageType = {
    List: { type: 1 },
    Calendar: { type: 2 },
    valueOf: jest.fn((value: number) => value === 1 || value === 2 ? { type: value } : null)
};

const mockTransactionType = {
    Income: 2,
    Expense: 3,
    Transfer: 4
};

const mockDateRange = {
    All: { type: 0 },
    Custom: { type: 99 },
    isBillingCycle: jest.fn((value: number) => value === 77)
};

const mockAmountFilterType = {
    valueOf: jest.fn((value: string) => {
        const counts: Record<string, number> = { equals: 1, between: 2, impossible: 3 };
        return Object.prototype.hasOwnProperty.call(counts, value) ? { paramCount: counts[value] } : null;
    })
};

const mockRouter = { push: jest.fn() };
const mockDisplay = { mdAndUp: null as any };
const mockTheme = { global: { name: null as any } };
const mockRouteCallbacks: Array<(to: any) => void> = [];
const mockWatchCallbacks: Array<(newValue: any, oldValue: any) => void> = [];

const mockGetRecentRanges = jest.fn<(...args: any[]) => any[]>();
const mockGetTagFilterTypes = jest.fn<() => any[]>();
const mockGetRecentDateRangeIndex = jest.fn<(...args: any[]) => number>();
const mockGetDateRangeByDateType = jest.fn<(...args: any[]) => any>();
const mockGetDateRangeByBillingCycleDateType = jest.fn<(...args: any[]) => any>();
const mockGetDateTypeByDateRange = jest.fn<(...args: any[]) => any>();
const mockGetDateTypeByBillingCycleDateRange = jest.fn<(...args: any[]) => any>();
const mockGetFullMonthDateRange = jest.fn<(...args: any[]) => any>();
const mockGetShiftedDateRange = jest.fn<(...args: any[]) => any>();
const mockGetShiftedBillingRange = jest.fn<(...args: any[]) => any>();
const mockGetCurrentUnixTime = jest.fn<() => number>();
const mockGetActualUnixTimeForStore = jest.fn<(...args: any[]) => number>();
const mockGetBrowserTimezoneOffsetMinutes = jest.fn<() => number>();
const mockGetDayFirstUnixTime = jest.fn<(value: number) => number>();
const mockGetYearMonthFirstUnixTime = jest.fn<(...args: any[]) => number>();
const mockGetYearMonthLastUnixTime = jest.fn<(...args: any[]) => number>();
const mockGetValidMonthDay = jest.fn<(...args: any[]) => string>();
const mockParseDateTime = jest.fn<(...args: any[]) => any>();
const mockScrollToSelectedItem = jest.fn();
const mockStartDownloadFile = jest.fn();

const mockLogger = {
    debug: jest.fn(),
    error: jest.fn(),
    info: jest.fn(),
    warn: jest.fn()
};

let mockLastBase: any;

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const query = ref({
        dateType: 10,
        minTime: 100,
        maxTime: 200,
        type: mockTransactionType.Expense,
        categoryIds: '',
        accountIds: '',
        tagIds: '',
        tagFilterType: 0,
        amountFilterCents: '',
        keyword: ''
    });
    const queryCategoryMap = ref({} as Record<string, boolean>);
    const queryAccountMap = ref({} as Record<string, boolean>);
    const queryTagMap = ref({} as Record<string, boolean>);

    return {
        pageType: ref(mockPageType.List.type),
        loading: ref(false),
        customMinDatetime: ref(0),
        customMaxDatetime: ref(0),
        currentCalendarDate: ref('2026-07-15'),
        currentTimezoneOffsetMinutes: ref(480),
        firstDayOfWeek: ref(1),
        fiscalYearStart: ref(1),
        defaultCurrency: ref('CNY'),
        showTotalAmountInTransactionListPage: ref(true),
        showTagInTransactionListPage: ref(true),
        allDateRanges: ref([]),
        allAccounts: ref([]),
        allAccountsMap: ref({}),
        allAvailableAccountsCount: ref(0),
        allCategories: ref({} as Record<string, any>),
        allPrimaryCategories: ref([]),
        allAvailableCategoriesCount: ref(0),
        allTransactionTags: ref([]),
        allAvailableTagsCount: ref(0),
        query,
        queryMinTime: computed(() => query.value.minTime),
        queryMaxTime: computed(() => query.value.maxTime),
        queryMonthlyData: ref(false),
        queryMonth: ref('2026-07'),
        queryAllFilterCategoryIds: queryCategoryMap,
        queryAllFilterAccountIds: queryAccountMap,
        queryAllFilterTagIds: queryTagMap,
        queryAllFilterCategoryIdsCount: computed(() => Object.keys(queryCategoryMap.value).length),
        queryAllFilterAccountIdsCount: computed(() => Object.keys(queryAccountMap.value).length),
        queryAllFilterTagIdsCount: computed(() => Object.keys(queryTagMap.value).length),
        queryAccountName: ref(''),
        queryCategoryName: ref(''),
        queryTagName: ref(''),
        queryAmount: ref(''),
        transactionCalendarMinDate: ref('2026-01-01'),
        transactionCalendarMaxDate: ref('2026-12-31'),
        currentMonthTransactionData: ref(null),
        canAddTransaction: ref(true),
        getDisplayTime: jest.fn(),
        getDisplayLongDate: jest.fn(),
        getDisplayTimezone: jest.fn(),
        getDisplayTimeInDefaultTimezone: jest.fn(),
        getDisplayAmount: jest.fn(),
        getDisplayMonthTotalAmount: jest.fn((amount: number) => `amount:${amount}`),
        getTransactionTypeName: jest.fn()
    };
}

const mockSettingsStore = {
    appSettings: { itemsCountInTransactionListPage: 10 }
};

const mockUserStore = {
    currentUserNickname: 'Alice',
    getExportedUserData: jest.fn<(...args: any[]) => Promise<any>>()
};

const mockAccountsStore = {
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<any>>(),
    getAccountStatementDate: jest.fn(() => 20)
};

const mockCategoryStore = {
    loadAllCategories: jest.fn<(...args: any[]) => Promise<any>>()
};

const mockTagStore = {
    loadAllTags: jest.fn<(...args: any[]) => Promise<any>>()
};

const mockTransactionStore = {
    clearTransactions: jest.fn(),
    getTransactionListPageParams: jest.fn(() => 'page=mock'),
    initTransactionListFilter: jest.fn((value: any) => {
        if (!mockLastBase) return;
        for (const [key, fieldValue] of Object.entries(value)) {
            if (fieldValue !== undefined) {
                mockLastBase.query.value[key] = fieldValue;
            }
        }
    }),
    updateTransactionListFilter: jest.fn((value: any) => {
        if (!mockLastBase) return false;
        let changed = false;
        for (const [key, fieldValue] of Object.entries(value)) {
            if (fieldValue !== undefined && mockLastBase.query.value[key] !== fieldValue) {
                mockLastBase.query.value[key] = fieldValue;
                changed = true;
            }
        }
        return changed;
    }),
    loadMonthlyAllTransactions: jest.fn<(...args: any[]) => Promise<any>>(),
    loadTransactions: jest.fn<(...args: any[]) => Promise<any>>(),
    getExportTransactionDataRequestByTransactionFilter: jest.fn(() => ({ scoped: true }))
};

const mockTemplateStore = {
    allVisibleTemplates: {} as Record<number, any[]>,
    loadAllTemplates: jest.fn()
};

const mockDesktopPageStore = {
    showAddTransactionDialogInTransactionList: false,
    resetShowAddTransactionDialogInTransactionList: jest.fn(() => {
        mockDesktopPageStore.showAddTransactionDialogInTransactionList = false;
    })
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null),
        watch: (_source: any, callback: (newValue: any, oldValue: any) => void) => {
            mockWatchCallbacks.push(callback);
            return jest.fn();
        },
        nextTick: (callback?: () => void) => Promise.resolve().then(() => callback?.())
    };
});

jest.mock('vue-router', () => ({
    useRouter: () => mockRouter,
    onBeforeRouteUpdate: (callback: (to: any) => void) => mockRouteCallbacks.push(callback)
}));

jest.mock('vuetify', () => {
    const { ref } = jest.requireActual('vue') as any;
    mockDisplay.mdAndUp ??= ref(true);
    mockTheme.global.name ??= ref('light');
    return {
        useDisplay: () => mockDisplay,
        useTheme: () => mockTheme
    };
});

jest.mock('vuetify/components/VMenu', () => ({ VMenu: { name: 'VMenu' } }));
jest.mock('@/lib/vue_external_template.ts', () => ({ useExternalTemplateBindings: jest.fn() }));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: any) => values?.nickname ? `${key}:${values.nickname}` : key,
        getAllRecentMonthDateRanges: (...args: any[]) => mockGetRecentRanges(...args),
        getAllTransactionTagFilterTypes: () => mockGetTagFilterTypes(),
        getWeekdayLongName: jest.fn(),
        getCurrentNumeralSystemType: () => ({
            replaceWesternArabicDigitsToLocalizedDigits: (value: string) => `n:${value}`
        })
    })
}));

jest.mock('@/views/base/transactions/TransactionListPageBase.ts', () => ({
    TransactionListPageType: mockPageType,
    useTransactionListPageBase: () => {
        mockLastBase = createBase();
        return mockLastBase;
    }
}));

jest.mock('@/views/base/transactions/TransactionEditPageBase.ts', () => ({
    TransactionEditPageType: { Transaction: 'transaction' }
}));

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionStore }));
jest.mock('@/stores/transactionTemplate.ts', () => ({ useTransactionTemplatesStore: () => mockTemplateStore }));
jest.mock('@/stores/desktopPage.ts', () => ({ useDesktopPageStore: () => mockDesktopPageStore }));

jest.mock('@/core/base.ts', () => ({ keys: (value: any) => Object.keys(value || {}) }));
jest.mock('@/core/datetime.ts', () => ({
    DateRangeScene: { Normal: 'normal' },
    DateRange: mockDateRange
}));
jest.mock('@/core/numeral.ts', () => ({ AmountFilterType: mockAmountFilterType }));
jest.mock('@/core/theme.ts', () => ({ isDarkApplicationTheme: (value: string) => value === 'dark' }));
jest.mock('@/core/transaction.ts', () => ({
    TransactionType: mockTransactionType,
    TransactionTagFilterType: {
        HasAny: { type: 1 },
        HasAll: { type: 2 },
        NotHasAny: { type: 3 },
        NotHasAll: { type: 4 }
    }
}));
jest.mock('@/core/template.ts', () => ({ TemplateType: { Normal: { type: 1 } } }));

jest.mock('@/lib/common.ts', () => ({
    isObject: (value: any) => value !== null && typeof value === 'object',
    isString: (value: any) => typeof value === 'string',
    isNumber: (value: any) => typeof value === 'number'
}));

jest.mock('@/lib/datetime.ts', () => ({
    getCurrentUnixTime: () => mockGetCurrentUnixTime(),
    parseDateTimeFromUnixTime: (...args: any[]) => mockParseDateTime(...args),
    getBrowserTimezoneOffsetMinutes: () => mockGetBrowserTimezoneOffsetMinutes(),
    getActualUnixTimeForStore: (...args: any[]) => mockGetActualUnixTimeForStore(...args),
    getDayFirstUnixTimeBySpecifiedUnixTime: (value: number) => mockGetDayFirstUnixTime(value),
    getYearMonthFirstUnixTime: (...args: any[]) => mockGetYearMonthFirstUnixTime(...args),
    getYearMonthLastUnixTime: (...args: any[]) => mockGetYearMonthLastUnixTime(...args),
    getShiftedDateRangeAndDateType: (...args: any[]) => mockGetShiftedDateRange(...args),
    getShiftedDateRangeAndDateTypeForBillingCycle: (...args: any[]) => mockGetShiftedBillingRange(...args),
    getDateTypeByDateRange: (...args: any[]) => mockGetDateTypeByDateRange(...args),
    getDateTypeByBillingCycleDateRange: (...args: any[]) => mockGetDateTypeByBillingCycleDateRange(...args),
    getDateRangeByDateType: (...args: any[]) => mockGetDateRangeByDateType(...args),
    getDateRangeByBillingCycleDateType: (...args: any[]) => mockGetDateRangeByBillingCycleDateType(...args),
    getRecentDateRangeIndex: (...args: any[]) => mockGetRecentDateRangeIndex(...args),
    getFullMonthDateRange: (...args: any[]) => mockGetFullMonthDateRange(...args),
    getValidMonthDayOrCurrentDayShortDate: (...args: any[]) => mockGetValidMonthDay(...args)
}));

jest.mock('@/lib/category.ts', () => ({
    categoryTypeToTransactionType: (value: number) => value,
    transactionTypeToCategoryType: (value: number) => value >= 2 && value <= 4 ? value + 10 : null
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isDataExportingEnabled: () => true,
    isDataImportingEnabled: () => true,
    isTransactionFromAIImageRecognitionEnabled: () => true
}));
jest.mock('@/lib/ui/common.ts', () => ({ startDownloadFile: (...args: any[]) => mockStartDownloadFile(...args) }));
jest.mock('@/lib/ui/desktop.ts', () => ({ scrollToSelectedItem: (...args: any[]) => mockScrollToSelectedItem(...args) }));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

jest.mock('@mdi/js', () => new Proxy({}, { get: (_target, key) => String(key) }));

for (const componentPath of [
    '@/components/desktop/PaginationButtons.vue',
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/transactions/list/dialogs/EditDialog.vue',
    '@/views/desktop/transactions/list/dialogs/BatchManualEntryDialog.vue',
    '@/views/desktop/transactions/list/dialogs/AIImageRecognitionDialog.vue',
    '@/views/desktop/transactions/import/ImportDialog.vue',
    '@/views/desktop/common/cards/AccountFilterSettingsCard.vue',
    '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue',
    '@/views/desktop/common/cards/TransactionTagFilterSettingsCard.vue'
]) {
    jest.mock(componentPath, () => ({ __esModule: true, default: { name: 'TransactionListCoverageStub' } }));
}

import ListPage from '@/views/desktop/transactions/ListPage.vue';

function makeBindings(props: Record<string, any> = {}): any {
    return (ListPage as any).setup(props, { expose: jest.fn() });
}

async function flushPromises(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) {
        await Promise.resolve();
    }
}

function installDialogs(bindings: any): any {
    const confirm = { open: jest.fn() };
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const edit = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const batch = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const ai = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const importer = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    bindings.confirmDialog.value = confirm;
    bindings.snackbar.value = snackbar;
    bindings.editDialog.value = edit;
    bindings.batchManualEntryDialog.value = batch;
    bindings.aiImageRecognitionDialog.value = ai;
    bindings.importDialog.value = importer;
    return { confirm, snackbar, edit, batch, ai, importer };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockRouteCallbacks.length = 0;
    mockWatchCallbacks.length = 0;
    mockSettingsStore.appSettings.itemsCountInTransactionListPage = 10;
    mockUserStore.currentUserNickname = 'Alice';
    mockDesktopPageStore.showAddTransactionDialogInTransactionList = false;
    mockTemplateStore.allVisibleTemplates = {};
    mockDisplay.mdAndUp.value = true;
    mockTheme.global.name.value = 'light';

    mockGetRecentRanges.mockReturnValue([
        { dateType: 10, minTime: 100, maxTime: 200 },
        { dateType: 20, minTime: 300, maxTime: 400 }
    ]);
    mockGetTagFilterTypes.mockReturnValue([
        { type: 1, displayName: 'Any' },
        { type: 999, displayName: 'Unknown' }
    ]);
    mockGetRecentDateRangeIndex.mockReturnValue(0);
    mockGetDateRangeByDateType.mockImplementation((value?: number) => value ? { dateType: value, minTime: 1000, maxTime: 2000 } : null);
    mockGetDateRangeByBillingCycleDateType.mockReturnValue({ dateType: 77, minTime: 3000, maxTime: 4000 });
    mockGetDateTypeByBillingCycleDateRange.mockReturnValue(null);
    mockGetDateTypeByDateRange.mockReturnValue(55);
    mockGetFullMonthDateRange.mockReturnValue({ dateType: 44, minTime: 5000, maxTime: 6000 });
    mockGetShiftedBillingRange.mockReturnValue(null);
    mockGetShiftedDateRange.mockReturnValue({ dateType: 66, minTime: 7000, maxTime: 8000 });
    mockGetCurrentUnixTime.mockReturnValue(150);
    mockGetActualUnixTimeForStore.mockReturnValue(9000);
    mockGetBrowserTimezoneOffsetMinutes.mockReturnValue(480);
    mockGetDayFirstUnixTime.mockReturnValue(8000);
    mockGetYearMonthFirstUnixTime.mockReturnValue(10_000);
    mockGetYearMonthLastUnixTime.mockReturnValue(20_000);
    mockGetValidMonthDay.mockReturnValue('2026-07-01');
    mockParseDateTime.mockReturnValue({
        getGregorianCalendarYear: () => 2026,
        getGregorianCalendarMonth: () => 6
    });

    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockTagStore.loadAllTags.mockResolvedValue(undefined);
    mockTransactionStore.loadTransactions.mockResolvedValue({ items: [{ id: 'default' }], totalCount: 1 });
    mockTransactionStore.loadMonthlyAllTransactions.mockResolvedValue({ items: [{ id: 'monthly' }], totalCount: 1 });
    mockUserStore.getExportedUserData.mockResolvedValue('csv-data');
});

describe('desktop transaction ListPage production behavior', () => {
    test('covers computed list, calendar, selection, pagination, and cents summary contracts', async () => {
        const bindings = makeBindings();
        await flushPromises();

        expect(bindings.isDarkMode.value).toBe(false);
        mockTheme.global.name.value = 'dark';
        expect(bindings.isDarkMode.value).toBe(true);
        expect(bindings.allPageCounts.value).toHaveLength(7);
        expect(bindings.allPageCounts.value[0]).toStrictEqual({ value: 5, name: 'n:5' });
        expect(bindings.recentMonthDateRanges.value).toHaveLength(2);
        expect(mockGetRecentRanges).toHaveBeenCalledWith(true, true);

        expect(bindings.allTransactionTemplates.value).toStrictEqual([]);
        mockTemplateStore.allVisibleTemplates = { 1: [{ id: 'template-1' }] };
        const templateBindings = makeBindings();
        await flushPromises();
        expect(templateBindings.allTransactionTemplates.value).toStrictEqual([{ id: 'template-1' }]);
        expect(bindings.allTransactionTagFilterTypes.value).toStrictEqual([
            { type: 1, displayName: 'Any', icon: 'mdiPlusBoxMultipleOutline' },
            { type: 999, displayName: 'Unknown', icon: '' }
        ]);

        bindings.query.value.type = mockTransactionType.Income;
        expect(bindings.allowCategoryTypes.value).toBe('12');
        bindings.query.value.type = 1;
        expect(bindings.allowCategoryTypes.value).toBe('');

        bindings.pageType.value = mockPageType.List.type;
        bindings.queryMonthlyData.value = false;
        bindings.currentPageTransactions.value = [{ id: 'direct' }];
        expect(bindings.transactions.value).toStrictEqual([{ id: 'direct' }]);
        bindings.queryMonthlyData.value = true;
        bindings.currentMonthTransactionData.value = null;
        expect(bindings.transactions.value).toStrictEqual([]);
        bindings.currentMonthTransactionData.value = {};
        expect(bindings.transactions.value).toStrictEqual([]);
        bindings.currentMonthTransactionData.value = { items: Array.from({ length: 12 }, (_, index) => ({ id: index })) };
        bindings.currentPage.value = 2;
        expect(bindings.transactions.value).toHaveLength(2);

        bindings.pageType.value = mockPageType.Calendar.type;
        bindings.queryMonthlyData.value = false;
        expect(bindings.transactions.value).toStrictEqual([]);
        bindings.queryMonthlyData.value = true;
        bindings.currentMonthTransactionData.value = null;
        expect(bindings.transactions.value).toStrictEqual([]);
        bindings.currentMonthTransactionData.value = { items: [
            { id: 'same', gregorianCalendarYearDashMonthDashDay: '2026-07-01' },
            { id: 'other', gregorianCalendarYearDashMonthDashDay: '2026-07-02' }
        ] };
        bindings.currentCalendarDate.value = '2026-07-01';
        expect(bindings.transactions.value).toStrictEqual([{ id: 'same', gregorianCalendarYearDashMonthDashDay: '2026-07-01' }]);
        bindings.pageType.value = 999;
        expect(bindings.transactions.value).toStrictEqual([]);

        const category = { id: 'parent', subCategories: [{ id: 'child' }] };
        expect(bindings.getCategoryListItemCheckedClass(category, { parent: true })).toHaveProperty('list-item-selected', true);
        expect(bindings.getCategoryListItemCheckedClass(category, { child: true })).toHaveProperty('has-children-item-selected', true);
        expect(bindings.getCategoryListItemCheckedClass(category, {})).toStrictEqual({});
        expect(bindings.getCategoryListItemCheckedClass({ id: 'leaf' }, {})).toStrictEqual({});

        bindings.queryAllFilterCategoryIds.value = {};
        expect(bindings.queryAllSelectedFilterCategoryIds.value).toBe('');
        bindings.query.value.categoryIds = 'cat-1';
        bindings.queryAllFilterCategoryIds.value = { 'cat-1': true };
        expect(bindings.queryAllSelectedFilterCategoryIds.value).toBe('cat-1');
        bindings.queryAllFilterCategoryIds.value = { 'cat-1': true, 'cat-2': true };
        expect(bindings.queryAllSelectedFilterCategoryIds.value).toBe('multiple');

        for (const [mapName, queryName, computedName] of [
            ['queryAllFilterAccountIds', 'accountIds', 'queryAllSelectedFilterAccountIds'],
            ['queryAllFilterTagIds', 'tagIds', 'queryAllSelectedFilterTagIds']
        ] as Array<[string, string, string]>) {
            bindings[mapName].value = {};
            expect(bindings[computedName].value).toBe('');
            bindings.query.value[queryName] = 'one';
            bindings[mapName].value = { one: true };
            expect(bindings[computedName].value).toBe('one');
            bindings[mapName].value = { one: true, two: true };
            expect(bindings[computedName].value).toBe('multiple');
        }

        bindings.pageType.value = mockPageType.List.type;
        bindings.temporaryCountPerPage.value = null;
        expect(bindings.countPerPage.value).toBe(10);
        bindings.temporaryCountPerPage.value = 5;
        expect(bindings.countPerPage.value).toBe(5);
        bindings.totalCount.value = 11;
        expect(bindings.totalPageCount.value).toBe(3);
        bindings.currentPage.value = 8;
        bindings.queryMonthlyData.value = true;
        bindings.countPerPage.value = 10;
        expect(bindings.currentPage.value).toBe(2);
        bindings.queryMonthlyData.value = false;
        bindings.countPerPage.value = 5;
        bindings.paginationCurrentPage.value = 2;
        expect(bindings.paginationCurrentPage.value).toBe(2);
        expect(bindings.skeletonData.value).toHaveLength(5);
        bindings.pageType.value = mockPageType.Calendar.type;
        expect(bindings.skeletonData.value).toHaveLength(3);

        bindings.queryMonthlyData.value = false;
        expect(bindings.currentMonthTotalAmount.value).toBeNull();
        bindings.queryMonthlyData.value = true;
        bindings.currentMonthTransactionData.value = null;
        expect(bindings.currentMonthTotalAmount.value).toBeNull();
        bindings.currentMonthTransactionData.value = {
            totalAmountCents: {
                incomeCents: 12_345,
                expenseCents: 6_789,
                incompleteIncome: false,
                incompleteExpense: true
            }
        };
        expect(bindings.currentMonthTotalAmount.value).toStrictEqual({ incomeText: 'amount:12345', expenseText: 'amount:6789' });
    });

    test('initializes route props, calendar normalization, route updates, and reload modes', async () => {
        const bindings = makeBindings({
            initDateType: '10',
            initType: '3',
            initCategoryIds: 'cat',
            initAccountIds: 'acc',
            initTagIds: 'tag',
            initTagFilterType: '0',
            initAmountFilterCents: 'equals:1234',
            initKeyword: 'coffee',
            initPageType: '1'
        });
        const dialogs = installDialogs(bindings);
        await flushPromises();
        expect(mockTransactionStore.initTransactionListFilter).toHaveBeenCalled();
        expect(bindings.searchKeyword.value).toBe('coffee');
        expect(mockTemplateStore.loadAllTemplates).toHaveBeenCalledWith({ templateType: 1, force: false });

        mockGetDateRangeByDateType.mockReturnValueOnce(null);
        bindings.init({ initDateType: '77', initMinTime: '3000', initMaxTime: '4000', initPageType: '2' });
        await flushPromises();
        expect(mockTransactionStore.initTransactionListFilter).toHaveBeenLastCalledWith(expect.objectContaining({
            dateType: 77,
            minTime: 3000,
            maxTime: 4000
        }));

        mockGetFullMonthDateRange.mockReturnValueOnce(null);
        bindings.init({ initPageType: '2' });
        await flushPromises();
        bindings.init({ initPageType: '999', initType: '0', initTagFilterType: '-1' });
        await flushPromises();

        mockTransactionStore.updateTransactionListFilter.mockReturnValueOnce(true);
        bindings.init({ initPageType: '2' });
        expect(mockRouter.push).toHaveBeenCalled();

        expect(mockRouteCallbacks).toHaveLength(1);
        mockRouteCallbacks[0]?.({ query: { type: '2', keyword: 'route' } });
        mockRouteCallbacks[0]?.({});
        await flushPromises();

        jest.clearAllMocks();
        bindings.reload(true, false);
        await flushPromises();
        expect(dialogs.snackbar.showMessage).toHaveBeenCalledWith('Data has been updated');
        expect(bindings.loading.value).toBe(false);

        bindings.currentPage.value = 2;
        mockTransactionStore.loadTransactions.mockResolvedValueOnce({ items: [], totalCount: 99 });
        bindings.reload(false, false);
        await flushPromises();
        expect(bindings.currentPageTransactions.value).toStrictEqual([]);

        bindings.queryMonthlyData.value = true;
        bindings.query.value.minTime = 5000;
        bindings.reload(false, false);
        await flushPromises();
        expect(mockTransactionStore.loadMonthlyAllTransactions).toHaveBeenCalledWith({
            year: 2026,
            month: 6,
            autoExpand: true,
            defaultCurrency: 'CNY'
        });

        mockDesktopPageStore.showAddTransactionDialogInTransactionList = true;
        bindings.queryMonthlyData.value = false;
        dialogs.edit.open.mockResolvedValueOnce(undefined);
        bindings.reload(false, true);
        await flushPromises();
        expect(mockDesktopPageStore.resetShowAddTransactionDialogInTransactionList).toHaveBeenCalled();

        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'load failed' });
        bindings.reload(false, false);
        await flushPromises();
        expect(dialogs.snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'load failed' }));
        dialogs.snackbar.showError.mockClear();
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: true });
        bindings.reload(false, false);
        await flushPromises();
        expect(dialogs.snackbar.showError).not.toHaveBeenCalled();
    });

    test('changes date, month, page, type, and shifted ranges through all filter shapes', async () => {
        const bindings = makeBindings();
        installDialogs(bindings);
        await flushPromises();
        jest.clearAllMocks();

        bindings.changePageType(mockPageType.List.type);
        mockGetFullMonthDateRange.mockReturnValueOnce(null);
        bindings.changePageType(mockPageType.Calendar.type);
        bindings.changePageType(mockPageType.Calendar.type);

        bindings.query.value.minTime = 0;
        bindings.query.value.maxTime = 0;
        bindings.pageType.value = mockPageType.List.type;
        bindings.changeDateFilter(mockDateRange.Custom.type);
        expect(bindings.customMaxDatetime.value).toBe(9000);
        expect(bindings.customMinDatetime.value).toBe(8000);
        expect(bindings.showCustomDateRangeDialog.value).toBe(true);
        bindings.query.value.minTime = 123;
        bindings.query.value.maxTime = 456;
        bindings.changeDateFilter({ dateType: 99 });
        expect(bindings.customMinDatetime.value).toBe(123);
        bindings.pageType.value = mockPageType.Calendar.type;
        bindings.changeDateFilter(mockDateRange.Custom.type);
        expect(bindings.showCustomMonthDialog.value).toBe(true);

        bindings.changeDateFilter(77);
        expect(mockGetDateRangeByBillingCycleDateType).toHaveBeenCalled();
        bindings.pageType.value = mockPageType.List.type;
        bindings.changeDateFilter(10);
        mockGetDateRangeByDateType.mockReturnValueOnce(null);
        bindings.changeDateFilter(123);
        bindings.changeDateFilter(null);

        bindings.pageType.value = mockPageType.Calendar.type;
        mockGetFullMonthDateRange.mockReturnValueOnce(null);
        bindings.changeDateFilter({ dateType: 12, minTime: 10, maxTime: 20 });
        bindings.changeDateFilter({ dateType: 13, minTime: 30, maxTime: 40 });
        bindings.query.value = { ...bindings.query.value, dateType: 44, minTime: 5000, maxTime: 6000 };
        bindings.changeDateFilter({ dateType: 44, minTime: 5000, maxTime: 6000 });
        mockTransactionStore.updateTransactionListFilter.mockReturnValueOnce(false);
        bindings.changeDateFilter({ dateType: 14, minTime: 50, maxTime: 60 });

        bindings.changeCustomDateFilter(0, 10);
        bindings.changeCustomDateFilter(10, 0);
        bindings.pageType.value = mockPageType.List.type;
        mockGetDateTypeByBillingCycleDateRange.mockReturnValueOnce(77);
        bindings.changeCustomDateFilter(11, 22);
        mockGetDateTypeByBillingCycleDateRange.mockReturnValueOnce(null);
        bindings.changeCustomDateFilter(33, 44);
        bindings.pageType.value = mockPageType.Calendar.type;
        mockGetFullMonthDateRange.mockReturnValueOnce(null);
        bindings.changeCustomDateFilter(55, 66);
        bindings.changeCustomDateFilter(77, 88);
        bindings.query.value = { ...bindings.query.value, dateType: 44, minTime: 5000, maxTime: 6000 };
        bindings.showCustomDateRangeDialog.value = true;
        bindings.changeCustomDateFilter(5000, 6000);
        expect(bindings.showCustomDateRangeDialog.value).toBe(false);

        bindings.changeCustomMonthDateFilter(null);
        bindings.pageType.value = mockPageType.List.type;
        bindings.changeCustomMonthDateFilter({ year: 2026, month: 6 });
        bindings.pageType.value = mockPageType.Calendar.type;
        bindings.changeCustomMonthDateFilter({ year: 2026, month: 7 });
        bindings.query.value = { ...bindings.query.value, dateType: 55, minTime: 10_000, maxTime: 20_000 };
        bindings.showCustomMonthDialog.value = true;
        bindings.changeCustomMonthDateFilter({ year: 2026, month: 8 });
        expect(bindings.showCustomMonthDialog.value).toBe(false);

        mockGetRecentRanges.mockReturnValueOnce([{ dateType: mockDateRange.All.type }]);
        bindings.recentDateRangeIndex.value = 0;
        bindings.shiftDateRange(100, 200, 1);
        mockGetRecentRanges.mockReturnValue([{ dateType: 10 }]);
        bindings.query.value.dateType = 77;
        mockGetShiftedBillingRange.mockReturnValueOnce({ dateType: 77, minTime: 1, maxTime: 2 });
        bindings.shiftDateRange(100, 200, 1);
        bindings.query.value.dateType = 10;
        bindings.pageType.value = mockPageType.Calendar.type;
        mockGetFullMonthDateRange.mockReturnValueOnce(null);
        bindings.shiftDateRange(100, 200, -1);
        bindings.shiftDateRange(100, 200, 1);

        bindings.allCategories.value = {
            keep: { id: 'keep', type: 12 },
            drop: { id: 'drop', type: 13 }
        };
        bindings.queryAllFilterCategoryIds.value = { keep: true, drop: true, missing: true };
        bindings.query.value.categoryIds = 'keep,drop,missing';
        bindings.changeTypeFilter(mockTransactionType.Income);
        bindings.changeTypeFilter(0);

        bindings.recentDateRangeIndex.value = -1;
        bindings.recentDateRangeIndex.value = 999;
        bindings.queryPageType.value = mockPageType.List.type;
        bindings.queryType.value = mockTransactionType.Expense;
    });

    test('updates category, account, tag, keyword, and cents amount filters', async () => {
        const bindings = makeBindings();
        const { snackbar } = installDialogs(bindings);
        await flushPromises();
        jest.clearAllMocks();

        bindings.query.value.categoryIds = 'same';
        bindings.categoryMenuState.value = true;
        bindings.changeCategoryFilter('same');
        bindings.changeCategoryFilter('new');
        bindings.showFilterCategoryDialog.value = true;
        bindings.changeMultipleCategoriesFilter(false);
        bindings.changeMultipleCategoriesFilter(true);

        bindings.query.value.accountIds = 'same';
        bindings.changeAccountFilter('same');
        bindings.changeAccountFilter('new');
        bindings.showFilterAccountDialog.value = true;
        bindings.changeMultipleAccountsFilter(false);
        bindings.changeMultipleAccountsFilter(true);

        bindings.query.value.tagIds = 'same';
        bindings.changeTagFilter('same');
        bindings.changeTagFilter('new');
        bindings.showFilterTagDialog.value = true;
        bindings.changeMultipleTagsFilter(false);
        bindings.changeMultipleTagsFilter(true);

        bindings.query.value.tagFilterType = 1;
        bindings.changeTagFilterType(1);
        bindings.changeTagFilterType(2);
        bindings.query.value.keyword = 'same';
        bindings.changeKeywordFilter('same');
        bindings.changeKeywordFilter('new');

        bindings.currentAmountFilterType.value = 'equals';
        bindings.onAmountFilterTypeClick('equals');
        expect(bindings.currentAmountFilterType.value).toBe('');
        bindings.onAmountFilterTypeClick('between');
        expect(bindings.currentAmountFilterType.value).toBe('between');
        expect(bindings.getAmountFilterParameterCount('missing')).toBe(0);
        expect(bindings.getAmountFilterParameterCount('equals')).toBe(1);

        bindings.query.value.amountFilterCents = '';
        bindings.changeAmountFilter('missing');
        bindings.currentAmountFilterValue1.value = 1234;
        bindings.changeAmountFilter('equals');
        expect(mockTransactionStore.updateTransactionListFilter).toHaveBeenCalledWith({ amountFilterCents: 'equals:1234' });
        bindings.currentAmountFilterValue1.value = 2000;
        bindings.currentAmountFilterValue2.value = 1000;
        bindings.changeAmountFilter('between');
        expect(snackbar.showMessage).toHaveBeenCalledWith('Incorrect amount range');
        bindings.currentAmountFilterValue2.value = 3000;
        bindings.changeAmountFilter('between');
        expect(mockTransactionStore.updateTransactionListFilter).toHaveBeenCalledWith({ amountFilterCents: 'between:2000:3000' });
        bindings.changeAmountFilter('impossible');
        bindings.query.value.amountFilterCents = 'same';
        bindings.changeAmountFilter('same');
        bindings.query.value.amountFilterCents = '';
        bindings.changeAmountFilter('');
    });

    test('adds, batch-adds, imports, and creates OCR drafts with explicit cents conversion', async () => {
        const bindings = makeBindings();
        const dialogs = installDialogs(bindings);
        await flushPromises();

        bindings.query.value = {
            ...bindings.query.value,
            minTime: 200,
            maxTime: 300,
            type: mockTransactionType.Expense,
            categoryIds: 'cat-1',
            accountIds: 'acc-1',
            tagIds: 'tag-1'
        };
        bindings.queryAllFilterCategoryIds.value = { 'cat-1': true };
        bindings.queryAllFilterAccountIds.value = { 'acc-1': true };
        dialogs.edit.open.mockResolvedValueOnce({ message: 'saved' });
        bindings.add({ id: 'template-1' });
        await flushPromises();
        expect(dialogs.edit.open).toHaveBeenCalledWith(expect.objectContaining({
            time: 200,
            categoryId: 'cat-1',
            accountId: 'acc-1',
            tagIds: 'tag-1'
        }));
        expect(dialogs.snackbar.showMessage).toHaveBeenCalledWith('saved');

        bindings.query.value.minTime = 10;
        bindings.query.value.maxTime = 100;
        bindings.queryAllFilterCategoryIds.value = {};
        bindings.queryAllFilterAccountIds.value = {};
        dialogs.edit.open.mockResolvedValueOnce(undefined);
        bindings.add();
        await flushPromises();
        expect(dialogs.edit.open).toHaveBeenLastCalledWith(expect.objectContaining({ time: 100, categoryId: '', accountId: '' }));
        dialogs.edit.open.mockRejectedValueOnce('add failed');
        bindings.add();
        await flushPromises();
        expect(dialogs.snackbar.showError).toHaveBeenCalledWith('add failed');
        dialogs.edit.open.mockRejectedValueOnce(undefined);
        bindings.add();
        await flushPromises();

        bindings.query.value.minTime = 100;
        bindings.query.value.maxTime = 200;
        dialogs.batch.open.mockResolvedValueOnce({ message: 'batch saved' });
        bindings.batchAdd();
        await flushPromises();
        expect(dialogs.snackbar.showMessage).toHaveBeenCalledWith('batch saved');
        bindings.query.value.minTime = 0;
        bindings.query.value.maxTime = 0;
        dialogs.batch.open.mockResolvedValueOnce(undefined);
        bindings.batchAdd();
        await flushPromises();
        dialogs.batch.open.mockRejectedValueOnce('batch failed');
        bindings.batchAdd();
        await flushPromises();
        dialogs.batch.open.mockRejectedValueOnce(undefined);
        bindings.batchAdd();
        await flushPromises();

        dialogs.importer.open.mockResolvedValueOnce(undefined);
        bindings.importTransaction();
        await flushPromises();
        dialogs.importer.open.mockRejectedValueOnce('import failed');
        bindings.importTransaction();
        await flushPromises();
        expect(dialogs.snackbar.showError).toHaveBeenCalledWith('import failed');
        dialogs.importer.open.mockRejectedValueOnce(undefined);
        bindings.importTransaction();
        await flushPromises();

        dialogs.ai.open.mockResolvedValueOnce({ amount: 12.34, tradeTime: '2026-07-15T10:00:00Z', description: 'coffee' });
        dialogs.edit.open.mockResolvedValueOnce({ message: 'ocr saved' });
        bindings.addByRecognizingImage();
        await flushPromises();
        expect(dialogs.edit.open).toHaveBeenLastCalledWith(expect.objectContaining({
            sourceAmountCents: 1234,
            comment: 'coffee',
            noTransactionDraft: true
        }));

        dialogs.ai.open.mockResolvedValueOnce({ amount: Number.POSITIVE_INFINITY, tradeTime: 'bad-date', description: null });
        dialogs.edit.open.mockResolvedValueOnce(undefined);
        bindings.addByRecognizingImage();
        await flushPromises();
        expect(dialogs.edit.open).toHaveBeenLastCalledWith({
            time: undefined,
            sourceAmountCents: undefined,
            comment: undefined,
            noTransactionDraft: true
        });
        dialogs.ai.open.mockResolvedValueOnce({ amount: null, tradeTime: null });
        dialogs.edit.open.mockRejectedValueOnce('ocr edit failed');
        bindings.addByRecognizingImage();
        await flushPromises();
        expect(dialogs.snackbar.showError).toHaveBeenCalledWith('ocr edit failed');
        dialogs.ai.open.mockResolvedValueOnce({ amount: null, tradeTime: null });
        dialogs.edit.open.mockRejectedValueOnce(undefined);
        bindings.addByRecognizingImage();
        await flushPromises();
    });

    test('exports, shows, and refreshes transactions across success and failure branches', async () => {
        const bindings = makeBindings();
        const dialogs = installDialogs(bindings);
        await flushPromises();

        bindings.exportTransactions('csv');
        bindings.exportTransactions('csv');
        await flushPromises();
        expect(mockStartDownloadFile).toHaveBeenCalledWith('dataExport.exportFilename:Alice.csv', 'csv-data');
        expect(bindings.exportingData.value).toBe(false);

        mockUserStore.currentUserNickname = '';
        mockUserStore.getExportedUserData.mockResolvedValueOnce('json-data');
        bindings.exportTransactions('json');
        await flushPromises();
        expect(mockStartDownloadFile).toHaveBeenCalledWith('dataExport.defaultExportFilename.json', 'json-data');

        mockUserStore.getExportedUserData.mockRejectedValueOnce({ processed: false, message: 'export failed' });
        bindings.exportTransactions('csv');
        await flushPromises();
        expect(dialogs.snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'export failed' }));
        dialogs.snackbar.showError.mockClear();
        mockUserStore.getExportedUserData.mockRejectedValueOnce({ processed: true });
        bindings.exportTransactions('csv');
        await flushPromises();
        expect(dialogs.snackbar.showError).not.toHaveBeenCalled();

        dialogs.edit.open.mockResolvedValueOnce({ message: 'updated' });
        bindings.show({ id: 'bill-1' });
        await flushPromises();
        expect(dialogs.snackbar.showMessage).toHaveBeenCalledWith('updated');
        dialogs.edit.open.mockResolvedValueOnce({ deleted: true });
        bindings.show({ id: 'bill-2' });
        await flushPromises();
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: true });
        dialogs.edit.open.mockResolvedValueOnce(undefined);
        bindings.show({ id: 'bill-3' });
        await flushPromises();
        dialogs.edit.open.mockRejectedValueOnce('show failed');
        bindings.show({ id: 'bill-4' });
        await flushPromises();
        expect(dialogs.snackbar.showError).toHaveBeenCalledWith('show failed');
        dialogs.edit.open.mockRejectedValueOnce(undefined);
        bindings.show({ id: 'bill-5' });
        await flushPromises();
    });

    test('scrolls menus, parses amount filters, reports errors, and executes watchers', async () => {
        const bindings = makeBindings();
        const { snackbar, edit } = installDialogs(bindings);
        await flushPromises();

        const menu = { contentEl: { id: 'menu' } };
        bindings.timeFilterMenu.value = menu;
        bindings.categoryFilterMenu.value = menu;
        bindings.amountFilterCentsMenu.value = menu;
        bindings.accountFilterMenu.value = menu;
        bindings.tagFilterMenu.value = menu;
        bindings.scrollTimeMenuToSelectedItem(false);
        bindings.scrollTimeMenuToSelectedItem(true);
        bindings.scrollCategoryMenuToSelectedItem(false);
        bindings.scrollCategoryMenuToSelectedItem(true);
        bindings.scrollAccountMenuToSelectedItem(false);
        bindings.scrollAccountMenuToSelectedItem(true);
        bindings.scrollTagMenuToSelectedItem(false);
        bindings.scrollTagMenuToSelectedItem(true);
        await flushPromises();
        expect(mockScrollToSelectedItem).toHaveBeenCalledWith(menu.contentEl, 'div.v-list', 'div.v-list-item.list-item-selected');

        bindings.scrollAmountMenuToSelectedItem(false);
        bindings.query.value.amountFilterCents = 'equals:1234';
        bindings.scrollAmountMenuToSelectedItem(true);
        await flushPromises();
        expect(bindings.currentAmountFilterValue1.value).toBe(1234);
        bindings.query.value.amountFilterCents = 'between:1000:2000';
        bindings.scrollAmountMenuToSelectedItem(true);
        await flushPromises();
        expect(bindings.currentAmountFilterValue2.value).toBe(2000);
        bindings.query.value.amountFilterCents = 'malformed';
        bindings.scrollAmountMenuToSelectedItem(true);
        bindings.query.value.amountFilterCents = null;
        bindings.scrollAmountMenuToSelectedItem(true);
        bindings.scrollMenuToSelectedItem(null);
        await flushPromises();

        bindings.onShowDateRangeError('bad date');
        expect(snackbar.showError).toHaveBeenCalledWith('bad date');

        expect(mockWatchCallbacks).toHaveLength(3);
        bindings.showNav.value = false;
        mockWatchCallbacks[0]?.(false, true);
        expect(bindings.alwaysShowNav.value).toBe(false);
        mockWatchCallbacks[0]?.(true, false);
        expect(bindings.showNav.value).toBe(true);

        edit.open.mockResolvedValue(undefined);
        mockDesktopPageStore.showAddTransactionDialogInTransactionList = true;
        mockWatchCallbacks[1]?.(true, false);
        mockWatchCallbacks[1]?.(false, true);
        await flushPromises();
        expect(mockDesktopPageStore.resetShowAddTransactionDialogInTransactionList).toHaveBeenCalled();

        bindings.pageType.value = mockPageType.List.type;
        mockWatchCallbacks[2]?.('2026-07-02', '2026-07-01');
        bindings.pageType.value = mockPageType.Calendar.type;
        mockWatchCallbacks[2]?.('2026-07-02', '2026-07-02');
        mockWatchCallbacks[2]?.('2026-07-03', '2026-07-02');
        await flushPromises();
        expect(mockLogger.debug).toHaveBeenCalled();
    });
});
