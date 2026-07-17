import { beforeEach, describe, expect, jest, test } from '@jest/globals';

let mockApplicationLockEnabled = false;

const mockNormalizeUserBasicInfo = jest.fn((user: Record<string, unknown>) => ({ ...user }));
const mockLogger = {
    debug: jest.fn(),
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn()
};
const mockAxios = {
    defaults: {
        headers: {
            common: {} as Record<string, string>
        }
    }
};

jest.mock('@/lib/settings.ts', () => ({
    __esModule: true,
    isEnableApplicationLock: () => mockApplicationLockEnabled
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: mockLogger
}));
jest.mock('@/models/user.ts', () => ({
    __esModule: true,
    normalizeUserBasicInfo: (user: Record<string, unknown>) => mockNormalizeUserBasicInfo(user)
}));
jest.mock('axios', () => ({
    __esModule: true,
    default: mockAxios
}));

import * as userstate from '@/lib/userstate.ts';

const tokenKey = 'ebk_user_token';
const refreshTokenKey = 'ebk_user_refresh_token';
const webauthnKey = 'ebk_user_webauthn_config';
const userInfoKey = 'ebk_user_info';
const draftKey = 'ebk_user_draft_transaction';
const sessionTokenKey = 'ebk_user_session_token';
const sessionEncryptedTokenKey = 'ebk_user_session_encrypted_token';
const sessionLockStateKey = 'ebk_user_app_lock_state';

function makeStorage(options: {
    readonly ignoredSetKeys?: ReadonlySet<string>;
    readonly stickyRemoveKeys?: ReadonlySet<string>;
} = {}): Storage {
    const values = new Map<string, string>();

    return {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => {
            if (!options.ignoredSetKeys?.has(key)) values.set(key, value);
        },
        removeItem: (key: string) => {
            if (!options.stickyRemoveKeys?.has(key)) values.delete(key);
        },
        clear: () => values.clear(),
        key: (index: number) => Array.from(values.keys())[index] ?? null,
        get length() { return values.size; }
    } as unknown as Storage;
}

function installStorage(local = makeStorage(), session = makeStorage()): void {
    (globalThis as unknown as { localStorage: Storage }).localStorage = local;
    (globalThis as unknown as { sessionStorage: Storage }).sessionStorage = session;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockApplicationLockEnabled = false;
    mockAxios.defaults.headers.common = {};
    mockNormalizeUserBasicInfo.mockImplementation(user => ({ ...user }));
    installStorage();
});

