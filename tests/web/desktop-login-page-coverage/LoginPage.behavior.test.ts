import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import {
    collectHostCallbacks,
    type HostNode,
    mountWithHostRenderer
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as any;
const mockKnownErrorCode = { UserEmailNotVerified: 201020 } as const;
const mockRouter = {
    push: jest.fn<(...args: any[]) => void>(),
    replace: jest.fn<(...args: any[]) => void>()
};
const mockThemeName = mockActualVue.ref('light');
const mockShowMessage = jest.fn<(...args: any[]) => void>();
const mockShowError = jest.fn<(...args: any[]) => void>();
const mockDoAfterLogin = jest.fn<(...args: any[]) => void>();
const mockGenerateRandomUUID = jest.fn(() => '<desktop-login-session>');
const mockTextFieldFocus = jest.fn();
const mockTextFieldSelect = jest.fn();

let mockInternalAuthEnabled = true;
let mockOAuth2Enabled = true;
let mockRegistrationEnabled = true;
let mockForgetPasswordEnabled = true;
let mockVerifyEmailEnabled = true;

const mockStore = {
    authorize: jest.fn<(...args: any[]) => Promise<any>>(),
    authorize2FA: jest.fn<(...args: any[]) => Promise<any>>()
};

function createLoginBase(): any {
    const { ref } = jest.requireActual('vue') as any;
    return {
        version: 'v-desktop-login-test',
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
        oauth2LoginUrl: ref('/api/oauth2/authorize?session=<desktop-login-session>'),
        oauth2LoginDisplayName: ref('Synthetic OAuth'),
        tips: ref('Authorized users only'),
        doAfterLogin: mockDoAfterLogin
    };
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});
jest.mock('vue-router', () => ({ useRouter: () => mockRouter }));
jest.mock('vuetify', () => ({
    useTheme: () => ({ global: { name: mockThemeName } })
}));
jest.mock('vuetify/components/VTextField', () => ({
    VTextField: mockActualVue.defineComponent({
        name: 'DesktopLoginTextFieldStub',
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, expose, slots }: any) => {
            expose({ focus: mockTextFieldFocus, select: mockTextFieldSelect });
            return () => mockActualVue.h(
                'text-field-stub',
                attrs,
                Object.values(slots).flatMap((slot: any) => slot?.() ?? [])
            );
        }
    })
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: mockActualVue.defineComponent({
        name: 'DesktopLoginSnackBarStub',
        setup: (_props: unknown, { expose }: any) => {
            expose({ showMessage: mockShowMessage, showError: mockShowError });
            return () => mockActualVue.h('snack-bar-stub');
        }
    })
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/views/base/LoginPageBase.ts', () => ({
    useLoginPageBase: (platform: string) => {
        if (platform !== 'desktop') throw new Error(`unexpected platform: ${platform}`);
        return createLoginBase();
    }
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => mockStore }));
jest.mock('@/core/theme.ts', () => ({
    isDarkApplicationTheme: (themeName: string) => themeName === 'dark'
}));
jest.mock('@/consts/asset.ts', () => ({ APPLICATION_LOGO_PATH: '/img/synthetic-logo.svg' }));
jest.mock('@/consts/api.ts', () => ({ KnownErrorCode: mockKnownErrorCode }));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => mockGenerateRandomUUID() }));
jest.mock('@/lib/server_settings.ts', () => ({
    isUserRegistrationEnabled: () => mockRegistrationEnabled,
    isUserForgetPasswordEnabled: () => mockForgetPasswordEnabled,
    isUserVerifyEmailEnabled: () => mockVerifyEmailEnabled,
    isInternalAuthEnabled: () => mockInternalAuthEnabled,
    isOAuth2Enabled: () => mockOAuth2Enabled
}));
jest.mock('@mdi/js', () => ({
    mdiOnepassword: 'one-password-icon',
    mdiHelpCircleOutline: 'help-circle-icon'
}));

import LoginPageModule from '@/views/desktop/LoginPage.vue';

const LoginPage = LoginPageModule as any;

function setup(): any {
    const bindings = LoginPage.setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    });
    bindings.snackbar.value = {
        showMessage: mockShowMessage,
        showError: mockShowError
    };
    return bindings;
}

