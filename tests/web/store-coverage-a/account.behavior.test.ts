import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { AccountCategory, AccountType } from '@/core/account.ts';
import { DISPLAY_HIDDEN_AMOUNT, INCOMPLETE_AMOUNT_SUFFIX } from '@/consts/numeral.ts';
import { Account, type AccountInfoResponse, type SyncBalancesResponse } from '@/models/account.ts';
import { useAccountsStore } from '@/stores/account.ts';

type ApiResponse<T> = { data: { success: boolean; result: T } };

const mockSettingsStore = {
    appSettings: {
        totalAmountExcludeAccountIds: {} as Record<string, boolean>
    }
};
const mockUserStore = { currentUserDefaultCurrency: 'CNY' };
const mockGetExchangedAmount = jest.fn<(amount: number, from: string, to: string) => number | null>();

const mockServices = {
    getAllAccounts: jest.fn<(request?: unknown) => Promise<unknown>>(),
    getAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    addAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    modifyAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    moveAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    hideAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    deleteAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    deleteSubAccount: jest.fn<(request: unknown) => Promise<unknown>>(),
    syncAllAccountBalances: jest.fn<() => Promise<unknown>>()
};

jest.mock('@/stores/setting.ts', () => ({
    __esModule: true,
    useSettingsStore: () => mockSettingsStore
}));

jest.mock('@/stores/user.ts', () => ({
    __esModule: true,
    useUserStore: () => mockUserStore
}));

jest.mock('@/stores/exchangeRates.ts', () => ({
    __esModule: true,
    useExchangeRatesStore: () => ({ getExchangedAmount: mockGetExchangedAmount })
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getAllAccounts: (request?: unknown) => mockServices.getAllAccounts(request),
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

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: jest.fn(),
        error: jest.fn()
    }
}));

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
        balanceCents: 1_000,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function account(overrides: Partial<AccountInfoResponse> = {}): Account {
    return Account.of(response(overrides));
}

function success<T>(result: T): Promise<ApiResponse<T>> {
    return Promise.resolve({ data: { success: true, result } });
}

function invalidResponse(): Promise<{ data: { success: false; result: null } }> {
    return Promise.resolve({ data: { success: false, result: null } });
}

function accountTree(): AccountInfoResponse[] {
    return [
        response({ id: 'cash', name: 'Cash', balanceCents: 1_000, displayOrder: 1 }),
        response({
            id: 'bank-parent',
            name: 'Bank',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: '---',
            balanceCents: 0,
            displayOrder: 2,
            subAccounts: [
                response({
                    id: 'bank-cny',
                    name: 'Bank CNY',
                    parentId: 'bank-parent',
                    category: AccountCategory.CheckingAccount.type,
                    currency: 'CNY',
                    balanceCents: 2_000,
                    displayOrder: 1
                }),
                response({
                    id: 'bank-usd',
                    name: 'Bank USD',
                    parentId: 'bank-parent',
                    category: AccountCategory.CheckingAccount.type,
                    currency: 'USD',
                    balanceCents: 300,
                    displayOrder: 2
                }),
                response({
                    id: 'bank-hidden',
                    name: 'Hidden bank',
                    parentId: 'bank-parent',
                    category: AccountCategory.CheckingAccount.type,
                    balanceCents: 9_999,
                    displayOrder: 3,
                    hidden: true
                })
            ]
        }),
        response({
            id: 'credit',
            name: 'Credit',
            category: AccountCategory.CreditCard.type,
            balanceCents: -500,
            creditCardStatementDate: 18,
            displayOrder: 3
        }),
        response({
            id: 'neutral',
            name: 'Neutral',
            category: AccountCategory.VirtualAccount.type,
            balanceCents: 70,
            isAsset: false,
            isLiability: false,
            displayOrder: 4
        }),
        response({
            id: 'empty-parent',
            name: 'Empty',
            category: AccountCategory.SavingsAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: '---',
            balanceCents: 0,
            displayOrder: 5,
            subAccounts: []
        }),
        response({
            id: 'hidden-cash',
            name: 'Hidden cash',
            balanceCents: 9_000,
            displayOrder: 6,
            hidden: true
        })
    ];
}

async function loadTree() {
    mockServices.getAllAccounts.mockReturnValue(success(accountTree()));
    const store = useAccountsStore();
    await store.loadAllAccounts({ force: true });
    return store;
}

