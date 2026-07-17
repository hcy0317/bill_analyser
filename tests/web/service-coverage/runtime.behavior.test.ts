import { beforeEach, describe, expect, jest, test } from '@jest/globals';

type RequestFulfilled = (config: Record<string, any>) => Record<string, any> | Promise<Record<string, any>>;
type RequestRejected = (error: unknown) => Promise<never>;
type ResponseFulfilled = (response: Record<string, any>) => Record<string, any> | Promise<Record<string, any>>;
type ResponseRejected = (error: any) => Promise<never>;

let requestFulfilled: RequestFulfilled | null = null;
let requestRejected: RequestRejected | null = null;
let responseFulfilled: ResponseFulfilled | null = null;
let responseRejected: ResponseRejected | null = null;

const axiosGet = jest.fn<(...args: any[]) => Promise<any>>();
const axiosPost = jest.fn<(...args: any[]) => Promise<any>>();
const axiosPut = jest.fn<(...args: any[]) => Promise<any>>();
const axiosDelete = jest.fn<(...args: any[]) => Promise<any>>();
const axiosPostForm = jest.fn<(...args: any[]) => Promise<any>>();
const requestUse = jest.fn((fulfilled: RequestFulfilled, rejected: RequestRejected) => {
    requestFulfilled = fulfilled;
    requestRejected = rejected;
    return 1;
});
const responseUse = jest.fn((fulfilled: ResponseFulfilled, rejected: ResponseRejected) => {
    responseFulfilled = fulfilled;
    responseRejected = rejected;
    return 1;
});

const axiosMock = {
    defaults: {
        baseURL: '',
        timeout: 0,
        headers: { common: {} as Record<string, string> }
    },
    interceptors: {
        request: { use: requestUse },
        response: { use: responseUse }
    },
    get: axiosGet,
    post: axiosPost,
    put: axiosPut,
    delete: axiosDelete,
    postForm: axiosPostForm
};

const getAccessToken = jest.fn<() => string>();
const getRefreshToken = jest.fn<() => string>();
const updateAccessToken = jest.fn<(token: string) => void>();
const updateRefreshToken = jest.fn<(token: string) => void>();
const clearCredentials = jest.fn();
const reloadPage = jest.fn();
const loggerWarn = jest.fn();
const loggerError = jest.fn();
let accessToken = '';
let refreshToken = 'refresh-token';
let appLockEnabled = false;
let apiErrorMessage = '';

jest.mock('axios', () => ({
    __esModule: true,
    default: axiosMock,
    AxiosHeaders: class {}
}));

jest.mock('@/lib/userstate.ts', () => ({
    __esModule: true,
    getCurrentToken: () => getAccessToken(),
    getCurrentRefreshToken: () => getRefreshToken(),
    updateCurrentToken: (token: string) => updateAccessToken(token),
    updateCurrentRefreshToken: (token: string) => updateRefreshToken(token),
    clearCurrentTokenAndUserInfo: (...args: any[]) => clearCredentials(...args)
}));

jest.mock('@/lib/settings.ts', () => ({
    __esModule: true,
    isEnableApplicationLock: () => appLockEnabled
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
        warn: (...args: any[]) => loggerWarn(...args),
        error: (...args: any[]) => loggerError(...args)
    }
}));

jest.mock('@/lib/api_error.ts', () => ({
    __esModule: true,
    getApiErrorMessage: () => apiErrorMessage
}));

function makeStorage(): Storage {
    const values = new Map<string, string>();
    return {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => { values.set(key, value); },
        removeItem: (key: string) => { values.delete(key); },
        clear: () => values.clear(),
        key: (index: number) => Array.from(values.keys())[index] ?? null,
        get length() { return values.size; }
    } as Storage;
}

async function loadServices(): Promise<typeof import('@/lib/services.ts')> {
    jest.resetModules();
    requestFulfilled = null;
    requestRejected = null;
    responseFulfilled = null;
    responseRejected = null;
    return import('@/lib/services.ts');
}

