import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import type {
    TransactionCreateRequest,
    TransactionImportRequest,
    TransactionModifyRequest,
    TransactionReconciliationStatementRequest
} from '@/models/transaction.ts';

const interceptorStub = {
    use: jest.fn(),
    eject: jest.fn()
};

const axiosMock = {
    defaults: { baseURL: '', timeout: 0, headers: { common: {} as Record<string, string> } },
    interceptors: { request: interceptorStub, response: interceptorStub },
    get: jest.fn(),
    post: jest.fn(),
    put: jest.fn(),
    delete: jest.fn(),
    postForm: jest.fn()
};

jest.mock('axios', () => ({
    __esModule: true,
    default: axiosMock,
    AxiosHeaders: class {}
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

(globalThis as unknown as { window: { location: { pathname: string; origin: string } } }).window = {
    location: { pathname: '/', origin: 'http://localhost' }
};

let services: typeof import('@/lib/services.ts').default;

function transactionCreateRequest(overrides: Partial<TransactionCreateRequest> = {}): TransactionCreateRequest {
    return {
        type: TransactionType.Expense,
        categoryId: 'cat-food',
        time: 1777636800,
        utcOffset: 480,
        sourceAccountId: 'wallet',
        destinationAccountId: '0',
        sourceAmount: 1234,
        destinationAmount: 0,
        hideAmount: false,
        tagIds: ['tag-breakfast'],
        pictureIds: [],
        comment: '早餐',
        clientSessionId: 'session-1',
        ...overrides
    };
}

function transactionModifyRequest(overrides: Partial<TransactionModifyRequest> = {}): TransactionModifyRequest {
    return {
        id: 'bill-1',
        type: TransactionType.Expense,
        categoryId: 'cat-food',
        time: 1777636800,
        utcOffset: 480,
        sourceAccountId: 'wallet',
        destinationAccountId: '0',
        sourceAmount: 1234,
        destinationAmount: 0,
        hideAmount: false,
        tagIds: ['tag-breakfast'],
        pictureIds: [],
        comment: '早餐',
        ...overrides
    };
}

beforeEach(async () => {
    axiosMock.get.mockReset();
    axiosMock.post.mockReset();
    axiosMock.put.mockReset();
    axiosMock.delete.mockReset();
    axiosMock.postForm.mockReset();
    axiosMock.get.mockImplementation(() => Promise.resolve({ data: { success: true, result: {} } }));
    axiosMock.post.mockImplementation(() => Promise.resolve({ data: { success: true, result: {} } }));
    axiosMock.put.mockImplementation(() => Promise.resolve({ data: { success: true, result: {} } }));
    axiosMock.delete.mockImplementation(() => Promise.resolve({ data: { success: true, result: true } }));
    jest.resetModules();
    services = (await import('@/lib/services.ts')).default;
});

describe('services transaction facade adapters', () => {
    test('gets paged transactions through the Rust bills route with encoded filter query', async () => {
        await services.getTransactions({
            maxTime: 1777636800999,
            minTime: 1777550400000,
            count: 20,
            page: 3,
            withCount: true,
            type: TransactionType.Expense,
            categoryIds: 'cat/food',
            accountIds: 'wallet,bank card',
            tagIds: 'tag-breakfast',
            tagFilterType: 1,
            amountFilter: 'gte:100',
            keyword: 'coffee shop'
        });

        expect(axiosMock.get).toHaveBeenCalledWith(
            'bills/?max_time=1777636800999&min_time=1777550400000&type=3&categoryIds=cat%2Ffood&accountIds=wallet%2Cbank%20card&tagIds=tag-breakfast&tagFilterType=1&amountFilter=gte%3A100&keyword=coffee%20shop&page_size=20&page=3&with_count=true'
        );
    });

    test('keeps by-month transaction filters on the Rust by-month route', async () => {
        await services.getAllTransactionsByMonth({
            year: 2026,
            month: 5,
            type: TransactionType.Income,
            categoryIds: 'salary',
            accountIds: 'bank card',
            tagIds: 'tag/payroll',
            tagFilterType: 0,
            amountFilter: 'lt:9999',
            keyword: 'monthly payroll'
        });

        expect(axiosMock.get).toHaveBeenCalledWith(
            'bills/by-month?year=2026&month=5&type=2&categoryIds=salary&accountIds=bank card&tagIds=tag/payroll&tagFilterType=0&amountFilter=lt%3A9999&keyword=monthly%20payroll'
        );
    });

    test('loads one transaction with picture and trim flags', async () => {
        await services.getTransaction({ id: 'bill-42', withPictures: false });

        expect(axiosMock.get).toHaveBeenCalledWith(
            'bills/get?id=bill-42&with_pictures=false&trim_account=true&trim_category=true&trim_tag=true'
        );
    });

    test('defaults single transaction pictures and appends reconciliation optional filters', async () => {
        await services.getTransaction({ id: 'bill-99', withPictures: undefined });
        await services.getReconciliationStatements({
            accountId: 'bank-card',
            startTime: 1777550400000,
            endTime: 1777636800999,
            categoryIds: 'cat/food',
            type: 2,
            keyword: 'salary bonus'
        } as TransactionReconciliationStatementRequest & {
            categoryIds: string;
            type: number;
            keyword: string;
        });

        expect(axiosMock.get).toHaveBeenNthCalledWith(
            1,
            'bills/get?id=bill-99&with_pictures=true&trim_account=true&trim_category=true&trim_tag=true'
        );
        expect(axiosMock.get).toHaveBeenNthCalledWith(
            2,
            'bills/reconciliation_statements?account_id=bank-card&start_time=1777550400000&end_time=1777636800999&category_ids=cat/food&type=2&keyword=salary%20bonus'
        );
    });

    test('creates, batches, modifies, moves, and deletes transactions on stable Rust routes', async () => {
        const createReq = transactionCreateRequest();
        const importReq: TransactionImportRequest = {
            transactions: [createReq],
            clientSessionId: 'session-1'
        };
        const modifyReq = transactionModifyRequest();

        await services.addTransaction(createReq);
        await services.addTransactions(importReq);
        await services.modifyTransaction(modifyReq);
        await services.moveAllTransactionsBetweenAccounts({
            fromAccountId: 'wallet',
            toAccountId: 'bank-card',
            password: 'secret'
        });
        await services.deleteTransaction({ id: 'bill-1' });

        expect(axiosMock.post).toHaveBeenNthCalledWith(1, 'bills', createReq);
        expect(axiosMock.post).toHaveBeenNthCalledWith(2, 'bills/batch', importReq);
        expect(axiosMock.put).toHaveBeenCalledWith('bills/bill-1', modifyReq);
        expect(axiosMock.post).toHaveBeenNthCalledWith(3, 'accounts/wallet/transactions/move', {
            toAccountId: 'bank-card',
            password: 'secret'
        });
        expect(axiosMock.delete).toHaveBeenCalledWith('bills/bill-1');
    });
});
