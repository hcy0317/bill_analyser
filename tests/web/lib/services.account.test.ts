import { beforeEach, describe, expect, jest, test } from '@jest/globals';

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

// services.ts reads window.location while initializing the shared axios facade.
(globalThis as unknown as { window: { location: { pathname: string; origin: string } } }).window = {
    location: { pathname: '/', origin: 'http://localhost' }
};

let services: typeof import('@/lib/services.ts').default;

beforeEach(async () => {
    axiosMock.get.mockReset();
    axiosMock.post.mockReset();
    axiosMock.put.mockReset();
    axiosMock.delete.mockReset();
    axiosMock.postForm.mockReset();
    axiosMock.get.mockImplementation(() => Promise.resolve({ data: { success: true, result: [] } }));
    axiosMock.post.mockImplementation(() => Promise.resolve({ data: { success: true, result: true } }));
    axiosMock.put.mockImplementation(() => Promise.resolve({ data: { success: true, result: true } }));
    axiosMock.delete.mockImplementation(() => Promise.resolve({ data: { success: true, result: true } }));
    jest.resetModules();
    services = (await import('@/lib/services.ts')).default;
});

describe('services account adapters', () => {
    test('loads accounts with the visible_only query and auth-refresh guard config', async () => {
        await services.getAllAccounts({ visibleOnly: true });

        expect(axiosMock.get).toHaveBeenCalledWith('accounts?visible_only=true', {
            headers: {},
            ignoreError: true
        });
    });

    test('clears account transactions through the account-scoped Rust REST route', async () => {
        await services.clearAllTransactionsOfAccount({ accountId: '42', password: 'secret' });

        expect(axiosMock.post).toHaveBeenCalledWith(
            'accounts/42/transactions/clear',
            { password: 'secret' },
            expect.objectContaining({ timeout: expect.any(Number) })
        );
    });

    test('moves all transactions between accounts through the account-scoped route', async () => {
        await services.moveAllTransactionsBetweenAccounts({
            fromAccountId: 'source-account',
            toAccountId: 'destination-account',
            password: 'secret'
        });

        expect(axiosMock.post).toHaveBeenCalledWith(
            'accounts/source-account/transactions/move',
            {
                toAccountId: 'destination-account',
                password: 'secret'
            }
        );
    });

    test('syncs balances through the dedicated account sync endpoint', async () => {
        await services.syncAllAccountBalances();

        expect(axiosMock.post).toHaveBeenCalledWith('accounts/sync-balances');
    });
});
