import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';
import {
    collectHostCallbacks,
    type HostNode,
    mountWithHostRenderer
} from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const sensitivePin = '482931';
const sensitivePassword = 'unit-test-password-must-not-appear';
const sensitiveAppLockSecret = 'unit-test-app-lock-secret-must-not-appear';

const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockEncryptToken = jest.fn<(...args: any[]) => void>();
const mockDecryptToken = jest.fn<(...args: any[]) => void>();
const mockIsCorrectPinCode = jest.fn<(...args: any[]) => boolean>();
const mockSaveWebAuthnConfig = jest.fn<(...args: any[]) => void>();
const mockClearWebAuthnConfig = jest.fn<(...args: any[]) => void>();
const mockRegisterWebAuthnCredential = jest.fn<(...args: any[]) => Promise<any>>();
const mockLoggerError = jest.fn<(...args: any[]) => void>();

let mockCurrentUser: { username: string } | null;
let mockAppLockState: { username: string; secret: string } | null;
let mockLastBase: ReturnType<typeof createAppLockBase>;
const mockActiveScopes: any[] = [];

const mockAppSettings = actualVue.reactive({
    applicationLock: false,
    applicationLockWebAuthn: false
});
const mockSettingsStore = {
    appSettings: mockAppSettings,
    setEnableApplicationLock: jest.fn((value: boolean) => {
        mockAppSettings.applicationLock = value;
    }),
    setEnableApplicationLockWebAuthn: jest.fn((value: boolean) => {
        mockAppSettings.applicationLockWebAuthn = value;
    })
};
const mockUserStore = {
    get currentUserBasicInfo(): { username: string } | null {
        return mockCurrentUser;
    }
};
const mockTransactionsStore = {
    saveTransactionDraft: jest.fn<(...args: any[]) => void>()
};

function createAppLockBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    return {
        isSupportedWebAuthn: ref(true),
        isEnableApplicationLock: computed({
            get: () => mockAppSettings.applicationLock,
            set: (value: boolean) => mockSettingsStore.setEnableApplicationLock(value)
        }),
        isEnableApplicationLockWebAuthn: ref(false)
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args)
}));
jest.mock('@/views/base/settings/AppLockPageBase.ts', () => ({
    useAppLockPageBase: () => {
        mockLastBase = createAppLockBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/lib/webauthn.ts', () => ({
    registerWebAuthnCredential: (...args: any[]) => mockRegisterWebAuthnCredential(...args)
}));
jest.mock('@/lib/userstate.ts', () => ({
    getUserAppLockState: () => mockAppLockState,
    encryptToken: (...args: any[]) => mockEncryptToken(...args),
    decryptToken: (...args: any[]) => mockDecryptToken(...args),
    isCorrectPinCode: (...args: any[]) => mockIsCorrectPinCode(...args),
    saveWebAuthnConfig: (...args: any[]) => mockSaveWebAuthnConfig(...args),
    clearWebAuthnConfig: (...args: any[]) => mockClearWebAuthnConfig(...args)
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { error: (...args: any[]) => mockLoggerError(...args) }
}));

import ApplicationLockPageComponent from '@/views/mobile/ApplicationLockPage.vue';

const ApplicationLockPage = ApplicationLockPageComponent as any;

function setup(): { bindings: any; stop: () => void } {
    const scope = actualVue.effectScope();
    const bindings = scope.run(() => ApplicationLockPage.setup(
        {},
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() }
    ));
    mockActiveScopes.push(scope);
    return { bindings, stop: () => scope.stop() };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function findNodes(node: HostNode, predicate: (candidate: HostNode) => boolean): HostNode[] {
    const matches = predicate(node) ? [node] : [];
    for (const child of node.children) matches.push(...findNodes(child, predicate));
    return matches;
}

function serializedCalls(mock: jest.Mock): string {
    return JSON.stringify(mock.mock.calls);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockAppSettings.applicationLock = false;
    mockAppSettings.applicationLockWebAuthn = false;
    mockCurrentUser = { username: 'synthetic-user' };
    mockAppLockState = {
        username: 'synthetic-user',
        secret: sensitiveAppLockSecret
    };
    mockIsCorrectPinCode.mockImplementation(pinCode => pinCode === sensitivePin);
    mockRegisterWebAuthnCredential.mockResolvedValue({ id: 'credential-id' });
});

afterEach(() => {
    while (mockActiveScopes.length) mockActiveScopes.pop()?.stop();
});

