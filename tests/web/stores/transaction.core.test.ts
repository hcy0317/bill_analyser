import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { AccountCategory, AccountType } from '@/core/account.ts';
import { DateRange } from '@/core/datetime.ts';
import { TransactionTagFilterType, TransactionType } from '@/core/transaction.ts';
import type { AccountInfoResponse } from '@/models/account.ts';
import {
    Transaction,
    type TransactionInfoPageWrapperResponse,
    type TransactionInfoResponse
} from '@/models/transaction.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useOverviewStore } from '@/stores/overview.ts';
import { useStatisticsStore } from '@/stores/statistics.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';

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

type ApiResponse<T> = {
    data: {
        success: boolean;
        result: T;
    };
};

function createDeferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (reason?: unknown) => void;
} {
    let resolve!: (value: T) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });

    return { promise, resolve, reject };
}

const mockGetAllAccounts = jest.fn<() => Promise<ApiResponse<AccountInfoResponse[]>>>();
const mockGetTransactions = jest.fn<(req: unknown) => Promise<ApiResponse<TransactionInfoPageWrapperResponse>>>();
const mockGetAllTransactionsByMonth = jest.fn<(req: unknown) => Promise<ApiResponse<TransactionInfoPageWrapperResponse>>>();
const mockAddTransactions = jest.fn<(req: unknown) => Promise<ApiResponse<{
    items: TransactionInfoResponse[];
    ids: string[];
    createdCount: number;
}>>>();
const mockMoveAllTransactionsBetweenAccounts = jest.fn<(req: unknown) => Promise<ApiResponse<boolean>>>();

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getAllAccounts: () => mockGetAllAccounts(),
        getTransactions: (req: unknown) => mockGetTransactions(req),
        getAllTransactionsByMonth: (req: unknown) => mockGetAllTransactionsByMonth(req),
        addTransactions: (req: unknown) => mockAddTransactions(req),
        moveAllTransactionsBetweenAccounts: (req: unknown) => mockMoveAllTransactionsBetweenAccounts(req)
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

jest.mock('@/locales/helpers.ts', () => ({
    __esModule: true,
    useI18n: () => ({
        t: (key: string) => key,
        tt: (key: string) => key,
        ti: (key: string) => key,
        te: () => false,
        getLocale: () => 'en',
        setLocale: () => undefined,
        getCurrentLanguageInfo: () => ({ name: 'en', alternativeLanguageTag: 'en' }),
        getCurrentLanguageDisplayName: () => 'English',
        getCurrentLanguageTag: () => 'en'
    })
}));

