import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const accountType = {
    SingleAccount: { type: 1 },
    MultiSubAccounts: { type: 2 }
} as const;

const accountCategory = {
    Cash: { type: 1, name: 'Cash' },
    Checking: { type: 2, name: 'Checking Account' },
    values: () => [
        { type: 1, name: 'Cash' },
        { type: 2, name: 'Checking Account' }
    ]
} as const;

const textDirection = { LTR: 1, RTL: 2 } as const;

const showAlert = jest.fn<(...args: any[]) => void>();
const showToast = jest.fn<(...args: any[]) => void>();
const routeBackOnError = jest.fn<(...args: any[]) => void>();
const showLoading = jest.fn<(...args: any[]) => void>();
const hideLoading = jest.fn<(...args: any[]) => void>();
const onSwipeoutDeleted = jest.fn<(...args: any[]) => void>();

function createAccount(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'cash',
        name: 'Cash Wallet',
        type: accountType.SingleAccount.type,
        category: accountCategory.Cash.type,
        currency: 'CNY',
        balanceCents: 12_345,
        hidden: false,
        icon: 'wallet',
        color: '#123456',
        comment: 'Daily cash',
        subAccounts: [],
        ...overrides
    };
}

const visibleCash = createAccount();
const hiddenCash = createAccount({ id: 'cash-hidden', name: 'Hidden Cash', balanceCents: 505, hidden: true, comment: '' });
const visibleSubAccount = createAccount({
    id: 'bank-sub-visible',
    name: 'Debit Card',
    category: accountCategory.Checking.type,
    balanceCents: 2_345,
    comment: 'Salary card'
});
const hiddenSubAccount = createAccount({
    id: 'bank-sub-hidden',
    name: 'Old Card',
    category: accountCategory.Checking.type,
    balanceCents: -105,
    hidden: true,
    comment: ''
});
const multiAccount = createAccount({
    id: 'bank-parent',
    name: 'Bank Accounts',
    type: accountType.MultiSubAccounts.type,
    category: accountCategory.Checking.type,
    balanceCents: 2_240,
    comment: 'All bank cards',
    subAccounts: [visibleSubAccount, hiddenSubAccount]
});

const categorizedAccountsMap: Record<number, { accounts: any[] }> = {
    [accountCategory.Cash.type]: { accounts: [visibleCash, hiddenCash] },
    [accountCategory.Checking.type]: { accounts: [multiAccount] }
};

const rootStore = {
    clearAllUserTransactionsOfAccount: jest.fn<(...args: any[]) => Promise<void>>()
};

const accountCounts = {
    available: (jest.requireActual('vue') as any).ref(5),
    visible: (jest.requireActual('vue') as any).ref(3)
};

const accountsStore = {
    get allAvailableAccountsCount(): number {
        return accountCounts.available.value;
    },
    set allAvailableAccountsCount(value: number) {
        accountCounts.available.value = value;
    },
    get allVisibleAccountsCount(): number {
        return accountCounts.visible.value;
    },
    set allVisibleAccountsCount(value: number) {
        accountCounts.visible.value = value;
    },
    accountListStateInvalid: false,
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
    getFirstShowingIds: jest.fn<(showHidden: boolean) => any>(),
    getLastShowingIds: jest.fn<(showHidden: boolean) => any>(),
    hasAccount: jest.fn<(category: any, visibleOnly: boolean) => boolean>(),
    hasVisibleSubAccount: jest.fn<(showHidden: boolean, account: any) => boolean>(),
    hideAccount: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteAccount: jest.fn<(...args: any[]) => Promise<void>>(),
    updateAccountDisplayOrders: jest.fn<(...args: any[]) => Promise<void>>(),
    changeAccountDisplayOrder: jest.fn<(...args: any[]) => Promise<void>>()
};

