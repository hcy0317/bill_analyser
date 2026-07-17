import { jest } from '@jest/globals';

let mockCurrentLanguage = 'en';
let mockLanguageLabel = 'Language';
let mockHasWebAuthnConfig = true;
let mockWebAuthnSupported = true;

const mockSetAmountColor = jest.fn();
const mockSetLanguage = jest.fn((language: string) => {
    mockCurrentLanguage = language;
    return {
        currency: language === 'fr' ? 'EUR' : 'USD',
        firstDayOfWeek: language === 'fr' ? 2 : 1
    };
});
const mockInitLocale = jest.fn((_language?: string, _timeZone?: string) => ({ currency: 'CNY', firstDayOfWeek: 1 }));
const mockGetLatestExchangeRates = jest.fn();
const mockRefreshToken = jest.fn<(...args: any[]) => Promise<any>>();
const mockWebAuthnCompletelySupported = jest.fn<(...args: any[]) => Promise<boolean>>();

const mockRootStore = {
    generateOAuth2LoginUrl: jest.fn((platform: string, sessionId: string) => `/oauth/${platform}/${sessionId}`),
    setNotificationContent: jest.fn(),
    forceLogout: jest.fn()
};
const mockSettingsStore = {
    localeDefaultSettings: { currency: 'USD', firstDayOfWeek: 1 },
    appSettings: {
        autoUpdateExchangeRatesData: true,
        applicationLock: false,
        applicationLockWebAuthn: true,
        timeZone: 'Asia/Shanghai'
    },
    updateLocalizedDefaultSettings: jest.fn((settings: Record<string, unknown>) => {
        Object.assign(mockSettingsStore.localeDefaultSettings, settings);
    }),
    clearAppSettings: jest.fn(),
    setEnableApplicationLock: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.applicationLock = value;
    }),
    setEnableApplicationLockWebAuthn: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.applicationLockWebAuthn = value;
    })
};
const mockUserStore = {
    generateNewUserModel: jest.fn((language: string) => ({
        username: '',
        password: '',
        confirmPassword: '',
        email: '',
        nickname: '',
        language,
        defaultCurrency: 'USD',
        firstDayOfWeek: 1
    })),
    currentUserLanguage: 'zh-Hans',
    currentUserExpenseAmountColor: 'red',
    currentUserIncomeAmountColor: 'green'
};
const mockTokensStore = { refreshTokenAndRevokeOldToken: (...args: unknown[]) => mockRefreshToken(...args) };
const mockTransactionsStore = { initTransactionDraft: jest.fn() };
const mockExchangeRatesStore = { getLatestExchangeRates: (...args: unknown[]) => mockGetLatestExchangeRates(...args) };

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key === 'Language' ? mockLanguageLabel : `translated:${key}`,
        getCurrentLanguageTag: () => mockCurrentLanguage,
        getLanguageInfo: (language: string) => language === 'missing' ? undefined : { displayName: `name:${language}` },
        setLanguage: (language: string) => mockSetLanguage(language),
        initLocale: (language?: string, timeZone?: string) => mockInitLocale(language, timeZone),
        getServerMultiLanguageConfigContent: (content: string) => `localized:${content}`,
        getLocalizedOAuth2LoginText: (provider: string, names: unknown) => `oauth:${provider}:${JSON.stringify(names)}`
    })
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => mockRootStore }));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/token.ts', () => ({ useTokensStore: () => mockTokensStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/stores/exchangeRates.ts', () => ({ useExchangeRatesStore: () => mockExchangeRatesStore }));
jest.mock('@/lib/ui/common.ts', () => ({
    setExpenseAndIncomeAmountColor: (...args: unknown[]) => mockSetAmountColor(...args)
}));
jest.mock('@/lib/server_settings.ts', () => ({
    getOAuth2Provider: () => 'oidc',
    getOIDCCustomDisplayNames: () => ({ en: 'Corporate Login' }),
    getLoginPageTips: () => 'Sign in safely'
}));
jest.mock('@/lib/version.ts', () => ({ getClientDisplayVersion: () => '10.2.0' }));
jest.mock('@/lib/webauthn.ts', () => ({
    isWebAuthnSupported: () => mockWebAuthnSupported,
    isWebAuthnCompletelySupported: () => mockWebAuthnCompletelySupported()
}));
jest.mock('@/lib/userstate.ts', () => ({ hasWebAuthnConfig: () => mockHasWebAuthnConfig }));
jest.mock('@/core/category.ts', () => ({
    CategoryType: { Income: 1, Expense: 2, Transfer: 3 }
}));

