import axios, { type AxiosRequestConfig, type AxiosRequestHeaders, type AxiosResponse } from 'axios';
import type { ApiResponse } from '@/core/api.ts';

import type {
    ApplicationCloudSetting
} from '@/core/setting.ts';
import type {
    VersionInfo
} from '@/core/version.ts';
import {
    BASE_API_URL_PATH,
    BASE_QRCODE_PATH,
    BASE_PROXY_URL_PATH,
    BASE_AMAP_API_PROXY_URL_PATH,
    DEFAULT_API_TIMEOUT,
    DEFAULT_UPLOAD_API_TIMEOUT,
    DEFAULT_EXPORT_API_TIMEOUT,
    DEFAULT_CLEAR_ALL_TRANSACTIONS_API_TIMEOUT,
    DEFAULT_LLM_API_TIMEOUT,
    GOOGLE_MAP_JAVASCRIPT_URL,
    BAIDU_MAP_JAVASCRIPT_URL,
    AMAP_JAVASCRIPT_URL
} from '@/consts/api.ts';
import type {
    AccountCreateRequest,
    AccountModifyRequest,
    AccountInfoResponse,
    AccountHideRequest,
    AccountMoveRequest,
    AccountDeleteRequest,
    SyncBalancesResponse
} from '@/models/account.ts';
import type {
    AuthResponse,
    RegisterResponse
} from '@/models/auth_response.ts';
import type {
    ExportTransactionDataRequest,
    ClearDataRequest,
    ClearAccountTransactionsRequest,
    DataStatisticsResponse,
    SettingsBundleImportResult,
    SettingsBundleExportAuth,
    SettingsBundleSectionKey
} from '@/models/data_management.ts';
import type {
    UserCustomExchangeRateUpdateRequest,
    UserCustomExchangeRateDeleteRequest,
    UserCustomExchangeRateUpdateResponse,
    LatestExchangeRateResponse
} from '@/models/exchange_rate.ts';
import type {
    ForgetPasswordRequest
} from '@/models/forget_password.ts';
import type {
    LearningSuggestionsResponse,
    LearningRulesResponse,
    BatchAcceptResponse,
    GenerateSuggestionsResponse
} from '@/models/learning_center.ts';
import type {
    RecurringSuggestionsResponse,
    RecurringDetectResponse,
    RecurringAcceptResponse,
} from '@/models/recurring_suggestion.ts';
import {
    normalizeSuggestionsResponse as normalizeRecurringSuggestionsResponse,
    normalizeDetectResponse,
} from '@/models/recurring_suggestion.ts';
import {
    normalizeSuggestionsResponse,
    normalizeRulesResponse,
    normalizeBatchAcceptResponse,
    normalizeGenerateResponse
} from '@/models/learning_center.ts';
import type {
    TransactionCreateRequest,
    TransactionImportRequest,
    TransactionModifyRequest,
    TransactionMoveBetweenAccountsRequest,
    TransactionDeleteRequest,
    TransactionListByMaxTimeRequest,
    TransactionListInMonthByPageRequest,
    TransactionInfoResponse,
    TransactionInfoPageWrapperResponse,
    TransactionInfoPageWrapperResponse2,
    TransactionReconciliationStatementRequest,
    TransactionReconciliationStatementResponse,
    TransactionStatisticRequest,
    TransactionStatisticResponse,
    TransactionStatisticTrendsRequest,
    TransactionStatisticTrendsResponseItem,
    TransactionStatisticAssetTrendsRequest,
    TransactionStatisticAssetTrendsResponseItem,
    TransactionAmountsRequestParams,
    TransactionAmountsResponse
} from '@/models/transaction.ts';
import {
    TransactionAmountsRequest
} from '@/models/transaction.ts';
import { buildTransactionListQuery } from '@/lib/services/transaction.ts';
import importPreviewServices from './services/importPreview.ts';
import {
    buildApiResponse,
    type ApiDataResponse,
    type ApiRequestConfig,
    type ApiResponsePromise
} from './services/http.ts';
export type { ApiResponsePromise } from './services/http.ts';
import type {
    AccountRuleCreateRequest, AnalyzeLLMTransactionsRequest, AnomalyListRequest,
    BudgetForecastQueryRequest, BudgetHistoryQueryRequest, CalendarEventsRequest,
    CreateLLMConfigRequest, LearningRuleListRequest, LLMAnalyzeTransactionsResponse,
    LLMMemoryEventsResponse, LLMPreviewRecommendAcceptRequest, LLMPreviewRecommendRejectRequest,
    LLMPreviewRecommendRequest, LLMRuleSynthesisRequest, LLMRuleSynthesisResponse,
    OCRConfigResponse, OCRConfigUpdateRequest, PagedStatusRequest
} from './services/contracts.ts';
export type { OCRConfigResponse } from './services/contracts.ts';
import type {
    TransactionCategoryCreateRequest,
    TransactionCategoryCreateBatchRequest,
    TransactionCategoryModifyRequest,
    TransactionCategoryHideRequest,
    TransactionCategoryMoveRequest,
    TransactionCategoryDeleteRequest,
    TransactionCategoryInfoResponse
} from '@/models/transaction_category.ts';
import type {
    TransactionPictureUnusedDeleteRequest,
    TransactionPictureInfoBasicResponse
} from '@/models/transaction_picture_info.ts';
import type {
    TransactionTagCreateRequest,
    TransactionTagCreateBatchRequest,
    TransactionTagModifyRequest,
    TransactionTagHideRequest,
    TransactionTagMoveRequest,
    TransactionTagDeleteRequest,
    TransactionTagInfoResponse
} from '@/models/transaction_tag.ts';
import type {
    TransactionTemplateCreateRequest,
    TransactionTemplateModifyRequest,
    TransactionTemplateHideRequest,
    TransactionTemplateMoveRequest,
    TransactionTemplateDeleteRequest,
    TransactionTemplateInfoResponse
} from '@/models/transaction_template.ts';
import type {
    TokenGenerateAPIRequest,
    TokenGenerateMCPRequest,
    TokenGenerateAPIResponse,
    TokenGenerateMCPResponse,
    TokenRefreshResponse,
    TokenInfoResponse
} from '@/models/token.ts';
import type {
    TwoFactorEnableConfirmRequest,
    TwoFactorEnableResponse,
    TwoFactorEnableConfirmResponse,
    TwoFactorDisableRequest,
    TwoFactorRegenerateRecoveryCodeRequest,
    TwoFactorStatusResponse
} from '@/models/two_factor.ts';
import type {
    UserLoginRequest,
    UserRegisterRequest,
    UserVerifyEmailResponse,
    UserResendVerifyEmailRequest,
    UserProfileResponse,
    UserProfileUpdateRequest,
    UserProfileUpdateResponse
} from '@/models/user.ts';
import type {
    UserExternalAuthUnlinkRequest,
    UserExternalAuthInfoResponse
} from '@/models/user_external_auth.ts';
import type {
    OAuth2CallbackLoginRequest
} from '@/models/oauth2.ts';
import type {
    UserApplicationCloudSettingsUpdateRequest
} from '@/models/user_app_cloud_setting.ts';
import type {
    RecognizedReceiptImageResponse
} from '@/models/large_language_model.ts';
import {
    getCurrentToken,
    getCurrentRefreshToken,
    updateCurrentToken,
    updateCurrentRefreshToken,
    clearCurrentTokenAndUserInfo
} from './userstate.ts';

import {
    isEnableApplicationLock
} from './settings.ts';

import {
    isDefined,
    isBoolean
} from './common.ts';
import {
    getGoogleMapAPIKey,
    getBaiduMapAK,
    getAmapApplicationKey,
    getExchangeRatesRequestTimeout
} from './server_settings.ts';
import { getTimezoneOffsetMinutes } from './datetime.ts';
import { generateRandomUUID } from './misc.ts';
import { getBasePath } from './web.ts';
import logger from './logger.ts';
import { getApiErrorMessage } from './api_error.ts';
import {
    buildBudgetExecutionQuery,
    buildBudgetForecastQuery,
    buildBudgetHistoryQuery,
    buildBudgetListQuery,
    mapBudgetRequestToRest,
    mapImportedBudgetToRest,
    mapRestBudgetToFrontend,
    mapRestExecutionToFrontend,
    mapRestForecastToFrontend,
    mapRestHistoryToFrontend
} from './services/budget.ts';

// ==== 代码版本标记：2025-11-20-00:05 ====
// 修复：添加initializeAxiosAuth()在main文件中调用
// 确保页面加载时就设置axios.defaults.headers.common['Authorization']
const SERVICES_CODE_VERSION = '2025-11-20-00:05-INIT-AXIOS-AUTH';
logger.debug(`[services.ts] Loading version: ${SERVICES_CODE_VERSION}`);

function isPublicNoAuthRequest(url: string): boolean {
    const normalizedUrl = (url.split('?')[0] || '')
        .replace(/^\/+/, '')
        .replace(/^api\/+/, '');
    return [
        'auth/login',
        'auth/register',
        'auth/email/resend-verification',
        'auth/email/verify',
        'auth/password/forgot',
        'auth/password/reset',
        '2fa/verify',
        '2fa/recovery/verify',
        'auth/oauth2/authorize'
    ].includes(normalizedUrl);
}

let needBlockRequest = false;
interface BlockedRequest {
    readonly resume: () => void;
    readonly reject: (reason?: unknown) => void;
}

