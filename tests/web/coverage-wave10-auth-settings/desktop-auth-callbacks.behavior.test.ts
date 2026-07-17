import { jest } from '@jest/globals';

let mockDarkTheme = false;
let mockVerifyEmailEnabled = true;
let mockLoggedIn = false;

const mockRouter = { push: jest.fn(), replace: jest.fn() };
const mockThemeName = { value: 'light' };
const mockAuthorizeOAuth2 = jest.fn<(...args: any[]) => Promise<any>>();
const mockVerifyEmail = jest.fn<(...args: any[]) => Promise<any>>();
const mockResendVerifyEmail = jest.fn<(...args: any[]) => Promise<any>>();
const mockResetPassword = jest.fn<(...args: any[]) => Promise<any>>();
const mockNavigateToHomePage = jest.fn();
const mockDoAfterLogin = jest.fn();
const mockShowMessage = jest.fn();
const mockShowError = jest.fn();
let mockLastLoginBase: any;

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});
jest.mock('vue-router', () => ({ useRouter: () => mockRouter }));
jest.mock('vuetify', () => ({ useTheme: () => ({ global: { name: mockThemeName } }) }));
jest.mock('vuetify/components/VTextField', () => ({ VTextField: {} }));
jest.mock('@/components/desktop/SnackBar.vue', () => ({ __esModule: true, default: {} }));
jest.mock('@/components/desktop/ConfirmDialog.vue', () => ({ __esModule: true, default: {} }));
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
        const { ref } = jest.requireActual('vue') as any;
        mockLastLoginBase = {
            version: '10.2.0',
            password: ref(''),
            loggingInByOAuth2: ref(false),
            doAfterLogin: (...args: any[]) => mockDoAfterLogin(...args)
        };
        return mockLastLoginBase;
    }
}));
jest.mock('@/stores/index.ts', () => ({
    useRootStore: () => ({
        authorizeOAuth2: (...args: any[]) => mockAuthorizeOAuth2(...args),
        verifyEmail: (...args: any[]) => mockVerifyEmail(...args),
        resendVerifyEmailByUnloginUser: (...args: any[]) => mockResendVerifyEmail(...args),
        resetPassword: (...args: any[]) => mockResetPassword(...args)
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
jest.mock('@/lib/web.ts', () => ({ navigateToHomePage: (...args: any[]) => mockNavigateToHomePage(...args) }));
jest.mock('@/lib/server_settings.ts', () => ({
    isUserVerifyEmailEnabled: () => mockVerifyEmailEnabled,
    getOIDCCustomDisplayNames: () => ({ en: 'OIDC' })
}));
jest.mock('@/lib/userstate.ts', () => ({ isUserLogined: () => mockLoggedIn }));
jest.mock('@/lib/version.ts', () => ({ getClientDisplayVersion: () => '10.2.0' }));
jest.mock('@mdi/js', () => ({ mdiChevronLeft: 'chevron-left' }));

const OAuth2CallbackPage = (jest.requireActual('@/views/desktop/OAuth2CallbackPage.vue') as any).default;
const VerifyEmailPage = (jest.requireActual('@/views/desktop/VerifyEmailPage.vue') as any).default;
const ResetPasswordPage = (jest.requireActual('@/views/desktop/ResetPasswordPage.vue') as any).default;

function setup(component: any, props: Record<string, unknown>): any {
    return component.setup(props, { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() });
}

function attachFeedback(bindings: any): void {
    if (bindings.snackbar) {
        bindings.snackbar.value = { showMessage: mockShowMessage, showError: mockShowError };
    }
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await (jest.requireActual('vue') as any).nextTick();
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
    mockResetPassword.mockResolvedValue({});
});

describe('OAuth2CallbackPage', () => {
    test('projects provider, error, theme, and password validation state', () => {
        const bindings = setup(OAuth2CallbackPage, {
            provider: 'oidc', userName: 'alice', errorCode: '401', message: 'denied'
        });
        attachFeedback(bindings);

        expect(bindings.oauth2ProviderDisplayName.value).toBe('provider:oidc');
        expect(bindings.oauth2LoginDisplayName.value).toBe('login:oidc');
        expect(bindings.error.value).toEqual({ errorCode: 401, message: 'denied' });
        expect(bindings.isDarkMode.value).toBe(false);
        bindings.verifyAndLogin();
        expect(mockShowMessage).toHaveBeenCalledWith('Password cannot be blank');

        const withoutError = setup(OAuth2CallbackPage, { provider: undefined, userName: 'alice' });
        expect(withoutError.error.value).toBeUndefined();
        expect(withoutError.oauth2ProviderDisplayName.value).toBe('provider:');
        mockDarkTheme = true;
        expect(setup(OAuth2CallbackPage, { userName: 'alice' }).isDarkMode.value).toBe(true);
    });

    test.each([
        ['desktop', 'desktop'],
        ['mobile', 'mobile'],
        ['unknown', null]
    ])('authorizes password callback and navigates for %s platform', async (platform, expectedPlatform) => {
        const bindings = setup(OAuth2CallbackPage, {
            provider: 'oidc', platform, token: 'callback-token', userName: 'alice'
        });
        attachFeedback(bindings);
        bindings.password.value = 'secret';
        bindings.passcode.value = '123456';
        bindings.verifyAndLogin();
        expect(mockAuthorizeOAuth2).toHaveBeenCalledWith({
            password: 'secret', passcode: '123456', callbackToken: 'callback-token'
        });
        await flush();

        expect(mockDoAfterLogin).toHaveBeenCalled();
        expect(bindings.loggingInByOAuth2.value).toBe(false);
        if (expectedPlatform) {
            expect(mockNavigateToHomePage).toHaveBeenCalledWith(expectedPlatform);
        } else {
            expect(mockRouter.replace).toHaveBeenCalledWith('/');
        }
    });

    test('handles email-verification, two-factor, unprocessed, and processed failures', async () => {
        const cases = [
            {
                error: { processed: false, error: { errorCode: 1001, context: { email: 'alice+oauth@example.invalid', hasValidEmailVerifyToken: true } } },
                assert: () => expect(mockRouter.push).toHaveBeenCalledWith('/verify_email?email=alice%2Boauth%40example.invalid&emailSent=true')
            },
            {
                error: { processed: false, error: { errorCode: 1002 } },
                assert: (bindings: any) => expect(bindings.show2faInput.value).toBe(true)
            },
            {
                error: { processed: false, message: 'oauth failed' },
                assert: () => expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'oauth failed' }))
            },
            {
                error: { processed: true, message: 'already handled' },
                assert: () => expect(mockShowError).not.toHaveBeenCalled()
            }
        ];

        for (const item of cases) {
            jest.clearAllMocks();
            mockAuthorizeOAuth2.mockRejectedValueOnce(item.error);
            const bindings = setup(OAuth2CallbackPage, { token: 'token', userName: 'alice' });
            attachFeedback(bindings);
            bindings.password.value = 'secret';
            bindings.verifyAndLogin();
            await flush();
            item.assert(bindings);
            expect(bindings.loggingInByOAuth2.value).toBe(false);
        }
    });

    test('automatically exchanges callback tokens and handles automatic verification errors', async () => {
        const success = setup(OAuth2CallbackPage, { platform: 'desktop', token: 'callback-token' });
        attachFeedback(success);
        expect(mockAuthorizeOAuth2).toHaveBeenCalledWith({ callbackToken: 'callback-token' });
        await flush();
        expect(mockNavigateToHomePage).toHaveBeenCalledWith('desktop');

        jest.clearAllMocks();
        mockAuthorizeOAuth2.mockRejectedValueOnce({
            processed: false,
            error: { errorCode: 1001, context: { email: 'alice@example.invalid' } }
        });
        const verifyFailure = setup(OAuth2CallbackPage, { platform: 'mobile', token: 'callback-token' });
        attachFeedback(verifyFailure);
        await flush();
        expect(mockRouter.push).toHaveBeenCalledWith('/verify_email?email=alice%40example.invalid&emailSent=false');

        jest.clearAllMocks();
        mockVerifyEmailEnabled = false;
        mockAuthorizeOAuth2.mockRejectedValueOnce({ processed: false, message: 'auto failure' });
        const genericFailure = setup(OAuth2CallbackPage, { platform: 'mobile', token: 'callback-token' });
        attachFeedback(genericFailure);
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'auto failure' }));
    });
});

