import { beforeEach, describe, expect, jest, test } from '@jest/globals';

type RequestInterceptor = (config: Record<string, any>) => Record<string, any> | Promise<Record<string, any>>;
type ResponseInterceptor = (response: Record<string, any>) => Record<string, any> | Promise<Record<string, any>>;

const mockAxiosGet = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAxiosPost = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAxiosPut = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAxiosDelete = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAxiosPostForm = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockRequestUse = jest.fn((onFulfilled: RequestInterceptor) => {
    mockRequestInterceptor = onFulfilled;
    return 1;
});
const mockResponseUse = jest.fn((onFulfilled: ResponseInterceptor) => {
    mockResponseInterceptor = onFulfilled;
    return 1;
});

let mockRequestInterceptor: RequestInterceptor | null = null;
let mockResponseInterceptor: ResponseInterceptor | null = null;
let mockAccessToken = '';
let mockRefreshToken = '';

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
        error: jest.fn()
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

    test('refreshToken unblocks queued requests with the latest stored access token', async () => {
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
        const queuedRequest = mockRequestInterceptor!({
            url: 'accounts',
            headers: {}
        }) as Promise<Record<string, any>>;

        let queuedResolved = false;
        queuedRequest.then(() => {
            queuedResolved = true;
        });
        await Promise.resolve();
        expect(queuedResolved).toBe(false);

        mockAccessToken = 'new-access-token';
        const completeRefresh = refreshDeferred.resolve;
        if (!completeRefresh) {
            throw new Error('refresh promise resolver was not captured');
        }
        completeRefresh(apiResponse({ newToken: 'new-access-token' }));

        await expect(refreshPromise).resolves.toMatchObject({
            data: {
                result: {
                    newToken: 'new-access-token'
                }
            }
        });
        await expect(queuedRequest).resolves.toMatchObject({
            headers: expect.objectContaining({
                Authorization: 'Bearer new-access-token'
            })
        });
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
        await services.confirmImportPreview({
            sessionId: 'session/a',
            previewUpdates: [{ id: 7, type: '支出' }],
            preserveUnpatchedSelection: true,
            historyRewriteAcknowledgement: { acknowledged: true }
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
                preserve_unpatched_selection: true,
                preview_updates: [{ id: 7, type: '支出' }],
                history_rewrite_acknowledgement: { acknowledged: true }
            },
            expect.objectContaining({
                timeout: expect.any(Number)
            })
        );
    });
});