describe('userstate login, unlock, and token helpers', () => {
    test('distinguishes missing, ordinary, locked, and fully unlocked sessions', () => {
        expect(userstate.isUserLogined()).toBe(false);
        expect(userstate.isUserUnlocked()).toBe(false);
        expect(userstate.hasUserAppLockState()).toBe(false);
        expect(userstate.getUserAppLockState()).toBeNull();

        userstate.updateCurrentToken('plain-access-token');
        expect(userstate.isUserLogined()).toBe(true);
        expect(userstate.isUserUnlocked()).toBe(true);
        expect(userstate.getCurrentToken()).toBe('plain-access-token');
        expect(mockAxios.defaults.headers.common['Authorization']).toBe('Bearer plain-access-token');

        mockApplicationLockEnabled = true;
        expect(userstate.isUserUnlocked()).toBe(false);

        sessionStorage.setItem(sessionLockStateKey, JSON.stringify({ username: 'alice' }));
        expect(userstate.getUserAppLockState()).toBeNull();
        sessionStorage.setItem(sessionLockStateKey, JSON.stringify({ username: 'alice', secret: 'secret' }));
        expect(userstate.hasUserAppLockState()).toBe(true);
        expect(userstate.isUserUnlocked()).toBe(false);
        sessionStorage.setItem(sessionTokenKey, 'plain-access-token');
        expect(userstate.isUserUnlocked()).toBe(true);
    });

    test('reports token lifecycle precondition failures and validates PIN state', () => {
        expect(() => userstate.unlockTokenByPinCode('alice', '123456')).toThrow('No token in local storage');
        expect(() => userstate.encryptToken('alice', '123456')).toThrow('No token in local storage');
        expect(() => userstate.decryptToken()).toThrow('No token in session storage');
        expect(userstate.isCorrectPinCode('123456')).toBe(false);

        userstate.updateCurrentToken('access-token');
        userstate.encryptToken('alice', '123456');
        expect(userstate.isCorrectPinCode('123456')).toBe(true);
        expect(userstate.isCorrectPinCode('654321')).toBe(false);

        sessionStorage.removeItem('ebk_user_session_refresh_token');
        userstate.decryptToken();
        expect(localStorage.getItem(tokenKey)).toBe('access-token');
        expect(localStorage.getItem(refreshTokenKey)).toBeNull();
        expect(sessionStorage.getItem(sessionLockStateKey)).toBeNull();
    });

    test('covers locked-token fallbacks, cache re-decryption, and encrypted rotation', () => {
        mockApplicationLockEnabled = true;
        sessionStorage.setItem(sessionLockStateKey, JSON.stringify({ username: 'alice', secret: 'secret' }));

        localStorage.setItem(tokenKey, 'eyJ.synthetic.jwt');
        expect(userstate.getCurrentToken()).toBe('eyJ.synthetic.jwt');
        localStorage.setItem(tokenKey, 'opaque-access-token');
        expect(userstate.getCurrentToken()).toBe('opaque-access-token');
        localStorage.removeItem(tokenKey);
        expect(userstate.getCurrentToken()).toBeNull();

        mockApplicationLockEnabled = false;
        userstate.updateCurrentToken('before-lock');
        userstate.encryptToken('alice', '123456');
        mockApplicationLockEnabled = true;
        sessionStorage.removeItem(sessionTokenKey);
        expect(userstate.getCurrentToken()).toBe('before-lock');
        expect(sessionStorage.getItem(sessionTokenKey)).toBe('before-lock');

        userstate.updateCurrentToken('rotated-access');
        expect(localStorage.getItem(tokenKey)).not.toBe('rotated-access');
        expect(sessionStorage.getItem(sessionTokenKey)).toBe('rotated-access');
        expect(userstate.getCurrentToken()).toBe('rotated-access');
    });

    test('ignores non-string tokens and reports failed persistent storage', () => {
        userstate.updateCurrentToken(42 as unknown as string);
        expect(localStorage.getItem(tokenKey)).toBeNull();
        expect(mockLogger.error).toHaveBeenCalledWith(expect.stringContaining('Token is not a string'));

        installStorage(makeStorage({ ignoredSetKeys: new Set([tokenKey]) }));
        userstate.updateCurrentToken('cannot-be-stored');
        expect(mockLogger.error).toHaveBeenCalledWith(expect.stringContaining('Token not found in localStorage'));
        expect(mockAxios.defaults.headers.common['Authorization']).toBeUndefined();
    });
});

describe('userstate WebAuthn, user info, and draft persistence', () => {
    test('validates, saves, reads, and clears WebAuthn credential configuration', () => {
        expect(userstate.hasWebAuthnConfig()).toBe(false);
        expect(userstate.getWebAuthnCredentialId()).toBeUndefined();
        expect(() => userstate.unlockTokenByWebAuthn('credential-1', 'alice', 'secret')).toThrow(
            'WebAuthn credential is not set'
        );

        userstate.saveWebAuthnConfig('credential-1');
        expect(userstate.hasWebAuthnConfig()).toBe(true);
        expect(userstate.getWebAuthnCredentialId()).toBe('credential-1');
        expect(() => userstate.unlockTokenByWebAuthn('credential-2', 'alice', 'secret')).toThrow(
            'WebAuthn credential is invalid'
        );
        expect(() => userstate.unlockTokenByWebAuthn('credential-1', 'alice', 'secret')).toThrow(
            'No token in local storage'
        );

        userstate.clearWebAuthnConfig();
        expect(localStorage.getItem(webauthnKey)).toBeNull();
    });

    test('normalizes user info and exercises populated, anomalous, empty, and clear paths', () => {
        expect(userstate.getCurrentUserInfo()).toBeNull();
        userstate.updateCurrentUserInfo(null as unknown as never);
        expect(localStorage.getItem(userInfoKey)).toBeNull();

        const user = { id: 7, username: 'alice', fiscalYearStart: 0x0201 } as never;
        userstate.updateCurrentUserInfo(user);
        expect(mockNormalizeUserBasicInfo).toHaveBeenCalledWith(user);
        expect(userstate.getCurrentUserInfo()).toEqual(user);

        localStorage.setItem(userInfoKey, JSON.stringify({ id: 8, username: 'legacy', fiscalYearStart: 1 }));
        expect(userstate.getCurrentUserInfo()).toEqual({ id: 8, username: 'legacy', fiscalYearStart: 1 });
        expect(mockLogger.warn).toHaveBeenCalledWith(expect.stringContaining('fiscalYearStart=1'));

        localStorage.setItem(userInfoKey, JSON.stringify({ id: 9, username: 'empty-fiscal', fiscalYearStart: 0 }));
        expect(userstate.getCurrentUserInfo()).toEqual({ id: 9, username: 'empty-fiscal', fiscalYearStart: 0 });
        expect(mockLogger.warn).toHaveBeenCalledWith(expect.stringContaining('fiscalYearStart'));

        userstate.clearCurrentUserInfo();
        expect(localStorage.getItem(userInfoKey)).toBeNull();
    });

    test('round-trips plain and encrypted drafts and refuses locked drafts without a key', () => {
        const draft = { type: 2, sourceAmountCents: 12_345, comment: 'Lunch' };
        expect(userstate.getUserTransactionDraft()).toBeNull();
        userstate.updateUserTransactionDraft(undefined);
        userstate.updateUserTransactionDraft(null);
        expect(localStorage.getItem(draftKey)).toBeNull();

        userstate.updateUserTransactionDraft(draft);
        expect(userstate.getUserTransactionDraft()).toEqual(draft);
        userstate.clearUserTransactionDraft();
        expect(userstate.getUserTransactionDraft()).toBeNull();

        mockApplicationLockEnabled = true;
        userstate.updateUserTransactionDraft(draft);
        expect(localStorage.getItem(draftKey)).toBeNull();
        localStorage.setItem(draftKey, JSON.stringify(draft));
        expect(userstate.getUserTransactionDraft()).toBeNull();

        mockApplicationLockEnabled = false;
        userstate.updateCurrentToken('access-token');
        userstate.encryptToken('alice', '123456');
        mockApplicationLockEnabled = true;
        userstate.updateUserTransactionDraft(draft);
        expect(localStorage.getItem(draftKey)).not.toBe(JSON.stringify(draft));
        expect(userstate.getUserTransactionDraft()).toEqual(draft);
    });
});

