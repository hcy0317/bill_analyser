import { beforeEach, describe, expect, jest, test } from '@jest/globals';

interface TestAccount {
    id: string;
    hidden: boolean;
    parentId?: string;
    subAccounts?: TestAccount[];
}

interface VueActual {
    reactive<T extends object>(target: T): T;
}

const actualVue = jest.requireActual('vue') as VueActual;
const mockSetStatisticsDefaultFilter = jest.fn();
const mockSetOverviewFilter = jest.fn();
const mockSetTotalAmountExcludeAccountIds = jest.fn();
const mockUpdateStatisticsFilter = jest.fn<(payload: unknown) => boolean>(() => true);
const mockUpdateTransactionListFilter = jest.fn<(payload: unknown) => boolean>(() => true);
const mockUpdateTransactionListInvalidState = jest.fn();
const mockUpdateOverviewInvalidState = jest.fn();
const mockCategorizedAccounts = jest.fn((categorizedAccounts: unknown) => ({ categorizedAccounts }));
const mockSelectAccountTree = jest.fn((target: Record<string, boolean>, account: TestAccount, value: boolean) => {
    if (account.subAccounts) {
        for (const subAccount of account.subAccounts) {
            target[subAccount.id] = value;
        }
    } else {
        target[account.id] = value;
    }
});
const mockAccountTreeChecked = jest.fn((account: TestAccount, filtered: Record<string, boolean>) => {
    if (!account.subAccounts) {
        return !filtered[account.id];
    }

    return account.subAccounts.every(subAccount => !filtered[subAccount.id]);
});

const cash: TestAccount = { id: 'cash', hidden: false };
const bankCny: TestAccount = { id: 'bank-cny', parentId: 'bank', hidden: false };
const bankUsd: TestAccount = { id: 'bank-usd', parentId: 'bank', hidden: false };
const bank: TestAccount = { id: 'bank', hidden: false, subAccounts: [bankCny, bankUsd] };
const hidden: TestAccount = { id: 'hidden', hidden: true };

const mockSettingsStore = actualVue.reactive({
    appSettings: {
        statistics: { defaultAccountFilter: {} as Record<string, boolean> },
        overviewAccountFilterInHomePage: {} as Record<string, boolean>,
        totalAmountExcludeAccountIds: {} as Record<string, boolean>
    },
    setStatisticsDefaultAccountFilter: mockSetStatisticsDefaultFilter,
    setOverviewAccountFilterInHomePage: mockSetOverviewFilter,
    setTotalAmountExcludeAccountIds: mockSetTotalAmountExcludeAccountIds
});
const mockAccountsStore = actualVue.reactive({
    allCategorizedAccountsMap: { 1: { accounts: [cash, bank, hidden] } },
    allAccountsMap: {
        cash,
        bank,
        'bank-cny': bankCny,
        'bank-usd': bankUsd,
        hidden
    } as Record<string, TestAccount>,
    allAvailableAccountsCount: 5,
    allVisibleAccountsCount: 4
});
const mockTransactionsStore = actualVue.reactive({
    allFilterAccountIdsCount: 0,
    allFilterAccountIds: {} as Record<string, boolean>,
    updateTransactionListFilter: mockUpdateTransactionListFilter,
    updateTransactionListInvalidState: mockUpdateTransactionListInvalidState
});
const mockStatisticsStore = actualVue.reactive({
    transactionStatisticsFilter: { filterAccountIds: {} as Record<string, boolean> },
    updateTransactionStatisticsFilter: mockUpdateStatisticsFilter
});
const mockOverviewStore = {
    updateTransactionOverviewInvalidState: mockUpdateOverviewInvalidState
};

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockOverviewStore }));
jest.mock('@/lib/account.ts', () => ({
    getCategorizedAccountsWithVisibleCount: (categorizedAccounts: unknown) => (
        mockCategorizedAccounts(categorizedAccounts)
    ),
    selectAccountOrSubAccounts: (
        target: Record<string, boolean>,
        account: unknown,
        value: boolean
    ) => mockSelectAccountTree(target, account as TestAccount, value),
    isAccountOrSubAccountsAllChecked: (account: unknown, filtered: Record<string, boolean>) => (
        mockAccountTreeChecked(account as TestAccount, filtered)
    )
}));

import {
    useAccountFilterSettingPageBase,
    type AccountFilterType
} from '@/views/base/settings/AccountFilterSettingPageBase.ts';

