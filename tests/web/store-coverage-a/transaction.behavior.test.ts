import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { AccountCategory, AccountType } from '@/core/account.ts';
import { CategoryType } from '@/core/category.ts';
import { DateRange } from '@/core/datetime.ts';
import { TransactionTagFilterType, TransactionType } from '@/core/transaction.ts';
import { Account, type AccountInfoResponse } from '@/models/account.ts';
import { Transaction, type TransactionInfoResponse } from '@/models/transaction.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';

type LoosePromise = Promise<unknown>;

const mockSettingsStore = {
    appSettings: {
        timeZone: 'UTC',
        autoSaveTransactionDraft: 'disabled'
    }
};
const mockUserStore = {
    currentUserDefaultAccountId: '',
    currentUserDefaultCurrency: 'CNY'
};
const mockAccountsStore = {
    allAccountsMap: {} as Record<string, Account>,
    accountListStateInvalid: true,
    expandAccountIds: jest.fn<(ids: string[]) => string[]>(),
    getAccountStatementDate: jest.fn<(ids?: string) => number | null | undefined>(),
    updateAccountListInvalidState: jest.fn<(invalid: boolean) => void>(),
    loadAllAccounts: jest.fn<(request: { force: boolean }) => Promise<Account[]>>()
};
const mockCategoriesStore = {
    allTransactionCategoriesMap: {} as Record<string, TransactionCategory>,
    allTransactionCategories: {} as Record<number, TransactionCategory[]>
};
const mockOverviewStore = {
    transactionOverviewStateInvalid: false,
    updateTransactionOverviewInvalidState: jest.fn<(invalid: boolean) => void>()
};
const mockStatisticsStore = {
    transactionStatisticsStateInvalid: false,
    updateTransactionStatisticsInvalidState: jest.fn<(invalid: boolean) => void>()
};
const mockGetExchangedAmount = jest.fn<(amount: number, from: string, to: string) => number | null>();
const mockGetUserTransactionDraft = jest.fn<() => unknown>();
const mockUpdateUserTransactionDraft = jest.fn<(draft: unknown) => void>();
const mockClearUserTransactionDraft = jest.fn<() => void>();

const mockServices = {
    getTransactions: jest.fn<(request: unknown) => LoosePromise>(),
    getAllTransactionsByMonth: jest.fn<(request: unknown) => LoosePromise>(),
    getReconciliationStatements: jest.fn<(request: unknown) => LoosePromise>(),
    getTransaction: jest.fn<(request: unknown) => LoosePromise>(),
    addTransaction: jest.fn<(request: unknown) => LoosePromise>(),
    modifyTransaction: jest.fn<(request: unknown) => LoosePromise>(),
    addTransactions: jest.fn<(request: unknown) => LoosePromise>(),
    moveAllTransactionsBetweenAccounts: jest.fn<(request: unknown) => LoosePromise>(),
    deleteTransaction: jest.fn<(request: unknown) => LoosePromise>(),
    recognizeReceiptImage: jest.fn<(request: unknown) => LoosePromise>(),
    cancelRequest: jest.fn<(id: string) => void>(),
    parseImportTransaction: jest.fn<(request: unknown) => LoosePromise>(),
    uploadTransactionPicture: jest.fn<(request: unknown) => LoosePromise>(),
    removeUnusedTransactionPicture: jest.fn<(request: unknown) => LoosePromise>(),
    getTransactionPictureUrlWithToken: jest.fn<(url: string, disable?: boolean | string) => string>()
};