describe('account store behavior coverage', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        mockSettingsStore.appSettings.totalAmountExcludeAccountIds = {};
        mockUserStore.currentUserDefaultCurrency = 'CNY';
        mockGetExchangedAmount.mockReset();
        mockGetExchangedAmount.mockImplementation((amount, from, to) => {
            if (from === to) return amount;
            if (from === 'USD' && to === 'CNY') return amount * 7;
            return null;
        });
        for (const service of Object.values(mockServices)) {
            service.mockReset();
        }
    });

    test('materializes account trees, visibility counts and showing ids', async () => {
        const store = await loadTree();

        expect(store.allPlainAccounts.map(item => item.id)).toEqual([
            'cash', 'hidden-cash', 'bank-cny', 'bank-usd', 'bank-hidden', 'credit', 'neutral'
        ]);
        expect(store.allMixedPlainAccounts.map(item => item.id)).toContain('bank-parent');
        expect(store.allMixedPlainAccounts.map(item => item.id)).toContain('empty-parent');
        expect(store.allVisiblePlainAccounts.map(item => item.id)).toEqual([
            'cash', 'bank-cny', 'bank-usd', 'credit', 'neutral'
        ]);
        expect(store.allAvailableAccountsCount).toBe(6);
        expect(store.allVisibleAccountsCount).toBe(5);

        expect(store.getFirstShowingIds(false)).toEqual({
            accounts: expect.objectContaining({
                [AccountCategory.Cash.type]: 'cash',
                [AccountCategory.CheckingAccount.type]: 'bank-parent'
            }),
            subAccounts: { 'bank-parent': 'bank-cny' }
        });
        expect(store.getLastShowingIds(false)).toEqual({
            accounts: expect.objectContaining({
                [AccountCategory.Cash.type]: 'cash',
                [AccountCategory.CheckingAccount.type]: 'bank-parent'
            }),
            subAccounts: { 'bank-parent': 'bank-usd' }
        });
        expect(store.getFirstShowingIds(true).accounts[AccountCategory.Cash.type]).toBe('cash');
        expect(store.getLastShowingIds(true).accounts[AccountCategory.Cash.type]).toBe('hidden-cash');

        expect(store.hasAccount(AccountCategory.Cash, false)).toBe(true);
        expect(store.hasAccount(AccountCategory.InvestmentAccount, false)).toBe(false);
        expect(store.hasAccount(AccountCategory.Cash, true)).toBe(true);
        store.allAccountsMap['cash']!.visible = false;
        store.allAccountsMap['hidden-cash']!.visible = false;
        expect(store.hasAccount(AccountCategory.Cash, true)).toBe(false);

        expect(store.hasVisibleSubAccount(false, store.allAccountsMap['bank-parent']!)).toBe(true);
        expect(store.hasVisibleSubAccount(false, store.allAccountsMap['empty-parent']!)).toBe(false);
        expect(store.hasVisibleSubAccount(true, store.allAccountsMap['bank-parent']!)).toBe(true);
        expect(store.hasVisibleSubAccount(false, store.allAccountsMap['cash']!)).toBe(false);
    });

    test('resolves statement dates only for one credit-card parent', async () => {
        const store = await loadTree();

        expect(store.getAccountStatementDate()).toBeNull();
        expect(store.getAccountStatementDate('missing')).toBeNull();
        expect(store.getAccountStatementDate('cash')).toBeNull();
        expect(store.getAccountStatementDate('credit')).toBe(18);
        expect(store.getAccountStatementDate('cash,credit')).toBeNull();

        const parent = account({
            id: 'credit-parent',
            category: AccountCategory.CreditCard.type,
            type: AccountType.MultiSubAccounts.type,
            creditCardStatementDate: 12,
            subAccounts: [
                response({
                    id: 'credit-a',
                    parentId: 'credit-parent',
                    category: AccountCategory.CreditCard.type
                }),
                response({
                    id: 'credit-b',
                    parentId: 'credit-parent',
                    category: AccountCategory.CreditCard.type
                })
            ]
        });
        store.allAccounts.push(parent);
        store.allAccountsMap[parent.id] = parent;
        for (const child of parent.subAccounts!) store.allAccountsMap[child.id] = child;
        expect(store.getAccountStatementDate('credit-a,credit-b')).toBe(12);
        delete store.allAccountsMap['credit-parent'];
        expect(store.getAccountStatementDate('credit-a')).toBeNull();
    });

    test('calculates assets, liabilities, category balances and incomplete exchange results in cents', async () => {
        const store = await loadTree();

        expect(store.getNetAssets(false)).toBe(DISPLAY_HIDDEN_AMOUNT);
        expect(store.getTotalAssets(false)).toBe(DISPLAY_HIDDEN_AMOUNT);
        expect(store.getTotalLiabilities(false)).toBe(DISPLAY_HIDDEN_AMOUNT);
        expect(store.getAccountCategoryTotalBalance(false, AccountCategory.Cash)).toBe(DISPLAY_HIDDEN_AMOUNT);

        expect(store.getNetAssets(true)).toBe(4_670);
        expect(store.getTotalAssets(true)).toBe(5_100);
        expect(store.getTotalLiabilities(true)).toBe(500);
        expect(store.getAccountCategoryTotalBalance(true, AccountCategory.CheckingAccount)).toBe(4_100);
        expect(store.getAccountCategoryTotalBalance(true, AccountCategory.CreditCard)).toBe(500);
        expect(store.getAccountCategoryTotalBalance(true, AccountCategory.VirtualAccount)).toBe(70);

        mockSettingsStore.appSettings.totalAmountExcludeAccountIds['cash'] = true;
        expect(store.getNetAssets(true)).toBe(3_670);
        expect(store.getTotalAssets(true)).toBe(4_100);

        mockGetExchangedAmount.mockReturnValue(null);
        expect(store.getNetAssets(true)).toEqual({ value: 1_570, suffix: INCOMPLETE_AMOUNT_SUFFIX });
        expect(store.getTotalAssets(true)).toEqual({ value: 2_000, suffix: INCOMPLETE_AMOUNT_SUFFIX });
        expect(store.getTotalLiabilities(true)).toBe(500);
        expect(store.getAccountCategoryTotalBalance(true, AccountCategory.CheckingAccount)).toEqual({
            value: 2_000,
            suffix: INCOMPLETE_AMOUNT_SUFFIX
        });
    });

    test('converts foreign liability and neutral balances and marks failed liability conversion incomplete', async () => {
        mockServices.getAllAccounts.mockReturnValue(success([
            response({
                id: 'foreign-debt',
                category: AccountCategory.DebtAccount.type,
                currency: 'USD',
                balanceCents: -100
            }),
            response({
                id: 'foreign-neutral',
                category: AccountCategory.VirtualAccount.type,
                currency: 'USD',
                balanceCents: 20,
                isAsset: false,
                isLiability: false
            })
        ]));
        const store = useAccountsStore();
        await store.loadAllAccounts({ force: true });

        expect(store.getNetAssets(true)).toBe(-560);
        expect(store.getTotalLiabilities(true)).toBe(700);
        expect(store.getAccountCategoryTotalBalance(true, AccountCategory.DebtAccount)).toBe(700);
        expect(store.getAccountCategoryTotalBalance(true, AccountCategory.VirtualAccount)).toBe(140);

        mockGetExchangedAmount.mockReturnValue(null);
        expect(store.getTotalLiabilities(true)).toEqual({ value: 0, suffix: INCOMPLETE_AMOUNT_SUFFIX });
        expect(store.getAccountCategoryTotalBalance(true, AccountCategory.DebtAccount)).toEqual({
            value: 0,
            suffix: INCOMPLETE_AMOUNT_SUFFIX
        });
    });

    test('returns single and sub-account balances for visible, hidden, selected and mixed-currency cases', async () => {
        const store = await loadTree();
        const cash = store.allAccountsMap['cash']!;
        const credit = store.allAccountsMap['credit']!;
        const neutral = store.allAccountsMap['neutral']!;
        const bank = store.allAccountsMap['bank-parent']!;
        const empty = store.allAccountsMap['empty-parent']!;

        expect(store.getAccountBalance(true, cash)).toBe(1_000);
        expect(store.getAccountBalance(true, credit)).toBe(500);
        expect(store.getAccountBalance(true, neutral)).toBe(70);
        expect(store.getAccountBalance(false, cash)).toBe(DISPLAY_HIDDEN_AMOUNT);
        expect(store.getAccountBalance(true, bank)).toBeNull();

        expect(store.getAccountSubAccountBalance(true, false, cash)).toBeNull();
        expect(store.getAccountSubAccountBalance(true, false, empty)).toEqual({ balance: 0, currency: 'CNY' });
        expect(store.getAccountSubAccountBalance(false, false, empty)).toEqual({
            balance: DISPLAY_HIDDEN_AMOUNT,
            currency: 'CNY'
        });
        expect(store.getAccountSubAccountBalance(true, false, bank, 'bank-usd')).toEqual({
            balance: 300,
            currency: 'USD'
        });
        expect(store.getAccountSubAccountBalance(false, false, bank, 'bank-usd')).toEqual({
            balance: DISPLAY_HIDDEN_AMOUNT,
            currency: 'USD'
        });
        expect(store.getAccountSubAccountBalance(true, false, bank, 'missing')).toBeNull();
        expect(store.getAccountSubAccountBalance(true, false, bank)).toEqual({
            balance: { value: 4_100, suffix: '' },
            currency: 'CNY'
        });
        expect(store.getAccountSubAccountBalance(false, false, bank)).toEqual({
            balance: DISPLAY_HIDDEN_AMOUNT,
            currency: 'CNY'
        });

        mockGetExchangedAmount.mockReturnValue(null);
        expect(store.getAccountSubAccountBalance(true, false, bank)).toEqual({
            balance: { value: 2_000, suffix: INCOMPLETE_AMOUNT_SUFFIX },
            currency: 'CNY'
        });

        for (const child of bank.subAccounts!) child.visible = false;
        expect(store.getAccountSubAccountBalance(true, false, bank)).toEqual({ balance: 0, currency: 'CNY' });
        expect(store.getAccountSubAccountBalance(true, true, bank)).toEqual({
            balance: expect.objectContaining({ suffix: INCOMPLETE_AMOUNT_SUFFIX }),
            currency: 'CNY'
        });
    });

    test('aggregates one-currency asset, liability and neutral sub-accounts without exchange', async () => {
        const parent = account({
            id: 'mixed-parent',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: '---',
            subAccounts: [
                response({ id: 'asset', parentId: 'mixed-parent', balanceCents: 500, isAsset: true, isLiability: false }),
                response({ id: 'liability', parentId: 'mixed-parent', balanceCents: -200, isAsset: false, isLiability: true }),
                response({ id: 'neutral-sub', parentId: 'mixed-parent', balanceCents: 30, isAsset: false, isLiability: false })
            ]
        });
        mockServices.getAllAccounts.mockReturnValue(success([parent]));
        const store = useAccountsStore();
        await store.loadAllAccounts({ force: true });

        expect(store.getAccountSubAccountBalance(true, true, store.allAccountsMap['mixed-parent']!)).toEqual({
            balance: { value: 730, suffix: '' },
            currency: 'CNY'
        });
    });

    test('converts mixed-currency liability and neutral sub-accounts', async () => {
        const parent = account({
            id: 'foreign-parent',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: '---',
            subAccounts: [
                response({ id: 'base', parentId: 'foreign-parent', currency: 'CNY', balanceCents: 1 }),
                response({ id: 'foreign-liability', parentId: 'foreign-parent', currency: 'USD', balanceCents: -20, isAsset: false, isLiability: true }),
                response({ id: 'foreign-neutral', parentId: 'foreign-parent', currency: 'USD', balanceCents: 3, isAsset: false, isLiability: false })
            ]
        });
        mockServices.getAllAccounts.mockReturnValue(success([parent]));
        const store = useAccountsStore();
        await store.loadAllAccounts({ force: true });

        expect(store.getAccountSubAccountBalance(true, true, store.allAccountsMap['foreign-parent']!)).toEqual({
            balance: { value: 162, suffix: '' },
            currency: 'CNY'
        });
    });

    test('loads, caches, force-compares and resets account lists', async () => {
        const store = await loadTree();
        expect(store.accountListStateInvalid).toBe(false);
        expect(mockServices.getAllAccounts).toHaveBeenCalledWith({ visibleOnly: false });

        await expect(store.loadAllAccounts({ force: false })).resolves.toHaveLength(6);
        expect(mockServices.getAllAccounts).toHaveBeenCalledTimes(1);

        mockServices.getAllAccounts.mockReturnValue(success(accountTree()));
        await expect(store.loadAllAccounts({ force: true })).resolves.toHaveLength(6);
        expect(store.allAccountsMap['cash']).toBeDefined();

        store.resetAccounts();
        expect(store.allAccounts).toEqual([]);
        expect(store.allAccountsMap).toEqual({});
        expect(store.allCategorizedAccountsMap).toEqual({});
        expect(store.accountListStateInvalid).toBe(true);
    });

    test('deduplicates in-flight loads and rejects invalid or failed loads with stable errors', async () => {
        let resolveLoad!: (value: unknown) => void;
        mockServices.getAllAccounts.mockReturnValue(new Promise(resolve => { resolveLoad = resolve; }));
        const store = useAccountsStore();
        const first = store.loadAllAccounts({ force: false });
        const second = store.loadAllAccounts({ force: true });
        expect(mockServices.getAllAccounts).toHaveBeenCalledTimes(1);
        resolveLoad(await success(accountTree()));
        await expect(Promise.all([first, second])).resolves.toHaveLength(2);

        for (const [failure, expected] of [
            [invalidResponse(), { message: 'Unable to retrieve account list' }],
            [Promise.reject({ response: { data: { message: 'backend detail' } } }), { error: { message: 'backend detail' } }],
            [Promise.reject({ processed: false }), { message: 'Unable to retrieve account list' }],
            [Promise.reject({ processed: true, marker: 1 }), { processed: true, marker: 1 }]
        ] as const) {
            setActivePinia(createPinia());
            mockServices.getAllAccounts.mockReturnValue(failure);
            await expect(useAccountsStore().loadAllAccounts({ force: true })).rejects.toMatchObject(expected);
        }

        setActivePinia(createPinia());
        mockServices.getAllAccounts.mockReturnValue(Promise.reject({ processed: false }));
        await expect(useAccountsStore().loadAllAccounts({ force: false })).rejects.toEqual({
            message: 'Unable to retrieve account list'
        });
    });

    test('gets an account and preserves the three service error channels', async () => {
        const store = useAccountsStore();
        mockServices.getAccount.mockReturnValue(success(response({ id: 'one' })));
        await expect(store.getAccount({ accountId: 'one' })).resolves.toMatchObject({ id: 'one' });
        expect(mockServices.getAccount).toHaveBeenCalledWith({ id: 'one' });

        for (const [failure, expected] of [
            [invalidResponse(), { message: 'Unable to retrieve account' }],
            [Promise.reject({ response: { data: { message: 'detail' } } }), { error: { message: 'detail' } }],
            [Promise.reject({ processed: false }), { message: 'Unable to retrieve account' }],
            [Promise.reject({ processed: true, marker: 2 }), { processed: true, marker: 2 }]
        ] as const) {
            mockServices.getAccount.mockReturnValue(failure);
            await expect(store.getAccount({ accountId: 'bad' })).rejects.toMatchObject(expected);
        }
    });

    test('adds and edits accounts while keeping local indexes coherent', async () => {
        const store = await loadTree();
        const created = account({
            id: 'new-saving',
            name: 'New saving',
            category: AccountCategory.SavingsAccount.type,
            displayOrder: 1
        });
        mockServices.addAccount.mockReturnValue(success(response({
            id: created.id,
            name: created.name,
            category: created.category,
            displayOrder: created.displayOrder
        })));
        await expect(store.saveAccount({
            account: created,
            subAccounts: [],
            isEdit: false,
            clientSessionId: 'create-session'
        })).resolves.toMatchObject({ id: 'new-saving' });
        expect(store.allAccountsMap['new-saving']).toBeDefined();
        expect(mockServices.addAccount).toHaveBeenCalledWith(expect.objectContaining({
            name: 'New saving',
            clientSessionId: 'create-session'
        }));

        const edited = account({ id: 'cash', name: 'Edited cash', balanceCents: 2_222 });
        mockServices.modifyAccount.mockReturnValue(success(response({
            id: 'cash',
            name: 'Edited cash',
            balanceCents: 2_222
        })));
        await store.saveAccount({ account: edited, subAccounts: [], isEdit: true, clientSessionId: 'edit-session' });
        expect(store.allAccountsMap['cash']!.name).toBe('Edited cash');

        store.updateAccountListInvalidState(false);
        const movedCategory = account({ id: 'cash', category: AccountCategory.InvestmentAccount.type });
        mockServices.modifyAccount.mockReturnValue(success(response({
            id: 'cash',
            category: AccountCategory.InvestmentAccount.type
        })));
        await store.saveAccount({ account: movedCategory, subAccounts: [], isEdit: true, clientSessionId: 'move-session' });
        expect(store.accountListStateInvalid).toBe(true);
    });

    test('indexes newly created sub-accounts, unknown categories and replacement children', async () => {
        const store = await loadTree();
        const newParent = account({
            id: 'new-parent',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: '---',
            subAccounts: [response({ id: 'new-child', parentId: 'new-parent', category: AccountCategory.CheckingAccount.type })]
        });
        mockServices.addAccount.mockReturnValue(success(response({
            id: newParent.id,
            category: newParent.category,
            type: newParent.type,
            currency: newParent.currency,
            subAccounts: [response({ id: 'new-child', parentId: 'new-parent', category: newParent.category })]
        })));
        await store.saveAccount({ account: newParent, subAccounts: newParent.subAccounts!, isEdit: false, clientSessionId: 'p' });
        expect(store.allAccountsMap['new-child']).toBeDefined();

        const unknown = account({ id: 'unknown-category', category: 999 });
        mockServices.addAccount.mockReturnValue(success(response({ id: unknown.id, category: 999 })));
        await store.saveAccount({ account: unknown, subAccounts: [], isEdit: false, clientSessionId: 'u' });
        expect(store.allAccountsMap['unknown-category']).toBeDefined();

        const replacement = account({
            id: 'bank-parent',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: '---',
            subAccounts: [response({
                id: 'replacement-child',
                parentId: 'bank-parent',
                category: AccountCategory.CheckingAccount.type
            })]
        });
        mockServices.modifyAccount.mockReturnValue(success(response({
            id: replacement.id,
            category: replacement.category,
            type: replacement.type,
            currency: replacement.currency,
            subAccounts: [response({
                id: 'replacement-child',
                parentId: 'bank-parent',
                category: AccountCategory.CheckingAccount.type
            })]
        })));
        await store.saveAccount({ account: replacement, subAccounts: replacement.subAccounts!, isEdit: true, clientSessionId: 'r' });
        expect(store.allAccountsMap['bank-cny']).toBeUndefined();
        expect(store.allAccountsMap['replacement-child']).toBeDefined();
    });

    test('saveAccount distinguishes validation and transport failures for create and edit', async () => {
        const store = useAccountsStore();
        const draft = account({ id: 'draft' });

        for (const isEdit of [false, true]) {
            const service = isEdit ? mockServices.modifyAccount : mockServices.addAccount;
            service.mockReturnValue(invalidResponse());
            await expect(store.saveAccount({ account: draft, subAccounts: [], isEdit, clientSessionId: 's' }))
                .rejects.toEqual({ message: isEdit ? 'Unable to save account' : 'Unable to add account' });

            service.mockReturnValue(Promise.reject({ response: { data: { message: 'api says no' } } }));
            await expect(store.saveAccount({ account: draft, subAccounts: [], isEdit, clientSessionId: 's' }))
                .rejects.toEqual({ message: 'api says no' });

            service.mockReturnValue(Promise.reject({ processed: false }));
            await expect(store.saveAccount({ account: draft, subAccounts: [], isEdit, clientSessionId: 's' }))
                .rejects.toEqual({ message: isEdit ? 'Unable to save account' : 'Unable to add account' });

            const processed = { processed: true, marker: isEdit ? 2 : 1 };
            service.mockReturnValue(Promise.reject(processed));
            await expect(store.saveAccount({ account: draft, subAccounts: [], isEdit, clientSessionId: 's' }))
                .rejects.toBe(processed);
        }
    });

    test('reorders categories locally and serializes all display orders', async () => {
        const store = await loadTree();
        store.updateAccountListInvalidState(false);
        const before = store.allCategorizedAccountsMap[AccountCategory.Cash.type]!.accounts.map(item => item.id);
        await store.changeAccountDisplayOrder({
            accountId: 'hidden-cash',
            from: 1,
            to: 0,
            updateListOrder: true,
            updateGlobalListOrder: true
        });
        await store.changeAccountDisplayOrder({
            accountId: 'cash',
            from: 1,
            to: 0,
            updateListOrder: false,
            updateGlobalListOrder: true
        });
        expect(before).toEqual(['cash', 'hidden-cash']);
        expect(store.allCategorizedAccountsMap[AccountCategory.Cash.type]!.accounts.map(item => item.id))
            .toEqual(['hidden-cash', 'cash']);
        expect(store.accountListStateInvalid).toBe(true);

        await store.changeAccountDisplayOrder({
            accountId: 'hidden-cash',
            from: 0,
            to: 1,
            updateListOrder: false,
            updateGlobalListOrder: true
        });
        await expect(store.changeAccountDisplayOrder({
            accountId: 'missing', from: 0, to: 1, updateListOrder: true, updateGlobalListOrder: true
        })).rejects.toEqual({ message: 'Unable to move account' });

        mockServices.moveAccount.mockReturnValue(success(true));
        await expect(store.updateAccountDisplayOrders()).resolves.toBe(true);
        expect(mockServices.moveAccount).toHaveBeenCalledWith({
            newDisplayOrders: expect.arrayContaining([
                { id: 'cash', displayOrder: expect.any(Number) },
                { id: 'credit', displayOrder: expect.any(Number) }
            ])
        });
        expect(store.accountListStateInvalid).toBe(false);
    });

    test('updateAccountDisplayOrders propagates invalid and transport failures', async () => {
        const store = useAccountsStore();
        for (const [failure, expected] of [
            [invalidResponse(), { message: 'Unable to move account' }],
            [Promise.reject({ response: { data: { message: 'detail' } } }), { error: { message: 'detail' } }],
            [Promise.reject({ processed: false }), { message: 'Unable to move account' }],
            [Promise.reject({ processed: true, marker: 3 }), { processed: true, marker: 3 }]
        ] as const) {
            mockServices.moveAccount.mockReturnValue(failure);
            await expect(store.updateAccountDisplayOrders()).rejects.toMatchObject(expected);
        }
    });

    test('hides and unhides accounts and reports operation-specific failures', async () => {
        const store = await loadTree();
        const cash = store.allAccountsMap['cash']!;
        mockServices.hideAccount.mockReturnValue(success(true));
        await expect(store.hideAccount({ account: cash, hidden: true })).resolves.toBe(true);
        expect(cash.hidden).toBe(true);
        await expect(store.hideAccount({ account: cash, hidden: false })).resolves.toBe(true);
        expect(cash.hidden).toBe(false);

        for (const hidden of [true, false]) {
            mockServices.hideAccount.mockReturnValue(invalidResponse());
            await expect(store.hideAccount({ account: cash, hidden })).rejects.toEqual({
                message: hidden ? 'Unable to hide this account' : 'Unable to unhide this account'
            });
            mockServices.hideAccount.mockReturnValue(Promise.reject({ processed: false }));
            await expect(store.hideAccount({ account: cash, hidden })).rejects.toEqual({
                message: hidden ? 'Unable to hide this account' : 'Unable to unhide this account'
            });
        }
        mockServices.hideAccount.mockReturnValue(Promise.reject({ response: { data: { message: 'detail' } } }));
        await expect(store.hideAccount({ account: cash, hidden: true })).rejects.toEqual({ error: { message: 'detail' } });
        const processed = { processed: true, marker: 4 };
        mockServices.hideAccount.mockReturnValue(Promise.reject(processed));
        await expect(store.hideAccount({ account: cash, hidden: true })).rejects.toBe(processed);
    });

    test('deletes parent and sub-accounts directly or through beforeResolve callbacks', async () => {
        const store = await loadTree();
        const parent = store.allAccountsMap['bank-parent']!;
        const child = store.allAccountsMap['bank-cny']!;
        mockServices.deleteAccount.mockReturnValue(success(true));
        let parentMutation: (() => void) | undefined;
        await expect(store.deleteAccount({
            account: parent,
            beforeResolve: mutation => { parentMutation = mutation; }
        })).resolves.toBe(true);
        expect(store.allAccountsMap['bank-parent']).toBeDefined();
        parentMutation!();
        expect(store.allAccountsMap['bank-parent']).toBeUndefined();
        expect(store.allAccountsMap['bank-usd']).toBeUndefined();

        const reloaded = await loadTree();
        const directChild = reloaded.allAccountsMap['bank-cny']!;
        mockServices.deleteSubAccount.mockReturnValue(success(true));
        await expect(reloaded.deleteSubAccount({ subAccount: directChild })).resolves.toBe(true);
        expect(reloaded.allAccountsMap['bank-cny']).toBeUndefined();
        expect(reloaded.allAccountsMap['bank-parent']!.subAccounts!.map(item => item.id)).not.toContain('bank-cny');

        let childMutation: (() => void) | undefined;
        mockServices.deleteSubAccount.mockReturnValue(success(true));
        await reloaded.deleteSubAccount({
            subAccount: reloaded.allAccountsMap['bank-usd']!,
            beforeResolve: mutation => { childMutation = mutation; }
        });
        expect(reloaded.allAccountsMap['bank-usd']).toBeDefined();
        childMutation!();
        expect(reloaded.allAccountsMap['bank-usd']).toBeUndefined();

        mockServices.deleteAccount.mockReturnValue(success(true));
        await reloaded.deleteAccount({ account: reloaded.allAccountsMap['cash']! });
        expect(reloaded.allAccountsMap['cash']).toBeUndefined();
        expect(child.id).toBe('bank-cny');
    });

    test('delete operations preserve response, backend, unprocessed and processed errors', async () => {
        const store = useAccountsStore();
        const item = account();
        const cases = [
            { service: mockServices.deleteAccount, call: () => store.deleteAccount({ account: item }), fallback: 'Unable to delete this account' },
            { service: mockServices.deleteSubAccount, call: () => store.deleteSubAccount({ subAccount: item }), fallback: 'Unable to delete this sub-account' }
        ];
        for (const { service, call, fallback } of cases) {
            service.mockReturnValue(invalidResponse());
            await expect(call()).rejects.toEqual({ message: fallback });
            service.mockReturnValue(Promise.reject({ response: { data: { message: 'detail' } } }));
            await expect(call()).rejects.toEqual({ error: { message: 'detail' } });
            service.mockReturnValue(Promise.reject({ processed: false }));
            await expect(call()).rejects.toEqual({ message: fallback });
            const processed = { processed: true, marker: 5 };
            service.mockReturnValue(Promise.reject(processed));
            await expect(call()).rejects.toBe(processed);
        }
    });

    test('syncs balances, optionally refreshes, deduplicates and maps snake-case fields', async () => {
        const syncResult: SyncBalancesResponse = {
            total_accounts: 2,
            synced_accounts: 1,
            discrepancies: [{
                account_id: 7,
                name: 'Cash',
                oldBalanceCents: 10,
                newBalanceCents: 12,
                diffCents: 2
            }],
            errors: ['one skipped']
        };
        let resolveSync!: (value: unknown) => void;
        mockServices.syncAllAccountBalances.mockReturnValue(new Promise(resolve => { resolveSync = resolve; }));
        mockServices.getAllAccounts.mockReturnValue(success(accountTree()));
        const store = useAccountsStore();
        const first = store.syncAllAccountBalances();
        const second = store.syncAllAccountBalances();
        expect(mockServices.syncAllAccountBalances).toHaveBeenCalledTimes(1);
        resolveSync(await success(syncResult));
        await expect(first).resolves.toEqual({
            totalAccounts: 2,
            syncedAccounts: 1,
            discrepancies: [{
                accountId: 7,
                name: 'Cash',
                oldBalanceCents: 10,
                newBalanceCents: 12,
                diffCents: 2
            }],
            errors: ['one skipped']
        });
        await expect(second).resolves.toEqual(await first);
        expect(mockServices.getAllAccounts).toHaveBeenCalledTimes(1);

        setActivePinia(createPinia());
        mockServices.syncAllAccountBalances.mockReturnValue(success(syncResult));
        await useAccountsStore().syncAllAccountBalances({ refreshAccounts: false });
        expect(mockServices.getAllAccounts).toHaveBeenCalledTimes(1);
    });

    test('syncAllAccountBalances preserves invalid and transport errors', async () => {
        for (const [failure, expected] of [
            [invalidResponse(), { message: 'Unable to sync account balances' }],
            [Promise.reject({ response: { data: { message: 'detail' } } }), { error: { message: 'detail' } }],
            [Promise.reject({ processed: false }), { message: 'Unable to sync account balances' }],
            [Promise.reject({ processed: true, marker: 6 }), { processed: true, marker: 6 }]
        ] as const) {
            setActivePinia(createPinia());
            mockServices.syncAllAccountBalances.mockReturnValue(failure);
            await expect(useAccountsStore().syncAllAccountBalances({ refreshAccounts: false }))
                .rejects.toMatchObject(expected);
        }
    });

    test('expands parent ids, keeps empty parents and removes duplicates or unknown ids', async () => {
        const store = await loadTree();
        expect(store.expandAccountIds([
            'bank-parent', 'bank-cny', 'bank-parent', 'empty-parent', 'missing', 'cash'
        ])).toEqual(['bank-cny', 'bank-usd', 'bank-hidden', 'empty-parent', 'cash']);
    });
});
