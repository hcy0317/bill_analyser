import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

type ApiResponse<T> = Promise<{ data?: { success: boolean; result?: T } }>;
type ServiceMock = jest.Mock<(...args: any[]) => ApiResponse<any>>;

function apiMock(): ServiceMock {
    return jest.fn<(...args: any[]) => ApiResponse<any>>();
}

function stringMock(): jest.Mock<(...args: any[]) => string> {
    return jest.fn<(...args: any[]) => string>();
}

const mockSetCloudSettings = jest.fn();
const mockSetApplicationLock = jest.fn();
const mockSetApplicationLockWebAuthn = jest.fn();
const mockResetUser = jest.fn();
const mockStoreUser = jest.fn();
const mockResetAccounts = jest.fn();
const mockInvalidateAccounts = jest.fn();
const mockResetCategories = jest.fn();
const mockInvalidateCategories = jest.fn();
const mockResetTags = jest.fn();
const mockInvalidateTags = jest.fn();
const mockResetTemplates = jest.fn();
const mockResetTransactions = jest.fn();
const mockResetOverview = jest.fn();
const mockInvalidateOverview = jest.fn();
const mockResetStatistics = jest.fn();
const mockInvalidateStatistics = jest.fn();
const mockResetRates = jest.fn();

const mockSettingsStore = {
    appSettings: { applicationLock: false },
    setApplicationSettingsFromCloudSettings: mockSetCloudSettings,
    setEnableApplicationLock: mockSetApplicationLock,
    setEnableApplicationLockWebAuthn: mockSetApplicationLockWebAuthn
};
const mockUserStore = {
    currentUserDefaultCurrency: 'CNY',
    resetUserBasicInfo: mockResetUser,
    storeUserBasicInfo: mockStoreUser
};
const mockAccountsStore = {
    accountListStateInvalid: false,
    resetAccounts: mockResetAccounts,
    updateAccountListInvalidState: mockInvalidateAccounts
};
const mockCategoriesStore = {
    transactionCategoryListStateInvalid: false,
    resetTransactionCategories: mockResetCategories,
    updateTransactionCategoryListInvalidState: mockInvalidateCategories
};
const mockTagsStore = {
    transactionTagListStateInvalid: false,
    resetTransactionTags: mockResetTags,
    updateTransactionTagListInvalidState: mockInvalidateTags
};
const mockTemplatesStore = { resetTransactionTemplates: mockResetTemplates };
const mockTransactionsStore = { resetTransactions: mockResetTransactions };
const mockOverviewStore = {
    transactionOverviewStateInvalid: false,
    resetTransactionOverview: mockResetOverview,
    updateTransactionOverviewInvalidState: mockInvalidateOverview
};
const mockStatisticsStore = {
    transactionStatisticsStateInvalid: false,
    resetTransactionStatistics: mockResetStatistics,
    updateTransactionStatisticsInvalidState: mockInvalidateStatistics
};
const mockRatesStore = { resetLatestExchangeRates: mockResetRates };

let mockHasAppLockState = false;
let mockAppLockState: { username: string } | null = null;
let mockCurrentToken: string | null = null;
const mockUpdateToken = jest.fn();
const mockUpdateRefreshToken = jest.fn();
const mockClearWebAuthn = jest.fn();
const mockClearSessionToken = jest.fn();
const mockClearTokenAndUser = jest.fn();
const mockUpdateApplicationSetting = jest.fn();

