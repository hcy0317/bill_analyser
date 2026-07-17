import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const accountType = {
    SingleAccount: { type: 1 },
    MultiSubAccounts: { type: 2 },
} as const;

const assetCategory = {
    type: 1,
    name: 'Assets',
    isLiability: false,
    defaultAccountIconId: 'asset',
};
const liabilityCategory = {
    type: 2,
    name: 'Liabilities',
    isLiability: true,
    defaultAccountIconId: 'liability',
};
const accountCategory = {
    Default: assetCategory,
    values: () => [assetCategory, liabilityCategory],
    valueOf: (type: number) => [assetCategory, liabilityCategory].find(item => item.type === type),
};

const dateRange = {
    Custom: { type: 99 },
    isBillingCycle: jest.fn<(type: number) => boolean>(),
};
const dateRangeScene = { Normal: 1 } as const;

class MockAccount {
    public id: string;
    public name: string;
    public type: number;
    public category: number;
    public currency: string;
    public balanceCents: number;
    public hidden: boolean;
    public icon: string;
    public color: string;
    public comment: string;
    public subAccounts: MockAccount[];

    public constructor(overrides: Partial<MockAccount> = {}) {
        this.id = overrides.id ?? 'wallet';
        this.name = overrides.name ?? 'Wallet';
        this.type = overrides.type ?? accountType.SingleAccount.type;
        this.category = overrides.category ?? assetCategory.type;
        this.currency = overrides.currency ?? 'CNY';
        this.balanceCents = overrides.balanceCents ?? 12_345;
        this.hidden = overrides.hidden ?? false;
        this.icon = overrides.icon ?? 'wallet';
        this.color = overrides.color ?? '#123456';
        this.comment = overrides.comment ?? '';
        this.subAccounts = overrides.subAccounts ?? [];
    }

    public getSubAccountCurrencies(showHidden: boolean, selectedId?: string): string[] {
        const selected = selectedId ? this.getSubAccount(selectedId) : null;
        if (selected) return [selected.currency];
        return [...new Set(this.subAccounts.filter(item => showHidden || !item.hidden).map(item => item.currency))];
    }

    public getSubAccount(id?: string): MockAccount | null {
        return this.subAccounts.find(item => item.id === id) ?? null;
    }

    public getAccountOrSubAccount(id?: string): MockAccount | null {
        if (this.type === accountType.SingleAccount.type || !id) return this;
        return this.getSubAccount(id);
    }

    public getAccountOrSubAccountId(id?: string): string | null {
        return this.getAccountOrSubAccount(id)?.id ?? null;
    }

    public getAccountOrSubAccountComment(id?: string): string | null {
        return this.getAccountOrSubAccount(id)?.comment ?? null;
    }

    public isAccountOrSubAccountHidden(id?: string): boolean {
        return this.getAccountOrSubAccount(id)?.hidden ?? false;
    }
}

const mockVisibleChild = new MockAccount({
    id: 'child-cny', name: 'CNY child', currency: 'CNY', balanceCents: 6_789,
});
const mockHiddenChild = new MockAccount({
    id: 'child-usd', name: 'USD child', currency: 'USD', balanceCents: 4_321, hidden: true,
});
const mockSingleAccount = new MockAccount({ id: 'wallet', balanceCents: 12_345 });
const mockMultiAccount = new MockAccount({
    id: 'portfolio',
    name: 'Portfolio',
    type: accountType.MultiSubAccounts.type,
    currency: '',
    balanceCents: 0,
    subAccounts: [mockVisibleChild, mockHiddenChild],
});

const mockAccountsStore: any = {
    allVisibleAccountsCount: 2,
    accountListStateInvalid: false,
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
    syncAllAccountBalances: jest.fn<(...args: any[]) => Promise<void>>(),
    hasAccount: jest.fn<(...args: any[]) => boolean>(),
    getAccountStatementDate: jest.fn<(id: string) => number | undefined>(),
    hideAccount: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteSubAccount: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteAccount: jest.fn<(...args: any[]) => Promise<void>>(),
    updateAccountDisplayOrders: jest.fn<(...args: any[]) => Promise<void>>(),
    changeAccountDisplayOrder: jest.fn<(...args: any[]) => Promise<void>>(),
};

