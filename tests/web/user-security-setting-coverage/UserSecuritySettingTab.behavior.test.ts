import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockTemplateRefs = new Map<string, any>();
const mockUpdateUserProfile = jest.fn<(...args: any[]) => Promise<any>>();
const mockGenerateOAuth2LinkUrl = jest.fn<(...args: any[]) => string>();
const mockUpdateLocalizedDefaultSettings = jest.fn<(...args: any[]) => void>();
const mockGetExternalAuths = jest.fn<() => Promise<any[]>>();
const mockGetAllTokens = jest.fn<() => Promise<any[]>>();
const mockRevokeToken = jest.fn<(...args: any[]) => Promise<boolean>>();
const mockRevokeAllTokens = jest.fn<() => Promise<boolean>>();
const mockIsEquals = jest.fn<(left: unknown, right: unknown) => boolean>();
const mockIsOAuth2Enabled = jest.fn<() => boolean>();
const mockIsAPITokenEnabled = jest.fn<() => boolean>();
const mockIsMCPServerEnabled = jest.fn<() => boolean>();
const mockGetOAuth2Provider = jest.fn<() => string>();
const mockGetOIDCCustomDisplayNames = jest.fn<() => Record<string, string>>();
const mockGenerateRandomUUID = jest.fn<() => string>();
const mockFormatUnixTime = jest.fn<(value: number) => string>();
const mockLocalizedProviderName = jest.fn<(type: string, names: Record<string, string>) => string>();
const mockSetLanguage = jest.fn<(language: string) => Record<string, unknown>>();
const mockParseSessionInfo = jest.fn((token: any) => ({
    tokenId: token.tokenId,
    isCurrent: token.isCurrent,
    deviceType: token.deviceType ?? 'default',
    deviceInfo: token.userAgent,
    deviceName: token.deviceName ?? (token.isCurrent ? 'Current' : 'Other Device'),
    lastSeen: token.lastSeen,
}));

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
    };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        formatUnixTimeToLongDateTime: (value: number) => mockFormatUnixTime(value),
        getLocalizedOAuth2ProviderName: (type: string, names: Record<string, string>) => (
            mockLocalizedProviderName(type, names)
        ),
        setLanguage: (language: string) => mockSetLanguage(language),
    }),
}));

jest.mock('@/stores/index.ts', () => ({
    useRootStore: () => ({
        updateUserProfile: (...args: any[]) => mockUpdateUserProfile(...args),
        generateOAuth2LinkUrl: (...args: any[]) => mockGenerateOAuth2LinkUrl(...args),
    }),
}));
jest.mock('@/stores/setting.ts', () => ({
    useSettingsStore: () => ({ updateLocalizedDefaultSettings: mockUpdateLocalizedDefaultSettings }),
}));
jest.mock('@/stores/userExternalAuth.ts', () => ({
    useUserExternalAuthStore: () => ({ getExternalAuths: () => mockGetExternalAuths() }),
}));
jest.mock('@/stores/token.ts', () => ({
    useTokensStore: () => ({
        getAllTokens: () => mockGetAllTokens(),
        revokeToken: (...args: any[]) => mockRevokeToken(...args),
        revokeAllTokens: () => mockRevokeAllTokens(),
    }),
}));
jest.mock('@/lib/common.ts', () => ({ isEquals: (left: unknown, right: unknown) => mockIsEquals(left, right) }));
jest.mock('@/lib/session.ts', () => ({ parseSessionInfo: (token: unknown) => mockParseSessionInfo(token) }));
jest.mock('@/lib/server_settings.ts', () => ({
    isOAuth2Enabled: () => mockIsOAuth2Enabled(),
    isAPITokenEnabled: () => mockIsAPITokenEnabled(),
    isMCPServerEnabled: () => mockIsMCPServerEnabled(),
    getOAuth2Provider: () => mockGetOAuth2Provider(),
    getOIDCCustomDisplayNames: () => mockGetOIDCCustomDisplayNames(),
}));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => mockGenerateRandomUUID() }));
jest.mock('vuetify/components/VTextField', () => ({ VTextField: {} }));

for (const componentPath of [
    '@/views/desktop/user/settings/dialogs/UnlinkThirdPartyLoginDialog.vue',
    '@/views/desktop/user/settings/dialogs/UserGenerateTokenDialog.vue',
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
]) {
    jest.mock(componentPath, () => ({ __esModule: true, default: { name: 'UserSecurityCoverageChildStub' } }));
}