beforeEach(() => {
    jest.clearAllMocks();
    mockSettingsStore.appSettings.statistics.defaultAccountFilter = {};
    mockSettingsStore.appSettings.overviewAccountFilterInHomePage = {};
    mockSettingsStore.appSettings.totalAmountExcludeAccountIds = {};
    mockStatisticsStore.transactionStatisticsFilter.filterAccountIds = {};
    mockTransactionsStore.allFilterAccountIdsCount = 0;
    mockTransactionsStore.allFilterAccountIds = {};
    mockAccountsStore.allAvailableAccountsCount = 5;
    mockAccountsStore.allVisibleAccountsCount = 4;
    mockUpdateStatisticsFilter.mockReturnValue(true);
    mockUpdateTransactionListFilter.mockReturnValue(true);
});

describe('account filter base projections', () => {
    test('derives labels, hidden-account policy, account categories, availability, and checked state', () => {
        const defaults = useAccountFilterSettingPageBase('statisticsDefault');

        expect(defaults.title.value).toBe('Default Account Filter');
        expect(defaults.applyText.value).toBe('Save');
        expect(defaults.allowHiddenAccount.value).toBe(true);
        expect(defaults.loading.value).toBe(true);
        expect(defaults.hasAnyAvailableAccount.value).toBe(true);
        expect(defaults.hasAnyVisibleAccount.value).toBe(true);
        expect(defaults.allCategorizedAccounts.value).toStrictEqual({
            categorizedAccounts: mockAccountsStore.allCategorizedAccountsMap
        });
        expect(mockCategorizedAccounts).toHaveBeenCalledWith(mockAccountsStore.allCategorizedAccountsMap);
        expect(defaults.isAccountChecked(cash as never, { cash: false })).toBe(true);
        expect(defaults.isAccountChecked(cash as never, { cash: true })).toBe(false);

        mockAccountsStore.allVisibleAccountsCount = 0;
        expect(defaults.hasAnyVisibleAccount.value).toBe(false);
        defaults.showHidden.value = true;
        expect(defaults.hasAnyVisibleAccount.value).toBe(true);

        mockAccountsStore.allAvailableAccountsCount = 0;
        expect(defaults.hasAnyAvailableAccount.value).toBe(false);
        expect(defaults.hasAnyVisibleAccount.value).toBe(false);

        const total = useAccountFilterSettingPageBase('accountListTotalAmount');
        expect(total.title.value).toBe('Filter Accounts');
        expect(total.applyText.value).toBe('Apply');
        expect(total.allowHiddenAccount.value).toBe(false);
    });

    test.each<AccountFilterType>([
        'statisticsCurrent',
        'homePageOverview',
        'transactionListCurrent'
    ])('allows hidden accounts for %s', type => {
        expect(useAccountFilterSettingPageBase(type).allowHiddenAccount.value).toBe(true);
    });
});

describe('account filter base loading', () => {
    test('loads default, current statistics, and home filters over all available accounts', () => {
        mockSettingsStore.appSettings.statistics.defaultAccountFilter = { cash: true, hidden: true };
        const defaults = useAccountFilterSettingPageBase('statisticsDefault');
        expect(defaults.loadFilterAccountIds()).toBe(true);
        expect(defaults.filterAccountIds.value).toStrictEqual({
            cash: true,
            bank: false,
            'bank-cny': false,
            'bank-usd': false,
            hidden: true
        });

        mockStatisticsStore.transactionStatisticsFilter.filterAccountIds = { 'bank-usd': true };
        const statistics = useAccountFilterSettingPageBase('statisticsCurrent');
        expect(statistics.loadFilterAccountIds()).toBe(true);
        expect(statistics.filterAccountIds.value['bank-usd']).toBe(true);
        expect(statistics.filterAccountIds.value['hidden']).toBe(false);

        mockSettingsStore.appSettings.overviewAccountFilterInHomePage = { hidden: true };
        const overview = useAccountFilterSettingPageBase('homePageOverview');
        expect(overview.loadFilterAccountIds()).toBe(true);
        expect(overview.filterAccountIds.value['hidden']).toBe(true);
    });

    test('loads transaction leaf and parent selections while ignoring missing account ids', () => {
        mockTransactionsStore.allFilterAccountIdsCount = 3;
        mockTransactionsStore.allFilterAccountIds = {
            cash: true,
            bank: true,
            missing: true
        };
        const current = useAccountFilterSettingPageBase('transactionListCurrent');

        expect(current.loadFilterAccountIds()).toBe(true);
        expect(current.filterAccountIds.value).toStrictEqual({
            cash: false,
            bank: true,
            'bank-cny': false,
            'bank-usd': false,
            hidden: true
        });
        expect(mockSelectAccountTree).toHaveBeenCalledTimes(2);
        expect(mockSelectAccountTree).toHaveBeenCalledWith(expect.any(Object), cash, false);
        expect(mockSelectAccountTree).toHaveBeenCalledWith(expect.any(Object), bank, false);

        mockTransactionsStore.allFilterAccountIdsCount = 0;
        mockTransactionsStore.allFilterAccountIds = {};
        const all = useAccountFilterSettingPageBase('transactionListCurrent');
        expect(all.loadFilterAccountIds()).toBe(true);
        expect(all.filterAccountIds.value).toStrictEqual({
            cash: false,
            bank: false,
            'bank-cny': false,
            'bank-usd': false,
            hidden: false
        });
    });

    test('excludes hidden accounts from total-amount loading and rejects an unknown target', () => {
        mockSettingsStore.appSettings.totalAmountExcludeAccountIds = { cash: true };
        const total = useAccountFilterSettingPageBase('accountListTotalAmount');

        expect(total.loadFilterAccountIds()).toBe(true);
        expect(total.filterAccountIds.value).toStrictEqual({
            cash: true,
            bank: false,
            'bank-cny': false,
            'bank-usd': false
        });

        const unknown = useAccountFilterSettingPageBase(undefined);
        expect(unknown.loadFilterAccountIds()).toBe(false);
        expect(unknown.filterAccountIds.value).toStrictEqual({});
    });
});