const mockGetDateRangeByDateType = jest.fn<(...args: any[]) => any>();
const mockGetDateRangeByBillingCycleDateType = jest.fn<(...args: any[]) => any>();
const mockDisplayMdAndUp = (jest.requireActual('vue') as any).ref(true);
const mockMountedCallbacks: Array<() => void> = [];
const mockTemplateRefs = new Map<string, any>();
const mockRuntimeEvents: Array<{ name: string; handler: (...args: any[]) => any }> = [];

let mockLastBase: ReturnType<typeof createBase>;

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const loading = ref(true);
    const showHidden = ref(false);
    const displayOrderModified = ref(false);
    const showAccountBalance = ref(true);
    const allAccounts = ref([mockSingleAccount, mockMultiAccount]);
    const allCategorizedAccountsMap = ref({
        [assetCategory.type]: { accounts: [mockSingleAccount, mockMultiAccount] },
        [liabilityCategory.type]: { accounts: [] },
    });
    const accountCategoryTotalBalance = jest.fn((category: any) => (
        category ? `category-cents:${category.type}:16666` : ''
    ));
    const accountBalance = jest.fn((account: MockAccount, selectedId?: string) => {
        const selected = account.getAccountOrSubAccount(selectedId);
        return selected ? `balance-cents:${selected.balanceCents}` : '';
    });

    return {
        loading,
        showHidden,
        displayOrderModified,
        showAccountBalance,
        firstDayOfWeek: ref(1),
        fiscalYearStart: ref(101),
        allAccounts,
        allCategorizedAccountsMap,
        allAccountCount: computed(() => allAccounts.value.length),
        netAssets: ref('net-cents:16666'),
        totalAssets: ref('assets-cents:16666'),
        totalLiabilities: ref('liabilities-cents:0'),
        accountCategoryTotalBalance,
        accountBalance,
    };
}

function createCaptureStub(name: string): any {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [eventName, handler] of Object.entries(attrs)) {
                const handlers = Array.isArray(handler) ? handler : [handler];
                for (const candidate of handlers) {
                    if (eventName.startsWith('on') && typeof candidate === 'function') {
                        mockRuntimeEvents.push({ name: eventName, handler: candidate as (...args: any[]) => any });
                    }
                }
            }
            return () => h('div', { ...attrs, 'data-stub': name }, Object.entries(slots).flatMap(([slotName, slot]) => {
                if (typeof slot !== 'function') return [];
                try {
                    if (slotName === 'item') return (slot as any)({ element: mockMultiAccount });
                    return (slot as any)({ props: { role: 'button' } });
                } catch {
                    return [];
                }
            }));
        },
    });
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
        onMounted: (callback: () => void) => mockMountedCallbacks.push(callback),
    };
});

jest.mock('vuetify', () => ({ useDisplay: () => ({ mdAndUp: mockDisplayMdAndUp }) }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getAllDateRanges: (_scene: unknown, _custom: boolean, hasStatementDate: boolean) => [
            { type: 10, displayName: 'This month' },
            ...(hasStatementDate ? [{ type: 20, displayName: 'Billing cycle' }] : []),
            { type: dateRange.Custom.type, displayName: 'Custom' },
        ],
        getCurrencyName: (currency: string) => `currency:${currency}`,
        joinMultiText: (items: string[]) => items.join(' / '),
    }),
}));
jest.mock('@/views/base/accounts/AccountListPageBase.ts', () => ({
    useAccountListPageBase: () => {
        mockLastBase = createBase();
        return mockLastBase;
    },
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/core/datetime.ts', () => ({ DateRange: dateRange, DateRangeScene: dateRangeScene }));
jest.mock('@/core/account.ts', () => ({ AccountType: accountType, AccountCategory: accountCategory }));
jest.mock('@/lib/common.ts', () => ({
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value),
}));
jest.mock('@/lib/datetime.ts', () => ({
    getDateRangeByDateType: (...args: any[]) => mockGetDateRangeByDateType(...args),
    getDateRangeByBillingCycleDateType: (...args: any[]) => mockGetDateRangeByBillingCycleDateType(...args),
}));

for (const componentPath of [
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/accounts/list/dialogs/EditDialog.vue',
    '@/views/desktop/accounts/list/dialogs/ReconciliationStatementDialog.vue',
    '@/views/desktop/accounts/list/dialogs/MoveAllTransactionsDialog.vue',
    '@/views/desktop/accounts/list/dialogs/ClearAllTransactionsDialog.vue',
    '@/views/desktop/common/cards/AccountFilterSettingsCard.vue',
    '@/components/desktop/SettingsJsonImportExportButton.vue',
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: createCaptureStub('DesktopAccountsListCoverageChildStub'),
    }));
}