const UserSecuritySettingTab = require(
    '@/views/desktop/user/settings/tabs/UserSecuritySettingTab.vue'
).default as any;

function externalAuth(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        externalAuthCategory: 'oauth2',
        externalAuthType: 'github',
        linked: true,
        externalUsername: 'octocat',
        createdAt: 1_720_000_000,
        ...overrides,
    };
}

function token(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        tokenId: 'token-1',
        tokenType: 0,
        userAgent: 'Windows (Browser)',
        lastSeen: 1_720_000_000,
        isCurrent: false,
        deviceType: 'default',
        deviceName: 'Other Device',
        ...overrides,
    };
}

function setupTab(): any {
    mockTemplateRefs.clear();
    return UserSecuritySettingTab.setup({}, { expose: jest.fn() });
}

function installRefs(bindings: any): {
    snackbar: { showMessage: jest.Mock; showError: jest.Mock };
    unlink: { open: jest.Mock<(...args: any[]) => Promise<void>> };
    generate: { open: jest.Mock<(...args: any[]) => Promise<void>> };
    confirm: { open: jest.Mock<(...args: any[]) => Promise<void>> };
    newPasswordInput: { focus: jest.Mock };
    confirmPasswordInput: { focus: jest.Mock };
} {
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const unlink = { open: jest.fn<(...args: any[]) => Promise<void>>().mockResolvedValue(undefined) };
    const generate = { open: jest.fn<(...args: any[]) => Promise<void>>().mockResolvedValue(undefined) };
    const confirm = { open: jest.fn<(...args: any[]) => Promise<void>>().mockResolvedValue(undefined) };
    const newPasswordInput = { focus: jest.fn() };
    const confirmPasswordInput = { focus: jest.fn() };
    bindings.snackbar.value = snackbar;
    bindings.unlinkThirdPartyLoginDialog.value = unlink;
    bindings.generateTokenDialog.value = generate;
    bindings.confirmDialog.value = confirm;
    bindings.newPasswordInput.value = newPasswordInput;
    bindings.confirmPasswordInput.value = confirmPasswordInput;
    return { snackbar, unlink, generate, confirm, newPasswordInput, confirmPasswordInput };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
    await new Promise(resolve => setImmediate(resolve));
}

function deferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (reason: unknown) => void;
} {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

type CapturedCallback = { name: string; callback: (...args: any[]) => unknown };

function renderTab(bindings: any): any {
    const exposed = actualVue.proxyRefs(bindings);
    return UserSecuritySettingTab.render(exposed, [], {}, exposed, {}, {});
}

function exerciseRenderTree(
    value: any,
    callbacks: CapturedCallback[],
    seen = new Set<any>(),
): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) return;
    seen.add(value);

    if (Array.isArray(value)) {
        for (const item of value) exerciseRenderTree(item, callbacks, seen);
        return;
    }

    if (value.props && typeof value.props === 'object') {
        for (const [name, prop] of Object.entries(value.props)) {
            for (const candidate of (Array.isArray(prop) ? prop : [prop])) {
                if (name.startsWith('on') && typeof candidate === 'function') {
                    callbacks.push({ name, callback: candidate as (...args: any[]) => unknown });
                }
            }
        }
    }

    if (value.children && typeof value.children === 'object' && !Array.isArray(value.children)) {
        for (const child of Object.values(value.children)) {
            if (typeof child === 'function') {
                try {
                    exerciseRenderTree((child as (...args: any[]) => unknown)({}), callbacks, seen);
                } catch {
                    // Named slots have heterogeneous contracts; incompatible probes are ignored.
                }
            } else {
                exerciseRenderTree(child, callbacks, seen);
            }
        }
    } else {
        exerciseRenderTree(value.children, callbacks, seen);
    }

    exerciseRenderTree(value.dynamicChildren, callbacks, seen);
    exerciseRenderTree(value.ssContent, callbacks, seen);
    exerciseRenderTree(value.ssFallback, callbacks, seen);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    mockUpdateUserProfile.mockResolvedValue({});
    mockGenerateOAuth2LinkUrl.mockReturnValue('/oauth/link/session-uuid');
    mockGetExternalAuths.mockResolvedValue([]);
    mockGetAllTokens.mockResolvedValue([]);
    mockRevokeToken.mockResolvedValue(true);
    mockRevokeAllTokens.mockResolvedValue(true);
    mockIsEquals.mockReturnValue(false);
    mockIsOAuth2Enabled.mockReturnValue(true);
    mockIsAPITokenEnabled.mockReturnValue(true);
    mockIsMCPServerEnabled.mockReturnValue(false);
    mockGetOAuth2Provider.mockReturnValue('github');
    mockGetOIDCCustomDisplayNames.mockReturnValue({ custom: 'Custom Provider' });
    mockGenerateRandomUUID.mockReturnValue('session-uuid');
    mockFormatUnixTime.mockImplementation(value => `time:${value}`);
    mockLocalizedProviderName.mockImplementation(type => `provider:${type}`);
    mockSetLanguage.mockImplementation(language => ({ language }));
    mockParseSessionInfo.mockImplementation((value: any) => ({
        tokenId: value.tokenId,
        isCurrent: value.isCurrent,
        deviceType: value.deviceType ?? 'default',
        deviceInfo: value.userAgent,
        deviceName: value.deviceName ?? (value.isCurrent ? 'Current' : 'Other Device'),
        lastSeen: value.lastSeen,
    }));
});