describe('mobile ApplicationLockPage PIN lifecycle', () => {
    test('initializes empty PIN sheets and handles already-enabled and open-sheet states', () => {
        const { bindings } = setup();
        expect(bindings.currentPinCodeForEnable.value).toBe('');
        expect(bindings.currentPinCodeForDisable.value).toBe('');
        expect(bindings.showInputPinCodeSheetForEnable.value).toBe(false);
        expect(bindings.showInputPinCodeSheetForDisable.value).toBe(false);

        mockAppSettings.applicationLock = true;
        bindings.enable(null);
        expect(mockShowToast).toHaveBeenCalledWith('Application lock has been enabled');

        mockAppSettings.applicationLock = false;
        bindings.currentPinCodeForEnable.value = sensitivePin;
        bindings.enable(null);
        expect(bindings.currentPinCodeForEnable.value).toBe('');
        expect(bindings.showInputPinCodeSheetForEnable.value).toBe(true);
        expect(mockEncryptToken).not.toHaveBeenCalled();
    });

    test('rejects malformed PINs and both missing-user identity variants', async () => {
        const { bindings } = setup();
        bindings.currentPinCodeForEnable.value = '123';
        bindings.enable('123');
        expect(mockShowToast).toHaveBeenCalledWith('Invalid PIN code');
        await flush();
        expect(bindings.currentPinCodeForEnable.value).toBe('');

        bindings.currentPinCodeForEnable.value = sensitivePin;
        mockCurrentUser = null;
        bindings.enable(sensitivePin);
        expect(mockShowToast).toHaveBeenCalledWith('An error occurred');
        await flush();
        expect(bindings.currentPinCodeForEnable.value).toBe('');

        bindings.currentPinCodeForEnable.value = sensitivePin;
        mockCurrentUser = { username: '' };
        bindings.enable(sensitivePin);
        await flush();
        expect(mockEncryptToken).not.toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenLastCalledWith('An error occurred');
    });

    test('enables the lock with a validated six-digit PIN and clears WebAuthn state', () => {
        const { bindings } = setup();
        bindings.currentPinCodeForEnable.value = sensitivePin;
        bindings.showInputPinCodeSheetForEnable.value = true;
        bindings.enable(sensitivePin);

        expect(mockEncryptToken).toHaveBeenCalledWith('synthetic-user', sensitivePin);
        expect(mockSettingsStore.setEnableApplicationLock).toHaveBeenCalledWith(true);
        expect(mockTransactionsStore.saveTransactionDraft).toHaveBeenCalledTimes(1);
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(mockClearWebAuthnConfig).toHaveBeenCalled();
        expect(bindings.showInputPinCodeSheetForEnable.value).toBe(false);
    });

    test('handles disabled, open-sheet, incorrect, and successful disable flows', async () => {
        const { bindings } = setup();
        bindings.disable(null);
        expect(mockShowToast).toHaveBeenCalledWith('Application lock is not enabled');

        mockAppSettings.applicationLock = true;
        bindings.currentPinCodeForDisable.value = sensitivePin;
        bindings.disable(null);
        expect(bindings.currentPinCodeForDisable.value).toBe('');
        expect(bindings.showInputPinCodeSheetForDisable.value).toBe(true);

        bindings.currentPinCodeForDisable.value = '000000';
        bindings.disable('000000');
        expect(mockShowToast).toHaveBeenCalledWith('Incorrect PIN code');
        await flush();
        expect(bindings.currentPinCodeForDisable.value).toBe('');
        expect(mockDecryptToken).not.toHaveBeenCalled();

        bindings.currentPinCodeForDisable.value = sensitivePin;
        bindings.disable(sensitivePin);
        expect(mockIsCorrectPinCode).toHaveBeenLastCalledWith(sensitivePin);
        expect(mockDecryptToken).toHaveBeenCalledTimes(1);
        expect(mockSettingsStore.setEnableApplicationLock).toHaveBeenCalledWith(false);
        expect(mockTransactionsStore.saveTransactionDraft).toHaveBeenCalledTimes(1);
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(bindings.showInputPinCodeSheetForDisable.value).toBe(false);
    });
});