const mockServices = {
    authorize: apiMock(),
    authorize2FA: apiMock(),
    authorize2FAByBackupCode: apiMock(),
    authorizeOAuth2: apiMock(),
    register: apiMock(),
    logout: apiMock(),
    verifyEmail: apiMock(),
    resendVerifyEmailByUnloginUser: apiMock(),
    requestResetPassword: apiMock(),
    resetPassword: apiMock(),
    updateProfile: apiMock(),
    resendVerifyEmailByLoginedUser: apiMock(),
    clearAllTransactionsOfAccount: apiMock(),
    clearAllTransactions: apiMock(),
    clearAllData: apiMock(),
    generateOAuth2LoginUrl: stringMock(),
    generateOAuth2LinkUrl: stringMock()
};

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoriesStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagsStore }));
jest.mock('@/stores/transactionTemplate.ts', () => ({ useTransactionTemplatesStore: () => mockTemplatesStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockOverviewStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/stores/exchangeRates.ts', () => ({ useExchangeRatesStore: () => mockRatesStore }));

jest.mock('@/lib/userstate.ts', () => ({
    hasUserAppLockState: () => mockHasAppLockState,
    getUserAppLockState: () => mockAppLockState,
    getCurrentToken: () => mockCurrentToken,
    updateCurrentToken: (token: string) => mockUpdateToken(token),
    updateCurrentRefreshToken: (token: string) => mockUpdateRefreshToken(token),
    clearWebAuthnConfig: () => mockClearWebAuthn(),
    clearCurrentSessionToken: () => mockClearSessionToken(),
    clearCurrentTokenAndUserInfo: (includeUserInfo?: boolean) => mockClearTokenAndUser(includeUserInfo)
}));

jest.mock('@/lib/settings.ts', () => ({
    updateApplicationSettingsValue: (key: string, value: unknown) => mockUpdateApplicationSetting(key, value)
}));

jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

import { useRootStore } from '@/stores/index.ts';

function ok<T>(result: T): ApiResponse<T> {
    return Promise.resolve({ data: { success: true, result } });
}

function invalid(): ApiResponse<never> {
    return Promise.resolve({ data: { success: false } });
}

function responseError(): { response: { data: { message: string } } } {
    return { response: { data: { message: 'server detail' } } };
}

function authResult(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        token: 'access-token',
        refreshToken: 'refresh-token',
        user: { username: 'alice', defaultCurrency: 'CNY' },
        applicationCloudSettings: [
            { settingKey: 'applicationLock', settingValue: true },
            { settingKey: 'applicationLockWebAuthn', settingValue: true },
            { settingKey: 'theme', settingValue: 'dark' }
        ],
        ...overrides
    };
}

function createStorage(): Storage {
    const values = new Map<string, string>();
    return {
        get length() { return values.size; },
        clear: () => values.clear(),
        getItem: key => values.get(key) ?? null,
        key: index => Array.from(values.keys())[index] ?? null,
        removeItem: key => { values.delete(key); },
        setItem: (key, value) => { values.set(key, String(value)); }
    };
}

function resetDomainFlags(): void {
    mockSettingsStore.appSettings.applicationLock = false;
    mockUserStore.currentUserDefaultCurrency = 'CNY';
    mockAccountsStore.accountListStateInvalid = false;
    mockCategoriesStore.transactionCategoryListStateInvalid = false;
    mockTagsStore.transactionTagListStateInvalid = false;
    mockOverviewStore.transactionOverviewStateInvalid = false;
    mockStatisticsStore.transactionStatisticsStateInvalid = false;
    mockHasAppLockState = false;
    mockAppLockState = null;
    mockCurrentToken = null;
}

beforeEach(() => {
    setActivePinia(createPinia());
    jest.clearAllMocks();
    resetDomainFlags();
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: createStorage() });
    Object.defineProperty(globalThis, 'sessionStorage', { configurable: true, value: createStorage() });
    mockServices.generateOAuth2LoginUrl.mockReturnValue('/oauth-login');
    mockServices.generateOAuth2LinkUrl.mockReturnValue('/oauth-link');
});

afterEach(() => {
    jest.useRealTimers();
});

