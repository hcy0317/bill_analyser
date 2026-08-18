import { beforeEach, describe, expect, jest, test } from '@jest/globals';

let mockApplicationLockEnabled = false;
const legacyAccessToken = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ0eXBlIjoiYWNjZXNzIiwiZXhwIjo0MTAyNDQ0ODAwfQ.signature';
const legacyAccessCiphertext = 'U2FsdGVkX18UKBz2l2ZPcsPQ10HiuyuQb7lDAL6P2lf7MzEKwQzL2WiGjuapbeKdA15RGImDGtcvO9pQKKcgTw8CTUKFXp3AArqW5ruzC22rYurcWIB/Q2Fplwkqsm2/qatNfPpiloiUa0bmc1Jx7A==';
const legacyRefreshToken = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ0eXBlIjoicmVmcmVzaCIsImV4cCI6NDEwMjQ0NDgwMH0.signature';
const legacyRefreshCiphertext = 'U2FsdGVkX19P3lq3w85d96A0m+s9ms+IIzvjBKUgeU1+JdEQ0tgfhH7jo8zu1x5vXzQbvdflkvP86gsFfuaVEt5szrrUTbdKlhwBz7aypRW44KT+7HOP4sdhaWh/bBrk0g8tJChovzUR07TeW4cz2A==';

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
    const replacement = credential[mutationIndex] === 'a' ? 'b' : 'a';
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
    const encryptedPayload = credential.split('.v1.')[0] ?? credential;
    return jest.spyOn(CryptoJS.AES, 'decrypt').mockImplementation((candidate, key) => {
        if (String(candidate) !== encryptedPayload) {
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
            mockApplicationLockEnabled = true;
            userstate.clearCurrentSessionToken();
            const decryptSpy = mockCredentialDecryption(encryptedRefreshToken, outcome);

            try {
                expect(() => userstate.unlockTokenByPinCode('alice', '123456')).toThrow('Unable to decrypt refresh credential');
                expect(decryptSpy).toHaveBeenCalled();
                expectNoUnlockedCredentialState();
                expect(localStorage.getItem('ebk_user_refresh_token')).toBe(encryptedRefreshToken);
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

    test('legacy ciphertext that yields wrong-key plaintext cannot unlock the session', async () => {
        const userstate = await loadUserState();
        localStorage.setItem(
            'ebk_user_token',
            'U2FsdGVkX1/acrHtmLIAugwD1mURuvcNowYoxMP8zj/yf8qIEl35sYF4vOdOX1Q3aKuAs/39ZvfsAREeqaXY+LYXSay4sXQcRbwZr47uq/4='
        );
        mockApplicationLockEnabled = true;

        expect(() => userstate.unlockTokenByPinCode('alice', '000000')).toThrow('Unable to decrypt token');
        expectNoUnlockedCredentialState();
    });

    test('valid legacy JWT ciphertext is upgraded after a successful unlock', async () => {
        const userstate = await loadUserState();
        localStorage.setItem('ebk_user_token', legacyAccessCiphertext);
        localStorage.setItem('ebk_user_refresh_token', legacyRefreshCiphertext);
        mockApplicationLockEnabled = true;

        userstate.unlockTokenByPinCode('alice', '123456');

        expect(userstate.getCurrentToken()).toBe(legacyAccessToken);
        expect(userstate.getCurrentRefreshToken()).toBe(legacyRefreshToken);
        expect(localStorage.getItem('ebk_user_token')).not.toBe(legacyAccessCiphertext);
        expect(localStorage.getItem('ebk_user_token')).toContain('.v1.');
        expect(localStorage.getItem('ebk_user_refresh_token')).not.toBe(legacyRefreshCiphertext);
        expect(localStorage.getItem('ebk_user_refresh_token')).toContain('.v1.');
    });

    test('malformed authenticated envelope is rejected by the credential boundary', async () => {
        const credentials = await import('@/lib/userstate/credentials.ts');
        const appLockState = {
            username: 'alice',
            secret: credentials.getAppLockSecret('123456')
        };
        const encryptedToken = credentials.getEncryptedToken('access-before-lock', appLockState);
        const ciphertext = encryptedToken.split('.v1.')[0];

        expect(() => credentials.getDecryptedToken(`${ciphertext}.v1.short`, appLockState)).toThrow(
            'Invalid encrypted credential'
        );
    });

    test.each([
        'U2FsdGVkX1+vVAB4pOHleZKmfZIfKIjQlpfPp99Lcaw=',
        'U2FsdGVkX1/+yXLRfO7bTCR14HS/U9MbgU5j7WOrAPcNa2/i5wc5VjY1ZJNSE0kS'
    ])('legacy ciphertext with a non-JWT payload cannot unlock the session', async (legacyCiphertext) => {
        const userstate = await loadUserState();
        localStorage.setItem('ebk_user_token', legacyCiphertext);
        mockApplicationLockEnabled = true;

        expect(() => userstate.unlockTokenByPinCode('alice', '123456')).toThrow('Unable to decrypt token');
        expectNoUnlockedCredentialState();
    });

    test('legacy encrypted refresh credential must have refresh JWT semantics', async () => {
        const userstate = await loadUserState();
        localStorage.setItem('ebk_user_token', legacyAccessCiphertext);
        localStorage.setItem('ebk_user_refresh_token', legacyAccessCiphertext);
        mockApplicationLockEnabled = true;

        expect(() => userstate.unlockTokenByPinCode('alice', '123456')).toThrow(
            'Unable to decrypt refresh credential'
        );
        expectNoUnlockedCredentialState();
        expect(localStorage.getItem('ebk_user_token')).toBe(legacyAccessCiphertext);
        expect(localStorage.getItem('ebk_user_refresh_token')).toBe(legacyAccessCiphertext);
    });

    test('tampered authenticated ciphertext fails before restoring session state', async () => {
        const userstate = await loadUserState();
        userstate.updateCurrentToken('access-before-lock');
        userstate.encryptToken('alice', '123456');
        const encryptedToken = localStorage.getItem('ebk_user_token');
        if (!encryptedToken) {
            throw new Error('encrypted access credential was not created');
        }
        localStorage.setItem('ebk_user_token', mutateEncryptedCredential(encryptedToken));
        mockApplicationLockEnabled = true;
        userstate.clearCurrentSessionToken();

        expect(() => userstate.unlockTokenByPinCode('alice', '123456')).toThrow('Unable to decrypt token');
        expectNoUnlockedCredentialState();
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
                expect(decryptSpy).toHaveBeenCalled();
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