beforeEach(() => {
    accessToken = '';
    refreshToken = 'refresh-token';
    appLockEnabled = false;
    apiErrorMessage = '';
    axiosMock.defaults.headers.common = {};
    axiosGet.mockReset().mockResolvedValue({ data: { success: true, result: {} } });
    axiosPost.mockReset().mockResolvedValue({ data: { success: true, result: {} } });
    axiosPut.mockReset().mockResolvedValue({ data: { success: true, result: {} } });
    axiosDelete.mockReset().mockResolvedValue({ data: { success: true, result: {} } });
    axiosPostForm.mockReset().mockResolvedValue({ data: { success: true, result: {} } });
    getAccessToken.mockReset().mockImplementation(() => accessToken);
    getRefreshToken.mockReset().mockImplementation(() => refreshToken);
    updateAccessToken.mockReset().mockImplementation(token => { accessToken = token; });
    updateRefreshToken.mockReset().mockImplementation(token => { refreshToken = token; });
    clearCredentials.mockReset();
    reloadPage.mockReset();
    loggerWarn.mockReset();
    loggerError.mockReset();

    (globalThis as unknown as { localStorage: Storage }).localStorage = makeStorage();
    (globalThis as unknown as { sessionStorage: Storage }).sessionStorage = makeStorage();
    (globalThis as unknown as { window: { location: { hash: string; origin: string; pathname: string } } }).window = {
        location: { hash: '', origin: 'https://app.example', pathname: '/' }
    };
    (globalThis as unknown as { location: { reload: () => void } }).location = { reload: reloadPage };
});

describe('services request interceptor behavior', () => {
    test('initializes and explicitly updates default authorization in both directions', async () => {
        accessToken = 'initial-token';
        const module = await loadServices();

        module.initializeAxiosAuth();
        expect(axiosMock.defaults.headers.common['Authorization']).toBe('Bearer initial-token');

        module.updateAxiosAuthorizationHeader('next-token');
        expect(axiosMock.defaults.headers.common['Authorization']).toBe('Bearer next-token');

        module.updateAxiosAuthorizationHeader('');
        expect(axiosMock.defaults.headers.common['Authorization']).toBeUndefined();
    });

    test('initializes an empty session and creates request headers with storage and app-lock telemetry', async () => {
        const module = await loadServices();
        module.initializeAxiosAuth();
        expect(axiosMock.defaults.headers.common['Authorization']).toBeUndefined();

        accessToken = 'access-token';
        appLockEnabled = true;
        localStorage.setItem('ebk_user_token', 'stored');
        sessionStorage.setItem('ebk_user_session_token', 'stored');
        const config = await requestFulfilled!({ url: 'profile' });

        expect(config['headers']).toEqual(expect.objectContaining({
            Authorization: 'Bearer access-token',
            authorization: 'Bearer access-token',
            'X-Timezone-Offset': 480
        }));
        expect(loggerWarn).toHaveBeenCalledWith(expect.stringContaining('config.headers is undefined'));
    });

    test('supports AxiosHeaders setters, setter failures, clear failures, and fail-closed header proxies', async () => {
        await loadServices();
        accessToken = 'access-token';

        const storedHeaders: Record<string, string> = {};
        const axiosHeaders = {
            set: (key: string, value: string) => { storedHeaders[key] = value; },
            get: (key: string) => storedHeaders[key]
        };
        await requestFulfilled!({ url: 'profile', headers: axiosHeaders });
        expect(storedHeaders['Authorization']).toBe('Bearer access-token');

        const throwingSetHeaders: Record<string, any> = {
            set: () => { throw new Error('set failed'); }
        };
        await requestFulfilled!({ url: 'profile', headers: throwingSetHeaders });
        expect(loggerWarn).toHaveBeenCalledWith('[setAuthorizationHeader] headers.set() failed', expect.any(Error));

        accessToken = '';
        const throwingDeleteHeaders: Record<string, any> = {
            Authorization: 'stale',
            delete: () => { throw new Error('delete failed'); }
        };
        await requestFulfilled!({ url: 'auth/login', headers: throwingDeleteHeaders });
        expect(throwingDeleteHeaders['Authorization']).toBeUndefined();
        expect(loggerWarn).toHaveBeenCalledWith('[clearAuthorizationHeader] headers.delete() failed');

        accessToken = 'access-token';
        const ignoredHeaders = new Proxy<Record<string, any>>({}, {
            get: () => undefined,
            set: () => true,
            ownKeys: () => [],
            getOwnPropertyDescriptor: () => undefined
        });
        const ignoredResult = await requestFulfilled!({ url: 'profile', headers: ignoredHeaders });
        expect(ignoredResult['headers']).toBe(ignoredHeaders);
        expect(loggerError).toHaveBeenCalledWith(expect.stringContaining('FAILED to set Authorization'));
        expect(loggerError).toHaveBeenCalledWith(expect.stringContaining('CRITICAL: Authorization not found'));

        const assignmentFailureHeaders = new Proxy<Record<string, any>>({}, {
            get: () => undefined,
            set: () => { throw new Error('assignment failed'); }
        });
        expect(() => requestFulfilled!({ url: 'profile', headers: assignmentFailureHeaders })).toThrow('assignment failed');
        expect(loggerWarn).toHaveBeenCalledWith('[setAuthorizationHeader] Direct assignment failed', expect.any(Error));
    });

    test('keeps explicit public authorization, strips stale auth, and rejects request-stage failures', async () => {
        await loadServices();
        accessToken = 'access-token';
        const explicit = await requestFulfilled!({
            url: '2fa/verify?mode=challenge',
            noAuth: true,
            preserveExplicitAuthorization: true,
            headers: { Authorization: 'Bearer challenge-token' }
        });
        expect(explicit['headers']['Authorization']).toBe('Bearer challenge-token');

        accessToken = '';
        const anonymous = await requestFulfilled!({
            url: '?probe=1',
            headers: { Authorization: 'Bearer stale', authorization: 'Bearer stale' }
        });
        expect(anonymous['headers']['Authorization']).toBeUndefined();
        expect(anonymous['headers']['authorization']).toBeUndefined();

        const requestFailure = new Error('request setup failed');
        await expect(requestRejected!(requestFailure)).rejects.toBe(requestFailure);
    });

    test('resumes a blocked authenticated request and reports a missing latest access token', async () => {
        let resolveRefresh!: (value: any) => void;
        axiosPost.mockReturnValueOnce(new Promise(resolve => { resolveRefresh = resolve; }));
        const module = await loadServices();
        updateAccessToken.mockImplementation(() => undefined);

        const refreshPromise = module.default.refreshToken();
        const blockedRequest = requestFulfilled!({ url: 'profile' });
        getAccessToken
            .mockReturnValueOnce('new-access-token')
            .mockReturnValueOnce('');
        resolveRefresh({
            data: {
                success: true,
                result: { newToken: 'new-access-token', refreshToken: 'next-refresh-token' }
            }
        });

        await expect(refreshPromise).resolves.toMatchObject({ data: { success: true } });
        await expect(blockedRequest).resolves.toEqual(expect.objectContaining({
            url: 'profile', headers: expect.any(Object)
        }));
        expect(loggerError).toHaveBeenCalledWith(expect.stringContaining('no token in localStorage'));
    });

    test('rejects missing refresh credentials and shares one valid in-flight refresh request', async () => {
        refreshToken = '';
        const module = await loadServices();
        await expect(module.default.refreshToken()).rejects.toMatchObject({
            message: 'No refresh token available', noRefreshToken: true
        });

        refreshToken = 'refresh-token';
        let resolveRefresh!: (value: any) => void;
        axiosPost.mockReturnValueOnce(new Promise(resolve => { resolveRefresh = resolve; }));
        const first = module.default.refreshToken();
        const second = module.default.refreshToken();
        expect(second).toBe(first);

        resolveRefresh({
            data: {
                success: true,
                result: { newToken: 'new-access-token', refreshToken: 'new-refresh-token' }
            }
        });
        await expect(first).resolves.toMatchObject({ data: { success: true } });
        expect(accessToken).toBe('new-access-token');
        expect(refreshToken).toBe('new-refresh-token');
        expect(axiosPost).toHaveBeenCalledTimes(1);
    });
});

