import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockShowMessage = jest.fn();
const mockEncryptToken = jest.fn();
const mockDecryptToken = jest.fn();
const mockIsCorrectPinCode = jest.fn<(pin: string) => boolean>(() => true);
const mockSaveWebAuthnConfig = jest.fn();
const mockClearWebAuthnConfig = jest.fn();
const mockGetUserAppLockState = jest.fn(() => ({ username: 'alice', secret: 'lock-secret' }));
const mockRegisterWebAuthnCredential = jest.fn<(...args: any[]) => Promise<{ id: string }>>();
const mockLoggerError = jest.fn();

let mockInitialLockEnabled = false;
let mockInitialWebAuthnEnabled = false;
let mockLastLockRefs: any;

const mockSettingsStore = {
    appSettings: {
        applicationLock: false,
        applicationLockWebAuthn: false
    },
    setEnableApplicationLock: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.applicationLock = value;
    }),
    setEnableApplicationLockWebAuthn: jest.fn((value: boolean) => {
        mockSettingsStore.appSettings.applicationLockWebAuthn = value;
    })
};
const mockUserStore = {
    currentUserBasicInfo: { username: 'alice', id: 1 }
};
const mockTransactionsStore = { saveTransactionDraft: jest.fn() };

const mockGetAllTokens = jest.fn<(...args: any[]) => Promise<any[]>>();
const mockRevokeToken = jest.fn<(...args: any[]) => Promise<void>>();
const mockRevokeAllTokens = jest.fn<(...args: any[]) => Promise<void>>();
const mockParseSessionInfo = jest.fn((token: any) => ({
    tokenId: token.tokenId,
    isCurrent: token.isCurrent,
    deviceType: token.deviceType,
    deviceInfo: token.deviceInfo ?? `info:${token.tokenId}`,
    deviceName: token.deviceName ?? `name:${token.tokenId}`,
    lastSeen: token.lastSeen
}));
const mockShowConfirm = jest.fn((_message: string, callback: () => void) => callback());
const mockShowToast = jest.fn();
const mockRouteBackOnError = jest.fn();
const mockShowLoading = jest.fn();
const mockHideLoading = jest.fn();
const mockOnSwipeoutDeleted = jest.fn((_id: string, callback: () => void) => callback());

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null)
    };
});
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: { name: 'Wave11SnackBarStub', template: '<div />' }
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageTextDirection: () => 1,
        formatUnixTimeToLongDateTime: (value: number) => `date:${value}`
    })
}));
jest.mock('@/views/base/settings/AppLockPageBase.ts', () => ({
    useAppLockPageBase: () => {
        const { ref } = jest.requireActual('vue') as any;
        mockLastLockRefs = {
            isSupportedWebAuthn: ref(true),
            isEnableApplicationLock: ref(mockInitialLockEnabled),
            isEnableApplicationLockWebAuthn: ref(mockInitialWebAuthnEnabled)
        };
        return mockLastLockRefs;
    }
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/lib/webauthn.ts', () => ({
    registerWebAuthnCredential: (...args: unknown[]) => mockRegisterWebAuthnCredential(...args)
}));
jest.mock('@/lib/userstate.ts', () => ({
    getUserAppLockState: () => mockGetUserAppLockState(),
    encryptToken: (...args: unknown[]) => mockEncryptToken(...args),
    decryptToken: () => mockDecryptToken(),
    isCorrectPinCode: (pin: string) => mockIsCorrectPinCode(pin),
    saveWebAuthnConfig: (id: string) => mockSaveWebAuthnConfig(id),
    clearWebAuthnConfig: () => mockClearWebAuthnConfig()
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { error: (...args: unknown[]) => mockLoggerError(...args) }
}));
jest.mock('@/stores/token.ts', () => ({
    useTokensStore: () => ({
        getAllTokens: (...args: unknown[]) => mockGetAllTokens(...args),
        revokeToken: (...args: unknown[]) => mockRevokeToken(...args),
        revokeAllTokens: (...args: unknown[]) => mockRevokeAllTokens(...args)
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showConfirm: (message: string, callback: () => void) => mockShowConfirm(message, callback),
        showToast: (...args: unknown[]) => mockShowToast(...args),
        routeBackOnError: (...args: unknown[]) => mockRouteBackOnError(...args)
    }),
    showLoading: () => mockShowLoading(),
    hideLoading: () => mockHideLoading(),
    onSwipeoutDeleted: (id: string, callback: () => void) => mockOnSwipeoutDeleted(id, callback)
}));
jest.mock('@/lib/session.ts', () => ({
    parseSessionInfo: (token: unknown) => mockParseSessionInfo(token)
}));

