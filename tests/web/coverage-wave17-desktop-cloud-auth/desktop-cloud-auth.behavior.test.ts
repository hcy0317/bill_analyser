import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;

let mockDarkTheme = false;
let mockVerifyEmailEnabled = true;
let mockLoggedIn = false;

const mockRouter = { push: jest.fn(), replace: jest.fn() };
const mockThemeName = actualVue.ref('light');
const mockAuthorizeOAuth2 = jest.fn<(...args: any[]) => Promise<any>>();
const mockVerifyEmail = jest.fn<(...args: any[]) => Promise<any>>();
const mockResendVerifyEmail = jest.fn<(...args: any[]) => Promise<any>>();
const mockGetCloudSettings = jest.fn<(...args: any[]) => Promise<any>>();
const mockFullUpdateCloudSettings = jest.fn<(...args: any[]) => Promise<void>>();
const mockDisableCloudSettings = jest.fn<(...args: any[]) => Promise<void>>();
const mockNavigateToHomePage = jest.fn<(...args: any[]) => void>();
const mockDoAfterLogin = jest.fn<(...args: any[]) => void>();
const mockShowMessage = jest.fn<(...args: any[]) => void>();
const mockShowError = jest.fn<(...args: any[]) => void>();

let mockLastLoginBase: any;
let mockLastCloudBase: any;

const cloudCatalog = [
    {
        categoryName: 'Appearance',
        categorySubName: 'Shared',
        items: [
            { settingKey: 'theme', settingName: 'Theme', mobile: true, desktop: true },
            { settingKey: 'fontSize', settingName: 'Font Size', mobile: false, desktop: true }
        ]
    },
    {
        categoryName: 'Regional',
        items: [
            { settingKey: 'language', settingName: 'Language', mobile: true, desktop: false },
            { settingKey: 'currency', settingName: 'Currency', mobile: false, desktop: false }
        ]
    }
];

