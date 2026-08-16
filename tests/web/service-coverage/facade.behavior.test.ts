import { beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

type AxiosMethod = jest.Mock<(...args: any[]) => Promise<any>>;
type ServiceFunction = (...args: any[]) => any;

const requestUse = jest.fn();
const responseUse = jest.fn();
const axiosGet: AxiosMethod = jest.fn();
const axiosPost: AxiosMethod = jest.fn();
const axiosPut: AxiosMethod = jest.fn();
const axiosDelete: AxiosMethod = jest.fn();
const axiosPostForm: AxiosMethod = jest.fn();

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

let currentToken = 'access-token';
let exchangeRateTimeout = 0;

jest.mock('axios', () => ({
    __esModule: true,
    default: axiosMock,
    AxiosHeaders: class {}
}));

jest.mock('@/lib/userstate.ts', () => ({
    __esModule: true,
    getCurrentToken: () => currentToken,
    getCurrentRefreshToken: () => 'refresh-token',
    updateCurrentToken: jest.fn(),
    updateCurrentRefreshToken: jest.fn(),
    clearCurrentTokenAndUserInfo: jest.fn()
}));

jest.mock('@/lib/settings.ts', () => ({
    __esModule: true,
    isEnableApplicationLock: () => false
}));

jest.mock('@/lib/common.ts', () => ({
    __esModule: true,
    isDefined: (value: unknown) => value !== undefined && value !== null,
    isBoolean: (value: unknown) => typeof value === 'boolean'
}));

jest.mock('@/lib/server_settings.ts', () => ({
    __esModule: true,
    getGoogleMapAPIKey: () => 'google-key',
    getBaiduMapAK: () => 'baidu-key',
    getAmapApplicationKey: () => 'amap-key',
    getExchangeRatesRequestTimeout: () => exchangeRateTimeout
}));

jest.mock('@/lib/datetime.ts', () => ({
    __esModule: true,
    getTimezoneOffsetMinutes: () => 480
}));

jest.mock('@/lib/misc.ts', () => ({
    __esModule: true,
    generateRandomUUID: () => 'stable-uuid'
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

jest.mock('@/models/transaction.ts', () => ({
    __esModule: true,
    TransactionAmountsRequest: {
        of: () => ({ buildQuery: () => 'base_query=1' })
    }
}));

jest.mock('@/lib/services/transaction.ts', () => ({
    __esModule: true,
    buildTransactionListQuery: () => 'transaction_query=1'
}));

jest.mock('@/lib/services/importPreview.ts', () => ({
    __esModule: true,
    default: {}
}));

jest.mock('@/models/learning_center.ts', () => ({
    __esModule: true,
    normalizeSuggestionsResponse: (value: unknown) => ({ kind: 'suggestions', value }),
    normalizeRulesResponse: (value: unknown) => ({ kind: 'rules', value }),
    normalizeBatchAcceptResponse: (value: unknown) => ({ kind: 'batch', value }),
    normalizeGenerateResponse: (value: unknown) => ({ kind: 'generate', value })
}));

jest.mock('@/models/recurring_suggestion.ts', () => ({
    __esModule: true,
    normalizeSuggestionsResponse: (value: unknown) => ({ kind: 'recurring', value }),
    normalizeDetectResponse: (value: unknown) => ({ kind: 'detect', value })
}));

jest.mock('@/lib/services/budget.ts', () => ({
    __esModule: true,
    buildBudgetExecutionQuery: () => '?execution=1',
    buildBudgetForecastQuery: () => '?forecast=1',
    buildBudgetHistoryQuery: () => '?history=1',
    buildBudgetListQuery: () => '?list=1',
    mapBudgetRequestToRest: (value: unknown) => ({ mappedBudget: value }),
    mapImportedBudgetToRest: (value: unknown) => ({ importedBudget: value }),
    mapRestBudgetToFrontend: (value: unknown, type?: unknown) => ({ value, type }),
    mapRestExecutionToFrontend: (value: unknown) => ({ execution: value }),
    mapRestForecastToFrontend: (value: unknown) => ({ forecast: value }),
    mapRestHistoryToFrontend: (value: unknown) => ({ history: value })
}));

const genericResponse = () => ({
    status: 200,
    data: {
        success: true,
        result: { id: 'result-1', items: [], count: 0 },
        data: { id: 'data-1' },
        total: 0
    }
});

let services: Record<string, ServiceFunction>;

function urls(method: AxiosMethod): string[] {
    return method.mock.calls.map(call => String(call[0]));
}

async function invoke(name: string, ...args: any[]): Promise<any> {
    return services[name]!(...args);
}

beforeAll(async () => {
    (globalThis as unknown as { window: { location: { origin: string; pathname: string } } }).window = {
        location: { origin: 'https://app.example', pathname: '/' }
    };
    services = (await import('@/lib/services.ts')).default as unknown as Record<string, ServiceFunction>;
});

beforeEach(() => {
    currentToken = 'access-token';
    exchangeRateTimeout = 0;
    axiosMock.defaults.headers.common = {};
    for (const method of [axiosGet, axiosPost, axiosPut, axiosDelete, axiosPostForm]) {
        method.mockReset().mockResolvedValue(genericResponse());
    }
});

describe('services authentication, profile, and data-management facade', () => {
    test('preserves every public authentication route and its credential boundary', async () => {
        services['setLocale']!('zh-CN');
        await invoke('authorize', { email: 'user@example.com', password: 'secret' });
        await invoke('authorize2FA', { passcode: '123456', token: 'challenge-token' });
        await invoke('authorize2FAByBackupCode', { recoveryCode: 'backup-code', token: 'challenge-token' });
        await invoke('authorizeOAuth2', { password: 'secret', passcode: '654321', callbackToken: 'callback-token' });
        await invoke('register', { email: 'new@example.com', password: 'secret' });
        await invoke('verifyEmail', { token: 'verify-token', requestNewToken: true });
        await invoke('resendVerifyEmailByUnloginUser', { email: 'user@example.com' });
        await invoke('requestResetPassword', { email: 'user@example.com' });
        await invoke('resetPassword', { email: 'user@example.com', token: 'reset-token', password: 'next-secret' });
        await invoke('logout');

        expect(axiosMock.defaults.headers.common['Accept-Language']).toBe('zh-CN');
        expect(urls(axiosPost)).toEqual([
            'auth/login',
            '2fa/verify',
            '2fa/recovery/verify',
            'auth/oauth2/authorize',
            'auth/register',
            'auth/email/verify',
            'auth/email/resend-verification',
            'auth/password/forgot',
            'auth/password/reset',
            'auth/logout'
        ]);
        expect(axiosPost).toHaveBeenCalledWith('2fa/verify', { passcode: '123456' }, expect.objectContaining({
            noAuth: true,
            preserveExplicitAuthorization: true,
            headers: { Authorization: 'Bearer challenge-token' }
        }));
        expect(axiosPost).toHaveBeenCalledWith('auth/oauth2/authorize', {
            password: 'secret',
            passcode: '654321',
            token: 'access-token'
        }, expect.objectContaining({ headers: { Authorization: 'Bearer callback-token' } }));
    });

    test('preserves token, profile, 2FA, and cloud-setting methods', async () => {
        await invoke('getExternalAuths');
        await invoke('unlinkExternalAuth', { provider: 'github' });
        await invoke('getTokens');
        await invoke('generateAPIToken', { name: 'api' });
        await invoke('generateMCPToken', { name: 'mcp' });
        await invoke('revokeToken', { tokenId: 'token-1', ignoreError: true });
        await invoke('revokeAllTokens');
        await invoke('getProfile');
        await invoke('updateProfile', { nickname: 'New Name' });
        await invoke('updateAvatar', { avatarFile: { name: 'avatar.png' } as File });
        await invoke('removeAvatar');
        await invoke('resendVerifyEmailByLoginedUser');
        await invoke('getUserApplicationCloudSettings');
        await invoke('updateUserApplicationCloudSettings', { enabled: true });
        await invoke('disableUserApplicationCloudSettings');
        await invoke('get2FAStatus');
        await invoke('enable2FA');
        await invoke('confirmEnable2FA', { passcode: '123456' });
        await invoke('disable2FA', { passcode: '123456' });
        await invoke('regenerate2FARecoveryCode', { passcode: '123456' });

        expect(urls(axiosGet)).toEqual([
            'profile/external-auths', 'tokens', 'profile', 'profile/cloud-settings', '2fa/status'
        ]);
        expect(urls(axiosPost)).toEqual([
            'profile/external-auths/unlink', 'tokens/api', 'tokens/mcp',
            'profile/email/resend-verification', '2fa/enable/request', '2fa/enable/confirm',
            '2fa/disable', '2fa/recovery/regenerate'
        ]);
        expect(urls(axiosPut)).toEqual(['profile', 'profile/cloud-settings']);
        expect(urls(axiosDelete)).toEqual([
            'tokens/token-1', 'tokens', 'profile/avatar', 'profile/cloud-settings'
        ]);
        expect(urls(axiosPostForm)).toEqual(['profile/avatar']);
        expect(axiosDelete).toHaveBeenCalledWith('tokens/token-1', { ignoreError: true });
        expect(axiosPostForm).toHaveBeenCalledWith('profile/avatar', {
            avatar: expect.objectContaining({ name: 'avatar.png' })
        }, expect.objectContaining({ timeout: expect.any(Number) }));
    });

    test('builds export, settings bundle, and destructive data requests for all branches', async () => {
        const filter = {
            maxTime: 20,
            minTime: 10,
            type: 1,
            categoryIds: 'cat-1',
            accountIds: 'acc-1',
            tagIds: 'tag-1',
            tagFilterType: 2,
            amountFilterCents: '>= 1200',
            keyword: '早餐 & 咖啡'
        };
        await invoke('getUserDataStatistics');
        await invoke('getExportedUserData', 'csv', filter);
        await invoke('getExportedUserData', 'tsv');
        await expect(invoke('getExportedUserData', 'pdf')).rejects.toBe('Parameter Invalid');
        await invoke('getExportedSettingsBundle');
        await invoke('getExportedSettingsBundleSection', 'accounts');
        await invoke('getExportedSettingsBundleSection', 'accounts', { password: 'step-up' });
        await invoke('previewImportSettingsBundle', { version: 1 });
        await invoke('previewImportSettingsBundleSection', 'accounts', { version: 1 });
        await invoke('importSettingsBundle', { version: 1 });
        await invoke('importSettingsBundleSection', 'accounts', { version: 1 });
        await invoke('clearAllData', { password: 'erase' });
        await invoke('clearAllTransactions', { password: 'erase' });

        expect(urls(axiosGet)).toEqual([
            'data/statistics',
            expect.stringContaining('data/export.csv?max_time=20&min_time=10'),
            expect.stringContaining('data/export.tsv?max_time=0&min_time=0'),
            'settings/bundle/export',
            'settings/bundle/sections/accounts/export'
        ]);
        expect(urls(axiosPost)).toEqual([
            'settings/bundle/sections/accounts/export',
            'settings/bundle/import/preview',
            'settings/bundle/sections/accounts/import/preview',
            'settings/bundle/import',
            'settings/bundle/sections/accounts/import',
            'data/clear/all',
            'data/clear/transactions'
        ]);
        expect(axiosGet.mock.calls[1]?.[0]).toContain('amount_filter_cents=%3E%3D%201200');
        expect(axiosGet.mock.calls[1]?.[0]).toContain('keyword=%E6%97%A9%E9%A4%90%20%26%20%E5%92%96%E5%95%A1');
    });

    test('propagates HTTP and network failures without converting their identity', async () => {
        const httpFailure = { response: { status: 503 }, code: 'ERR_BAD_RESPONSE' };
        axiosGet.mockRejectedValueOnce(httpFailure);
        await expect(invoke('getProfile')).rejects.toBe(httpFailure);

        const networkFailure = new Error('network unavailable');
        axiosPost.mockRejectedValueOnce(networkFailure);
        await expect(invoke('logout')).rejects.toBe(networkFailure);
    });
});

describe('services account, transaction, taxonomy, and media facade', () => {
    test('maps account and transaction commands to the Rust REST routes', async () => {
        await invoke('clearAllTransactionsOfAccount', { accountId: 'acc-1', password: 'step-up' });
        await invoke('getAllAccounts', { visibleOnly: true });
        await invoke('getAccount', { id: 'acc-1' });
        await invoke('addAccount', { name: 'Wallet' });
        await invoke('modifyAccount', { id: 'acc-1', name: 'Cash' });
        await invoke('hideAccount', { id: 'acc-1', hidden: true });
        await invoke('moveAccount', { id: 'acc-1', displayOrder: 2 });
        await invoke('deleteAccount', { id: 'acc-1' });
        await invoke('deleteSubAccount', { id: 'sub-1' });
        await invoke('syncAllAccountBalances');
        await invoke('getTransactions', { maxTime: 1 });
        await invoke('getAllTransactionsByMonth', {
            year: 2026,
            month: 7,
            type: 1,
            categoryIds: 'cat-1',
            accountIds: 'acc-1',
            tagIds: 'tag-1',
            tagFilterType: 2,
            amountFilterCents: '>= 100',
            keyword: '奶茶 & 点心'
        });
        await invoke('getReconciliationStatements', {
            accountId: 'acc-1', startTime: 1, endTime: 2,
            categoryIds: 'cat-1', type: 1, keyword: '咖啡 & 茶'
        });
        await invoke('getReconciliationStatements', { accountId: 'acc-1', startTime: 1, endTime: 2 });
        await invoke('getTransaction', { id: 'bill-1', withPictures: false });
        await invoke('getTransaction', { id: 'bill-2', withPictures: undefined });
        await invoke('addTransaction', { comment: 'one' });
        await invoke('addTransactions', { transactions: [] });
        await invoke('modifyTransaction', { id: 'bill-1', comment: 'next' });
        await invoke('moveAllTransactionsBetweenAccounts', {
            fromAccountId: 'acc-1', toAccountId: 'acc-2', password: 'step-up'
        });
        await invoke('deleteTransaction', { id: 'bill-1' });

        expect(urls(axiosGet)).toEqual([
            'accounts?visible_only=true',
            'accounts/acc-1',
            'bills/?transaction_query=1',
            expect.stringContaining('bills/by-month?year=2026&month=7'),
            expect.stringContaining('bills/reconciliation_statements?account_id=acc-1&start_time=1&end_time=2&category_ids=cat-1&type=1&keyword='),
            'bills/reconciliation_statements?account_id=acc-1&start_time=1&end_time=2',
            'bills/get?id=bill-1&with_pictures=false&trim_account=true&trim_category=true&trim_tag=true',
            'bills/get?id=bill-2&with_pictures=true&trim_account=true&trim_category=true&trim_tag=true'
        ]);
        expect(urls(axiosPost)).toEqual([
            'accounts/acc-1/transactions/clear', 'accounts', 'accounts/sync-balances',
            'bills', 'bills/batch', 'accounts/acc-1/transactions/move'
        ]);
        expect(urls(axiosPut)).toEqual(['accounts/acc-1', 'accounts/acc-1', 'accounts/display-orders', 'bills/bill-1']);
        expect(urls(axiosDelete)).toEqual(['accounts/acc-1', 'accounts/sub-1', 'bills/bill-1']);
    });

    test('builds statistics requests with present and absent optional filters', async () => {
        await invoke('getTransactionStatistics', {
            useTransactionTimezone: true,
            startTime: 0,
            endTime: 2,
            tagIds: 'tag-1',
            tagFilterType: 1,
            keyword: '咖啡 & 茶'
        });
        await invoke('getTransactionStatistics', { useTransactionTimezone: false });
        await invoke('getTransactionStatisticsTrends', {
            useTransactionTimezone: true,
            startYearMonth: '2026-01', endYearMonth: '2026-07',
            tagIds: 'tag-1', tagFilterType: 1, keyword: 'breakfast'
        });
        await invoke('getTransactionStatisticsTrends', { useTransactionTimezone: false });
        await invoke('getTransactionStatisticsAssetTrends', { startTime: 0, endTime: 2 });
        await invoke('getTransactionStatisticsAssetTrends', {});
        await invoke('getTransactionAmounts', {}, ['acc-1'], ['cat-1']);
        await invoke('getTransactionAmounts', {}, [], []);

        expect(urls(axiosGet)).toEqual([
            expect.stringContaining('statistics/category-statistics?use_transaction_timezone=true&start_time=0&end_time=2&tag_ids=tag-1&tag_filter_type=1&keyword='),
            'statistics/category-statistics?use_transaction_timezone=false',
            expect.stringContaining('statistics/category-statistics/trends?use_transaction_timezone=true&start_year_month=2026-01&end_year_month=2026-07'),
            'statistics/category-statistics/trends?use_transaction_timezone=false',
            'statistics/asset-trends?start_time=0&end_time=2',
            'statistics/asset-trends',
            'statistics/amounts?base_query=1&exclude_account_ids=acc-1&exclude_category_ids=cat-1',
            'statistics/amounts?base_query=1'
        ]);
    });

    test('preserves category, tag, template, picture, OCR, exchange-rate, and version contracts', async () => {
        const file = { name: 'receipt.png' } as File;
        await invoke('uploadTransactionPicture', { pictureFile: file, clientSessionId: 'client-1' });
        await invoke('removeUnusedTransactionPicture', { pictureId: 'picture-1' });
        await invoke('getAllTransactionCategories');
        await invoke('getTransactionCategory', { id: 'cat-1' });
        await invoke('addTransactionCategory', { name: 'Food' });
        await invoke('addTransactionCategoryBatch', { categories: [] });
        await invoke('modifyTransactionCategory', { id: 'cat-1', name: 'Meals' });
        await invoke('hideTransactionCategory', { id: 'cat-1', hidden: true });
        await invoke('moveTransactionCategory', { id: 'cat-1', parentId: 'cat-2' });
        await invoke('deleteTransactionCategory', { id: 'cat-1' });
        await invoke('exportTransactionCategories');
        await invoke('importTransactionCategories', [{ id: 'cat-1' }]);
        await invoke('getAllTransactionTags');
        await invoke('getTransactionTag', { id: 'tag-1' });
        await invoke('addTransactionTag', { name: 'Breakfast' });
        await invoke('addTransactionTagBatch', { tags: [] });
        await invoke('modifyTransactionTag', { id: 'tag-1', name: 'Morning' });
        await invoke('hideTransactionTag', { id: 'tag-1', hidden: true });
        await invoke('moveTransactionTag', { id: 'tag-1', displayOrder: 2 });
        await invoke('deleteTransactionTag', { id: 'tag-1' });
        await invoke('getAllTransactionTemplates', { templateType: 2 });
        await invoke('getTransactionTemplate', { id: 'template-1', templateType: 2 });
        await invoke('getTransactionTemplate', { id: 'template-2' });
        await invoke('addTransactionTemplate', { name: 'Rent' });
        await invoke('modifyTransactionTemplate', { id: 'template-1', templateType: 2 });
        await invoke('hideTransactionTemplate', { id: 'template-1', templateType: 2, hidden: true });
        await invoke('moveTransactionTemplate', { id: 'template-1', displayOrder: 2 });
        await invoke('deleteTransactionTemplate', { id: 'template-1', templateType: 2 });
        await invoke('recognizeReceiptImage', { imageFile: file, cancelableUuid: 'cancel-1' });
        await invoke('getOCRConfig');
        await invoke('updateOCRConfig', { provider: 'tesseract' });
        await invoke('getLatestExchangeRates', { ignoreError: true, provider: 'boc_cn' });
        exchangeRateTimeout = 3456;
        await invoke('getLatestExchangeRates', {});
        await invoke('updateUserCustomExchangeRate', { currency: 'USD', rate: 7.2 });
        await invoke('deleteUserCustomExchangeRate', { currency: 'USD' });
        await invoke('getServerVersion');

        expect(urls(axiosGet)).toEqual([
            'categories', 'categories/cat-1', 'categories/export',
            'tags', 'tags/tag-1',
            'templates?templateType=2', 'templates/template-1?templateType=2', 'templates/template-2',
            'ml/receipt-recognition/config',
            'statistics/exchange-rates', 'statistics/exchange-rates', 'system/version'
        ]);
        expect(urls(axiosPost)).toEqual([
            'bills/pictures/unused', 'categories', 'categories/batch', 'categories/move',
            'categories/import', 'tags', 'tags/batch', 'templates'
        ]);
        expect(urls(axiosPut)).toEqual([
            'categories/cat-1', 'categories/cat-1',
            'tags/tag-1', 'tags/tag-1', 'tags/display-orders',
            'templates/template-1?templateType=2', 'templates/template-1?templateType=2',
            'templates/display-orders', 'ml/receipt-recognition/config',
            'statistics/exchange-rates/custom'
        ]);
        expect(urls(axiosDelete)).toEqual([
            'categories/cat-1', 'tags/tag-1', 'templates/template-1?templateType=2',
            'statistics/exchange-rates/custom/USD'
        ]);
        expect(urls(axiosPostForm)).toEqual(['bills/pictures', 'ml/receipt-recognition']);
        expect(axiosGet.mock.calls[9]?.[1]).toEqual(expect.objectContaining({
            params: { provider: 'boc_cn' }, ignoreError: true, timeout: expect.any(Number)
        }));
        expect(axiosGet.mock.calls[10]?.[1]).toEqual(expect.objectContaining({
            params: { provider: 'auto' }, ignoreError: false, timeout: 3456
        }));
    });
});

describe('services URL helpers', () => {
    test('builds OAuth, QR, map, avatar, and picture URLs across cache branches', async () => {
        services['cancelRequest']!('cancel-1');
        expect(services['generateOAuth2LoginUrl']!('desktop', 'client-1')).toBe(
            '/desktop/oauth2/login?platform=desktop&client_session_id=client-1'
        );
        expect(services['generateOAuth2LinkUrl']!('mobile', 'client-2')).toBe(
            '/desktop/oauth2/login?platform=mobile&client_session_id=client-2&token=access-token'
        );
        expect(services['generateQrCodeUrl']!('login')).toContain('/login.png');
        expect(services['generateMapProxyTileImageUrl']!('osm', '')).not.toContain('&language=');
        expect(services['generateMapProxyTileImageUrl']!('osm', 'zh-CN')).toContain('&language=zh-CN');
        expect(services['generateMapProxyAnnotationImageUrl']!('osm', '')).not.toContain('&language=');
        expect(services['generateMapProxyAnnotationImageUrl']!('osm', 'en')).toContain('&language=en');
        expect(services['generateGoogleMapJavascriptUrl']!(undefined, 'ready')).not.toContain('&language=');
        expect(services['generateGoogleMapJavascriptUrl']!('zh-CN', 'ready')).toContain('&language=zh-CN');
        expect(services['generateBaiduMapJavascriptUrl']!('ready')).toContain('ak=baidu-key');
        expect(services['generateAmapJavascriptUrl']!('ready')).toContain('key=amap-key');
        expect(services['generateAmapApiInternalProxyUrl']!()).toMatch(/^https:\/\/app\.example\/desktop\//);

        expect(services['getInternalAvatarUrlWithToken']!('')).toBe('');
        expect(services['getInternalAvatarUrlWithToken']!('/avatar.png')).toBe('/avatar.png?token=access-token');
        expect(services['getInternalAvatarUrlWithToken']!('/avatar.png?size=small', true)).toBe(
            '/avatar.png?size=small&token=access-token&_nocache=stable-uuid'
        );
        expect(services['getInternalAvatarUrlWithToken']!('/avatar.png', 'cache-key')).toBe(
            '/avatar.png?token=access-token&_nocache=cache-key'
        );
        expect(services['getTransactionPictureUrlWithToken']!('')).toBe('');
        expect(services['getTransactionPictureUrlWithToken']!('data:image/png;base64,abc')).toBe('data:image/png;base64,abc');
        expect(services['getTransactionPictureUrlWithToken']!('/picture.png')).toBe('/picture.png?token=access-token');
        expect(services['getTransactionPictureUrlWithToken']!('/picture.png?thumb=1', true)).toBe(
            '/picture.png?thumb=1&token=access-token&_nocache=stable-uuid'
        );
        expect(services['getTransactionPictureUrlWithToken']!('/picture.png', 'cache-key')).toBe(
            '/picture.png?token=access-token&_nocache=cache-key'
        );
    });
});

describe('services budget, learning, recurring, rules, and LLM facade', () => {
    test('normalizes budget REST responses and maps all budget commands', async () => {
        axiosGet.mockResolvedValueOnce({ ...genericResponse(), data: { success: true, result: [{ id: 1 }] } });
        await invoke('getAllBudgets', { type: 1, periodType: 'month', enabled: true, category: 'cat-1' });
        axiosGet.mockResolvedValueOnce({ ...genericResponse(), data: { success: true, result: { items: [{ id: 2 }] } } });
        await invoke('getAllBudgets');
        await invoke('getBudget', { id: 'budget-1' });
        await invoke('getBudgetExecution', { year: 2026, month: 7 });
        await invoke('createBudgetHistorySnapshot', {
            type: 1, periodType: 'month', year: 2026, month: 7, quarter: 3,
            startDate: '2026-07-01', endDate: '2026-07-31', budgetId: 'budget-1',
            categoryId: 'cat-1', accountIds: ['acc-1'], tagIds: ['tag-1']
        });
        await invoke('getBudgetHistory', { year: 2026 });
        await invoke('getBudgetForecast', { year: 2026 });
        await invoke('addBudget', { type: 1, amountCents: 10000 });
        await invoke('modifyBudget', { id: 'budget-1', type: 1, amountCents: 12000 });
        await invoke('deleteBudget', { id: 'budget-1' });
        axiosGet.mockResolvedValueOnce({ ...genericResponse(), data: { success: true, result: [{ id: 1 }] } });
        await invoke('exportBudgets');
        axiosGet.mockResolvedValueOnce({ ...genericResponse(), data: { success: true, result: null } });
        await invoke('exportBudgets');
        axiosPost.mockResolvedValueOnce({
            ...genericResponse(),
            data: { success: true, result: { created: 2, updated: 1, errors: 1, error_details: ['bad'] } }
        });
        await invoke('importBudgets', { budgets: [{ id: 1 }] });
        axiosPost.mockResolvedValueOnce({ ...genericResponse(), data: { success: true, result: null } });
        await invoke('importBudgets', { budgets: [] });

        expect(urls(axiosGet)).toEqual([
            'budgets/?list=1', 'budgets/?list=1', 'budgets/budget-1',
            'budgets/execution?execution=1', 'budgets/history?history=1',
            'budgets/forecast?forecast=1', 'budgets/budget-1',
            'budgets/export', 'budgets/export'
        ]);
        expect(urls(axiosPost)).toEqual([
            'budgets/history/snapshot', 'budgets/', 'budgets/import', 'budgets/import'
        ]);
        expect(urls(axiosPut)).toEqual(['budgets/budget-1']);
        expect(urls(axiosDelete)).toEqual(['budgets/budget-1']);
        expect(axiosPost).toHaveBeenCalledWith('budgets/history/snapshot', expect.objectContaining({
            budget_type: 1, period_type: 'month', account_ids: ['acc-1'], tag_ids: ['tag-1']
        }));
    });

    test('preserves learning, recurring, calendar, net-worth, and rule-center endpoints', async () => {
        await invoke('getLearningSuggestions', { status: 'pending', limit: 10, offset: 2 });
        await invoke('getLearningSuggestions');
        await invoke('generateLearningSuggestions');
        await invoke('acceptLearningSuggestion', { suggestionId: 1 });
        await invoke('rejectLearningSuggestion', { suggestionId: 2 });
        await invoke('batchAcceptLearningSuggestions', { suggestionIds: [1, 2] });
        await invoke('getLearningRules', { enabledOnly: true, limit: 10, offset: 1 });
        await invoke('getLearningRules');
        await invoke('toggleLearningRule', { ruleId: 1, enabled: false });
        await invoke('updateLearningRule', { ruleId: 1, matchValue: 'coffee', enabled: true });
        axiosPut.mockResolvedValueOnce({ ...genericResponse(), data: { success: true, data: undefined, result: { fallback: true } } });
        await invoke('updateLearningRule', { ruleId: 2, learnedType: 'expense' });
        await invoke('deleteLearningRule', { ruleId: 1 });
        await invoke('getRecurringSuggestions', { status: 'pending', limit: 10, offset: 2 });
        await invoke('getRecurringSuggestions');
        await invoke('detectRecurringPatterns');
        await invoke('acceptRecurringSuggestion', { suggestionId: 1 });
        await invoke('rejectRecurringSuggestion', { suggestionId: 2 });
        await invoke('getCalendarEvents', { startDate: '2026-07-01', endDate: '2026-07-31' });
        await invoke('getNetWorthSnapshot');
        await invoke('getRulesOverview');
        await invoke('getCategoryRules', 3);
        await invoke('getCategoryRules');
        await invoke('createCategoryRule', { category_id: 3, name: 'coffee', priority: 1, rule_expression: 'coffee' });
        await invoke('updateCategoryRule', 1, { enabled: false });
        await invoke('deleteCategoryRule', 1);
        await invoke('reorderCategoryRules', [2, 1]);
        await invoke('testCategoryRule', 1, 'coffee shop');
        await invoke('getAccountRules', 4);
        await invoke('getAccountRules');
        await invoke('createAccountRule', { account_id: 4, name: 'wallet', priority: 1, rule_expression: 'wallet' });
        await invoke('updateAccountRule', 1, { enabled: false });
        await invoke('deleteAccountRule', 1);
        await invoke('reorderAccountRules', [2, 1]);
        await invoke('testAccountRule', 1, { paymentMethod: 'wallet' });

        expect(urls(axiosGet)).toEqual([
            'learning/suggestions', 'learning/suggestions', 'learning/rules', 'learning/rules',
            'recurring/suggestions', 'recurring/suggestions', 'calendar/events',
            'networth/snapshot', 'rules/overview', 'category-rules/', 'category-rules/',
            'account-rules/', 'account-rules/'
        ]);
        expect(axiosGet).toHaveBeenCalledWith('category-rules/', { params: { category_id: 3 } });
        expect(axiosGet).toHaveBeenCalledWith('category-rules/', { params: {} });
        expect(axiosGet).toHaveBeenCalledWith('account-rules/', { params: { account_id: 4 } });
        expect(axiosGet).toHaveBeenCalledWith('account-rules/', { params: {} });
        expect(urls(axiosDelete)).toEqual([
            'learning/rules/1', 'category-rules/1', 'account-rules/1'
        ]);
    });

    test('preserves LLM configuration, analysis, review, memory, and anomaly contracts', async () => {
        await invoke('getLLMConfig');
        await invoke('updateLLMConfig', { provider: 'openai' });
        await invoke('analyzeLLMTransactions', {
            billIds: ['bill-1'], limit: 5, sessionId: 'session-1', previewIds: ['preview-1'],
            previewUpdates: { 'preview-1': { comment: 'coffee' } }, actionScope: 'selected'
        });
        await invoke('analyzeLLMTransactions');
        await invoke('generateLLMRuleSynthesis', { limit: 6 });
        await invoke('generateLLMRuleSynthesis');
        await invoke('getLLMCandidates', { status: 'pending', limit: 10 });
        await invoke('acceptLLMCandidate', 1);
        await invoke('rejectLLMCandidate', 2);
        await invoke('getLLMConfigs');
        await invoke('createLLMConfig', { name: 'primary', provider: 'openai' });
        await invoke('updateLLMSavedConfig', 1, { name: 'renamed' });
        await invoke('deleteLLMConfig', 1);
        await invoke('activateLLMConfig', 2);
        await invoke('testLLMConfig', 3);
        await invoke('llmPreviewRecommend', {
            sessionId: 'session-1', previewIds: ['preview-1'], previewUpdates: {}, actionScope: 'selected', limit: 4
        });
        await invoke('llmPreviewRecommend', { sessionId: 'session-1' });
        await invoke('llmPreviewRecommendAccept', {
            sessionId: 'session-1', previewId: 'preview-1', suggestion: { categoryId: 3 },
            expectedState: { rowVersion: 7 }
        });
        await invoke('llmPreviewRecommendReject', {
            sessionId: 'session-1', previewId: 'preview-1', suggestion: { categoryId: 3 },
            userCorrection: { categoryId: 4 }, expectedState: { rowVersion: 8 }
        });
        axiosGet.mockResolvedValueOnce({
            ...genericResponse(), data: { success: true, data: [{ id: 1 }], total: 3 }
        });
        await invoke('getLLMMemoryEvents', { session_id: 'session-1', limit: 10 });
        axiosGet.mockResolvedValueOnce({
            ...genericResponse(), data: { success: true, data: null, total: undefined }
        });
        await invoke('getLLMMemoryEvents');
        await invoke('getAnomalies', { months: 6 });
        await invoke('getAnomalies');

        expect(urls(axiosGet)).toEqual([
            'llm/config', 'llm/candidates', 'llm/configs',
            'llm/memory', 'llm/memory', 'insights/anomalies', 'insights/anomalies'
        ]);
        expect(urls(axiosPost)).toEqual([
            'llm/config', 'llm/analyze-transactions', 'llm/analyze-transactions',
            'llm/rule-synthesis', 'llm/rule-synthesis',
            'llm/candidates/1/accept', 'llm/candidates/2/reject',
            'llm/configs', 'llm/configs/2/activate', 'llm/configs/3/test',
            'llm/preview-recommend', 'llm/preview-recommend',
            'llm/preview-recommend/accept', 'llm/preview-recommend/reject'
        ]);
        expect(axiosPost).toHaveBeenCalledWith('llm/analyze-transactions', expect.objectContaining({
            bill_ids: ['bill-1'], limit: 5, session_id: 'session-1', action_scope: 'selected'
        }), expect.objectContaining({ timeout: expect.any(Number) }));
        expect(axiosPost).toHaveBeenCalledWith('llm/analyze-transactions', expect.objectContaining({ limit: 20 }), expect.any(Object));
        expect(axiosPost).toHaveBeenCalledWith('llm/preview-recommend', expect.objectContaining({ limit: 20 }), expect.any(Object));
        expect(axiosPost).toHaveBeenCalledWith('llm/preview-recommend/accept', expect.objectContaining({
            expected_state: { rowVersion: 7 }
        }));
        expect(axiosPost).toHaveBeenCalledWith('llm/preview-recommend/reject', expect.objectContaining({
            expected_state: { rowVersion: 8 }
        }));
        expect(urls(axiosPut)).toEqual(['llm/configs/1']);
        expect(urls(axiosDelete)).toEqual(['llm/configs/1']);
    });
});
