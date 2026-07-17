import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { TransactionType } from '@/core/transaction.ts';
import { Transaction, type TransactionInfoResponse } from '@/models/transaction.ts';

const mockSettingsStore = { appSettings: { timeZone: 'UTC', autoSaveTransactionDraft: 'disabled' } };
const mockUserStore = { currentUserDefaultAccountId: '', currentUserDefaultCurrency: 'CNY' };
const mockAccountsStore = {
    allAccountsMap: {} as Record<string, unknown>,
    accountListStateInvalid: true,
    expandAccountIds: jest.fn<(ids: string[]) => string[]>(),
    getAccountStatementDate: jest.fn<() => number | null>(),
    updateAccountListInvalidState: jest.fn(),
    loadAllAccounts: jest.fn<() => Promise<unknown>>()
};
const mockCategoriesStore = {
    allTransactionCategoriesMap: {},
    allTransactionCategories: {}
};
const mockOverviewStore = {
    transactionOverviewStateInvalid: true,
    updateTransactionOverviewInvalidState: jest.fn()
};
const mockStatisticsStore = {
    transactionStatisticsStateInvalid: true,
    updateTransactionStatisticsInvalidState: jest.fn()
};
const mockServices = {
    getTransactions: jest.fn<(request: unknown) => Promise<unknown>>(),
    getAllTransactionsByMonth: jest.fn<(request: unknown) => Promise<unknown>>(),
    getReconciliationStatements: jest.fn<(request: unknown) => Promise<unknown>>(),
    getTransaction: jest.fn<(request: unknown) => Promise<unknown>>(),
    addTransaction: jest.fn<(request: unknown) => Promise<unknown>>(),
    modifyTransaction: jest.fn<(request: unknown) => Promise<unknown>>(),
    addTransactions: jest.fn<(request: unknown) => Promise<unknown>>(),
    moveAllTransactionsBetweenAccounts: jest.fn<(request: unknown) => Promise<unknown>>(),
    deleteTransaction: jest.fn<(request: unknown) => Promise<unknown>>(),
    recognizeReceiptImage: jest.fn<(request: unknown) => Promise<unknown>>(),
    cancelRequest: jest.fn(),
    parseImportTransaction: jest.fn<(request: unknown) => Promise<unknown>>(),
    uploadTransactionPicture: jest.fn<(request: unknown) => Promise<unknown>>(),
    removeUnusedTransactionPicture: jest.fn<(request: unknown) => Promise<unknown>>(),
    getTransactionPictureUrlWithToken: jest.fn()
};

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoriesStore }));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockOverviewStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/stores/exchangeRates.ts', () => ({
    useExchangeRatesStore: () => ({ getExchangedAmount: (amount: number) => amount })
}));
jest.mock('@/lib/userstate.ts', () => ({
    getUserTransactionDraft: () => null,
    updateUserTransactionDraft: jest.fn(),
    clearUserTransactionDraft: jest.fn()
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getTransactions: (request: unknown) => mockServices.getTransactions(request),
        getAllTransactionsByMonth: (request: unknown) => mockServices.getAllTransactionsByMonth(request),
        getReconciliationStatements: (request: unknown) => mockServices.getReconciliationStatements(request),
        getTransaction: (request: unknown) => mockServices.getTransaction(request),
        addTransaction: (request: unknown) => mockServices.addTransaction(request),
        modifyTransaction: (request: unknown) => mockServices.modifyTransaction(request),
        addTransactions: (request: unknown) => mockServices.addTransactions(request),
        moveAllTransactionsBetweenAccounts: (request: unknown) => mockServices.moveAllTransactionsBetweenAccounts(request),
        deleteTransaction: (request: unknown) => mockServices.deleteTransaction(request),
        recognizeReceiptImage: (request: unknown) => mockServices.recognizeReceiptImage(request),
        cancelRequest: (id: string) => mockServices.cancelRequest(id),
        parseImportTransaction: (request: unknown) => mockServices.parseImportTransaction(request),
        uploadTransactionPicture: (request: unknown) => mockServices.uploadTransactionPicture(request),
        removeUnusedTransactionPicture: (request: unknown) => mockServices.removeUnusedTransactionPicture(request),
        getTransactionPictureUrlWithToken: (...args: unknown[]) => mockServices.getTransactionPictureUrlWithToken(...args)
    }
}));

import { useTransactionsStore } from '@/stores/transaction.ts';

function success<T>(result: T): Promise<{ data: { success: true; result: T } }> {
    return Promise.resolve({ data: { success: true, result } });
}

function malformed(): Promise<{ data: { success: false; result: null } }> {
    return Promise.resolve({ data: { success: false, result: null } });
}

function transactionResponse(overrides: Partial<TransactionInfoResponse> = {}): TransactionInfoResponse {
    return {
        id: 'tx-1',
        timeSequenceId: '1000',
        type: TransactionType.Expense,
        categoryId: 'food',
        time: Date.UTC(2026, 6, 16, 12) / 1000,
        utcOffset: 0,
        sourceAccountId: 'cash',
        destinationAccountId: '0',
        sourceAmountCents: 1250,
        destinationAmountCents: 0,
        hideAmount: false,
        tagIds: [],
        comment: '',
        editable: true,
        gregorianCalendarYearDashMonthDashDay: '2026-07-16',
        gregorianCalendarDayOfMonth: 16,
        displayDayOfWeek: 4,
        ...overrides
    };
}