import { SessionDeviceType } from '@/models/token.ts';
import AppLockSettingTab from '@/views/desktop/app/settings/tabs/AppLockSettingTab.vue';
import SessionListPage from '@/views/mobile/users/SessionListPage.vue';

function setupAppLock(): any {
    const bindings = (AppLockSettingTab as any).setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    });
    bindings.snackbar.value = { showMessage: mockShowMessage };
    return bindings;
}

function setupSessions(router = { back: jest.fn(), navigate: jest.fn() }): any {
    return (SessionListPage as any).setup({ f7router: router }, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    });
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await (jest.requireActual('vue') as any).nextTick();
    await new Promise(resolve => setImmediate(resolve));
}

function token(
    tokenId: string,
    deviceType: SessionDeviceType,
    isCurrent = false,
    lastSeen = 100
): any {
    return { tokenId, deviceType, isCurrent, lastSeen };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockInitialLockEnabled = false;
    mockInitialWebAuthnEnabled = false;
    Object.assign(mockSettingsStore.appSettings, {
        applicationLock: false,
        applicationLockWebAuthn: false
    });
    mockUserStore.currentUserBasicInfo = { username: 'alice', id: 1 };
    mockIsCorrectPinCode.mockReturnValue(true);
    mockGetUserAppLockState.mockReturnValue({ username: 'alice', secret: 'lock-secret' });
    mockRegisterWebAuthnCredential.mockResolvedValue({ id: 'credential-id' });
    mockGetAllTokens.mockResolvedValue([]);
    mockRevokeToken.mockResolvedValue(undefined);
    mockRevokeAllTokens.mockResolvedValue(undefined);
});