function createCloudBase(): any {
    const enabledApplicationCloudSettings = actualVue.ref({
        theme: true,
        fontSize: false,
        language: true,
        currency: false
    } as Record<string, boolean>);
    const isEnableCloudSync = actualVue.ref(false);
    const loading = actualVue.ref(false);
    const enabling = actualVue.ref(false);
    const disabling = actualVue.ref(false);

    const isAllSettingsSelected = jest.fn((category: any) => (
        category.items.every((item: any) => !!enabledApplicationCloudSettings.value[item.settingKey])
    ));
    const hasSettingSelectedButNotAllChecked = jest.fn((category: any) => {
        const selected = category.items.filter((item: any) => (
            !!enabledApplicationCloudSettings.value[item.settingKey]
        )).length;
        return selected > 0 && selected < category.items.length;
    });
    const updateSettingsSelected = jest.fn((category: any, selected: boolean) => {
        for (const item of category.items) enabledApplicationCloudSettings.value[item.settingKey] = selected;
    });
    const selectAllSettings = jest.fn(() => {
        for (const category of cloudCatalog) {
            for (const item of category.items) enabledApplicationCloudSettings.value[item.settingKey] = true;
        }
    });
    const selectNoneSettings = jest.fn(() => {
        for (const category of cloudCatalog) {
            for (const item of category.items) enabledApplicationCloudSettings.value[item.settingKey] = false;
        }
    });
    const selectInvertSettings = jest.fn(() => {
        for (const category of cloudCatalog) {
            for (const item of category.items) {
                enabledApplicationCloudSettings.value[item.settingKey] = !enabledApplicationCloudSettings.value[item.settingKey];
            }
        }
    });
    const setUserApplicationCloudSettings = jest.fn((settings: any[] | false) => {
        if (!settings || settings.length === 0) {
            enabledApplicationCloudSettings.value = {};
            isEnableCloudSync.value = false;
            return;
        }
        isEnableCloudSync.value = true;
        for (const setting of settings) enabledApplicationCloudSettings.value[setting.settingKey] = true;
    });

    return {
        ALL_APPLICATION_CLOUD_SETTINGS: cloudCatalog,
        loading,
        enabling,
        disabling,
        enabledApplicationCloudSettings,
        isEnableCloudSync,
        hasEnabledApplicationCloudSettings: actualVue.computed(() => (
            Object.values(enabledApplicationCloudSettings.value).some(Boolean)
        )),
        enabledApplicationCloudSettingKeys: actualVue.computed(() => (
            Object.entries(enabledApplicationCloudSettings.value)
                .filter(([, enabled]) => enabled)
                .map(([key]) => key)
        )),
        isAllSettingsSelected,
        hasSettingSelectedButNotAllChecked,
        updateSettingsSelected,
        selectAllSettings,
        selectNoneSettings,
        selectInvertSettings,
        setUserApplicationCloudSettings
    };
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});
jest.mock('vue-router', () => ({ useRouter: () => mockRouter }));
jest.mock('vuetify', () => ({ useTheme: () => ({ global: { name: mockThemeName } }) }));
jest.mock('vuetify/components/VTextField', () => ({ VTextField: {} }));
jest.mock('@/components/desktop/SnackBar.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return { __esModule: true, default: defineComponent({ name: 'SnackBarStub', setup: () => () => h('stub') }) };
});
jest.mock('@/components/desktop/ConfirmDialog.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return { __esModule: true, default: defineComponent({ name: 'ConfirmDialogStub', setup: () => () => h('stub') }) };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        te: (value: unknown) => `te:${String(value)}`,
        getLocalizedOAuth2ProviderName: (provider: string) => `provider:${provider}`,
        getLocalizedOAuth2LoginText: (provider: string) => `login:${provider}`
    })
}));
jest.mock('@/views/base/LoginPageBase.ts', () => ({
    useLoginPageBase: () => {
        mockLastLoginBase = {
            version: '17.0.0',
            password: actualVue.ref(''),
            loggingInByOAuth2: actualVue.ref(false),
            doAfterLogin: (...args: any[]) => mockDoAfterLogin(...args)
        };
        return mockLastLoginBase;
    }
}));
jest.mock('@/views/base/settings/AppCloudSyncPageBase.ts', () => ({
    useAppCloudSyncBase: () => {
        mockLastCloudBase = createCloudBase();
        return mockLastCloudBase;
    }
}));
jest.mock('@/stores/index.ts', () => ({
    useRootStore: () => ({
        authorizeOAuth2: (...args: any[]) => mockAuthorizeOAuth2(...args),
        verifyEmail: (...args: any[]) => mockVerifyEmail(...args),
        resendVerifyEmailByUnloginUser: (...args: any[]) => mockResendVerifyEmail(...args)
    })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({
        getUserApplicationCloudSettings: (...args: any[]) => mockGetCloudSettings(...args),
        fullUpdateUserApplicationCloudSettings: (...args: any[]) => mockFullUpdateCloudSettings(...args),
        disableUserApplicationCloudSettings: (...args: any[]) => mockDisableCloudSettings(...args)
    })
}));
jest.mock('@/core/theme.ts', () => ({ isDarkApplicationTheme: () => mockDarkTheme }));
jest.mock('@/core/api.ts', () => ({
    buildErrorResponse: (errorCode: number, message: string) => ({ errorCode, message })
}));
jest.mock('@/consts/asset.ts', () => ({ APPLICATION_LOGO_PATH: '/logo.svg' }));
jest.mock('@/consts/api.ts', () => ({
    KnownErrorCode: {
        UserEmailNotVerified: 1001,
        TwoFactorAuthorizationPasscodeEmpty: 1002
    }
}));
jest.mock('@/lib/web.ts', () => ({
    navigateToHomePage: (...args: any[]) => mockNavigateToHomePage(...args)
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isUserVerifyEmailEnabled: () => mockVerifyEmailEnabled,
    getOIDCCustomDisplayNames: () => ({ en: 'OIDC' })
}));
jest.mock('@/lib/userstate.ts', () => ({ isUserLogined: () => mockLoggedIn }));
jest.mock('@/lib/version.ts', () => ({ getClientDisplayVersion: () => '17.0.0' }));
jest.mock('@mdi/js', () => ({
    mdiChevronLeft: 'chevron-left',
    mdiDotsVertical: 'dots-vertical',
    mdiSelectAll: 'select-all',
    mdiSelect: 'select-none',
    mdiSelectInverse: 'select-inverse',
    mdiCellphone: 'cellphone',
    mdiMonitor: 'monitor'
}));