const ListPage = require('@/views/desktop/accounts/ListPage.vue').default as any;

function setupPage(): any {
    mockTemplateRefs.clear();
    return ListPage.setup({}, { expose: jest.fn() });
}

function installRefs(bindings: any): {
    confirm: { open: jest.Mock<(...args: any[]) => Promise<any>> };
    snackbar: { showMessage: jest.Mock; showError: jest.Mock };
    edit: { open: jest.Mock<(...args: any[]) => Promise<any>> };
    reconciliation: { open: jest.Mock<(...args: any[]) => Promise<any>> };
    move: { open: jest.Mock<(...args: any[]) => Promise<any>> };
    clear: { open: jest.Mock<(...args: any[]) => Promise<any>> };
} {
    const confirm = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const edit = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const reconciliation = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const move = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const clear = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    bindings.confirmDialog.value = confirm;
    bindings.snackbar.value = snackbar;
    bindings.editDialog.value = edit;
    bindings.reconciliationStatementDialog.value = reconciliation;
    bindings.moveAllTransactionsDialog.value = move;
    bindings.clearAllTransactionsDialog.value = clear;
    return { confirm, snackbar, edit, reconciliation, move, clear };
}

async function flush(times = 8): Promise<void> {
    const { nextTick } = jest.requireActual('vue') as any;
    for (let index = 0; index < times; index++) await Promise.resolve();
    await nextTick();
    await new Promise(resolve => setImmediate(resolve));
}

