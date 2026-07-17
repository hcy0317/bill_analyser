import { beforeEach, describe, expect, jest, test } from '@jest/globals';

let mockApplicationLockEnabled = false;

jest.mock('@/lib/settings.ts', () => ({
    __esModule: true,
    isEnableApplicationLock: () => mockApplicationLockEnabled
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: jest.fn(),
        error: jest.fn()
    }
}));

const mockAxios = {
    defaults: {
        headers: {
            common: {} as Record<string, string>
        }
    }
};

jest.mock('axios', () => ({
    __esModule: true,
    default: mockAxios
}));

function makeStorage(): Storage {
    const values = new Map<string, string>();

    return {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => { values.set(key, value); },
        removeItem: (key: string) => { values.delete(key); },
        clear: () => { values.clear(); },
        key: (index: number) => Array.from(values.keys())[index] ?? null,
        get length() { return values.size; }
    } as unknown as Storage;
}

function expectNoUnlockedCredentialState(): void {
    expect(sessionStorage.getItem('ebk_user_app_lock_state')).toBeNull();
    expect(sessionStorage.getItem('ebk_user_session_token')).toBeNull();
    expect(sessionStorage.getItem('ebk_user_session_encrypted_token')).toBeNull();
    expect(sessionStorage.getItem('ebk_user_session_refresh_token')).toBeNull();
    expect(sessionStorage.getItem('ebk_user_session_encrypted_refresh_token')).toBeNull();
    expect(mockAxios.defaults.headers.common['Authorization']).toBeUndefined();
}

function mutateEncryptedCredential(credential: string): string {
    const mutationIndex = credential.endsWith('=') ? credential.length - 2 : credential.length - 1;
    const replacement = credential[mutationIndex] === 'A' ? 'B' : 'A';
    return `${credential.slice(0, mutationIndex)}${replacement}${credential.slice(mutationIndex + 1)}`;
}

function mockCredentialDecryption(
    credential: string,
    outcome: 'throw' | 'empty'
): ReturnType<typeof jest.spyOn> {
    const CryptoJS = jest.requireActual('crypto-js') as {
        AES: { decrypt: (candidate: unknown, key: unknown) => unknown };
    };
    const originalDecrypt = CryptoJS.AES.decrypt.bind(CryptoJS.AES);
    return jest.spyOn(CryptoJS.AES, 'decrypt').mockImplementation((candidate, key) => {
        if (String(candidate) !== credential) {
            return originalDecrypt(candidate, key);
        }

        if (outcome === 'throw') {
            throw new Error('synthetic corrupted credential');
        }

        return { toString: () => '' };
    });
}

async function loadUserState(): Promise<typeof import('@/lib/userstate.ts')> {
    jest.resetModules();
    return import('@/lib/userstate.ts');
}

