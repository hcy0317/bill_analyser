import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { AccountCategory, AccountType } from '@/core/account.ts';
import { DISPLAY_HIDDEN_AMOUNT } from '@/consts/numeral.ts';
import { Account, type AccountInfoResponse } from '@/models/account.ts';

const mockSettingsStore = { appSettings: { totalAmountExcludeAccountIds: {} as Record<string, boolean> } };
const mockUserStore = { currentUserDefaultCurrency: 'CNY' };
const mockGetExchangedAmount = jest.fn<(amount: number, from: string, to: string) => number | null>();
const mockServices = {
    getAllAccounts: jest.fn<(request: unknown) => Promise<unknown>>(),
    getAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    addAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    modifyAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    moveAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    hideAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    deleteAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    deleteSubAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    syncAllAccountBalances: jest.fn<() => Promise<unknown>>()
};

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/exchangeRates.ts', () => ({
    useExchangeRatesStore: () => ({ getExchangedAmount: mockGetExchangedAmount })
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getAllAccounts: (request: unknown) => mockServices.getAllAccounts(request),
        getAccount: (request: unknown) => mockServices.getAccount(request),
        addAccount: (request: unknown) => mockServices.addAccount(request),
        modifyAccount: (request: unknown) => mockServices.modifyAccount(request),
        moveAccount: (request: unknown) => mockServices.moveAccount(request),
        hideAccount: (request: unknown) => mockServices.hideAccount(request),
        deleteAccount: (request: unknown) => mockServices.deleteAccount(request),
        deleteSubAccount: (request: unknown) => mockServices.deleteSubAccount(request),
        syncAllAccountBalances: () => mockServices.syncAllAccountBalances()
    }
}));

import { useAccountsStore } from '@/stores/account.ts';