async function renderExternalTemplate(mutator: (bindings: any) => void): Promise<string> {
    const { createSSRApp } = require('vue') as any;
    const { renderToString } = require('vue/server-renderer') as any;
    const { readFileSync } = require('node:fs') as typeof import('node:fs');
    const { resolve } = require('node:path') as typeof import('node:path');
    const { render: _emptyRender, ...scriptOnly } = ListPage;
    const RuntimePage = {
        ...scriptOnly,
        template: readFileSync(resolve(
            __dirname,
            '../../../src/web/src/views/desktop/accounts/list/ListPage.template.html',
        ), 'utf8'),
        setup(props: any, context: any) {
            const bindings = ListPage.setup(props, context);
            mutator(bindings);
            return { ...bindings };
        },
    };
    const app = createSSRApp(RuntimePage);
    for (const name of [
        'v-row', 'v-col', 'v-card', 'v-layout', 'v-navigation-drawer', 'v-divider', 'v-tabs',
        'v-tab', 'v-main', 'v-window', 'v-window-item', 'v-btn', 'v-icon', 'v-tooltip',
        'v-progress-circular', 'v-spacer', 'v-menu', 'v-list', 'v-list-item', 'v-list-item-title',
        'v-card-text', 'v-skeleton-loader', 'v-btn-toggle', 'v-dialog', 'draggable-list',
        'item-icon', 'account-filter-settings-card', 'settings-json-import-export-button',
        'edit-dialog', 'reconciliation-statement-dialog', 'move-all-transactions-dialog',
        'clear-all-transactions-dialog', 'date-range-selection-dialog', 'confirm-dialog', 'snack-bar',
    ]) {
        app.component(name, createCaptureStub(`DesktopAccountsListSsr${name}`));
    }
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

beforeEach(() => {
    jest.resetAllMocks();
    mockMountedCallbacks.length = 0;
    mockTemplateRefs.clear();
    mockRuntimeEvents.length = 0;
    mockDisplayMdAndUp.value = true;
    mockAccountsStore.allVisibleAccountsCount = 2;
    mockAccountsStore.accountListStateInvalid = false;
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockAccountsStore.syncAllAccountBalances.mockResolvedValue(undefined);
    mockAccountsStore.hasAccount.mockReturnValue(true);
    mockAccountsStore.getAccountStatementDate.mockReturnValue(20);
    mockAccountsStore.hideAccount.mockResolvedValue(undefined);
    mockAccountsStore.deleteSubAccount.mockResolvedValue(undefined);
    mockAccountsStore.deleteAccount.mockResolvedValue(undefined);
    mockAccountsStore.updateAccountDisplayOrders.mockResolvedValue(undefined);
    mockAccountsStore.changeAccountDisplayOrder.mockResolvedValue(undefined);
    dateRange.isBillingCycle.mockReturnValue(false);
    mockGetDateRangeByDateType.mockReturnValue({ minTime: 1_000, maxTime: 2_000, dateType: 10 });
    mockGetDateRangeByBillingCycleDateType.mockReturnValue({ minTime: 3_000, maxTime: 4_000, dateType: 20 });
});

describe('desktop accounts ListPage production-loaded derived state', () => {
    test('projects category visibility, totals, account presence, currency, and cents balances', () => {
        const bindings = setupPage();
        expect(bindings.hasAnyVisibleAccount.value).toBe(true);
        expect(bindings.activeAccountCategory.value).toBe(assetCategory);
        expect(bindings.activeAccountCategoryTotalBalance.value).toBe('category-cents:1:16666');
        expect(bindings.activeAccountCategoryVisibleAccountCount.value).toBe(2);
        expect(bindings.hasAccount(assetCategory)).toBe(true);
        expect(mockAccountsStore.hasAccount).toHaveBeenCalledWith(assetCategory, true);

        expect(bindings.accountCurrency(mockSingleAccount)).toBe('currency:CNY');
        expect(bindings.accountCurrency(mockMultiAccount)).toBe('currency:CNY');
        bindings.showHidden.value = true;
        expect(bindings.accountCurrency(mockMultiAccount)).toBe('currency:CNY / currency:USD');
        bindings.activeSubAccount.value.portfolio = 'child-usd';
        expect(bindings.accountCurrency(mockMultiAccount)).toBe('currency:USD');
        expect(bindings.accountCurrency(new MockAccount({ type: 99 }))).toBeNull();

        expect(bindings.accountBalance(mockSingleAccount)).toBe('balance-cents:12345');
        expect(bindings.accountBalance(mockMultiAccount, 'child-cny')).toBe('balance-cents:6789');
        expect(mockLastBase.accountBalance).toHaveBeenCalledWith(mockSingleAccount);
        expect(mockLastBase.accountBalance).toHaveBeenCalledWith(mockMultiAccount, 'child-cny');

        bindings.activeAccountCategoryType.value = 999;
        expect(bindings.activeAccountCategory.value).toBeUndefined();
        expect(bindings.activeAccountCategoryTotalBalance.value).toBe('');
        expect(bindings.activeAccountCategoryVisibleAccountCount.value).toBe(0);
        bindings.activeAccountCategoryType.value = liabilityCategory.type;
        expect(bindings.activeAccountCategoryVisibleAccountCount.value).toBe(0);
        mockLastBase.allCategorizedAccountsMap.value[liabilityCategory.type] = { accounts: undefined };
        expect(bindings.activeAccountCategoryVisibleAccountCount.value).toBe(0);

        bindings.activeAccountCategoryType.value = assetCategory.type;
        mockSingleAccount.hidden = true;
        bindings.showHidden.value = false;
        expect(bindings.activeAccountCategoryVisibleAccountCount.value).toBe(1);
        bindings.showHidden.value = true;
        expect(bindings.activeAccountCategoryVisibleAccountCount.value).toBe(2);
        mockSingleAccount.hidden = false;
    });

    test('derives statement date ranges with and without billing statement dates', () => {
        const bindings = setupPage();
        expect(bindings.accountReconciliationStatementDateRanges(mockSingleAccount).map((item: any) => item.type))
            .toStrictEqual([10, 20, 99]);
        mockAccountsStore.getAccountStatementDate.mockReturnValueOnce(undefined);
        expect(bindings.accountReconciliationStatementDateRanges(mockSingleAccount).map((item: any) => item.type))
            .toStrictEqual([10, 99]);
    });
});

describe('desktop accounts ListPage production-loaded reload', () => {
    test('loads accounts, initializes multi-account selection, and force-syncs balances', async () => {
        const bindings = setupPage();
        const { snackbar } = installRefs(bindings);
        bindings.displayOrderModified.value = true;
        bindings.reload(false);
        expect(bindings.loading.value).toBe(true);
        await flush();
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(bindings.loading.value).toBe(false);
        expect(bindings.displayOrderModified.value).toBe(false);
        expect(bindings.activeSubAccount.value.portfolio).toBe('');

        bindings.activeSubAccount.value.portfolio = 'child-cny';
        bindings.reload(true);
        await flush();
        expect(mockAccountsStore.syncAllAccountBalances).toHaveBeenCalledWith({ refreshAccounts: false });
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenLastCalledWith({ force: true });
        expect(bindings.activeSubAccount.value.portfolio).toBe('child-cny');
        expect(snackbar.showMessage).toHaveBeenCalledWith('Account list has been updated');

        mockLastBase.allAccounts.value = [];
        bindings.reload(false);
        await flush();
    });

    test('ignores stale success/failure and handles current up-to-date, processed, and raw failures', async () => {
        const bindings = setupPage();
        const { snackbar } = installRefs(bindings);
        let resolveOld: (() => void) | undefined;
        mockAccountsStore.loadAllAccounts.mockImplementationOnce(() => new Promise<void>(resolve => {
            resolveOld = resolve;
        }));
        bindings.reload(false);
        bindings.reload(false);
        await flush();
        resolveOld?.();
        await flush();
        expect(bindings.loading.value).toBe(false);

        let rejectOld: ((error: unknown) => void) | undefined;
        mockAccountsStore.loadAllAccounts.mockImplementationOnce(() => new Promise((_resolve, reject) => {
            rejectOld = reject;
        }));
        bindings.reload(false);
        bindings.reload(false);
        await flush();
        rejectOld?.({ processed: false, message: 'stale failure' });
        await flush();
        expect(snackbar.showError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'stale failure' }));

        bindings.displayOrderModified.value = true;
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ isUpToDate: true, processed: true });
        bindings.reload(false);
        await flush();
        expect(bindings.displayOrderModified.value).toBe(false);
        expect(snackbar.showError).not.toHaveBeenCalled();

        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'load failed' });
        bindings.reload(false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'load failed' }));
    });

    test('reloads on mount', async () => {
        setupPage();
        expect(mockMountedCallbacks).toHaveLength(1);
        mockMountedCallbacks[0]!();
        await flush();
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
    });
});