import { CategoryType } from '@/core/category.ts';
import { useLoginPageBase } from '@/views/base/LoginPageBase.ts';
import { useSignupPageBase } from '@/views/base/SignupPageBase.ts';
import { useUnlockPageBase } from '@/views/base/UnlockPageBase.ts';
import { useAppLockPageBase } from '@/views/base/settings/AppLockPageBase.ts';

async function flush(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
    await (jest.requireActual('vue') as any).nextTick();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockCurrentLanguage = 'en';
    mockLanguageLabel = 'Language';
    mockHasWebAuthnConfig = true;
    mockWebAuthnSupported = true;
    Object.assign(mockSettingsStore.localeDefaultSettings, { currency: 'USD', firstDayOfWeek: 1 });
    Object.assign(mockSettingsStore.appSettings, {
        autoUpdateExchangeRatesData: true,
        applicationLock: false,
        applicationLockWebAuthn: true,
        timeZone: 'Asia/Shanghai'
    });
    mockRefreshToken.mockResolvedValue({});
    mockWebAuthnCompletelySupported.mockResolvedValue(true);
});

describe('useLoginPageBase', () => {
    test('exposes credential, two-factor, OAuth, version, and tip state', () => {
        const base = useLoginPageBase('desktop');

        expect(base.version).toBe('10.2.0');
        expect(base.inputIsEmpty.value).toBe(true);
        base.username.value = 'alice';
        base.password.value = 'secret';
        expect(base.inputIsEmpty.value).toBe(false);

        expect(base.twoFAInputIsEmpty.value).toBe(true);
        base.passcode.value = '123456';
        expect(base.twoFAInputIsEmpty.value).toBe(false);
        base.twoFAVerifyType.value = 'backupcode';
        expect(base.twoFAInputIsEmpty.value).toBe(true);
        base.backupCode.value = 'backup-code';
        expect(base.twoFAInputIsEmpty.value).toBe(false);

        base.oauth2ClientSessionId.value = 'session-id';
        expect(base.oauth2LoginUrl.value).toBe('/oauth/desktop/session-id');
        expect(base.oauth2LoginDisplayName.value).toContain('Corporate Login');
        expect(base.tips.value).toBe('localized:Sign in safely');
    });

    test('applies authenticated user preferences, exchange refresh, and notification', () => {
        const base = useLoginPageBase('mobile');
        base.doAfterLogin({
            token: 'token',
            need2FA: false,
            user: { language: 'fr', expenseAmountColor: 'orange', incomeAmountColor: 'blue' } as any,
            notificationContent: 'maintenance'
        });

        expect(mockSetLanguage).toHaveBeenCalledWith('fr');
        expect(mockSettingsStore.updateLocalizedDefaultSettings).toHaveBeenCalledWith({ currency: 'EUR', firstDayOfWeek: 2 });
        expect(mockSetAmountColor).toHaveBeenCalledWith('orange', 'blue');
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: true, force: false });
        expect(mockRootStore.setNotificationContent).toHaveBeenCalledWith('maintenance');

        jest.clearAllMocks();
        mockSettingsStore.appSettings.autoUpdateExchangeRatesData = false;
        base.doAfterLogin({ token: 'token', need2FA: false });
        expect(mockSetLanguage).not.toHaveBeenCalled();
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();
        expect(mockRootStore.setNotificationContent).not.toHaveBeenCalled();
    });
});