describe('root authentication behavior coverage', () => {
    test('exports notification and OAuth URL behavior', () => {
        const store = useRootStore();
        store.setNotificationContent('hello');
        expect(store.currentNotification).toBe('hello');
        expect(store.generateOAuth2LoginUrl('desktop', 'session')).toBe('/oauth-login');
        expect(store.generateOAuth2LinkUrl('mobile', 'session')).toBe('/oauth-link');
        expect(mockServices.generateOAuth2LoginUrl).toHaveBeenCalledWith('desktop', 'session');
        expect(mockServices.generateOAuth2LinkUrl).toHaveBeenCalledWith('mobile', 'session');
    });

    test('authorizes unlocked user, filters cloud lock settings and verifies stored token later', async () => {
        jest.useFakeTimers();
        mockCurrentToken = 'access-token';
        mockServices.authorize.mockReturnValue(ok(authResult()));
        const store = useRootStore();
        await expect(store.authorize({ username: 'alice', password: 'secret' } as any)).resolves.toMatchObject({ token: 'access-token' });
        expect(mockUpdateToken).toHaveBeenCalledWith('access-token');
        expect(mockUpdateRefreshToken).toHaveBeenCalledWith('refresh-token');
        expect(mockSetCloudSettings).toHaveBeenCalledWith([
            { settingKey: 'theme', settingValue: 'dark' }
        ]);
        expect(mockStoreUser).toHaveBeenCalledWith(expect.objectContaining({ username: 'alice' }));
        expect(localStorage.getItem('ebk_last_login_time')).not.toBeNull();
        jest.runOnlyPendingTimers();
    });

    test('returns 2FA challenge without changing local session', async () => {
        mockServices.authorize.mockReturnValue(ok(authResult({ need2FA: true })));
        await expect(useRootStore().authorize({} as any)).resolves.toMatchObject({ need2FA: true });
        expect(mockUpdateToken).not.toHaveBeenCalled();
        expect(mockSetCloudSettings).not.toHaveBeenCalled();
    });

    test('clears mismatched AppLock state after preserving the newly issued token', async () => {
        mockSettingsStore.appSettings.applicationLock = true;
        mockHasAppLockState = true;
        mockAppLockState = { username: 'bob' };
        sessionStorage.setItem('ebk_user_app_lock_state', 'locked');
        mockServices.authorize.mockReturnValue(ok(authResult()));

        await useRootStore().authorize({} as any);

        expect(mockUpdateToken).toHaveBeenCalledTimes(1);
        expect(mockUpdateRefreshToken).toHaveBeenCalledTimes(1);
        expect(sessionStorage.getItem('ebk_user_app_lock_state')).toBeNull();
        expect(mockUpdateApplicationSetting).toHaveBeenCalledWith('applicationLock', false);
        expect(mockUpdateApplicationSetting).toHaveBeenCalledWith('applicationLockWebAuthn', false);
        expect(mockSetApplicationLock).toHaveBeenCalledWith(false);
        expect(mockSetApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(mockClearWebAuthn).toHaveBeenCalled();
    });

    test('keeps matching AppLock user and also accepts auth payloads without optional fields', async () => {
        mockSettingsStore.appSettings.applicationLock = true;
        mockHasAppLockState = true;
        mockAppLockState = { username: 'alice' };
        localStorage.setItem('ebk_user_token', 'eyJexisting');
        mockServices.authorize.mockReturnValue(ok(authResult({
            token: 'eyJnew', refreshToken: null, applicationCloudSettings: null
        })));
        await useRootStore().authorize({} as any);
        expect(mockClearWebAuthn).not.toHaveBeenCalled();
        expect(mockSetCloudSettings).toHaveBeenCalledWith(null);
        expect(mockUpdateToken).toHaveBeenCalledWith('eyJnew');
        expect(mockUpdateRefreshToken).not.toHaveBeenCalled();
        expect(mockStoreUser).toHaveBeenCalled();

        jest.clearAllMocks();
        mockSettingsStore.appSettings.applicationLock = false;
        mockHasAppLockState = false;
        mockAppLockState = null;
        mockServices.authorize.mockReturnValueOnce(ok(authResult({
            token: 'plain-token', refreshToken: null, applicationCloudSettings: null, user: null
        })));
        await useRootStore().authorize({} as any);
        expect(mockStoreUser).not.toHaveBeenCalled();
    });

    test.each([
        ['missing data', Promise.resolve({}), { message: 'Unable to log in' }],
        ['invalid result', invalid(), { message: 'Unable to log in' }],
        ['processed error', Promise.reject({ processed: true, code: 'kept' }), { processed: true, code: 'kept' }],
        ['server error', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['transport error', Promise.reject(new Error('offline')), { message: 'Unable to log in' }]
    ])('maps authorize %s', async (_name, response, expected) => {
        mockServices.authorize.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useRootStore().authorize({} as any)).rejects.toEqual(expected);
    });

    test('authorizes 2FA using passcode and backup code and clears mismatched lock state', async () => {
        mockSettingsStore.appSettings.applicationLock = true;
        mockHasAppLockState = true;
        mockAppLockState = { username: 'other' };
        mockServices.authorize2FA.mockReturnValueOnce(ok(authResult()));
        const store = useRootStore();
        await store.authorize2FA({ token: 'challenge', passcode: '123456', recoveryCode: null });
        expect(mockServices.authorize2FA).toHaveBeenCalledWith({ token: 'challenge', passcode: '123456' });
        expect(mockClearTokenAndUser).toHaveBeenCalledWith(true);
        expect(mockUpdateApplicationSetting).toHaveBeenCalledTimes(2);

        mockSettingsStore.appSettings.applicationLock = false;
        mockHasAppLockState = false;
        mockServices.authorize2FAByBackupCode.mockReturnValueOnce(ok(authResult({
            refreshToken: null, applicationCloudSettings: null, user: null
        })));
        await store.authorize2FA({ token: 'challenge', passcode: null, recoveryCode: 'backup' });
        expect(mockServices.authorize2FAByBackupCode).toHaveBeenCalledWith({ token: 'challenge', recoveryCode: 'backup' });
        await expect(store.authorize2FA({ token: 'challenge', passcode: null, recoveryCode: null })).rejects.toEqual({ message: 'An error occurred' });
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to verify' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['transport', Promise.reject(new Error('offline')), { message: 'Unable to verify' }]
    ])('maps 2FA %s response', async (_name, response, expected) => {
        mockServices.authorize2FA.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useRootStore().authorize2FA({ token: 'x', passcode: '1', recoveryCode: null })).rejects.toEqual(expected);
    });

    test('authorizes OAuth, clears mismatched lock and persists optional auth data', async () => {
        mockSettingsStore.appSettings.applicationLock = true;
        mockAppLockState = null;
        mockServices.authorizeOAuth2.mockReturnValueOnce(ok(authResult()));
        await useRootStore().authorizeOAuth2({ password: 'secret', passcode: '1', callbackToken: 'callback' });
        expect(mockServices.authorizeOAuth2).toHaveBeenCalledWith({ password: 'secret', passcode: '1', callbackToken: 'callback' });
        expect(mockClearTokenAndUser).toHaveBeenCalledWith(true);
        expect(mockClearWebAuthn).toHaveBeenCalled();
        expect(mockUpdateToken).toHaveBeenCalledWith('access-token');
        expect(mockUpdateRefreshToken).toHaveBeenCalledWith('refresh-token');
        expect(mockStoreUser).toHaveBeenCalled();

        mockSettingsStore.appSettings.applicationLock = false;
        mockServices.authorizeOAuth2.mockReturnValueOnce(ok(authResult({ refreshToken: null, user: null })));
        await useRootStore().authorizeOAuth2({ callbackToken: 'callback' });
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to log in' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['transport', Promise.reject(new Error('offline')), { message: 'Unable to log in' }]
    ])('maps OAuth %s response', async (_name, response, expected) => {
        mockServices.authorizeOAuth2.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useRootStore().authorizeOAuth2({ callbackToken: 'x' })).rejects.toEqual(expected);
    });

    test('registers user, passes package choices and clears application lock', async () => {
        mockSettingsStore.appSettings.applicationLock = true;
        const request = { username: 'new-user' };
        const user = { toRegisterRequest: jest.fn((_preset?: unknown, _default?: unknown) => request) };
        mockServices.register.mockReturnValueOnce(ok(authResult()));
        await useRootStore().register({ user: user as any, presetCategories: [] as any, defaultPackage: {} as any });
        expect(user.toRegisterRequest).toHaveBeenCalledWith([], {});
        expect(mockServices.register).toHaveBeenCalledWith(request);
        expect(mockSetApplicationLock).toHaveBeenCalledWith(false);
        expect(mockSetApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(mockUpdateToken).toHaveBeenCalledWith('access-token');
        expect(mockUpdateRefreshToken).toHaveBeenCalledWith('refresh-token');
        expect(mockStoreUser).toHaveBeenCalled();

        mockSettingsStore.appSettings.applicationLock = false;
        mockServices.register.mockReturnValueOnce(ok(authResult({ token: null, refreshToken: null, user: null })));
        await useRootStore().register({ user: user as any });
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to sign up' }],
        ['server detail', Promise.reject(responseError()), { message: 'server detail', error: { message: 'server detail' } }],
        ['server no detail', Promise.reject({ response: { data: {} }, processed: false }), { message: 'Unable to sign up', error: {} }],
        ['transport', Promise.reject({ processed: false }), { message: 'Unable to sign up' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }]
    ])('maps register %s response', async (_name, response, expected) => {
        mockServices.register.mockReturnValueOnce(response as ApiResponse<any>);
        const user = { toRegisterRequest: () => ({}) };
        await expect(useRootStore().register({ user: user as any })).rejects.toEqual(expected);
    });
});