import AppCloudSyncSettingTab from '@/views/desktop/app/settings/tabs/AppCloudSyncSettingTab.vue';
import VerifyEmailPage from '@/views/desktop/VerifyEmailPage.vue';
import OAuth2CallbackPage from '@/views/desktop/OAuth2CallbackPage.vue';

const feedback = { showMessage: mockShowMessage, showError: mockShowError };
const commonDesktopComponents = [
    'router-link', 'v-row', 'v-col', 'v-card', 'v-card-text', 'v-img', 'v-form',
    'v-text-field', 'v-btn', 'v-progress-circular', 'v-icon', 'v-spacer', 'v-divider',
    'language-select-button'
];

function setup(component: any, props: Record<string, unknown> = {}): any {
    return (component as any).setup(props, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    });
}

function attachFeedback(bindings: any): void {
    bindings.snackbar.value = feedback;
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

async function invokeCallbacks(root: any): Promise<string[]> {
    const callbacks = collectHostCallbacks(root);
    for (const { name, callback } of callbacks) {
        if (name === 'onUpdate:modelValue') callback('template-value');
        else if (name === 'onUpdate:show') callback(false);
        else if (name === 'onKeyup') callback({ key: 'Enter' });
        else callback();
        await flush(1);
    }
    return callbacks.map(item => item.name);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockDarkTheme = false;
    mockVerifyEmailEnabled = true;
    mockLoggedIn = false;
    mockThemeName.value = 'light';
    mockAuthorizeOAuth2.mockResolvedValue({ user: { id: 'alice' } });
    mockVerifyEmail.mockResolvedValue({});
    mockResendVerifyEmail.mockResolvedValue({});
    mockGetCloudSettings.mockResolvedValue([
        { settingKey: 'theme', settingValue: 'dark' },
        { settingKey: 'language', settingValue: 'zh_CN' }
    ]);
    mockFullUpdateCloudSettings.mockResolvedValue(undefined);
    mockDisableCloudSettings.mockResolvedValue(undefined);
});

