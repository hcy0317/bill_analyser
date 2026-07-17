import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import {
    collectHostCallbacks,
    mountWithHostRenderer
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as typeof import('@/../node_modules/vue');
const mockTemplateRefs = new Map<string, ReturnType<typeof mockActualVue.ref>>();
const mockRouter = { replace: jest.fn() };
const mockThemeName = mockActualVue.ref('light');
const mockShowMessage = jest.fn();
const mockConfirmOpen = jest.fn<() => Promise<void>>();
const mockShowToast = jest.fn();
const mockShowConfirm = jest.fn();
const mockOpenExternalUrl = jest.fn();
const mockShowLoading = jest.fn();
const mockHideLoading = jest.fn();
const mockUnlockTokenByWebAuthn = jest.fn();
const mockUnlockTokenByPinCode = jest.fn();
const mockVerifyWebAuthnCredential = jest.fn<() => Promise<{
    id: string;
    userName: string;
    userSecret: string;
}>>();
const mockDoAfterUnlocked = jest.fn();
const mockDoRelogin = jest.fn();
const mockLoggerError = jest.fn();

let mockCredentialId: string | null = 'credential-id';
let mockHasWebAuthnConfig = true;
let mockWebAuthnSupported = true;
let mockModalShowing = false;
let mockConfirmCallback: (() => void) | null = null;
let mockLastBase: ReturnType<typeof createUnlockBase>;

const mockSettingsStore = mockActualVue.reactive({
    appSettings: { applicationLockWebAuthn: true }
});
const mockUserStore = mockActualVue.reactive({
    currentUserBasicInfo: { username: 'alice' } as { username?: string } | null
});

function createUnlockBase() {
    const pinCode = mockActualVue.ref('');
    return {
        version: '10.2.0-test',
        pinCode,
        isWebAuthnAvailable: mockActualVue.computed(() => (
            mockSettingsStore.appSettings.applicationLockWebAuthn
            && mockHasWebAuthnConfig
            && mockWebAuthnSupported
        )),
        isPinCodeValid: (value: string) => /^\d{6}$/.test(value),
        doAfterUnlocked: mockDoAfterUnlocked,
        doRelogin: mockDoRelogin
    };
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as typeof import('@/../node_modules/vue');
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        }
    };
});

jest.mock('vue-router', () => ({ useRouter: () => mockRouter }));
jest.mock('vuetify', () => ({
    useTheme: () => ({ global: { name: mockThemeName } })
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/views/base/UnlockPageBase.ts', () => ({
    useUnlockPageBase: () => {
        mockLastBase = createUnlockBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/core/theme.ts', () => ({
    isDarkApplicationTheme: (name: string) => name === 'dark'
}));
jest.mock('@/consts/asset.ts', () => ({ APPLICATION_LOGO_PATH: '/logo.svg' }));
jest.mock('@/lib/webauthn.ts', () => ({
    isWebAuthnSupported: () => mockWebAuthnSupported,
    verifyWebAuthnCredential: () => mockVerifyWebAuthnCredential()
}));
jest.mock('@/lib/userstate.ts', () => ({
    unlockTokenByWebAuthn: (...args: unknown[]) => mockUnlockTokenByWebAuthn(...args),
    unlockTokenByPinCode: (...args: unknown[]) => mockUnlockTokenByPinCode(...args),
    hasWebAuthnConfig: () => mockHasWebAuthnConfig,
    getWebAuthnCredentialId: () => mockCredentialId
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showToast: (...args: unknown[]) => mockShowToast(...args),
        showConfirm: (message: string, callback: () => void) => {
            mockConfirmCallback = callback;
            mockShowConfirm(message, callback);
        },
        openExternalUrl: (...args: unknown[]) => mockOpenExternalUrl(...args)
    }),
    showLoading: (...args: unknown[]) => mockShowLoading(...args),
    hideLoading: (...args: unknown[]) => mockHideLoading(...args),
    isModalShowing: () => mockModalShowing
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { error: (...args: unknown[]) => mockLoggerError(...args) }
}));

const mockChildComponent = (name: string, exposed: Record<string, unknown> = {}) => ({
    __esModule: true,
    default: mockActualVue.defineComponent({
        name,
        setup: (_props, { expose }) => {
            expose(exposed);
            return () => mockActualVue.h(`${name}-stub`);
        }
    })
});

