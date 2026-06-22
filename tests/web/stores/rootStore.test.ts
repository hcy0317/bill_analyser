import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

type ApiResponse<T> = {
    data: {
        success: boolean;
        result: T;
    };
};

const mockSetApplicationSettingsFromCloudSettings = jest.fn();
const mockSetEnableApplicationLock = jest.fn();
const mockSetEnableApplicationLockWebAuthn = jest.fn();
const mockResetUserBasicInfo = jest.fn();
const mockStoreUserBasicInfo = jest.fn();
const mockResetAccounts = jest.fn();
const mockUpdateAccountListInvalidState = jest.fn();
const mockResetTransactionCategories = jest.fn();
const mockUpdateTransactionCategoryListInvalidState = jest.fn();
const mockResetTransactionTags = jest.fn();
const mockUpdateTransactionTagListInvalidState = jest.fn();
const mockResetTransactionTemplates = jest.fn();
const mockResetTransactions = jest.fn();
const mockResetTransactionOverview = jest.fn();
const mockUpdateTransactionOverviewInvalidState = jest.fn();
const mockResetTransactionStatistics = jest.fn();
const mockUpdateTransactionStatisticsInvalidState = jest.fn();
const mockResetLatestExchangeRates = jest.fn();
const mockClearCurrentSessionToken = jest.fn();
const mockClearCurrentTokenAndUserInfo = jest.fn();
const mockClearWebAuthnConfig = jest.fn();
const mockUpdateCurrentToken = jest.fn();

const mockSettingsStore = {
    appSettings: {
        applicationLock: false
    },
    setApplicationSettingsFromCloudSettings: mockSetApplicationSettingsFromCloudSettings,
    setEnableApplicationLock: mockSetEnableApplicationLock,
    setEnableApplicationLockWebAuthn: mockSetEnableApplicationLockWebAuthn
};
const mockUserStore = {
    currentUserDefaultCurrency: 'CNY',
    resetUserBasicInfo: mockResetUserBasicInfo,
    storeUserBasicInfo: mockStoreUserBasicInfo
};
const mockAccountsStore = {
    accountListStateInvalid: false,
    resetAccounts: mockResetAccounts,
    updateAccountListInvalidState: mockUpdateAccountListInvalidState
};
const mockTransactionCategoriesStore = {
    transactionCategoryListStateInvalid: false,
    resetTransactionCategories: mockResetTransactionCategories,
    updateTransactionCategoryListInvalidState: mockUpdateTransactionCategoryListInvalidState
};
const mockTransactionTagsStore = {
    transactionTagListStateInvalid: false,
    resetTransactionTags: mockResetTransactionTags,
    updateTransactionTagListInvalidState: mockUpdateTransactionTagListInvalidState
};
const mockTransactionTemplatesStore = {
    resetTransactionTemplates: mockResetTransactionTemplates
};
const mockTransactionsStore = {
    resetTransactions: mockResetTransactions
};
const mockOverviewStore = {
    transactionOverviewStateInvalid: false,
    resetTransactionOverview: mockResetTransactionOverview,
    updateTransactionOverviewInvalidState: mockUpdateTransactionOverviewInvalidState
};
const mockStatisticsStore = {
    transactionStatisticsStateInvalid: false,
    resetTransactionStatistics: mockResetTransactionStatistics,
    updateTransactionStatisticsInvalidState: mockUpdateTransactionStatisticsInvalidState
};
const mockExchangeRatesStore = {
    resetLatestExchangeRates: mockResetLatestExchangeRates
};

const mockLogout = jest.fn<() => Promise<ApiResponse<boolean>>>();
const mockUpdateProfile = jest.fn<(payload: unknown) => Promise<ApiResponse<unknown>>>();
const mockClearAllTransactionsOfAccount = jest.fn<(payload: unknown) => Promise<ApiResponse<boolean>>>();
const mockClearAllTransactions = jest.fn<(payload: unknown) => Promise<ApiResponse<boolean>>>();
const mockClearAllData = jest.fn<(payload: unknown) => Promise<ApiResponse<boolean>>>();

