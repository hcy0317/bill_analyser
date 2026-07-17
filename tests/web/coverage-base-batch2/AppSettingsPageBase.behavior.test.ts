import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const { reactive } = jest.requireActual('vue') as {
    reactive: <T extends object>(value: T) => T;
};

interface AccountStub {
    readonly id: string;
}

interface CategoryStub {
    readonly id: string;
    readonly type: number;
}

const mockTt = jest.fn((key: string) => `tt:${key}`);
const mockGetAllTimezones = jest.fn((includeLocal: boolean) => [{ name: 'Asia/Shanghai', includeLocal }]);
const mockGetAllTimezoneTypesUsedForStatistics = jest.fn(() => [{ type: 1, displayName: 'Transaction Time' }]);
const mockGetAllCurrencySortingTypes = jest.fn(() => [{ type: 2, displayName: 'Currency Code' }]);
const mockSetLocalizedTimeZone = jest.fn();
const mockGetThemePreferenceOptions = jest.fn((_tt: (key: string) => string) => [
    { name: 'tt:Follow System', value: 'system' }
]);

const mockSettingsStore = reactive({
    appSettings: {
        timeZone: 'Asia/Shanghai',
        autoUpdateExchangeRatesData: true,
        showAccountBalance: true,
        showAmountInHomePage: false,
        timezoneUsedForStatisticsInHomePage: 1,
        showTotalAmountInTransactionListPage: true,
        showTagInTransactionListPage: false,
        itemsCountInTransactionListPage: 25,
        autoSaveTransactionDraft: 'confirmation',
        autoGetCurrentGeoLocation: false,
        currencySortByInExchangeRatesPage: 2,
        overviewAccountFilterInHomePage: {} as Record<string, boolean>,
        totalAmountExcludeAccountIds: {} as Record<string, boolean>,
        overviewTransactionCategoryFilterInHomePage: {} as Record<string, boolean>
    },
    setTimeZone: jest.fn((value: string) => {
        mockSettingsStore.appSettings.timeZone = value;
    }),
    setAutoUpdateExchangeRatesData: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.autoUpdateExchangeRatesData = value;
    }),
    setShowAccountBalance: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.showAccountBalance = value;
    }),
    setShowAmountInHomePage: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.showAmountInHomePage = value;
    }),
    setTimezoneUsedForStatisticsInHomePage: jest.fn((value: number) => {
        mockSettingsStore.appSettings.timezoneUsedForStatisticsInHomePage = value;
    }),
    setShowTotalAmountInTransactionListPage: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.showTotalAmountInTransactionListPage = value;
    }),
    setShowTagInTransactionListPage: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.showTagInTransactionListPage = value;
    }),
    setItemsCountInTransactionListPage: jest.fn((value: number) => {
        mockSettingsStore.appSettings.itemsCountInTransactionListPage = value;
    }),
    setAutoSaveTransactionDraft: jest.fn((value: string) => {
        mockSettingsStore.appSettings.autoSaveTransactionDraft = value;
    }),
    setAutoGetCurrentGeoLocation: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.autoGetCurrentGeoLocation = value;
    }),
    setCurrencySortByInExchangeRatesPage: jest.fn((value: number) => {
        mockSettingsStore.appSettings.currencySortByInExchangeRatesPage = value;
    })
});

const mockAccountsStore = reactive({
    allPlainAccounts: [{ id: 'cash' }, { id: 'bank' }] as AccountStub[] | undefined,
    allVisiblePlainAccounts: [{ id: 'cash' }] as AccountStub[],
    allVisibleAccountsCount: 1,
    allAccountsMap: {
        cash: { id: 'cash' },
        bank: { id: 'bank' }
    } as Record<string, AccountStub>
});

const mockTransactionCategoriesStore = reactive({
    allTransactionCategoriesMap: {
        salary: { id: 'salary', type: 2 },
        food: { id: 'food', type: 3 },
        transfer: { id: 'transfer', type: 4 },
        investment: { id: 'investment', type: 5 }
    } as Record<string, CategoryStub> | null
});

const mockTransactionsStore = {
    updateTransactionListInvalidState: jest.fn(),
    clearTransactionDraft: jest.fn()
};
const mockOverviewStore = {
    updateTransactionOverviewInvalidState: jest.fn()
};
const mockStatisticsStore = {
    updateTransactionStatisticsInvalidState: jest.fn()
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: mockTt,
        getAllTimezones: mockGetAllTimezones,
        getAllTimezoneTypesUsedForStatistics: mockGetAllTimezoneTypesUsedForStatistics,
        getAllCurrencySortingTypes: mockGetAllCurrencySortingTypes,
        setTimeZone: mockSetLocalizedTimeZone
    })
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockTransactionCategoriesStore
}));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockOverviewStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/core/base.ts', () => ({
    keysIfValueEquals: (items: Record<string, unknown>, expected: unknown) => (
        Object.keys(items).filter(key => items[key] === expected)
    ),
    values: (items: Record<string, unknown>) => Object.values(items)
}));
jest.mock('@/core/category.ts', () => ({
    CategoryType: { Income: 2, Expense: 3, Transfer: 4, Investment: 5 }
}));
jest.mock('@/core/theme.ts', () => ({
    getThemePreferenceOptions: mockGetThemePreferenceOptions
}));
jest.mock('@/lib/common.ts', () => ({
    isObjectEmpty: (value: Record<string, unknown>) => Object.keys(value).length === 0
}));