let lastBase: ReturnType<typeof createBase>;

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const loading = ref(true);
    const showHidden = ref(false);
    const displayOrderModified = ref(false);
    const showAccountBalance = ref(true);
    const allCategorizedAccountsMap = ref(categorizedAccountsMap);
    const accountBalance = jest.fn((account: any, subAccountId?: string) => {
        if (subAccountId) {
            const subAccount = account.subAccounts.find((item: any) => item.id === subAccountId);
            return `cents:${subAccount?.balanceCents ?? 0}`;
        }
        return `cents:${account.balanceCents}`;
    });
    const accountCategoryTotalBalance = jest.fn((category: { type: number }) => {
        const cents = category.type === accountCategory.Cash.type ? 12_850 : 2_240;
        return `category-cents:${cents}`;
    });

    return {
        loading,
        showHidden,
        displayOrderModified,
        showAccountBalance,
        allCategorizedAccountsMap,
        allAccountCount: computed(() => accountsStore.allAvailableAccountsCount),
        netAssets: ref('CNY 150.90'),
        totalAssets: ref('CNY 151.95'),
        totalLiabilities: ref('CNY 1.05'),
        accountCategoryTotalBalance,
        accountBalance
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => values ? `${key}:${JSON.stringify(values)}` : key,
        getCurrentLanguageTextDirection: () => textDirection.LTR
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showAlert, showToast, routeBackOnError }),
    showLoading,
    hideLoading,
    onSwipeoutDeleted: (...args: any[]) => onSwipeoutDeleted(...args)
}));
jest.mock('@/views/base/accounts/AccountListPageBase.ts', () => ({
    useAccountListPageBase: () => {
        lastBase = createBase();
        return lastBase;
    }
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => rootStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => accountsStore }));
jest.mock('@/core/text.ts', () => ({ TextDirection: textDirection }));
jest.mock('@/core/account.ts', () => ({ AccountType: accountType, AccountCategory: accountCategory }));

import ListPage from '@/views/mobile/accounts/ListPage.vue';

function setup(): { bindings: any; router: any } {
    const router = { navigate: jest.fn(), back: jest.fn() };
    const bindings = (ListPage as any).setup(
        { f7router: router },
        { expose: jest.fn() }
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function render(bindings: any): unknown {
    const { proxyRefs } = jest.requireActual('vue') as any;
    const exposed = proxyRefs(bindings);
    return (ListPage as any).render(exposed, [], {}, exposed, {}, {});
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(): { app: any; root: any; router: any; state: any } {
    const { createRenderer, defineComponent, h } = jest.requireActual('vue') as any;
    const renderer = createRenderer({
        patchProp(node: any, key: string, _previous: unknown, value: unknown) {
            node.props[key] = value;
        },
        insert(child: any, parent: any, anchor: any = null) {
            child.parent = parent;
            if (!anchor) {
                parent.children.push(child);
                return;
            }
            const index = parent.children.indexOf(anchor);
            parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
        },
        remove(child: any) {
            const index = child.parent?.children.indexOf(child) ?? -1;
            if (index >= 0) child.parent.children.splice(index, 1);
        },
        createElement(type: string) {
            return createHostNode(type);
        },
        createText(text: string) {
            return createHostNode('#text', text);
        },
        createComment(text: string) {
            return createHostNode('#comment', text);
        },
        setText(node: any, text: string) {
            node.text = text;
        },
        setElementText(node: any, text: string) {
            node.text = text;
            node.children = [];
        },
        parentNode(node: any) {
            return node.parent;
        },
        nextSibling(node: any) {
            const siblings = node.parent?.children ?? [];
            return siblings[siblings.indexOf(node) + 1] ?? null;
        },
        querySelector() {
            return null;
        },
        setScopeId(node: any, id: string) {
            node.props[id] = '';
        },
        cloneNode(node: any) {
            return { ...node, children: [...node.children], props: { ...node.props }, parent: null };
        },
        insertStaticContent(content: string, parent: any, anchor: any) {
            const node = createHostNode('#static', content);
            node.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, node);
            return [node, node];
        }
    });
    const SlotHost = defineComponent({
        name: 'SlotHost',
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h('stub', attrs, Object.values(slots).flatMap((slot: any) => slot?.() ?? []));
        }
    });
    const router = { navigate: jest.fn(), back: jest.fn() };
    const app = renderer.createApp(ListPage as any, { f7router: router });
    for (const name of [
        'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link', 'f7-icon',
        'f7-card', 'f7-card-header', 'f7-list', 'f7-list-item', 'f7-badge', 'f7-swipeout-actions',
        'f7-swipeout-button', 'f7-actions', 'f7-actions-group', 'f7-actions-button', 'f7-actions-label',
        'password-input-sheet', 'ItemIcon'
    ]) {
        app.component(name, SlotHost);
    }
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, router, state: vm.$.setupState };
}

