/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const { reactive, ref } = actualVue;
const mockShowToast = jest.fn();
const mockRouteBackOnError = jest.fn();
const mockLoadAccounts = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadFilterIds = jest.fn();
const mockSaveFilterIds = jest.fn();
const mockSelectAccountOrSubs = jest.fn();
const mockSelectAll = jest.fn();
const mockSelectNone = jest.fn();
const mockSelectInvert = jest.fn();
const mockSelectAllVisible = jest.fn();

const mockBase = {
    loading: ref(true),
    showHidden: ref(false),
    filterAccountIds: ref({} as Record<string, boolean>),
    title: ref('Accounts'),
    applyText: ref('Apply'),
    allowHiddenAccount: ref(false),
    allCategorizedAccounts: ref([]),
    hasAnyAvailableAccount: ref(true),
    hasAnyVisibleAccount: ref(true),
    isAccountChecked: jest.fn(() => false),
    loadFilterAccountIds: mockLoadFilterIds,
    saveFilterAccountIds: mockSaveFilterIds,
};
const account = { id: 'account-1', type: 1, name: 'Wallet' };
const mockAccountsStore = {
    allAccountsMap: { 'account-1': account } as Record<string, any>,
    loadAllAccounts: mockLoadAccounts,
};

jest.mock('@/locales/helpers.ts', () => ({ useI18n: () => ({ tt: (key: string) => key }) }));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast, routeBackOnError: mockRouteBackOnError }),
}));
jest.mock('@/views/base/settings/AccountFilterSettingPageBase.ts', () => ({
    useAccountFilterSettingPageBase: () => mockBase,
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/core/account.ts', () => ({
    AccountType: { MultiSubAccounts: { type: 2 } },
    AccountCategory: { values: () => [{ type: 1 }, { type: 2 }, { type: 3 }] },
}));
jest.mock('@/lib/account.ts', () => ({
    selectAccountOrSubAccounts: (...args: any[]) => mockSelectAccountOrSubs(...args),
    selectAllVisible: (...args: any[]) => mockSelectAllVisible(...args),
    selectAll: (...args: any[]) => mockSelectAll(...args),
    selectNone: (...args: any[]) => mockSelectNone(...args),
    selectInvert: (...args: any[]) => mockSelectInvert(...args),
    isAccountOrSubAccountsAllChecked: jest.fn(() => false),
    isAccountOrSubAccountsHasButNotAllChecked: jest.fn(() => false),
}));

const AccountFilterSettingsPage = require('@/views/mobile/settings/AccountFilterSettingsPage.vue').default as any;

function setup(router = { back: jest.fn() }): any {
    return AccountFilterSettingsPage.setup(reactive({
        f7route: { query: { type: 'homePageOverview' } },
        f7router: router,
    }), { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() });
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockLoadAccounts.mockResolvedValue(undefined);
    mockLoadFilterIds.mockReturnValue(true);
    mockBase.loading.value = true;
    mockBase.allowHiddenAccount.value = false;
    mockBase.filterAccountIds.value = {};
});

describe('mobile AccountFilterSettingsPage production behavior', () => {
    test('builds an opened collapse state for every account category and initializes valid filters', async () => {
        const bindings = setup();
        expect(bindings.collapseStates.value).toEqual({
            1: { opened: true }, 2: { opened: true }, 3: { opened: true },
        });
        expect(mockLoadAccounts).toHaveBeenCalledWith({ force: false });
        await flush();
        expect(mockBase.loading.value).toBe(false);
        expect(mockLoadFilterIds).toHaveBeenCalled();
        expect(mockShowToast).not.toHaveBeenCalled();
    });

    test('marks invalid filter parameters and maps processed and unprocessed load failures', async () => {
        mockLoadFilterIds.mockReturnValueOnce(false);
        const invalid = setup();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('Parameter Invalid');
        expect(invalid.loadingError.value).toBe('Parameter Invalid');

        mockLoadAccounts.mockRejectedValueOnce({ processed: true, message: 'handled' });
        setup();
        await flush();
        expect(mockBase.loading.value).toBe(false);

        const raw = { processed: false, message: '' };
        mockLoadAccounts.mockRejectedValueOnce(raw);
        const failed = setup();
        await flush();
        expect(failed.loadingError.value).toStrictEqual(raw);
        expect(mockShowToast).toHaveBeenLastCalledWith(raw);

        mockLoadAccounts.mockRejectedValueOnce({ processed: false, message: 'accounts failed' });
        setup();
        await flush();
        expect(mockShowToast).toHaveBeenLastCalledWith('accounts failed');
    });

    test('updates parent and leaf account selections while ignoring missing account ids', () => {
        const bindings = setup();
        bindings.updateAccountOrSubAccountsSelected({ target: { value: 'missing', checked: false } });
        bindings.updateAccountSelected({ target: { value: 'missing', checked: false } });
        expect(mockSelectAccountOrSubs).not.toHaveBeenCalled();

        bindings.updateAccountOrSubAccountsSelected({ target: { value: 'account-1', checked: false } });
        expect(mockSelectAccountOrSubs).toHaveBeenCalledWith(mockBase.filterAccountIds.value, account, true);
        bindings.updateAccountOrSubAccountsSelected({ target: { value: 'account-1', checked: true } });
        expect(mockSelectAccountOrSubs).toHaveBeenLastCalledWith(mockBase.filterAccountIds.value, account, false);

        bindings.updateAccountSelected({ target: { value: 'account-1', checked: false } });
        expect(mockBase.filterAccountIds.value['account-1']).toBe(true);
        bindings.updateAccountSelected({ target: { value: 'account-1', checked: true } });
        expect(mockBase.filterAccountIds.value['account-1']).toBe(false);
    });

    test('executes bulk selection, save/back, and page error routing for hidden policies', () => {
        const router = { back: jest.fn() };
        const bindings = setup(router);
        bindings.selectAllAccounts();
        bindings.selectNoneAccounts();
        bindings.selectInvertAccounts();
        expect(mockSelectAll).toHaveBeenLastCalledWith(mockBase.filterAccountIds.value, mockAccountsStore.allAccountsMap, true);
        expect(mockSelectNone).toHaveBeenLastCalledWith(mockBase.filterAccountIds.value, mockAccountsStore.allAccountsMap, true);
        expect(mockSelectInvert).toHaveBeenLastCalledWith(mockBase.filterAccountIds.value, mockAccountsStore.allAccountsMap, true);

        mockBase.allowHiddenAccount.value = true;
        bindings.selectAllAccounts();
        expect(mockSelectAll).toHaveBeenLastCalledWith(mockBase.filterAccountIds.value, mockAccountsStore.allAccountsMap, false);
        bindings.selectAllVisibleAccounts();
        expect(mockSelectAllVisible).toHaveBeenCalledWith(mockBase.filterAccountIds.value, mockAccountsStore.allAccountsMap);

        bindings.save();
        expect(mockSaveFilterIds).toHaveBeenCalled();
        expect(router.back).toHaveBeenCalled();
        bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });
});