jest.mock('@/components/desktop/ConfirmDialog.vue', () => mockChildComponent(
    'UnlockConfirmDialog', { open: mockConfirmOpen }
));
jest.mock('@/components/desktop/SnackBar.vue', () => mockChildComponent(
    'UnlockSnackBar', { showMessage: mockShowMessage }
));

import DesktopUnlockPageModule from '@/views/desktop/UnlockPage.vue';
import MobileUnlockPageModule from '@/views/mobile/UnlockPage.vue';

const DesktopUnlockPage = DesktopUnlockPageModule as unknown as {
    setup: (props: object, context: object) => Record<string, unknown>;
};
const MobileUnlockPage = MobileUnlockPageModule as unknown as {
    setup: (props: object, context: object) => Record<string, unknown>;
};

function setupDesktop() {
    const bindings = DesktopUnlockPage.setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    }) as any;
    bindings.snackbar.value = { showMessage: mockShowMessage };
    bindings.confirmDialog.value = { open: mockConfirmOpen };
    return bindings;
}

function createMobileRouter() {
    return {
        refreshPage: jest.fn(),
        navigate: jest.fn()
    };
}

function setupMobile() {
    const router = createMobileRouter();
    const bindings = MobileUnlockPage.setup({ f7router: router }, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    }) as any;
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
}