describe('VerifyEmailPage', () => {
    test('settles immediately without a token and requests verification with the correct login state', async () => {
        const idle = setup(VerifyEmailPage, { email: 'alice@example.invalid', hasValidEmailVerifyToken: false });
        attachFeedback(idle);
        expect(idle.loading.value).toBe(false);
        expect(mockVerifyEmail).not.toHaveBeenCalled();

        const verifying = setup(VerifyEmailPage, {
            email: 'alice@example.invalid', token: 'verify-token', hasValidEmailVerifyToken: true
        });
        attachFeedback(verifying);
        expect(mockVerifyEmail).toHaveBeenCalledWith({ token: 'verify-token', requestNewToken: true });
        await flush();
        expect(verifying.verified.value).toBe(true);
        expect(mockShowMessage).toHaveBeenCalledWith('Email address is verified');
    });

    test('shows unprocessed verification errors and redirects logged-in users after success feedback closes', async () => {
        mockLoggedIn = true;
        mockVerifyEmail.mockRejectedValueOnce({ processed: false, message: 'expired' });
        const failed = setup(VerifyEmailPage, {
            email: 'alice@example.invalid', token: 'expired-token', hasValidEmailVerifyToken: false
        });
        attachFeedback(failed);
        await flush();
        expect(mockVerifyEmail).toHaveBeenCalledWith({ token: 'expired-token', requestNewToken: false });
        expect(failed.verificationMessage.value).toBe('te:expired');
        expect(mockShowError).toHaveBeenCalled();

        mockVerifyEmail.mockResolvedValueOnce({});
        const success = setup(VerifyEmailPage, {
            email: 'alice@example.invalid', token: 'valid-token', hasValidEmailVerifyToken: false
        });
        attachFeedback(success);
        await flush();
        success.onSnackbarShowStateChanged(true);
        expect(mockRouter.replace).not.toHaveBeenCalled();
        success.onSnackbarShowStateChanged(false);
        expect(mockRouter.replace).toHaveBeenCalledWith('/');
    });

    test('resends validation email and reports only unprocessed errors', async () => {
        const bindings = setup(VerifyEmailPage, { email: 'alice@example.invalid', hasValidEmailVerifyToken: false });
        attachFeedback(bindings);
        bindings.password.value = 'secret';
        bindings.resendEmail();
        expect(mockResendVerifyEmail).toHaveBeenCalledWith({ email: 'alice@example.invalid', password: 'secret' });
        await flush();
        expect(mockShowMessage).toHaveBeenCalledWith('Validation email has been sent');

        mockResendVerifyEmail.mockRejectedValueOnce({ processed: false, message: 'resend failed' });
        bindings.resendEmail();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'resend failed' }));

        jest.clearAllMocks();
        mockResendVerifyEmail.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.resendEmail();
        await flush();
        expect(mockShowError).not.toHaveBeenCalled();
        expect(bindings.resending.value).toBe(false);
    });
});

