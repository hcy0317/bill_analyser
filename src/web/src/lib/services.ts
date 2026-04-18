import axios, { type AxiosRequestConfig, type AxiosRequestHeaders, type AxiosResponse } from 'axios';

import type { ApiResponse } from '@/core/api.ts';

import type {
    ApplicationCloudSetting
} from '@/core/setting.ts';
import type {
    VersionInfo
} from '@/core/version.ts';
import {
    TransactionType
} from '@/core/transaction.ts';

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
import {
    DEFAULT_BUDGET_ALERT_THRESHOLD,
    DEFAULT_BUDGET_ENABLED,
    DEFAULT_BUDGET_FORECAST_HISTORY_PERIODS
} from '@/config/budget.ts';

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
    DataStatisticsResponse
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
    ImportTransactionResponsePageWrapper
} from '@/models/imported_transaction.ts';
import type {
    ImportLearningPromoteResponse,
    ImportLearningSuggestionsResponse
} from '@/models/import_learning.ts';
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
    BillMatchingCandidatesResponse,
    BillMatchingFeedbackResponse,
    BillMatchingPairSummary,
    MatchingPairsResponse,
    ReconcileHistoryResponse
} from '@/models/bill_matching.ts';
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
    BudgetForecastStrategy,
    BudgetPeriodType,
    BudgetType
} from '@/models/budget.ts';