function response(overrides: Partial<AccountInfoResponse> = {}): AccountInfoResponse {
    return {
        id: 'cash',
        name: 'Cash',
        parentId: '0',
        category: AccountCategory.Cash.type,
        type: AccountType.SingleAccount.type,
        icon: '1',
        color: '#fff',
        currency: 'CNY',
        balanceCents: 100,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function account(overrides: Partial<AccountInfoResponse> = {}): Account {
    return Account.of(response(overrides));
}

function success<T>(result: T): Promise<{ data: { success: true; result: T } }> {
    return Promise.resolve({ data: { success: true, result } });
}

const tree = (): AccountInfoResponse[] => [
    response({ id: 'cash', displayOrder: 1 }),
    response({ id: 'hidden', hidden: true, displayOrder: 2 }),
    response({
        id: 'bank',
        name: 'Bank',
        category: AccountCategory.CheckingAccount.type,
        type: AccountType.MultiSubAccounts.type,
        currency: '---',
        displayOrder: 3,
        subAccounts: [
            response({
                id: 'bank-hidden', parentId: 'bank', category: AccountCategory.CheckingAccount.type,
                currency: 'USD', hidden: true, displayOrder: 1
            }),
            response({
                id: 'bank-visible', parentId: 'bank', category: AccountCategory.CheckingAccount.type,
                currency: 'CNY', displayOrder: 2
            })
        ]
    }),
    response({
        id: 'empty-bank', category: AccountCategory.CheckingAccount.type,
        type: AccountType.MultiSubAccounts.type, currency: '---', displayOrder: 4, hidden: true
    }),
    response({
        id: 'visible-empty-bank', category: AccountCategory.CheckingAccount.type,
        type: AccountType.MultiSubAccounts.type, currency: '---', displayOrder: 5
    }),
    response({
        id: 'hidden-only-bank', category: AccountCategory.SavingsAccount.type,
        type: AccountType.MultiSubAccounts.type, currency: '---', displayOrder: 6,
        subAccounts: [response({
            id: 'hidden-only-child', parentId: 'hidden-only-bank', category: AccountCategory.SavingsAccount.type,
            hidden: true
        })]
    }),
    response({
        id: 'unknown-shape', category: AccountCategory.VirtualAccount.type,
        type: 999, displayOrder: 7, hidden: true
    })
];

async function loadTree() {
    mockServices.getAllAccounts.mockReturnValueOnce(success(tree()));
    const store = useAccountsStore();
    await store.loadAllAccounts({ force: true });
    return store;
}

describe('account store edge behavior', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        jest.clearAllMocks();
        mockSettingsStore.appSettings.totalAmountExcludeAccountIds = {};
        mockUserStore.currentUserDefaultCurrency = 'CNY';
        mockGetExchangedAmount.mockImplementation((amount, from, to) => from === to ? amount : amount * 7);
    });

    test('flattens parents with and without children and applies every visibility boundary', async () => {
        const store = await loadTree();

        expect(store.allPlainAccounts.map(item => item.id)).toEqual([
            'cash', 'hidden', 'bank-hidden', 'bank-visible', 'hidden-only-child'
        ]);
        expect(store.allMixedPlainAccounts.map(item => item.id)).toEqual([
            'cash', 'hidden', 'bank', 'bank-hidden', 'bank-visible', 'empty-bank', 'visible-empty-bank',
            'hidden-only-bank', 'hidden-only-child'
        ]);
        expect(store.allVisiblePlainAccounts.map(item => item.id)).toEqual(['cash', 'bank-visible']);
        expect(store.getFirstShowingIds(false).subAccounts['bank']).toBe('bank-visible');
        expect(store.getFirstShowingIds(true).subAccounts['bank']).toBe('bank-hidden');
        expect(store.getLastShowingIds(false).subAccounts['bank']).toBeUndefined();
        expect(store.getLastShowingIds(true).subAccounts['bank']).toBeUndefined();

        const bank = store.allAccountsMap['bank']!;
        const emptyBank = store.allAccountsMap['empty-bank']!;
        expect(store.getAccountSubAccountBalance(true, false, account())).toBeNull();
        expect(store.getAccountSubAccountBalance(true, false, emptyBank)).toEqual({ balance: 0, currency: 'CNY' });
        expect(store.getAccountSubAccountBalance(false, false, emptyBank)).toEqual({
            balance: DISPLAY_HIDDEN_AMOUNT, currency: 'CNY'
        });
        expect(store.getAccountSubAccountBalance(false, false, store.allAccountsMap['hidden-only-bank']!)).toEqual({
            balance: DISPLAY_HIDDEN_AMOUNT, currency: 'CNY'
        });
        expect(store.getAccountSubAccountBalance(true, false, bank, 'missing')).toBeNull();
        expect(store.getAccountSubAccountBalance(false, true, bank, 'bank-hidden')).toEqual({
            balance: DISPLAY_HIDDEN_AMOUNT, currency: 'USD'
        });
        expect(store.hasVisibleSubAccount(false, bank)).toBe(true);
        expect(store.hasVisibleSubAccount(true, bank)).toBe(true);
        expect(store.hasVisibleSubAccount(false, emptyBank)).toBe(false);
    });

    test('moves within a category in both directions and safely ignores unavailable global peers', async () => {
        const store = await loadTree();
        store.updateAccountListInvalidState(false);

        await expect(store.changeAccountDisplayOrder({
            accountId: 'empty-bank', from: 1, to: 0,
            updateListOrder: false, updateGlobalListOrder: true
        })).resolves.toBeUndefined();
        expect(store.accountListStateInvalid).toBe(true);

        await expect(store.changeAccountDisplayOrder({
            accountId: 'bank', from: 0, to: 1,
            updateListOrder: false, updateGlobalListOrder: false
        })).resolves.toBeUndefined();
        await expect(store.changeAccountDisplayOrder({
            accountId: 'bank', from: 0, to: 1,
            updateListOrder: true, updateGlobalListOrder: true
        })).resolves.toBeUndefined();
        await expect(store.changeAccountDisplayOrder({
            accountId: 'missing', from: 0, to: 1,
            updateListOrder: true, updateGlobalListOrder: true
        })).rejects.toEqual({ message: 'Unable to move account' });
    });

    test('adds, edits and invalidates accounts according to category membership', async () => {
        const store = await loadTree();
        const created = response({ id: 'new-cash', name: 'New cash', displayOrder: 9 });
        mockServices.addAccount.mockReturnValueOnce(success(created));
        await expect(store.saveAccount({
            account: account({ id: '', name: 'New cash' }), subAccounts: [], isEdit: false, clientSessionId: 'client'
        })).resolves.toEqual(expect.objectContaining({ id: 'new-cash' }));
        expect(store.allAccountsMap['new-cash']).toBeDefined();

        const oldBank = store.allAccountsMap['bank']!;
        delete store.allAccountsMap['bank-hidden'];
        mockServices.modifyAccount.mockReturnValueOnce(success(response({
            id: 'bank', name: 'Updated bank', category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type, currency: '---', subAccounts: []
        })));
        await store.saveAccount({ account: oldBank, subAccounts: [], isEdit: true, clientSessionId: 'client' });
        expect(store.allAccountsMap['bank']?.name).toBe('Updated bank');

        const updated = store.allAccountsMap['bank']!;
        store.updateAccountListInvalidState(false);
        mockServices.modifyAccount.mockReturnValueOnce(success(response({
            id: 'bank', category: AccountCategory.Cash.type
        })));
        await store.saveAccount({ account: updated, subAccounts: [], isEdit: true, clientSessionId: 'client' });
        expect(store.accountListStateInvalid).toBe(true);
    });

    test('tolerates stale category caches while editing, hiding and deleting detached records', async () => {
        const store = await loadTree();
        const cash = store.allAccountsMap['cash']!;
        const cashCategory = store.allCategorizedAccountsMap[AccountCategory.Cash.type]!;

        cashCategory.accounts.splice(0, cashCategory.accounts.length,
            ...cashCategory.accounts.filter(item => item.id !== cash.id));
        mockServices.modifyAccount.mockReturnValueOnce(success(response({ id: 'cash', name: 'Edited cash' })));
        await expect(store.saveAccount({ account: cash, subAccounts: [], isEdit: true, clientSessionId: 'client' }))
            .resolves.toEqual(expect.objectContaining({ name: 'Edited cash' }));

        const virtual = store.allAccountsMap['unknown-shape']!;
        delete store.allCategorizedAccountsMap[AccountCategory.VirtualAccount.type];
        mockServices.modifyAccount.mockReturnValueOnce(success(response({
            id: virtual.id, category: virtual.category, type: virtual.type
        })));
        await expect(store.saveAccount({ account: virtual, subAccounts: [], isEdit: true, clientSessionId: 'client' }))
            .resolves.toEqual(expect.objectContaining({ id: 'unknown-shape' }));

        delete store.allAccountsMap['cash'];
        mockServices.hideAccount.mockReturnValueOnce(success(true));
        await expect(store.hideAccount({ account: cash, hidden: true })).resolves.toBe(true);

        const detached = account({ id: 'detached', category: AccountCategory.Cash.type });
        mockServices.deleteAccount.mockReturnValueOnce(success(true));
        await expect(store.deleteAccount({ account: detached })).resolves.toBe(true);
        delete store.allCategorizedAccountsMap[AccountCategory.Cash.type];
        mockServices.deleteAccount.mockReturnValueOnce(success(true));
        await expect(store.deleteAccount({ account: detached })).resolves.toBe(true);

        const detachedChild = account({
            id: 'detached-child', parentId: 'detached-parent', category: AccountCategory.SavingsAccount.type
        });
        delete store.allCategorizedAccountsMap[AccountCategory.SavingsAccount.type];
        mockServices.deleteSubAccount.mockReturnValueOnce(success(true));
        await expect(store.deleteSubAccount({ subAccount: detachedChild })).resolves.toBe(true);
    });

    test('skips malformed category buckets and categories containing only hidden accounts', async () => {
        const store = await loadTree();
        (store.allCategorizedAccountsMap as Record<number, unknown>)[999] = null;

        expect(() => store.getFirstShowingIds(false)).not.toThrow();
        expect(() => store.getLastShowingIds(false)).not.toThrow();
        expect(store.getFirstShowingIds(false).accounts[AccountCategory.VirtualAccount.type]).toBeUndefined();
    });

    test('deletes parent and child entries even when some cached relationships are already absent', async () => {
        const store = await loadTree();
        const bank = store.allAccountsMap['bank']!;
        delete store.allAccountsMap['bank-hidden'];
        mockServices.deleteAccount.mockReturnValueOnce(success(true));
        await expect(store.deleteAccount({ account: bank })).resolves.toBe(true);
        expect(store.allAccountsMap['bank']).toBeUndefined();

        mockServices.getAllAccounts.mockReturnValueOnce(success(tree()));
        await store.loadAllAccounts({ force: true });
        const child = store.allAccountsMap['bank-visible']!;
        delete store.allAccountsMap['bank-visible'];
        mockServices.deleteSubAccount.mockReturnValueOnce(success(true));
        await expect(store.deleteSubAccount({ subAccount: child })).resolves.toBe(true);
        expect(store.allAccountsMap['bank']?.subAccounts?.map(item => item.id)).not.toContain('bank-visible');
    });

    test('keeps invalid-state false after an already-valid display-order save', async () => {
        const store = await loadTree();
        store.updateAccountListInvalidState(false);
        mockServices.moveAccount.mockReturnValueOnce(success(true));

        await expect(store.updateAccountDisplayOrders()).resolves.toBe(true);
        expect(store.accountListStateInvalid).toBe(false);
    });
});