function draft(): Transaction {
    const transaction = Transaction.createNewTransaction(
        TransactionType.Expense, Date.UTC(2026, 6, 16, 12) / 1000, 'UTC', 0
    );
    transaction.setCategoryId('food');
    transaction.sourceAccountId = 'cash';
    transaction.sourceAmountCents = 1250;
    return transaction;
}

describe('transaction store state and failure boundaries', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        jest.clearAllMocks();
        mockAccountsStore.accountListStateInvalid = true;
        mockAccountsStore.allAccountsMap = {};
        mockAccountsStore.expandAccountIds.mockImplementation(ids => ids);
        mockAccountsStore.getAccountStatementDate.mockReturnValue(null);
        mockAccountsStore.loadAllAccounts.mockResolvedValue([]);
        mockOverviewStore.transactionOverviewStateInvalid = true;
        mockStatisticsStore.transactionStatisticsStateInvalid = true;
    });

    test('keeps already-invalid dependent stores unchanged after batch, move and delete success', async () => {
        const store = useTransactionsStore();
        mockServices.addTransactions.mockReturnValueOnce(success({ items: [transactionResponse()] }));
        await expect(store.saveTransactions({ transactions: [draft()], clientSessionId: 'client' }))
            .resolves.toHaveLength(1);
        expect(mockOverviewStore.updateTransactionOverviewInvalidState).not.toHaveBeenCalled();
        expect(mockStatisticsStore.updateTransactionStatisticsInvalidState).not.toHaveBeenCalled();

        mockServices.moveAllTransactionsBetweenAccounts.mockReturnValueOnce(success(true));
        await expect(store.moveAllTransactionsBetweenAccounts({
            fromAccountId: 'cash', toAccountId: 'bank', password: 'secret'
        })).resolves.toBe(true);

        mockServices.deleteTransaction.mockReturnValueOnce(success(true));
        await expect(store.deleteTransaction({ transaction: transactionResponse(), defaultCurrency: 'CNY' }))
            .resolves.toBe(true);
        expect(mockOverviewStore.updateTransactionOverviewInvalidState).not.toHaveBeenCalled();
        expect(mockStatisticsStore.updateTransactionStatisticsInvalidState).not.toHaveBeenCalled();
    });

    test('resets malformed current requests without re-invalidating an already-invalid list', async () => {
        const store = useTransactionsStore();
        mockServices.getTransactions.mockReturnValueOnce(malformed());
        await expect(store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ message: 'Unable to retrieve transaction list' });

        mockServices.getTransactions.mockRejectedValueOnce({ processed: true });
        await expect(store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ processed: true });
        mockServices.getTransactions.mockRejectedValueOnce({ processed: false });
        await expect(store.loadTransactions({ reload: false, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ message: 'Unable to retrieve transaction list' });

        mockServices.getAllTransactionsByMonth.mockReturnValueOnce(malformed());
        await expect(store.loadMonthlyAllTransactions({ year: 2026, month: 7, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ message: 'Unable to retrieve transaction list' });
        mockServices.getAllTransactionsByMonth.mockRejectedValueOnce({ processed: true });
        await expect(store.loadMonthlyAllTransactions({ year: 2026, month: 7, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ processed: true });
    });

    test('handles empty expanded filters and stale monthly completions without corrupting the latest page', async () => {
        const store = useTransactionsStore();
        store.transactionsFilter.accountIds = ',';
        mockServices.getTransactions.mockReturnValueOnce(malformed());
        await expect(store.loadTransactions({ reload: false, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ message: 'Unable to retrieve transaction list' });

        mockServices.getAllTransactionsByMonth.mockReturnValueOnce(malformed());
        await expect(store.loadMonthlyAllTransactions({ year: 2026, month: 7, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ message: 'Unable to retrieve transaction list' });

        let rejectStale!: (reason: unknown) => void;
        mockServices.getAllTransactionsByMonth
            .mockReturnValueOnce(new Promise((_resolve, reject) => { rejectStale = reject; }))
            .mockReturnValueOnce(success({ items: [], totalCount: 0 }));
        const staleFailure = store.loadMonthlyAllTransactions({
            year: 2026, month: 6, autoExpand: false, defaultCurrency: 'CNY'
        });
        const currentSuccess = store.loadMonthlyAllTransactions({
            year: 2026, month: 7, autoExpand: false, defaultCurrency: 'CNY'
        });
        await expect(currentSuccess).resolves.toEqual({ items: [], totalCount: 0 });
        rejectStale({ processed: false });
        await expect(staleFailure).resolves.toEqual({ items: [], totalCount: 0 });

        let resolveStale!: (value: unknown) => void;
        mockServices.getAllTransactionsByMonth
            .mockReturnValueOnce(new Promise(resolve => { resolveStale = resolve; }))
            .mockReturnValueOnce(success({ items: [], totalCount: 0 }));
        const staleMalformed = store.loadMonthlyAllTransactions({
            year: 2026, month: 5, autoExpand: false, defaultCurrency: 'CNY'
        });
        await store.loadMonthlyAllTransactions({ year: 2026, month: 7, autoExpand: false, defaultCurrency: 'CNY' });
        resolveStale({ data: { success: false, result: null } });
        await expect(staleMalformed).rejects.toEqual({ message: 'Unable to retrieve transaction list' });
    });

    test('appends to a completed month without recalculating its already-complete prior totals', async () => {
        const store = useTransactionsStore();
        const first = transactionResponse({ id: 'first', timeSequenceId: '2000' });
        const second = transactionResponse({ id: 'second', timeSequenceId: '1000' });
        mockServices.getTransactions.mockReturnValueOnce(success({
            items: [first], totalCount: 2, nextTimeSequenceId: 1000
        }));
        await store.loadTransactions({ reload: true, autoExpand: true, defaultCurrency: 'CNY' });
        store.transactions[0]!.totalAmountCents.incompleteExpense = false;
        store.transactions[0]!.totalAmountCents.incompleteIncome = false;
        mockServices.getTransactions.mockReturnValueOnce(success({ items: [second], totalCount: 2 }));

        await expect(store.loadTransactions({ reload: false, autoExpand: true, defaultCurrency: 'CNY' }))
            .resolves.toEqual(expect.objectContaining({ totalCount: 2 }));
        expect(store.transactions[0]!.items.map(item => item.id)).toEqual(['first', 'second']);
    });

    test('does not mark already-invalid overview and statistics state again after one save', async () => {
        const store = useTransactionsStore();
        mockServices.addTransaction.mockReturnValueOnce(success(transactionResponse()));

        await expect(store.saveTransaction({
            transaction: draft(), defaultCurrency: 'CNY', isEdit: false, clientSessionId: 'client'
        })).resolves.toEqual(expect.objectContaining({ id: 'tx-1' }));
        expect(mockOverviewStore.updateTransactionOverviewInvalidState).not.toHaveBeenCalled();
        expect(mockStatisticsStore.updateTransactionStatisticsInvalidState).not.toHaveBeenCalled();
    });

    test('treats a sparse month containing only an empty slot as having no transaction', () => {
        const store = useTransactionsStore();
        store.transactions.push({
            year: 2026,
            month: 7,
            yearDashMonth: '2026-07',
            opened: true,
            items: [null as never],
            totalAmountCents: {
                expenseCents: 0, incompleteExpense: false, incomeCents: 0, incompleteIncome: false
            },
            dailyTotalAmountsCents: {}
        });

        expect(store.noTransaction).toBe(true);
    });

    test('keeps reconciliation invalid-state transitions idempotent on malformed and successful responses', async () => {
        const store = useTransactionsStore();
        mockServices.getReconciliationStatements.mockReturnValueOnce(malformed());
        await expect(store.getReconciliationStatements({ accountId: 'cash', startTime: 1, endTime: 2 }))
            .rejects.toEqual({ message: 'Unable to retrieve reconciliation statements' });

        store.updateTransactionReconciliationStatementInvalidState(false);
        mockServices.getReconciliationStatements.mockReturnValueOnce(success({ openingBalanceCents: 0 }));
        await expect(store.getReconciliationStatements({ accountId: 'cash', startTime: 1, endTime: 2 }))
            .resolves.toEqual({ openingBalanceCents: 0 });
        expect(store.transactionReconciliationStatementStateInvalid).toBe(false);
    });

    test('normalizes sparse OCR success and bounded fallback, cancellation and unknown failures', async () => {
        const store = useTransactionsStore();
        const imageFile = new File(['image'], 'receipt.png', { type: 'image/png' });
        mockServices.recognizeReceiptImage.mockReturnValueOnce(success({
            amount: 'bad', trade_time: 1, description: null, payment_platform: false,
            confidence: 'bad', draft: null
        }));
        await expect(store.recognizeReceiptImage({ imageFile })).resolves.toEqual({
            amount: null,
            tradeTime: null,
            description: null,
            paymentPlatform: null,
            provenance: { provider: 'unknown', model: undefined, requestId: '' },
            confidence: null
        });

        mockServices.recognizeReceiptImage.mockRejectedValueOnce({ canceled: true });
        await expect(store.recognizeReceiptImage({ imageFile })).rejects.toEqual(expect.objectContaining({
            errorCode: 'cancelled', status: 499
        }));
        mockServices.recognizeReceiptImage.mockRejectedValueOnce({ response: {} });
        await expect(store.recognizeReceiptImage({ imageFile })).rejects.toEqual(expect.objectContaining({
            errorCode: 'unknown', message: 'Unable to recognize image', status: 0
        }));
    });

    test('treats missing picture input and missing month list as safe no-ops', () => {
        const store = useTransactionsStore();
        expect(store.getTransactionPictureUrl()).toBeUndefined();
        expect(() => store.collapseMonthInTransactionList({ monthList: null as never, collapse: true })).not.toThrow();
    });
});