describe('root account lifecycle and simple actions coverage', () => {
    test('locks current session and force-logs out all user-scoped state', () => {
        const store = useRootStore();
        store.setNotificationContent('pending');
        store.lock();
        expect(mockClearSessionToken).toHaveBeenCalled();
        expect(store.currentNotification).toBeNull();
        expect(mockResetUser).not.toHaveBeenCalled();
        expect(mockResetRates).not.toHaveBeenCalled();
        store.forceLogout();
        expect(mockClearTokenAndUser).toHaveBeenCalledWith(true);
        expect(mockClearWebAuthn).toHaveBeenCalled();
        expect(mockResetUser).toHaveBeenCalled();
        expect(mockResetRates).toHaveBeenCalled();
        expect(mockResetAccounts).toHaveBeenCalledTimes(2);
        expect(mockResetCategories).toHaveBeenCalledTimes(2);
        expect(mockResetTags).toHaveBeenCalledTimes(2);
        expect(mockResetTemplates).toHaveBeenCalledTimes(2);
        expect(mockResetTransactions).toHaveBeenCalledTimes(2);
        expect(mockResetOverview).toHaveBeenCalledTimes(2);
        expect(mockResetStatistics).toHaveBeenCalledTimes(2);
    });

    test('logs out remotely then clears all local state', async () => {
        mockServices.logout.mockReturnValueOnce(ok(true));
        const store = useRootStore();
        await expect(store.logout()).resolves.toBe(true);
        expect(mockClearTokenAndUser).toHaveBeenCalledWith(true);
        expect(mockClearWebAuthn).toHaveBeenCalled();
        expect(mockResetUser).toHaveBeenCalled();
        expect(mockResetRates).toHaveBeenCalled();
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to logout' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['transport', Promise.reject(new Error('offline')), { message: 'Unable to logout' }]
    ])('maps logout %s response', async (_name, response, expected) => {
        mockServices.logout.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useRootStore().logout()).rejects.toEqual(expected);
    });

    test('verifies email and persists rotated token/user when present', async () => {
        mockServices.verifyEmail.mockReturnValueOnce(ok({ newToken: 'rotated', user: { username: 'alice' } }));
        const store = useRootStore();
        await store.verifyEmail({ token: 'verify', requestNewToken: true });
        expect(mockServices.verifyEmail).toHaveBeenCalledWith({ token: 'verify', requestNewToken: true });
        expect(mockUpdateToken).toHaveBeenCalledWith('rotated');
        expect(mockStoreUser).toHaveBeenCalled();
        mockServices.verifyEmail.mockReturnValueOnce(ok({ newToken: null, user: null }));
        await store.verifyEmail({ token: 'verify', requestNewToken: false });
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to verify email' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['transport', Promise.reject(new Error('offline')), { message: 'Unable to verify email' }]
    ])('maps verify-email %s response', async (_name, response, expected) => {
        mockServices.verifyEmail.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useRootStore().verifyEmail({ token: 'x', requestNewToken: false })).rejects.toEqual(expected);
    });

    test.each([
        ['resendVerifyEmailByUnloginUser', 'resendVerifyEmailByUnloginUser', [{ email: 'a@example.com' }], 'Unable to resend validation email'],
        ['requestResetPassword', 'requestResetPassword', [{ email: 'a@example.com' }], 'Unable to send password reset email'],
        ['resetPassword', 'resetPassword', [{ email: 'a@example.com', token: 't', password: 'p' }], 'Unable to reset password'],
        ['resendVerifyEmailByLoginedUser', 'resendVerifyEmailByLoginedUser', [], 'Unable to resend validation email']
    ])('executes and maps all %s response classes', async (_label, serviceName, args, fallback) => {
        const store = useRootStore() as any;
        const service = mockServices[serviceName as keyof typeof mockServices] as ServiceMock;
        service.mockReturnValueOnce(ok(true));
        await expect(store[_label](...args)).resolves.toBe(true);
        service.mockReturnValueOnce(invalid());
        await expect(store[_label](...args)).rejects.toEqual({ message: fallback });
        service.mockRejectedValueOnce({ processed: true });
        await expect(store[_label](...args)).rejects.toEqual({ processed: true });
        service.mockRejectedValueOnce(responseError());
        await expect(store[_label](...args)).rejects.toEqual({ error: { message: 'server detail' } });
        service.mockRejectedValueOnce(new Error('offline'));
        await expect(store[_label](...args)).rejects.toEqual({ message: fallback });
    });
});

