import { expect, jest, test } from '@jest/globals';

import type { E2EEnvironment } from '../../../src/web/e2e/helpers/env';
import {
    AUTH_STORAGE_STATE_PATH,
    saveAuthenticatedStorageState,
    type AuthResponse
} from '../../../src/web/e2e/helpers/testAccount';

function buildEnvironment(): E2EEnvironment {
    return {
        baseURL: 'http://127.0.0.1:8081',
        apiBaseURL: 'http://127.0.0.1:8081/api',
        account: {
            username: 'bill-analyser-e2e',
            email: 'bill-analyser-e2e@example.test',
            nickname: 'Bill Analyser E2E',
            password: 'BillAnalyserE2E!2026'
        },
        runId: 'e2e-test',
        expectedWeaviatePrefix: 'BillAnalyserE2E',
        allowSharedLocalDatabase: false,
        allowCIServiceHosts: false,
        healthTimeoutMs: 10_000
    };
}

test('authenticated storage setup waits only for the document response commit', async () => {
    const goto = jest.fn<(url: string, options: { waitUntil: string }) => Promise<FakeNavigationResponse>>(
        async () => navigationResponse(200)
    );
    const evaluate = jest.fn<(...args: unknown[]) => Promise<void>>(async () => undefined);
    const storageState = jest.fn<(options: { path: string }) => Promise<void>>(async () => undefined);
    const close = jest.fn(async () => undefined);
    const newPage = jest.fn(async () => ({ goto, evaluate }));
    const newContext = jest.fn<(options: { baseURL: string }) => Promise<{
        newPage: typeof newPage;
        storageState: typeof storageState;
        close: typeof close;
    }>>(async () => ({ newPage, storageState, close }));
    const env = buildEnvironment();
    const auth: AuthResponse = {
        token: 'access-token',
        refreshToken: 'refresh-token',
        user: { id: 7, username: 'bill-analyser-e2e' }
    };

    await saveAuthenticatedStorageState({ newContext } as never, env, auth);

    expect(newContext).toHaveBeenCalledWith({ baseURL: env.baseURL });
    expect(goto).toHaveBeenCalledWith(
        'http://127.0.0.1:8081/desktop.html#/login',
        { waitUntil: 'commit' }
    );
    expect(evaluate).toHaveBeenCalledWith(expect.any(Function), { authState: auth });
    expect(storageState).toHaveBeenCalledWith({ path: AUTH_STORAGE_STATE_PATH });
    expect(close).toHaveBeenCalledTimes(1);
});

test('authenticated storage setup rejects a non-success document response without persisting credentials', async () => {
    const goto = jest.fn<(url: string, options: { waitUntil: string }) => Promise<FakeNavigationResponse>>(
        async () => navigationResponse(503)
    );
    const evaluate = jest.fn<(...args: unknown[]) => Promise<void>>(async () => undefined);
    const storageState = jest.fn<(options: { path: string }) => Promise<void>>(async () => undefined);
    const close = jest.fn(async () => undefined);
    const newPage = jest.fn(async () => ({ goto, evaluate }));
    const newContext = jest.fn(async () => ({ newPage, storageState, close }));

    await expect(saveAuthenticatedStorageState(
        { newContext } as never,
        buildEnvironment(),
        { token: 'access-token' }
    )).rejects.toThrow('E2E auth storage seed route returned HTTP 503');

    expect(evaluate).not.toHaveBeenCalled();
    expect(storageState).not.toHaveBeenCalled();
    expect(close).toHaveBeenCalledTimes(1);
});

test('authenticated storage setup closes its temporary context when navigation fails', async () => {
    const goto = jest.fn(async () => {
        throw new Error('navigation failed');
    });
    const close = jest.fn(async () => undefined);
    const newPage = jest.fn(async () => ({ goto, evaluate: jest.fn() }));
    const newContext = jest.fn(async () => ({ newPage, storageState: jest.fn(), close }));

    await expect(saveAuthenticatedStorageState(
        { newContext } as never,
        buildEnvironment(),
        { token: 'access-token' }
    )).rejects.toThrow('navigation failed');

    expect(close).toHaveBeenCalledTimes(1);
});

test('authenticated storage setup preserves both navigation and context cleanup failures', async () => {
    const navigationError = new Error('navigation failed');
    const cleanupError = new Error('context close failed');
    const goto = jest.fn(async () => {
        throw navigationError;
    });
    const close = jest.fn(async () => {
        throw cleanupError;
    });
    const newPage = jest.fn(async () => ({ goto, evaluate: jest.fn() }));
    const newContext = jest.fn(async () => ({ newPage, storageState: jest.fn(), close }));

    let caught: unknown;
    try {
        await saveAuthenticatedStorageState(
            { newContext } as never,
            buildEnvironment(),
            { token: 'access-token' }
        );
    } catch (error) {
        caught = error;
    }

    expect(caught).toBeInstanceOf(AggregateError);
    expect((caught as AggregateError).errors).toEqual([navigationError, cleanupError]);
    expect((caught as AggregateError).message)
        .toBe('E2E auth storage seed failed and temporary context cleanup failed.');
});

interface FakeNavigationResponse {
    readonly ok: () => boolean;
    readonly status: () => number;
}

function navigationResponse(status: number): FakeNavigationResponse {
    return {
        ok: () => status >= 200 && status < 300,
        status: () => status
    };
}