import {
    getCurrentToken,
    getCurrentRefreshToken,
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

// ==== 代码版本标记：2025-11-20-00:05 ====
// 修复：添加initializeAxiosAuth()在main文件中调用
// 确保页面加载时就设置axios.defaults.headers.common['Authorization']
const SERVICES_CODE_VERSION = '2025-11-20-00:05-INIT-AXIOS-AUTH';
logger.info(`[services.ts] Loading version: ${SERVICES_CODE_VERSION}`);

interface ApiRequestConfig extends AxiosRequestConfig {
    headers: AxiosRequestHeaders;  // 移除readonly，允许修改
    readonly noAuth?: boolean;
    readonly ignoreBlocked?: boolean;
    readonly ignoreError?: boolean;
    readonly timeout?: number;
    readonly cancelableUuid?: string;
}

export type ApiResponsePromise<T> = Promise<AxiosResponse<ApiResponse<T>>>;

interface ApiDataResponse<T> {
    success: boolean;
    data: T;
}

type MatchingCandidateActionName = 'accept' | 'reject' | 'clear';

interface MatchingCandidateActionResponse {
    candidateId: string;
    action: string;
    previewId?: number;
    sessionId?: string;
    recurringId?: number;
    preview?: Array<Record<string, unknown>>;
    pair?: Record<string, unknown>;
    bill?: Record<string, unknown>;
}

interface MatchingSessionCandidatesResponse {
    session_id?: string;
    summary?: Record<string, unknown>;
    candidates?: Array<Record<string, unknown>>;
}

interface MatchingPairOperationResponse {
    pair?: BillMatchingPairSummary;
}

interface UpdateImportPreviewItemPayload {
    id: number;
    type?: string;
    amount?: number;
    destinationAmount?: number;
    mainCategory?: string;
    subCategory?: string;
    sourceAccountId?: number | null;
    destinationAccountId?: number | null;
    counterparty?: string;
    paymentMethod?: string;
    description?: string;
    isSelected?: boolean;
}

function buildApiResponse<T>(response: AxiosResponse<any>, result: T): AxiosResponse<ApiResponse<T>> {
    return {
        ...response,
        data: {
            success: response.data?.success ?? false,
            result
        }
    } as AxiosResponse<ApiResponse<T>>;
}

function toBudgetAmountInCents(value: unknown): number {
    const amount = Number(value ?? 0);

    if (Number.isNaN(amount)) {
        return 0;
    }

    return Math.round(amount * 100);
}

function mapRestBudgetToFrontend(item: any, fallbackType = BudgetType.Expense): any {
    const categoryInfo = item?.category_info || {};

    return {
        id: String(item?.id ?? ''),
        name: item?.name || '',
        category: item?.category || '',
        subCategory: item?.subCategory || item?.sub_category || '',
        categoryId: String(item?.categoryId || categoryInfo?.id || ''),
        periodType: item?.periodType || item?.period_type || BudgetPeriodType.Monthly,
        amount: toBudgetAmountInCents(item?.amount ?? item?.budget_amount ?? 0),
        startDate: item?.startDate || item?.start_date || '',
        endDate: item?.endDate || item?.end_date || '',
        alertThreshold: item?.alertThreshold ?? item?.alert_threshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
        enabled: item?.enabled ?? DEFAULT_BUDGET_ENABLED,
        type: item?.type ?? fallbackType,
        createdAt: item?.createdAt || item?.created_at || '',
        updatedAt: item?.updatedAt || item?.updated_at || '',
        spentAmount: item?.spentAmount ?? toBudgetAmountInCents(item?.spent_amount ?? 0),
        remainingAmount: item?.remainingAmount ?? toBudgetAmountInCents(item?.remaining_amount ?? 0),
        executionRate: item?.executionRate ?? item?.execution_rate ?? 0,
        categoryName: item?.categoryName || item?.category || '',
        categoryIcon: item?.categoryIcon || categoryInfo?.icon || '',
        categoryColor: item?.categoryColor || categoryInfo?.color || ''
    };
}

function buildBudgetExecutionQuery(req?: {
    type?: number,
    periodType?: string,
    year?: number,
    month?: number,
    quarter?: number,
    startDate?: string,
    endDate?: string
}): string {
    const queryParams: string[] = [];

    if (req?.type !== undefined) {
        queryParams.push(`budget_type=${req.type}`);
    }
    if (req?.periodType) {
        queryParams.push(`period_type=${req.periodType}`);
    }
    if (req?.year !== undefined) {
        queryParams.push(`year=${req.year}`);
    }
    if (req?.month !== undefined) {
        queryParams.push(`month=${req.month}`);
    }
    if (req?.quarter !== undefined) {
        queryParams.push(`quarter=${req.quarter}`);
    }
    if (req?.startDate) {
        queryParams.push(`start_date=${encodeURIComponent(req.startDate)}`);
    }
    if (req?.endDate) {
        queryParams.push(`end_date=${encodeURIComponent(req.endDate)}`);
    }

    return queryParams.length > 0 ? `?${queryParams.join('&')}` : '';
}

function buildBudgetHistoryQuery(req?: {
    type?: number,
    periodType?: string,
    year?: number,
    month?: number,
    quarter?: number,
    startDate?: string,
    endDate?: string,
    budgetId?: string,
    categoryId?: string,
    accountIds?: string[],
    tagIds?: string[]
}): string {
    const queryParams: string[] = [];

    if (req?.type !== undefined) {
        queryParams.push(`budget_type=${req.type}`);
    }
    if (req?.periodType) {
        queryParams.push(`period_type=${req.periodType}`);
    }
    if (req?.year !== undefined) {
        queryParams.push(`year=${req.year}`);
    }
    if (req?.month !== undefined) {
        queryParams.push(`month=${req.month}`);
    }
    if (req?.quarter !== undefined) {
        queryParams.push(`quarter=${req.quarter}`);
    }
    if (req?.startDate) {
        queryParams.push(`start_date=${encodeURIComponent(req.startDate)}`);
    }
    if (req?.endDate) {
        queryParams.push(`end_date=${encodeURIComponent(req.endDate)}`);
    }
    if (req?.budgetId) {
        queryParams.push(`budget_id=${encodeURIComponent(req.budgetId)}`);
    }
    if (req?.categoryId) {
        queryParams.push(`category_id=${encodeURIComponent(req.categoryId)}`);
    }
    if (req?.accountIds?.length) {
        queryParams.push(`account_ids=${encodeURIComponent(req.accountIds.join(','))}`);
    }
    if (req?.tagIds?.length) {
        queryParams.push(`tag_ids=${encodeURIComponent(req.tagIds.join(','))}`);
    }

    return queryParams.length > 0 ? `?${queryParams.join('&')}` : '';
}

function mapRestExecutionToBudgetList(restResult: any, fallbackType = BudgetType.Expense): any {
    const items = Array.isArray(restResult?.items) ? restResult.items : [];
    const summary = restResult?.summary || {};

    return {
        items: items.map((item: any) => mapRestBudgetToFrontend(item, fallbackType)),
        totalBudget: toBudgetAmountInCents(summary?.total_budget ?? 0),
        totalSpent: toBudgetAmountInCents(summary?.total_spent ?? 0),
        totalRemaining: toBudgetAmountInCents(summary?.total_remaining ?? 0),
        count: summary?.count ?? items.length
    };
}

function mapRestExecutionToFrontend(restResult: any): any {
    const items = Array.isArray(restResult?.items) ? restResult.items : [];
    const summary = restResult?.summary || {};

    return {
        totalBudget: toBudgetAmountInCents(summary?.total_budget ?? 0),
        totalSpent: toBudgetAmountInCents(summary?.total_spent ?? 0),
        totalExecutionRate: summary?.overall_execution_rate ?? 0,
        categories: items.map((item: any) => ({
            budgetId: String(item?.id || ''),
            categoryId: String(item?.category_id || item?.category_info?.id || ''),
            categoryName: item?.sub_category ? `${item.category}-${item.sub_category}` : (item?.category || ''),
            categoryIcon: item?.category_info?.icon || '',
            categoryColor: item?.category_info?.color || '',
            budgetAmount: toBudgetAmountInCents(item?.budget_amount ?? 0),
            spentAmount: toBudgetAmountInCents(item?.spent_amount ?? 0),
            remainingAmount: toBudgetAmountInCents(item?.remaining_amount ?? 0),
            executionRate: item?.execution_rate ?? 0,
            alertThreshold: item?.alert_threshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
            isOverBudget: Number(item?.spent_amount ?? 0) > Number(item?.budget_amount ?? 0),
            alertTriggered: Number(item?.execution_rate ?? 0) >= Number(item?.alert_threshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD)
        })),
        periodStart: restResult?.periodStart || restResult?.period_start || '',
        periodEnd: restResult?.periodEnd || restResult?.period_end || ''
    };
}

function mapRestForecastToFrontend(restResult: any): any {
    const items = Array.isArray(restResult?.items) ? restResult.items : [];
    const summary = restResult?.summary || {};

    return {
        forecasts: items.map((item: any) => {
            const forecastAmount = toBudgetAmountInCents(item?.forecast_amount ?? item?.forecastAmount ?? 0);
            const historicalAverage = toBudgetAmountInCents(item?.average_amount ?? item?.averageAmount ?? 0);
            const currentSpent = toBudgetAmountInCents(
                item?.current_spent ?? item?.periods?.[item?.periods?.length - 1]?.amount ?? item?.total_amount ?? item?.totalAmount ?? 0
            );
            const budgetAmount = toBudgetAmountInCents(item?.budget_amount ?? item?.budgetAmount ?? 0);

            return {
                categoryId: String(item?.category_info?.id || item?.categoryId || ''),
                categoryName: item?.category || item?.categoryName || '',
                historicalAverage,
                currentSpent,
                projectedTotal: forecastAmount,
                budgetAmount,
                projectedOverBudget: !!(item?.projected_over_budget ?? item?.projectedOverBudget ?? (budgetAmount > 0 && forecastAmount > budgetAmount)),
                trend: item?.trend || 'stable',
                samplePeriods: item?.sample_periods ?? item?.samplePeriods ?? 0,
                strategyExplanation: item?.strategy_explanation || item?.strategyExplanation || '',
                backtestMape: item?.backtest_mape ?? item?.backtestMape ?? null,
                confidence: item?.confidence || 'low',
                periods: Array.isArray(item?.periods) ? item.periods.map((period: any) => ({
                    period: period?.period || '',
                    amount: toBudgetAmountInCents(period?.amount ?? 0)
                })) : []
            };
        }),
        periodStart: restResult?.periodStart || restResult?.period_start || '',
        periodEnd: restResult?.periodEnd || restResult?.period_end || '',
        daysRemaining: restResult?.daysRemaining ?? 0,
        daysElapsed: restResult?.daysElapsed ?? 0,
        forecastStrategy: summary?.forecast_strategy || summary?.forecastStrategy || BudgetForecastStrategy.HistoricalAverage,
        historyPeriods: summary?.history_periods ?? summary?.historyPeriods ?? DEFAULT_BUDGET_FORECAST_HISTORY_PERIODS,
        avgBacktestMape: summary?.avg_backtest_mape ?? summary?.avgBacktestMape ?? null
    };
}

function mapRestHistoryToFrontend(restResult: any): any {
    const items = Array.isArray(restResult?.items) ? restResult.items : [];
    const summary = restResult?.summary || {};

    return {
        items: items.map((item: any) => ({
            id: String(item?.id ?? ''),
            budgetId: String(item?.budget_id ?? item?.budgetId ?? ''),
            name: item?.name || '',
            category: item?.category || '',
            subCategory: item?.sub_category || item?.subCategory || '',
            periodType: item?.period_type || item?.periodType || BudgetPeriodType.Monthly,
            periodStart: item?.period_start || item?.periodStart || '',
            periodEnd: item?.period_end || item?.periodEnd || '',
            budgetAmount: toBudgetAmountInCents(item?.budget_amount ?? item?.budgetAmount ?? 0),
            spentAmount: toBudgetAmountInCents(item?.spent_amount ?? item?.spentAmount ?? 0),
            remainingAmount: toBudgetAmountInCents(item?.remaining_amount ?? item?.remainingAmount ?? 0),
            executionRate: item?.execution_rate ?? item?.executionRate ?? 0,
            status: item?.status || '',
            filterSummary: item?.filter_summary || item?.filterSummary || '',
            calculatedAt: item?.calculated_at || item?.calculatedAt || '',
            alertThreshold: item?.alert_threshold ?? item?.alertThreshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
            enabled: item?.enabled ?? DEFAULT_BUDGET_ENABLED
        })),
        count: summary?.count ?? items.length,
        periodStart: summary?.period_start || summary?.periodStart || '',
        periodEnd: summary?.period_end || summary?.periodEnd || ''
    };
}

function mapBudgetRequestToRest(req: any): any {
    return {
        name: req?.name || '',
        category: req?.category || '',
        sub_category: req?.subCategory || '',
        period_type: req?.periodType || BudgetPeriodType.Monthly,
        amount: Number(req?.amount ?? 0) / 100,
        start_date: req?.startDate,
        end_date: req?.endDate,
        alert_threshold: req?.alertThreshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
        enabled: req?.enabled ?? DEFAULT_BUDGET_ENABLED
    };
}

function mapImportedBudgetToRest(budget: any): any {
    const amount = Number(budget?.amount ?? 0);
    const usesFrontendShape = Object.prototype.hasOwnProperty.call(budget || {}, 'periodType')
        || Object.prototype.hasOwnProperty.call(budget || {}, 'subCategory')
        || Object.prototype.hasOwnProperty.call(budget || {}, 'startDate');

    return {
        name: budget?.name || '',
        category: budget?.category || '',
        sub_category: budget?.subCategory || budget?.sub_category || '',
        period_type: budget?.periodType || budget?.period_type || BudgetPeriodType.Monthly,
        amount: usesFrontendShape ? amount / 100 : amount,
        start_date: budget?.startDate || budget?.start_date,
        end_date: budget?.endDate || budget?.end_date,
        alert_threshold: budget?.alertThreshold ?? budget?.alert_threshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
        enabled: budget?.enabled ?? DEFAULT_BUDGET_ENABLED
    };
}

function postMatchingCandidateAction(
    action: MatchingCandidateActionName,
    candidateId: string,
    payload?: Record<string, unknown>
): ApiResponsePromise<MatchingCandidateActionResponse> {
    return axios.post<ApiDataResponse<MatchingCandidateActionResponse>>(
        `matching/candidates/${encodeURIComponent(candidateId)}/${action}`,
        payload ?? {}
    ).then(response => {
        return buildApiResponse(response, response.data?.data);
    });
}

let needBlockRequest = false;
const blockedRequests: (() => void)[] = [];  // 改为无参数函数
const cancelableRequests: Record<string, boolean> = {};

axios.defaults.baseURL = getBasePath() + BASE_API_URL_PATH;
axios.defaults.timeout = DEFAULT_API_TIMEOUT;

// ==== 初始化Authorization ====
// 注意：不在模块加载时调用getCurrentToken，因为可能触发AppLock检查
// 而是在页面加载完成后，由initializeAxiosAuth()函数统一处理
// @version 2025-11-20-00:15-ULTIMATE-FIX
logger.info('[Axios Init] axios.defaults configured, waiting for initializeAxiosAuth() call');

// ==== 全局函数：初始化Axios Authorization（页面加载后调用） ====
export function initializeAxiosAuth(): void {
    logger.info('[initializeAxiosAuth] ★★★ VERSION: 2025-11-20-00:15-ULTIMATE-FIX ★★★');
    const token = getCurrentToken();
    if (token) {
        axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
        logger.info(`[initializeAxiosAuth] Set Authorization header in axios.defaults.headers.common (token length: ${token.length})`);
        logger.info(`[initializeAxiosAuth] Verification: axios.defaults.headers.common['Authorization'] = ${axios.defaults.headers.common['Authorization'] ? 'SET ✓' : 'NOT_SET ✗'}`);
    } else {
        logger.warn('[initializeAxiosAuth] No token found in localStorage');
    }
}

// ==== 全局函数：更新axios.defaults中的Authorization ====
// 在登录后调用此函数，确保后续所有请求都自动带上Authorization
export function updateAxiosAuthorizationHeader(token: string): void {
    if (token) {
        axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
        logger.info(`[updateAxiosAuth] Updated axios.defaults.headers.common['Authorization'] (token length: ${token.length})`);
    } else {
        delete axios.defaults.headers.common['Authorization'];
        logger.info('[updateAxiosAuth] Cleared axios.defaults.headers.common[\'Authorization\']');
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
        logger.debug(`[setAuthorizationHeader] Success, value: ${String(checkValue).substring(0, 30)}...`);
    }
}

// ==== 拦截器注册标记 ====
logger.info(`[services.ts] Registering request interceptor...`);

axios.interceptors.request.use((config: ApiRequestConfig) => {
    const url = (config as any).url || 'unknown';

    // 强制日志：验证拦截器是否被调用
    logger.info(`[Interceptor START] ${url}`);

    // 检查是否需要阻塞
    if (needBlockRequest && !config.ignoreBlocked) {
        logger.info(`[Interceptor] Blocking request ${url}, total blocked: ${blockedRequests.length + 1}`);

        // 关键修复：被阻塞的请求等待Token refresh完成后，自动获取最新Token
        return new Promise(resolve => {
            blockedRequests.push(() => {
                // 解除阻塞时，重新从localStorage获取最新Token
                const latestToken = getCurrentToken();
                logger.info(`[Interceptor] Unblocking ${url}, fetching latest token from storage`);

                if (latestToken && !config.noAuth) {
                    // 双重保险：同时更新axios.defaults和config.headers
                    axios.defaults.headers.common['Authorization'] = `Bearer ${latestToken}`;

                    // 确保headers对象存在
                    if (!config.headers) {
                        config.headers = {} as AxiosRequestHeaders;
                    }

                    setAuthorizationHeader(config.headers, latestToken);
                    logger.info(`[Interceptor] ✓ Unblocked ${url} with latest token (defaults+config), length=${latestToken.length}`);
                } else if (!latestToken && !config.noAuth) {
                    logger.error(`[Interceptor] ✗ Unblocked ${url} but no token in localStorage!`);
                } else {
                    logger.info(`[Interceptor] Unblocked ${url} (noAuth request)`);
                }

                resolve(config);
            });
        });
    }

    // 正常请求：附加Token
    const token = getCurrentToken();

    // 强制日志：每次请求都记录Token状态
    const tokenStatus = {
        hasToken: !!token,
        tokenLength: token ? token.length : 0,
        tokenPreview: token ? `${token.substring(0, 20)}...` : 'null',
        localStorage: localStorage.getItem('ebk_user_token') ? 'exists' : 'missing',
        sessionStorage: sessionStorage.getItem('ebk_user_session_token') ? 'exists' : 'missing',
        appLock: isEnableApplicationLock(),
        needBlock: needBlockRequest,
        noAuth: config.noAuth,
        hasHeaders: !!config.headers,
        headersType: typeof config.headers
    };

    logger.info(`[Interceptor] Request to ${url}:`, JSON.stringify(tokenStatus));

    // 确保headers对象存在
    if (!config.headers) {
        logger.warn(`[Interceptor] config.headers is undefined for ${url}, creating new object`);
        config.headers = {} as AxiosRequestHeaders;
    }

    if (token && !config.noAuth) {
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

        logger.info(`[Interceptor] ✓ Token attached to ${url} (multi-method)`);

        // 验证所有方式
        const defaultsAuth = axios.defaults.headers.common['Authorization'];
        const configAuthCap = config.headers['Authorization'];
        const configAuthLow = config.headers['authorization'];

        logger.info(`[Interceptor] Verification:`, {
            defaultsAuth: defaultsAuth ? 'SET' : 'NOT_SET',
            configAuthCapital: configAuthCap ? 'SET' : 'NOT_SET',
            configAuthLower: configAuthLow ? 'SET' : 'NOT_SET',
            value: configAuthCap ? `${String(configAuthCap).substring(0, 30)}...` : 'MISSING',
            allKeys: Object.keys(config.headers).join(', ')
        });

        // 验证headers确实被设置（使用多种方式获取）
        const authHeader = (typeof config.headers.get === 'function' ? config.headers.get('Authorization') : null)
                        || config.headers.Authorization
                        || config.headers['Authorization'];

        logger.info(`[Interceptor] Verifying headers after set:`, {
            hasAuthHeader: !!authHeader,
            authHeaderValue: authHeader ? `${String(authHeader).substring(0, 30)}...` : 'undefined',
            allHeaderKeys: Object.keys(config.headers).join(', '),
            headersObjectType: Object.prototype.toString.call(config.headers),
            hasGetMethod: typeof config.headers.get === 'function',
            getMethodResult: typeof config.headers.get === 'function' ? (config.headers.get('Authorization') ? 'HAS_VALUE' : 'NULL') : 'N/A'
        });

        // 关键修复：如果使用get()方法获取不到，说明AxiosHeaders有问题
        if (!authHeader) {
            logger.error(`[Interceptor] CRITICAL: Authorization not found after set! This should never happen!`);
        }
    } else if (!config.noAuth) {
        logger.error(`[Interceptor] ✗ NO TOKEN for ${url}!`, tokenStatus);
    }

    config.headers['X-Timezone-Offset'] = getTimezoneOffsetMinutes();

    // 最终验证：在返回前再次检查Authorization（使用get方法）
    const finalAuthCheck = (typeof config.headers.get === 'function' ? config.headers.get('Authorization') : null)
                        || config.headers.Authorization
                        || config.headers['Authorization'];
    logger.info(`[Interceptor] Final check before return:`, {
        url: url,
        hasAuth: !!finalAuthCheck,
        authPreview: finalAuthCheck ? `${String(finalAuthCheck).substring(0, 20)}...` : 'MISSING'
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

    logger.info(`[Response Success] ${url} - Config had Authorization: ${authInConfig ? 'YES' : 'NO'}`);

    if ('cancelableUuid' in response.config && response.config.cancelableUuid && cancelableRequests[response.config.cancelableUuid as string]) {
        logger.debug('Response canceled by user request, url: ' + response.config.url + ', cancelableUuid: ' + response.config.cancelableUuid);
        delete cancelableRequests[response.config.cancelableUuid as string];
        return Promise.reject({ canceled: true });
    }

    return response;
}, (error: any) => {
    // 记录错误响应的请求config
    if (error.response) {
        const url = error.response.config?.url || 'unknown';
        const authInConfig = error.response.config?.headers?.Authorization
                          || error.response.config?.headers?.['Authorization']
                          || (typeof error.response.config?.headers?.get === 'function' ? error.response.config.headers.get('Authorization') : null);

        logger.error(`[Response Error] ${error.response.status} ${url} - Config had Authorization: ${authInConfig ? 'YES' : 'NO'}`, {
            authValue: authInConfig ? `${String(authInConfig).substring(0, 30)}...` : 'NONE',
            allConfigHeaders: error.response.config?.headers ? Object.keys(error.response.config.headers).join(', ') : 'N/A'
        });
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
        return axios.post<ApiResponse<AuthResponse>>('auth/login', data);
    },
    authorize2FA: ({ passcode, token }: { passcode: string, token: string }): ApiResponsePromise<AuthResponse> => {
        return axios.post<ApiResponse<AuthResponse>>('2fa/verify', {
            passcode: passcode
        }, {
            noAuth: true,
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
            headers: {
                Authorization: `Bearer ${callbackToken}`
            }
        } as ApiRequestConfig);
    },
    register: (req: UserRegisterRequest): ApiResponsePromise<RegisterResponse> => {
        return axios.post<ApiResponse<RegisterResponse>>('auth/register', req);
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
        return axios.post<ApiResponse<boolean>>('auth/email/resend-verification', req);
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
    refreshToken: (): ApiResponsePromise<TokenRefreshResponse> => {
        return new Promise((resolve, reject) => {
            const refreshToken = getCurrentRefreshToken();

            logger.info(`[refreshToken] Called, current needBlockRequest=${needBlockRequest}, blockedRequests=${blockedRequests.length}`);

            // 如果没有refreshToken，直接返回错误
            if (!refreshToken) {
                logger.warn('[refreshToken] No refresh token available, clearing block state');
                needBlockRequest = false;
                blockedRequests.length = 0;
                reject({
                    message: 'No refresh token available',
                    noRefreshToken: true
                });
                return;
            }

            logger.info(`[refreshToken] Starting token refresh, currently ${blockedRequests.length} blocked requests`);

            const requestBody = { refreshToken };

            // 关键修复：先发起请求，然后再设置 needBlockRequest
            // 这样 Token refresh 请求本身不会被阻塞标志影响
            const refreshPromise = axios.post<ApiResponse<TokenRefreshResponse>>('tokens/refresh', requestBody, {
                ignoreBlocked: true,
                noAuth: true  // 使用 refreshToken 而非旧 token
            } as ApiRequestConfig);

            // 在请求发出后再设置阻塞标志，防止后续请求干扰
            logger.info('[refreshToken] Setting needBlockRequest=true AFTER request sent');
            needBlockRequest = true;

            refreshPromise.then((response: any) => {
                const data = response.data;
                const newToken = data.result?.newToken;

                if (newToken) {
                    logger.info(`[refreshToken] Token refreshed successfully, unblocking ${blockedRequests.length} requests`);

                    // 关键修复：不传递newToken，让被阻塞的请求自己从localStorage读取
                    blockedRequests.forEach(func => func());
                    blockedRequests.length = 0;
                } else {
                    logger.error('[refreshToken] No newToken in response');
                }

                // 解除阻塞状态
                logger.info('[refreshToken] Clearing needBlockRequest=false after success');
                needBlockRequest = false;

                resolve(response);
            }).catch((error: any) => {
                logger.error('[refreshToken] Failed to refresh token', error);
                logger.info('[refreshToken] Clearing needBlockRequest=false after error');
                needBlockRequest = false;
                blockedRequests.length = 0;
                reject(error);
            });
        });
    },
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
            const amountFilter = encodeURIComponent(req.amountFilter);
            const keyword = encodeURIComponent(req.keyword);
            params = `max_time=${req.maxTime}&min_time=${req.minTime}&type=${req.type}&category_ids=${req.categoryIds}&account_ids=${req.accountIds}&tag_ids=${req.tagIds}&tag_filter_type=${req.tagFilterType}&amount_filter=${amountFilter}&keyword=${keyword}`;
        } else {
            params = 'max_time=0&min_time=0&type=0&category_ids=&account_ids=&tag_ids=&tag_filter_type=0&amount_filter=&keyword=';
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
        logger.info('[getAllAccounts] Making request with visibleOnly=' + visibleOnly);
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
        const amountFilter = encodeURIComponent(req.amountFilter);
        const keyword = encodeURIComponent(req.keyword);
        return axios.get<ApiResponse<TransactionInfoPageWrapperResponse>>(`bills/?max_time=${req.maxTime}&min_time=${req.minTime}&type=${req.type}&categoryIds=${req.categoryIds}&accountIds=${req.accountIds}&tagIds=${req.tagIds}&tagFilterType=${req.tagFilterType}&amountFilter=${amountFilter}&keyword=${keyword}&page_size=${req.count}&page=${req.page}&with_count=${req.withCount}`);
    },
    getAllTransactionsByMonth: (req: TransactionListInMonthByPageRequest): ApiResponsePromise<TransactionInfoPageWrapperResponse2> => {
        const amountFilter = encodeURIComponent(req.amountFilter);
        const keyword = encodeURIComponent(req.keyword);
        return axios.get<ApiResponse<TransactionInfoPageWrapperResponse2>>(`bills/by-month?year=${req.year}&month=${req.month}&type=${req.type}&categoryIds=${req.categoryIds}&accountIds=${req.accountIds}&tagIds=${req.tagIds}&tagFilterType=${req.tagFilterType}&amountFilter=${amountFilter}&keyword=${keyword}`);
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
    parseImportTransaction: ({ fileType, fileEncoding, importFile, columnMapping, transactionTypeMapping, hasHeaderLine, timeFormat, timezoneFormat, amountDecimalSeparator, amountDigitGroupingSymbol, geoSeparator, geoOrder, tagSeparator, delimiter }: { fileType: string, fileEncoding?: string, importFile: File, columnMapping?: Record<number, number>, transactionTypeMapping?: Record<string, TransactionType>, hasHeaderLine?: boolean, timeFormat?: string, timezoneFormat?: string, amountDecimalSeparator?: string, amountDigitGroupingSymbol?: string, geoSeparator?: string, geoOrder?: string, tagSeparator?: string, delimiter?: string }): ApiResponsePromise<ImportTransactionResponsePageWrapper> => {
        let textualColumnMapping: string | undefined = undefined;
        let textualTransactionTypeMapping: string | undefined = undefined;
        let textualHasHeaderLine: string | undefined = undefined;

        if (columnMapping) {
            textualColumnMapping = JSON.stringify(columnMapping);
        }

        if (transactionTypeMapping) {
            textualTransactionTypeMapping = JSON.stringify(transactionTypeMapping);
        }

        if (hasHeaderLine !== undefined) {
            textualHasHeaderLine = hasHeaderLine ? 'true' : 'false';
        }

        return axios.postForm<ApiResponse<ImportTransactionResponsePageWrapper>>('bills/parse_import', {
            fileType: fileType,
            fileEncoding: fileEncoding,
            file: importFile,
            columnMapping: textualColumnMapping,
            transactionTypeMapping: textualTransactionTypeMapping,
            hasHeaderLine: textualHasHeaderLine,
            timeFormat: timeFormat,
            timezoneFormat: timezoneFormat,
            amountDecimalSeparator: amountDecimalSeparator,
            amountDigitGroupingSymbol: amountDigitGroupingSymbol,
            geoSeparator: geoSeparator,
            geoOrder: geoOrder,
            tagSeparator: tagSeparator,
            delimiter: delimiter
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    getImportLearningSuggestions: ({
        sessionId,
        previewUpdates,
        previewIds
    }: {
        sessionId: string,
        previewUpdates?: Array<Record<string, unknown>>,
        previewIds?: number[]
    }): ApiResponsePromise<ImportLearningSuggestionsResponse> => {
        const payload: Record<string, unknown> = {};

        if (previewUpdates !== undefined) {
            payload['preview_updates'] = previewUpdates;
        }
        if (previewIds !== undefined) {
            payload['previewIds'] = previewIds;
        }

        const request = Object.keys(payload).length > 0
            ? axios.post<ApiDataResponse<ImportLearningSuggestionsResponse>>(`bills/import/v2/learning/${sessionId}/suggestions`, payload)
            : axios.get<ApiDataResponse<ImportLearningSuggestionsResponse>>(`bills/import/v2/learning/${sessionId}/suggestions`);

        return request.then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    promoteImportLearning: ({
        sessionId,
        previewUpdates,
        previewIds
    }: {
        sessionId: string,
        previewUpdates?: Array<Record<string, unknown>>,
        previewIds?: number[]
    }): ApiResponsePromise<ImportLearningPromoteResponse> => {
        const payload: Record<string, unknown> = {};

        if (previewUpdates !== undefined) {
            payload['preview_updates'] = previewUpdates;
        }
        if (previewIds !== undefined) {
            payload['previewIds'] = previewIds;
        }

        return axios.post<ApiDataResponse<ImportLearningPromoteResponse>>(`bills/import/v2/learning/${sessionId}/promote`, payload).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    updateImportPreviewItem: ({
        sessionId,
        payload
    }: {
        sessionId: string,
        payload: UpdateImportPreviewItemPayload
    }): ApiResponsePromise<boolean> => {
        return axios.put<{ success?: boolean }>(`bills/import/v2/preview/${encodeURIComponent(sessionId)}/update`, payload).then(response => {
            return buildApiResponse(response, !!response.data?.success);
        });
    },
    getMatchingSessionCandidates: ({
        sessionId
    }: {
        sessionId: string
    }): ApiResponsePromise<MatchingSessionCandidatesResponse> => {
        return axios.get<ApiDataResponse<MatchingSessionCandidatesResponse>>(`matching/candidates?sessionId=${encodeURIComponent(sessionId)}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getMatchingBillCandidates: ({
        billId
    }: {
        billId: string | number
    }): ApiResponsePromise<BillMatchingCandidatesResponse> => {
        return axios.get<ApiDataResponse<BillMatchingCandidatesResponse>>(`matching/candidates?billId=${encodeURIComponent(String(billId))}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getMatchingBillFeedback: ({
        billId
    }: {
        billId: string | number
    }): ApiResponsePromise<BillMatchingFeedbackResponse> => {
        return axios.get<ApiDataResponse<BillMatchingFeedbackResponse>>(`matching/bills/${encodeURIComponent(String(billId))}/feedback`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    acceptMatchingCandidate: ({
        candidateId,
        payload
    }: {
        candidateId: string,
        payload?: Record<string, unknown>
    }): ApiResponsePromise<MatchingCandidateActionResponse> => {
        return postMatchingCandidateAction('accept', candidateId, payload);
    },
    rejectMatchingCandidate: ({
        candidateId,
        payload
    }: {
        candidateId: string,
        payload?: Record<string, unknown>
    }): ApiResponsePromise<MatchingCandidateActionResponse> => {
        return postMatchingCandidateAction('reject', candidateId, payload);
    },
    clearMatchingCandidate: ({
        candidateId,
        payload
    }: {
        candidateId: string,
        payload?: Record<string, unknown>
    }): ApiResponsePromise<MatchingCandidateActionResponse> => {
        return postMatchingCandidateAction('clear', candidateId, payload);
    },
    deleteMatchingPair: ({
        pairId
    }: {
        pairId: string | number
    }): ApiResponsePromise<MatchingPairOperationResponse> => {
        return axios.delete<ApiDataResponse<MatchingPairOperationResponse>>(`matching/pairs/${encodeURIComponent(String(pairId))}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getMatchingPairs: ({
        pairType,
        page,
        pageSize
    }: {
        pairType?: string,
        page?: number,
        pageSize?: number
    } = {}): ApiResponsePromise<MatchingPairsResponse> => {
        const params: Record<string, string> = {};
        if (pairType) params['pair_type'] = pairType;
        if (page !== undefined) params['page'] = String(page);
        if (pageSize !== undefined) params['page_size'] = String(pageSize);
        return axios.get<ApiDataResponse<MatchingPairsResponse>>('matching/pairs', { params }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    reconcileMatchingHistory: ({
        billIds,
        families
    }: {
        billIds: number[],
        families?: string[]
    }): ApiResponsePromise<ReconcileHistoryResponse> => {
        const body: Record<string, unknown> = { billIds };
        if (families && families.length > 0) {
            body['families'] = families;
        }
        return axios.post<ApiDataResponse<ReconcileHistoryResponse>>('matching/reconcile-history', body).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    createManualPair: ({
        billId,
        candidateBillId,
        pairType
    }: {
        billId: number,
        candidateBillId: number,
        pairType?: string
    }): ApiResponsePromise<MatchingPairOperationResponse> => {
        return axios.post<ApiDataResponse<MatchingPairOperationResponse>>('matching/manual-pair', {
            billId,
            candidateBillId,
            pairType: pairType || 'transfer'
        }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getImportConfigs: ({ fileFormat }: { fileFormat?: string } = {}): ApiResponsePromise<any[]> => {
        return axios.get<ApiResponse<any[]>>('bills/import/configs', {
            params: {
                file_format: fileFormat
            }
        });
    },
    previewImportFile: ({ importFile, fileEncoding, delimiter }: { importFile: File, fileEncoding?: string, delimiter?: string }): ApiResponsePromise<any> => {
        return axios.postForm<ApiResponse<any>>('bills/import/preview', {
            file: importFile,
            fileEncoding: fileEncoding,
            delimiter: delimiter
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    previewImportFileFromTemp: ({ tempPath, fileEncoding, delimiter }: { tempPath: string, fileEncoding?: string, delimiter?: string }): ApiResponsePromise<any> => {
        return axios.postForm<ApiResponse<any>>('bills/import/preview', {
            temp_path: tempPath,
            fileEncoding: fileEncoding,
            delimiter: delimiter
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    parseGenericIntoSession: ({ sessionId, tempPath, columnMapping, transactionTypeMapping, hasHeaderLine, timeFormat, timezoneFormat, amountDecimalSeparator, amountDigitGroupingSymbol, delimiter }: {
        sessionId: string;
        tempPath: string;
        columnMapping: Record<string, number>;
        transactionTypeMapping?: Record<string, number>;
        hasHeaderLine?: boolean;
        timeFormat?: string;
        timezoneFormat?: string;
        amountDecimalSeparator?: string;
        amountDigitGroupingSymbol?: string;
        delimiter?: string;
    }): ApiResponsePromise<any> => {
        return axios.post<ApiResponse<any>>('bills/import/v2/parse_generic', {
            session_id: sessionId,
            temp_path: tempPath,
            column_mapping: columnMapping,
            transaction_type_mapping: transactionTypeMapping,
            has_header_line: hasHeaderLine,
            time_format: timeFormat,
            timezone_format: timezoneFormat,
            amount_decimal_separator: amountDecimalSeparator,
            amount_digit_grouping_symbol: amountDigitGroupingSymbol,
            delimiter: delimiter
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    matchImportConfig: ({ fileFormat, headers }: { fileFormat: string, headers: string[] }): ApiResponsePromise<any | null> => {
        return axios.post<ApiResponse<any | null>>('bills/import/configs/match', {
            fileFormat,
            headers
        });
    },
    suggestImportConfig: ({ fileFormat, headers, sampleRows }: { fileFormat: string, headers: string[], sampleRows?: string[][] }): ApiResponsePromise<any> => {
        return axios.post<ApiResponse<any>>('bills/import/configs/suggest', {
            fileFormat,
            headers,
            sampleRows
        });
    },
    saveImportConfig: (req: any): ApiResponsePromise<{ id: number }> => {
        return axios.post<ApiResponse<{ id: number }>>('bills/import/configs', req);
    },
    deleteImportConfig: ({ id }: { id: number | string }): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>(`bills/import/configs/${id}`);
    },
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
    getAllBudgets: (req?: { type?: number, periodType?: string, enabled?: boolean, category?: string, keyword?: string }): ApiResponsePromise<any[]> => {
        const queryString = buildBudgetExecutionQuery({
            type: req?.type,
            periodType: req?.periodType
        });

        return axios.get<ApiResponse<any>>('budgets/execution' + queryString).then(response => {
            return buildApiResponse(response, mapRestExecutionToBudgetList(response.data?.result, req?.type));
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
    createBudgetHistorySnapshot: (req?: {
        type?: number,
        periodType?: string,
        year?: number,
        month?: number,
        quarter?: number,
        startDate?: string,
        endDate?: string,
        budgetId?: string,
        categoryId?: string,
        accountIds?: string[],
        tagIds?: string[]
    }): ApiResponsePromise<any> => {
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
    getBudgetHistory: (req?: {
        type?: number,
        periodType?: string,
        year?: number,
        month?: number,
        quarter?: number,
        startDate?: string,
        endDate?: string,
        budgetId?: string,
        categoryId?: string,
        accountIds?: string[],
        tagIds?: string[]
    }): ApiResponsePromise<any> => {
        const queryString = buildBudgetHistoryQuery(req);
        return axios.get<ApiResponse<any>>('budgets/history' + queryString).then(response => {
            return buildApiResponse(response, mapRestHistoryToFrontend(response.data?.result));
        });
    },

    /**
     * 获取周期预计（基于历史数据预测）
     * @param req 查询条件
     */
    getBudgetForecast: (req?: {
        type?: number,
        periodType?: string,
        year?: number,
        month?: number,
        quarter?: number,
        monthsHistory?: number,
        forecastStrategy?: string,
        startDate?: string,
        endDate?: string,
    }): ApiResponsePromise<any> => {
        const queryParams: string[] = [];

        if (req?.type !== undefined) {
            queryParams.push(`budget_type=${req.type}`);
        }
        if (req?.periodType) {
            queryParams.push(`period_type=${req.periodType}`);
        }
        if (req?.year !== undefined) {
            queryParams.push(`year=${req.year}`);
        }
        if (req?.month !== undefined) {
            queryParams.push(`month=${req.month}`);
        }
        if (req?.quarter !== undefined) {
            queryParams.push(`quarter=${req.quarter}`);
        }
        if (req?.monthsHistory !== undefined) {
            queryParams.push(`months_history=${req.monthsHistory}`);
        }
        if (req?.forecastStrategy) {
            queryParams.push(`forecast_strategy=${encodeURIComponent(req.forecastStrategy)}`);
        }
        if (req?.startDate) {
            queryParams.push(`start_date=${encodeURIComponent(req.startDate)}`);
        }
        if (req?.endDate) {
            queryParams.push(`end_date=${encodeURIComponent(req.endDate)}`);
        }

        const queryString = queryParams.length > 0 ? '?' + queryParams.join('&') : '';
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

    getLearningSuggestions: ({
        status,
        limit,
        offset
    }: {
        status?: string,
        limit?: number,
        offset?: number
    } = {}): ApiResponsePromise<LearningSuggestionsResponse> => {
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

    getLearningRules: ({
        enabledOnly,
        limit,
        offset
    }: {
        enabledOnly?: boolean,
        limit?: number,
        offset?: number
    } = {}): ApiResponsePromise<LearningRulesResponse> => {
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

    deleteLearningRule: ({ ruleId }: { ruleId: number }): ApiResponsePromise<any> => {
        return axios.delete(`learning/rules/${ruleId}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },

    // ── Recurring Detection (周期自动发现) ──────────

    getRecurringSuggestions: ({
        status,
        limit,
        offset
    }: {
        status?: string,
        limit?: number,
        offset?: number
    } = {}): ApiResponsePromise<RecurringSuggestionsResponse> => {
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
    }
};