describe('desktop AppLockSettingTab', () => {
    test('rejects redundant, invalid, and userless enable attempts', () => {
        const bindings = setupAppLock();

        mockSettingsStore.appSettings.applicationLock = true;
        bindings.enable();
        expect(mockShowMessage).toHaveBeenLastCalledWith('Application lock has been enabled');

        mockSettingsStore.appSettings.applicationLock = false;
        bindings.pinCode.value = '123';
        bindings.enable();
        expect(bindings.pinCode.value).toBe('');
        expect(mockShowMessage).toHaveBeenLastCalledWith('Invalid PIN code');

        bindings.pinCode.value = '123456';
        mockUserStore.currentUserBasicInfo = null as any;
        bindings.enable();
        expect(bindings.pinCode.value).toBe('');
        expect(mockShowMessage).toHaveBeenLastCalledWith('An error occurred');
        expect(mockEncryptToken).not.toHaveBeenCalled();
    });

    test('enables and disables the lock while preserving draft and WebAuthn state ordering', () => {
        const bindings = setupAppLock();
        bindings.pinCode.value = '123456';

        bindings.enable();

        expect(mockEncryptToken).toHaveBeenCalledWith('alice', '123456');
        expect(mockSettingsStore.setEnableApplicationLock).toHaveBeenCalledWith(true);
        expect(mockTransactionsStore.saveTransactionDraft).toHaveBeenCalledTimes(1);
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(mockClearWebAuthnConfig).toHaveBeenCalled();
        expect(bindings.pinCode.value).toBe('');

        mockSettingsStore.appSettings.applicationLock = true;
        bindings.pinCode.value = '654321';
        bindings.disable();
        expect(mockIsCorrectPinCode).toHaveBeenCalledWith('654321');
        expect(mockDecryptToken).toHaveBeenCalled();
        expect(mockSettingsStore.setEnableApplicationLock).toHaveBeenCalledWith(false);
        expect(mockTransactionsStore.saveTransactionDraft).toHaveBeenCalledTimes(2);
        expect(bindings.pinCode.value).toBe('');
    });

    test('fails closed when disabling an absent lock or using an incorrect PIN', () => {
        const bindings = setupAppLock();

        bindings.disable();
        expect(mockShowMessage).toHaveBeenLastCalledWith('Application lock is not enabled');

        mockSettingsStore.appSettings.applicationLock = true;
        mockIsCorrectPinCode.mockReturnValue(false);
        bindings.pinCode.value = '000000';
        bindings.disable();
        expect(bindings.pinCode.value).toBe('');
        expect(mockShowMessage).toHaveBeenLastCalledWith('Incorrect PIN code');
        expect(mockDecryptToken).not.toHaveBeenCalled();
    });

    test('confirm delegates to enable or disable according to the base lock binding', () => {
        const enableBindings = setupAppLock();
        enableBindings.pinCode.value = '123456';
        mockLastLockRefs.isEnableApplicationLock.value = false;
        enableBindings.confirm();
        expect(mockEncryptToken).toHaveBeenCalledWith('alice', '123456');

        const disableBindings = setupAppLock();
        mockSettingsStore.appSettings.applicationLock = true;
        disableBindings.pinCode.value = '654321';
        mockLastLockRefs.isEnableApplicationLock.value = true;
        disableBindings.confirm();
        expect(mockDecryptToken).toHaveBeenCalled();
    });

    test('registers WebAuthn and clears it again when the binding is disabled', async () => {
        const bindings = setupAppLock();

        mockLastLockRefs.isEnableApplicationLockWebAuthn.value = true;
        await flush();

        expect(mockRegisterWebAuthnCredential).toHaveBeenCalledWith(
            { username: 'alice', secret: 'lock-secret' },
            { username: 'alice', id: 1 }
        );
        expect(mockSaveWebAuthnConfig).toHaveBeenCalledWith('credential-id');
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(true);
        expect(mockShowMessage).toHaveBeenCalledWith('You have enabled WebAuthn successfully');
        expect(bindings.enablingWebAuthn.value).toBe(false);

        jest.clearAllMocks();
        mockLastLockRefs.isEnableApplicationLockWebAuthn.value = false;
        await flush();
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(mockClearWebAuthnConfig).toHaveBeenCalled();
    });

    test.each([
        [{ notSupported: true }, 'WebAuth is not supported on this device'],
        [{ name: 'NotAllowedError' }, 'User has canceled authentication'],
        [{ invalid: true }, 'Failed to enable WebAuthn'],
        [{ message: 'generic failure' }, 'User has canceled or this device does not support WebAuthn']
    ])('rolls back WebAuthn after registration failure %#', async (error, message) => {
        mockRegisterWebAuthnCredential.mockRejectedValueOnce(error);
        const bindings = setupAppLock();

        mockLastLockRefs.isEnableApplicationLockWebAuthn.value = true;
        await flush();

        expect(mockLoggerError).toHaveBeenCalledWith('failed to enable WebAuthn', error);
        expect(bindings.enablingWebAuthn.value).toBe(false);
        expect(mockShowMessage).toHaveBeenCalledWith(message);
        expect(mockLastLockRefs.isEnableApplicationLockWebAuthn.value).toBe(false);
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(mockClearWebAuthnConfig).toHaveBeenCalled();
    });

    test('does not start registration when unlocked credential state is unavailable', async () => {
        mockGetUserAppLockState.mockReturnValueOnce(null as any);
        setupAppLock();
        mockLastLockRefs.isEnableApplicationLockWebAuthn.value = true;
        await flush();

        expect(mockRegisterWebAuthnCredential).not.toHaveBeenCalled();
        expect(mockSettingsStore.setEnableApplicationLockWebAuthn).toHaveBeenCalledWith(false);
        expect(mockClearWebAuthnConfig).toHaveBeenCalled();
    });
});