function setPasswordCredentials(bindings: any): void {
    bindings.username.value = '<desktop-user>';
    bindings.password.value = '<desktop-password>';
    bindings.inputIsEmpty.value = false;
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
}

function findNodes(node: HostNode, predicate: (candidate: HostNode) => boolean): HostNode[] {
    const matches = predicate(node) ? [node] : [];
    for (const child of node.children) matches.push(...findNodes(child, predicate));
    return matches;
}

function findByTestId(root: HostNode, testId: string): HostNode | undefined {
    return findNodes(root, node => node.props['data-testid'] === testId)[0];
}

async function invokeTemplateCallbacks(root: HostNode): Promise<void> {
    for (const { name, callback } of collectHostCallbacks(root)) {
        if (name === 'onUpdate:modelValue') {
            callback('<template-input>');
        } else if (name === 'onUpdate:show') {
            callback(true);
        } else if (name === 'onKeyup') {
            callback({ key: 'Escape' });
            callback({ key: 'Enter' });
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
    mockThemeName.value = 'light';
    mockStore.authorize.mockResolvedValue({ user: { id: '<desktop-user-id>' } });
    mockStore.authorize2FA.mockResolvedValue({ user: { id: '<desktop-user-id>' } });
});

describe('desktop LoginPage password authentication', () => {
    test('initializes OAuth state, derives theme, and validates every password guard', () => {
        const bindings = setup();
        expect(mockGenerateRandomUUID).toHaveBeenCalledTimes(1);
        expect(bindings.oauth2ClientSessionId.value).toBe('<desktop-login-session>');
        expect(bindings.isDarkMode.value).toBe(false);
        mockThemeName.value = 'dark';
        expect(bindings.isDarkMode.value).toBe(true);

        bindings.login();
        expect(mockShowMessage).toHaveBeenCalledWith('Username cannot be blank');
        bindings.username.value = '<desktop-user>';
        bindings.login();
        expect(mockShowMessage).toHaveBeenCalledWith('Password cannot be blank');

        setPasswordCredentials(bindings);
        bindings.tempToken.value = '<challenge-token>';
        bindings.login();
        expect(bindings.show2faInput.value).toBe(true);
        expect(mockStore.authorize).not.toHaveBeenCalled();

        bindings.tempToken.value = '';
        bindings.loggingInByPassword.value = true;
        bindings.login();
        expect(mockStore.authorize).not.toHaveBeenCalled();
    });

    test('completes password login and routes home', async () => {
        const authResponse = { user: { id: '<password-user>' }, notificationContent: 'notice' };
        mockStore.authorize.mockResolvedValueOnce(authResponse);
        const bindings = setup();
        setPasswordCredentials(bindings);

        bindings.login();
        expect(bindings.loggingInByPassword.value).toBe(true);
        expect(mockStore.authorize).toHaveBeenCalledWith({
            loginName: '<desktop-user>',
            password: '<desktop-password>'
        });
        await flush();

        expect(bindings.loggingInByPassword.value).toBe(false);
        expect(mockDoAfterLogin).toHaveBeenCalledWith(authResponse);
        expect(mockRouter.replace).toHaveBeenCalledWith('/');
    });

    test('opens and focuses a second-factor challenge without completing login', async () => {
        mockStore.authorize.mockResolvedValueOnce({ need2FA: true, token: '<challenge-token>' });
        const bindings = setup();
        setPasswordCredentials(bindings);
        bindings.passcodeInput.value = {
            focus: mockTextFieldFocus,
            select: mockTextFieldSelect
        };

        bindings.login();
        await flush();
        expect(bindings.tempToken.value).toBe('<challenge-token>');
        expect(bindings.show2faInput.value).toBe(true);
        expect(mockTextFieldFocus).toHaveBeenCalled();
        expect(mockTextFieldSelect).toHaveBeenCalled();
        expect(mockDoAfterLogin).not.toHaveBeenCalled();

        mockStore.authorize.mockResolvedValueOnce({ need2FA: true, token: '<second-token>' });
        const withoutRef = setup();
        setPasswordCredentials(withoutRef);
        withoutRef.login();
        await flush();
        expect(withoutRef.tempToken.value).toBe('<second-token>');
    });

    test('routes complete unverified-email errors and rejects incomplete variants', async () => {
        const bindings = setup();
        setPasswordCredentials(bindings);
        const validError = (hasValidEmailVerifyToken?: boolean): any => ({
            processed: false,
            error: {
                errorCode: mockKnownErrorCode.UserEmailNotVerified,
                context: {
                    email: 'desktop+test@example.invalid',
                    hasValidEmailVerifyToken
                }
            }
        });

        mockStore.authorize.mockRejectedValueOnce(validError(true));
        bindings.login();
        await flush();
        expect(mockRouter.push).toHaveBeenLastCalledWith(
            '/verify_email?email=desktop%2Btest%40example.invalid&emailSent=true'
        );

        mockStore.authorize.mockRejectedValueOnce(validError());
        bindings.login();
        await flush();
        expect(mockRouter.push).toHaveBeenLastCalledWith(
            '/verify_email?email=desktop%2Btest%40example.invalid&emailSent=false'
        );

        const incompleteErrors = [
            { processed: true },
            { processed: true, error: { errorCode: 999999, context: { email: 'x@example.invalid' } } },
            { processed: true, error: { errorCode: mockKnownErrorCode.UserEmailNotVerified } },
            { processed: true, error: { errorCode: mockKnownErrorCode.UserEmailNotVerified, context: {} } }
        ];
        for (const error of incompleteErrors) {
            mockStore.authorize.mockRejectedValueOnce(error);
            bindings.login();
            await flush();
        }

        mockVerifyEmailEnabled = false;
        mockStore.authorize.mockRejectedValueOnce(validError(true));
        bindings.login();
        await flush();
        expect(mockRouter.push).toHaveBeenCalledTimes(2);
    });

    test('reports only unprocessed password failures and always releases busy state', async () => {
        const bindings = setup();
        setPasswordCredentials(bindings);

        const processed = { processed: true, message: 'already handled' };
        mockStore.authorize.mockRejectedValueOnce(processed);
        bindings.login();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(processed);

        const unprocessed = { processed: false, message: 'show this failure' };
        mockStore.authorize.mockRejectedValueOnce(unprocessed);
        bindings.login();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(unprocessed);
        expect(bindings.loggingInByPassword.value).toBe(false);
    });
});

describe('desktop LoginPage second-factor authentication', () => {
    test('validates guard, passcode, and recovery-code states', () => {
        const bindings = setup();
        bindings.verify();
        bindings.twoFAInputIsEmpty.value = false;
        bindings.verifying.value = true;
        bindings.verify();
        expect(mockStore.authorize2FA).not.toHaveBeenCalled();

        bindings.verifying.value = false;
        bindings.twoFAVerifyType.value = 'passcode';
        bindings.verify();
        expect(mockShowMessage).toHaveBeenCalledWith('Passcode cannot be blank');

        bindings.twoFAVerifyType.value = 'backupcode';
        bindings.verify();
        expect(mockShowMessage).toHaveBeenCalledWith('Backup code cannot be blank');
    });

    test('submits mutually exclusive passcode and recovery-code payloads', async () => {
        const passcodeResponse = { user: { id: '<passcode-user>' } };
        mockStore.authorize2FA.mockResolvedValueOnce(passcodeResponse);
        const passcode = setup();
        passcode.tempToken.value = '<challenge-token>';
        passcode.passcode.value = '012345';
        passcode.twoFAInputIsEmpty.value = false;
        passcode.verify();
        expect(mockStore.authorize2FA).toHaveBeenLastCalledWith({
            token: '<challenge-token>',
            passcode: '012345',
            recoveryCode: null
        });
        await flush();
        expect(mockDoAfterLogin).toHaveBeenCalledWith(passcodeResponse);
        expect(mockRouter.replace).toHaveBeenCalledWith('/');

        const recoveryResponse = { user: { id: '<recovery-user>' } };
        mockStore.authorize2FA.mockResolvedValueOnce(recoveryResponse);
        const recovery = setup();
        recovery.tempToken.value = '<challenge-token>';
        recovery.twoFAVerifyType.value = 'backupcode';
        recovery.backupCode.value = '<recovery-code>';
        recovery.twoFAInputIsEmpty.value = false;
        recovery.verify();
        expect(mockStore.authorize2FA).toHaveBeenLastCalledWith({
            token: '<challenge-token>',
            passcode: null,
            recoveryCode: '<recovery-code>'
        });
        await flush();
        expect(mockDoAfterLogin).toHaveBeenCalledWith(recoveryResponse);
    });

    test('reports only unprocessed verification failures and resets verifying', async () => {
        const bindings = setup();
        bindings.tempToken.value = '<challenge-token>';
        bindings.passcode.value = '012345';
        bindings.twoFAInputIsEmpty.value = false;

        const processed = { processed: true, message: 'already handled 2fa' };
        mockStore.authorize2FA.mockRejectedValueOnce(processed);
        bindings.verify();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(processed);

        const unprocessed = { processed: false, message: 'show 2fa failure' };
        mockStore.authorize2FA.mockRejectedValueOnce(unprocessed);
        bindings.verify();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(unprocessed);
        expect(bindings.verifying.value).toBe(false);
    });
});

describe('desktop LoginPage production template', () => {
    test('renders and executes internal, OAuth, 2FA, dark, and external-only states', async () => {
        const mounted = mountWithHostRenderer(LoginPage, {}, [
            'router-link', 'v-row', 'v-col', 'v-img', 'v-card', 'v-card-text', 'v-form',
            'v-spacer', 'v-divider', 'v-btn', 'v-progress-circular', 'language-select-button',
            'switch-to-mobile-dialog'
        ]);
        try {
            mounted.state.username = '<desktop-user>';
            mounted.state.password = '<desktop-password>';
            mounted.state.passcode = '012345';
            mounted.state.backupCode = '<recovery-code>';
            mounted.state.inputIsEmpty = false;
            mounted.state.twoFAInputIsEmpty = false;
            await flush();

            expect(findByTestId(mounted.root, 'desktop.auth.login.page')).toBeDefined();
            expect(findByTestId(mounted.root, 'desktop.auth.login.username')).toBeDefined();
            expect(findByTestId(mounted.root, 'desktop.auth.login.password')).toBeDefined();
            expect(findByTestId(mounted.root, 'desktop.auth.login.submit')).toBeDefined();
            expect(findNodes(mounted.root, node => node.props['src'] === 'img/desktop/background.svg')).toHaveLength(1);
            await invokeTemplateCallbacks(mounted.root);
            expect(mockTextFieldFocus).toHaveBeenCalled();

            mockThemeName.value = 'dark';
            mockOAuth2Enabled = false;
            mockRegistrationEnabled = false;
            mockForgetPasswordEnabled = false;
            mounted.state.show2faInput = true;
            mounted.state.twoFAVerifyType = 'passcode';
            mounted.state.loggingInByPassword = true;
            mounted.state.verifying = true;
            await flush();
            expect(findNodes(mounted.root, node => node.props['src'] === 'img/desktop/background-dark.svg')).toHaveLength(1);
            expect(findNodes(mounted.root, node => node.props['src'] === 'img/desktop/people1-dark.svg')).toHaveLength(1);

            mounted.state.loggingInByPassword = false;
            mounted.state.verifying = false;
            await flush();
            await invokeTemplateCallbacks(mounted.root);

            mounted.state.twoFAVerifyType = 'backupcode';
            mounted.state.tips = '';
            await flush();
            await invokeTemplateCallbacks(mounted.root);

            mockInternalAuthEnabled = false;
            mockOAuth2Enabled = true;
            mounted.state.show2faInput = false;
            mounted.state.loggingInByOAuth2 = true;
            await flush();
            expect(findByTestId(mounted.root, 'desktop.auth.login.username')).toBeUndefined();
            expect(findByTestId(mounted.root, 'desktop.auth.login.password')).toBeUndefined();
            expect(findNodes(mounted.root, node => node.props['href'] === '/api/oauth2/authorize?session=<desktop-login-session>')).toHaveLength(1);
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
