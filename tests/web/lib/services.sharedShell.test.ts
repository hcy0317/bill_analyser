import { beforeEach, describe, expect, jest, test } from '@jest/globals';

type RequestInterceptor = (config: Record<string, any>) => Record<string, any> | Promise<Record<string, any>>;
type ResponseInterceptor = (response: Record<string, any>) => Record<string, any> | Promise<Record<string, any>>;
type ResponseErrorInterceptor = (error: Record<string, any>) => Promise<never>;

const mockAxiosGet = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAxiosPost = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAxiosPut = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAxiosDelete = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAxiosPostForm = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockRequestUse = jest.fn((onFulfilled: RequestInterceptor) => {
    mockRequestInterceptor = onFulfilled;
    return 1;
});
const mockResponseUse = jest.fn((onFulfilled: ResponseInterceptor, onRejected?: ResponseErrorInterceptor) => {
    mockResponseInterceptor = onFulfilled;
    mockResponseErrorInterceptor = onRejected ?? null;
    return 1;
});

let mockRequestInterceptor: RequestInterceptor | null = null;
let mockResponseInterceptor: ResponseInterceptor | null = null;
let mockResponseErrorInterceptor: ResponseErrorInterceptor | null = null;
let mockAccessToken = '';
let mockRefreshToken = '';
const mockUpdateCurrentToken = jest.fn((token: string) => {
    mockAccessToken = token;
});
const mockUpdateCurrentRefreshToken = jest.fn((token: string) => {
    mockRefreshToken = token;
});
const mockLoggerError = jest.fn();

const mockAxios = {
    defaults: {
        baseURL: '',
        timeout: 0,
        headers: {
            common: {} as Record<string, string>
        }
    },
    interceptors: {
        request: {
            use: mockRequestUse
        },
        response: {
            use: mockResponseUse
        }
    },
    get: mockAxiosGet,
    post: mockAxiosPost,
    put: mockAxiosPut,
    delete: mockAxiosDelete,
    postForm: mockAxiosPostForm
};

jest.mock('axios', () => ({
    __esModule: true,
    default: mockAxios,
    AxiosHeaders: class {
        private readonly values: Record<string, string> = {};

        public set(key: string, value: string): void {
            this.values[key] = value;
        }

        public get(key: string): string | undefined {
            return this.values[key];
        }
    }
}));

jest.mock('@/lib/userstate.ts', () => ({
    __esModule: true,
    getCurrentToken: () => mockAccessToken,
    getCurrentRefreshToken: () => mockRefreshToken,
    updateCurrentToken: (token: string) => mockUpdateCurrentToken(token),
    updateCurrentRefreshToken: (token: string) => mockUpdateCurrentRefreshToken(token),
    clearCurrentTokenAndUserInfo: jest.fn()
}));

jest.mock('@/lib/settings.ts', () => ({
    __esModule: true,
    isEnableApplicationLock: () => false
}));

jest.mock('@/lib/datetime.ts', () => ({
    __esModule: true,
    getTimezoneOffsetMinutes: () => 480
}));

jest.mock('@/lib/web.ts', () => ({
    __esModule: true,
    getBasePath: () => '/desktop'
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: jest.fn(),
        error: (...args: Array<unknown>) => mockLoggerError(...args)
    }
}));

function makeStorage(): Storage {
    const values = new Map<string, string>();

    return {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => { values.set(key, value); },
        removeItem: (key: string) => { values.delete(key); },
        clear: () => { values.clear(); },
        key: (index: number) => Array.from(values.keys())[index] ?? null,
        get length() { return values.size; }
    } as unknown as Storage;
}

function apiResponse<T>(result: T): { data: { success: boolean; result: T } } {
    return {
        data: {
            success: true,
            result
        }
    };
}

async function loadServices(): Promise<typeof import('@/lib/services.ts')> {
    jest.resetModules();
    mockRequestInterceptor = null;
    mockResponseInterceptor = null;
    mockResponseErrorInterceptor = null;
    return import('@/lib/services.ts');
}