import { useAppSettingPageBase } from '@/views/base/settings/AppSettingsPageBase.ts';

function resetStores(): void {
    Object.assign(mockSettingsStore.appSettings, {
        timeZone: 'Asia/Shanghai',
        autoUpdateExchangeRatesData: true,
        showAccountBalance: true,
        showAmountInHomePage: false,
        timezoneUsedForStatisticsInHomePage: 1,
        showTotalAmountInTransactionListPage: true,
        showTagInTransactionListPage: false,
        itemsCountInTransactionListPage: 25,
        autoSaveTransactionDraft: 'confirmation',
        autoGetCurrentGeoLocation: false,
        currencySortByInExchangeRatesPage: 2,
        overviewAccountFilterInHomePage: {},
        totalAmountExcludeAccountIds: {},
        overviewTransactionCategoryFilterInHomePage: {}
    });
    mockAccountsStore.allPlainAccounts = [{ id: 'cash' }, { id: 'bank' }];
    mockAccountsStore.allVisiblePlainAccounts = [{ id: 'cash' }];
    mockAccountsStore.allVisibleAccountsCount = 1;
    mockAccountsStore.allAccountsMap = {
        cash: { id: 'cash' },
        bank: { id: 'bank' }
    };
    mockTransactionCategoriesStore.allTransactionCategoriesMap = {
        salary: { id: 'salary', type: 2 },
        food: { id: 'food', type: 3 },
        transfer: { id: 'transfer', type: 4 },
        investment: { id: 'investment', type: 5 }
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    resetStores();
});