describe('UserSecuritySettingTab production-loaded derived security state', () => {
    test('maps OAuth identities, missing display fields, session devices, dates, and empty inputs', async () => {
        const bindings = setupTab();
        await flush();

        expect(bindings.oauth2ClientSessionId.value).toBe('session-uuid');
        expect(bindings.oauth2LinkUrl.value).toBe('/oauth/link/session-uuid');
        expect(mockGenerateOAuth2LinkUrl).toHaveBeenCalledWith('desktop', 'session-uuid');

        bindings.externalAuths.value = null;
        expect(bindings.thirdPartyLogins.value).toStrictEqual([]);
        bindings.externalAuths.value = [
            externalAuth(),
            externalAuth({ externalAuthType: 'oidc', linked: false, externalUsername: '', createdAt: 0 }),
            externalAuth({
                externalAuthCategory: 'saml',
                externalAuthType: 'enterprise',
                linked: false,
                externalUsername: undefined,
                createdAt: undefined,
            }),
        ];
        expect(bindings.thirdPartyLogins.value).toMatchObject([
            { externalAuthType: 'github', displayName: 'provider:github', linked: true, externalUsername: 'octocat' },
            { externalAuthType: 'oidc', displayName: 'provider:oidc', linked: false, externalUsername: '-', createdAt: '-' },
            { externalAuthType: 'enterprise', displayName: 'enterprise', linked: false, externalUsername: '-', createdAt: '-' },
        ]);
        expect(bindings.thirdPartyLogins.value[0].createdAt).toBe('time:1720000000');

        bindings.tokens.value = null;
        expect(bindings.sessions.value).toStrictEqual([]);
        bindings.tokens.value = [
            token({ tokenId: 'phone', deviceType: 'phone' }),
            token({ tokenId: 'wearable', deviceType: 'wearable' }),
            token({ tokenId: 'tablet', deviceType: 'tablet' }),
            token({ tokenId: 'tv', deviceType: 'tv' }),
            token({ tokenId: 'api', deviceType: 'api' }),
            token({ tokenId: 'mcp', deviceType: 'mcp' }),
            token({ tokenId: 'default', deviceType: 'default', lastSeen: 0 }),
        ];
        const sessions = bindings.sessions.value;
        expect(sessions.map((session: any) => session.icon).filter(Boolean)).toHaveLength(7);
        expect(new Set(sessions.map((session: any) => session.icon)).size).toBe(7);
        expect(sessions[0].lastSeenDateTime).toBe('time:1720000000');
        expect(sessions[6].lastSeenDateTime).toBe('-');
    });

    test('reports every local password input problem before any profile request', async () => {
        const bindings = setupTab();
        await flush();
        const { snackbar } = installRefs(bindings);
        const cases = [
            ['', '', 'Nothing has been modified'],
            ['', 'confirmation', 'New password cannot be blank'],
            ['new-password', '', 'Password confirmation cannot be blank'],
            ['new-password', 'different', 'Password and password confirmation do not match'],
        ];

        for (const [nextPassword, confirmation, expectedMessage] of cases) {
            bindings.newPassword.value = nextPassword;
            bindings.confirmPassword.value = confirmation;
            bindings.updatePassword();
            expect(snackbar.showMessage).toHaveBeenLastCalledWith(expectedMessage);
        }
        expect(mockUpdateUserProfile).not.toHaveBeenCalled();
    });
});