describe('useSignupPageBase', () => {
    test('localizes language labels and updates only locale-derived defaults', () => {
        mockLanguageLabel = '语言';
        const base = useSignupPageBase();

        expect(base.languageTitle.value).toBe('语言 / Language');
        base.currentLocale.value = 'fr';
        expect(base.user.value).toEqual(expect.objectContaining({
            language: 'fr', defaultCurrency: 'EUR', firstDayOfWeek: 2
        }));

        base.user.value.defaultCurrency = 'JPY';
        base.user.value.firstDayOfWeek = 7;
        base.currentLocale.value = 'missing';
        expect(base.user.value.defaultCurrency).toBe('JPY');
        expect(base.user.value.firstDayOfWeek).toBe(7);

        mockCurrentLanguage = 'missing';
        expect(useSignupPageBase().currentLanguageName.value).toBe('');
        mockCurrentLanguage = 'en';
        expect(useSignupPageBase().currentLanguageName.value).toBe('name:en');

        mockLanguageLabel = 'Language';
        expect(useSignupPageBase().languageTitle.value).toBe('Language');
    });

    test('reports each missing or inconsistent signup field', () => {
        const base = useSignupPageBase();
        const expected = [
            'Username cannot be blank',
            'Password cannot be blank',
            'Password confirmation cannot be blank',
            'Email address cannot be blank',
            'Nickname cannot be blank',
            'Default currency cannot be blank'
        ];

        expect(base.inputIsEmpty.value).toBe(true);
        expect(base.inputEmptyProblemMessage.value).toBe(expected[0]);
        base.user.value.username = 'alice';
        expect(base.inputEmptyProblemMessage.value).toBe(expected[1]);
        base.user.value.password = 'secret';
        expect(base.inputEmptyProblemMessage.value).toBe(expected[2]);
        base.user.value.confirmPassword = 'different';
        expect(base.inputEmptyProblemMessage.value).toBe(expected[3]);
        base.user.value.email = 'alice@example.invalid';
        expect(base.inputEmptyProblemMessage.value).toBe(expected[4]);
        base.user.value.nickname = 'Alice';
        base.user.value.defaultCurrency = '';
        expect(base.inputEmptyProblemMessage.value).toBe(expected[5]);
        base.user.value.defaultCurrency = 'USD';
        expect(base.inputEmptyProblemMessage.value).toBe('');
        expect(base.inputIsEmpty.value).toBe(false);
        expect(base.inputInvalidProblemMessage.value).toBe('Password and password confirmation do not match');
        expect(base.inputIsInvalid.value).toBe(true);
        base.user.value.confirmPassword = 'secret';
        expect(base.inputInvalidProblemMessage.value).toBe('');
        expect(base.inputIsInvalid.value).toBe(false);
    });

    test('names all category families and applies successful signup state', () => {
        const base = useSignupPageBase();
        expect(base.getCategoryTypeName(CategoryType.Income)).toBe('translated:Income Categories');
        expect(base.getCategoryTypeName(CategoryType.Expense)).toBe('translated:Expense Categories');
        expect(base.getCategoryTypeName(CategoryType.Transfer)).toBe('translated:Transfer Categories');
        expect(base.getCategoryTypeName(999 as any)).toBe('translated:Transaction Categories');

        base.doAfterSignupSuccess({
            token: 'token',
            need2FA: false,
            needVerifyEmail: false,
            presetCategoriesSaved: true,
            user: { language: 'fr', expenseAmountColor: 'red', incomeAmountColor: 'green' } as any,
            notificationContent: 'welcome'
        });
        expect(mockSetAmountColor).toHaveBeenCalledWith('red', 'green');
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: true, force: false });
        expect(mockRootStore.setNotificationContent).toHaveBeenCalledWith('welcome');

        jest.clearAllMocks();
        mockSettingsStore.appSettings.autoUpdateExchangeRatesData = false;
        base.doAfterSignupSuccess({
            token: 'token', need2FA: false, needVerifyEmail: false, presetCategoriesSaved: true
        });
        expect(mockSetLanguage).not.toHaveBeenCalled();
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();
        expect(mockRootStore.setNotificationContent).not.toHaveBeenCalled();
    });
});

