import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockKnownErrorCode = { UserEmailNotVerified: 201020 } as const;
const mockUsername = '<unit-test-user>';
const mockPassword = '<unit-test-password>';
const mockPasscode = '000000';
const mockBackupCode = '<unit-test-backup-code>';
const mockTempToken = '<unit-test-challenge-token>';
const mockEmail = 'unit-test@example.invalid';
const mockDesktopPath = '/desktop/#/login';
const mockOAuthUrl = '/api/oauth2/authorize?session=<unit-test-session>';

const mockShowAlert = jest.fn<(...args: any[]) => void>();
const mockShowConfirm = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockOpenExternalUrl = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockDoAfterLogin = jest.fn<(...args: any[]) => void>();
const mockWindowReplace = jest.fn<(...args: any[]) => void>();
const mockGetDesktopVersionPath = jest.fn(() => mockDesktopPath);
const mockGenerateRandomUUID = jest.fn(() => '<unit-test-session>');

let mockInternalAuthEnabled = true;
let mockOAuth2Enabled = true;
let mockRegistrationEnabled = true;
let mockForgetPasswordEnabled = true;
let mockVerifyEmailEnabled = true;
let mockModalShowing = false;

const mockStore = {
    authorize: jest.fn<(...args: any[]) => Promise<any>>(),
    authorize2FA: jest.fn<(...args: any[]) => Promise<any>>(),
    requestResetPassword: jest.fn<(...args: any[]) => Promise<void>>(),
    resendVerifyEmailByUnloginUser: jest.fn<(...args: any[]) => Promise<void>>()
};

let mockLastBase: ReturnType<typeof createBase>;

function createBase(): any {
    const { ref } = jest.requireActual('vue') as any;
    return {
        version: 'v-test',
        username: ref(''),
        password: ref(''),
        passcode: ref(''),
        backupCode: ref(''),
        tempToken: ref(''),
        twoFAVerifyType: ref('passcode'),
        oauth2ClientSessionId: ref(''),
        loggingInByPassword: ref(false),
        loggingInByOAuth2: ref(false),
        verifying: ref(false),
        inputIsEmpty: ref(true),
        twoFAInputIsEmpty: ref(true),
        oauth2LoginUrl: ref(mockOAuthUrl),
        oauth2LoginDisplayName: ref('Unit Test OAuth'),
        tips: ref('Authorized users only'),
        doAfterLogin: mockDoAfterLogin
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => (
            values ? `tt:${key}:${JSON.stringify(values)}` : `tt:${key}`
        )
    })
}));
jest.mock('@/views/base/LoginPageBase.ts', () => ({
    useLoginPageBase: (platform: string) => {
        if (platform !== 'mobile') throw new Error('unexpected platform');
        mockLastBase = createBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => mockStore }));
jest.mock('@/consts/asset.ts', () => ({ APPLICATION_LOGO_PATH: '/img/unit-test-logo.svg' }));
jest.mock('@/consts/api.ts', () => ({ KnownErrorCode: mockKnownErrorCode }));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => mockGenerateRandomUUID() }));
jest.mock('@/lib/server_settings.ts', () => ({
    isUserRegistrationEnabled: () => mockRegistrationEnabled,
    isUserForgetPasswordEnabled: () => mockForgetPasswordEnabled,
    isUserVerifyEmailEnabled: () => mockVerifyEmailEnabled,
    isInternalAuthEnabled: () => mockInternalAuthEnabled,
    isOAuth2Enabled: () => mockOAuth2Enabled
}));
jest.mock('@/lib/version.ts', () => ({ getDesktopVersionPath: () => mockGetDesktopVersionPath() }));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showConfirm: mockShowConfirm,
        showToast: mockShowToast,
        openExternalUrl: mockOpenExternalUrl
    }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args),
    isModalShowing: () => mockModalShowing
}));

import LoginPage from '@/views/mobile/LoginPage.vue';