describe('UserSecuritySettingTab production-loaded password behavior', () => {
    test('submits the current password, clears secrets, applies locale, and refreshes sessions on success', async () => {
        const profile = deferred<any>();
        mockUpdateUserProfile.mockReturnValueOnce(profile.promise);
        const bindings = setupTab();
        await flush();
        const { snackbar } = installRefs(bindings);
        mockGetAllTokens.mockClear();
        bindings.currentPassword.value = 'current-secret';
        bindings.newPassword.value = 'next-secret';
        bindings.confirmPassword.value = 'next-secret';

        bindings.updatePassword();
        expect(bindings.updatingPassword.value).toBe(true);
        expect(mockUpdateUserProfile).toHaveBeenCalledWith({
            password: 'next-secret',
            oldPassword: 'current-secret',
        });
        profile.resolve({ user: { language: 'zh-Hans' } });
        await flush();

        expect(bindings.updatingPassword.value).toBe(false);
        expect(bindings.currentPassword.value).toBe('');
        expect(bindings.newPassword.value).toBe('');
        expect(bindings.confirmPassword.value).toBe('');
        expect(mockSetLanguage).toHaveBeenCalledWith('zh-Hans');
        expect(mockUpdateLocalizedDefaultSettings).toHaveBeenCalledWith({ language: 'zh-Hans' });
        expect(mockGetAllTokens).toHaveBeenCalledTimes(1);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Your profile has been successfully updated');

        bindings.newPassword.value = 'another-secret';
        bindings.confirmPassword.value = 'another-secret';
        mockUpdateUserProfile.mockResolvedValueOnce({});
        bindings.updatePassword();
        await flush();
        expect(mockSetLanguage).toHaveBeenCalledTimes(1);
    });

    test('clears only the current password on failure and delegates processed errors upstream', async () => {
        const bindings = setupTab();
        await flush();
        const { snackbar } = installRefs(bindings);
        bindings.currentPassword.value = 'current-secret';
        bindings.newPassword.value = 'next-secret';
        bindings.confirmPassword.value = 'next-secret';

        mockUpdateUserProfile.mockRejectedValueOnce({ processed: false, message: 'wrong password' });
        bindings.updatePassword();
        await flush();
        expect(bindings.updatingPassword.value).toBe(false);
        expect(bindings.currentPassword.value).toBe('');
        expect(bindings.newPassword.value).toBe('next-secret');
        expect(bindings.confirmPassword.value).toBe('next-secret');
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'wrong password' }));

        bindings.currentPassword.value = 'retry-current';
        mockUpdateUserProfile.mockRejectedValueOnce({ processed: true, message: 'already shown' });
        bindings.updatePassword();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
    });
});

describe('UserSecuritySettingTab production-loaded OAuth behavior', () => {
    test('honors OAuth availability and distinguishes silent, unchanged, changed, and failed reloads', async () => {
        const bindings = setupTab();
        await flush();
        const { snackbar } = installRefs(bindings);
        mockGetExternalAuths.mockClear();

        mockIsOAuth2Enabled.mockReturnValueOnce(false);
        bindings.reloadExternalAuth(false);
        expect(mockGetExternalAuths).not.toHaveBeenCalled();

        bindings.externalAuths.value = [externalAuth()];
        mockIsEquals.mockReturnValueOnce(true);
        mockGetExternalAuths.mockResolvedValueOnce([externalAuth()]);
        bindings.reloadExternalAuth(false);
        await flush();
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Third-party logins list is up to date');

        mockIsEquals.mockReturnValueOnce(false);
        mockGetExternalAuths.mockResolvedValueOnce([externalAuth({ externalAuthType: 'oidc' })]);
        bindings.reloadExternalAuth(false);
        await flush();
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Third-party logins list has been updated');

        snackbar.showMessage.mockClear();
        mockGetExternalAuths.mockResolvedValueOnce([]);
        bindings.reloadExternalAuth(true);
        await flush();
        expect(snackbar.showMessage).not.toHaveBeenCalled();
        expect(bindings.loadingExternalAuth.value).toBe(false);

        mockGetExternalAuths.mockRejectedValueOnce({ processed: false, message: 'oauth failed' });
        bindings.reloadExternalAuth(false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'oauth failed' }));
        mockGetExternalAuths.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.reloadExternalAuth(false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
    });

    test('requires OAuth availability for unlink and reloads only after dialog completion', async () => {
        const bindings = setupTab();
        await flush();
        const { unlink } = installRefs(bindings);
        const login = bindings.thirdPartyLogins.value[0] ?? {
            externalAuthType: 'github',
        };
        mockGetExternalAuths.mockClear();

        mockIsOAuth2Enabled.mockReturnValueOnce(false);
        bindings.unlinkExternalAuth(login);
        expect(unlink.open).not.toHaveBeenCalled();

        bindings.unlinkExternalAuth(login);
        expect(unlink.open).toHaveBeenCalledWith('github');
        await flush();
        expect(mockGetExternalAuths).toHaveBeenCalledTimes(1);

        bindings.unlinkThirdPartyLoginDialog.value = null;
        expect(() => bindings.unlinkExternalAuth(login)).not.toThrow();
    });
});