jest.mock('@/stores/setting.ts', () => ({
    __esModule: true,
    useSettingsStore: () => mockSettingsStore
}));

jest.mock('@/stores/user.ts', () => ({
    __esModule: true,
    useUserStore: () => mockUserStore
}));

jest.mock('@/stores/account.ts', () => ({
    __esModule: true,
    useAccountsStore: () => mockAccountsStore
}));

jest.mock('@/stores/transactionCategory.ts', () => ({
    __esModule: true,
    useTransactionCategoriesStore: () => mockTransactionCategoriesStore
}));

jest.mock('@/stores/transactionTag.ts', () => ({
    __esModule: true,
    useTransactionTagsStore: () => mockTransactionTagsStore
}));

jest.mock('@/stores/transactionTemplate.ts', () => ({
    __esModule: true,
    useTransactionTemplatesStore: () => mockTransactionTemplatesStore
}));

jest.mock('@/stores/transaction.ts', () => ({
    __esModule: true,
    useTransactionsStore: () => mockTransactionsStore
}));

jest.mock('@/stores/overview.ts', () => ({
    __esModule: true,
    useOverviewStore: () => mockOverviewStore
}));

jest.mock('@/stores/statistics.ts', () => ({
    __esModule: true,
    useStatisticsStore: () => mockStatisticsStore
}));

jest.mock('@/stores/exchangeRates.ts', () => ({
    __esModule: true,
    useExchangeRatesStore: () => mockExchangeRatesStore
}));

jest.mock('@/lib/userstate.ts', () => ({
    __esModule: true,
    hasUserAppLockState: () => false,
    getUserAppLockState: () => null,
    getCurrentToken: () => 'access-token',
    updateCurrentToken: (token: string) => mockUpdateCurrentToken(token),
    updateCurrentRefreshToken: jest.fn(),
    clearWebAuthnConfig: () => mockClearWebAuthnConfig(),
    clearCurrentSessionToken: () => mockClearCurrentSessionToken(),
    clearCurrentTokenAndUserInfo: (includeUserInfo?: boolean) => mockClearCurrentTokenAndUserInfo(includeUserInfo)
}));