describe('userstate cleanup', () => {
    test('clears both authorization header casings and preserves lock state when requested', () => {
        userstate.updateCurrentToken('access-token');
        userstate.updateCurrentRefreshToken('refresh-token');
        userstate.updateCurrentUserInfo({ username: 'alice', fiscalYearStart: 0x0101 } as never);
        userstate.updateUserTransactionDraft({ comment: 'draft' });
        sessionStorage.setItem(sessionLockStateKey, JSON.stringify({ username: 'alice', secret: 'secret' }));
        sessionStorage.setItem(sessionEncryptedTokenKey, 'encrypted-token');
        mockAxios.defaults.headers.common['authorization'] = 'Bearer lowercase';

        userstate.clearCurrentSessionToken();
        expect(mockAxios.defaults.headers.common['Authorization']).toBeUndefined();
        expect(mockAxios.defaults.headers.common['authorization']).toBeUndefined();

        sessionStorage.setItem(sessionLockStateKey, JSON.stringify({ username: 'alice', secret: 'secret' }));
        userstate.clearCurrentTokenAndUserInfo(false);
        expect(localStorage.getItem(tokenKey)).toBeNull();
        expect(localStorage.getItem(refreshTokenKey)).toBeNull();
        expect(localStorage.getItem(userInfoKey)).toBeNull();
        expect(localStorage.getItem(draftKey)).toBeNull();
        expect(sessionStorage.getItem(sessionLockStateKey)).not.toBeNull();
        expect(mockLogger.debug).toHaveBeenCalledWith('[clearCurrentTokenAndUserInfo] 所有token和用户信息已清理');
    });

    test('removes lock state when requested and reports stubborn credential storage', () => {
        const stickyLocalStorage = makeStorage({ stickyRemoveKeys: new Set([tokenKey]) });
        installStorage(stickyLocalStorage);
        localStorage.setItem(tokenKey, 'stubborn-token');
        sessionStorage.setItem(sessionLockStateKey, JSON.stringify({ username: 'alice', secret: 'secret' }));
        mockAxios.defaults.headers.common['Authorization'] = 'Bearer stubborn-token';

        userstate.clearCurrentTokenAndUserInfo(true);

        expect(sessionStorage.getItem(sessionLockStateKey)).toBeNull();
        expect(mockLogger.error).toHaveBeenCalledWith(
            '[clearCurrentTokenAndUserInfo] ❌ 清理失败！',
            { tokenStillExists: true, axiosAuthStillExists: false }
        );
    });
});