describe('services shared runtime shell', () => {
    beforeEach(() => {
        mockAccessToken = 'access-token';
        mockRefreshToken = 'refresh-token';
        mockAxios.defaults.baseURL = '';
        mockAxios.defaults.timeout = 0;
        mockAxios.defaults.headers.common = {};
        mockAxiosGet.mockReset().mockResolvedValue(apiResponse({}));
        mockAxiosPost.mockReset().mockResolvedValue(apiResponse({}));
        mockAxiosPut.mockReset().mockResolvedValue(apiResponse({}));
        mockAxiosDelete.mockReset().mockResolvedValue(apiResponse(true));
        mockAxiosPostForm.mockReset().mockResolvedValue(apiResponse({}));
        mockRequestUse.mockClear();
        mockResponseUse.mockClear();
        mockUpdateCurrentToken.mockClear();
        mockUpdateCurrentRefreshToken.mockClear();
        mockLoggerError.mockClear();

        (globalThis as unknown as { localStorage: Storage }).localStorage = makeStorage();
        (globalThis as unknown as { sessionStorage: Storage }).sessionStorage = makeStorage();
        (globalThis as unknown as { window: { location: { pathname: string; origin: string; hash: string } } }).window = {
            location: {
                pathname: '/',
                origin: 'http://localhost',
                hash: ''
            }
        };
        (globalThis as unknown as { location: { reload: () => void } }).location = {
            reload: jest.fn()
        };
    });

    test('module initialization keeps the /api base URL and registers axios interceptors', async () => {
        await loadServices();

        expect(mockAxios.defaults.baseURL).toBe('/desktop/api');
        expect(mockAxios.defaults.timeout).toBeGreaterThan(0);
        expect(mockRequestUse).toHaveBeenCalledTimes(1);
        expect(mockResponseUse).toHaveBeenCalledTimes(1);
        expect(mockResponseErrorInterceptor).not.toBeNull();
    });

    test.each([
        ['refresh', 'tokens/refresh', 'post', {
            message: 'Token refresh failed',
            route: 'tokens/refresh'
        }],
        ['revoke', 'tokens/session-id', 'delete', {
            message: 'Token revoke failed',
            route: 'tokens/:id'
        }]
    ])('response interceptor logs only safe metadata for hostile token %s errors', async (_operation, url, method, expectedFailure) => {
        await loadServices();
        expect(mockResponseErrorInterceptor).not.toBeNull();

        const sentinel = `TOKEN-RESPONSE-INTERCEPTOR-${String(_operation).toUpperCase()}-SENTINEL`;
        const hostileConfig = {
            url,
            method,
            headers: {
                Authorization: `Bearer ${sentinel}`
            },
            data: JSON.stringify({ refreshToken: sentinel })
        };
        const hostileError = Object.assign(new Error(`token request failed: ${sentinel}`), {
            code: 'ERR_BAD_RESPONSE',
            cause: { secret: sentinel },
            raw: { secret: sentinel },
            config: hostileConfig,
            request: { body: sentinel },
            response: {
                status: 503,
                config: hostileConfig,
                data: {
                    error: sentinel,
                    message: sentinel,
                    refreshToken: sentinel,
                    raw: { secret: sentinel }
                }
            }
        });

        await expect(mockResponseErrorInterceptor!(hostileError)).rejects.toBe(hostileError);

        const safeFailure = {
            ...expectedFailure,
            code: 'ERR_BAD_RESPONSE',
            status: 503
        };
        expect(mockLoggerError).toHaveBeenCalledTimes(1);
        expect(mockLoggerError).toHaveBeenCalledWith('[auth-token] Token operation failed', safeFailure);
        expect(Object.isFrozen(mockLoggerError.mock.calls[0]?.[1])).toBe(true);

        const serializedLoggerCalls = JSON.stringify(mockLoggerError.mock.calls).toLowerCase();
        for (const forbiddenValue of [
            sentinel.toLowerCase(),
            'refreshtoken',
            'bearer'
        ]) {
            expect(serializedLoggerCalls).not.toContain(forbiddenValue);
        }
        for (const forbiddenKey of [
            'config',
            'request',
            'response',
            'data',
            'cause',
            'raw'
        ]) {
            expect(serializedLoggerCalls).not.toContain(`"${forbiddenKey}"`);
        }
    });

    test('response interceptor preserves the existing error log contract for ordinary API routes', async () => {
        await loadServices();
        expect(mockResponseErrorInterceptor).not.toBeNull();

        const ordinaryError = {
            response: {
                status: 422,
                config: {
                    url: 'bills',
                    method: 'get',
                    headers: {
                        Authorization: 'Bearer ordinary-access-token'
                    }
                },
                data: {
                    message: 'Invalid bill request'
                }
            }
        };

        await expect(mockResponseErrorInterceptor!(ordinaryError)).rejects.toBe(ordinaryError);
        expect(mockLoggerError).toHaveBeenCalledWith(
            '[Response Error] 422 bills - Config had Authorization: YES',
            {
                allConfigHeaders: 'Authorization',
                responseMessage: 'Invalid bill request'
            }
        );
    });

    test('auth helpers write and clear the shared axios Authorization header', async () => {
        const servicesModule = await loadServices();

        servicesModule.initializeAxiosAuth();
        expect(mockAxios.defaults.headers.common['Authorization']).toBe('Bearer access-token');

        servicesModule.updateAxiosAuthorizationHeader('next-token');
        expect(mockAxios.defaults.headers.common['Authorization']).toBe('Bearer next-token');

        servicesModule.updateAxiosAuthorizationHeader('');
        expect(mockAxios.defaults.headers.common['Authorization']).toBeUndefined();
    });

    test('request interceptor attaches token and timezone headers only for protected calls', async () => {
        await loadServices();
        expect(mockRequestInterceptor).not.toBeNull();

        const protectedConfig = await mockRequestInterceptor!({
            url: 'accounts',
            headers: {}
        });
        const publicConfig = await mockRequestInterceptor!({
            url: 'auth/login?next=/',
            headers: {}
        });

        expect(protectedConfig['headers']['Authorization']).toBe('Bearer access-token');
        expect(protectedConfig['headers']['authorization']).toBe('Bearer access-token');
        expect(protectedConfig['headers']['X-Timezone-Offset']).toBe(480);
        expect(publicConfig['headers']['Authorization']).toBeUndefined();
        expect(publicConfig['headers']['X-Timezone-Offset']).toBe(480);
    });

    test('request interceptor strips merged bearer headers from locked and noAuth wire configs', async () => {
        await loadServices();
        expect(mockRequestInterceptor).not.toBeNull();

        mockAccessToken = '';
        mockAxios.defaults.headers.common['Authorization'] = 'Bearer stale-default-token';
        const lockedConfig = await mockRequestInterceptor!({
            url: 'accounts',
            headers: {
                Authorization: 'Bearer stale-default-token',
                authorization: 'Bearer stale-lowercase-token'
            }
        });

        expect(lockedConfig['headers']['Authorization']).toBeUndefined();
        expect(lockedConfig['headers']['authorization']).toBeUndefined();
        expect(mockAxios.defaults.headers.common['Authorization']).toBeUndefined();

        mockAccessToken = 'unlocked-access-token';
        mockAxios.defaults.headers.common['Authorization'] = 'Bearer unlocked-access-token';
        const publicConfig = await mockRequestInterceptor!({
            url: 'auth/login',
            noAuth: true,
            headers: {
                Authorization: 'Bearer unlocked-access-token',
                authorization: 'Bearer unlocked-access-token'
            }
        });

        expect(publicConfig['headers']['Authorization']).toBeUndefined();
        expect(publicConfig['headers']['authorization']).toBeUndefined();
        expect(mockAxios.defaults.headers.common['Authorization']).toBe('Bearer unlocked-access-token');

        const explicitChallengeConfig = await mockRequestInterceptor!({
            url: '2fa/verify',
            noAuth: true,
            preserveExplicitAuthorization: true,
            headers: {
                Authorization: 'Bearer explicit-2fa-challenge'
            }
        });
        expect(explicitChallengeConfig['headers']['Authorization']).toBe('Bearer explicit-2fa-challenge');
    });

    test('refreshToken coalesces callers and persists rotated credentials before unblocking every request', async () => {
        const { default: services } = await loadServices();
        expect(mockRequestInterceptor).not.toBeNull();

        const refreshDeferred: { resolve?: (value: unknown) => void } = {};
        mockAxiosPost.mockImplementationOnce((url: unknown, body: unknown, config: unknown) => {
            expect(url).toBe('tokens/refresh');
            expect(body).toStrictEqual({ refreshToken: 'refresh-token' });
            expect(config).toMatchObject({
                ignoreBlocked: true,
                noAuth: true
            });
            return new Promise(resolve => {
                refreshDeferred.resolve = resolve;
            });
        });

        const refreshPromise = services.refreshToken();
        const duplicateRefreshPromise = services.refreshToken();
        const firstQueuedRequest = mockRequestInterceptor!({
            url: 'accounts',
            headers: {}
        }) as Promise<Record<string, any>>;
        const secondQueuedRequest = mockRequestInterceptor!({
            url: 'bills',
            headers: {}
        }) as Promise<Record<string, any>>;

        let firstQueuedResolved = false;
        firstQueuedRequest.then(() => {
            firstQueuedResolved = true;
        });
        await Promise.resolve();
        expect(firstQueuedResolved).toBe(false);
        expect(mockAxiosPost).toHaveBeenCalledTimes(1);

        const completeRefresh = refreshDeferred.resolve;
        if (!completeRefresh) {
            throw new Error('refresh promise resolver was not captured');
        }
        completeRefresh(apiResponse({
            newToken: 'new-access-token',
            refreshToken: 'rotated-refresh-token'
        }));

        await expect(refreshPromise).resolves.toMatchObject({
            data: {
                result: {
                    newToken: 'new-access-token',
                    refreshToken: 'rotated-refresh-token'
                }
            }
        });
        await expect(duplicateRefreshPromise).resolves.toMatchObject({
            data: {
                result: {
                    newToken: 'new-access-token'
                }
            }
        });
        expect(mockUpdateCurrentToken).toHaveBeenCalledWith('new-access-token');
        expect(mockUpdateCurrentRefreshToken).toHaveBeenCalledWith('rotated-refresh-token');
        await expect(firstQueuedRequest).resolves.toMatchObject({
            headers: expect.objectContaining({
                Authorization: 'Bearer new-access-token'
            })
        });
        await expect(secondQueuedRequest).resolves.toMatchObject({
            headers: expect.objectContaining({
                Authorization: 'Bearer new-access-token'
            })
        });
    });

    test('refreshToken strips inherited bearer from queued noAuth requests but preserves explicit challenges', async () => {
        const { default: services } = await loadServices();
        const refreshDeferred: { resolve?: (value: unknown) => void } = {};
        mockAxiosPost.mockImplementationOnce(() => new Promise(resolve => {
            refreshDeferred.resolve = resolve;
        }));

        const refreshPromise = services.refreshToken();
        const queuedLogin = mockRequestInterceptor!({
            url: 'auth/login',
            noAuth: true,
            headers: {
                Authorization: 'Bearer stale-default-token',
                authorization: 'Bearer stale-default-token'
            }
        }) as Promise<Record<string, any>>;
        const queuedChallenge = mockRequestInterceptor!({
            url: '2fa/verify',
            noAuth: true,
            preserveExplicitAuthorization: true,
            headers: {
                Authorization: 'Bearer explicit-2fa-challenge'
            }
        }) as Promise<Record<string, any>>;

        refreshDeferred.resolve?.(apiResponse({
            newToken: 'new-access-token',
            refreshToken: 'new-refresh-token'
        }));

        await expect(refreshPromise).resolves.toBeDefined();
        await expect(queuedLogin).resolves.toMatchObject({
            headers: expect.not.objectContaining({
                Authorization: expect.anything(),
                authorization: expect.anything()
            })
        });
        await expect(queuedChallenge).resolves.toMatchObject({
            headers: expect.objectContaining({
                Authorization: 'Bearer explicit-2fa-challenge'
            })
        });
    });

    test('refreshToken rejects every blocked request when the refresh request fails', async () => {
        const { default: services } = await loadServices();
        const refreshDeferred: { reject?: (reason: unknown) => void } = {};
        mockAxiosPost.mockImplementationOnce(() => new Promise((_resolve, reject) => {
            refreshDeferred.reject = reject;
        }));

        const refreshPromise = services.refreshToken();
        const firstQueuedRequest = mockRequestInterceptor!({ url: 'accounts', headers: {} }) as Promise<Record<string, any>>;
        const secondQueuedRequest = mockRequestInterceptor!({ url: 'bills', headers: {} }) as Promise<Record<string, any>>;
        let firstOutcome = 'pending';
        let secondOutcome = 'pending';
        void firstQueuedRequest.then(
            () => { firstOutcome = 'resolved'; },
            () => { firstOutcome = 'rejected'; }
        );
        void secondQueuedRequest.then(
            () => { secondOutcome = 'resolved'; },
            () => { secondOutcome = 'rejected'; }
        );

        const networkError = new Error('refresh network timeout');
        refreshDeferred.reject?.(networkError);

        await expect(refreshPromise).rejects.toStrictEqual({
            message: 'Token refresh failed',
            route: 'tokens/refresh'
        });
        await Promise.resolve();
        expect(firstOutcome).toBe('rejected');
        expect(secondOutcome).toBe('rejected');
    });

    test('refreshToken logs only safe failure metadata when Axios error internals contain credentials', async () => {
        const { default: services } = await loadServices();
        const refreshDeferred: { reject?: (reason: unknown) => void } = {};
        mockAxiosPost.mockImplementationOnce(() => new Promise((_resolve, reject) => {
            refreshDeferred.reject = reject;
        }));
        const sentinel = 'SENTINEL-LONG-LIVED-REFRESH-CREDENTIAL';
        const hostileError = Object.assign(new Error(`refresh request failed: ${sentinel}`), {
            code: 'ERR_BAD_RESPONSE',
            config: {
                url: 'tokens/refresh',
                data: JSON.stringify({ refreshToken: sentinel })
            },
            request: {
                body: JSON.stringify({ refreshToken: sentinel })
            },
            response: {
                status: 503,
                data: {
                    refreshToken: sentinel,
                    requestBody: sentinel
                }
            }
        });

        const refreshPromise = services.refreshToken();
        const queuedRequest = mockRequestInterceptor!({ url: 'accounts', headers: {} }) as Promise<Record<string, any>>;
        let queuedOutcome = 'pending';
        let queuedFailure: unknown;
        void queuedRequest.then(
            () => { queuedOutcome = 'resolved'; },
            reason => {
                queuedOutcome = 'rejected';
                queuedFailure = reason;
            }
        );

        refreshDeferred.reject?.(hostileError);

        const consumerFailure = await refreshPromise.then(
            () => { throw new Error('refresh unexpectedly resolved'); },
            reason => reason
        );
        await Promise.resolve();
        expect(queuedOutcome).toBe('rejected');
        expect(queuedFailure).toBe(consumerFailure);
        expect(consumerFailure).toStrictEqual({
            message: 'Token refresh failed',
            code: 'ERR_BAD_RESPONSE',
            status: 503,
            route: 'tokens/refresh'
        });
        expect(Object.keys(consumerFailure as Record<string, unknown>).sort()).toStrictEqual([
            'code',
            'message',
            'route',
            'status'
        ]);
        expect(mockLoggerError).toHaveBeenCalledWith('[auth-refresh] Token refresh failed', {
            message: 'Token refresh failed',
            code: 'ERR_BAD_RESPONSE',
            status: 503,
            route: 'tokens/refresh'
        });

        const serializedLoggerCalls = JSON.stringify(mockLoggerError.mock.calls);
        const serializedConsumerFailure = JSON.stringify(consumerFailure);
        expect(serializedLoggerCalls).not.toContain(sentinel);
        expect(serializedConsumerFailure).not.toContain(sentinel);
        expect(serializedLoggerCalls).not.toContain('"refreshToken"');
        expect(serializedConsumerFailure).not.toContain('"refreshToken"');
        expect(serializedLoggerCalls).not.toContain('requestBody');
        expect(serializedLoggerCalls).not.toContain('"config"');
        expect(serializedLoggerCalls).not.toContain('"request"');
        expect(serializedLoggerCalls).not.toContain('"response"');
        expect(serializedConsumerFailure).not.toContain('"config"');
        expect(serializedConsumerFailure).not.toContain('"request"');
        expect(serializedConsumerFailure).not.toContain('"response"');
        expect(serializedConsumerFailure).not.toContain('"data"');
    });

    test.each([
        ['undefined', undefined],
        ['null', null],
        ['false', false]
    ] as Array<[string, undefined | null | false]>)('refreshToken rejects waiters when the network rejects with %s', async (_label, rejectionReason) => {
        const { default: services } = await loadServices();
        const refreshDeferred: { reject?: (reason?: unknown) => void } = {};
        mockAxiosPost.mockImplementationOnce(() => new Promise((_resolve, reject) => {
            refreshDeferred.reject = reject;
        }));

        const refreshPromise = services.refreshToken();
        const queuedRequest = mockRequestInterceptor!({ url: 'accounts', headers: {} }) as Promise<Record<string, any>>;
        let queuedOutcome = 'pending';
        let replayedAuthorization: string | null = null;
        void queuedRequest.then(
            config => {
                queuedOutcome = 'resolved';
                replayedAuthorization = config['headers']?.['Authorization'] ?? null;
            },
            () => { queuedOutcome = 'rejected'; }
        );

        refreshDeferred.reject?.(rejectionReason);

        const refreshOutcome = await refreshPromise.then(
            () => 'resolved',
            reason => {
                expect(reason).toStrictEqual({
                    message: 'Token refresh failed',
                    route: 'tokens/refresh'
                });
                return 'rejected';
            }
        );
        await Promise.resolve();

        expect(refreshOutcome).toBe('rejected');
        expect(queuedOutcome).toBe('rejected');
        expect(replayedAuthorization).toBeNull();
        expect(mockLoggerError).toHaveBeenCalledWith('[auth-refresh] Token refresh failed', {
            message: 'Token refresh failed',
            route: 'tokens/refresh'
        });

        const subsequentPublicRequest = mockRequestInterceptor!({ url: 'auth/login', headers: {} });
        expect(subsequentPublicRequest).not.toBeInstanceOf(Promise);
    });

    test('refreshToken rejects a malformed success envelope and settles blocked requests', async () => {
        const { default: services } = await loadServices();
        const refreshDeferred: { resolve?: (value: unknown) => void } = {};
        mockAxiosPost.mockImplementationOnce(() => new Promise(resolve => {
            refreshDeferred.resolve = resolve;
        }));

        const refreshPromise = services.refreshToken();
        const queuedRequest = mockRequestInterceptor!({ url: 'accounts', headers: {} }) as Promise<Record<string, any>>;
        let queuedOutcome = 'pending';
        void queuedRequest.then(
            () => { queuedOutcome = 'resolved'; },
            () => { queuedOutcome = 'rejected'; }
        );

        refreshDeferred.resolve?.(apiResponse({ refreshToken: 'rotated-without-access-token' }));

        await expect(refreshPromise).rejects.toMatchObject({
            message: 'Token refresh failed',
            route: 'tokens/refresh'
        });
        await Promise.resolve();
        expect(queuedOutcome).toBe('rejected');
        expect(mockUpdateCurrentToken).not.toHaveBeenCalled();
        expect(mockUpdateCurrentRefreshToken).not.toHaveBeenCalled();
    });

    test('refreshToken rejects nominal success without the rotated refresh credential', async () => {
        const { default: services } = await loadServices();
        const refreshDeferred: { resolve?: (value: unknown) => void } = {};
        mockAxiosPost.mockImplementationOnce(() => new Promise(resolve => {
            refreshDeferred.resolve = resolve;
        }));

        const refreshPromise = services.refreshToken();
        const queuedRequest = mockRequestInterceptor!({ url: 'accounts', headers: {} }) as Promise<Record<string, any>>;
        let queuedOutcome = 'pending';
        void queuedRequest.then(
            () => { queuedOutcome = 'resolved'; },
            () => { queuedOutcome = 'rejected'; }
        );

        refreshDeferred.resolve?.(apiResponse({ newToken: 'new-access-without-rotation' }));

        await expect(refreshPromise).rejects.toMatchObject({
            message: 'Token refresh failed',
            route: 'tokens/refresh'
        });
        await Promise.resolve();
        expect(queuedOutcome).toBe('rejected');
        expect(mockUpdateCurrentToken).not.toHaveBeenCalled();
        expect(mockUpdateCurrentRefreshToken).not.toHaveBeenCalled();
    });

    test('refreshToken rejects when either rotated credential cannot be read back from storage', async () => {
        const { default: services } = await loadServices();
        mockUpdateCurrentRefreshToken.mockImplementationOnce(() => undefined);
        mockAxiosPost.mockResolvedValueOnce(apiResponse({
            newToken: 'new-access-token',
            refreshToken: 'unstored-refresh-token'
        }));

        await expect(services.refreshToken()).rejects.toMatchObject({
            message: 'Token refresh failed',
            route: 'tokens/refresh'
        });

        mockUpdateCurrentToken.mockImplementationOnce(() => undefined);
        mockAxiosPost.mockResolvedValueOnce(apiResponse({
            newToken: 'unstored-access-token',
            refreshToken: 'rotated-before-access-persist-failure'
        }));

        await expect(services.refreshToken()).rejects.toMatchObject({
            message: 'Token refresh failed',
            route: 'tokens/refresh'
        });
    });

    test('refreshToken fails closed without a consumable refresh credential', async () => {
        const { default: services } = await loadServices();
        mockRefreshToken = '';

        await expect(services.refreshToken()).rejects.toMatchObject({
            message: 'No refresh token available',
            noRefreshToken: true
        });
        expect(mockAxiosPost).not.toHaveBeenCalled();
    });

    test('cancelRequest makes matching successful responses reject as canceled', async () => {
        const { default: services } = await loadServices();
        expect(mockResponseInterceptor).not.toBeNull();

        services.cancelRequest('cancel-1');

        await expect(mockResponseInterceptor!({
            config: {
                url: 'bills',
                cancelableUuid: 'cancel-1'
            }
        })).rejects.toStrictEqual({ canceled: true });
    });

    test('import preview facade keeps session encoding, query params and confirm payload stable', async () => {
        const { default: services } = await loadServices();
        mockAxiosGet.mockResolvedValueOnce({
            data: {
                success: true,
                data: {
                    items: []
                }
            }
        });
        mockAxiosPost.mockResolvedValueOnce({
            data: {
                success: true,
                data: {
                    imported: 2
                }
            }
        });

        await services.getImportPreviewPage({
            sessionId: 'session/a',
            page: 2,
            pageSize: 50,
            selectedOnly: true,
            previewIds: [7, 9],
            signal: 'transfer'
        });
        const historyRewriteAcknowledgement = {
            acknowledged: true as const,
            selected_preview_ids: [7],
            operations: [{
                preview_id: 7,
                operation_id: 'rewrite-7',
                planned_operation: 'replace_history_bill',
                history_bill_id: 70,
                history_bill_version: 2,
                acknowledgement_token: 'ack-7'
            }],
            selection_scope: {
                mode: 'visible-preview',
                selected_count: 1,
                history_rewrite_count: 1
            }
        };
        await services.confirmImportPreview({
            sessionId: 'session/a',
            expectedSessionVersion: 11,
            previewUpdates: [{ id: 7, expected_row_version: 3, type: '支出' }],
            preserveUnpatchedSelection: true,
            historyRewriteAcknowledgement
        });

        expect(mockAxiosGet).toHaveBeenCalledWith('bills/import/v2/preview/session%2Fa', {
            params: {
                page: 2,
                page_size: 50,
                selected_only: true,
                preview_ids: '7,9',
                signal: 'transfer'
            }
        });
        expect(mockAxiosPost).toHaveBeenCalledWith(
            'bills/import/v2/confirm',
            {
                session_id: 'session/a',
                expected_session_version: 11,
                preserve_unpatched_selection: true,
                preview_updates: [{ id: 7, expected_row_version: 3, type: '支出' }],
                history_rewrite_acknowledgement: historyRewriteAcknowledgement
            },
            expect.objectContaining({
                timeout: expect.any(Number)
            })
        );
    });

    test('learning rule facade keeps list, toggle, update, and delete on the live route contract', async () => {
        const { default: services } = await loadServices();
        const updatedRule = {
            id: 42,
            matchValue: 'coffee',
            learnedType: 'Expense',
            learnedCategoryId: 7,
            enabled: true
        };

        mockAxiosGet.mockResolvedValueOnce({
            data: {
                success: true,
                data: {
                    items: [],
                    total: 0,
                    limit: 25,
                    offset: 50
                }
            }
        });
        mockAxiosPut
            .mockResolvedValueOnce({ data: { success: true, data: updatedRule } })
            .mockResolvedValueOnce({ data: { success: true, data: updatedRule } });
        mockAxiosDelete.mockResolvedValueOnce({ data: { success: true, data: updatedRule } });

        const listResponse = await services.getLearningRules({
            enabledOnly: true,
            limit: 25,
            offset: 50
        });
        await services.toggleLearningRule({ ruleId: 42, enabled: false });
        const updateResponse = await services.updateLearningRule({
            ruleId: 42,
            matchValue: 'coffee',
            learnedType: 'Expense',
            learnedCategoryId: 7,
            enabled: true
        });
        await services.deleteLearningRule({ ruleId: 42 });

        expect(mockAxiosGet).toHaveBeenCalledWith('learning/rules', {
            params: {
                enabled_only: true,
                limit: 25,
                offset: 50
            }
        });
        expect(listResponse.data.result).toStrictEqual({
            items: [],
            total: 0,
            limit: 25,
            offset: 50
        });
        expect(mockAxiosPut).toHaveBeenNthCalledWith(1, 'learning/rules/42/toggle', {
            enabled: false
        });
        expect(mockAxiosPut).toHaveBeenNthCalledWith(2, 'learning/rules/42', {
            matchValue: 'coffee',
            learnedType: 'Expense',
            learnedCategoryId: 7,
            enabled: true
        });
        expect(updateResponse.data.result).toStrictEqual(updatedRule);
        expect(mockAxiosDelete).toHaveBeenCalledWith('learning/rules/42');
        expect(mockAxiosPut.mock.calls.some(([url]) => (
            typeof url === 'string' && url.startsWith('bills/import/learning-rules')
        ))).toBe(false);
    });
});