const blockedRequests: BlockedRequest[] = [];
let activeRefreshPromise: ApiResponsePromise<TokenRefreshResponse> | null = null;
const cancelableRequests: Record<string, boolean> = {};
const refreshFailureLogCodes = new Set([
    'ERR_BAD_RESPONSE',
    'ERR_CANCELED',
    'ERR_NETWORK',
    'ECONNABORTED',
    'ETIMEDOUT'
]);

type TokenFailureOperation = 'refresh' | 'revoke';

interface TokenFailureLog {
    message: 'Token refresh failed' | 'Token revoke failed';
    route: 'tokens/refresh' | 'tokens/:id';
    code?: string;
    status?: number;
}

function getTokenFailureLog(reason: unknown, operation: TokenFailureOperation): TokenFailureLog {
    const failureLog: TokenFailureLog = operation === 'refresh'
        ? { message: 'Token refresh failed', route: 'tokens/refresh' }
        : { message: 'Token revoke failed', route: 'tokens/:id' };

    if (typeof reason !== 'object' || reason === null) {
        return failureLog;
    }

    try {
        const errorRecord = reason as Record<string, unknown>;
        const code = errorRecord['code'];

        if (typeof code === 'string' && refreshFailureLogCodes.has(code)) {
            failureLog.code = code;
        }

        const response = errorRecord['response'];
        if (typeof response === 'object' && response !== null) {
            const status = (response as Record<string, unknown>)['status'];
            if (typeof status === 'number' && Number.isInteger(status) && status >= 100 && status <= 599) {
                failureLog.status = status;
            }
        }
    } catch {
        return failureLog;
    }

    return failureLog;
}

function getRefreshFailureLog(reason: unknown): TokenFailureLog {
    return getTokenFailureLog(reason, 'refresh');
}