jest.mock('@/lib/settings.ts', () => ({
    __esModule: true,
    updateApplicationSettingsValue: jest.fn()
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        logout: () => mockLogout(),
        updateProfile: (payload: unknown) => mockUpdateProfile(payload),
        clearAllTransactionsOfAccount: (payload: unknown) => mockClearAllTransactionsOfAccount(payload),
        clearAllTransactions: (payload: unknown) => mockClearAllTransactions(payload),
        clearAllData: (payload: unknown) => mockClearAllData(payload),
        generateOAuth2LoginUrl: jest.fn(() => '/oauth-login'),
        generateOAuth2LinkUrl: jest.fn(() => '/oauth-link')
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

import { useRootStore } from '@/stores/index.ts';

function apiResponse<T>(result: T): ApiResponse<T> {
    return {
        data: {
            success: true,
            result
        }
    };
}

function resetMockStores(): void {
    mockSettingsStore.appSettings.applicationLock = false;
    mockUserStore.currentUserDefaultCurrency = 'CNY';
    mockAccountsStore.accountListStateInvalid = false;
    mockTransactionCategoriesStore.transactionCategoryListStateInvalid = false;
    mockTransactionTagsStore.transactionTagListStateInvalid = false;
    mockOverviewStore.transactionOverviewStateInvalid = false;
    mockStatisticsStore.transactionStatisticsStateInvalid = false;
}

describe('root store shared runtime shell', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        jest.clearAllMocks();
        resetMockStores();
        mockLogout.mockResolvedValue(apiResponse(true));
        mockUpdateProfile.mockResolvedValue(apiResponse({}));
        mockClearAllTransactionsOfAccount.mockResolvedValue(apiResponse(true));
        mockClearAllTransactions.mockResolvedValue(apiResponse(true));
        mockClearAllData.mockResolvedValue(apiResponse(true));
    });

    test('lock clears only session token and keeps user/settings scoped state intact', () => {
        const store = useRootStore();

        store.setNotificationContent('pending message');
        store.lock();

        expect(mockClearCurrentSessionToken).toHaveBeenCalledTimes(1);
        expect(store.currentNotification).toBeNull();
        expect(mockResetTransactionStatistics).toHaveBeenCalledTimes(1);
        expect(mockResetTransactionOverview).toHaveBeenCalledTimes(1);
        expect(mockResetTransactions).toHaveBeenCalledTimes(1);
        expect(mockResetTransactionTags).toHaveBeenCalledTimes(1);
        expect(mockResetTransactionCategories).toHaveBeenCalledTimes(1);
        expect(mockResetTransactionTemplates).toHaveBeenCalledTimes(1);
        expect(mockResetAccounts).toHaveBeenCalledTimes(1);
        expect(mockResetUserBasicInfo).not.toHaveBeenCalled();
        expect(mockResetLatestExchangeRates).not.toHaveBeenCalled();
    });

    test('forceLogout clears auth, WebAuthn, user state and exchange-rate cache', () => {
        const store = useRootStore();

        store.forceLogout();

        expect(mockClearCurrentTokenAndUserInfo).toHaveBeenCalledWith(true);
        expect(mockClearWebAuthnConfig).toHaveBeenCalledTimes(1);
        expect(mockResetUserBasicInfo).toHaveBeenCalledTimes(1);
        expect(mockResetLatestExchangeRates).toHaveBeenCalledTimes(1);
    });

    test('updateUserProfile stores rotated token and invalidates dependent account/statistics views', async () => {
        mockUpdateProfile.mockResolvedValue(apiResponse({
            newToken: 'rotated-token',
            user: {
                username: 'profile-user',
                defaultCurrency: 'USD'
            }
        }));
        const store = useRootStore();

        await expect(store.updateUserProfile({ nickname: 'Profile User' })).resolves.toMatchObject({
            newToken: 'rotated-token'
        });

        expect(mockUpdateProfile).toHaveBeenCalledWith({ nickname: 'Profile User' });
        expect(mockUpdateCurrentToken).toHaveBeenCalledWith('rotated-token');
        expect(mockStoreUserBasicInfo).toHaveBeenCalledWith(expect.objectContaining({
            username: 'profile-user'
        }));
        expect(mockUpdateAccountListInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionOverviewInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionStatisticsInvalidState).toHaveBeenCalledWith(true);
        expect(mockResetLatestExchangeRates).toHaveBeenCalledTimes(1);
    });

    test('account-scoped transaction clear leaves taxonomy stores untouched', async () => {
        const store = useRootStore();

        await expect(store.clearAllUserTransactionsOfAccount({
            accountId: 'wallet',
            password: 'secret'
        })).resolves.toBe(true);

        expect(mockClearAllTransactionsOfAccount).toHaveBeenCalledWith({
            accountId: 'wallet',
            password: 'secret'
        });
        expect(mockUpdateAccountListInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionOverviewInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionStatisticsInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionCategoryListInvalidState).not.toHaveBeenCalled();
        expect(mockUpdateTransactionTagListInvalidState).not.toHaveBeenCalled();
    });

    test('full user-data clear invalidates accounts, taxonomy, overview and statistics', async () => {
        const store = useRootStore();

        await expect(store.clearAllUserData({ password: 'secret' })).resolves.toBe(true);

        expect(mockClearAllData).toHaveBeenCalledWith({ password: 'secret' });
        expect(mockUpdateAccountListInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionCategoryListInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionTagListInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionOverviewInvalidState).toHaveBeenCalledWith(true);
        expect(mockUpdateTransactionStatisticsInvalidState).toHaveBeenCalledWith(true);
    });
});