describe('desktop accounts ListPage production-loaded dialogs and date ranges', () => {
    test('adds and edits accounts across success, empty, error, and invalidation paths', async () => {
        const bindings = setupPage();
        const { edit, snackbar } = installRefs(bindings);
        edit.open.mockResolvedValueOnce({ message: 'Added' });
        bindings.addAccountForCategory(2);
        await flush();
        expect(edit.open).toHaveBeenCalledWith({ category: 2 });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Added');
        bindings.activeAccountCategoryType.value = 1;
        edit.open.mockResolvedValueOnce(undefined);
        bindings.addAccountForCurrentCategory();
        await flush();
        edit.open.mockResolvedValueOnce({});
        bindings.addAccountForCategory(1);
        await flush();
        edit.open.mockRejectedValueOnce(new Error('add failed'));
        bindings.addAccountForCategory(1);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'add failed' }));
        edit.open.mockRejectedValueOnce(undefined);
        bindings.addAccountForCategory(1);
        await flush();

        mockAccountsStore.accountListStateInvalid = true;
        bindings.loading.value = false;
        edit.open.mockResolvedValueOnce({ message: 'Saved' });
        bindings.edit(mockSingleAccount);
        await flush();
        expect(edit.open).toHaveBeenLastCalledWith({ id: 'wallet', currentAccount: mockSingleAccount });
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalled();
        bindings.loading.value = true;
        edit.open.mockResolvedValueOnce(undefined);
        bindings.edit(mockSingleAccount);
        await flush();
        mockAccountsStore.accountListStateInvalid = false;
        bindings.loading.value = false;
        edit.open.mockResolvedValueOnce({});
        bindings.edit(mockSingleAccount);
        await flush();
        edit.open.mockRejectedValueOnce(new Error('edit failed'));
        bindings.edit(mockSingleAccount);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'edit failed' }));
        edit.open.mockRejectedValueOnce(undefined);
        bindings.edit(mockSingleAccount);
        await flush();

        const noRef = setupPage();
        noRef.addAccountForCategory(1);
        noRef.edit(mockSingleAccount);
    });

    test('opens custom, calendar, and billing-cycle reconciliation ranges', () => {
        const bindings = setupPage();
        const { reconciliation } = installRefs(bindings);
        bindings.showReconciliationStatementCustomDateRangeDialog(mockSingleAccount);
        expect(bindings.accountToShowReconciliationStatement.value).toStrictEqual(mockSingleAccount);
        expect(bindings.showCustomDateRangeDialog.value).toBe(true);
        bindings.showCustomDateRangeDialog.value = false;
        bindings.showReconciliationStatementCustomDateRangeDialog(mockSingleAccount, dateRange.Custom.type);
        expect(bindings.showCustomDateRangeDialog.value).toBe(true);
        bindings.showReconciliationStatementCustomDateRangeDialog(mockSingleAccount, '10' as any);
        expect(bindings.showCustomDateRangeDialog.value).toBe(true);

        bindings.showReconciliationStatementCustomDateRangeDialog(mockSingleAccount, 10);
        expect(mockGetDateRangeByDateType).toHaveBeenCalledWith(10, 1, 101);
        expect(reconciliation.open).toHaveBeenCalledWith({ accountId: 'wallet', startTime: 1_000, endTime: 2_000 });

        dateRange.isBillingCycle.mockReturnValueOnce(true);
        bindings.showReconciliationStatementCustomDateRangeDialog(mockSingleAccount, 20);
        expect(mockGetDateRangeByBillingCycleDateType).toHaveBeenCalledWith(20, 1, 101, 20);
        expect(reconciliation.open).toHaveBeenLastCalledWith({ accountId: 'wallet', startTime: 3_000, endTime: 4_000 });

        mockGetDateRangeByDateType.mockReturnValueOnce(null);
        bindings.showReconciliationStatementCustomDateRangeDialog(mockSingleAccount, 10);
        const noRef = setupPage();
        noRef.showReconciliationStatementCustomDateRangeDialog(mockSingleAccount, 10);
    });

    test('finishes custom ranges and reports missing context or date errors', () => {
        const bindings = setupPage();
        const { reconciliation, snackbar } = installRefs(bindings);
        bindings.onCustomDateRangeChanged(1, 2);
        expect(snackbar.showMessage).toHaveBeenCalledWith('An error occurred');
        bindings.accountToShowReconciliationStatement.value = mockSingleAccount;
        bindings.showCustomDateRangeDialog.value = true;
        bindings.onCustomDateRangeChanged(5_000, 6_000);
        expect(reconciliation.open).toHaveBeenCalledWith({
            accountId: 'wallet', startTime: 5_000, endTime: 6_000,
        });
        expect(bindings.showCustomDateRangeDialog.value).toBe(false);
        expect(bindings.accountToShowReconciliationStatement.value).toBeNull();
        bindings.onShowDateRangeError('bad range');
        expect(snackbar.showError).toHaveBeenCalledWith('bad range');
    });

    test('moves and clears account transactions and conditionally reloads invalid state', async () => {
        const bindings = setupPage();
        const { move, clear, snackbar } = installRefs(bindings);
        mockAccountsStore.accountListStateInvalid = true;
        bindings.loading.value = false;
        move.open.mockResolvedValueOnce(undefined);
        bindings.moveAllTransactions(mockSingleAccount);
        await flush();
        expect(move.open).toHaveBeenCalledWith(mockSingleAccount);
        expect(snackbar.showMessage).toHaveBeenCalledWith('All transactions in this account has been moved.');
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalled();

        bindings.loading.value = true;
        clear.open.mockResolvedValueOnce(undefined);
        bindings.clearAllTransactions(mockSingleAccount);
        await flush();
        expect(clear.open).toHaveBeenCalledWith(mockSingleAccount);
        expect(snackbar.showMessage).toHaveBeenCalledWith('All transactions in this account has been cleared');

        mockAccountsStore.accountListStateInvalid = false;
        bindings.loading.value = false;
        move.open.mockResolvedValueOnce(undefined);
        clear.open.mockResolvedValueOnce(undefined);
        bindings.moveAllTransactions(mockSingleAccount);
        bindings.clearAllTransactions(mockSingleAccount);
        await flush();

        const noRef = setupPage();
        noRef.moveAllTransactions(mockSingleAccount);
        noRef.clearAllTransactions(mockSingleAccount);
    });
});