jest.mock('@/stores/setting.ts', () => ({ __esModule: true, useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ __esModule: true, useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ __esModule: true, useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    __esModule: true,
    useTransactionCategoriesStore: () => mockCategoriesStore
}));
jest.mock('@/stores/overview.ts', () => ({ __esModule: true, useOverviewStore: () => mockOverviewStore }));
jest.mock('@/stores/statistics.ts', () => ({ __esModule: true, useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/stores/exchangeRates.ts', () => ({
    __esModule: true,
    useExchangeRatesStore: () => ({ getExchangedAmount: mockGetExchangedAmount })
}));
jest.mock('@/lib/userstate.ts', () => ({
    __esModule: true,
    getUserTransactionDraft: () => mockGetUserTransactionDraft(),
    updateUserTransactionDraft: (draft: unknown) => mockUpdateUserTransactionDraft(draft),
    clearUserTransactionDraft: () => mockClearUserTransactionDraft()
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
        getTransactionPictureUrlWithToken: (url: string, disable?: boolean | string) =>
            mockServices.getTransactionPictureUrlWithToken(url, disable)
    }
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

function accountResponse(overrides: Partial<AccountInfoResponse> = {}): AccountInfoResponse {
    return {
        id: 'cash',
        name: 'Cash',
        parentId: '0',
        category: AccountCategory.Cash.type,
        type: AccountType.SingleAccount.type,
        icon: '1',
        color: '#fff',
        currency: 'CNY',
        balanceCents: 0,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function makeAccount(overrides: Partial<AccountInfoResponse> = {}): Account {
    return Account.of(accountResponse(overrides));
}

function makeCategory(type: CategoryType, id: string): TransactionCategory {
    const category = TransactionCategory.createNewCategory(type);
    category.id = id;
    category.name = id;
    return category;
}

function installCategories(): void {
    mockCategoriesStore.allTransactionCategoriesMap = {};
    mockCategoriesStore.allTransactionCategories = {};
    const definitions: Array<[CategoryType, string]> = [
        [CategoryType.Expense, 'expense-default'],
        [CategoryType.Income, 'income-default'],
        [CategoryType.Transfer, 'transfer-default'],
        [CategoryType.Investment, 'investment-default']
    ];
    for (const [type, id] of definitions) {
        const primary = makeCategory(type, `${id}-primary`);
        const secondary = makeCategory(type, id);
        secondary.parentId = primary.id;
        primary.subCategories = [secondary];
        mockCategoriesStore.allTransactionCategories[type] = [primary];
        mockCategoriesStore.allTransactionCategoriesMap[id] = secondary;
    }
}

function installAccounts(): void {
    const parent = makeAccount({
        id: 'bank-parent',
        category: AccountCategory.CheckingAccount.type,
        type: AccountType.MultiSubAccounts.type,
        currency: '---',
        subAccounts: [
            accountResponse({ id: 'bank-cny', parentId: 'bank-parent', currency: 'CNY' }),
            accountResponse({ id: 'bank-usd', parentId: 'bank-parent', currency: 'USD' })
        ]
    });
    const cash = makeAccount({ id: 'cash', currency: 'CNY' });
    const usd = makeAccount({ id: 'usd', currency: 'USD' });
    const destination = makeAccount({ id: 'destination', currency: 'CNY' });
    mockAccountsStore.allAccountsMap = { cash, usd, destination, 'bank-parent': parent };
    for (const child of parent.subAccounts!) mockAccountsStore.allAccountsMap[child.id] = child;
}

function transactionResponse(overrides: Partial<TransactionInfoResponse> = {}): TransactionInfoResponse {
    return {
        id: 'tx-1',
        timeSequenceId: '1000',
        type: TransactionType.Expense,
        categoryId: 'expense-default',
        time: Date.UTC(2026, 0, 15, 12) / 1000,
        utcOffset: 0,
        sourceAccountId: 'cash',
        destinationAccountId: '0',
        sourceAmountCents: 100,
        destinationAmountCents: 0,
        hideAmount: false,
        tagIds: [],
        comment: '',
        editable: true,
        gregorianCalendarYearDashMonthDashDay: '2026-01-15',
        gregorianCalendarDayOfMonth: 15,
        displayDayOfWeek: 5,
        ...overrides
    };
}

function makeTransaction(type: number = TransactionType.Expense, overrides: Partial<TransactionInfoResponse> = {}): Transaction {
    const tx = Transaction.createNewTransaction(type, overrides.time ?? Date.UTC(2026, 0, 15, 12) / 1000, 'UTC', 0);
    tx.id = overrides.id ?? '';
    tx.timeSequenceId = overrides.timeSequenceId ?? '';
    tx.setCategoryId(overrides.categoryId ?? (
        type === TransactionType.Income ? 'income-default'
            : type === TransactionType.Transfer ? 'transfer-default'
                : type === TransactionType.Investment ? 'investment-default' : 'expense-default'
    ));
    tx.sourceAccountId = overrides.sourceAccountId ?? 'cash';
    tx.destinationAccountId = overrides.destinationAccountId ?? '0';
    tx.sourceAmountCents = overrides.sourceAmountCents ?? 100;
    tx.destinationAmountCents = overrides.destinationAmountCents ?? 0;
    tx.hideAmount = overrides.hideAmount ?? false;
    tx.tagIds = overrides.tagIds ? [...overrides.tagIds] : [];
    tx.comment = overrides.comment ?? '';
    return tx;
}

function success<T>(result: T): Promise<{ data: { success: true; result: T } }> {
    return Promise.resolve({ data: { success: true, result } });
}

function invalidResponse(): Promise<{ data: { success: false; result: null } }> {
    return Promise.resolve({ data: { success: false, result: null } });
}

function page(items: TransactionInfoResponse[], nextTimeSequenceId?: number) {
    return success({ items, totalCount: items.length, nextTimeSequenceId });
}

async function expectThreeErrorChannels(
    service: { mockReturnValue(value: LoosePromise): unknown },
    invoke: () => Promise<unknown>,
    fallback: string
): Promise<void> {
    service.mockReturnValue(Promise.reject({ response: { data: { message: 'backend detail' } } }));
    await expect(invoke()).rejects.toEqual({ error: { message: 'backend detail' } });
    service.mockReturnValue(Promise.reject({ processed: false }));
    await expect(invoke()).rejects.toEqual({ message: fallback });
    const processed = { processed: true, marker: fallback };
    service.mockReturnValue(Promise.reject(processed));
    await expect(invoke()).rejects.toBe(processed);
}

describe('transaction store behavior coverage', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        mockSettingsStore.appSettings.timeZone = 'UTC';
        mockSettingsStore.appSettings.autoSaveTransactionDraft = 'disabled';
        mockUserStore.currentUserDefaultAccountId = '';
        mockUserStore.currentUserDefaultCurrency = 'CNY';
        installAccounts();
        installCategories();
        mockAccountsStore.accountListStateInvalid = true;
        mockAccountsStore.expandAccountIds.mockReset();
        mockAccountsStore.expandAccountIds.mockImplementation(ids => ids.flatMap(id =>
            id === 'bank-parent' ? ['bank-cny', 'bank-usd'] : [id]
        ));
        mockAccountsStore.getAccountStatementDate.mockReset();
        mockAccountsStore.getAccountStatementDate.mockReturnValue(18);
        mockAccountsStore.updateAccountListInvalidState.mockReset();
        mockAccountsStore.loadAllAccounts.mockReset();
        mockAccountsStore.loadAllAccounts.mockResolvedValue([]);
        mockOverviewStore.transactionOverviewStateInvalid = false;
        mockOverviewStore.updateTransactionOverviewInvalidState.mockReset();
        mockStatisticsStore.transactionStatisticsStateInvalid = false;
        mockStatisticsStore.updateTransactionStatisticsInvalidState.mockReset();
        mockGetExchangedAmount.mockReset();
        mockGetExchangedAmount.mockImplementation((amount, from, to) => {
            if (from === to) return amount;
            if (from === 'USD' && to === 'CNY') return amount * 7;
            if (from === 'CNY' && to === 'USD') return amount / 7;
            return null;
        });
        mockGetUserTransactionDraft.mockReset();
        mockGetUserTransactionDraft.mockReturnValue(null);
        mockUpdateUserTransactionDraft.mockReset();
        mockClearUserTransactionDraft.mockReset();
        for (const service of Object.values(mockServices)) service.mockReset();
        mockServices.getTransactionPictureUrlWithToken.mockImplementation((url, disable) =>
            `${url}?cache=${String(disable)}`
        );
    });

    test('initializes, updates, serializes and resets every transaction-list filter', () => {
        const store = useTransactionsStore();
        store.initTransactionListFilter({
            dateType: DateRange.Custom.type,
            maxTime: 200,
            minTime: 100,
            type: TransactionType.Investment,
            categoryIds: 'cat-a,cat-b',
            accountIds: 'cash,usd',
            tagIds: 'tag-a,tag-b',
            tagFilterType: TransactionTagFilterType.NotHasAny.type,
            amountFilterCents: 'bt:10:20',
            keyword: 'coffee & tea'
        });
        expect(store.allFilterCategoryIds).toEqual({ 'cat-a': true, 'cat-b': true });
        expect(store.allFilterAccountIds).toEqual({ cash: true, usd: true });
        expect(store.allFilterTagIds).toEqual({ 'tag-a': true, 'tag-b': true });
        expect([store.allFilterCategoryIdsCount, store.allFilterAccountIdsCount, store.allFilterTagIdsCount])
            .toEqual([2, 2, 2]);
        expect(store.getTransactionListPageParams(3)).toBe(
            'pageType=3&type=5&accountIds=cash,usd&categoryIds=cat-a,cat-b&tagIds=tag-a,tag-b'
            + '&tagFilterType=2&dateType=255&maxTime=200&minTime=100'
            + '&amountFilterCents=bt%3A10%3A20&keyword=coffee%20%26%20tea'
        );
        expect(store.getExportTransactionDataRequestByTransactionFilter()).toEqual({
            maxTime: 200,
            minTime: 100,
            type: TransactionType.Investment,
            categoryIds: 'cat-a,cat-b',
            accountIds: 'cash,usd',
            tagIds: 'tag-a,tag-b',
            tagFilterType: TransactionTagFilterType.NotHasAny.type,
            amountFilterCents: 'bt:10:20',
            keyword: 'coffee & tea'
        });

        expect(store.updateTransactionListFilter({})).toBe(false);
        expect(store.updateTransactionListFilter({
            dateType: DateRange.CurrentBillingCycle.type,
            maxTime: 300,
            minTime: 150,
            type: TransactionType.Income,
            categoryIds: 'income-default',
            accountIds: 'bank-parent',
            tagIds: 'tag-c',
            tagFilterType: TransactionTagFilterType.HasAny.type,
            amountFilterCents: 'gt:30',
            keyword: 'salary'
        })).toBe(true);
        expect(store.transactionsFilter.dateType).toBe(DateRange.CurrentBillingCycle.type);
        mockAccountsStore.getAccountStatementDate.mockReturnValueOnce(null);
        expect(store.updateTransactionListFilter({ accountIds: 'cash' })).toBe(true);
        expect(store.transactionsFilter.dateType).toBe(DateRange.Custom.type);

        store.resetTransactions();
        expect(store.transactionsFilter).toEqual({
            dateType: DateRange.All.type,
            maxTime: 0,
            minTime: 0,
            type: 0,
            categoryIds: '',
            accountIds: '',
            tagIds: '',
            tagFilterType: TransactionTagFilterType.Default.type,
            amountFilterCents: '',
            keyword: ''
        });
        expect(store.transactionListStateInvalid).toBe(true);
        expect(store.transactionReconciliationStatementStateInvalid).toBe(true);

        store.initTransactionListFilter({
            dateType: 'bad' as unknown as number,
            maxTime: 'bad' as unknown as number,
            minTime: 'bad' as unknown as number,
            type: 'bad' as unknown as number,
            categoryIds: 1 as unknown as string,
            accountIds: 1 as unknown as string,
            tagIds: 1 as unknown as string,
            tagFilterType: 'bad' as unknown as number,
            amountFilterCents: 1 as unknown as string,
            keyword: 1 as unknown as string
        });
        expect(store.getTransactionListPageParams(1)).toBe(`pageType=1&dateType=${DateRange.All.type}`);
    });

    test('detects meaningful transaction drafts across amount, account, category, tags and attachments', () => {
        const store = useTransactionsStore();
        expect(store.isTransactionDraftModified()).toBe(false);
        const unchanged = makeTransaction(TransactionType.Expense, { sourceAmountCents: 0, sourceAccountId: 'cash' });
        unchanged.setCategoryId('expense-default');
        expect(store.isTransactionDraftModified(unchanged, 0, 'expense-default', 'cash', '', 'cash')).toBe(false);

        expect(store.isTransactionDraftModified(makeTransaction(TransactionType.Expense, { sourceAmountCents: 1 }), 0)).toBe(true);
        expect(store.isTransactionDraftModified(makeTransaction(TransactionType.Transfer, {
            sourceAmountCents: 0,
            destinationAmountCents: 1
        }), 0)).toBe(true);
        expect(store.isTransactionDraftModified(makeTransaction(TransactionType.Expense, { sourceAmountCents: 0, sourceAccountId: 'usd' }), 0, '', '', '', 'cash')).toBe(true);
        expect(store.isTransactionDraftModified(makeTransaction(TransactionType.Transfer, {
            sourceAmountCents: 0, sourceAccountId: 'cash', destinationAccountId: 'destination'
        }), 0, '', 'cash')).toBe(true);

        for (const [type, categoryId] of [
            [TransactionType.Expense, 'expense-other'],
            [TransactionType.Income, 'income-other'],
            [TransactionType.Transfer, 'transfer-other']
        ] as const) {
            const tx = makeTransaction(type, { sourceAmountCents: 0, sourceAccountId: 'cash', destinationAccountId: '0' });
            tx.destinationAmountCents = 0;
            tx.setCategoryId(categoryId);
            expect(store.isTransactionDraftModified(tx, 0, '', 'cash', '', 'cash')).toBe(true);
        }

        const hidden = makeTransaction(TransactionType.Expense, { sourceAmountCents: 0, sourceAccountId: 'cash', hideAmount: true });
        expect(store.isTransactionDraftModified(hidden, 0, 'expense-default', 'cash', '', 'cash')).toBe(true);
        const tagged = makeTransaction(TransactionType.Expense, { sourceAmountCents: 0, sourceAccountId: 'cash', tagIds: ['a'] });
        expect(store.isTransactionDraftModified(tagged, 0, 'expense-default', 'cash', 'a,b', 'cash')).toBe(false);
        expect(store.isTransactionDraftModified(tagged, 0, 'expense-default', 'cash', 'b', 'cash')).toBe(true);
        const pictured = makeTransaction(TransactionType.Expense, { sourceAmountCents: 0, sourceAccountId: 'cash' });
        pictured.addPicture({ pictureId: 'p', originalUrl: '/p' });
        expect(store.isTransactionDraftModified(pictured, 0, 'expense-default', 'cash', '', 'cash')).toBe(true);
        const commented = makeTransaction(TransactionType.Expense, { sourceAmountCents: 0, sourceAccountId: 'cash', comment: ' note ' });
        expect(store.isTransactionDraftModified(commented, 0, 'expense-default', 'cash', '', 'cash')).toBe(true);
    });

    test('loads, saves and clears persisted transaction drafts according to settings', () => {
        const store = useTransactionsStore();
        mockSettingsStore.appSettings.autoSaveTransactionDraft = 'enabled';
        mockGetUserTransactionDraft.mockReturnValue({ type: TransactionType.Expense, sourceAmountCents: 7 });
        store.initTransactionDraft();
        expect(store.transactionDraft).toEqual({ type: TransactionType.Expense, sourceAmountCents: 7 });

        const modified = makeTransaction(TransactionType.Expense, { sourceAmountCents: 99 });
        store.saveTransactionDraft(modified, 0);
        expect(store.transactionDraft).toEqual(expect.objectContaining({ sourceAmountCents: 99 }));
        expect(mockUpdateUserTransactionDraft).toHaveBeenLastCalledWith(store.transactionDraft);

        const unchanged = makeTransaction(TransactionType.Expense, { sourceAmountCents: 0, sourceAccountId: 'cash' });
        store.saveTransactionDraft(unchanged, 0, 'expense-default', 'cash', '', 'cash');
        expect(store.transactionDraft).toBeNull();
        expect(mockClearUserTransactionDraft).toHaveBeenCalled();

        mockSettingsStore.appSettings.autoSaveTransactionDraft = 'disabled';
        store.saveTransactionDraft(modified, 0);
        store.initTransactionDraft();
        expect(store.transactionDraft).toBeNull();
        store.clearTransactionDraft();
        expect(mockClearUserTransactionDraft).toHaveBeenCalledTimes(3);
    });

    test('keeps destination amounts aligned for one-sided and cross-currency transfers', () => {
        const store = useTransactionsStore();
        const expense = makeTransaction(TransactionType.Expense);
        store.setTransactionSuitableDestinationAmount(expense, 10, 20);
        expect(expense.destinationAmountCents).toBe(20);

        const transfer = makeTransaction(TransactionType.Transfer, {
            sourceAccountId: 'cash', destinationAccountId: 'usd', destinationAmountCents: 100
        });
        store.setTransactionSuitableDestinationAmount(transfer, 700, 1_400);
        expect(transfer.destinationAmountCents).toBe(200);
        transfer.destinationAmountCents = 999;
        store.setTransactionSuitableDestinationAmount(transfer, 700, 2_100);
        expect(transfer.destinationAmountCents).toBe(999);

        mockGetExchangedAmount.mockReturnValue(null);
        transfer.destinationAmountCents = 0;
        store.setTransactionSuitableDestinationAmount(transfer, 700, 1_400);
        expect(transfer.destinationAmountCents).toBe(0);

        const missingAccount = makeTransaction(TransactionType.Transfer, {
            sourceAccountId: 'missing', destinationAccountId: 'missing-2', destinationAmountCents: 0
        });
        store.setTransactionSuitableDestinationAmount(missingAccount, 1, 2);
        expect(missingAccount.destinationAmountCents).toBe(2);
    });

    test('groups loaded transactions by month and calculates complete daily totals in cents', async () => {
        const store = useTransactionsStore();
        store.initTransactionListFilter({ accountIds: 'cash' });
        mockServices.getTransactions.mockReturnValue(page([
            transactionResponse({ id: 'expense', sourceAmountCents: 100 }),
            transactionResponse({
                id: 'income-usd', type: TransactionType.Income, categoryId: 'income-default',
                sourceAccountId: 'usd', sourceAmountCents: 10, time: Date.UTC(2026, 0, 14, 12) / 1000,
                gregorianCalendarYearDashMonthDashDay: '2026-01-14', gregorianCalendarDayOfMonth: 14
            }),
            transactionResponse({
                id: 'unknown-category', categoryId: 'missing-category', sourceAccountId: 'missing-account',
                time: Date.UTC(2025, 11, 31, 12) / 1000,
                gregorianCalendarYearDashMonthDashDay: undefined, gregorianCalendarDayOfMonth: undefined,
                displayDayOfWeek: undefined
            })
        ], 88));

        await store.loadTransactions({ reload: true, count: 3, page: 1, withCount: true, autoExpand: true, defaultCurrency: 'CNY' });
        expect(store.transactions.map(month => month.yearDashMonth)).toEqual(['2026-01', '2025-12']);
        expect(store.transactions[0]!.opened).toBe(true);
        expect(store.transactions[0]!.items[0]!.category?.id).toBe('expense-default');
        expect(store.transactions[1]!.items[0]!.category?.name).toBe('Unknown');
        expect(store.transactionsNextTimeId).toBe(88);
        expect(store.hasMoreTransaction).toBe(true);
        expect(store.noTransaction).toBe(false);
        expect(mockServices.getTransactions).toHaveBeenCalledWith(expect.objectContaining({
            accountIds: 'cash', count: 3, page: 1, withCount: true
        }));

        mockServices.getTransactions.mockReturnValue(page([
            transactionResponse({
                id: 'older', time: Date.UTC(2025, 11, 30, 12) / 1000,
                gregorianCalendarYearDashMonthDashDay: '2025-12-30', gregorianCalendarDayOfMonth: 30
            })
        ]));
        await store.loadTransactions({ reload: false, autoExpand: false, defaultCurrency: 'CNY' });
        expect(store.transactions[1]!.items).toHaveLength(2);
        expect(store.transactionsNextTimeId).toBe(-1);
        expect(store.hasMoreTransaction).toBe(false);
        expect(store.transactions[0]!.totalAmountCents.expenseCents).toBe(100);
        expect(store.transactions[0]!.totalAmountCents.incomeCents).toBe(70);

        store.collapseMonthInTransactionList({ monthList: store.transactions[0]!, collapse: true });
        expect(store.transactions[0]!.opened).toBe(false);
        store.clearTransactions();
        expect(store.noTransaction).toBe(true);
        expect(store.transactionsNextTimeId).toBe(0);
    });

    test('calculates transfer totals from selected source, destination and parent account combinations', async () => {
        const transfer = transactionResponse({
            id: 'transfer', type: TransactionType.Transfer, categoryId: 'transfer-default',
            sourceAccountId: 'bank-cny', destinationAccountId: 'destination',
            sourceAmountCents: 100, destinationAmountCents: 100
        });
        const store = useTransactionsStore();
        for (const [accountIds, expense, income] of [
            ['bank-cny', 100, 0],
            ['destination', 0, 100],
            ['bank-cny,destination', 0, 0],
            ['bank-parent,destination', 0, 0]
        ] as const) {
            store.initTransactionListFilter({ accountIds });
            mockServices.getAllTransactionsByMonth.mockReturnValue(page([transfer]));
            await store.loadMonthlyAllTransactions({ year: 2026, month: 1, autoExpand: false, defaultCurrency: 'CNY' });
            expect(store.transactions[0]!.totalAmountCents.expenseCents).toBe(expense);
            expect(store.transactions[0]!.totalAmountCents.incomeCents).toBe(income);
        }
    });

    test('marks unavailable exchange totals incomplete and ignores transactions without accounts', async () => {
        mockGetExchangedAmount.mockReturnValue(null);
        mockServices.getAllTransactionsByMonth.mockReturnValue(page([
            transactionResponse({ id: 'expense-usd', sourceAccountId: 'usd', sourceAmountCents: 10 }),
            transactionResponse({ id: 'income-usd', type: TransactionType.Income, categoryId: 'income-default', sourceAccountId: 'usd', sourceAmountCents: 10 }),
            transactionResponse({ id: 'missing', sourceAccountId: 'missing' })
        ]));
        const store = useTransactionsStore();
        await store.loadMonthlyAllTransactions({ year: 2026, month: 1, autoExpand: false, defaultCurrency: 'CNY' });
        expect(store.transactions[0]!.totalAmountCents).toEqual({
            expenseCents: 0, incompleteExpense: true, incomeCents: 0, incompleteIncome: true
        });
        expect(store.transactions[0]!.dailyTotalAmountsCents['15']).toEqual(expect.objectContaining({
            incompleteExpense: true, incompleteIncome: true
        }));
    });

    test('loadTransactions handles empty envelopes and all transport error channels', async () => {
        const store = useTransactionsStore();
        store.updateTransactionListInvalidState(false);
        mockServices.getTransactions.mockReturnValue(invalidResponse());
        await expect(store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ message: 'Unable to retrieve transaction list' });
        expect(store.transactions).toEqual([]);
        expect(store.transactionListStateInvalid).toBe(true);

        store.updateTransactionListInvalidState(false);
        await expectThreeErrorChannels(
            mockServices.getTransactions,
            () => store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' }),
            'Unable to retrieve transaction list'
        );
    });

    test('converts a reloaded max-time filter to the sequence-id boundary', async () => {
        const store = useTransactionsStore();
        store.initTransactionListFilter({ maxTime: 123, minTime: 45 });
        mockServices.getTransactions.mockReturnValue(page([]));
        await store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' });
        expect(mockServices.getTransactions).toHaveBeenCalledWith(expect.objectContaining({
            maxTime: 123_999,
            minTime: 45_000
        }));
    });

    test('monthly loading expands parent accounts and handles empty and failed responses', async () => {
        const store = useTransactionsStore();
        store.initTransactionListFilter({ accountIds: 'bank-parent' });
        mockServices.getAllTransactionsByMonth.mockReturnValue(page([transactionResponse()]));
        await store.loadMonthlyAllTransactions({ year: 2026, month: 1, autoExpand: true, defaultCurrency: 'CNY' });
        expect(mockServices.getAllTransactionsByMonth).toHaveBeenCalledWith(expect.objectContaining({
            accountIds: 'bank-cny,bank-usd'
        }));

        mockServices.getAllTransactionsByMonth.mockReturnValue(invalidResponse());
        await expect(store.loadMonthlyAllTransactions({ year: 2026, month: 1, autoExpand: false, defaultCurrency: 'CNY' }))
            .rejects.toEqual({ message: 'Unable to retrieve transaction list' });
        store.updateTransactionListInvalidState(false);
        await expectThreeErrorChannels(
            mockServices.getAllTransactionsByMonth,
            () => store.loadMonthlyAllTransactions({ year: 2026, month: 1, autoExpand: false, defaultCurrency: 'CNY' }),
            'Unable to retrieve transaction list'
        );
    });

    test('loads reconciliation statements and transaction details with state transitions', async () => {
        const store = useTransactionsStore();
        const reconciliation = { items: [], startTime: 1, endTime: 2 };
        mockServices.getReconciliationStatements.mockReturnValue(success(reconciliation));
        await expect(store.getReconciliationStatements({ accountId: 'cash', startTime: 1, endTime: 2 }))
            .resolves.toBe(reconciliation);
        expect(store.transactionReconciliationStatementStateInvalid).toBe(false);
        mockServices.getReconciliationStatements.mockReturnValue(invalidResponse());
        await expect(store.getReconciliationStatements({ accountId: 'cash', startTime: 1, endTime: 2 }))
            .rejects.toEqual({ message: 'Unable to retrieve reconciliation statements' });
        expect(store.transactionReconciliationStatementStateInvalid).toBe(true);
        store.updateTransactionReconciliationStatementInvalidState(false);
        await expectThreeErrorChannels(
            mockServices.getReconciliationStatements,
            () => store.getReconciliationStatements({ accountId: 'cash', startTime: 1, endTime: 2 }),
            'Unable to retrieve reconciliation statements'
        );

        mockServices.getTransaction.mockReturnValue(success(transactionResponse({ id: 'detail' })));
        await expect(store.getTransaction({ transactionId: 'detail' })).resolves.toMatchObject({ id: 'detail' });
        expect(mockServices.getTransaction).toHaveBeenCalledWith({ id: 'detail', withPictures: true });
        mockServices.getTransaction.mockReturnValue(success(transactionResponse({ id: 'no-pictures' })));
        await store.getTransaction({ transactionId: 'no-pictures', withPictures: false });
        expect(mockServices.getTransaction).toHaveBeenLastCalledWith({ id: 'no-pictures', withPictures: false });
        mockServices.getTransaction.mockReturnValue(invalidResponse());
        await expect(store.getTransaction({ transactionId: 'bad' })).rejects.toEqual({ message: 'Unable to retrieve transaction' });
        await expectThreeErrorChannels(
            mockServices.getTransaction,
            () => store.getTransaction({ transactionId: 'bad' }),
            'Unable to retrieve transaction'
        );
    });

    test('creates and edits supported transactions while invalidating dependent stores', async () => {
        const store = useTransactionsStore();
        store.updateTransactionListInvalidState(false);
        store.updateTransactionReconciliationStatementInvalidState(false);
        const createdRaw = transactionResponse({ id: 'created' });
        mockServices.addTransaction.mockReturnValue(success(createdRaw));
        await expect(store.saveTransaction({
            transaction: makeTransaction(TransactionType.Expense),
            defaultCurrency: 'CNY',
            isEdit: false,
            clientSessionId: 'create'
        })).resolves.toMatchObject({ id: 'created' });
        expect(store.transactionListStateInvalid).toBe(true);
        expect(store.transactionReconciliationStatementStateInvalid).toBe(true);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: true });
        expect(mockOverviewStore.updateTransactionOverviewInvalidState).toHaveBeenCalledWith(true);
        expect(mockStatisticsStore.updateTransactionStatisticsInvalidState).toHaveBeenCalledWith(true);

        mockServices.getTransactions.mockReturnValue(page([transactionResponse({ id: 'edit-me' })]));
        await store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' });
        const editedRaw = transactionResponse({ id: 'edit-me', sourceAmountCents: 250 });
        mockServices.modifyTransaction.mockReturnValue(success(editedRaw));
        await store.saveTransaction({
            transaction: Transaction.of(editedRaw), defaultCurrency: 'CNY', isEdit: true, clientSessionId: 'ignored'
        });
        expect(store.transactions[0]!.items[0]!.sourceAmountCents).toBe(250);
    });

    test('edit removes filtered transactions and invalidates when the display day changes', async () => {
        const store = useTransactionsStore();
        store.initTransactionListFilter({ categoryIds: 'expense-default' });
        mockServices.getTransactions.mockReturnValue(page([transactionResponse({ id: 'edit-me' })]));
        await store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' });

        const filteredOut = transactionResponse({ id: 'edit-me', categoryId: 'other' });
        mockServices.modifyTransaction.mockReturnValue(success(filteredOut));
        await store.saveTransaction({ transaction: Transaction.of(filteredOut), defaultCurrency: 'CNY', isEdit: true, clientSessionId: 'x' });
        expect(store.transactions).toEqual([]);

        store.initTransactionListFilter({});
        mockServices.getTransactions.mockReturnValue(page([transactionResponse({ id: 'date-change' })]));
        await store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' });
        store.updateTransactionListInvalidState(false);
        const changedDay = transactionResponse({
            id: 'date-change', time: Date.UTC(2026, 0, 16, 12) / 1000,
            gregorianCalendarYearDashMonthDashDay: '2026-01-16', gregorianCalendarDayOfMonth: 16
        });
        mockServices.modifyTransaction.mockReturnValue(success(changedDay));
        await store.saveTransaction({ transaction: Transaction.of(changedDay), defaultCurrency: 'CNY', isEdit: true, clientSessionId: 'x' });
        expect(store.transactionListStateInvalid).toBe(true);
    });

    test('saveTransaction rejects unsupported types and operation-specific response or transport failures', async () => {
        const store = useTransactionsStore();
        await expect(store.saveTransaction({
            transaction: makeTransaction(99), defaultCurrency: 'CNY', isEdit: false, clientSessionId: 'x'
        })).rejects.toEqual({ message: 'An error occurred' });
        await expect(store.saveTransaction({
            transaction: makeTransaction(TransactionType.ModifyBalance), defaultCurrency: 'CNY', isEdit: false, clientSessionId: 'x'
        })).rejects.toEqual({ message: 'An error occurred' });

        for (const isEdit of [false, true]) {
            const service = isEdit ? mockServices.modifyTransaction : mockServices.addTransaction;
            const fallback = isEdit ? 'Unable to save transaction' : 'Unable to add transaction';
            service.mockReturnValue(invalidResponse());
            await expect(store.saveTransaction({
                transaction: makeTransaction(TransactionType.Expense), defaultCurrency: 'CNY', isEdit, clientSessionId: 'x'
            })).rejects.toEqual({ message: fallback });
            await expectThreeErrorChannels(
                service,
                () => store.saveTransaction({
                    transaction: makeTransaction(TransactionType.Expense), defaultCurrency: 'CNY', isEdit, clientSessionId: 'x'
                }),
                fallback
            );
        }
    });

    test('batch-saves valid transaction types and rejects empty, invalid or failed batches', async () => {
        const store = useTransactionsStore();
        await expect(store.saveTransactions({
            transactions: [makeTransaction(TransactionType.ModifyBalance)], clientSessionId: 'batch'
        })).rejects.toEqual({ message: 'Unable to add transaction' });

        mockServices.addTransactions.mockReturnValue(success({
            items: [transactionResponse({ id: 'batch-1' })], ids: ['batch-1'], createdCount: 1
        }));
        const result = await store.saveTransactions({
            transactions: [
                makeTransaction(TransactionType.Expense),
                makeTransaction(TransactionType.Income),
                makeTransaction(TransactionType.Transfer),
                makeTransaction(TransactionType.Investment),
                makeTransaction(TransactionType.ModifyBalance)
            ],
            clientSessionId: 'batch'
        });
        expect(result.map(item => item.id)).toEqual(['batch-1']);
        expect((mockServices.addTransactions.mock.calls[0]![0] as { transactions: unknown[] }).transactions).toHaveLength(4);

        mockServices.addTransactions.mockReturnValue(invalidResponse());
        await expect(store.saveTransactions({
            transactions: [makeTransaction(TransactionType.Expense)], clientSessionId: 'bad'
        })).rejects.toEqual({ message: 'Unable to add transaction' });
        await expectThreeErrorChannels(
            mockServices.addTransactions,
            () => store.saveTransactions({ transactions: [makeTransaction(TransactionType.Expense)], clientSessionId: 'bad' }),
            'Unable to add transaction'
        );
    });

    test('moves and deletes transactions with local mutation callbacks and dependent invalidation', async () => {
        const store = useTransactionsStore();
        store.updateTransactionListInvalidState(false);
        store.updateTransactionReconciliationStatementInvalidState(false);
        mockAccountsStore.accountListStateInvalid = false;
        mockServices.moveAllTransactionsBetweenAccounts.mockReturnValue(success(true));
        await expect(store.moveAllTransactionsBetweenAccounts({
            fromAccountId: 'cash', toAccountId: 'destination', password: 'secret'
        })).resolves.toBe(true);
        expect(mockAccountsStore.updateAccountListInvalidState).toHaveBeenCalledWith(true);

        mockServices.getTransactions.mockReturnValue(page([transactionResponse({ id: 'delete-me' })]));
        await store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' });
        mockServices.deleteTransaction.mockReturnValue(success(true));
        let mutation: (() => void) | undefined;
        await store.deleteTransaction({
            transaction: transactionResponse({ id: 'delete-me' }),
            defaultCurrency: 'CNY',
            beforeResolve: callback => { mutation = callback; }
        });
        expect(store.transactions).toHaveLength(1);
        mutation!();
        expect(store.transactions).toEqual([]);

        mockServices.getTransactions.mockReturnValue(page([
            transactionResponse({ id: 'delete-direct' }),
            transactionResponse({ id: 'keep-me', time: Date.UTC(2026, 0, 14, 12) / 1000 })
        ], 5));
        await store.loadTransactions({ reload: true, autoExpand: false, defaultCurrency: 'CNY' });
        store.updateTransactionReconciliationStatementInvalidState(false);
        mockServices.deleteTransaction.mockReturnValue(success(true));
        await store.deleteTransaction({
            transaction: transactionResponse({ id: 'delete-direct' }), defaultCurrency: 'CNY'
        });
        expect(store.transactions[0]!.items.map(item => item.id)).toEqual(['keep-me']);
        expect(store.transactionReconciliationStatementStateInvalid).toBe(true);

        mockServices.deleteTransaction.mockReturnValue(success(true));
        await store.deleteTransaction({
            transaction: transactionResponse({
                id: 'outside-range',
                time: Date.UTC(2030, 0, 1, 12) / 1000
            }),
            defaultCurrency: 'CNY'
        });
        expect(store.transactions[0]!.items.map(item => item.id)).toEqual(['keep-me']);
    });

    test('move and delete operations preserve invalid and transport errors', async () => {
        const store = useTransactionsStore();
        mockServices.moveAllTransactionsBetweenAccounts.mockReturnValue(invalidResponse());
        await expect(store.moveAllTransactionsBetweenAccounts({ fromAccountId: 'a', toAccountId: 'b', password: 'p' }))
            .rejects.toEqual({ message: 'Unable to move transactions' });
        await expectThreeErrorChannels(
            mockServices.moveAllTransactionsBetweenAccounts,
            () => store.moveAllTransactionsBetweenAccounts({ fromAccountId: 'a', toAccountId: 'b', password: 'p' }),
            'Unable to move transactions'
        );

        mockServices.deleteTransaction.mockReturnValue(invalidResponse());
        await expect(store.deleteTransaction({ transaction: transactionResponse(), defaultCurrency: 'CNY' }))
            .rejects.toEqual({ message: 'Unable to delete this transaction' });
        await expectThreeErrorChannels(
            mockServices.deleteTransaction,
            () => store.deleteTransaction({ transaction: transactionResponse(), defaultCurrency: 'CNY' }),
            'Unable to delete this transaction'
        );
    });

    test('parses imports and manages transaction pictures through service boundaries', async () => {
        const store = useTransactionsStore();
        const file = { name: 'bill.csv' } as File;
        const parsed = { items: [], totalCount: 0, nextPage: null };
        mockServices.parseImportTransaction.mockReturnValue(success(parsed));
        await expect(store.parseImportTransaction({ fileType: 'csv', importFile: file, delimiter: ',' }))
            .resolves.toBe(parsed);
        expect(mockServices.parseImportTransaction).toHaveBeenCalledWith(expect.objectContaining({
            fileType: 'csv', importFile: file, delimiter: ','
        }));

        const picture = { pictureId: 'picture-1', originalUrl: '/pictures/1' };
        mockServices.uploadTransactionPicture.mockReturnValue(success(picture));
        await expect(store.uploadTransactionPicture({ pictureFile: file, clientSessionId: 'upload' }))
            .resolves.toBe(picture);
        mockServices.removeUnusedTransactionPicture.mockReturnValue(success(true));
        await expect(store.removeUnusedTransactionPicture({ pictureInfo: picture })).resolves.toBe(true);
        expect(store.getTransactionPictureUrl()).toBeUndefined();
        expect(store.getTransactionPictureUrl({ pictureId: 'no-url', originalUrl: '' })).toBeUndefined();
        expect(store.getTransactionPictureUrl(picture, true)).toBe('/pictures/1?cache=true');
        store.cancelRecognizeReceiptImage('cancel-id');
        expect(mockServices.cancelRequest).toHaveBeenCalledWith('cancel-id');
    });

    test('import and picture helpers reject invalid responses and all transport error channels', async () => {
        const store = useTransactionsStore();
        const file = { name: 'bill.csv' } as File;
        const actions = [
            {
                service: mockServices.parseImportTransaction,
                invoke: () => store.parseImportTransaction({ fileType: 'csv', importFile: file }),
                fallback: 'Unable to parse import file'
            },
            {
                service: mockServices.uploadTransactionPicture,
                invoke: () => store.uploadTransactionPicture({ pictureFile: file }),
                fallback: 'Unable to upload transaction picture'
            },
            {
                service: mockServices.removeUnusedTransactionPicture,
                invoke: () => store.removeUnusedTransactionPicture({ pictureInfo: { pictureId: 'p', originalUrl: '/p' } }),
                fallback: 'Unable to remove transaction picture'
            }
        ];
        for (const action of actions) {
            action.service.mockReturnValue(invalidResponse());
            await expect(action.invoke()).rejects.toEqual({ message: action.fallback });
            await expectThreeErrorChannels(action.service, action.invoke, action.fallback);
        }
    });
});