describe('userstate application-lock credential lifecycle', () => {
    beforeEach(() => {
        mockApplicationLockEnabled = false;
        mockAxios.defaults.headers.common = {};
        (globalThis as unknown as { localStorage: Storage }).localStorage = makeStorage();
        (globalThis as unknown as { sessionStorage: Storage }).sessionStorage = makeStorage();
    });

    test('locking encrypts both access and refresh credentials and makes neither consumable', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        expect(mockAxios.defaults.headers.common['Authorization']).toBe('Bearer access-before-lock');

        userstate.encryptToken('alice', '123456');

        expect(localStorage.getItem('ebk_user_token')).not.toBe('access-before-lock');
        expect(localStorage.getItem('ebk_user_refresh_token')).not.toBe('refresh-before-lock');
        expect(sessionStorage.getItem('ebk_user_session_refresh_token')).toBe('refresh-before-lock');

        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();

        expect(mockAxios.defaults.headers.common['Authorization']).toBeUndefined();
        expect(userstate.getCurrentToken()).toBeNull();
        expect(userstate.getCurrentRefreshToken()).toBeNull();
    });

    test('unlock restores both credentials and a rotated refresh token stays encrypted at rest', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();
        expect(mockAxios.defaults.headers.common['Authorization']).toBeUndefined();

        userstate.unlockTokenByPinCode('alice', '123456');

        expect(userstate.getCurrentToken()).toBe('access-before-lock');
        expect(userstate.getCurrentRefreshToken()).toBe('refresh-before-lock');
        expect(mockAxios.defaults.headers.common['Authorization']).toBe('Bearer access-before-lock');

        userstate.updateCurrentRefreshToken('rotated-refresh-token');
        expect(userstate.getCurrentRefreshToken()).toBe('rotated-refresh-token');
        expect(localStorage.getItem('ebk_user_refresh_token')).not.toBe('rotated-refresh-token');

        userstate.clearCurrentSessionToken();
        expect(userstate.getCurrentRefreshToken()).toBeNull();

        userstate.unlockTokenByPinCode('alice', '123456');
        expect(userstate.getCurrentRefreshToken()).toBe('rotated-refresh-token');
    });

    test('disabling the application lock restores both credentials to normal storage', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');
        mockApplicationLockEnabled = true;

        userstate.decryptToken();
        mockApplicationLockEnabled = false;

        expect(userstate.getCurrentToken()).toBe('access-before-lock');
        expect(userstate.getCurrentRefreshToken()).toBe('refresh-before-lock');
        expect(sessionStorage.getItem('ebk_user_session_refresh_token')).toBeNull();
        expect(sessionStorage.getItem('ebk_user_session_encrypted_refresh_token')).toBeNull();
    });

    test('logout cleanup removes refresh credential state from both storage scopes', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');

        userstate.clearCurrentTokenAndUserInfo(true);

        expect(localStorage.getItem('ebk_user_refresh_token')).toBeNull();
        expect(sessionStorage.getItem('ebk_user_session_refresh_token')).toBeNull();
        expect(sessionStorage.getItem('ebk_user_session_encrypted_refresh_token')).toBeNull();
        expect(mockAxios.defaults.headers.common['Authorization']).toBeUndefined();
    });

    test('unlock without a refresh credential keeps refresh unavailable and disabling lock removes stale state', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-only');
        userstate.encryptToken('alice', '123456');
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();

        userstate.unlockTokenByPinCode('alice', '123456');
        expect(userstate.getCurrentRefreshToken()).toBeNull();

        userstate.decryptToken();
        expect(localStorage.getItem('ebk_user_refresh_token')).toBeNull();
    });

    test('unlock migrates a legacy plaintext refresh credential without attempting AES decryption', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');
        const legacyRefreshToken = 'legacy-plaintext-refresh';
        localStorage.setItem('ebk_user_refresh_token', legacyRefreshToken);
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();
        const CryptoJS = jest.requireActual('crypto-js') as {
            AES: { decrypt: (credential: unknown, key: unknown) => unknown };
        };
        const decryptSpy = jest.spyOn(CryptoJS.AES, 'decrypt');

        try {
            userstate.unlockTokenByPinCode('alice', '123456');

            expect(decryptSpy.mock.calls.map(([credential]) => String(credential))).not.toContain(legacyRefreshToken);
            expect(userstate.getCurrentRefreshToken()).toBe(legacyRefreshToken);
            expect(localStorage.getItem('ebk_user_refresh_token')).not.toBe(legacyRefreshToken);
        } finally {
            decryptSpy.mockRestore();
        }
    });

    test('unlock rejects a malformed OpenSSL marker without migrating it as plaintext', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');
        const malformedCredential = 'U2FsdGVkX1-not-an-openssl-ciphertext';
        localStorage.setItem('ebk_user_refresh_token', malformedCredential);
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();

        expect(() => userstate.unlockTokenByPinCode('alice', '123456')).toThrow('Unable to decrypt refresh credential');

        expectNoUnlockedCredentialState();
        expect(localStorage.getItem('ebk_user_refresh_token')).toBe(malformedCredential);
    });

    test.each(['throw', 'empty'] as const)(
        'unlock fails atomically when an encrypted refresh credential produces %s during decryption',
        async (outcome) => {
            const userstate = await loadUserState();
            userstate.updateCurrentToken('access-before-lock');
            userstate.updateCurrentRefreshToken('refresh-before-lock');
            userstate.encryptToken('alice', '123456');
            const encryptedRefreshToken = localStorage.getItem('ebk_user_refresh_token');
            if (!encryptedRefreshToken) {
                throw new Error('encrypted refresh credential was not created');
            }
            const corruptedRefreshToken = mutateEncryptedCredential(encryptedRefreshToken);
            localStorage.setItem('ebk_user_refresh_token', corruptedRefreshToken);
            mockApplicationLockEnabled = true;
            userstate.clearCurrentSessionToken();
            const decryptSpy = mockCredentialDecryption(corruptedRefreshToken, outcome);

            try {
                expect(() => userstate.unlockTokenByPinCode('alice', '123456')).toThrow('Unable to decrypt refresh credential');
                expectNoUnlockedCredentialState();
                expect(localStorage.getItem('ebk_user_refresh_token')).toBe(corruptedRefreshToken);
            } finally {
                decryptSpy.mockRestore();
            }
        }
    );

    test('a locked upgraded session discards a legacy plaintext refresh credential before auth initialization', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');
        localStorage.setItem('ebk_user_refresh_token', 'legacy-plaintext-refresh');
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();

        expect(userstate.getCurrentToken()).toBeNull();
        expect(localStorage.getItem('ebk_user_refresh_token')).toBeNull();
        expect(userstate.getCurrentRefreshToken()).toBeNull();
    });

    test('WebAuthn unlock restores refresh and cache misses re-decrypt it with the unlocked state', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');
        const appLockState = userstate.getUserAppLockState();
        if (!appLockState) {
            throw new Error('application lock state was not created');
        }
        userstate.saveWebAuthnConfig('credential-1');
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();

        userstate.unlockTokenByWebAuthn('credential-1', 'alice', appLockState.secret);
        sessionStorage.removeItem('ebk_user_session_refresh_token');
        sessionStorage.removeItem('ebk_user_session_encrypted_refresh_token');

        expect(userstate.getCurrentRefreshToken()).toBe('refresh-before-lock');
        expect(sessionStorage.getItem('ebk_user_session_refresh_token')).toBe('refresh-before-lock');
    });

    test('invalid unlock and locked refresh updates fail closed', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();

        expect(() => userstate.unlockTokenByPinCode('alice', '000000')).toThrow('Unable to decrypt token');
        expectNoUnlockedCredentialState();
        userstate.updateCurrentRefreshToken('must-not-be-stored');
        userstate.updateCurrentRefreshToken(42 as unknown as string);

        expect(userstate.getCurrentRefreshToken()).toBeNull();
        expect(localStorage.getItem('ebk_user_refresh_token')).not.toBe('must-not-be-stored');
    });

    test.each(['throw', 'empty'] as const)(
        'refresh cache miss fails closed when encrypted credential decryption produces %s',
        async (outcome) => {
            const userstate = await loadUserState();
            userstate.updateCurrentToken('access-before-lock');
            userstate.updateCurrentRefreshToken('refresh-before-lock');
            userstate.encryptToken('alice', '123456');
            const encryptedRefreshToken = localStorage.getItem('ebk_user_refresh_token');
            if (!encryptedRefreshToken) {
                throw new Error('encrypted refresh credential was not created');
            }
            mockApplicationLockEnabled = true;
            userstate.clearCurrentSessionToken();
            userstate.unlockTokenByPinCode('alice', '123456');
            sessionStorage.removeItem('ebk_user_session_refresh_token');
            sessionStorage.removeItem('ebk_user_session_encrypted_refresh_token');
            const decryptSpy = mockCredentialDecryption(encryptedRefreshToken, outcome);

            try {
                expect(userstate.getCurrentRefreshToken()).toBeNull();
                expect(sessionStorage.getItem('ebk_user_session_refresh_token')).toBeNull();
                expect(sessionStorage.getItem('ebk_user_session_encrypted_refresh_token')).toBeNull();
            } finally {
                decryptSpy.mockRestore();
            }
        }
    );

    test('refresh cache miss rejects a malformed encrypted credential envelope', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.updateCurrentRefreshToken('refresh-before-lock');
        userstate.encryptToken('alice', '123456');
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();
        userstate.unlockTokenByPinCode('alice', '123456');
        sessionStorage.removeItem('ebk_user_session_refresh_token');
        sessionStorage.removeItem('ebk_user_session_encrypted_refresh_token');
        localStorage.setItem('ebk_user_refresh_token', 'U2FsdGVkX1-not-an-openssl-ciphertext');

        expect(userstate.getCurrentRefreshToken()).toBeNull();
        expect(sessionStorage.getItem('ebk_user_session_refresh_token')).toBeNull();
        expect(sessionStorage.getItem('ebk_user_session_encrypted_refresh_token')).toBeNull();
    });
});