function setup(): { bindings: any; router: any } {
    const router = { refreshPage: jest.fn(), navigate: jest.fn(), back: jest.fn() };
    const bindings = (LoginPage as any).setup(
        { f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() }
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function setPasswordCredentials(bindings: any): void {
    bindings.username.value = mockUsername;
    bindings.password.value = mockPassword;
    bindings.inputIsEmpty.value = false;
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(): { app: any; root: any; router: any; state: any } {
    const { createRenderer, defineComponent, h } = jest.requireActual('vue') as any;
    const renderer = createRenderer({
        patchProp(node: any, key: string, _previous: unknown, value: unknown) {
            node.props[key] = value;
        },
        insert(child: any, parent: any, anchor: any = null) {
            child.parent = parent;
            if (!anchor) {
                parent.children.push(child);
                return;
            }
            const index = parent.children.indexOf(anchor);
            parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
        },
        remove(child: any) {
            const index = child.parent?.children.indexOf(child) ?? -1;
            if (index >= 0) child.parent.children.splice(index, 1);
        },
        createElement(hostType: string) {
            return createHostNode(hostType);
        },
        createText(text: string) {
            return createHostNode('#text', text);
        },
        createComment(text: string) {
            return createHostNode('#comment', text);
        },
        setText(node: any, text: string) {
            node.text = text;
        },
        setElementText(node: any, text: string) {
            node.text = text;
            node.children = [];
        },
        parentNode(node: any) {
            return node.parent;
        },
        nextSibling(node: any) {
            const siblings = node.parent?.children ?? [];
            return siblings[siblings.indexOf(node) + 1] ?? null;
        },
        querySelector() {
            return null;
        },
        setScopeId(node: any, scopeId: string) {
            node.props[scopeId] = '';
        },
        cloneNode(node: any) {
            return { ...node, children: [...node.children], props: { ...node.props }, parent: null };
        },
        insertStaticContent(content: string, parent: any, anchor: any) {
            const node = createHostNode('#static', content);
            node.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, node);
            return [node, node];
        }
    });
    const SlotHost = defineComponent({
        name: 'SlotHost',
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h('stub', attrs, Object.values(slots).flatMap((slot: any) => slot?.() ?? []));
        }
    });
    const router = { refreshPage: jest.fn(), navigate: jest.fn(), back: jest.fn() };
    const app = renderer.createApp(LoginPage as any, { f7router: router });
    app.config.warnHandler = () => undefined;
    for (const name of [
        'f7-page', 'f7-login-screen-title', 'f7-block', 'f7-block-footer', 'f7-list', 'f7-list-input',
        'f7-list-item', 'f7-list-button', 'f7-link', 'f7-toolbar', 'f7-sheet', 'f7-page-content',
        'f7-button', 'language-select-button', 'password-input-sheet'
    ]) app.component(name, SlotHost);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, router, state: vm.$.setupState };
}

function collectHostCallbacks(
    node: any,
    callbacks: Array<{ name: string; callback: (...args: any[]) => any }>,
    seen = new Set<any>()
): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    for (const [name, value] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        if (typeof value === 'function') {
            callbacks.push({ name, callback: value as (...args: any[]) => any });
        } else if (Array.isArray(value)) {
            for (const callback of value) {
                if (typeof callback === 'function') callbacks.push({ name, callback });
            }
        }
    }
    for (const child of node.children ?? []) collectHostCallbacks(child, callbacks, seen);
}