describe('UserSecuritySettingTab production-loaded session behavior', () => {
    test('opens token generation and distinguishes silent, unchanged, changed, and failed session reloads', async () => {
        const bindings = setupTab();
        await flush();
        const { generate, snackbar } = installRefs(bindings);
        mockGetAllTokens.mockClear();

        bindings.generateToken();
        expect(generate.open).toHaveBeenCalledTimes(1);
        await flush();
        expect(mockGetAllTokens).toHaveBeenCalledTimes(1);
        bindings.generateTokenDialog.value = null;
        expect(() => bindings.generateToken()).not.toThrow();

        bindings.tokens.value = [token()];
        mockIsEquals.mockReturnValueOnce(true);
        mockGetAllTokens.mockResolvedValueOnce([token()]);
        bindings.reloadSessions(false);
        await flush();
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Session list is up to date');

        mockIsEquals.mockReturnValueOnce(false);
        mockGetAllTokens.mockResolvedValueOnce([token({ tokenId: 'token-2' })]);
        bindings.reloadSessions(false);
        await flush();
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Session list has been updated');

        snackbar.showMessage.mockClear();
        mockGetAllTokens.mockResolvedValueOnce([]);
        bindings.reloadSessions(true);
        await flush();
        expect(snackbar.showMessage).not.toHaveBeenCalled();

        mockGetAllTokens.mockRejectedValueOnce({ processed: false, message: 'sessions failed' });
        bindings.reloadSessions(false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'sessions failed' }));
        mockGetAllTokens.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.reloadSessions(false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
        expect(bindings.loadingSession.value).toBe(false);
    });

    test('confirms one-session logout, removes only the matching token, and reports unprocessed failures', async () => {
        const bindings = setupTab();
        await flush();
        const { confirm, snackbar } = installRefs(bindings);
        bindings.tokens.value = [
            token({ tokenId: 'current', isCurrent: true }),
            token({ tokenId: 'other' }),
        ];

        bindings.revokeSession(bindings.sessions.value[1]);
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to logout from this session?');
        await flush();
        expect(mockRevokeToken).toHaveBeenCalledWith({ tokenId: 'other' });
        expect(bindings.tokens.value.map((item: any) => item.tokenId)).toStrictEqual(['current']);

        bindings.tokens.value = [token({ tokenId: 'current', isCurrent: true })];
        bindings.revokeSession({ tokenId: 'missing' });
        await flush();
        expect(bindings.tokens.value).toHaveLength(1);

        mockRevokeToken.mockRejectedValueOnce({ processed: false, message: 'revoke failed' });
        bindings.revokeSession({ tokenId: 'current' });
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'revoke failed' }));
        mockRevokeToken.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.revokeSession({ tokenId: 'current' });
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
        expect(bindings.loadingSession.value).toBe(false);

        bindings.confirmDialog.value = null;
        expect(() => bindings.revokeSession({ tokenId: 'current' })).not.toThrow();
    });

    test('guards bulk logout, preserves the current session, and reports unprocessed failures', async () => {
        const bindings = setupTab();
        await flush();
        const { confirm, snackbar } = installRefs(bindings);

        bindings.tokens.value = [token({ tokenId: 'current', isCurrent: true })];
        bindings.revokeAllSessions();
        expect(confirm.open).not.toHaveBeenCalled();
        expect(mockRevokeAllTokens).not.toHaveBeenCalled();

        bindings.tokens.value = [
            token({ tokenId: 'current', isCurrent: true }),
            token({ tokenId: 'other-1' }),
            token({ tokenId: 'other-2' }),
        ];
        bindings.revokeAllSessions();
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to logout all other sessions?');
        await flush();
        expect(bindings.tokens.value.map((item: any) => item.tokenId)).toStrictEqual(['current']);
        expect(snackbar.showMessage).toHaveBeenCalledWith('You have logged out all other sessions');

        bindings.tokens.value = [token({ tokenId: 'current', isCurrent: true }), token({ tokenId: 'other' })];
        mockRevokeAllTokens.mockRejectedValueOnce({ processed: false, message: 'bulk failed' });
        bindings.revokeAllSessions();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'bulk failed' }));
        mockRevokeAllTokens.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.revokeAllSessions();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
        expect(bindings.loadingSession.value).toBe(false);

        bindings.confirmDialog.value = null;
        expect(() => bindings.revokeAllSessions()).not.toThrow();
    });
});