async function invokeTemplateCallbacks(root: Parameters<typeof collectHostCallbacks>[0]): Promise<void> {
    for (const { name, callback } of collectHostCallbacks(root)) {
        if (name === 'onUpdate:modelValue' || name === 'onPincode:confirm') {
            callback('123456');
        } else {
            callback({ preventDefault: jest.fn(), stopPropagation: jest.fn() });
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    mockCredentialId = 'credential-id';
    mockHasWebAuthnConfig = true;
    mockWebAuthnSupported = true;
    mockModalShowing = false;
    mockConfirmCallback = null;
    mockThemeName.value = 'light';
    mockSettingsStore.appSettings.applicationLockWebAuthn = true;
    mockUserStore.currentUserBasicInfo = { username: 'alice' };
    mockConfirmOpen.mockResolvedValue(undefined);
    mockVerifyWebAuthnCredential.mockResolvedValue({
        id: 'verified-id',
        userName: 'alice',
        userSecret: 'verified-secret'
    });
});

describe('desktop UnlockPage behavior', () => {
    test('covers PIN validation, missing identity, successful unlock, and failure reporting', () => {
        const bindings = setupDesktop();

        bindings.unlockByPin('12345');
        expect(mockUnlockTokenByPinCode).not.toHaveBeenCalled();

        mockUserStore.currentUserBasicInfo = null;
        bindings.unlockByPin('123456');
        expect(mockShowMessage).toHaveBeenCalledWith('An error occurred');

        mockUserStore.currentUserBasicInfo = {};
        bindings.unlockByPin('123456');
        expect(mockShowMessage).toHaveBeenLastCalledWith('An error occurred');

        mockUserStore.currentUserBasicInfo = { username: 'alice' };
        bindings.unlockByPin('123456');
        expect(mockUnlockTokenByPinCode).toHaveBeenCalledWith('alice', '123456');
        expect(mockDoAfterUnlocked).toHaveBeenCalled();
        expect(mockRouter.replace).toHaveBeenCalledWith('/');

        mockUnlockTokenByPinCode.mockImplementationOnce(() => {
            throw new Error('bad pin');
        });
        bindings.unlockByPin('654321');
        expect(mockLoggerError).toHaveBeenCalledWith('failed to unlock with pin code', expect.any(Error));
        expect(mockShowMessage).toHaveBeenLastCalledWith('Incorrect PIN code');
    });

    test('guards WebAuthn prerequisites and completes a successful credential verification', async () => {
        const bindings = setupDesktop();

        mockUserStore.currentUserBasicInfo = null;
        bindings.unlockByWebAuthn();
        expect(mockShowMessage).toHaveBeenLastCalledWith('An error occurred');

        mockUserStore.currentUserBasicInfo = { username: 'alice' };
        mockCredentialId = null;
        bindings.unlockByWebAuthn();
        expect(mockShowMessage).toHaveBeenLastCalledWith('An error occurred');

        mockCredentialId = 'credential-id';
        mockSettingsStore.appSettings.applicationLockWebAuthn = false;
        bindings.unlockByWebAuthn();
        expect(mockShowMessage).toHaveBeenLastCalledWith('WebAuthn is not enabled');

        mockSettingsStore.appSettings.applicationLockWebAuthn = true;
        mockHasWebAuthnConfig = false;
        bindings.unlockByWebAuthn();
        expect(mockShowMessage).toHaveBeenLastCalledWith('WebAuthn is not enabled');

        mockHasWebAuthnConfig = true;
        mockWebAuthnSupported = false;
        bindings.unlockByWebAuthn();
        expect(mockShowMessage).toHaveBeenLastCalledWith('WebAuth is not supported on this device');

        mockWebAuthnSupported = true;
        bindings.unlockByWebAuthn();
        expect(bindings.verifyingByWebAuthn.value).toBe(true);
        await flush();
        expect(bindings.verifyingByWebAuthn.value).toBe(false);
        expect(mockUnlockTokenByWebAuthn).toHaveBeenCalledWith(
            'verified-id', 'alice', 'verified-secret'
        );
        expect(mockRouter.replace).toHaveBeenLastCalledWith('/');
    });

    test.each([
        [{ notSupported: true }, 'WebAuth is not supported on this device'],
        [{ name: 'NotAllowedError' }, 'User has canceled authentication'],
        [{ invalid: true }, 'Failed to authenticate with WebAuthn'],
        [{ name: 'UnknownError' }, 'User has canceled or this device does not support WebAuthn']
    ])('maps WebAuthn failure %# to its user-facing message', async (error, message) => {
        mockVerifyWebAuthnCredential.mockRejectedValueOnce(error);
        const bindings = setupDesktop();
        bindings.unlockByWebAuthn();
        await flush();

        expect(bindings.verifyingByWebAuthn.value).toBe(false);
        expect(mockLoggerError).toHaveBeenCalledWith('failed to use webauthn to verify', error);
        expect(mockShowMessage).toHaveBeenLastCalledWith(message);
    });

    test('relogs in after confirmation and renders light, dark, available, and busy states', async () => {
        const bindings = setupDesktop();
        bindings.relogin();
        await flush();
        expect(mockDoRelogin).toHaveBeenCalled();
        expect(mockRouter.replace).toHaveBeenCalledWith('/login');

        const light = mountWithHostRenderer(DesktopUnlockPageModule as any, {}, [
            'RouterLink', 'VRow', 'VCol', 'VImg', 'VCard', 'VCardText', 'VForm',
            'VBtn', 'VProgressCircular', 'VSpacer', 'VDivider', 'PinCodeInput',
            'LanguageSelectButton'
        ]);
        expect(light.state.isDarkMode).toBe(false);
        expect(collectHostCallbacks(light.root).length).toBeGreaterThan(0);
        await invokeTemplateCallbacks(light.root);
        light.app.unmount();

        mockThemeName.value = 'dark';
        mockSettingsStore.appSettings.applicationLockWebAuthn = false;
        const dark = mountWithHostRenderer(DesktopUnlockPageModule as any, {}, [
            'RouterLink', 'VRow', 'VCol', 'VImg', 'VCard', 'VCardText', 'VForm',
            'VBtn', 'VProgressCircular', 'VSpacer', 'VDivider', 'PinCodeInput',
            'LanguageSelectButton'
        ]);
        expect(dark.state.isDarkMode).toBe(true);
        dark.state.verifyingByWebAuthn = true;
        await mockActualVue.nextTick();
        dark.app.unmount();
    });
});

describe('mobile UnlockPage behavior', () => {
    test('guards modal and identity state, then unlocks or reports a bad PIN', () => {
        const { bindings, router } = setupMobile();
        bindings.unlockByPin('12345');
        expect(mockUnlockTokenByPinCode).not.toHaveBeenCalled();

        mockModalShowing = true;
        bindings.unlockByPin('123456');
        expect(mockUnlockTokenByPinCode).not.toHaveBeenCalled();

        mockModalShowing = false;
        mockUserStore.currentUserBasicInfo = null;
        bindings.unlockByPin('123456');
        expect(mockShowToast).toHaveBeenLastCalledWith('An error occurred');

        mockUserStore.currentUserBasicInfo = {};
        bindings.unlockByPin('123456');
        expect(mockShowToast).toHaveBeenLastCalledWith('An error occurred');

        mockUserStore.currentUserBasicInfo = { username: 'alice' };
        bindings.unlockByPin('123456');
        expect(mockUnlockTokenByPinCode).toHaveBeenCalledWith('alice', '123456');
        expect(router.refreshPage).toHaveBeenCalled();

        mockUnlockTokenByPinCode.mockImplementationOnce(() => {
            throw new Error('bad pin');
        });
        bindings.unlockByPin('654321');
        expect(mockShowToast).toHaveBeenLastCalledWith('Incorrect PIN code');
    });

    test('guards WebAuthn prerequisites and completes a successful verification', async () => {
        const { bindings, router } = setupMobile();

        mockUserStore.currentUserBasicInfo = null;
        bindings.unlockByWebAuthn();
        expect(mockShowToast).toHaveBeenLastCalledWith('An error occurred');

        mockUserStore.currentUserBasicInfo = { username: 'alice' };
        mockCredentialId = null;
        bindings.unlockByWebAuthn();
        expect(mockShowToast).toHaveBeenLastCalledWith('An error occurred');

        mockCredentialId = 'credential-id';
        mockSettingsStore.appSettings.applicationLockWebAuthn = false;
        bindings.unlockByWebAuthn();
        expect(mockShowToast).toHaveBeenLastCalledWith('WebAuthn is not enabled');

        mockSettingsStore.appSettings.applicationLockWebAuthn = true;
        mockHasWebAuthnConfig = false;
        bindings.unlockByWebAuthn();
        expect(mockShowToast).toHaveBeenLastCalledWith('WebAuthn is not enabled');

        mockHasWebAuthnConfig = true;
        mockWebAuthnSupported = false;
        bindings.unlockByWebAuthn();
        expect(mockShowToast).toHaveBeenLastCalledWith('WebAuth is not supported on this device');

        mockWebAuthnSupported = true;
        bindings.unlockByWebAuthn();
        expect(mockShowLoading).toHaveBeenCalled();
        await flush();
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockUnlockTokenByWebAuthn).toHaveBeenCalledWith(
            'verified-id', 'alice', 'verified-secret'
        );
        expect(router.refreshPage).toHaveBeenCalled();
    });

    test.each([
        [{ notSupported: true }, 'WebAuth is not supported on this device'],
        [{ name: 'NotAllowedError' }, 'User has canceled authentication'],
        [{ invalid: true }, 'Failed to authenticate with WebAuthn'],
        [{ name: 'UnknownError' }, 'User has canceled or this device does not support WebAuthn']
    ])('maps mobile WebAuthn failure %# to its user-facing message', async (error, message) => {
        mockVerifyWebAuthnCredential.mockRejectedValueOnce(error);
        const { bindings } = setupMobile();
        bindings.unlockByWebAuthn();
        await flush();

        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockLoggerError).toHaveBeenCalledWith('failed to use webauthn to verify', error);
        expect(mockShowToast).toHaveBeenLastCalledWith(message);
    });

    test('relogs in with history cleanup and renders WebAuthn-present and PIN-only templates', async () => {
        const { bindings, router } = setupMobile();
        bindings.relogin();
        expect(mockShowConfirm).toHaveBeenCalledWith(
            'Are you sure you want to re-login?', expect.any(Function)
        );
        mockConfirmCallback?.();
        expect(mockDoRelogin).toHaveBeenCalled();
        expect(router.navigate).toHaveBeenCalledWith('/login', { clearPreviousHistory: true });

        const availableRouter = createMobileRouter();
        const available = mountWithHostRenderer(
            MobileUnlockPageModule as any,
            { f7router: availableRouter },
            [
                'F7Page', 'F7LoginScreenTitle', 'F7Block', 'F7List', 'F7ListItem',
                'F7ListButton', 'F7BlockFooter', 'F7Link', 'F7Toolbar', 'PinCodeInput',
                'LanguageSelectButton'
            ]
        );
        expect(collectHostCallbacks(available.root).length).toBeGreaterThan(0);
        await invokeTemplateCallbacks(available.root);
        available.state.openExternalUrl('https://example.invalid');
        expect(mockOpenExternalUrl).toHaveBeenCalledWith('https://example.invalid');
        available.app.unmount();

        mockSettingsStore.appSettings.applicationLockWebAuthn = false;
        const pinOnly = mountWithHostRenderer(
            MobileUnlockPageModule as any,
            { f7router: createMobileRouter() },
            [
                'F7Page', 'F7LoginScreenTitle', 'F7Block', 'F7List', 'F7ListItem',
                'F7ListButton', 'F7BlockFooter', 'F7Link', 'F7Toolbar', 'PinCodeInput',
                'LanguageSelectButton'
            ]
        );
        mockLastBase.pinCode.value = '123456';
        await mockActualVue.nextTick();
        pinOnly.app.unmount();
    });
});
