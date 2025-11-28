import { ref } from 'vue';
import { defineStore } from 'pinia';

import { useSettingsStore } from './setting.ts';
import { useUserStore } from './user.ts';
import { useAccountsStore } from './account.ts';
import { useTransactionCategoriesStore } from './transactionCategory.ts';
import { useTransactionTagsStore } from './transactionTag.ts';
import { useTransactionTemplatesStore } from './transactionTemplate.ts';
import { useTransactionsStore } from './transaction.ts';
import { useOverviewStore } from './overview.ts';
import { useStatisticsStore } from './statistics.ts';
import { useExchangeRatesStore } from './exchangeRates.ts';

import type { AuthResponse, RegisterResponse } from '@/models/auth_response.ts';
import type {
    User,
    UserLoginRequest,
    UserResendVerifyEmailRequest,
    UserVerifyEmailResponse,
    UserProfileUpdateRequest,
    UserProfileUpdateResponse
} from '@/models/user.ts';
import type { ForgetPasswordRequest } from '@/models/forget_password.ts';
import type { LocalizedPresetCategory } from '@/core/category.ts';
import type { ApplicationCloudSetting } from '@/core/setting.ts';

import {
    isObject,
    isString
} from '@/lib/common.ts';
import {
    hasUserAppLockState,
    getUserAppLockState,
    getCurrentToken,
    updateCurrentToken,
    updateCurrentRefreshToken,
    clearWebAuthnConfig,
    clearCurrentSessionToken,
    clearCurrentTokenAndUserInfo
} from '@/lib/userstate.ts';
import services, { type ApiResponsePromise } from '@/lib/services.ts';
import logger from '@/lib/logger.ts';
import {
    updateApplicationSettingsValue
} from '@/lib/settings.ts';

