import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { AccountCategory, AccountType } from '@/core/account.ts';
import type { AccountInfoResponse, SyncBalancesResponse } from '@/models/account.ts';
import { useAccountsStore } from '@/stores/account.ts';

const memStore = new Map<string, string>();
(globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (key: string) => memStore.get(key) ?? null,
    setItem: (key: string, value: string) => { memStore.set(key, value); },
    removeItem: (key: string) => { memStore.delete(key); },
    clear: () => { memStore.clear(); },
    key: (index: number) => Array.from(memStore.keys())[index] ?? null,
    get length() { return memStore.size; }
} as unknown as Storage;
(globalThis as unknown as { window: { location: { pathname: string; origin: string } } }).window = {
    location: { pathname: '/', origin: 'http://localhost' }
};

type AccountsApiResponse = {
    data: {
        success: boolean;
        result: AccountInfoResponse[];
    };
};

type SyncBalancesApiResponse = {
    data: {
        success: boolean;
        result: SyncBalancesResponse;
    };
};

type Deferred<T> = {
    promise: Promise<T>;
    resolve: (value: T | PromiseLike<T>) => void;
    reject: (reason?: unknown) => void;
};

const mockGetAllAccounts = jest.fn<() => Promise<AccountsApiResponse>>();
const mockSyncAllAccountBalances = jest.fn<() => Promise<SyncBalancesApiResponse>>();

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getAllAccounts: () => mockGetAllAccounts(),
        syncAllAccountBalances: () => mockSyncAllAccountBalances()
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

function createDeferred<T>(): Deferred<T> {
    let resolve!: Deferred<T>['resolve'];
    let reject!: Deferred<T>['reject'];
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });

    return { promise, resolve, reject };
}

function accountResponse(overrides: Partial<AccountInfoResponse>): AccountInfoResponse {
    return {
        id: 'account-1',
        name: '现金',
        parentId: '0',
        category: AccountCategory.Cash.type,
        type: AccountType.SingleAccount.type,
        icon: '1',
        color: '#ffffff',
        currency: 'CNY',
        balanceCents: 1234,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function buildAccountResponse(): AccountInfoResponse[] {
    return [
        accountResponse({
            id: 'wallet',
            name: '现金',
            category: AccountCategory.Cash.type,
            displayOrder: 1
        }),
        accountResponse({
            id: 'bank-parent',
            name: '银行',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: '---',
            balanceCents: 0,
            displayOrder: 2,
            subAccounts: [
                accountResponse({
                    id: 'bank-card-a',
                    name: '银行卡 A',
                    parentId: 'bank-parent',
                    category: AccountCategory.CheckingAccount.type,
                    displayOrder: 1
                }),
                accountResponse({
                    id: 'bank-card-b',
                    name: '银行卡 B',
                    parentId: 'bank-parent',
                    category: AccountCategory.CheckingAccount.type,
                    displayOrder: 2,
                    hidden: true
                })
            ]
        })
    ];
}

function accountsApiResponse(accounts = buildAccountResponse()): AccountsApiResponse {
    return {
        data: {
            success: true,
            result: accounts
        }
    };
}

function syncBalancesApiResponse(): SyncBalancesApiResponse {
    return {
        data: {
            success: true,
            result: {
                total_accounts: 3,
                synced_accounts: 2,
                discrepancies: [{
                account_id: 7,
                name: '银行卡 A',
                oldBalanceCents: 1200,
                newBalanceCents: 1234,
                diffCents: 34
                }],
                errors: ['skipped archived account']
            }
        }
    };
}

describe('accounts store service boundary', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        memStore.clear();
        mockGetAllAccounts.mockReset();
        mockSyncAllAccountBalances.mockReset();
    });

    test('deduplicates in-flight account loads and maps parent/sub-account ids', async () => {
        const deferred = createDeferred<AccountsApiResponse>();
        mockGetAllAccounts.mockReturnValue(deferred.promise);

        const store = useAccountsStore();
        const firstLoad = store.loadAllAccounts({ force: false });
        const secondLoad = store.loadAllAccounts({ force: false });

        expect(mockGetAllAccounts).toHaveBeenCalledTimes(1);

        deferred.resolve(accountsApiResponse());
        const [firstResult, secondResult] = await Promise.all([firstLoad, secondLoad]);

        expect(firstResult).toStrictEqual(secondResult);
        expect(store.accountListStateInvalid).toBe(false);
        expect(store.allAccountsMap['wallet']?.name).toBe('现金');
        expect(store.allAccountsMap['bank-parent']?.name).toBe('银行');
        expect(store.allAccountsMap['bank-card-a']?.parentId).toBe('bank-parent');
        expect(store.allAccountsMap['bank-card-b']?.hidden).toBe(true);
        expect(store.expandAccountIds(['bank-parent', 'wallet'])).toStrictEqual([
            'bank-card-a',
            'bank-card-b',
            'wallet'
        ]);

        await store.loadAllAccounts({ force: false });
        expect(mockGetAllAccounts).toHaveBeenCalledTimes(1);
    });

    test('syncAllAccountBalances deduplicates requests and refreshes account cache by default', async () => {
        const syncDeferred = createDeferred<SyncBalancesApiResponse>();
        mockSyncAllAccountBalances.mockReturnValue(syncDeferred.promise);
        mockGetAllAccounts.mockResolvedValue(accountsApiResponse());

        const store = useAccountsStore();
        const firstSync = store.syncAllAccountBalances();
        const secondSync = store.syncAllAccountBalances();

        expect(mockSyncAllAccountBalances).toHaveBeenCalledTimes(1);

        syncDeferred.resolve(syncBalancesApiResponse());
        const [firstResult, secondResult] = await Promise.all([firstSync, secondSync]);

        expect(firstResult).toStrictEqual(secondResult);
        expect(firstResult).toStrictEqual({
            totalAccounts: 3,
            syncedAccounts: 2,
            discrepancies: [{
                accountId: 7,
                name: '银行卡 A',
                oldBalanceCents: 1200,
                newBalanceCents: 1234,
                diffCents: 34
            }],
            errors: ['skipped archived account']
        });
        expect(mockGetAllAccounts).toHaveBeenCalledTimes(1);
        expect(store.accountListStateInvalid).toBe(false);
        expect(store.allAccountsMap['bank-card-a']?.name).toBe('银行卡 A');
    });

    test('syncAllAccountBalances can keep the existing account cache when refresh is disabled', async () => {
        mockSyncAllAccountBalances.mockResolvedValue(syncBalancesApiResponse());

        const store = useAccountsStore();
        await store.syncAllAccountBalances({ refreshAccounts: false });

        expect(mockGetAllAccounts).not.toHaveBeenCalled();
        expect(store.accountListStateInvalid).toBe(true);
    });
});