describe('ResetPasswordPage', () => {
    test('reports every invalid reset combination before submitting', () => {
        const bindings = setup(ResetPasswordPage, { token: 'reset-token' });
        attachFeedback(bindings);
        expect(bindings.inputProblemMessage.value).toBe('Email address cannot be blank');
        bindings.email.value = 'alice@example.invalid';
        expect(bindings.inputProblemMessage.value).toBe('Nothing has been modified');
        bindings.confirmPassword.value = 'secret';
        expect(bindings.inputProblemMessage.value).toBe('New password cannot be blank');
        bindings.confirmPassword.value = '';
        bindings.newPassword.value = 'secret';
        expect(bindings.inputProblemMessage.value).toBe('Password confirmation cannot be blank');
        bindings.confirmPassword.value = 'different';
        expect(bindings.inputProblemMessage.value).toBe('Password and password confirmation do not match');
        bindings.resetPassword();
        expect(mockShowMessage).toHaveBeenCalledWith('Password and password confirmation do not match');
    });

    test('submits valid password changes and navigates only after success feedback closes', async () => {
        const bindings = setup(ResetPasswordPage, { token: 'reset-token' });
        attachFeedback(bindings);
        bindings.email.value = 'alice@example.invalid';
        bindings.newPassword.value = 'secret';
        bindings.confirmPassword.value = 'secret';
        expect(bindings.inputProblemMessage.value).toBeNull();
        bindings.resetPassword();
        expect(mockResetPassword).toHaveBeenCalledWith({
            token: 'reset-token', email: 'alice@example.invalid', password: 'secret'
        });
        await flush();
        expect(bindings.passwordChanged.value).toBe(true);
        expect(mockShowMessage).toHaveBeenCalledWith('Password has been updated');
        bindings.onSnackbarShowStateChanged(true);
        expect(mockRouter.replace).not.toHaveBeenCalled();
        bindings.onSnackbarShowStateChanged(false);
        expect(mockRouter.replace).toHaveBeenCalledWith('/login');
    });

    test('reports only unprocessed reset failures and clears pending state', async () => {
        const bindings = setup(ResetPasswordPage, { token: 'reset-token' });
        attachFeedback(bindings);
        bindings.email.value = 'alice@example.invalid';
        bindings.newPassword.value = 'secret';
        bindings.confirmPassword.value = 'secret';

        mockResetPassword.mockRejectedValueOnce({ processed: false, message: 'reset failed' });
        bindings.resetPassword();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'reset failed' }));
        expect(bindings.updating.value).toBe(false);
        expect(bindings.passwordChanged.value).toBe(false);

        jest.clearAllMocks();
        mockResetPassword.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.resetPassword();
        await flush();
        expect(mockShowError).not.toHaveBeenCalled();
    });
});