export const useRootStore = defineStore('root', () => {
    const settingsStore = useSettingsStore();
    const userStore = useUserStore();
    const accountsStore = useAccountsStore();
    const transactionCategoriesStore = useTransactionCategoriesStore();
    const transactionTagsStore = useTransactionTagsStore();
    const transactionTemplatesStore = useTransactionTemplatesStore();
    const transactionsStore = useTransactionsStore();
    const overviewStore = useOverviewStore();
    const statisticsStore = useStatisticsStore();
    const exchangeRatesStore = useExchangeRatesStore();

    const currentNotification = ref<string | null>(null);

    function resetAllStates(resetUserInfoAndSettings: boolean): void {
        if (resetUserInfoAndSettings) {
            exchangeRatesStore.resetLatestExchangeRates();
        }

        setNotificationContent(null);

        statisticsStore.resetTransactionStatistics();
        overviewStore.resetTransactionOverview();
        transactionsStore.resetTransactions();
        transactionTagsStore.resetTransactionTags();
        transactionCategoriesStore.resetTransactionCategories();
        transactionTemplatesStore.resetTransactionTemplates();
        accountsStore.resetAccounts();

        if (resetUserInfoAndSettings) {
            userStore.resetUserBasicInfo();
        }
    }

    function setNotificationContent(content: string | null): void {
        currentNotification.value = content;
    }

    function generateOAuth2LoginUrl(platform: 'mobile' | 'desktop', clientSessionId: string): string {
        return services.generateOAuth2LoginUrl(platform, clientSessionId);
    }

    function generateOAuth2LinkUrl(platform: 'mobile' | 'desktop', clientSessionId: string): string {
        return services.generateOAuth2LinkUrl(platform, clientSessionId);
    }

    function authorize(req: UserLoginRequest): Promise<AuthResponse> {
        return new Promise((resolve, reject) => {
            services.authorize(req).then(response => {
                const data = response.data;

                logger.info('[Login] Authorize response received:', {
                    hasData: !!data,
                    success: data?.success,
                    hasResult: !!data?.result,
                    hasToken: !!data?.result?.token,
                    tokenPreview: data?.result?.token ? `${data.result.token.substring(0, 30)}...` : 'null',
                    tokenLength: data?.result?.token ? data.result.token.length : 0
                });

                if (!data || !data.success || !data.result || !data.result.token) {
                    logger.error('[Login] Invalid authorize response, rejecting');
                    reject({ message: 'Unable to log in' });
                    return;
                }

                if (data.result.need2FA) {
                    resolve(data.result);
                    return;
                }

                logger.info('[Login] Checking AppLock state', {
                    appLockEnabled: settingsStore.appSettings.applicationLock,
                    hasUserAppLockState: hasUserAppLockState(),
                    username: data.result.user?.username
                });

                if (settingsStore.appSettings.applicationLock || hasUserAppLockState()) {
                    const appLockState = getUserAppLockState();

                    logger.info('[Login] AppLock active, comparing users', {
                        appLockUsername: appLockState?.username,
                        loginUsername: data.result.user?.username,
                        match: appLockState?.username === data.result.user?.username
                    });

                    if (!appLockState || appLockState.username !== data.result.user?.username) {
                        logger.warn('[Login] AppLock username mismatch, will clear tokens AFTER saving new token to prevent 401 errors');

                        // 🆕 关键修复：先保存新token，再清理旧数据
                        // 这避免了在token更新过程中出现"无token"的窗口期
                        logger.info('[Login] Step 1: Save new token FIRST');
                        updateCurrentToken(data.result.token);

                        if (data.result.refreshToken && isString(data.result.refreshToken)) {
                            updateCurrentRefreshToken(data.result.refreshToken);
                        }

                        // 🆕 Step 2: 清理AppLock相关状态（但不删除token）
                        logger.info('[Login] Step 2: Clear AppLock state only');
                        sessionStorage.removeItem('ebk_user_app_lock_state');

                        // Force update localStorage directly to ensure isEnableApplicationLock() returns false immediately
                        updateApplicationSettingsValue('applicationLock', false);
                        updateApplicationSettingsValue('applicationLockWebAuthn', false);

                        settingsStore.setEnableApplicationLock(false);
                        settingsStore.setEnableApplicationLockWebAuthn(false);
                        clearWebAuthnConfig();

                        logger.info('[Login] AppLock state cleared, new token preserved');
                    } else {
                        logger.info('[Login] AppLock user matches, no clearing needed');
                    }
                } else {
                    logger.info('[Login] AppLock not enabled, skipping AppLock logic');
                }

                // Filter out application lock settings from cloud sync to prevent immediate lockout
                // because we don't have the PIN/Secret to encrypt the token yet.
                let applicationCloudSettings = data.result.applicationCloudSettings;
                if (applicationCloudSettings) {
                    applicationCloudSettings = applicationCloudSettings.filter(
                        (s: ApplicationCloudSetting) => s.settingKey !== 'applicationLock' && s.settingKey !== 'applicationLockWebAuthn'
                    );
                }

                settingsStore.setApplicationSettingsFromCloudSettings(applicationCloudSettings);

                // Mark login time to skip immediate token refresh in App.vue
                localStorage.setItem('ebk_last_login_time', Date.now().toString());
                logger.info('[Login] Set last login time to skip auto token refresh');

                // 🆕 关键修复：根据前面的逻辑判断是否需要保存token
                const appLockState = getUserAppLockState();
                const needSaveToken = !settingsStore.appSettings.applicationLock &&
                                     !appLockState ||
                                     (appLockState && appLockState.username === data.result.user?.username);

                if (needSaveToken) {
                    // 保存Token之前检查
                    logger.info('[Login] BEFORE updateCurrentToken:', {
                        tokenToSave: data.result.token ? `${data.result.token.substring(0, 30)}...` : 'null',
                        tokenLength: data.result.token.length,
                        currentLS: localStorage.getItem('ebk_user_token') ? 'exists' : 'null',
                        appLockEnabled: settingsStore.appSettings.applicationLock,
                        hasLockState: hasUserAppLockState()
                    });

                    updateCurrentToken(data.result.token);

                    // 保存Token之后立即检查
                    logger.info('[Login] AFTER updateCurrentToken:', {
                        tokenInLS: localStorage.getItem('ebk_user_token') ? `${localStorage.getItem('ebk_user_token')!.substring(0, 30)}...` : 'null',
                        matchesOriginal: localStorage.getItem('ebk_user_token') === data.result.token ||
                                         (localStorage.getItem('ebk_user_token')?.startsWith('eyJ') && data.result.token.startsWith('eyJ'))
                    });

                    // 验证Token是否正确存储
                    setTimeout(() => {
                        const storedToken = getCurrentToken();
                        const lsToken = localStorage.getItem('ebk_user_token');
                        logger.info('[Login] Token verification after storage:', {
                            tokenFromResult: data.result.token ? `${data.result.token.substring(0, 20)}...` : 'null',
                            storedToken: storedToken ? `${storedToken.substring(0, 20)}...` : 'null',
                            localStorage: lsToken ? `${lsToken.substring(0, 20)}...` : 'null',
                            match: storedToken === data.result.token
                        });
                    }, 100);

                    if (data.result.refreshToken && isString(data.result.refreshToken)) {
                        updateCurrentRefreshToken(data.result.refreshToken);
                    }
                } else {
                    logger.info('[Login] Token already saved in AppLock mismatch branch, skipping duplicate save');
                }

                if (data.result.user && isObject(data.result.user)) {
                    userStore.storeUserBasicInfo(data.result.user);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to login', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to log in' });
                }
            });
        });
    }

    function authorize2FA({ token, passcode, recoveryCode }: { token: string, passcode: string | null, recoveryCode: string | null }): Promise<AuthResponse> {
        return new Promise((resolve, reject) => {
            let promise: ApiResponsePromise<AuthResponse>;

            if (passcode) {
                promise = services.authorize2FA({
                    passcode: passcode,
                    token: token
                });
            } else if (recoveryCode) {
                promise = services.authorize2FAByBackupCode({
                    recoveryCode: recoveryCode,
                    token: token
                });
            } else {
                reject({ message: 'An error occurred' });
                return;
            }

            promise.then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result || !data.result.token) {
                    reject({ message: 'Unable to verify' });
                    return;
                }

                if (settingsStore.appSettings.applicationLock || hasUserAppLockState()) {
                    const appLockState = getUserAppLockState();

                    if (!appLockState || appLockState.username !== data.result.user?.username) {
                        clearCurrentTokenAndUserInfo(true);
                        // Force update localStorage directly to ensure isEnableApplicationLock() returns false immediately
                        updateApplicationSettingsValue('applicationLock', false);
                        updateApplicationSettingsValue('applicationLockWebAuthn', false);

                        settingsStore.setEnableApplicationLock(false);
                        settingsStore.setEnableApplicationLockWebAuthn(false);
                        clearWebAuthnConfig();
                    }
                }

                // Filter out application lock settings from cloud sync to prevent immediate lockout
                // because we don't have the PIN/Secret to encrypt the token yet.
                let applicationCloudSettings = data.result.applicationCloudSettings;
                if (applicationCloudSettings) {
                    applicationCloudSettings = applicationCloudSettings.filter(
                        (s: ApplicationCloudSetting) => s.settingKey !== 'applicationLock' && s.settingKey !== 'applicationLockWebAuthn'
                    );
                }

                settingsStore.setApplicationSettingsFromCloudSettings(applicationCloudSettings);

                updateCurrentToken(data.result.token);

                if (data.result.refreshToken && isString(data.result.refreshToken)) {
                    updateCurrentRefreshToken(data.result.refreshToken);
                }

                if (data.result.user && isObject(data.result.user)) {
                    userStore.storeUserBasicInfo(data.result.user);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to verify 2fa', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to verify' });
                }
            });
        });
    }

    function authorizeOAuth2({ password, passcode, callbackToken }: { password?: string, passcode?: string, callbackToken: string }): Promise<AuthResponse> {
        return new Promise((resolve, reject) => {
            services.authorizeOAuth2({
                password,
                passcode,
                callbackToken
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result || !data.result.token) {
                    reject({ message: 'Unable to log in' });
                    return;
                }

                if (settingsStore.appSettings.applicationLock || hasUserAppLockState()) {
                    const appLockState = getUserAppLockState();

                    if (!appLockState || appLockState.username !== data.result.user?.username) {
                        clearCurrentTokenAndUserInfo(true);
                        settingsStore.setEnableApplicationLock(false);
                        settingsStore.setEnableApplicationLockWebAuthn(false);
                        clearWebAuthnConfig();
                    }
                }

                settingsStore.setApplicationSettingsFromCloudSettings(data.result.applicationCloudSettings);

                updateCurrentToken(data.result.token);

                if (data.result.refreshToken && isString(data.result.refreshToken)) {
                    updateCurrentRefreshToken(data.result.refreshToken);
                }

                if (data.result.user && isObject(data.result.user)) {
                    userStore.storeUserBasicInfo(data.result.user);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to authorize oauth2', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to log in' });
                }
            });
        });
    }

    function register({ user, presetCategories }: { user: User, presetCategories?: LocalizedPresetCategory[] }): Promise<RegisterResponse> {
        return new Promise((resolve, reject) => {
            services.register(user.toRegisterRequest(presetCategories)).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to sign up' });
                    return;
                }

                if (settingsStore.appSettings.applicationLock) {
                    settingsStore.setEnableApplicationLock(false);
                    settingsStore.setEnableApplicationLockWebAuthn(false);
                    clearWebAuthnConfig();
                }

                if (data.result.token && isString(data.result.token)) {
                    updateCurrentToken(data.result.token);

                    if (data.result.refreshToken && isString(data.result.refreshToken)) {
                        updateCurrentRefreshToken(data.result.refreshToken);
                    }
                }

                if (data.result.user && isObject(data.result.user)) {
                    userStore.storeUserBasicInfo(data.result.user);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to sign up', error);

                if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to sign up' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function lock(): void {
        clearCurrentSessionToken();
        resetAllStates(false);
    }

    function logout(): Promise<boolean> {
        return new Promise((resolve, reject) => {
            logger.info('[Logout] 开始登出流程...');

            services.logout().then(response => {
                const data = response.data;

                logger.info('[Logout] 收到服务器响应:', {
                    hasData: !!data,
                    success: data?.success,
                    hasResult: !!data?.result
                });

                if (!data || !data.success || !data.result) {
                    logger.error('[Logout] 登出失败: 响应数据不完整', { data });
                    reject({ message: 'Unable to logout' });
                    return;
                }

                logger.info('[Logout] 服务器登出成功，开始清理本地数据...');

                logger.info('[Logout] Step 1: 清理Token和用户信息');
                clearCurrentTokenAndUserInfo(true);

                logger.info('[Logout] Step 2: 清理WebAuthn配置');
                clearWebAuthnConfig();

                logger.info('[Logout] Step 3: 重置所有Store状态');
                resetAllStates(true);

                logger.info('[Logout] ✅ 登出流程完成');

                resolve(data.result);
            }).catch(error => {
                logger.error('[Logout] 登出请求失败', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to logout' });
                }
            });
        });
    }

    function forceLogout(): void {
        clearCurrentTokenAndUserInfo(true);
        clearWebAuthnConfig();
        resetAllStates(true);
    }

    function verifyEmail({ token, requestNewToken }: { token: string, requestNewToken: boolean }): Promise<UserVerifyEmailResponse> {
        return new Promise((resolve, reject) => {
            services.verifyEmail({
                token,
                requestNewToken
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to verify email' });
                    return;
                }

                if (data.result.newToken && isString(data.result.newToken)) {
                    updateCurrentToken(data.result.newToken);
                }

                if (data.result.user && isObject(data.result.user)) {
                    userStore.storeUserBasicInfo(data.result.user);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to verify email', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to verify email' });
                }
            });
        });
    }

    function resendVerifyEmailByUnloginUser(req: UserResendVerifyEmailRequest): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.resendVerifyEmailByUnloginUser(req).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to resend validation email' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to resend verify email', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to resend validation email' });
                }
            });
        });
    }

    function requestResetPassword(req: ForgetPasswordRequest): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.requestResetPassword(req).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to send password reset email' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to send password reset email', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to send password reset email' });
                }
            });
        });
    }

    function resetPassword({ email, token, password }: { email: string, token: string, password: string }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.resetPassword({
                email,
                token,
                password
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to reset password' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to reset password', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to reset password' });
                }
            });
        });
    }

    function updateUserProfile(req: UserProfileUpdateRequest): Promise<UserProfileUpdateResponse> {
        const userDefaultCurrency = userStore.currentUserDefaultCurrency;

        return new Promise((resolve, reject) => {
            services.updateProfile(req).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to update user profile' });
                    return;
                }

                if (data.result.newToken && isString(data.result.newToken)) {
                    updateCurrentToken(data.result.newToken);
                }

                if (data.result.user && isObject(data.result.user)) {
                    userStore.storeUserBasicInfo(data.result.user);
                }

                if (!accountsStore.accountListStateInvalid) {
                    accountsStore.updateAccountListInvalidState(true);
                }

                if (!overviewStore.transactionOverviewStateInvalid) {
                    overviewStore.updateTransactionOverviewInvalidState(true);
                }

                if (!statisticsStore.transactionStatisticsStateInvalid) {
                    statisticsStore.updateTransactionStatisticsInvalidState(true);
                }

                if (data.result.user && data.result.user.defaultCurrency !== userDefaultCurrency) {
                    exchangeRatesStore.resetLatestExchangeRates();
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to save user profile', error);

                if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to update user profile' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function resendVerifyEmailByLoginedUser(): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.resendVerifyEmailByLoginedUser().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to resend validation email' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to resend verify email', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to resend validation email' });
                }
            });
        });
    }

    function clearAllUserTransactionsOfAccount({ accountId, password }: { accountId: string, password: string }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.clearAllTransactionsOfAccount({
                accountId: accountId,
                password: password
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to clear user data' });
                    return;
                }

                if (!accountsStore.accountListStateInvalid) {
                    accountsStore.updateAccountListInvalidState(true);
                }

                if (!overviewStore.transactionOverviewStateInvalid) {
                    overviewStore.updateTransactionOverviewInvalidState(true);
                }

                if (!statisticsStore.transactionStatisticsStateInvalid) {
                    statisticsStore.updateTransactionStatisticsInvalidState(true);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to clear user data', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to clear user data' });
                }
            });
        });
    }

    function clearAllUserTransactions({ password }: { password: string }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.clearAllTransactions({
                password: password
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to clear user data' });
                    return;
                }

                if (!accountsStore.accountListStateInvalid) {
                    accountsStore.updateAccountListInvalidState(true);
                }

                if (!overviewStore.transactionOverviewStateInvalid) {
                    overviewStore.updateTransactionOverviewInvalidState(true);
                }

                if (!statisticsStore.transactionStatisticsStateInvalid) {
                    statisticsStore.updateTransactionStatisticsInvalidState(true);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to clear user data', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to clear user data' });
                }
            });
        });
    }

    function clearAllUserData({ password }: { password: string }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.clearAllData({
                password: password
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to clear user data' });
                    return;
                }

                if (!accountsStore.accountListStateInvalid) {
                    accountsStore.updateAccountListInvalidState(true);
                }

                if (!transactionCategoriesStore.transactionCategoryListStateInvalid) {
                    transactionCategoriesStore.updateTransactionCategoryListInvalidState(true);
                }

                if (!transactionTagsStore.transactionTagListStateInvalid) {
                    transactionTagsStore.updateTransactionTagListInvalidState(true);
                }

                if (!overviewStore.transactionOverviewStateInvalid) {
                    overviewStore.updateTransactionOverviewInvalidState(true);
                }

                if (!statisticsStore.transactionStatisticsStateInvalid) {
                    statisticsStore.updateTransactionStatisticsInvalidState(true);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to clear user data', error);

                if (error && error.processed) {
                    reject(error);
                } else if (error.response && error.response.data && error.response.data.errorMessage) {
                    reject({ error: error.response.data });
                } else {
                    reject({ message: 'Unable to clear user data' });
                }
            });
        });
    }

    return {
        // states
        currentNotification,
        // functions
        setNotificationContent,
        generateOAuth2LoginUrl,
        generateOAuth2LinkUrl,
        authorize,
        authorize2FA,
        authorizeOAuth2,
        register,
        lock,
        logout,
        forceLogout,
        verifyEmail,
        resendVerifyEmailByUnloginUser,
        requestResetPassword,
        resetPassword,
        updateUserProfile,
        resendVerifyEmailByLoginedUser,
        clearAllUserTransactionsOfAccount,
        clearAllUserTransactions,
        clearAllUserData
    };
});