async function invokeHostCallbacks(callbacks: Array<{ name: string; callback: (...args: any[]) => any }>): Promise<void> {
    for (const { name, callback } of callbacks) {
        if (name === 'onUpdate:value') {
            callback('<unit-test-input>');
        } else if (name === 'onUpdate:show') {
            callback(true);
        } else if (name === 'onUpdate:modelValue') {
            callback(mockPassword);
        } else if (name === 'onKeyup') {
            callback({ key: 'Enter' });
        } else if (name === 'onPassword:confirm') {
            callback(mockPassword);
        } else {
            callback();
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockInternalAuthEnabled = true;
    mockOAuth2Enabled = true;
    mockRegistrationEnabled = true;
    mockForgetPasswordEnabled = true;
    mockVerifyEmailEnabled = true;
    mockModalShowing = false;
    mockStore.authorize.mockResolvedValue({ user: { id: 'unit-test-user-id' } });
    mockStore.authorize2FA.mockResolvedValue({ user: { id: 'unit-test-user-id' } });
    mockStore.requestResetPassword.mockResolvedValue(undefined);
    mockStore.resendVerifyEmailByUnloginUser.mockResolvedValue(undefined);
    mockShowLoading.mockImplementation((predicate?: () => unknown) => {
        predicate?.();
    });
    (globalThis as any).window.location = { replace: mockWindowReplace };
});

describe('mobile LoginPage password authentication', () => {
    test('initializes a fresh OAuth client session and validates required credentials', () => {
        const { bindings } = setup();
        expect(mockGenerateRandomUUID).toHaveBeenCalledTimes(1);
        expect(bindings.oauth2ClientSessionId.value).toBe('<unit-test-session>');
        expect(bindings.twoFAVerifyTypeSwitchName.value).toBe('Use Backup Code');

        bindings.login();
        expect(mockShowAlert).toHaveBeenCalledWith('Username cannot be blank');
        bindings.username.value = mockUsername;
        bindings.login();
        expect(mockShowAlert).toHaveBeenCalledWith('Password cannot be blank');
        expect(mockStore.authorize).not.toHaveBeenCalled();

        setPasswordCredentials(bindings);
        bindings.tempToken.value = mockTempToken;
        bindings.login();
        expect(bindings.show2faSheet.value).toBe(true);
        expect(mockStore.authorize).not.toHaveBeenCalled();
    });

    test('authorizes synthetic credentials, completes login, and refreshes the route', async () => {
        const authResponse = { user: { id: 'unit-test-user-id' }, notificationContent: 'test notice' };
        mockStore.authorize.mockResolvedValueOnce(authResponse);
        const { bindings, router } = setup();
        setPasswordCredentials(bindings);
        bindings.resendVerifyEmail.value = 'stale@example.invalid';
        bindings.hasValidEmailVerifyToken.value = true;
        bindings.currentPasswordForResendVerifyEmail.value = '<stale-test-password>';

        bindings.login();
        expect(bindings.loggingInByPassword.value).toBe(true);
        expect(bindings.resendVerifyEmail.value).toBe('');
        expect(bindings.hasValidEmailVerifyToken.value).toBe(false);
        expect(bindings.currentPasswordForResendVerifyEmail.value).toBe('');
        expect(mockStore.authorize).toHaveBeenCalledWith({
            loginName: mockUsername,
            password: mockPassword
        });
        await flush();

        expect(bindings.loggingInByPassword.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockDoAfterLogin).toHaveBeenCalledWith(authResponse);
        expect(router.refreshPage).toHaveBeenCalled();
    });

    test('opens the second-factor sheet without completing a challenge login', async () => {
        mockStore.authorize.mockResolvedValueOnce({ need2FA: true, token: mockTempToken });
        const { bindings, router } = setup();
        setPasswordCredentials(bindings);
        bindings.login();
        await flush();

        expect(bindings.tempToken.value).toBe(mockTempToken);
        expect(bindings.show2faSheet.value).toBe(true);
        expect(mockDoAfterLogin).not.toHaveBeenCalled();
        expect(router.refreshPage).not.toHaveBeenCalled();
    });

    test('offers email verification only for the complete enabled error contract', async () => {
        const { bindings } = setup();
        setPasswordCredentials(bindings);
        const unverified = (context: Record<string, unknown> | undefined, errorCode: number = mockKnownErrorCode.UserEmailNotVerified) => ({
            processed: false,
            message: 'synthetic authorization failure',
            error: context === undefined ? { errorCode } : { errorCode, context }
        });

        mockVerifyEmailEnabled = false;
        mockStore.authorize.mockRejectedValueOnce(unverified({ email: mockEmail }));
        bindings.login();
        await flush();
        expect(bindings.showVerifyEmailSheet.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('synthetic authorization failure');

        mockVerifyEmailEnabled = true;
        mockStore.authorize.mockRejectedValueOnce({ processed: false, message: 'missing structured error' });
        bindings.login();
        await flush();
        mockStore.authorize.mockRejectedValueOnce(unverified({}, 999999));
        bindings.login();
        await flush();
        mockStore.authorize.mockRejectedValueOnce(unverified(undefined));
        bindings.login();
        await flush();
        mockStore.authorize.mockRejectedValueOnce(unverified({}));
        bindings.login();
        await flush();
        expect(bindings.showVerifyEmailSheet.value).toBe(false);

        mockStore.authorize.mockRejectedValueOnce(unverified({ email: mockEmail, hasValidEmailVerifyToken: true }));
        bindings.login();
        await flush();
        expect(bindings.resendVerifyEmail.value).toBe(mockEmail);
        expect(bindings.hasValidEmailVerifyToken.value).toBe(true);
        expect(bindings.currentPasswordForResendVerifyEmail.value).toBe('');
        expect(bindings.showVerifyEmailSheet.value).toBe(true);

        bindings.showVerifyEmailSheet.value = false;
        mockStore.authorize.mockRejectedValueOnce(unverified({ email: mockEmail }));
        bindings.login();
        await flush();
        expect(bindings.hasValidEmailVerifyToken.value).toBe(false);
        expect(bindings.showVerifyEmailSheet.value).toBe(true);
    });

    test('handles processed, readable, and raw authorization failures without leaking credentials', async () => {
        const { bindings } = setup();
        setPasswordCredentials(bindings);

        mockStore.authorize.mockRejectedValueOnce({ processed: true, message: 'handled authorization failure' });
        bindings.login();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled authorization failure');

        mockStore.authorize.mockRejectedValueOnce({ processed: false, message: 'readable authorization failure' });
        bindings.login();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('readable authorization failure');

        mockStore.authorize.mockRejectedValueOnce('raw authorization failure');
        bindings.login();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw authorization failure');
        expect(mockShowToast).not.toHaveBeenCalledWith(expect.stringContaining(mockPassword));
    });

    test('guards enter-key login behind modal state and starts OAuth login explicitly', async () => {
        const { bindings } = setup();
        setPasswordCredentials(bindings);
        mockModalShowing = true;
        bindings.loginByPressEnter();
        expect(mockStore.authorize).not.toHaveBeenCalled();

        mockModalShowing = false;
        bindings.loginByPressEnter();
        await flush();
        expect(mockStore.authorize).toHaveBeenCalledTimes(1);

        bindings.loginByOAuth2();
        expect(bindings.loggingInByOAuth2.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalled();
    });

    test('redirects to the desktop path only through the confirmation callback', () => {
        const { bindings } = setup();
        mockShowConfirm.mockImplementationOnce((_message: string, confirm: () => void) => confirm());
        bindings.switchToDesktopVersion();
        expect(mockShowConfirm).toHaveBeenCalledWith(
            'Are you sure you want to switch to desktop version?',
            expect.any(Function)
        );
        expect(mockGetDesktopVersionPath).toHaveBeenCalled();
        expect(mockWindowReplace).toHaveBeenCalledWith(mockDesktopPath);
    });
});

describe('mobile LoginPage second factor and account recovery', () => {
    test('validates and switches passcode and backup-code modes', () => {
        const { bindings } = setup();
        bindings.twoFAInputIsEmpty.value = true;
        bindings.verify();
        bindings.twoFAInputIsEmpty.value = false;
        bindings.verifying.value = true;
        bindings.verify();
        expect(mockStore.authorize2FA).not.toHaveBeenCalled();

        bindings.verifying.value = false;
        bindings.twoFAVerifyType.value = 'passcode';
        bindings.passcode.value = '';
        bindings.verify();
        expect(mockShowAlert).toHaveBeenCalledWith('Passcode cannot be blank');

        bindings.switch2FAVerifyType();
        expect(bindings.twoFAVerifyType.value).toBe('backupcode');
        expect(bindings.twoFAVerifyTypeSwitchName.value).toBe('Use Passcode');
        bindings.backupCode.value = '';
        bindings.verify();
        expect(mockShowAlert).toHaveBeenCalledWith('Backup code cannot be blank');
        bindings.switch2FAVerifyType();
        expect(bindings.twoFAVerifyType.value).toBe('passcode');
    });

    test('verifies passcodes and recovery codes using mutually exclusive request fields', async () => {
        const passcodeResponse = { user: { id: 'passcode-user' } };
        mockStore.authorize2FA.mockResolvedValueOnce(passcodeResponse);
        const passcode = setup();
        passcode.bindings.tempToken.value = mockTempToken;
        passcode.bindings.twoFAInputIsEmpty.value = false;
        passcode.bindings.passcode.value = mockPasscode;
        passcode.bindings.verify();
        expect(mockStore.authorize2FA).toHaveBeenCalledWith({
            token: mockTempToken,
            passcode: mockPasscode,
            recoveryCode: null
        });
        await flush();
        expect(mockDoAfterLogin).toHaveBeenCalledWith(passcodeResponse);
        expect(passcode.bindings.show2faSheet.value).toBe(false);
        expect(passcode.router.refreshPage).toHaveBeenCalled();

        const backupResponse = { user: { id: 'backup-user' } };
        mockStore.authorize2FA.mockResolvedValueOnce(backupResponse);
        const backup = setup();
        backup.bindings.tempToken.value = mockTempToken;
        backup.bindings.twoFAInputIsEmpty.value = false;
        backup.bindings.twoFAVerifyType.value = 'backupcode';
        backup.bindings.backupCode.value = mockBackupCode;
        backup.bindings.verify();
        expect(mockStore.authorize2FA).toHaveBeenLastCalledWith({
            token: mockTempToken,
            passcode: null,
            recoveryCode: mockBackupCode
        });
        await flush();
        expect(mockDoAfterLogin).toHaveBeenCalledWith(backupResponse);
        expect(backup.router.refreshPage).toHaveBeenCalled();
    });

    test('handles processed, readable, and raw second-factor failures', async () => {
        const { bindings } = setup();
        bindings.tempToken.value = mockTempToken;
        bindings.twoFAInputIsEmpty.value = false;
        bindings.passcode.value = mockPasscode;

        mockStore.authorize2FA.mockRejectedValueOnce({ processed: true, message: 'handled 2fa failure' });
        bindings.verify();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled 2fa failure');

        mockStore.authorize2FA.mockRejectedValueOnce({ processed: false, message: 'readable 2fa failure' });
        bindings.verify();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('readable 2fa failure');

        mockStore.authorize2FA.mockRejectedValueOnce('raw 2fa failure');
        bindings.verify();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw 2fa failure');
        expect(bindings.verifying.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
    });

    test('requests password-reset email and handles every service outcome', async () => {
        const { bindings } = setup();
        bindings.requestResetPassword();
        expect(mockShowAlert).toHaveBeenCalledWith('Email address cannot be blank');
        expect(mockStore.requestResetPassword).not.toHaveBeenCalled();

        bindings.forgetPasswordEmail.value = mockEmail;
        bindings.showForgetPasswordSheet.value = true;
        bindings.requestResetPassword();
        expect(mockStore.requestResetPassword).toHaveBeenCalledWith({ email: mockEmail });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('Password reset email has been sent');
        expect(bindings.showForgetPasswordSheet.value).toBe(false);

        mockStore.requestResetPassword.mockRejectedValueOnce({ processed: true, message: 'handled reset failure' });
        bindings.requestResetPassword();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled reset failure');

        mockStore.requestResetPassword.mockRejectedValueOnce({ processed: false, message: 'readable reset failure' });
        bindings.requestResetPassword();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('readable reset failure');

        mockStore.requestResetPassword.mockRejectedValueOnce('raw reset failure');
        bindings.requestResetPassword();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw reset failure');
        expect(bindings.requestingForgetPassword.value).toBe(false);
    });

    test('resends verification email only with a current synthetic password', async () => {
        const { bindings } = setup();
        bindings.resendVerifyEmail.value = mockEmail;
        bindings.requestResendVerifyEmail();
        expect(mockShowToast).toHaveBeenCalledWith('Current password cannot be blank');
        expect(mockStore.resendVerifyEmailByUnloginUser).not.toHaveBeenCalled();

        bindings.currentPasswordForResendVerifyEmail.value = mockPassword;
        bindings.showVerifyEmailSheet.value = true;
        bindings.requestResendVerifyEmail();
        expect(mockStore.resendVerifyEmailByUnloginUser).toHaveBeenCalledWith({
            email: mockEmail,
            password: mockPassword
        });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('Validation email has been sent');
        expect(bindings.showVerifyEmailSheet.value).toBe(false);

        mockStore.resendVerifyEmailByUnloginUser.mockRejectedValueOnce({ processed: true, message: 'handled resend failure' });
        bindings.requestResendVerifyEmail();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled resend failure');

        mockStore.resendVerifyEmailByUnloginUser.mockRejectedValueOnce({ processed: false, message: 'readable resend failure' });
        bindings.requestResendVerifyEmail();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('readable resend failure');

        mockStore.resendVerifyEmailByUnloginUser.mockRejectedValueOnce('raw resend failure');
        bindings.requestResendVerifyEmail();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw resend failure');
        expect(bindings.requestingResendVerifyEmail.value).toBe(false);
    });
});

describe('mobile LoginPage production template', () => {
    test('renders authentication variants and executes visible template event wrappers', async () => {
        const mounted = mountWithHostRenderer();
        try {
            const { nextTick } = jest.requireActual('vue') as any;
            mounted.state.username = mockUsername;
            mounted.state.password = mockPassword;
            mounted.state.inputIsEmpty = false;
            mounted.state.tempToken = mockTempToken;
            mounted.state.passcode = mockPasscode;
            mounted.state.backupCode = mockBackupCode;
            mounted.state.twoFAInputIsEmpty = false;
            mounted.state.forgetPasswordEmail = mockEmail;
            mounted.state.resendVerifyEmail = mockEmail;
            mounted.state.currentPasswordForResendVerifyEmail = mockPassword;
            mounted.state.show2faSheet = true;
            mounted.state.showForgetPasswordSheet = true;
            mounted.state.showVerifyEmailSheet = true;
            await nextTick();

            const fullCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(mounted.root, fullCallbacks);
            await invokeHostCallbacks(fullCallbacks);
            expect(fullCallbacks.length).toBeGreaterThan(15);
            expect(mockOpenExternalUrl).toHaveBeenCalledWith('https://github.com/mayswind/bill_analyser');

            mockOAuth2Enabled = false;
            mockRegistrationEnabled = false;
            mockForgetPasswordEnabled = false;
            mounted.state.tips = '';
            mounted.state.twoFAVerifyType = 'backupcode';
            mounted.state.hasValidEmailVerifyToken = true;
            mounted.state.loggingInByPassword = true;
            mounted.state.verifying = true;
            await nextTick();

            const restrictedCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(mounted.root, restrictedCallbacks);
            await invokeHostCallbacks(restrictedCallbacks);
            expect(restrictedCallbacks.length).toBeGreaterThan(8);

            mockInternalAuthEnabled = false;
            mockOAuth2Enabled = true;
            mounted.state.loggingInByPassword = false;
            mounted.state.loggingInByOAuth2 = true;
            await nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