function collectHostCallbacks(
    node: any,
    callbacks: Array<{ name: string; callback: (...args: any[]) => any }>,
    seen = new Set<any>()
): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    for (const [name, value] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        if (typeof value === 'function') {
            callbacks.push({ name, callback: value as (...args: any[]) => any });
        } else if (Array.isArray(value)) {
            for (const callback of value) {
                if (typeof callback === 'function') callbacks.push({ name, callback });
            }
        }
    }
    for (const child of node.children ?? []) collectHostCallbacks(child, callbacks, seen);
}

async function invokeHostCallbacks(callbacks: Array<{ name: string; callback: (...args: any[]) => any }>): Promise<void> {
    for (const { name, callback } of callbacks) {
        if (name === 'onSortable:sort') {
            callback({ el: { id: 'account_cash' }, from: 2, to: 3 });
        } else if (name === 'onPtr:refresh') {
            callback(jest.fn());
        } else if (name === 'onPassword:confirm') {
            callback('operation-password');
        } else if (name === 'onUpdate:modelValue') {
            callback('operation-password');
        } else if (name.startsWith('onUpdate:')) {
            callback(true);
        } else {
            callback();
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    accountsStore.allAvailableAccountsCount = 5;
    accountsStore.allVisibleAccountsCount = 3;
    accountsStore.accountListStateInvalid = false;
    accountsStore.loadAllAccounts.mockResolvedValue(undefined);
    accountsStore.getFirstShowingIds.mockReturnValue({
        accounts: { 1: 'cash', 2: 'bank-parent' },
        subAccounts: { 'bank-parent': 'bank-sub-visible' }
    });
    accountsStore.getLastShowingIds.mockReturnValue({
        accounts: { 1: 'cash-hidden', 2: 'bank-parent' },
        subAccounts: { 'bank-parent': 'bank-sub-hidden' }
    });
    accountsStore.hasAccount.mockImplementation((category, visibleOnly) => {
        const accounts = categorizedAccountsMap[category.type]?.accounts ?? [];
        return accounts.some(account => !visibleOnly || !account.hidden);
    });
    accountsStore.hasVisibleSubAccount.mockImplementation((showHidden, account) => (
        account.subAccounts.some((item: any) => showHidden || !item.hidden)
    ));
    accountsStore.hideAccount.mockResolvedValue(undefined);
    accountsStore.deleteAccount.mockResolvedValue(undefined);
    accountsStore.updateAccountDisplayOrders.mockResolvedValue(undefined);
    accountsStore.changeAccountDisplayOrder.mockResolvedValue(undefined);
    rootStore.clearAllUserTransactionsOfAccount.mockResolvedValue(undefined);
    showLoading.mockImplementation((predicate?: () => unknown) => predicate?.());
});

describe('mobile account ListPage production behavior', () => {
    test('initializes accounts and exposes visibility, identity, and cents display contracts', async () => {
        const { bindings } = setup();
        expect(lastBase.loading.value).toBe(true);
        await flush();

        expect(lastBase.loading.value).toBe(false);
        expect(accountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(bindings.textDirection.value).toBe(textDirection.LTR);
        expect(bindings.firstShowingIds.value.accounts[1]).toBe('cash');
        expect(bindings.lastShowingIds.value.accounts[1]).toBe('cash-hidden');
        expect(bindings.hasAnyVisibleAccount.value).toBe(true);
        expect(bindings.noAvailableAccount.value).toBe(false);
        expect(bindings.hasAccount(accountCategory.Cash, true)).toBe(true);
        expect(bindings.hasVisibleSubAccount(multiAccount)).toBe(true);
        expect(bindings.getAccountDomId(visibleCash)).toBe('account_cash');
        expect(bindings.parseAccountIdFromDomId('account_cash')).toBe('cash');
        expect(bindings.parseAccountIdFromDomId('')).toBeNull();
        expect(bindings.parseAccountIdFromDomId('transaction_cash')).toBeNull();

        expect(lastBase.accountBalance(visibleCash)).toBe('cents:12345');
        expect(lastBase.accountBalance(multiAccount, visibleSubAccount.id)).toBe('cents:2345');
        expect(lastBase.accountCategoryTotalBalance(accountCategory.Cash)).toBe('category-cents:12850');

        accountsStore.allVisibleAccountsCount = 0;
        expect(bindings.hasAnyVisibleAccount.value).toBe(false);
        expect(bindings.noAvailableAccount.value).toBe(true);
        lastBase.showHidden.value = true;
        expect(bindings.noAvailableAccount.value).toBe(false);
        accountsStore.allAvailableAccountsCount = 0;
        expect(bindings.noAvailableAccount.value).toBe(true);
    });

    test('handles processed, readable, and raw initialization failures', async () => {
        accountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: true, message: 'handled' });
        setup();
        await flush();
        expect(lastBase.loading.value).toBe(false);
        expect(showToast).not.toHaveBeenCalledWith('handled');

        accountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'accounts failed' });
        const readable = setup();
        await flush();
        expect(readable.bindings.loadingError.value).toMatchObject({ message: 'accounts failed' });
        expect(showToast).toHaveBeenCalledWith('accounts failed');

        accountsStore.loadAllAccounts.mockRejectedValueOnce('raw accounts failure');
        setup();
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw accounts failure');
    });

    test('reloads normally and by pull-to-refresh while honoring sortable and error paths', async () => {
        const { bindings } = setup();
        await flush();
        accountsStore.loadAllAccounts.mockClear();

        bindings.reload();
        await flush();
        expect(accountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });

        const done = jest.fn();
        bindings.reload(done);
        await flush();
        expect(accountsStore.loadAllAccounts).toHaveBeenLastCalledWith({ force: true });
        expect(done).toHaveBeenCalled();
        expect(showToast).toHaveBeenCalledWith('Account list has been updated');

        bindings.sortable.value = true;
        accountsStore.loadAllAccounts.mockClear();
        const sortableDone = jest.fn();
        bindings.reload(sortableDone);
        expect(sortableDone).toHaveBeenCalled();
        expect(accountsStore.loadAllAccounts).not.toHaveBeenCalled();
        bindings.sortable.value = false;

        accountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: true, message: 'handled reload' });
        bindings.reload(jest.fn());
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled reload');

        accountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'reload failed' });
        bindings.reload(jest.fn());
        await flush();
        expect(showToast).toHaveBeenCalledWith('reload failed');

        accountsStore.loadAllAccounts.mockRejectedValueOnce('raw reload failure');
        bindings.reload(jest.fn());
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw reload failure');
    });

    test('navigates edit, reconciliation, move, password, and total-inclusion actions', () => {
        const { bindings, router } = setup();
        bindings.edit(visibleCash);
        expect(router.navigate).toHaveBeenLastCalledWith('/account/edit?id=cash');

        bindings.showMoreActionSheetForAccount(visibleCash);
        expect(bindings.accountForMoreActionSheet.value).toStrictEqual(visibleCash);
        expect(bindings.showAccountMoreActionSheet.value).toBe(true);

        bindings.showReconciliationStatement(null);
        bindings.moveAllTransactions(null);
        bindings.showPasswordSheetForClearAllTransaction(null);
        expect(showAlert).toHaveBeenCalledTimes(3);

        bindings.showMoreActionSheetForAccount(visibleCash);
        bindings.showReconciliationStatement(visibleCash);
        expect(router.navigate).toHaveBeenLastCalledWith('/account/reconciliation_statements?accountId=cash');
        expect(bindings.accountForMoreActionSheet.value).toBeNull();

        bindings.showMoreActionSheetForAccount(visibleCash);
        bindings.moveAllTransactions(visibleCash);
        expect(router.navigate).toHaveBeenLastCalledWith('/account/move_all_transactions?fromAccountId=cash');

        bindings.showMoreActionSheetForAccount(visibleCash);
        bindings.currentPasswordForClearData.value = 'old';
        bindings.showPasswordSheetForClearAllTransaction(visibleCash);
        expect(bindings.accountToClearTransactions.value).toStrictEqual(visibleCash);
        expect(bindings.currentPasswordForClearData.value).toBe('');
        expect(bindings.showInputPasswordSheetForClearAllTransactions.value).toBe(true);

        bindings.setAccountsIncludedInTotal();
        expect(router.navigate).toHaveBeenLastCalledWith('/settings/filter/account?type=accountListTotalAmount');
    });

    test('clears all transactions with account scope and handles all service outcomes', async () => {
        const { bindings } = setup();
        bindings.clearAllTransactions('ignored');
        expect(showAlert).toHaveBeenCalledWith('An error occurred');

        bindings.accountToClearTransactions.value = visibleCash;
        bindings.currentPasswordForClearData.value = 'operation-password';
        bindings.clearAllTransactions('operation-password');
        expect(showLoading).toHaveBeenCalled();
        expect(bindings.clearingData.value).toBe(true);
        await flush();
        expect(rootStore.clearAllUserTransactionsOfAccount).toHaveBeenCalledWith({
            accountId: 'cash', password: 'operation-password'
        });
        expect(bindings.clearingData.value).toBe(false);
        expect(bindings.currentPasswordForClearData.value).toBe('');
        expect(bindings.showInputPasswordSheetForClearAllTransactions.value).toBe(false);
        expect(showToast).toHaveBeenCalledWith('All transactions in this account has been cleared');

        rootStore.clearAllUserTransactionsOfAccount.mockRejectedValueOnce({ processed: true, message: 'handled clear' });
        bindings.clearAllTransactions('password');
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled clear');

        rootStore.clearAllUserTransactionsOfAccount.mockRejectedValueOnce({ processed: false, message: 'clear failed' });
        bindings.clearAllTransactions('password');
        await flush();
        expect(showToast).toHaveBeenCalledWith('clear failed');

        rootStore.clearAllUserTransactionsOfAccount.mockRejectedValueOnce('raw clear failure');
        bindings.clearAllTransactions('password');
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw clear failure');
    });

    test('hides and restores accounts and reports processed and unprocessed failures', async () => {
        const { bindings } = setup();
        bindings.hide(visibleCash, true);
        expect(accountsStore.hideAccount).toHaveBeenCalledWith({ account: visibleCash, hidden: true });
        await flush();
        expect(hideLoading).toHaveBeenCalled();

        accountsStore.hideAccount.mockRejectedValueOnce({ processed: true, message: 'handled hide' });
        bindings.hide(visibleCash, false);
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled hide');

        accountsStore.hideAccount.mockRejectedValueOnce({ processed: false, message: 'hide failed' });
        bindings.hide(visibleCash, false);
        await flush();
        expect(showToast).toHaveBeenCalledWith('hide failed');

        accountsStore.hideAccount.mockRejectedValueOnce('raw hide failure');
        bindings.hide(visibleCash, false);
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw hide failure');
    });

    test('prompts, deletes with swipeout completion, and reports delete failures', async () => {
        const { bindings } = setup();
        bindings.remove(null, false);
        expect(showAlert).toHaveBeenCalledWith('An error occurred');

        bindings.remove(visibleCash, false);
        expect(bindings.accountToDelete.value).toStrictEqual(visibleCash);
        expect(bindings.showDeleteActionSheet.value).toBe(true);

        accountsStore.deleteAccount.mockImplementationOnce(async ({ beforeResolve }: any) => {
            beforeResolve('done-callback');
        });
        bindings.remove(visibleCash, true);
        await flush();
        expect(onSwipeoutDeleted).toHaveBeenCalledWith('account_cash', 'done-callback');
        expect(bindings.accountToDelete.value).toBeNull();
        expect(bindings.showDeleteActionSheet.value).toBe(false);
        expect(hideLoading).toHaveBeenCalled();

        accountsStore.deleteAccount.mockRejectedValueOnce({ processed: true, message: 'handled delete' });
        bindings.remove(visibleCash, true);
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled delete');

        accountsStore.deleteAccount.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove(visibleCash, true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('delete failed');

        accountsStore.deleteAccount.mockRejectedValueOnce('raw delete failure');
        bindings.remove(visibleCash, true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw delete failure');
    });

    test('enters sorting and saves changed or unchanged display orders', async () => {
        const { bindings } = setup();
        bindings.setSortable();
        expect(bindings.sortable.value).toBe(true);
        expect(lastBase.showHidden.value).toBe(true);
        expect(lastBase.displayOrderModified.value).toBe(false);
        bindings.setSortable();
        expect(bindings.sortable.value).toBe(true);

        bindings.saveSortResult();
        expect(bindings.sortable.value).toBe(false);
        expect(lastBase.showHidden.value).toBe(false);
        expect(accountsStore.updateAccountDisplayOrders).not.toHaveBeenCalled();

        bindings.setSortable();
        lastBase.displayOrderModified.value = true;
        bindings.saveSortResult();
        expect(bindings.displayOrderSaving.value).toBe(true);
        await flush();
        expect(accountsStore.updateAccountDisplayOrders).toHaveBeenCalled();
        expect(bindings.displayOrderSaving.value).toBe(false);
        expect(bindings.sortable.value).toBe(false);
        expect(lastBase.showHidden.value).toBe(false);
        expect(lastBase.displayOrderModified.value).toBe(false);

        bindings.setSortable();
        lastBase.displayOrderModified.value = true;
        accountsStore.updateAccountDisplayOrders.mockRejectedValueOnce({ processed: true, message: 'handled sort' });
        bindings.saveSortResult();
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled sort');

        accountsStore.updateAccountDisplayOrders.mockRejectedValueOnce({ processed: false, message: 'sort failed' });
        bindings.saveSortResult();
        await flush();
        expect(showToast).toHaveBeenCalledWith('sort failed');

        accountsStore.updateAccountDisplayOrders.mockRejectedValueOnce('raw sort failure');
        bindings.saveSortResult();
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw sort failure');
    });

    test('validates sortable events and persists zero-based account display order changes', async () => {
        const { bindings } = setup();
        bindings.onSort(null);
        bindings.onSort({ el: { id: '' }, from: 1, to: 2 });
        bindings.onSort({ el: { id: 'transaction_cash' }, from: 1, to: 2 });
        expect(showToast).toHaveBeenCalledTimes(3);

        bindings.onSort({ el: { id: 'account_cash' }, from: 2, to: 4 });
        await flush();
        expect(accountsStore.changeAccountDisplayOrder).toHaveBeenCalledWith({
            accountId: 'cash', from: 1, to: 3, updateListOrder: true, updateGlobalListOrder: true
        });
        expect(lastBase.displayOrderModified.value).toBe(true);

        accountsStore.changeAccountDisplayOrder.mockRejectedValueOnce(new Error('move failed'));
        bindings.onSort({ el: { id: 'account_cash' }, from: 2, to: 3 });
        await flush();
        expect(showToast).toHaveBeenCalledWith('move failed');

        accountsStore.changeAccountDisplayOrder.mockRejectedValueOnce('raw move failure');
        bindings.onSort({ el: { id: 'account_cash' }, from: 2, to: 3 });
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw move failure');
    });

    test('refreshes invalid account state after page entry and always runs route recovery', async () => {
        const { bindings, router } = setup();
        await flush();
        accountsStore.loadAllAccounts.mockClear();
        accountsStore.accountListStateInvalid = true;

        lastBase.loading.value = true;
        bindings.onPageAfterIn();
        expect(accountsStore.loadAllAccounts).not.toHaveBeenCalled();

        lastBase.loading.value = false;
        bindings.onPageAfterIn();
        await flush();
        expect(accountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(routeBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });

    test('renders direct facade branches for loading, empty, populated, hidden, and sortable states', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const { bindings } = setup();
            expect(render(bindings)).toBeDefined();
            await flush();

            accountsStore.allVisibleAccountsCount = 0;
            expect(render(bindings)).toBeDefined();
            accountsStore.allVisibleAccountsCount = 3;
            lastBase.showHidden.value = true;
            bindings.accountForMoreActionSheet.value = multiAccount;
            expect(render(bindings)).toBeDefined();

            bindings.sortable.value = true;
            lastBase.displayOrderModified.value = true;
            expect(render(bindings)).toBeDefined();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('renders named slots and executes template handlers in the no-DOM Vue runtime', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const mounted = mountWithHostRenderer();
        try {
            const { nextTick } = jest.requireActual('vue') as any;
            await flush();
            await nextTick();

            mounted.state.accountForMoreActionSheet = visibleCash;
            mounted.state.accountToDelete = visibleCash;
            mounted.state.accountToClearTransactions = visibleCash;
            const normalCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(mounted.root, normalCallbacks);
            await invokeHostCallbacks(normalCallbacks);
            expect(normalCallbacks.length).toBeGreaterThan(10);

            mounted.state.sortable = true;
            mounted.state.showHidden = true;
            mounted.state.accountForMoreActionSheet = multiAccount;
            await nextTick();
            const sortableCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(mounted.root, sortableCallbacks);
            await invokeHostCallbacks(sortableCallbacks);

            mounted.state.loading = true;
            await nextTick();
            mounted.state.loading = false;
            accountsStore.allAvailableAccountsCount = 0;
            accountsStore.allVisibleAccountsCount = 0;
            mounted.state.showHidden = false;
            await nextTick();

            expect(mounted.root.children.length).toBeGreaterThan(0);
            expect(lastBase.accountBalance).toHaveBeenCalledWith(expect.objectContaining({ balanceCents: 12_345 }));
            expect(lastBase.accountBalance).toHaveBeenCalledWith(
                expect.objectContaining({ id: 'bank-parent' }),
                'bank-sub-visible'
            );
        } finally {
            mounted.app.unmount();
            warnSpy.mockRestore();
        }
    });
});