describe('desktop application cloud sync actions', () => {
    test('loads settings and distinguishes processed, readable, and raw initialization failures', async () => {
        const success = setup(AppCloudSyncSettingTab);
        attachFeedback(success);
        expect(success.loading.value).toBe(true);
        await flush();
        expect(mockLastCloudBase.setUserApplicationCloudSettings).toHaveBeenCalledWith([
            { settingKey: 'theme', settingValue: 'dark' },
            { settingKey: 'language', settingValue: 'zh_CN' }
        ]);
        expect(success.loading.value).toBe(false);
        expect(mockShowError).not.toHaveBeenCalled();

        mockGetCloudSettings.mockRejectedValueOnce({ processed: true, message: 'handled load failure' });
        const processed = setup(AppCloudSyncSettingTab);
        attachFeedback(processed);
        await flush();
        expect(processed.loading.value).toBe(false);
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled load failure' }));

        const readableError = { processed: false, message: 'cloud load failed' };
        mockGetCloudSettings.mockRejectedValueOnce(readableError);
        const readable = setup(AppCloudSyncSettingTab);
        attachFeedback(readable);
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(readableError);

        mockGetCloudSettings.mockRejectedValueOnce('raw cloud load failure');
        const raw = setup(AppCloudSyncSettingTab);
        attachFeedback(raw);
        await flush();
        expect(mockShowError).toHaveBeenCalledWith('raw cloud load failure');
        expect(raw.loading.value).toBe(false);
    });

    test('enables and updates selected settings with distinct success messages', async () => {
        const bindings = setup(AppCloudSyncSettingTab);
        attachFeedback(bindings);
        await flush();

        bindings.enable(false);
        expect(bindings.enabling.value).toBe(true);
        expect(mockFullUpdateCloudSettings).toHaveBeenCalledWith(['theme', 'language']);
        await flush();
        expect(bindings.enabling.value).toBe(false);
        expect(mockShowMessage).toHaveBeenCalledWith('Settings sync has been enabled');

        bindings.enable(true);
        await flush();
        expect(mockShowMessage).toHaveBeenCalledWith('Synchronized settings have been updated');
    });

    test('releases enable state and reports only unprocessed update failures', async () => {
        const bindings = setup(AppCloudSyncSettingTab);
        attachFeedback(bindings);
        await flush();

        mockFullUpdateCloudSettings.mockRejectedValueOnce({ processed: true, message: 'handled update failure' });
        bindings.enable(false);
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled update failure' }));

        const readableError = { processed: false, message: 'cloud update failed' };
        mockFullUpdateCloudSettings.mockRejectedValueOnce(readableError);
        bindings.enable(true);
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(readableError);
        expect(bindings.enabling.value).toBe(false);
    });

    test('disables settings and reports only unprocessed disable failures', async () => {
        const bindings = setup(AppCloudSyncSettingTab);
        attachFeedback(bindings);
        await flush();

        bindings.disable();
        expect(bindings.disabling.value).toBe(true);
        await flush();
        expect(bindings.enabledApplicationCloudSettings.value).toEqual({});
        expect(bindings.disabling.value).toBe(false);
        expect(mockShowMessage).toHaveBeenCalledWith('Settings sync has been disabled');

        mockDisableCloudSettings.mockRejectedValueOnce({ processed: true, message: 'handled disable failure' });
        bindings.disable();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled disable failure' }));

        const readableError = { processed: false, message: 'cloud disable failed' };
        mockDisableCloudSettings.mockRejectedValueOnce(readableError);
        bindings.disable();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(readableError);
        expect(bindings.disabling.value).toBe(false);
    });
});

describe('desktop cloud sync production template', () => {
    test('renders loading, selection, enabled, disabled, and progress states and executes events', async () => {
        let resolveLoad: ((settings: any[]) => void) | undefined;
        mockGetCloudSettings.mockReturnValueOnce(new Promise(resolve => { resolveLoad = resolve; }));
        const mounted = mountWithHostRenderer(AppCloudSyncSettingTab as any, {}, [
            'v-row', 'v-col', 'v-card', 'v-card-text', 'v-progress-circular', 'v-skeleton-loader',
            'v-expansion-panels', 'v-expansion-panel', 'v-expansion-panel-title',
            'v-expansion-panel-text', 'v-btn', 'v-icon', 'v-menu', 'v-list', 'v-list-item',
            'v-divider', 'v-checkbox'
        ]);
        try {
            mounted.state.snackbar = feedback;
            expect(mounted.state.loading).toBe(true);
            expect(mounted.root.children.length).toBeGreaterThan(0);

            resolveLoad?.([{ settingKey: 'theme', settingValue: 'dark' }]);
            await flush();
            expect(mounted.state.loading).toBe(false);
            expect(mockLastCloudBase.isAllSettingsSelected).toHaveBeenCalled();
            expect(mockLastCloudBase.hasSettingSelectedButNotAllChecked).toHaveBeenCalled();

            mockLastCloudBase.isEnableCloudSync.value = false;
            await flush();
            const disabledCallbacks = await invokeCallbacks(mounted.root);
            expect(disabledCallbacks).toEqual(expect.arrayContaining(['onClick', 'onUpdate:modelValue']));
            expect(mockLastCloudBase.selectAllSettings).toHaveBeenCalled();
            expect(mockLastCloudBase.selectNoneSettings).toHaveBeenCalled();
            expect(mockLastCloudBase.selectInvertSettings).toHaveBeenCalled();
            expect(mockLastCloudBase.updateSettingsSelected).toHaveBeenCalled();

            mockLastCloudBase.isEnableCloudSync.value = true;
            mockLastCloudBase.enabledApplicationCloudSettings.value = {
                theme: true, fontSize: true, language: false, currency: false
            };
            mounted.state.enabling = true;
            mounted.state.disabling = true;
            await flush();
            expect(mounted.root.children.length).toBeGreaterThan(0);

            mounted.state.enabling = false;
            mounted.state.disabling = false;
            await flush();
            const enabledCallbacks = await invokeCallbacks(mounted.root);
            expect(enabledCallbacks).toContain('onClick');

            mockLastCloudBase.enabledApplicationCloudSettings.value = {};
            mockLastCloudBase.isEnableCloudSync.value = false;
            await flush();
            expect(mounted.state.hasEnabledApplicationCloudSettings).toBe(false);
        } finally {
            mounted.app.unmount();
        }
    });
});