describe('services response interceptor behavior', () => {
    test('returns normal responses, reads AxiosHeaders authorization, and cancels marked successes', async () => {
        const module = await loadServices();
        const response = {
            config: {
                url: 'profile',
                headers: { get: (key: string) => key === 'Authorization' ? 'Bearer token' : undefined }
            },
            data: { success: true }
        };
        expect(responseFulfilled!(response)).toBe(response);

        expect(responseFulfilled!({ config: {}, data: {} })).toEqual({ config: {}, data: {} });

        module.default.cancelRequest('cancel-success');
        await expect(responseFulfilled!({
            config: { url: 'slow', cancelableUuid: 'cancel-success' }
        })).rejects.toEqual({ canceled: true });
        expect(responseFulfilled!({
            config: { url: 'slow', cancelableUuid: 'cancel-success' }
        })).toEqual(expect.objectContaining({ config: expect.any(Object) }));
    });

    test('handles malformed error shapes without inventing token-operation details', async () => {
        await loadServices();
        const primitiveFailure = 'network failed';
        await expect(responseRejected!(primitiveFailure)).rejects.toBe(primitiveFailure);

        let missingConfigReads = 0;
        const missingConfig = {
            get response() {
                missingConfigReads += 1;
                return missingConfigReads === 2
                    ? { status: 500 }
                    : { status: 500, config: { url: 'profile', headers: {} }, data: {} };
            }
        };
        await expect(responseRejected!(missingConfig)).rejects.toBe(missingConfig);

        const nonStringUrl = { response: { status: 500, config: { url: 42, headers: {} } } };
        await expect(responseRejected!(nonStringUrl)).rejects.toBe(nonStringUrl);

        let urlReads = 0;
        const transientConfig = new Proxy<Record<string, any>>({ headers: {} }, {
            get: (target, key) => {
                if (key === 'url' && urlReads++ === 0) throw new Error('url getter failed');
                return Reflect.get(target, key);
            }
        });
        const throwingUrl = { response: { status: 500, config: transientConfig } };
        await expect(responseRejected!(throwingUrl)).rejects.toBe(throwingUrl);

        let responseReads = 0;
        const transientResponse = {
            get response() {
                responseReads += 1;
                if (responseReads === 1) return { status: 500, config: {} };
                if (responseReads === 2) return null;
                return { status: 500, config: {} };
            }
        };
        await expect(responseRejected!(transientResponse)).rejects.toBe(transientResponse);
    });

    test('redacts refresh failures even when error accessors throw or status is invalid', async () => {
        const module = await loadServices();
        const throwingFailure = new Proxy<Record<string, unknown>>({}, {
            get: () => { throw new Error('secret getter failed'); }
        });
        axiosPost.mockRejectedValueOnce(throwingFailure);
        await expect(module.default.refreshToken()).rejects.toEqual({
            message: 'Token refresh failed', route: 'tokens/refresh'
        });

        axiosPost.mockRejectedValueOnce({ code: 'UNKNOWN', response: { status: 600 } });
        await expect(module.default.refreshToken()).rejects.toEqual({
            message: 'Token refresh failed', route: 'tokens/refresh'
        });
    });

    test('keeps only allowlisted code and valid status for refresh and revoke failures', async () => {
        const module = await loadServices();
        axiosPost.mockRejectedValueOnce({ code: 'ERR_NETWORK', response: { status: 503 } });
        await expect(module.default.refreshToken()).rejects.toEqual({
            message: 'Token refresh failed',
            route: 'tokens/refresh',
            code: 'ERR_NETWORK',
            status: 503
        });

        const revokeFailure = {
            code: 'ERR_BAD_RESPONSE',
            response: {
                status: 403,
                config: {
                    url: 'https://app.example/desktop/api/tokens/token-1?audit=1',
                    method: 'DELETE',
                    headers: {}
                }
            }
        };
        await expect(responseRejected!(revokeFailure)).rejects.toBe(revokeFailure);
        expect(loggerError).toHaveBeenCalledWith('[auth-token] Token operation failed', {
            message: 'Token revoke failed',
            route: 'tokens/:id',
            code: 'ERR_BAD_RESPONSE',
            status: 403
        });
    });

    test('logs ordinary failures, honors ignoreError, and cancels marked errors', async () => {
        const module = await loadServices();
        apiErrorMessage = 'server detail';
        const ordinaryFailure = {
            response: {
                status: 500,
                config: {
                    url: 'profile',
                    headers: { get: () => 'Bearer token' }
                },
                data: {}
            }
        };
        await expect(responseRejected!(ordinaryFailure)).rejects.toBe(ordinaryFailure);
        expect(loggerError).toHaveBeenCalledWith(
            expect.stringContaining('500 profile'),
            expect.objectContaining({ responseMessage: 'server detail' })
        );

        const ignoredFailure = {
            response: { status: 401, config: { url: 'profile', ignoreError: true, headers: {} } }
        };
        await expect(responseRejected!(ignoredFailure)).rejects.toBe(ignoredFailure);

        module.default.cancelRequest('cancel-error');
        const canceledFailure = {
            response: { status: 500, config: { url: 'slow', cancelableUuid: 'cancel-error', headers: {} } }
        };
        await expect(responseRejected!(canceledFailure)).rejects.toEqual({ canceled: true });
    });

    test('processes 401 outside login, suppresses reload on login, and handles every legacy auth code', async () => {
        await loadServices();
        const unauthorized = {
            response: { status: 401, config: { url: 'profile', headers: {} }, data: {} }
        };
        await expect(responseRejected!(unauthorized)).rejects.toEqual({ processed: true });
        expect(clearCredentials).toHaveBeenCalledWith(false);
        expect(reloadPage).toHaveBeenCalledTimes(1);

        clearCredentials.mockClear();
        reloadPage.mockClear();
        window.location.hash = '#/login';
        await expect(responseRejected!(unauthorized)).rejects.toEqual({ processed: true });
        expect(clearCredentials).not.toHaveBeenCalled();
        expect(reloadPage).not.toHaveBeenCalled();

        window.location.hash = '';
        for (const errorCode of [202001, 202002, 202003, 202004, 202005, 202006, 202012]) {
            const legacyFailure = {
                response: {
                    status: 400,
                    config: { url: 'profile', headers: {} },
                    data: { errorCode }
                }
            };
            await expect(responseRejected!(legacyFailure)).rejects.toEqual({ processed: true });
        }
        const unrelatedFailure = {
            response: {
                status: 400,
                config: { url: 'profile', headers: {} },
                data: { errorCode: 299999 }
            }
        };
        await expect(responseRejected!(unrelatedFailure)).rejects.toBe(unrelatedFailure);
    });
});