function accountResponse(overrides: Partial<AccountInfoResponse>): AccountInfoResponse {
    return {
        id: 'wallet',
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

function accountListResponse(): AccountInfoResponse[] {
    return [
        accountResponse({ id: 'wallet', name: '现金' }),
        accountResponse({
            id: 'bank-parent',
            name: '银行',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: '---',
            balanceCents: 0,
            subAccounts: [
                accountResponse({
                    id: 'bank-card-a',
                    name: '银行卡 A',
                    parentId: 'bank-parent',
                    category: AccountCategory.CheckingAccount.type
                }),
                accountResponse({
                    id: 'bank-card-b',
                    name: '银行卡 B',
                    parentId: 'bank-parent',
                    category: AccountCategory.CheckingAccount.type
                })
            ]
        })
    ];
}

function transactionResponse(overrides: Partial<TransactionInfoResponse> = {}): TransactionInfoResponse {
    return {
        id: 'bill-1',
        timeSequenceId: '1000',
        type: TransactionType.Expense,
        categoryId: 'cat-food',
        time: 1777636800,
        utcOffset: 480,
        sourceAccountId: 'wallet',
        destinationAccountId: '0',
        sourceAmountCents: 1234,
        destinationAmountCents: 0,
        hideAmount: false,
        tagIds: ['tag-breakfast'],
        comment: '早餐',
        editable: true,
        ...overrides
    };
}

function expenseTransaction(overrides: Partial<Pick<Transaction, 'sourceAccountId' | 'sourceAmountCents' | 'comment'>> = {}): Transaction {
    const transaction = Transaction.createNewTransaction(TransactionType.Expense, 1777636800, 'Asia/Shanghai', 480);
    transaction.setCategoryId('cat-food');
    transaction.sourceAccountId = overrides.sourceAccountId ?? 'wallet';
    transaction.sourceAmountCents = overrides.sourceAmountCents ?? 1234;
    transaction.comment = overrides.comment ?? '早餐';
    transaction.tagIds = ['tag-breakfast'];
    return transaction;
}

describe('transaction store service boundary', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        memStore.clear();
        mockGetAllAccounts.mockReset();
        mockGetTransactions.mockReset();
        mockGetAllTransactionsByMonth.mockReset();
        mockAddTransactions.mockReset();
        mockMoveAllTransactionsBetweenAccounts.mockReset();
        mockGetAllAccounts.mockResolvedValue({
            data: {
                success: true,
                result: accountListResponse()
            }
        });
    });

    test('loadTransactions expands selected parent-account filters before calling services', async () => {
        const accountsStore = useAccountsStore();
        await accountsStore.loadAllAccounts({ force: true });

        mockGetTransactions.mockResolvedValue({
            data: {
                success: true,
                result: {
                    items: [transactionResponse()],
                    totalCount: 1,
                    nextTimeSequenceId: 999
                }
            }
        });

        const transactionsStore = useTransactionsStore();
        transactionsStore.initTransactionListFilter({
            accountIds: 'bank-parent,wallet',
            keyword: '早餐'
        });

        const page = await transactionsStore.loadTransactions({
            reload: true,
            count: 25,
            page: 2,
            withCount: true,
            autoExpand: false,
            defaultCurrency: 'CNY'
        });

        expect(mockGetTransactions).toHaveBeenCalledWith(expect.objectContaining({
            maxTime: 0,
            minTime: 0,
            count: 25,
            page: 2,
            withCount: true,
            accountIds: 'bank-card-a,bank-card-b,wallet',
            keyword: '早餐'
        }));
        expect(page.totalCount).toBe(1);
        expect(transactionsStore.transactionListStateInvalid).toBe(false);
    });

    test('loadTransactions keeps the newest response when an older request resolves last', async () => {
        const olderRequest = createDeferred<ApiResponse<TransactionInfoPageWrapperResponse>>();
        const newerRequest = createDeferred<ApiResponse<TransactionInfoPageWrapperResponse>>();
        mockGetTransactions
            .mockReturnValueOnce(olderRequest.promise)
            .mockReturnValueOnce(newerRequest.promise);

        const store = useTransactionsStore();
        store.initTransactionListFilter({ keyword: 'older-filter' });
        const olderLoad = store.loadTransactions({
            reload: true,
            autoExpand: false,
            defaultCurrency: 'CNY'
        });

        store.updateTransactionListFilter({ keyword: 'newer-filter' });
        const newerLoad = store.loadTransactions({
            reload: true,
            autoExpand: false,
            defaultCurrency: 'CNY'
        });

        newerRequest.resolve({
            data: {
                success: true,
                result: {
                    items: [transactionResponse({ id: 'newer-bill', comment: 'newer-filter' })],
                    totalCount: 1,
                    nextTimeSequenceId: 200
                }
            }
        });
        await newerLoad;

        olderRequest.resolve({
            data: {
                success: true,
                result: {
                    items: [transactionResponse({ id: 'older-bill', comment: 'older-filter' })],
                    totalCount: 1,
                    nextTimeSequenceId: 100
                }
            }
        });
        const staleResult = await olderLoad;

        expect(store.transactions.flatMap(month => month.items).map(transaction => transaction.id))
            .toStrictEqual(['newer-bill']);
        expect(store.transactionsNextTimeId).toBe(200);
        expect(store.transactionListStateInvalid).toBe(false);
        expect(staleResult.items.map(transaction => transaction.id)).toStrictEqual(['newer-bill']);
    });

    test('loadTransactions does not clear newer state when an older request fails last', async () => {
        const olderRequest = createDeferred<ApiResponse<TransactionInfoPageWrapperResponse>>();
        const newerRequest = createDeferred<ApiResponse<TransactionInfoPageWrapperResponse>>();
        mockGetTransactions
            .mockReturnValueOnce(olderRequest.promise)
            .mockReturnValueOnce(newerRequest.promise);

        const store = useTransactionsStore();
        const olderLoad = store.loadTransactions({
            reload: true,
            autoExpand: false,
            defaultCurrency: 'CNY'
        });
        const newerLoad = store.loadTransactions({
            reload: true,
            autoExpand: false,
            defaultCurrency: 'CNY'
        });

        newerRequest.resolve({
            data: {
                success: true,
                result: {
                    items: [transactionResponse({ id: 'newer-bill' })],
                    totalCount: 1,
                    nextTimeSequenceId: 200
                }
            }
        });
        await newerLoad;

        olderRequest.reject({ processed: false });
        await expect(olderLoad).resolves.toMatchObject({
            items: [expect.objectContaining({ id: 'newer-bill' })],
            totalCount: 1
        });

        expect(store.transactions.flatMap(month => month.items).map(transaction => transaction.id))
            .toStrictEqual(['newer-bill']);
        expect(store.transactionListStateInvalid).toBe(false);
    });

    test('loadMonthlyAllTransactions keeps the newest monthly response', async () => {
        const olderRequest = createDeferred<ApiResponse<TransactionInfoPageWrapperResponse>>();
        const newerRequest = createDeferred<ApiResponse<TransactionInfoPageWrapperResponse>>();
        mockGetAllTransactionsByMonth
            .mockReturnValueOnce(olderRequest.promise)
            .mockReturnValueOnce(newerRequest.promise);

        const store = useTransactionsStore();
        const olderLoad = store.loadMonthlyAllTransactions({
            year: 2026,
            month: 1,
            autoExpand: false,
            defaultCurrency: 'CNY'
        });
        const newerLoad = store.loadMonthlyAllTransactions({
            year: 2026,
            month: 2,
            autoExpand: false,
            defaultCurrency: 'CNY'
        });

        newerRequest.resolve({
            data: {
                success: true,
                result: {
                    items: [transactionResponse({ id: 'newer-month-bill' })],
                    totalCount: 1,
                    nextTimeSequenceId: 0
                }
            }
        });
        await newerLoad;

        olderRequest.resolve({
            data: {
                success: true,
                result: {
                    items: [transactionResponse({ id: 'older-month-bill' })],
                    totalCount: 1,
                    nextTimeSequenceId: 0
                }
            }
        });
        const staleResult = await olderLoad;

        expect(store.transactions.flatMap(month => month.items).map(transaction => transaction.id))
            .toStrictEqual(['newer-month-bill']);
        expect(staleResult.items.map(transaction => transaction.id)).toStrictEqual(['newer-month-bill']);
        expect(store.transactionListStateInvalid).toBe(false);
    });

    test('filter URL params and export requests keep the transaction query contract', () => {
        const transactionsStore = useTransactionsStore();

        transactionsStore.initTransactionListFilter({
            dateType: DateRange.Custom.type,
            minTime: 1_767_225_600,
            maxTime: 1_770_422_399,
            type: TransactionType.Investment,
            categoryIds: 'fund,bond',
            accountIds: 'brokerage,bank',
            tagIds: 'tag-plan',
            tagFilterType: TransactionTagFilterType.NotHasAny.type,
            amountFilterCents: 'bt:10000:20000',
            keyword: '沪深300 ETF'
        });

        expect(transactionsStore.getTransactionListPageParams(2)).toBe(
            'pageType=2'
            + '&type=5'
            + '&accountIds=brokerage,bank'
            + '&categoryIds=fund,bond'
            + '&tagIds=tag-plan'
            + '&tagFilterType=2'
            + '&dateType=255'
            + '&maxTime=1770422399'
            + '&minTime=1767225600'
            + '&amountFilterCents=bt%3A10000%3A20000'
            + '&keyword=%E6%B2%AA%E6%B7%B1300%20ETF'
        );

        expect(transactionsStore.getExportTransactionDataRequestByTransactionFilter()).toStrictEqual({
            maxTime: 1_770_422_399,
            minTime: 1_767_225_600,
            type: TransactionType.Investment,
            categoryIds: 'fund,bond',
            accountIds: 'brokerage,bank',
            tagIds: 'tag-plan',
            tagFilterType: TransactionTagFilterType.NotHasAny.type,
            amountFilterCents: 'bt:10000:20000',
            keyword: '沪深300 ETF'
        });
    });

    test('saveTransactions filters unsupported drafts and invalidates high-fanout account/overview/statistics state', async () => {
        const transactionsStore = useTransactionsStore();
        const accountsStore = useAccountsStore();
        const overviewStore = useOverviewStore();
        const statisticsStore = useStatisticsStore();
        transactionsStore.updateTransactionListInvalidState(false);
        transactionsStore.updateTransactionReconciliationStatementInvalidState(false);
        accountsStore.updateAccountListInvalidState(false);
        overviewStore.updateTransactionOverviewInvalidState(false);
        statisticsStore.updateTransactionStatisticsInvalidState(false);
        mockAddTransactions.mockResolvedValue({
            data: {
                success: true,
                result: {
                    items: [transactionResponse({ id: 'created-1' })],
                    ids: ['created-1'],
                    createdCount: 1
                }
            }
        });

        const validExpense = expenseTransaction();
        const unsupportedModifyBalance = Transaction.createNewTransaction(
            TransactionType.ModifyBalance,
            1777636800,
            'Asia/Shanghai',
            480
        );

        const created = await transactionsStore.saveTransactions({
            transactions: [validExpense, unsupportedModifyBalance],
            clientSessionId: 'batch-session'
        });

        expect(mockAddTransactions).toHaveBeenCalledWith(expect.objectContaining({
            clientSessionId: 'batch-session',
            transactions: [
                expect.objectContaining({
                    type: TransactionType.Expense,
                    sourceAccountId: 'wallet',
                    sourceAmountCents: 1234,
                    clientSessionId: 'batch-session'
                })
            ]
        }));
        const addBatchRequest = mockAddTransactions.mock.calls[0]?.[0] as
            | { transactions: unknown[] }
            | undefined;
        expect(addBatchRequest?.transactions).toHaveLength(1);
        expect(created.map(transaction => transaction.id)).toStrictEqual(['created-1']);
        expect(transactionsStore.transactionListStateInvalid).toBe(true);
        expect(transactionsStore.transactionReconciliationStatementStateInvalid).toBe(true);
        expect(accountsStore.accountListStateInvalid).toBe(false);
        expect(mockGetAllAccounts).toHaveBeenCalledTimes(1);
        expect(overviewStore.transactionOverviewStateInvalid).toBe(true);
        expect(statisticsStore.transactionStatisticsStateInvalid).toBe(true);
    });

    test('moveAllTransactionsBetweenAccounts delegates and marks dependent views stale without forcing an account reload', async () => {
        const transactionsStore = useTransactionsStore();
        const accountsStore = useAccountsStore();
        const overviewStore = useOverviewStore();
        const statisticsStore = useStatisticsStore();
        transactionsStore.updateTransactionListInvalidState(false);
        transactionsStore.updateTransactionReconciliationStatementInvalidState(false);
        accountsStore.updateAccountListInvalidState(false);
        overviewStore.updateTransactionOverviewInvalidState(false);
        statisticsStore.updateTransactionStatisticsInvalidState(false);
        mockMoveAllTransactionsBetweenAccounts.mockResolvedValue({
            data: {
                success: true,
                result: true
            }
        });

        await expect(transactionsStore.moveAllTransactionsBetweenAccounts({
            fromAccountId: 'wallet',
            toAccountId: 'bank-card-a',
            password: 'secret'
        })).resolves.toBe(true);

        expect(mockMoveAllTransactionsBetweenAccounts).toHaveBeenCalledWith({
            fromAccountId: 'wallet',
            toAccountId: 'bank-card-a',
            password: 'secret'
        });
        expect(transactionsStore.transactionListStateInvalid).toBe(true);
        expect(transactionsStore.transactionReconciliationStatementStateInvalid).toBe(true);
        expect(accountsStore.accountListStateInvalid).toBe(true);
        expect(mockGetAllAccounts).not.toHaveBeenCalled();
        expect(overviewStore.transactionOverviewStateInvalid).toBe(true);
        expect(statisticsStore.transactionStatisticsStateInvalid).toBe(true);
    });
});
