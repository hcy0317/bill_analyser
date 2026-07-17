import type { Browser } from '@playwright/test';

import { desktopRoute } from './routes';
import { E2EApiClient, E2EApiError } from './apiClient';
import type { E2EEnvironment } from './env';

export const AUTH_STORAGE_STATE_PATH = 'e2e/.auth/e2e-user.json';

export interface AuthResponse {
    readonly token: string;
    readonly refreshToken?: string;
    readonly need2FA?: boolean;
    readonly user?: Record<string, unknown>;
    readonly applicationCloudSettings?: unknown[];
}

interface RegisterResponse extends AuthResponse {
    readonly needVerifyEmail?: boolean;
}

export async function ensureE2EAccount(client: E2EApiClient, env: E2EEnvironment): Promise<AuthResponse> {
    try {
        return await loginE2EAccount(client, env);
    } catch (loginError) {
        const registered = await registerE2EAccount(client, env, loginError);
        if (registered.needVerifyEmail) {
            throw new Error(
                'E2E account registration requires email verification. Start the backend with BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION=false or provide a deterministic verification helper.'
            );
        }

        return registered.token ? registered : loginE2EAccount(client, env);
    }
}

export async function loginE2EAccount(client: E2EApiClient, env: E2EEnvironment): Promise<AuthResponse> {
    const auth = await client.post<AuthResponse>('auth/login', {
        loginName: env.account.email,
        password: env.account.password
    });

    if (!auth.token) {
        throw new Error('Login succeeded without an access token.');
    }

    if (auth.need2FA) {
        throw new Error('E2E test account requires 2FA; use a dedicated account without 2FA for deterministic automation.');
    }

    return auth;
}

export async function saveAuthenticatedStorageState(
    browser: Browser,
    env: E2EEnvironment,
    auth: AuthResponse
): Promise<void> {
    const context = await browser.newContext({ baseURL: env.baseURL });
    let primaryError: unknown = null;
    let cleanupError: unknown = null;

    try {
        const page = await context.newPage();
        const response = await page.goto(desktopRoute('/login', env), { waitUntil: 'commit' });
        if (!response) {
            throw new Error('E2E auth storage seed route did not return a document response.');
        }
        if (!response.ok()) {
            throw new Error(`E2E auth storage seed route returned HTTP ${response.status()}.`);
        }

        await page.evaluate(({ authState }) => {
            localStorage.setItem('ebk_user_token', authState.token);
            localStorage.setItem('ebk_last_login_time', String(Date.now()));

            if (authState.refreshToken) {
                localStorage.setItem('ebk_user_refresh_token', authState.refreshToken);
            }

            if (authState.user) {
                localStorage.setItem('ebk_user_info', JSON.stringify(authState.user));
            }
        }, { authState: auth });

        await context.storageState({ path: AUTH_STORAGE_STATE_PATH });
    } catch (error) {
        primaryError = error;
    }

    try {
        await context.close();
    } catch (error) {
        cleanupError = error;
    }

    if (primaryError && cleanupError) {
        throw new AggregateError(
            [primaryError, cleanupError],
            'E2E auth storage seed failed and temporary context cleanup failed.'
        );
    }
    if (cleanupError) {
        throw cleanupError;
    }
    if (primaryError) {
        throw primaryError;
    }
}

async function registerE2EAccount(
    client: E2EApiClient,
    env: E2EEnvironment,
    loginError: unknown
): Promise<RegisterResponse> {
    try {
        return await client.post<RegisterResponse>('auth/register', {
            username: env.account.username,
            email: env.account.email,
            nickname: env.account.nickname,
            password: env.account.password,
            language: 'en',
            defaultCurrency: 'USD',
            firstDayOfWeek: 1
        });
    } catch (registerError) {
        throw new Error(
            `Unable to login or register the E2E account. Login error: ${describeError(loginError)}. Register error: ${describeError(registerError)}.`
        );
    }
}

function describeError(error: unknown): string {
    if (error instanceof E2EApiError) {
        return `${error.message} ${JSON.stringify(error.body)}`;
    }

    if (error instanceof Error) {
        return error.message;
    }

    return String(error);
}