describe('UserSecuritySettingTab production template behavior', () => {
    test('renders and executes password, OAuth, loading, token, and session branches', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const bindings = setupTab();
            await flush();
            const refs = installRefs(bindings);
            const callbacks: CapturedCallback[] = [];

            bindings.updatingPassword.value = false;
            bindings.currentPassword.value = '';
            bindings.newPassword.value = '';
            bindings.confirmPassword.value = '';
            bindings.loadingExternalAuth.value = true;
            bindings.externalAuths.value = [];
            bindings.loadingSession.value = true;
            bindings.tokens.value = [];
            exerciseRenderTree(renderTab(bindings), callbacks);

            bindings.updatingPassword.value = true;
            bindings.currentPassword.value = 'current-secret';
            bindings.newPassword.value = 'next-secret';
            bindings.confirmPassword.value = 'next-secret';
            bindings.loadingExternalAuth.value = false;
            bindings.externalAuths.value = [
                externalAuth({ linked: true }),
                externalAuth({ linked: false }),
                externalAuth({ externalAuthType: 'oidc', linked: false }),
            ];
            bindings.loggingInByOAuth2.value = true;
            bindings.loadingSession.value = false;
            bindings.tokens.value = [
                token({ tokenId: 'current', isCurrent: true }),
                token({ tokenId: 'other', isCurrent: false }),
            ];
            exerciseRenderTree(renderTab(bindings), callbacks);

            bindings.updatingPassword.value = false;
            bindings.newPassword.value = 'only-new';
            bindings.confirmPassword.value = '';
            bindings.loadingSession.value = true;
            bindings.tokens.value = [token({ tokenId: 'busy-other' })];
            mockIsOAuth2Enabled.mockReturnValue(false);
            mockIsAPITokenEnabled.mockReturnValue(false);
            mockIsMCPServerEnabled.mockReturnValue(false);
            exerciseRenderTree(renderTab(bindings), callbacks);

            bindings.newPassword.value = 'matching';
            bindings.confirmPassword.value = 'matching';
            bindings.loadingSession.value = false;
            mockIsOAuth2Enabled.mockReturnValue(true);
            mockIsMCPServerEnabled.mockReturnValue(true);
            bindings.externalAuths.value = null;
            exerciseRenderTree(renderTab(bindings), callbacks);

            expect(callbacks.filter(item => item.name === 'onClick').length).toBeGreaterThan(10);
            expect(callbacks.some(item => item.name === 'onKeyup')).toBe(true);
            expect(callbacks.some(item => item.name.startsWith('onUpdate:'))).toBe(true);

            for (const { name, callback } of callbacks) {
                try {
                    if (name.startsWith('onUpdate:')) {
                        callback('typed-secret');
                    } else if (name === 'onKeyup') {
                        callback({ key: 'Enter' });
                    } else {
                        callback({ preventDefault: jest.fn(), stopPropagation: jest.fn() });
                    }
                } catch {
                    // Generated event wrappers use different payload contracts.
                }
            }
            await flush();

            expect(refs.newPasswordInput.focus).toHaveBeenCalled();
            expect(refs.confirmPasswordInput.focus).toHaveBeenCalled();
            expect(mockUpdateUserProfile).toHaveBeenCalled();
            expect(mockGetExternalAuths).toHaveBeenCalled();
            expect(mockGetAllTokens).toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });
});