describe('desktop VerifyEmailPage production template', () => {
    test('renders invalid and resend states, supports dark mode, and executes form and snackbar events', async () => {
        const invalid = mountWithHostRenderer(VerifyEmailPage as any, {
            email: '', hasValidEmailVerifyToken: false
        }, commonDesktopComponents);
        try {
            expect(invalid.state.loading).toBe(false);
            expect(invalid.root.children.length).toBeGreaterThan(0);
        } finally {
            invalid.app.unmount();
        }

        const resend = mountWithHostRenderer(VerifyEmailPage as any, {
            email: 'alice@example.invalid', hasValidEmailVerifyToken: true
        }, commonDesktopComponents);
        try {
            resend.state.snackbar = feedback;
            resend.state.password = 'secret';
            const callbackNames = await invokeCallbacks(resend.root);
            expect(callbackNames).toEqual(expect.arrayContaining([
                'onKeyup', 'onClick', 'onUpdate:modelValue', 'onUpdate:show'
            ]));
            expect(mockResendVerifyEmail).toHaveBeenCalledWith({
                email: 'alice@example.invalid', password: 'template-value'
            });

            mockDarkTheme = true;
            mockThemeName.value = 'dark';
            mockVerifyEmailEnabled = false;
            resend.state.resending = true;
            await flush();
            expect(resend.state.isDarkMode).toBe(true);
            expect(resend.root.children.length).toBeGreaterThan(0);
        } finally {
            resend.app.unmount();
        }

        const ordinaryResend = mountWithHostRenderer(VerifyEmailPage as any, {
            email: 'alice@example.invalid', hasValidEmailVerifyToken: false
        }, commonDesktopComponents);
        try {
            expect(ordinaryResend.root.children.length).toBeGreaterThan(0);
        } finally {
            ordinaryResend.app.unmount();
        }
    });

    test('renders verifying, verified, and unprocessed failure states', async () => {
        let resolveVerification: ((value?: unknown) => void) | undefined;
        mockVerifyEmail.mockReturnValueOnce(new Promise(resolve => { resolveVerification = resolve; }));
        const verifying = mountWithHostRenderer(VerifyEmailPage as any, {
            email: 'alice@example.invalid',
            token: 'verify-token',
            hasValidEmailVerifyToken: true
        }, commonDesktopComponents);
        try {
            verifying.state.snackbar = feedback;
            expect(verifying.state.loading).toBe(true);
            resolveVerification?.();
            await flush();
            expect(verifying.state.verified).toBe(true);
            expect(verifying.state.loading).toBe(false);
            mockLoggedIn = true;
            const callbackNames = await invokeCallbacks(verifying.root);
            expect(callbackNames).toContain('onUpdate:show');
            expect(mockRouter.replace).toHaveBeenCalledWith('/');
        } finally {
            verifying.app.unmount();
        }

        mockVerifyEmail.mockRejectedValueOnce({ processed: false, message: 'expired token' });
        const failed = mountWithHostRenderer(VerifyEmailPage as any, {
            email: 'alice@example.invalid',
            token: 'expired-token',
            hasValidEmailVerifyToken: false
        }, commonDesktopComponents);
        try {
            failed.state.snackbar = feedback;
            await flush();
            expect(failed.state.verified).toBe(false);
            expect(failed.state.verificationMessage).toBe('te:expired token');
            expect(failed.root.children.length).toBeGreaterThan(0);
        } finally {
            failed.app.unmount();
        }
    });
});