describe('AppSettingsPageBase production-loaded behavior', () => {
    test('projects localized options, draft modes, and reactive availability state', () => {
        const base = useAppSettingPageBase();

        expect(base.loadingAccounts.value).toBe(false);
        expect(base.loadingTransactionCategories.value).toBe(false);
        expect(base.allThemes.value).toEqual([{ name: 'tt:Follow System', value: 'system' }]);
        expect(mockGetThemePreferenceOptions).toHaveBeenCalledWith(mockTt);
        expect(base.allTimezones.value).toEqual([{ name: 'Asia/Shanghai', includeLocal: true }]);
        expect(base.allTimezoneTypesUsedForStatistics.value).toEqual([
            { type: 1, displayName: 'Transaction Time' }
        ]);
        expect(base.allCurrencySortingTypes.value).toEqual([{ type: 2, displayName: 'Currency Code' }]);
        expect(base.allAutoSaveTransactionDraftTypes.value).toEqual([
            { name: 'tt:Disabled', value: 'disabled' },
            { name: 'tt:Enabled', value: 'enabled' },
            { name: 'tt:Show Confirmation Every Time', value: 'confirmation' }
        ]);
        expect(base.hasAnyAccount.value).toBe(true);
        expect(base.hasAnyVisibleAccount.value).toBe(true);
        expect(base.hasAnyTransactionCategory.value).toBe(true);

        mockAccountsStore.allPlainAccounts = [];
        mockAccountsStore.allVisibleAccountsCount = 0;
        mockTransactionCategoriesStore.allTransactionCategoriesMap = {};
        expect(base.hasAnyAccount.value).toBe(false);
        expect(base.hasAnyVisibleAccount.value).toBe(false);
        expect(base.hasAnyTransactionCategory.value).toBe(false);
    });

    test('writes every setting and invalidates dependent views at the correct boundaries', () => {
        const base = useAppSettingPageBase();

        expect(base.timeZone.value).toBe('Asia/Shanghai');
        expect(base.isAutoUpdateExchangeRatesData.value).toBe(true);
        expect(base.showAccountBalance.value).toBe(true);
        expect(base.showAmountInHomePage.value).toBe(false);
        expect(base.timezoneUsedForStatisticsInHomePage.value).toBe(1);
        expect(base.showTotalAmountInTransactionListPage.value).toBe(true);
        expect(base.showTagInTransactionListPage.value).toBe(false);
        expect(base.itemsCountInTransactionListPage.value).toBe(25);
        expect(base.autoSaveTransactionDraft.value).toBe('confirmation');
        expect(base.isAutoGetCurrentGeoLocation.value).toBe(false);
        expect(base.currencySortByInExchangeRatesPage.value).toBe(2);

        base.timeZone.value = 'UTC';
        expect(mockSettingsStore.setTimeZone).toHaveBeenCalledWith('UTC');
        expect(mockSetLocalizedTimeZone).toHaveBeenCalledWith('UTC');
        expect(mockTransactionsStore.updateTransactionListInvalidState).toHaveBeenCalledWith(true);
        expect(mockOverviewStore.updateTransactionOverviewInvalidState).toHaveBeenCalledWith(true);
        expect(mockStatisticsStore.updateTransactionStatisticsInvalidState).toHaveBeenCalledWith(true);

        base.isAutoUpdateExchangeRatesData.value = false;
        base.showAccountBalance.value = false;
        base.showAmountInHomePage.value = true;
        base.timezoneUsedForStatisticsInHomePage.value = 2;
        base.showTotalAmountInTransactionListPage.value = false;
        base.showTagInTransactionListPage.value = true;
        base.itemsCountInTransactionListPage.value = 50;
        base.isAutoGetCurrentGeoLocation.value = true;
        base.currencySortByInExchangeRatesPage.value = 3;
        expect(mockSettingsStore.setAutoUpdateExchangeRatesData).toHaveBeenCalledWith(false);
        expect(mockSettingsStore.setShowAccountBalance).toHaveBeenCalledWith(false);
        expect(mockSettingsStore.setShowAmountInHomePage).toHaveBeenCalledWith(true);
        expect(mockSettingsStore.setTimezoneUsedForStatisticsInHomePage).toHaveBeenCalledWith(2);
        expect(mockOverviewStore.updateTransactionOverviewInvalidState).toHaveBeenCalledTimes(2);
        expect(mockSettingsStore.setShowTotalAmountInTransactionListPage).toHaveBeenCalledWith(false);
        expect(mockSettingsStore.setShowTagInTransactionListPage).toHaveBeenCalledWith(true);
        expect(mockSettingsStore.setItemsCountInTransactionListPage).toHaveBeenCalledWith(50);
        expect(mockSettingsStore.setAutoGetCurrentGeoLocation).toHaveBeenCalledWith(true);
        expect(mockSettingsStore.setCurrencySortByInExchangeRatesPage).toHaveBeenCalledWith(3);

        base.autoSaveTransactionDraft.value = 'enabled';
        expect(mockTransactionsStore.clearTransactionDraft).not.toHaveBeenCalled();
        base.autoSaveTransactionDraft.value = 'disabled';
        expect(mockSettingsStore.setAutoSaveTransactionDraft).toHaveBeenNthCalledWith(1, 'enabled');
        expect(mockSettingsStore.setAutoSaveTransactionDraft).toHaveBeenNthCalledWith(2, 'disabled');
        expect(mockTransactionsStore.clearTransactionDraft).toHaveBeenCalledTimes(1);
    });

    test('summarizes account filters across loading, missing, empty, all, none, and partial states', () => {
        const base = useAppSettingPageBase();

        expect(base.accountsIncludedInHomePageOverviewDisplayContent.value).toBe('tt:All');
        mockSettingsStore.appSettings.overviewAccountFilterInHomePage = { ghost: true };
        expect(base.accountsIncludedInHomePageOverviewDisplayContent.value).toBe('tt:All');
        mockSettingsStore.appSettings.overviewAccountFilterInHomePage = { cash: true };
        expect(base.accountsIncludedInHomePageOverviewDisplayContent.value).toBe('tt:Partial');
        mockSettingsStore.appSettings.overviewAccountFilterInHomePage = { cash: true, bank: true };
        expect(base.accountsIncludedInHomePageOverviewDisplayContent.value).toBe('tt:None');

        mockSettingsStore.appSettings.totalAmountExcludeAccountIds = { cash: true };
        expect(base.accountsIncludedInTotalDisplayContent.value).toBe('tt:None');

        base.loadingAccounts.value = true;
        expect(base.accountsIncludedInHomePageOverviewDisplayContent.value).toBe('');
        base.loadingAccounts.value = false;
        mockAccountsStore.allPlainAccounts = undefined;
        expect(base.accountsIncludedInHomePageOverviewDisplayContent.value).toBe('');
        mockAccountsStore.allPlainAccounts = [];
        expect(base.accountsIncludedInHomePageOverviewDisplayContent.value).toBe('');
    });

    test('summarizes only income and expense category filters and tolerates unavailable maps', () => {
        const base = useAppSettingPageBase();

        expect(base.transactionCategoriesIncludedInHomePageOverviewDisplayContent.value).toBe('tt:All');
        mockSettingsStore.appSettings.overviewTransactionCategoryFilterInHomePage = { ghost: true };
        expect(base.transactionCategoriesIncludedInHomePageOverviewDisplayContent.value).toBe('tt:All');
        mockSettingsStore.appSettings.overviewTransactionCategoryFilterInHomePage = { salary: true };
        expect(base.transactionCategoriesIncludedInHomePageOverviewDisplayContent.value).toBe('tt:Partial');
        mockSettingsStore.appSettings.overviewTransactionCategoryFilterInHomePage = {
            salary: true,
            food: true,
            transfer: true,
            investment: true
        };
        expect(base.transactionCategoriesIncludedInHomePageOverviewDisplayContent.value).toBe('tt:None');

        base.loadingTransactionCategories.value = true;
        expect(base.transactionCategoriesIncludedInHomePageOverviewDisplayContent.value).toBe('');
        base.loadingTransactionCategories.value = false;
        mockTransactionCategoriesStore.allTransactionCategoriesMap = null;
        expect(base.transactionCategoriesIncludedInHomePageOverviewDisplayContent.value).toBe('');
    });
});