describe('account filter base saving', () => {
    function createPartiallySelectedBase(type?: AccountFilterType) {
        const base = useAccountFilterSettingPageBase(type);
        base.filterAccountIds.value = {
            cash: false,
            bank: false,
            'bank-cny': false,
            'bank-usd': true,
            hidden: true,
            ghost: true
        };
        return base;
    }

    test('persists default and current statistics exclusions and propagates the change result', () => {
        const defaults = createPartiallySelectedBase('statisticsDefault');
        expect(defaults.saveFilterAccountIds()).toBe(true);
        expect(mockSetStatisticsDefaultFilter).toHaveBeenCalledWith({
            bank: true,
            'bank-usd': true,
            hidden: true
        });

        mockUpdateStatisticsFilter.mockReturnValue(false);
        const statistics = createPartiallySelectedBase('statisticsCurrent');
        expect(statistics.saveFilterAccountIds()).toBe(false);
        expect(mockUpdateStatisticsFilter).toHaveBeenCalledWith({
            filterAccountIds: {
                bank: true,
                'bank-usd': true,
                hidden: true
            }
        });
    });

    test('persists the home filter and always invalidates the dependent overview', () => {
        const overview = createPartiallySelectedBase('homePageOverview');

        expect(overview.saveFilterAccountIds()).toBe(true);
        expect(mockSetOverviewFilter).toHaveBeenCalledWith({
            bank: true,
            'bank-usd': true,
            hidden: true
        });
        expect(mockUpdateOverviewInvalidState).toHaveBeenCalledWith(true);
    });

    test('writes selected transaction account ids and invalidates only after a changed filter', () => {
        const current = createPartiallySelectedBase('transactionListCurrent');

        expect(current.saveFilterAccountIds()).toBe(true);
        expect(mockUpdateTransactionListFilter).toHaveBeenCalledWith({
            accountIds: 'cash,bank-cny'
        });
        expect(mockUpdateTransactionListInvalidState).toHaveBeenCalledWith(true);

        mockUpdateTransactionListFilter.mockReturnValue(false);
        mockUpdateTransactionListInvalidState.mockClear();
        expect(current.saveFilterAccountIds()).toBe(false);
        expect(mockUpdateTransactionListInvalidState).not.toHaveBeenCalled();
    });

    test('uses an empty transaction account id string when every account is selected', () => {
        const current = useAccountFilterSettingPageBase('transactionListCurrent');
        current.filterAccountIds.value = {
            cash: false,
            bank: false,
            'bank-cny': false,
            'bank-usd': false,
            hidden: false
        };

        expect(current.saveFilterAccountIds()).toBe(true);
        expect(mockUpdateTransactionListFilter).toHaveBeenCalledWith({ accountIds: '' });
        expect(mockUpdateTransactionListInvalidState).toHaveBeenCalledWith(true);
    });

    test('excludes hidden and missing accounts from total-amount persistence', () => {
        const total = createPartiallySelectedBase('accountListTotalAmount');

        expect(total.saveFilterAccountIds()).toBe(true);
        expect(mockSetTotalAmountExcludeAccountIds).toHaveBeenCalledWith({
            bank: true,
            'bank-usd': true
        });
    });

    test('keeps the default changed result for an unknown save target', () => {
        expect(createPartiallySelectedBase(undefined).saveFilterAccountIds()).toBe(true);
    });
});