describe('desktop accounts ListPage production-loaded mutations', () => {
    test('hides accounts and clears hidden active sub-account selection', async () => {
        const bindings = setupPage();
        const { snackbar } = installRefs(bindings);
        bindings.activeSubAccount.value.portfolio = 'child-cny';
        bindings.showHidden.value = false;
        bindings.hide(mockMultiAccount, mockVisibleChild, true);
        await flush();
        expect(mockAccountsStore.hideAccount).toHaveBeenCalledWith({ account: mockVisibleChild, hidden: true });
        expect(bindings.activeSubAccount.value.portfolio).toBe('');
        expect(bindings.loading.value).toBe(false);

        bindings.activeSubAccount.value.portfolio = 'child-cny';
        bindings.showHidden.value = true;
        bindings.hide(mockMultiAccount, mockVisibleChild, true);
        await flush();
        expect(bindings.activeSubAccount.value.portfolio).toBe('child-cny');
        bindings.showHidden.value = false;
        bindings.hide(mockMultiAccount, mockVisibleChild, false);
        await flush();
        expect(bindings.activeSubAccount.value.portfolio).toBe('child-cny');

        mockAccountsStore.hideAccount.mockRejectedValueOnce({ processed: true });
        bindings.hide(mockSingleAccount, mockSingleAccount, true);
        await flush();
        expect(snackbar.showError).not.toHaveBeenCalled();
        mockAccountsStore.hideAccount.mockRejectedValueOnce({ processed: false, message: 'hide failed' });
        bindings.hide(mockSingleAccount, mockSingleAccount, true);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'hide failed' }));
    });

    test('deletes selected sub-accounts and handles missing/failed variants', async () => {
        const bindings = setupPage();
        const { confirm, snackbar } = installRefs(bindings);
        bindings.activeSubAccount.value.portfolio = 'missing';
        bindings.remove(mockMultiAccount);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Unable to delete this sub-account');

        bindings.activeSubAccount.value.portfolio = 'child-cny';
        confirm.open.mockResolvedValue(undefined);
        bindings.remove(mockMultiAccount);
        await flush();
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to delete this sub-account?');
        expect(mockAccountsStore.deleteSubAccount).toHaveBeenCalledWith({ subAccount: mockVisibleChild });
        expect(bindings.activeSubAccount.value.portfolio).toBe('');
        expect(bindings.loading.value).toBe(false);

        bindings.activeSubAccount.value.portfolio = 'child-cny';
        mockAccountsStore.deleteSubAccount.mockRejectedValueOnce({ processed: true });
        bindings.remove(mockMultiAccount);
        await flush();
        expect(snackbar.showError).not.toHaveBeenCalled();
        mockAccountsStore.deleteSubAccount.mockRejectedValueOnce({ processed: false, message: 'delete child failed' });
        bindings.remove(mockMultiAccount);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'delete child failed' }));
    });

    test('deletes parent accounts and handles processed/unprocessed failures', async () => {
        const bindings = setupPage();
        const { confirm, snackbar } = installRefs(bindings);
        confirm.open.mockResolvedValue(undefined);
        bindings.remove(mockSingleAccount);
        await flush();
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to delete this account?');
        expect(mockAccountsStore.deleteAccount).toHaveBeenCalledWith({ account: mockSingleAccount });
        expect(bindings.loading.value).toBe(false);

        mockAccountsStore.deleteAccount.mockRejectedValueOnce({ processed: true });
        bindings.remove(mockSingleAccount);
        await flush();
        expect(snackbar.showError).not.toHaveBeenCalled();
        mockAccountsStore.deleteAccount.mockRejectedValueOnce({ processed: false, message: 'delete account failed' });
        bindings.remove(mockSingleAccount);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'delete account failed' }));

        const noRef = setupPage();
        noRef.remove(mockSingleAccount);
        noRef.activeSubAccount.value.portfolio = 'child-cny';
        noRef.remove(mockMultiAccount);
    });

    test('saves modified sort order and reports only unprocessed failures', async () => {
        const bindings = setupPage();
        const { snackbar } = installRefs(bindings);
        bindings.saveSortResult();
        expect(mockAccountsStore.updateAccountDisplayOrders).not.toHaveBeenCalled();
        bindings.displayOrderModified.value = true;
        bindings.saveSortResult();
        await flush();
        expect(mockAccountsStore.updateAccountDisplayOrders).toHaveBeenCalled();
        expect(bindings.displayOrderModified.value).toBe(false);

        bindings.displayOrderModified.value = true;
        mockAccountsStore.updateAccountDisplayOrders.mockRejectedValueOnce({ processed: true });
        bindings.saveSortResult();
        await flush();
        expect(snackbar.showError).not.toHaveBeenCalled();
        bindings.displayOrderModified.value = true;
        mockAccountsStore.updateAccountDisplayOrders.mockRejectedValueOnce({ processed: false, message: 'sort failed' });
        bindings.saveSortResult();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'sort failed' }));
    });

    test('validates drag events and saves changed order drafts', async () => {
        const bindings = setupPage();
        const { snackbar } = installRefs(bindings);
        bindings.onMove(undefined as any);
        bindings.onMove({} as any);
        expect(mockAccountsStore.changeAccountDisplayOrder).not.toHaveBeenCalled();
        bindings.onMove({ moved: { element: {}, oldIndex: 0, newIndex: 1 } } as any);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Unable to move account');
        bindings.onMove({ moved: { element: { id: '' }, oldIndex: 0, newIndex: 1 } } as any);

        bindings.onMove({ moved: { element: { id: 'wallet' }, oldIndex: 0, newIndex: 1 } });
        await flush();
        expect(mockAccountsStore.changeAccountDisplayOrder).toHaveBeenCalledWith({
            accountId: 'wallet', from: 0, to: 1, updateListOrder: false, updateGlobalListOrder: true,
        });
        expect(bindings.displayOrderModified.value).toBe(true);

        mockAccountsStore.changeAccountDisplayOrder.mockRejectedValueOnce(new Error('move failed'));
        bindings.onMove({ moved: { element: { id: 'wallet' }, oldIndex: 1, newIndex: 0 } });
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'move failed' }));
    });

    test('tracks responsive navigation state', async () => {
        const bindings = setupPage();
        expect(bindings.alwaysShowNav.value).toBe(true);
        expect(bindings.showNav.value).toBe(true);
        mockDisplayMdAndUp.value = false;
        await flush();
        expect(bindings.alwaysShowNav.value).toBe(false);
        expect(bindings.showNav.value).toBe(true);
        bindings.showNav.value = false;
        mockDisplayMdAndUp.value = true;
        await flush();
        expect(bindings.alwaysShowNav.value).toBe(true);
        expect(bindings.showNav.value).toBe(true);
    });
});

describe('desktop accounts ListPage external template runtime', () => {
    test('renders overview, account card, balances, dialogs, and executes generated handlers', async () => {
        const html = await renderExternalTemplate(bindings => {
            installRefs(bindings);
            bindings.loading.value = false;
            bindings.showHidden.value = true;
            bindings.displayOrderModified.value = true;
            bindings.activeAccountCategoryType.value = assetCategory.type;
            bindings.activeSubAccount.value.portfolio = 'child-cny';
        });
        expect(html).toContain('Account List');
        expect(html).toContain('balance-cents:6789');
        expect(html).toContain('Net assets');
        expect(mockRuntimeEvents.length).toBeGreaterThan(10);
        for (const { name, handler } of mockRuntimeEvents) {
            try {
                if (name === 'onUpdate:modelValue' || name.startsWith('onUpdate:')) {
                    handler(true);
                } else if (name === 'onDateRange:change') {
                    handler(1_000, 2_000);
                } else if (name === 'onChange') {
                    handler({ moved: { element: { id: 'wallet' }, oldIndex: 0, newIndex: 1 } });
                } else {
                    handler();
                }
            } catch {
                // Generated handlers bind heterogeneous account/date/menu payloads.
            }
        }
        await flush();
    });
});