describe('useUnlockPageBase', () => {
    test('requires all WebAuthn gates and validates six-digit PINs', () => {
        const base = useUnlockPageBase();
        expect(base.version).toBe('10.2.0');
        expect(base.isWebAuthnAvailable.value).toBe(true);
        mockHasWebAuthnConfig = false;
        expect(useUnlockPageBase().isWebAuthnAvailable.value).toBe(false);
        mockHasWebAuthnConfig = true;
        mockWebAuthnSupported = false;
        expect(useUnlockPageBase().isWebAuthnAvailable.value).toBe(false);
        mockSettingsStore.appSettings.applicationLockWebAuthn = false;
        mockWebAuthnSupported = true;
        expect(useUnlockPageBase().isWebAuthnAvailable.value).toBe(false);

        expect(base.isPinCodeValid('')).toBe(false);
        expect(base.isPinCodeValid('12345')).toBe(false);
        expect(base.isPinCodeValid('123456')).toBe(true);
    });

    test('refreshes unlocked state and applies the returned user and notification', async () => {
        mockRefreshToken.mockResolvedValueOnce({
            user: { language: 'fr', expenseAmountColor: 'red', incomeAmountColor: 'green' },
            notificationContent: 'unlocked'
        });
        const base = useUnlockPageBase();
        base.doAfterUnlocked();
        await flush();

        expect(mockTransactionsStore.initTransactionDraft).toHaveBeenCalled();
        expect(mockRefreshToken).toHaveBeenCalled();
        expect(mockSetAmountColor).toHaveBeenCalledWith('red', 'green');
        expect(mockRootStore.setNotificationContent).toHaveBeenCalledWith('unlocked');
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: true, force: false });

        jest.clearAllMocks();
        mockSettingsStore.appSettings.autoUpdateExchangeRatesData = false;
        mockRefreshToken.mockResolvedValueOnce({});
        base.doAfterUnlocked();
        await flush();
        expect(mockSetLanguage).not.toHaveBeenCalled();
        expect(mockRootStore.setNotificationContent).not.toHaveBeenCalled();
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();
    });

    test('relogs in through a clean app-state and locale reset', () => {
        const base = useUnlockPageBase();
        base.doRelogin();

        expect(mockRootStore.forceLogout).toHaveBeenCalled();
        expect(mockSettingsStore.clearAppSettings).toHaveBeenCalled();
        expect(mockInitLocale).toHaveBeenCalledWith('zh-Hans', 'Asia/Shanghai');
        expect(mockSettingsStore.updateLocalizedDefaultSettings).toHaveBeenCalledWith({ currency: 'CNY', firstDayOfWeek: 1 });
        expect(mockSetAmountColor).toHaveBeenCalledWith('red', 'green');
    });
});

describe('useAppLockPageBase', () => {
    test('probes WebAuthn support and delegates both lock settings', async () => {
        const base = useAppLockPageBase();
        expect(base.isSupportedWebAuthn.value).toBe(false);
        await flush();
        expect(base.isSupportedWebAuthn.value).toBe(true);

        expect(base.isEnableApplicationLock.value).toBe(false);
        base.isEnableApplicationLock.value = true;
        expect(mockSettingsStore.setEnableApplicationLock).toHaveBeenCalledWith(true);

        expect(base.isEnableApplicationLockWebAuthn.value).toBe(true);
        base.isEnableApplicationLockWebAuthn.value = false;
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);
    });
});