describe('root profile and destructive-data behavior coverage', () => {
    test('updates profile, rotates token and invalidates every dependent view', async () => {
        mockServices.updateProfile.mockReturnValueOnce(ok({
            newToken: 'rotated',
            user: { username: 'alice', defaultCurrency: 'USD' }
        }));
        const store = useRootStore();
        await expect(store.updateUserProfile({ nickname: 'Alice' } as any)).resolves.toMatchObject({ newToken: 'rotated' });
        expect(mockUpdateToken).toHaveBeenCalledWith('rotated');
        expect(mockStoreUser).toHaveBeenCalled();
        expect(mockInvalidateAccounts).toHaveBeenCalledWith(true);
        expect(mockInvalidateOverview).toHaveBeenCalledWith(true);
        expect(mockInvalidateStatistics).toHaveBeenCalledWith(true);
        expect(mockResetRates).toHaveBeenCalled();

        mockAccountsStore.accountListStateInvalid = true;
        mockOverviewStore.transactionOverviewStateInvalid = true;
        mockStatisticsStore.transactionStatisticsStateInvalid = true;
        mockUserStore.currentUserDefaultCurrency = 'USD';
        jest.clearAllMocks();
        mockServices.updateProfile.mockReturnValueOnce(ok({ newToken: null, user: { defaultCurrency: 'USD' } }));
        await store.updateUserProfile({} as any);
        expect(mockUpdateToken).not.toHaveBeenCalled();
        expect(mockInvalidateAccounts).not.toHaveBeenCalled();
        expect(mockInvalidateOverview).not.toHaveBeenCalled();
        expect(mockInvalidateStatistics).not.toHaveBeenCalled();
        expect(mockResetRates).not.toHaveBeenCalled();
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to update user profile' }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['unprocessed', Promise.reject({ processed: false }), { message: 'Unable to update user profile' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }]
    ])('maps profile %s response', async (_name, response, expected) => {
        mockServices.updateProfile.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useRootStore().updateUserProfile({} as any)).rejects.toEqual(expected);
    });

    test('clears account/all transactions and full user data with precise invalidations', async () => {
        mockServices.clearAllTransactionsOfAccount.mockReturnValue(ok(true));
        mockServices.clearAllTransactions.mockReturnValue(ok(true));
        mockServices.clearAllData.mockReturnValue(ok(true));
        const store = useRootStore();
        await store.clearAllUserTransactionsOfAccount({ accountId: 'wallet', password: 'secret' });
        await store.clearAllUserTransactions({ password: 'secret' });
        await store.clearAllUserData({ password: 'secret' });
        expect(mockInvalidateAccounts).toHaveBeenCalledTimes(3);
        expect(mockInvalidateOverview).toHaveBeenCalledTimes(3);
        expect(mockInvalidateStatistics).toHaveBeenCalledTimes(3);
        expect(mockInvalidateCategories).toHaveBeenCalledWith(true);
        expect(mockInvalidateTags).toHaveBeenCalledWith(true);

        mockAccountsStore.accountListStateInvalid = true;
        mockCategoriesStore.transactionCategoryListStateInvalid = true;
        mockTagsStore.transactionTagListStateInvalid = true;
        mockOverviewStore.transactionOverviewStateInvalid = true;
        mockStatisticsStore.transactionStatisticsStateInvalid = true;
        jest.clearAllMocks();
        mockServices.clearAllTransactionsOfAccount.mockReturnValue(ok(true));
        mockServices.clearAllTransactions.mockReturnValue(ok(true));
        mockServices.clearAllData.mockReturnValue(ok(true));
        await store.clearAllUserTransactionsOfAccount({ accountId: 'wallet', password: 'secret' });
        await store.clearAllUserTransactions({ password: 'secret' });
        await store.clearAllUserData({ password: 'secret' });
        expect(mockInvalidateAccounts).not.toHaveBeenCalled();
        expect(mockInvalidateCategories).not.toHaveBeenCalled();
        expect(mockInvalidateTags).not.toHaveBeenCalled();
        expect(mockInvalidateOverview).not.toHaveBeenCalled();
        expect(mockInvalidateStatistics).not.toHaveBeenCalled();
    });

    test.each([
        ['clearAllUserTransactionsOfAccount', 'clearAllTransactionsOfAccount', { accountId: 'a', password: 'p' }],
        ['clearAllUserTransactions', 'clearAllTransactions', { password: 'p' }],
        ['clearAllUserData', 'clearAllData', { password: 'p' }]
    ])('maps every error class for %s', async (actionName, serviceName, args) => {
        const store = useRootStore() as any;
        const service = mockServices[serviceName as keyof typeof mockServices] as ServiceMock;
        service.mockReturnValueOnce(invalid());
        await expect(store[actionName](args)).rejects.toEqual({ message: 'Unable to clear user data' });
        service.mockRejectedValueOnce({ processed: true });
        await expect(store[actionName](args)).rejects.toEqual({ processed: true });
        service.mockRejectedValueOnce(responseError());
        await expect(store[actionName](args)).rejects.toEqual({ error: { message: 'server detail' } });
        service.mockRejectedValueOnce(new Error('offline'));
        await expect(store[actionName](args)).rejects.toEqual({ message: 'Unable to clear user data' });
    });
});