function getTokenResponseFailureOperation(reason: unknown): TokenFailureOperation | null {
    if (typeof reason !== 'object' || reason === null) {
        return null;
    }

    try {
        const response = (reason as Record<string, unknown>)['response'];
        if (typeof response !== 'object' || response === null) {
            return null;
        }

        const config = (response as Record<string, unknown>)['config'];
        if (typeof config !== 'object' || config === null) {
            return null;
        }

        const configRecord = config as Record<string, unknown>;
        const url = configRecord['url'];
        if (typeof url !== 'string') {
            return null;
        }

        const normalizedUrl = (url.split(/[?#]/, 1)[0] || '')
            .replace(/^[a-z][a-z\d+.-]*:\/\/[^/]+/i, '')
            .replace(/^.*\/api\//, '')
            .replace(/^\/+/, '')
            .replace(/^api\/+/, '')
            .replace(/\/+$/, '');

        if (normalizedUrl === 'tokens/refresh') {
            return 'refresh';
        }

        const method = configRecord['method'];
        if (typeof method === 'string' && method.toLowerCase() === 'delete'
            && /^tokens\/[^/]+$/.test(normalizedUrl)) {
            return 'revoke';
        }
    } catch {
        return null;
    }

    return null;
}

function resolveBlockedRequests(): void {
    const pendingRequests = blockedRequests.splice(0, blockedRequests.length);

    for (const request of pendingRequests) {
        request.resume();
    }
}

function rejectBlockedRequests(reason: unknown): void {
    const pendingRequests = blockedRequests.splice(0, blockedRequests.length);

    for (const request of pendingRequests) {
        request.reject(reason);
    }
}

function refreshAuthenticationToken(): ApiResponsePromise<TokenRefreshResponse> {
    if (activeRefreshPromise) {
        return activeRefreshPromise;
    }

    const refreshToken = getCurrentRefreshToken();

    logger.debug(`[refreshToken] Called, current needBlockRequest=${needBlockRequest}, blockedRequests=${blockedRequests.length}`);

    if (!refreshToken) {
        const error = Object.assign(new Error('No refresh token available'), {
            noRefreshToken: true
        });
        needBlockRequest = false;
        rejectBlockedRequests(error);
        return Promise.reject(error);
    }

    needBlockRequest = true;
    const refreshRequest = axios.post<ApiResponse<TokenRefreshResponse>>('tokens/refresh', { refreshToken }, {
        ignoreBlocked: true,
        noAuth: true
    } as ApiRequestConfig);

    activeRefreshPromise = refreshRequest.then(response => {
        const data = response.data;
        const result = data?.result;

        if (!data?.success || !result
            || typeof result.newToken !== 'string' || !result.newToken
            || typeof result.refreshToken !== 'string' || !result.refreshToken) {
            throw new Error('Invalid token refresh response');
        }

        updateCurrentRefreshToken(result.refreshToken);

        if (getCurrentRefreshToken() !== result.refreshToken) {
            throw new Error('Unable to persist refreshed credential');
        }

        updateCurrentToken(result.newToken);

        if (getCurrentToken() !== result.newToken) {
            throw new Error('Unable to persist refreshed access token');
        }

        needBlockRequest = false;
        resolveBlockedRequests();
        return response;
    }).catch(error => {
        const failure = Object.freeze(getRefreshFailureLog(error));
        logger.error('[auth-refresh] Token refresh failed', failure);
        needBlockRequest = false;
        rejectBlockedRequests(failure);
        throw failure;
    }).finally(() => {
        needBlockRequest = false;
        activeRefreshPromise = null;
    });

    return activeRefreshPromise;
}

axios.defaults.baseURL = getBasePath() + BASE_API_URL_PATH;
axios.defaults.timeout = DEFAULT_API_TIMEOUT;

// ==== 初始化Authorization ====
// 注意：不在模块加载时调用getCurrentToken，因为可能触发AppLock检查
// 而是在页面加载完成后，由initializeAxiosAuth()函数统一处理
// @version 2025-11-20-00:15-ULTIMATE-FIX
logger.debug('[Axios Init] axios.defaults configured, waiting for initializeAxiosAuth() call');

// ==== 全局函数：初始化Axios Authorization（页面加载后调用） ====
export function initializeAxiosAuth(): void {
    logger.debug('[initializeAxiosAuth] axios auth initialization started');
    const token = getCurrentToken();
    if (token) {
        axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
        logger.debug(`[initializeAxiosAuth] Set Authorization header in axios.defaults.headers.common (token length: ${token.length})`);
        logger.debug(`[initializeAxiosAuth] Verification: axios.defaults.headers.common['Authorization'] = ${axios.defaults.headers.common['Authorization'] ? 'SET' : 'NOT_SET'}`);
    } else {
        logger.debug('[initializeAxiosAuth] No token found in localStorage');
    }
}

// ==== 全局函数：更新axios.defaults中的Authorization ====
// 在登录后调用此函数，确保后续所有请求都自动带上Authorization
export function updateAxiosAuthorizationHeader(token: string): void {
    if (token) {
        axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
        logger.debug(`[updateAxiosAuth] Updated axios.defaults.headers.common['Authorization'] (token length: ${token.length})`);
    } else {
        delete axios.defaults.headers.common['Authorization'];
        logger.debug('[updateAxiosAuth] Cleared axios.defaults.headers.common[\'Authorization\']');
    }
}

// ==== 辅助函数：设置Authorization头 ====
// 注意：这个函数现在主要用于拦截器中的双重保险
// 主要的Authorization应该通过axios.defaults.headers.common设置
function setAuthorizationHeader(headers: any, token: string): void {
    const authValue = `Bearer ${token}`;

    // 使用多种方式确保设置生效
    try {
        if (typeof headers.set === 'function') {
            // AxiosHeaders对象使用set方法
            headers.set('Authorization', authValue);
            logger.debug('[setAuthorizationHeader] Used headers.set() method');
        }
    } catch (e) {
        logger.warn('[setAuthorizationHeader] headers.set() failed', e);
    }

    // 同时使用直接赋值作为备份
    try {
        headers['Authorization'] = authValue;
        headers.Authorization = authValue;
        logger.debug('[setAuthorizationHeader] Used direct assignment');
    } catch (e) {
        logger.warn('[setAuthorizationHeader] Direct assignment failed', e);
    }

    // 验证是否成功
    const checkValue = headers.Authorization || headers['Authorization'] || (typeof headers.get === 'function' ? headers.get('Authorization') : null);
    if (!checkValue) {
        logger.error('[setAuthorizationHeader] FAILED to set Authorization header!');
    } else {
        logger.debug('[setAuthorizationHeader] Success, authorization header present');
    }
}

function clearAuthorizationHeader(headers: AxiosRequestHeaders): void {
    const deletableHeaders = headers as AxiosRequestHeaders & {
        delete?: (header: string | string[]) => unknown;
    };

    try {
        deletableHeaders.delete?.(['Authorization', 'authorization']);
    } catch {
        logger.warn('[clearAuthorizationHeader] headers.delete() failed');
    }

    delete headers['Authorization'];
    delete headers['authorization'];
    delete headers.Authorization;
}

// ==== 拦截器注册标记 ====
logger.debug('[services.ts] Registering request interceptor');

axios.interceptors.request.use((config: ApiRequestConfig) => {
    const url = (config as any).url || 'unknown';
    const effectiveNoAuth = !!config.noAuth || isPublicNoAuthRequest(url);
    const preserveExplicitAuthorization = !!config.preserveExplicitAuthorization;

    // 强制日志：验证拦截器是否被调用
    logger.debug(`[Interceptor START] ${url}`);

    // 检查是否需要阻塞
    if (needBlockRequest && !config.ignoreBlocked) {
        logger.debug(`[Interceptor] Blocking request ${url}, total blocked: ${blockedRequests.length + 1}`);

        // 关键修复：被阻塞的请求等待Token refresh完成后，自动获取最新Token
        return new Promise((resolve, reject) => {
            blockedRequests.push({
                reject,
                resume: () => {
                    // 解除阻塞时，重新从localStorage获取最新Token
                    const latestToken = getCurrentToken();
                    logger.debug(`[Interceptor] Unblocking ${url}, fetching latest token from storage`);

                    if (!config.headers) {
                        config.headers = {} as AxiosRequestHeaders;
                    }

                    if ((effectiveNoAuth || !latestToken) && !preserveExplicitAuthorization) {
                        clearAuthorizationHeader(config.headers);
                    }

                    if (latestToken && !effectiveNoAuth) {
                        // 双重保险：同时更新axios.defaults和config.headers
                        axios.defaults.headers.common['Authorization'] = `Bearer ${latestToken}`;

                        // 确保headers对象存在
                        if (!config.headers) {
                            config.headers = {} as AxiosRequestHeaders;
                        }

                        setAuthorizationHeader(config.headers, latestToken);
                        logger.debug(`[Interceptor] Unblocked ${url} with latest token (defaults+config), length=${latestToken.length}`);
                    } else if (!latestToken && !effectiveNoAuth) {
                        logger.error(`[Interceptor] ✗ Unblocked ${url} but no token in localStorage!`);
                    } else {
                        logger.debug(`[Interceptor] Unblocked ${url} (noAuth request)`);
                    }

                    resolve(config);
                }
            });
        });
    }

    // 正常请求：附加Token
    const token = getCurrentToken();

    // 强制日志：每次请求都记录Token状态
    const tokenStatus = {
        hasToken: !!token,
        tokenLength: token ? token.length : 0,
        localStorage: localStorage.getItem('ebk_user_token') ? 'exists' : 'missing',
        sessionStorage: sessionStorage.getItem('ebk_user_session_token') ? 'exists' : 'missing',
        appLock: isEnableApplicationLock(),
        needBlock: needBlockRequest,
        noAuth: effectiveNoAuth,
        hasHeaders: !!config.headers,
        headersType: typeof config.headers
    };

    logger.debug(`[Interceptor] Request to ${url}:`, JSON.stringify(tokenStatus));

    // 确保headers对象存在
    if (!config.headers) {
        logger.warn(`[Interceptor] config.headers is undefined for ${url}, creating new object`);
        config.headers = {} as AxiosRequestHeaders;
    }

    if ((effectiveNoAuth || !token) && !preserveExplicitAuthorization) {
        clearAuthorizationHeader(config.headers);

        if (!token) {
            delete axios.defaults.headers.common['Authorization'];
            delete axios.defaults.headers.common['authorization'];
        }
    }

    if (token && !effectiveNoAuth) {
        //  终极修复：多种方式确保Authorization头被传递到HTTP请求
        const authValue = `Bearer ${token}`;

        // 方式1: axios.defaults.headers.common
        axios.defaults.headers.common['Authorization'] = authValue;

        // 方式2: 使用辅助函数设置config.headers
        setAuthorizationHeader(config.headers, token);

        // 方式3: 直接赋值多个变体（大小写、属性方式）
        config.headers['Authorization'] = authValue;
        config.headers.Authorization = authValue;
        config.headers['authorization'] = authValue; // 小写版本（某些情况下需要）

        logger.debug(`[Interceptor] Token attached to ${url} (multi-method)`);

        // 验证所有方式
        const defaultsAuth = axios.defaults.headers.common['Authorization'];
        const configAuthCap = config.headers['Authorization'];
        const configAuthLow = config.headers['authorization'];

        logger.debug(`[Interceptor] Verification:`, {
            defaultsAuth: defaultsAuth ? 'SET' : 'NOT_SET',
            configAuthCapital: configAuthCap ? 'SET' : 'NOT_SET',
            configAuthLower: configAuthLow ? 'SET' : 'NOT_SET',
            allKeys: Object.keys(config.headers).join(', ')
        });

        // 验证headers确实被设置（使用多种方式获取）
        const authHeader = (typeof config.headers.get === 'function' ? config.headers.get('Authorization') : null)
                        || config.headers.Authorization
                        || config.headers['Authorization'];

        logger.debug(`[Interceptor] Verifying headers after set:`, {
            hasAuthHeader: !!authHeader,
            allHeaderKeys: Object.keys(config.headers).join(', '),
            headersObjectType: Object.prototype.toString.call(config.headers),
            hasGetMethod: typeof config.headers.get === 'function',
            getMethodResult: typeof config.headers.get === 'function' ? (config.headers.get('Authorization') ? 'HAS_VALUE' : 'NULL') : 'N/A'
        });

        // 关键修复：如果使用get()方法获取不到，说明AxiosHeaders有问题
        if (!authHeader) {
            logger.error(`[Interceptor] CRITICAL: Authorization not found after set! This should never happen!`);
        }
    } else if (!effectiveNoAuth) {
        logger.error(`[Interceptor] ✗ NO TOKEN for ${url}!`, tokenStatus);
    }

    config.headers['X-Timezone-Offset'] = getTimezoneOffsetMinutes();

    // 最终验证：在返回前再次检查Authorization（使用get方法）
    const finalAuthCheck = (typeof config.headers.get === 'function' ? config.headers.get('Authorization') : null)
                        || config.headers.Authorization
                        || config.headers['Authorization'];
    logger.debug(`[Interceptor] Final check before return:`, {
        url: url,
        hasAuth: !!finalAuthCheck
    });

    return config;
}, (error: any) => {
    logger.error('[Interceptor] Request error', error);
    return Promise.reject(error);
});

axios.interceptors.response.use((response: any) => {
    // 记录成功响应的请求config，验证Authorization是否真的发送了
    const url = response.config?.url || 'unknown';
    const authInConfig = response.config?.headers?.Authorization
                      || response.config?.headers?.['Authorization']
                      || (typeof response.config?.headers?.get === 'function' ? response.config.headers.get('Authorization') : null);

    logger.debug(`[Response Success] ${url} - Config had Authorization: ${authInConfig ? 'YES' : 'NO'}`);

    if ('cancelableUuid' in response.config && response.config.cancelableUuid && cancelableRequests[response.config.cancelableUuid as string]) {
        logger.debug('Response canceled by user request, url: ' + response.config.url + ', cancelableUuid: ' + response.config.cancelableUuid);
        delete cancelableRequests[response.config.cancelableUuid as string];
        return Promise.reject({ canceled: true });
    }

    return response;
}, (error: any) => {
    // 记录错误响应的请求config
    if (error.response) {
        const tokenOperation = getTokenResponseFailureOperation(error);

        if (tokenOperation) {
            logger.error(
                '[auth-token] Token operation failed',
                Object.freeze(getTokenFailureLog(error, tokenOperation))
            );
        } else {
            const url = error.response.config?.url || 'unknown';
            const authInConfig = error.response.config?.headers?.Authorization
                              || error.response.config?.headers?.['Authorization']
                              || (typeof error.response.config?.headers?.get === 'function' ? error.response.config.headers.get('Authorization') : null);

            logger.error(`[Response Error] ${error.response.status} ${url} - Config had Authorization: ${authInConfig ? 'YES' : 'NO'}`, {
                allConfigHeaders: error.response.config?.headers ? Object.keys(error.response.config.headers).join(', ') : 'N/A',
                responseMessage: getApiErrorMessage(error) || 'N/A'
            });
        }
    }

    if (error.response?.config && 'cancelableUuid' in error.response.config
        && error.response.config.cancelableUuid
        && cancelableRequests[error.response.config.cancelableUuid]) {
        logger.debug('Response canceled by user request, url: ' + error.response.config.url + ', cancelableUuid: ' + error.response.config.cancelableUuid);
        delete cancelableRequests[error.response.config.cancelableUuid];
        return Promise.reject({ canceled: true });
    }

    if (error.response && !error.response.config.ignoreError) {
        // 处理标准 401 未授权状态
        if (error.response.status === 401) {
            // 仅当当前不在登录页时才刷新
            if (!window.location.hash.includes('/login')) {
                clearCurrentTokenAndUserInfo(false);
                location.reload();
            }
            return Promise.reject({ processed: true });
        }

        if (error.response.data && error.response.data.errorCode) {
            const errorCode = error.response.data.errorCode;

            if (errorCode === 202001 // 未授权访问
                || errorCode === 202002 // 当前 token 无效
                || errorCode === 202003 // 当前 token 已过期
                || errorCode === 202004 // 当前 token 类型无效
                || errorCode === 202005 // 当前 token 需要双因素认证
                || errorCode === 202006 // 当前 token 不需要双因素认证
                || errorCode === 202012 // token 为空
            ) {
                clearCurrentTokenAndUserInfo(false);
                location.reload();
                return Promise.reject({ processed: true });
            }
        }
    }

    return Promise.reject(error);
});

export default {
    setLocale: (locale: string) => {
        axios.defaults.headers.common['Accept-Language'] = locale;
    },
    authorize: (data: UserLoginRequest): ApiResponsePromise<AuthResponse> => {
        return axios.post<ApiResponse<AuthResponse>>('auth/login', data, {
            noAuth: true
        } as ApiRequestConfig);
    },
    authorize2FA: ({ passcode, token }: { passcode: string, token: string }): ApiResponsePromise<AuthResponse> => {
        return axios.post<ApiResponse<AuthResponse>>('2fa/verify', {
            passcode: passcode
        }, {
            noAuth: true,
            preserveExplicitAuthorization: true,
            headers: {
                Authorization: `Bearer ${token}`
            }
        } as ApiRequestConfig);
    },
    authorize2FAByBackupCode: ({ recoveryCode, token }: { recoveryCode: string, token: string }): ApiResponsePromise<AuthResponse> => {
        return axios.post<ApiResponse<AuthResponse>>('2fa/recovery/verify', {
            recoveryCode: recoveryCode
        }, {
            noAuth: true,
            preserveExplicitAuthorization: true,
            headers: {
                Authorization: `Bearer ${token}`
            }
        } as ApiRequestConfig);
    },
    authorizeOAuth2: ({ password, passcode, callbackToken }: { password?: string, passcode?: string, callbackToken: string }): ApiResponsePromise<AuthResponse> => {
        const req: OAuth2CallbackLoginRequest = {
            password,
            passcode,
            token: getCurrentToken() || undefined
        };

        return axios.post<ApiResponse<AuthResponse>>('auth/oauth2/authorize', req, {
            noAuth: true,
            preserveExplicitAuthorization: true,
            headers: {
                Authorization: `Bearer ${callbackToken}`
            }
        } as ApiRequestConfig);
    },
    register: (req: UserRegisterRequest): ApiResponsePromise<RegisterResponse> => {
        return axios.post<ApiResponse<RegisterResponse>>('auth/register', req, {
            noAuth: true
        } as ApiRequestConfig);
    },
    verifyEmail: ({ token, requestNewToken }: { token: string, requestNewToken: boolean }): ApiResponsePromise<UserVerifyEmailResponse> => {
        return axios.post<ApiResponse<UserVerifyEmailResponse>>('auth/email/verify', {
            token: token,
            requestNewToken: requestNewToken
        }, {
            noAuth: true,
            ignoreError: true
        } as ApiRequestConfig);
    },
    resendVerifyEmailByUnloginUser: (req: UserResendVerifyEmailRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('auth/email/resend-verification', req, {
            noAuth: true
        } as ApiRequestConfig);
    },
    requestResetPassword: (req: ForgetPasswordRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('auth/password/forgot', req, {
            noAuth: true,
            ignoreError: true
        } as ApiRequestConfig);
    },
    resetPassword: ({ email, token, password }: { email: string, token: string, password: string }): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('auth/password/reset', {
            email: email,
            password: password,
            token: token
        }, {
            noAuth: true,
            ignoreError: true
        } as ApiRequestConfig);
    },
    logout: (): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('auth/logout');
    },
    refreshToken: refreshAuthenticationToken,
    getExternalAuths: (): ApiResponsePromise<UserExternalAuthInfoResponse[]> => {
        return axios.get<ApiResponse<UserExternalAuthInfoResponse[]>>('profile/external-auths');
    },
    unlinkExternalAuth: (req: UserExternalAuthUnlinkRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('profile/external-auths/unlink', req);
    },
    getTokens: (): ApiResponsePromise<TokenInfoResponse[]> => {
        return axios.get<ApiResponse<TokenInfoResponse[]>>('tokens');
    },
    generateAPIToken: (req: TokenGenerateAPIRequest): ApiResponsePromise<TokenGenerateAPIResponse> => {
        return axios.post<ApiResponse<TokenGenerateAPIResponse>>('tokens/api', req);
    },
    generateMCPToken: (req: TokenGenerateMCPRequest): ApiResponsePromise<TokenGenerateMCPResponse> => {
        return axios.post<ApiResponse<TokenGenerateMCPResponse>>('tokens/mcp', req);
    },
    revokeToken: ({ tokenId, ignoreError }: { tokenId: string, ignoreError?: boolean }): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>(`tokens/${tokenId}`, {
            ignoreError: !!ignoreError
        } as ApiRequestConfig);
    },
    revokeAllTokens: (): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>('tokens');
    },
    getProfile: (): ApiResponsePromise<UserProfileResponse> => {
        return axios.get<ApiResponse<UserProfileResponse>>('profile');
    },
    updateProfile: (req: UserProfileUpdateRequest): ApiResponsePromise<UserProfileUpdateResponse> => {
        return axios.put<ApiResponse<UserProfileUpdateResponse>>('profile', req);
    },
    updateAvatar: ({ avatarFile }: { avatarFile: File }): ApiResponsePromise<UserProfileResponse> => {
        return axios.postForm<ApiResponse<UserProfileResponse>>('profile/avatar', {
            avatar: avatarFile
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        });
    },
    removeAvatar: (): ApiResponsePromise<UserProfileResponse> => {
        return axios.delete<ApiResponse<UserProfileResponse>>('profile/avatar');
    },
    resendVerifyEmailByLoginedUser: (): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('profile/email/resend-verification');
    },
    getUserApplicationCloudSettings: (): ApiResponsePromise<ApplicationCloudSetting[] | false> => {
        return axios.get<ApiResponse<ApplicationCloudSetting[] | false>>('profile/cloud-settings');
    },
    updateUserApplicationCloudSettings: (req: UserApplicationCloudSettingsUpdateRequest): ApiResponsePromise<boolean> => {
        return axios.put<ApiResponse<boolean>>('profile/cloud-settings', req);
    },
    disableUserApplicationCloudSettings: (): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>('profile/cloud-settings');
    },
    get2FAStatus: (): ApiResponsePromise<TwoFactorStatusResponse> => {
        return axios.get<ApiResponse<TwoFactorStatusResponse>>('2fa/status');
    },
    enable2FA: (): ApiResponsePromise<TwoFactorEnableResponse> => {
        return axios.post<ApiResponse<TwoFactorEnableResponse>>('2fa/enable/request');
    },
    confirmEnable2FA: (req: TwoFactorEnableConfirmRequest): ApiResponsePromise<TwoFactorEnableConfirmResponse> => {
        return axios.post<ApiResponse<TwoFactorEnableConfirmResponse>>('2fa/enable/confirm', req);
    },
    disable2FA: (req: TwoFactorDisableRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('2fa/disable', req);
    },
    regenerate2FARecoveryCode: (req: TwoFactorRegenerateRecoveryCodeRequest): ApiResponsePromise<TwoFactorEnableConfirmResponse> => {
        return axios.post<ApiResponse<TwoFactorEnableConfirmResponse>>('2fa/recovery/regenerate', req);
    },
    getUserDataStatistics: (): ApiResponsePromise<DataStatisticsResponse> => {
        return axios.get<ApiResponse<DataStatisticsResponse>>('data/statistics');
    },
    getExportedUserData: (fileType: string, req?: ExportTransactionDataRequest): Promise<AxiosResponse<BlobPart>> => {
        let params = '';

        if (req) {
            const amountFilterCents = encodeURIComponent(req.amountFilterCents);
            const keyword = encodeURIComponent(req.keyword);
            params = `max_time=${req.maxTime}&min_time=${req.minTime}&type=${req.type}&category_ids=${req.categoryIds}&account_ids=${req.accountIds}&tag_ids=${req.tagIds}&tag_filter_type=${req.tagFilterType}&amount_filter_cents=${amountFilterCents}&keyword=${keyword}`;
        } else {
            params = 'max_time=0&min_time=0&type=0&category_ids=&account_ids=&tag_ids=&tag_filter_type=0&amount_filter_cents=&keyword=';
        }

        if (fileType === 'csv') {
            return axios.get<BlobPart>('data/export.csv?' + params, {
                timeout: DEFAULT_EXPORT_API_TIMEOUT
            } as ApiRequestConfig);
        } else if (fileType === 'tsv') {
            return axios.get<BlobPart>('data/export.tsv?' + params, {
                timeout: DEFAULT_EXPORT_API_TIMEOUT
            } as ApiRequestConfig);
        } else {
            return Promise.reject('Parameter Invalid');
        }
    },
    getExportedSettingsBundle: (): Promise<AxiosResponse<BlobPart>> => {
        return axios.get<BlobPart>('settings/bundle/export', {
            responseType: 'blob',
            timeout: DEFAULT_EXPORT_API_TIMEOUT
        } as ApiRequestConfig);
    },
    getExportedSettingsBundleSection: (
        sectionKey: SettingsBundleSectionKey,
        auth?: SettingsBundleExportAuth
    ): Promise<AxiosResponse<BlobPart>> => {
        const config = {
            responseType: 'blob',
            timeout: DEFAULT_EXPORT_API_TIMEOUT
        } as ApiRequestConfig;
        if (auth?.password) {
            return axios.post<BlobPart>(`settings/bundle/sections/${sectionKey}/export`, auth, config);
        }
        return axios.get<BlobPart>(`settings/bundle/sections/${sectionKey}/export`, config);
    },
    previewImportSettingsBundle: (bundle: unknown): ApiResponsePromise<SettingsBundleImportResult> => {
        return axios.post<ApiResponse<SettingsBundleImportResult>>('settings/bundle/import/preview', bundle);
    },
    previewImportSettingsBundleSection: (
        sectionKey: SettingsBundleSectionKey,
        bundle: unknown
    ): ApiResponsePromise<SettingsBundleImportResult> => {
        return axios.post<ApiResponse<SettingsBundleImportResult>>(
            `settings/bundle/sections/${sectionKey}/import/preview`,
            bundle
        );
    },
    importSettingsBundle: (bundle: unknown): ApiResponsePromise<SettingsBundleImportResult> => {
        return axios.post<ApiResponse<SettingsBundleImportResult>>('settings/bundle/import', bundle);
    },
    importSettingsBundleSection: (
        sectionKey: SettingsBundleSectionKey,
        bundle: unknown
    ): ApiResponsePromise<SettingsBundleImportResult> => {
        return axios.post<ApiResponse<SettingsBundleImportResult>>(
            `settings/bundle/sections/${sectionKey}/import`,
            bundle
        );
    },
    clearAllData: (req: ClearDataRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('data/clear/all', req, {
            timeout: DEFAULT_CLEAR_ALL_TRANSACTIONS_API_TIMEOUT
        } as ApiRequestConfig);
    },
    clearAllTransactions: (req: ClearDataRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('data/clear/transactions', req, {
            timeout: DEFAULT_CLEAR_ALL_TRANSACTIONS_API_TIMEOUT
        } as ApiRequestConfig);
    },
    clearAllTransactionsOfAccount: (req: ClearAccountTransactionsRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>(`accounts/${req.accountId}/transactions/clear`, {
            password: req.password
        }, {
            timeout: DEFAULT_CLEAR_ALL_TRANSACTIONS_API_TIMEOUT
        } as ApiRequestConfig);
    },
    getAllAccounts: ({ visibleOnly }: { visibleOnly: boolean }): ApiResponsePromise<AccountInfoResponse[]> => {
        logger.debug('[getAllAccounts] Making request with visibleOnly=' + visibleOnly);
        return axios.get<ApiResponse<AccountInfoResponse[]>>('accounts?visible_only=' + visibleOnly, {
            headers: {} as AxiosRequestHeaders,  // 显式创建headers对象
            ignoreError: true // 防止全局 401 处理器立即刷新页面
        } as ApiRequestConfig);
    },
    getAccount: ({ id }: { id: string }): ApiResponsePromise<AccountInfoResponse> => {
        return axios.get<ApiResponse<AccountInfoResponse>>('accounts/' + id);
    },
    addAccount: (req: AccountCreateRequest): ApiResponsePromise<AccountInfoResponse> => {
        return axios.post<ApiResponse<AccountInfoResponse>>('accounts', req);
    },
    modifyAccount: (req: AccountModifyRequest): ApiResponsePromise<AccountInfoResponse> => {
        return axios.put<ApiResponse<AccountInfoResponse>>('accounts/' + req.id, req);
    },
    hideAccount: (req: AccountHideRequest): ApiResponsePromise<AccountInfoResponse> => {
        return axios.put<ApiResponse<AccountInfoResponse>>('accounts/' + req.id, {
            hidden: req.hidden
        });
    },
    moveAccount: (req: AccountMoveRequest): ApiResponsePromise<boolean> => {
        return axios.put<ApiResponse<boolean>>('accounts/display-orders', req);
    },
    deleteAccount: (req: AccountDeleteRequest): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>('accounts/' + req.id);
    },
    deleteSubAccount: (req: AccountDeleteRequest): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>('accounts/' + req.id);
    },
    // v6.68: 同步所有账户余额
    syncAllAccountBalances: (): ApiResponsePromise<SyncBalancesResponse> => {
        return axios.post<ApiResponse<SyncBalancesResponse>>('accounts/sync-balances');
    },
    getTransactions: (req: TransactionListByMaxTimeRequest): ApiResponsePromise<TransactionInfoPageWrapperResponse> => {
        return axios.get<ApiResponse<TransactionInfoPageWrapperResponse>>(`bills/?${buildTransactionListQuery(req)}`);
    },
    getAllTransactionsByMonth: (req: TransactionListInMonthByPageRequest): ApiResponsePromise<TransactionInfoPageWrapperResponse2> => {
        const amountFilterCents = encodeURIComponent(req.amountFilterCents);
        const keyword = encodeURIComponent(req.keyword);
        return axios.get<ApiResponse<TransactionInfoPageWrapperResponse2>>(`bills/by-month?year=${req.year}&month=${req.month}&type=${req.type}&categoryIds=${req.categoryIds}&accountIds=${req.accountIds}&tagIds=${req.tagIds}&tagFilterType=${req.tagFilterType}&amountFilterCents=${amountFilterCents}&keyword=${keyword}`);
    },
    getReconciliationStatements: (req: TransactionReconciliationStatementRequest): ApiResponsePromise<TransactionReconciliationStatementResponse> => {
        // 修复：使用正确的后端API路径 /api/bills/reconciliation_statements
        // 添加可选的筛选参数：category_ids, type, keyword
        let url = `bills/reconciliation_statements?account_id=${req.accountId}&start_time=${req.startTime}&end_time=${req.endTime}`;

        // 添加可选筛选参数支持
        // 使用类型安全的方式访问可选属性
        const categoryIds = (req as any).categoryIds;
        const type = (req as any).type;
        const keyword = (req as any).keyword;

        if (categoryIds) {
            url += `&category_ids=${categoryIds}`;
        }
        if (type) {
            url += `&type=${type}`;
        }
        if (keyword) {
            url += `&keyword=${encodeURIComponent(keyword)}`;
        }

        return axios.get<ApiResponse<TransactionReconciliationStatementResponse>>(url);
    },
    getTransactionStatistics: (req: TransactionStatisticRequest): ApiResponsePromise<TransactionStatisticResponse> => {
        const queryParams = [];

        if (isDefined(req.startTime)) {
            queryParams.push(`start_time=${req.startTime}`);
        }

        if (isDefined(req.endTime)) {
            queryParams.push(`end_time=${req.endTime}`);
        }

        if (req.tagIds) {
            queryParams.push(`tag_ids=${req.tagIds}`);
        }

        if (req.tagFilterType) {
            queryParams.push(`tag_filter_type=${req.tagFilterType}`);
        }

        if (req.keyword) {
            queryParams.push(`keyword=${encodeURIComponent(req.keyword)}`);
        }

        return axios.get<ApiResponse<TransactionStatisticResponse>>(`statistics/category-statistics?use_transaction_timezone=${req.useTransactionTimezone}` + (queryParams.length ? '&' + queryParams.join('&') : ''));
    },
    getTransactionStatisticsTrends: (req: TransactionStatisticTrendsRequest): ApiResponsePromise<TransactionStatisticTrendsResponseItem[]> => {
        const queryParams = [];

        if (req.startYearMonth) {
            queryParams.push(`start_year_month=${req.startYearMonth}`);
        }

        if (req.endYearMonth) {
            queryParams.push(`end_year_month=${req.endYearMonth}`);
        }

        if (req.tagIds) {
            queryParams.push(`tag_ids=${req.tagIds}`);
        }

        if (req.tagFilterType) {
            queryParams.push(`tag_filter_type=${req.tagFilterType}`);
        }

        if (req.keyword) {
            queryParams.push(`keyword=${encodeURIComponent(req.keyword)}`);
        }

        return axios.get<ApiResponse<TransactionStatisticTrendsResponseItem[]>>(`statistics/category-statistics/trends?use_transaction_timezone=${req.useTransactionTimezone}` + (queryParams.length ? '&' + queryParams.join('&') : ''));
    },
    getTransactionStatisticsAssetTrends: (req: TransactionStatisticAssetTrendsRequest): ApiResponsePromise<TransactionStatisticAssetTrendsResponseItem[]> => {
        const queryParams = [];

        if (isDefined(req.startTime)) {
            queryParams.push(`start_time=${req.startTime}`);
        }

        if (isDefined(req.endTime)) {
            queryParams.push(`end_time=${req.endTime}`);
        }

        return axios.get<ApiResponse<TransactionStatisticAssetTrendsResponseItem[]>>('statistics/asset-trends' + (queryParams.length ? '?' + queryParams.join('&') : ''));
    },
    getTransactionAmounts: (params: TransactionAmountsRequestParams, excludeAccountIds: string[], excludeCategoryIds: string[]): ApiResponsePromise<TransactionAmountsResponse> => {
        const req = TransactionAmountsRequest.of(params);
        let queryParams = req.buildQuery();

        if (excludeAccountIds && excludeAccountIds.length) {
            queryParams = queryParams + `&exclude_account_ids=${excludeAccountIds.join(',')}`;
        }

        if (excludeCategoryIds && excludeCategoryIds.length) {
            queryParams = queryParams + `&exclude_category_ids=${excludeCategoryIds.join(',')}`;
        }

        return axios.get<ApiResponse<TransactionAmountsResponse>>(`statistics/amounts?${queryParams}`);
    },
    getTransaction: ({ id, withPictures }: { id: string, withPictures: boolean | undefined }): ApiResponsePromise<TransactionInfoResponse> => {
        if (!isDefined(withPictures)) {
            withPictures = true;
        }

        return axios.get<ApiResponse<TransactionInfoResponse>>(`bills/get?id=${id}&with_pictures=${withPictures}&trim_account=true&trim_category=true&trim_tag=true`);
    },
    addTransaction: (req: TransactionCreateRequest): ApiResponsePromise<TransactionInfoResponse> => {
        return axios.post<ApiResponse<TransactionInfoResponse>>('bills', req);
    },
    addTransactions: (req: TransactionImportRequest): ApiResponsePromise<{
        items: TransactionInfoResponse[],
        ids: string[],
        createdCount: number
    }> => {
        return axios.post<ApiResponse<{
            items: TransactionInfoResponse[],
            ids: string[],
            createdCount: number
        }>>('bills/batch', req);
    },
    modifyTransaction: (req: TransactionModifyRequest): ApiResponsePromise<TransactionInfoResponse> => {
        return axios.put<ApiResponse<TransactionInfoResponse>>('bills/' + req.id, req);
    },
    moveAllTransactionsBetweenAccounts: (req: TransactionMoveBetweenAccountsRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>(`accounts/${req.fromAccountId}/transactions/move`, {
            toAccountId: req.toAccountId,
            password: req.password
        });
    },
    deleteTransaction: (req: TransactionDeleteRequest): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>('bills/' + req.id);
    },
    ...importPreviewServices,
    uploadTransactionPicture: ({ pictureFile, clientSessionId }: { pictureFile: File, clientSessionId?: string }): ApiResponsePromise<TransactionPictureInfoBasicResponse> => {
        return axios.postForm<ApiResponse<TransactionPictureInfoBasicResponse>>('bills/pictures', {
            picture: pictureFile,
            clientSessionId: clientSessionId
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    removeUnusedTransactionPicture: (req: TransactionPictureUnusedDeleteRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('bills/pictures/unused', req);
    },
    getAllTransactionCategories: (): ApiResponsePromise<Record<number, TransactionCategoryInfoResponse[]>> => {
        return axios.get<ApiResponse<Record<number, TransactionCategoryInfoResponse[]>>>('categories');
    },
    getTransactionCategory: ({ id }: { id: string }): ApiResponsePromise<TransactionCategoryInfoResponse> => {
        return axios.get<ApiResponse<TransactionCategoryInfoResponse>>('categories/' + id);
    },
    addTransactionCategory: (req: TransactionCategoryCreateRequest): ApiResponsePromise<TransactionCategoryInfoResponse> => {
        return axios.post<ApiResponse<TransactionCategoryInfoResponse>>('categories', req);
    },
    addTransactionCategoryBatch: (req: TransactionCategoryCreateBatchRequest): ApiResponsePromise<Record<number, TransactionCategoryInfoResponse[]>> => {
        return axios.post<ApiResponse<Record<number, TransactionCategoryInfoResponse[]>>>('categories/batch', req);
    },
    modifyTransactionCategory: (req: TransactionCategoryModifyRequest): ApiResponsePromise<TransactionCategoryInfoResponse> => {
        return axios.put<ApiResponse<TransactionCategoryInfoResponse>>('categories/' + req.id, req);
    },
    hideTransactionCategory: (req: TransactionCategoryHideRequest): ApiResponsePromise<boolean> => {
        return axios.put<ApiResponse<boolean>>('categories/' + req.id, {
            visible: !req.hidden
        });
    },
    moveTransactionCategory: (req: TransactionCategoryMoveRequest): ApiResponsePromise<boolean> => {
        return axios.post<ApiResponse<boolean>>('categories/move', req);
    },
    deleteTransactionCategory: (req: TransactionCategoryDeleteRequest): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>('categories/' + req.id);
    },
    exportTransactionCategories: (): ApiResponsePromise<TransactionCategoryInfoResponse[]> => {
        return axios.get<ApiResponse<TransactionCategoryInfoResponse[]>>('categories/export');
    },
    importTransactionCategories: (categories: any[]): ApiResponsePromise<{ imported: number, updated: number }> => {
        return axios.post<ApiResponse<{ imported: number, updated: number }>>('categories/import', { categories });
    },
    getAllTransactionTags: (): ApiResponsePromise<TransactionTagInfoResponse[]> => {
        return axios.get<ApiResponse<TransactionTagInfoResponse[]>>('tags');
    },
    getTransactionTag: ({ id }: { id: string }): ApiResponsePromise<TransactionTagInfoResponse> => {
        return axios.get<ApiResponse<TransactionTagInfoResponse>>('tags/' + id);
    },
    addTransactionTag: (req: TransactionTagCreateRequest): ApiResponsePromise<TransactionTagInfoResponse> => {
        return axios.post<ApiResponse<TransactionTagInfoResponse>>('tags', req);
    },
    addTransactionTagBatch: (req: TransactionTagCreateBatchRequest): ApiResponsePromise<TransactionTagInfoResponse[]> => {
        return axios.post<ApiResponse<TransactionTagInfoResponse[]>>('tags/batch', req);
    },
    modifyTransactionTag: (req: TransactionTagModifyRequest): ApiResponsePromise<TransactionTagInfoResponse> => {
        return axios.put<ApiResponse<TransactionTagInfoResponse>>('tags/' + req.id, req);
    },
    hideTransactionTag: (req: TransactionTagHideRequest): ApiResponsePromise<TransactionTagInfoResponse> => {
        return axios.put<ApiResponse<TransactionTagInfoResponse>>('tags/' + req.id, {
            hidden: req.hidden
        });
    },
    moveTransactionTag: (req: TransactionTagMoveRequest): ApiResponsePromise<boolean> => {
        return axios.put<ApiResponse<boolean>>('tags/display-orders', req);
    },
    deleteTransactionTag: (req: TransactionTagDeleteRequest): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>('tags/' + req.id);
    },
    getAllTransactionTemplates: ({ templateType }: { templateType: number }): ApiResponsePromise<TransactionTemplateInfoResponse[]> => {
        return axios.get<ApiResponse<TransactionTemplateInfoResponse[]>>('templates?templateType=' + templateType);
    },
    getTransactionTemplate: ({ id, templateType }: { id: string, templateType?: number }): ApiResponsePromise<TransactionTemplateInfoResponse> => {
        const query = templateType ? `?templateType=${templateType}` : '';
        return axios.get<ApiResponse<TransactionTemplateInfoResponse>>('templates/' + id + query);
    },
    addTransactionTemplate: (req: TransactionTemplateCreateRequest): ApiResponsePromise<TransactionTemplateInfoResponse> => {
        return axios.post<ApiResponse<TransactionTemplateInfoResponse>>('templates', req);
    },
    modifyTransactionTemplate: (req: TransactionTemplateModifyRequest): ApiResponsePromise<TransactionTemplateInfoResponse> => {
        return axios.put<ApiResponse<TransactionTemplateInfoResponse>>(`templates/${req.id}?templateType=${req.templateType}`, req);
    },
    hideTransactionTemplate: (req: TransactionTemplateHideRequest): ApiResponsePromise<TransactionTemplateInfoResponse> => {
        return axios.put<ApiResponse<TransactionTemplateInfoResponse>>(`templates/${req.id}?templateType=${req.templateType}`, {
            hidden: req.hidden,
            templateType: req.templateType
        });
    },
    moveTransactionTemplate: (req: TransactionTemplateMoveRequest): ApiResponsePromise<boolean> => {
        return axios.put<ApiResponse<boolean>>('templates/display-orders', req);
    },
    deleteTransactionTemplate: (req: TransactionTemplateDeleteRequest): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>(`templates/${req.id}?templateType=${req.templateType}`);
    },
    recognizeReceiptImage: ({ imageFile, cancelableUuid }: { imageFile: File, cancelableUuid?: string }): ApiResponsePromise<RecognizedReceiptImageResponse> => {
        return axios.postForm<ApiResponse<RecognizedReceiptImageResponse>>('ml/receipt-recognition', {
            image: imageFile
        }, {
            timeout: DEFAULT_LLM_API_TIMEOUT,
            cancelableUuid: cancelableUuid
        } as ApiRequestConfig);
    },
    getOCRConfig: (): ApiResponsePromise<OCRConfigResponse> => {
        return axios.get<ApiResponse<OCRConfigResponse>>('ml/receipt-recognition/config');
    },
    updateOCRConfig: (config: OCRConfigUpdateRequest): ApiResponsePromise<OCRConfigResponse> => {
        return axios.put<ApiResponse<OCRConfigResponse>>('ml/receipt-recognition/config', config);
    },
    getLatestExchangeRates: (param: { ignoreError?: boolean, provider?: string }): ApiResponsePromise<LatestExchangeRateResponse> => {
        return axios.get<ApiResponse<LatestExchangeRateResponse>>('statistics/exchange-rates', {
            params: {
                provider: param.provider || 'auto'
            },
            ignoreError: !!param.ignoreError,
            timeout: getExchangeRatesRequestTimeout() || DEFAULT_API_TIMEOUT
        } as ApiRequestConfig);
    },
    updateUserCustomExchangeRate: (req: UserCustomExchangeRateUpdateRequest): ApiResponsePromise<UserCustomExchangeRateUpdateResponse> => {
        return axios.put<ApiResponse<UserCustomExchangeRateUpdateResponse>>('statistics/exchange-rates/custom', req);
    },
    deleteUserCustomExchangeRate: (req: UserCustomExchangeRateDeleteRequest): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>(`statistics/exchange-rates/custom/${req.currency}`);
    },
    getServerVersion: (): ApiResponsePromise<VersionInfo> => {
        return axios.get<ApiResponse<VersionInfo>>('system/version');
    },
    cancelRequest: (cancelableUuid: string) => {
        cancelableRequests[cancelableUuid] = true;
    },
    generateOAuth2LoginUrl: (platform: 'mobile' | 'desktop', clientSessionId: string): string => {
        return `${getBasePath()}/oauth2/login?platform=${platform}&client_session_id=${clientSessionId}`;
    },
    generateOAuth2LinkUrl: (platform: 'mobile' | 'desktop', clientSessionId: string): string => {
        return `${getBasePath()}/oauth2/login?platform=${platform}&client_session_id=${clientSessionId}&token=${getCurrentToken()}`;
    },
    generateQrCodeUrl: (qrCodeName: string): string => {
        return `${getBasePath()}${BASE_QRCODE_PATH}/${qrCodeName}.png`;
    },
    generateMapProxyTileImageUrl: (mapProvider: string, language: string): string => {
        const token = getCurrentToken();
        let url = `${getBasePath()}${BASE_PROXY_URL_PATH}/map/tile/{z}/{x}/{y}.png?provider=${mapProvider}&token=${token}`;

        if (language) {
            url = url + `&language=${language}`;
        }

        return url;
    },
    generateMapProxyAnnotationImageUrl: (mapProvider: string, language: string): string => {
        const token = getCurrentToken();
        let url = `${getBasePath()}${BASE_PROXY_URL_PATH}/map/annotation/{z}/{x}/{y}.png?provider=${mapProvider}&token=${token}`;

        if (language) {
            url = url + `&language=${language}`;
        }

        return url;
    },
    generateGoogleMapJavascriptUrl: (language: string | undefined, callbackFnName: string): string => {
        let url = `${GOOGLE_MAP_JAVASCRIPT_URL}?key=${getGoogleMapAPIKey()}&libraries=core,marker&callback=${callbackFnName}`;

        if (language) {
            url = url + `&language=${language}`;
        }

        return url;
    },
    generateBaiduMapJavascriptUrl: (callbackFnName: string): string => {
        return `${BAIDU_MAP_JAVASCRIPT_URL}&ak=${getBaiduMapAK()}&callback=${callbackFnName}`;
    },
    generateAmapJavascriptUrl: (callbackFnName: string): string => {
        return `${AMAP_JAVASCRIPT_URL}&key=${getAmapApplicationKey()}&plugin=AMap.ToolBar&callback=${callbackFnName}`;
    },
    generateAmapApiInternalProxyUrl: (): string => {
        return `${window.location.origin}${getBasePath()}${BASE_AMAP_API_PROXY_URL_PATH}`;
    },
    getInternalAvatarUrlWithToken(avatarUrl: string, disableBrowserCache?: boolean | string): string {
        if (!avatarUrl) {
            return avatarUrl;
        }

        const params = [];
        params.push('token=' + getCurrentToken());

        if (disableBrowserCache) {
            if (isBoolean(disableBrowserCache)) {
                params.push('_nocache=' + generateRandomUUID());
            } else {
                params.push('_nocache=' + disableBrowserCache);
            }
        }

        if (avatarUrl.indexOf('?') >= 0) {
            return avatarUrl + '&' + params.join('&');
        } else {
            return avatarUrl + '?' + params.join('&');
        }
    },
    getTransactionPictureUrlWithToken(pictureUrl: string, disableBrowserCache?: boolean | string): string {
        if (!pictureUrl) {
            return pictureUrl;
        }

        if (pictureUrl.startsWith('data:')) {
            return pictureUrl;
        }

        const params = [];
        params.push('token=' + getCurrentToken());

        if (disableBrowserCache) {
            if (isBoolean(disableBrowserCache)) {
                params.push('_nocache=' + generateRandomUUID());
            } else {
                params.push('_nocache=' + disableBrowserCache);
            }
        }

        if (pictureUrl.indexOf('?') >= 0) {
            return pictureUrl + '&' + params.join('&');
        } else {
            return pictureUrl + '?' + params.join('&');
        }
    },

    // ============================================================================
    // 预算管理 API (Budget Management)
    // ============================================================================

    /**
     * 获取预算列表
     * @param req 筛选条件
     */
    getAllBudgets: (req?: { type?: number, periodType?: string, enabled?: boolean, category?: string, keyword?: string }): ApiResponsePromise<any> => {
        const queryString = buildBudgetListQuery({
            type: req?.type,
            periodType: req?.periodType,
            enabled: req?.enabled,
            category: req?.category
        });

        return axios.get<ApiResponse<any>>('budgets/' + queryString).then(response => {
            const rawResult = response.data?.result;
            const rawItems = Array.isArray(rawResult)
                ? rawResult
                : Array.isArray(rawResult?.items)
                    ? rawResult.items
                    : [];
            const items = rawItems.map((item: any) => mapRestBudgetToFrontend(item, req?.type));

            return buildApiResponse(response, {
                items,
                count: rawResult?.count ?? items.length
            });
        });
    },

    /**
     * 获取单个预算详情
     * @param id 预算ID
     */
    getBudget: ({ id }: { id: string }): ApiResponsePromise<any> => {
        return axios.get<ApiResponse<any>>(`budgets/${id}`).then(response => {
            return buildApiResponse(response, mapRestBudgetToFrontend(response.data?.result));
        });
    },

    /**
     * 获取预算执行详情（分类维度统计）
     * @param req 查询条件
     */
    getBudgetExecution: (req?: { type?: number, periodType?: string, year?: number, month?: number, quarter?: number, startDate?: string, endDate?: string }): ApiResponsePromise<any> => {
        const queryString = buildBudgetExecutionQuery(req);
        return axios.get<ApiResponse<any>>('budgets/execution' + queryString).then(response => {
            return buildApiResponse(response, mapRestExecutionToFrontend(response.data?.result));
        });
    },

    /**
     * 创建预算历史快照
     * @param req 查询条件
     */
    createBudgetHistorySnapshot: (req?: BudgetHistoryQueryRequest): ApiResponsePromise<any> => {
        return axios.post<ApiResponse<any>>('budgets/history/snapshot', {
            budget_type: req?.type,
            period_type: req?.periodType,
            year: req?.year,
            month: req?.month,
            quarter: req?.quarter,
            start_date: req?.startDate,
            end_date: req?.endDate,
            budget_id: req?.budgetId,
            category_id: req?.categoryId,
            account_ids: req?.accountIds,
            tag_ids: req?.tagIds
        });
    },

    /**
     * 获取预算历史快照
     * @param req 查询条件
     */
    getBudgetHistory: (req?: BudgetHistoryQueryRequest): ApiResponsePromise<any> => {
        const queryString = buildBudgetHistoryQuery(req);
        return axios.get<ApiResponse<any>>('budgets/history' + queryString).then(response => {
            return buildApiResponse(response, mapRestHistoryToFrontend(response.data?.result));
        });
    },

    /**
     * 获取周期预计（基于历史数据预测）
     * @param req 查询条件
     */
    getBudgetForecast: (req?: BudgetForecastQueryRequest): ApiResponsePromise<any> => {
        const queryString = buildBudgetForecastQuery(req);
        return axios.get<ApiResponse<any>>('budgets/forecast' + queryString).then(response => {
            return buildApiResponse(response, mapRestForecastToFrontend(response.data?.result));
        });
    },

    /**
     * 添加预算
     * @param req 预算创建请求
     */
    addBudget: (req: any): ApiResponsePromise<any> => {
        return axios.post<ApiResponse<any>>('budgets/', mapBudgetRequestToRest(req)).then(response => {
            return buildApiResponse(response, mapRestBudgetToFrontend(response.data?.result, req?.type));
        });
    },

    /**
     * 修改预算
     * @param req 预算修改请求
     */
    modifyBudget: (req: any): ApiResponsePromise<any> => {
        return axios.put<ApiResponse<any>>(`budgets/${req.id}`, mapBudgetRequestToRest(req)).then(async response => {
            const detailResponse = await axios.get<ApiResponse<any>>(`budgets/${req.id}`);
            return buildApiResponse(response, mapRestBudgetToFrontend(detailResponse.data?.result, req?.type));
        });
    },

    /**
     * 删除预算
     * @param req 删除请求（包含id）
     */
    deleteBudget: (req: { id: string }): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>(`budgets/${req.id}`);
    },

    /**
     * 导出所有预算
     */
    exportBudgets: (): ApiResponsePromise<any> => {
        return axios.get<ApiResponse<any>>('budgets/export').then(response => {
            const budgets = Array.isArray(response.data?.result)
                ? response.data.result.map((item: any) => mapRestBudgetToFrontend(item))
                : [];
            return buildApiResponse(response, {
                budgets,
                exportedAt: new Date().toISOString()
            });
        });
    },

    /**
     * 导入预算
     * @param req 导入请求
     */
    importBudgets: (req: { budgets: any[], overwriteExisting?: boolean }): ApiResponsePromise<any> => {
        return axios.post<ApiResponse<any>>('budgets/import', req.budgets.map(mapImportedBudgetToRest)).then(response => {
            const result = response.data?.result || {};
            return buildApiResponse(response, {
                importedCount: result.created ?? 0,
                updatedCount: result.updated ?? 0,
                failedCount: result.errors ?? 0,
                errors: result.error_details || []
            });
        });
    },

    // ── Learning Center ──────────────────────────

    getLearningSuggestions: ({ status, limit, offset }: PagedStatusRequest = {}): ApiResponsePromise<LearningSuggestionsResponse> => {
        return axios.get<ApiDataResponse<LearningSuggestionsResponse>>('learning/suggestions', {
            params: { status, limit, offset }
        }).then(response => {
            const normalized = normalizeSuggestionsResponse(response.data?.data);
            return buildApiResponse(response, normalized);
        });
    },

    generateLearningSuggestions: (): ApiResponsePromise<GenerateSuggestionsResponse> => {
        return axios.post<ApiDataResponse<GenerateSuggestionsResponse>>('learning/suggestions/generate').then(response => {
            const normalized = normalizeGenerateResponse(response.data?.data);
            return buildApiResponse(response, normalized);
        });
    },

    acceptLearningSuggestion: ({ suggestionId }: { suggestionId: number }): ApiResponsePromise<any> => {
        return axios.post(`learning/suggestions/${suggestionId}/accept`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    rejectLearningSuggestion: ({ suggestionId }: { suggestionId: number }): ApiResponsePromise<any> => {
        return axios.post(`learning/suggestions/${suggestionId}/reject`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    batchAcceptLearningSuggestions: ({ suggestionIds }: { suggestionIds: number[] }): ApiResponsePromise<BatchAcceptResponse> => {
        return axios.post<ApiDataResponse<BatchAcceptResponse>>('learning/suggestions/batch-accept', {
            suggestionIds
        }).then(response => {
            const normalized = normalizeBatchAcceptResponse(response.data?.data);
            return buildApiResponse(response, normalized);
        });
    },

    getLearningRules: ({ enabledOnly, limit, offset }: LearningRuleListRequest = {}): ApiResponsePromise<LearningRulesResponse> => {
        return axios.get<ApiDataResponse<LearningRulesResponse>>('learning/rules', {
            params: { enabled_only: enabledOnly, limit, offset }
        }).then(response => {
            const normalized = normalizeRulesResponse(response.data?.data);
            return buildApiResponse(response, normalized);
        });
    },

    toggleLearningRule: ({ ruleId, enabled }: { ruleId: number, enabled: boolean }): ApiResponsePromise<any> => {
        return axios.put(`learning/rules/${ruleId}/toggle`, { enabled }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    updateLearningRule: ({ ruleId, ...fields }: { ruleId: number, matchValue?: string, learnedType?: string, learnedCategoryId?: number, enabled?: boolean }): ApiResponsePromise<any> => {
        return axios.put(`learning/rules/${ruleId}`, fields).then(response => {
            return buildApiResponse(response, response.data?.data ?? response.data?.result);
        });
    },

    deleteLearningRule: ({ ruleId }: { ruleId: number }): ApiResponsePromise<any> => {
        return axios.delete(`learning/rules/${ruleId}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── Recurring Detection (周期自动发现) ──────────

    getRecurringSuggestions: ({ status, limit, offset }: PagedStatusRequest = {}): ApiResponsePromise<RecurringSuggestionsResponse> => {
        return axios.get<ApiDataResponse<RecurringSuggestionsResponse>>('recurring/suggestions', {
            params: { status, limit, offset }
        }).then(response => {
            const normalized = normalizeRecurringSuggestionsResponse(response.data?.data);
            return buildApiResponse(response, normalized);
        });
    },

    detectRecurringPatterns: (): ApiResponsePromise<RecurringDetectResponse> => {
        return axios.post<ApiDataResponse<RecurringDetectResponse>>('recurring/suggestions/detect').then(response => {
            const normalized = normalizeDetectResponse(response.data?.data);
            return buildApiResponse(response, normalized);
        });
    },

    acceptRecurringSuggestion: ({ suggestionId }: { suggestionId: number }): ApiResponsePromise<RecurringAcceptResponse> => {
        return axios.post<ApiDataResponse<RecurringAcceptResponse>>(`recurring/suggestions/${suggestionId}/accept`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    rejectRecurringSuggestion: ({ suggestionId }: { suggestionId: number }): ApiResponsePromise<any> => {
        return axios.post(`recurring/suggestions/${suggestionId}/reject`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── Calendar Events (日历/现金流) ──────────

    getCalendarEvents: ({ startDate, endDate }: CalendarEventsRequest): ApiResponsePromise<any> => {
        return axios.get('calendar/events', {
            params: { start_date: startDate, end_date: endDate }
        }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── Net Worth (净资产) ──────────

    getNetWorthSnapshot: (): ApiResponsePromise<any> => {
        return axios.get('networth/snapshot').then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── Rule Center (规则中心) ──────────

    getRulesOverview: (): ApiResponsePromise<any> => {
        return axios.get('rules/overview').then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── Category Rules (分类规则) ──────────

    getCategoryRules: (categoryId?: number): ApiResponsePromise<any> => {
        const params: any = {};
        if (categoryId) params.category_id = categoryId;
        return axios.get('category-rules/', { params }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    createCategoryRule: (data: { category_id: number; name: string; priority: number; rule_expression: string; regex_enabled?: boolean; enabled?: boolean }): ApiResponsePromise<any> => {
        return axios.post('category-rules/', data).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    updateCategoryRule: (id: number, data: Record<string, any>): ApiResponsePromise<any> => {
        return axios.put(`category-rules/${id}`, data).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    deleteCategoryRule: (id: number): ApiResponsePromise<any> => {
        return axios.delete(`category-rules/${id}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    reorderCategoryRules: (ruleIds: number[]): ApiResponsePromise<any> => {
        return axios.post('category-rules/reorder', { rule_ids: ruleIds }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    testCategoryRule: (id: number, text: string): ApiResponsePromise<any> => {
        return axios.post(`category-rules/${id}/test`, { text }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── Account Rules (账户识别规则) ──────────

    getAccountRules: (accountId?: number): ApiResponsePromise<any> => {
        const params: any = {};
        if (accountId) params.account_id = accountId;
        return axios.get('account-rules/', { params }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    createAccountRule: (data: AccountRuleCreateRequest): ApiResponsePromise<any> => {
        return axios.post('account-rules/', data).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    updateAccountRule: (id: number, data: Record<string, any>): ApiResponsePromise<any> => {
        return axios.put(`account-rules/${id}`, data).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    deleteAccountRule: (id: number): ApiResponsePromise<any> => {
        return axios.delete(`account-rules/${id}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    reorderAccountRules: (ruleIds: number[]): ApiResponsePromise<any> => {
        return axios.post('account-rules/reorder', { rule_ids: ruleIds }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    testAccountRule: (id: number, context: Record<string, any>): ApiResponsePromise<any> => {
        return axios.post(`account-rules/${id}/test`, context).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── LLM Learning (LLM 归纳学习) ──────────

    getLLMConfig: (): ApiResponsePromise<any> => {
        return axios.get('llm/config').then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    updateLLMConfig: (config: Record<string, any>): ApiResponsePromise<any> => {
        return axios.post('llm/config', config).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    analyzeLLMTransactions: ({
        billIds,
        limit,
        sessionId,
        previewIds,
        previewUpdates,
        actionScope
    }: AnalyzeLLMTransactionsRequest = {}): ApiResponsePromise<LLMAnalyzeTransactionsResponse> => {
        const requestConfig: AxiosRequestConfig = {
            timeout: DEFAULT_LLM_API_TIMEOUT
        };
        return axios.post('llm/analyze-transactions', {
            bill_ids: billIds,
            limit: limit || 20,
            session_id: sessionId,
            preview_ids: previewIds,
            preview_updates: previewUpdates,
            action_scope: actionScope
        }, requestConfig).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    generateLLMRuleSynthesis: ({ limit }: LLMRuleSynthesisRequest = {}): ApiResponsePromise<LLMRuleSynthesisResponse> => {
        return axios.post('llm/rule-synthesis', {
            limit: limit || 8
        }, { timeout: DEFAULT_LLM_API_TIMEOUT } as any).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getLLMCandidates: (params?: { status?: string; type?: string; limit?: number; offset?: number }): ApiResponsePromise<any> => {
        return axios.get('llm/candidates', { params }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    acceptLLMCandidate: (candidateId: number): ApiResponsePromise<any> => {
        return axios.post(`llm/candidates/${candidateId}/accept`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    rejectLLMCandidate: (candidateId: number): ApiResponsePromise<any> => {
        return axios.post(`llm/candidates/${candidateId}/reject`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── LLM Multi-Config (LLM 多配置管理) ──────────

    getLLMConfigs: (): ApiResponsePromise<any> => {
        return axios.get('llm/configs').then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    createLLMConfig: (config: CreateLLMConfigRequest): ApiResponsePromise<any> => {
        return axios.post('llm/configs', config).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    updateLLMSavedConfig: (configId: number, fields: Record<string, any>): ApiResponsePromise<any> => {
        return axios.put(`llm/configs/${configId}`, fields).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    deleteLLMConfig: (configId: number): ApiResponsePromise<any> => {
        return axios.delete(`llm/configs/${configId}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    activateLLMConfig: (configId: number): ApiResponsePromise<any> => {
        return axios.post(`llm/configs/${configId}/activate`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── LLM Preview Recommend (A5 黄色推荐) ──────────

    llmPreviewRecommend: ({ sessionId, previewIds, previewUpdates, actionScope, limit }: LLMPreviewRecommendRequest): ApiResponsePromise<any> => {
        return axios.post('llm/preview-recommend', {
            session_id: sessionId,
            preview_ids: previewIds,
            preview_updates: previewUpdates,
            action_scope: actionScope,
            limit: limit || 20
        }, { timeout: DEFAULT_LLM_API_TIMEOUT } as any).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    llmPreviewRecommendAccept: ({ sessionId, previewId, suggestion }: LLMPreviewRecommendAcceptRequest): ApiResponsePromise<any> => {
        return axios.post('llm/preview-recommend/accept', {
            session_id: sessionId,
            preview_id: previewId,
            suggestion
        }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    llmPreviewRecommendReject: ({ sessionId, previewId, suggestion, userCorrection }: LLMPreviewRecommendRejectRequest): ApiResponsePromise<any> => {
        return axios.post('llm/preview-recommend/reject', {
            session_id: sessionId,
            preview_id: previewId,
            suggestion,
            user_correction: userCorrection
        }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    getLLMMemoryEvents: (params?: { session_id?: string; event_type?: string; limit?: number; offset?: number }): ApiResponsePromise<LLMMemoryEventsResponse> => {
        return axios.get('llm/memory', { params }).then(response => {
            return buildApiResponse(response, {
                events: Array.isArray(response.data?.data)
                    ? response.data.data
                    : [],
                total: Number(response.data?.total || 0)
            });
        });
    },

    // ── Anomaly Insights (异常洞察) ──────────

    getAnomalies: ({ months }: AnomalyListRequest = {}): ApiResponsePromise<any> => {
        return axios.get('insights/anomalies', {
            params: { months }
        }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    }
};