describe('desktop OAuth2CallbackPage production template', () => {
    test('renders binding form and executes password, passcode, button, and navigation events', async () => {
        const mounted = mountWithHostRenderer(OAuth2CallbackPage as any, {
            provider: 'oidc', platform: 'desktop', token: 'callback-token', userName: 'alice'
        }, commonDesktopComponents);
        try {
            mounted.state.snackbar = feedback;
            mounted.state.password = 'secret';
            mounted.state.passcode = '123456';
            const callbackNames = await invokeCallbacks(mounted.root);
            expect(callbackNames).toEqual(expect.arrayContaining(['onKeyup', 'onClick', 'onUpdate:modelValue']));
            expect(mockAuthorizeOAuth2).toHaveBeenCalledWith({
                password: 'template-value', passcode: 'template-value', callbackToken: 'callback-token'
            });

            mounted.state.show2faInput = true;
            mounted.state.loggingInByOAuth2 = true;
            mockDarkTheme = true;
            mockThemeName.value = 'dark';
            await flush();
            expect(mounted.state.isDarkMode).toBe(true);
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });

    test('renders automatic login, structured error, plain message, and empty fallback states', async () => {
        let resolveAuthorization: ((response: any) => void) | undefined;
        mockAuthorizeOAuth2.mockReturnValueOnce(new Promise(resolve => { resolveAuthorization = resolve; }));
        const automatic = mountWithHostRenderer(OAuth2CallbackPage as any, {
            provider: 'oidc', platform: 'mobile', token: 'callback-token'
        }, commonDesktopComponents);
        try {
            automatic.state.snackbar = feedback;
            expect(automatic.state.loggingInByOAuth2).toBe(true);
            resolveAuthorization?.({ user: { id: 'alice' } });
            await flush();
            expect(automatic.state.loggingInByOAuth2).toBe(false);
            expect(mockNavigateToHomePage).toHaveBeenCalledWith('mobile');
        } finally {
            automatic.app.unmount();
        }

        const error = mountWithHostRenderer(OAuth2CallbackPage as any, {
            provider: 'oidc', errorCode: '401', message: 'denied'
        }, commonDesktopComponents);
        try {
            expect(error.state.error).toEqual({ errorCode: 401, message: 'denied' });
            expect(error.root.children.length).toBeGreaterThan(0);
        } finally {
            error.app.unmount();
        }

        const message = mountWithHostRenderer(OAuth2CallbackPage as any, {
            provider: 'oidc', message: 'Authorization cancelled'
        }, commonDesktopComponents);
        try {
            expect(message.state.error).toBeUndefined();
            expect(message.root.children.length).toBeGreaterThan(0);
        } finally {
            message.app.unmount();
        }

        const fallback = mountWithHostRenderer(OAuth2CallbackPage as any, {}, commonDesktopComponents);
        try {
            expect(fallback.state.error).toBeUndefined();
            expect(fallback.root.children.length).toBeGreaterThan(0);
        } finally {
            fallback.app.unmount();
        }
    });
});