describe('mobile ApplicationLockPage WebAuthn registration', () => {
    test('registers the current identity, persists the credential, and releases loading', async () => {
        const { bindings } = setup();
        bindings.isEnableApplicationLockWebAuthn.value = true;
        await flush();

        expect(mockShowLoading).toHaveBeenCalledTimes(1);
        expect(mockRegisterWebAuthnCredential).toHaveBeenCalledWith(
            mockAppLockState,
            mockCurrentUser
        );
        expect(mockHideLoading).toHaveBeenCalledTimes(1);
        expect(mockSaveWebAuthnConfig).toHaveBeenCalledWith('credential-id');
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(true);
        expect(mockShowToast).toHaveBeenCalledWith('You have enabled WebAuthn successfully');

        bindings.isEnableApplicationLockWebAuthn.value = false;
        await flush();
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenLastCalledWith(false);
        expect(mockClearWebAuthnConfig).toHaveBeenCalled();
    });

    test.each([
        [{ notSupported: true }, 'WebAuth is not supported on this device'],
        [{ name: 'NotAllowedError' }, 'User has canceled authentication'],
        [{ invalid: true }, 'Failed to enable WebAuthn'],
        [{ message: 'generic WebAuthn failure' }, 'User has canceled or this device does not support WebAuthn']
    ])('maps WebAuthn failure %# to a fixed visible error', async (error, expectedToast) => {
        mockRegisterWebAuthnCredential.mockRejectedValueOnce(error);
        const { bindings } = setup();
        bindings.isEnableApplicationLockWebAuthn.value = true;
        await flush();

        expect(mockLoggerError).toHaveBeenCalledWith('failed to enable WebAuthn', error);
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith(expectedToast);
        expect(bindings.isEnableApplicationLockWebAuthn.value).toBe(false);
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(mockClearWebAuthnConfig).toHaveBeenCalled();
    });

    test('fails closed when lock state or current identity is unavailable', async () => {
        mockAppLockState = null;
        const missingState = setup();
        missingState.bindings.isEnableApplicationLockWebAuthn.value = true;
        await flush();
        expect(mockRegisterWebAuthnCredential).not.toHaveBeenCalled();
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);

        mockAppLockState = { username: 'synthetic-user', secret: sensitiveAppLockSecret };
        mockCurrentUser = null;
        const missingUser = setup();
        missingUser.bindings.isEnableApplicationLockWebAuthn.value = true;
        await flush();
        expect(mockRegisterWebAuthnCredential).not.toHaveBeenCalled();
        expect(mockClearWebAuthnConfig).toHaveBeenCalled();
    });
});

describe('mobile ApplicationLockPage template and sensitive output', () => {
    test('renders back navigation and executes disabled, enabled, WebAuthn, and PIN-sheet events', async () => {
        const mounted = mountWithHostRenderer(ApplicationLockPage, {}, [
            'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-list',
            'f7-list-item', 'f7-toggle', 'f7-list-button', 'pin-code-input-sheet'
        ]);
        try {
            await flush();
            expect(findNodes(mounted.root, node => (
                node.props['backLink'] === 'tt:Back' || node.props['back-link'] === 'tt:Back'
            ))).toHaveLength(1);

            let callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onClick', 'onUpdate:show', 'onUpdate:modelValue', 'onPincode:confirm'
            ]));
            for (const { name, callback } of callbacks) {
                if (name === 'onUpdate:show') callback(true);
                else if (name === 'onUpdate:modelValue') callback(sensitivePin);
                else if (name === 'onPincode:confirm') callback(sensitivePin);
                else callback();
                await flush(1);
            }

            mockAppSettings.applicationLock = true;
            mounted.state.isSupportedWebAuthn = true;
            await actualVue.nextTick();
            callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.map(item => item.name)).toContain('onToggle:change');
            for (const { name, callback } of callbacks) {
                if (name === 'onToggle:change') callback(true);
                else if (name === 'onUpdate:show') callback(false);
                else if (name === 'onUpdate:modelValue') callback(sensitivePin);
                else if (name === 'onPincode:confirm') callback(sensitivePin);
                else if (name === 'onClick') callback();
                await flush(1);
            }
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });

    test('never projects PIN, password, or app-lock secret into logs or visible errors', async () => {
        const { bindings } = setup();
        bindings.currentPinCodeForEnable.value = sensitivePin;
        bindings.enable(sensitivePin);
        mockAppSettings.applicationLock = true;
        bindings.currentPinCodeForDisable.value = sensitivePin;
        bindings.disable(sensitivePin);

        mockRegisterWebAuthnCredential.mockRejectedValueOnce({ notSupported: true });
        bindings.isEnableApplicationLockWebAuthn.value = true;
        await flush();

        for (const output of [serializedCalls(mockShowToast), serializedCalls(mockLoggerError)]) {
            expect(output).not.toContain(sensitivePin);
            expect(output).not.toContain(sensitivePassword);
            expect(output).not.toContain(sensitiveAppLockSecret);
        }
        expect(mockShowToast.mock.calls.flat().every(message => typeof message === 'string')).toBe(true);
    });
});