describe('mobile SessionListPage', () => {
    test('loads sessions and derives stable DOM ids, timestamps, and device icons', async () => {
        mockGetAllTokens.mockResolvedValueOnce([
            token('phone:1', SessionDeviceType.Phone, true),
            token('wearable:2', SessionDeviceType.Wearable),
            token('tablet:3', SessionDeviceType.Tablet),
            token('tv:4', SessionDeviceType.TV),
            token('api:5', SessionDeviceType.Api),
            token('mcp:6', SessionDeviceType.MCP),
            token('desktop:7', SessionDeviceType.Default, false, 0)
        ]);
        const bindings = setupSessions();

        expect(bindings.loading.value).toBe(true);
        await flush();

        expect(bindings.loading.value).toBe(false);
        expect(bindings.sessions.value.map((session: any) => session.icon)).toEqual([
            'device_phone_portrait',
            'device_phone_portrait',
            'device_tablet_portrait',
            'tv',
            'chevron_left_slash_chevron_right',
            'sparkles',
            'device_desktop'
        ]);
        expect(bindings.sessions.value[0]).toEqual(expect.objectContaining({
            domId: 'token_phone_1',
            lastSeenDateTime: 'date:100'
        }));
        expect(bindings.sessions.value[6].lastSeenDateTime).toBe('-');
        expect(bindings.textDirection.value).toBe(1);
    });

    test('records unprocessed initialization errors and forwards them on page entry', async () => {
        const router = { back: jest.fn(), navigate: jest.fn() };
        const failure = { processed: false, message: 'session load failed' };
        mockGetAllTokens.mockRejectedValueOnce(failure);
        const bindings = setupSessions(router);
        await flush();

        expect(bindings.loadingError.value).toStrictEqual(failure);
        expect(mockShowToast).toHaveBeenCalledWith('session load failed');
        bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);

        jest.clearAllMocks();
        mockGetAllTokens.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processed = setupSessions();
        await flush();
        expect(processed.loading.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalled();
    });

    test('reload reports unchanged, changed, and failed results while always finishing refresh', async () => {
        const initial = [token('current', SessionDeviceType.Default, true)];
        mockGetAllTokens.mockResolvedValueOnce(initial);
        const bindings = setupSessions();
        await flush();
        const done = jest.fn();

        mockGetAllTokens.mockResolvedValueOnce([...initial]);
        bindings.reload(done);
        await flush();
        expect(done).toHaveBeenCalledTimes(1);
        expect(mockShowToast).toHaveBeenLastCalledWith('Session list is up to date');

        mockGetAllTokens.mockResolvedValueOnce([...initial, token('other', SessionDeviceType.Phone)]);
        bindings.reload(done);
        await flush();
        expect(mockShowToast).toHaveBeenLastCalledWith('Session list has been updated');
        expect(bindings.tokens.value).toHaveLength(2);

        mockGetAllTokens.mockRejectedValueOnce({ processed: false, message: 'reload failed' });
        bindings.reload(done);
        await flush();
        expect(done).toHaveBeenCalledTimes(3);
        expect(mockShowToast).toHaveBeenLastCalledWith('reload failed');
    });

    test('revokes one non-current session after confirmation and swipeout deletion', async () => {
        mockGetAllTokens.mockResolvedValueOnce([
            token('current', SessionDeviceType.Default, true),
            token('other:1', SessionDeviceType.Phone)
        ]);
        const bindings = setupSessions();
        await flush();

        bindings.revoke(bindings.sessions.value[1]);
        await flush();

        expect(mockShowConfirm).toHaveBeenCalledWith(
            'Are you sure you want to logout from this session?',
            expect.any(Function)
        );
        expect(mockShowLoading).toHaveBeenCalled();
        expect(mockRevokeToken).toHaveBeenCalledWith({ tokenId: 'other:1' });
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockOnSwipeoutDeleted).toHaveBeenCalledWith('token_other_1', expect.any(Function));
        expect(bindings.tokens.value.map((item: any) => item.tokenId)).toEqual(['current']);
    });

    test('surfaces one-session revoke failures unless already processed', async () => {
        mockGetAllTokens.mockResolvedValueOnce([token('other', SessionDeviceType.Phone)]);
        mockRevokeToken.mockRejectedValueOnce({ processed: false, message: 'revoke failed' });
        const bindings = setupSessions();
        await flush();

        bindings.revoke(bindings.sessions.value[0]);
        await flush();
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('revoke failed');
    });

    test('revokes all other sessions, keeps the current one, and handles guarded/error paths', async () => {
        mockGetAllTokens.mockResolvedValueOnce([token('current', SessionDeviceType.Default, true)]);
        const guarded = setupSessions();
        await flush();
        guarded.revokeAll();
        expect(mockShowConfirm).not.toHaveBeenCalled();

        jest.clearAllMocks();
        mockGetAllTokens.mockResolvedValueOnce([
            token('other-1', SessionDeviceType.Phone),
            token('current', SessionDeviceType.Default, true),
            token('other-2', SessionDeviceType.Tablet)
        ]);
        const successful = setupSessions();
        await flush();
        successful.revokeAll();
        await flush();
        expect(mockRevokeAllTokens).toHaveBeenCalled();
        expect(successful.tokens.value.map((item: any) => item.tokenId)).toEqual(['current']);
        expect(mockShowToast).toHaveBeenCalledWith('You have logged out all other sessions');

        jest.clearAllMocks();
        mockGetAllTokens.mockResolvedValueOnce([
            token('current', SessionDeviceType.Default, true),
            token('other', SessionDeviceType.Phone)
        ]);
        mockRevokeAllTokens.mockRejectedValueOnce({ processed: false, message: 'revoke all failed' });
        const failed = setupSessions();
        await flush();
        failed.revokeAll();
        await flush();
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('revoke all failed');
    });
});
