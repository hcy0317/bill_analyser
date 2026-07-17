import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

const mockSettingsStore = {
    setApplicationSettingsFromCloudSettings: jest.fn()
};
const mockUserStore = {
    storeUserBasicInfo: jest.fn()
};
const mockLogger = {
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn()
};
const mockServices = {
    getTokens: jest.fn<() => Promise<unknown>>(),
    refreshToken: jest.fn<() => Promise<unknown>>(),
    generateAPIToken: jest.fn<(request: unknown) => Promise<unknown>>(),
    generateMCPToken: jest.fn<(request: unknown) => Promise<unknown>>(),
    revokeToken: jest.fn<(request: unknown) => Promise<unknown>>(),
    revokeAllTokens: jest.fn<() => Promise<unknown>>()
};

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getTokens: () => mockServices.getTokens(),
        refreshToken: () => mockServices.refreshToken(),
        generateAPIToken: (request: unknown) => mockServices.generateAPIToken(request),
        generateMCPToken: (request: unknown) => mockServices.generateMCPToken(request),
        revokeToken: (request: unknown) => mockServices.revokeToken(request),
        revokeAllTokens: () => mockServices.revokeAllTokens()
    }
}));

import { useTokensStore } from '@/stores/token.ts';

function success<T>(result: T): Promise<{ data: { success: true; result: T } }> {
    return Promise.resolve({ data: { success: true, result } });
}

function malformed(): Promise<{ data: { success: false; result: null } }> {
    return Promise.resolve({ data: { success: false, result: null } });
}

describe('token store observable behavior', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        jest.clearAllMocks();
    });

    test('lists sessions and exposes malformed, backend, local and processed failures', async () => {
        const store = useTokensStore();
        mockServices.getTokens.mockReturnValueOnce(success([{ tokenId: 'current' }]));
        await expect(store.getAllTokens()).resolves.toEqual([{ tokenId: 'current' }]);

        mockServices.getTokens.mockReturnValueOnce(malformed());
        await expect(store.getAllTokens()).rejects.toEqual({ message: 'Unable to retrieve session list' });
        mockServices.getTokens.mockRejectedValueOnce({ response: { data: { message: 'backend detail' } } });
        await expect(store.getAllTokens()).rejects.toEqual({ error: { message: 'backend detail' } });
        mockServices.getTokens.mockRejectedValueOnce({ processed: false });
        await expect(store.getAllTokens()).rejects.toEqual({ message: 'Unable to retrieve session list' });
        const processed = { processed: true, marker: 'shown' };
        mockServices.getTokens.mockRejectedValueOnce(processed);
        await expect(store.getAllTokens()).rejects.toBe(processed);
    });

    test('refreshes profile and cloud state while old-session cleanup remains best effort', async () => {
        const store = useTokensStore();
        mockServices.refreshToken.mockReturnValueOnce(success({
            newToken: 'access',
            refreshToken: 'refresh',
            applicationCloudSettings: [{ settingKey: 'theme', settingValue: 'dark' }],
            user: { username: 'alice' },
            oldTokenId: 'old-session'
        }));
        mockServices.revokeToken.mockRejectedValueOnce({
            code: 'ERR_BAD_RESPONSE',
            response: { status: 503, data: { secret: 'must not leak' } }
        });

        await expect(store.refreshTokenAndRevokeOldToken()).resolves.toEqual(expect.objectContaining({
            newToken: 'access', oldTokenId: 'old-session'
        }));
        await Promise.resolve();
        await Promise.resolve();

        expect(mockSettingsStore.setApplicationSettingsFromCloudSettings).toHaveBeenCalledTimes(1);
        expect(mockUserStore.storeUserBasicInfo).toHaveBeenCalledWith({ username: 'alice' });
        expect(mockServices.revokeToken).toHaveBeenCalledWith({ tokenId: 'old-session', ignoreError: true });
        expect(mockLogger.warn).toHaveBeenCalledWith('[TokenStore] Failed to revoke old token', {
            message: 'Token revoke failed', route: 'tokens/:id', code: 'ERR_BAD_RESPONSE', status: 503
        });
    });

    test('refresh accepts optional fields being absent and rejects sanitized failure shapes', async () => {
        const store = useTokensStore();
        mockServices.refreshToken.mockReturnValueOnce(success({ newToken: 'access' }));
        await expect(store.refreshTokenAndRevokeOldToken()).resolves.toEqual({ newToken: 'access' });
        expect(mockSettingsStore.setApplicationSettingsFromCloudSettings).not.toHaveBeenCalled();
        expect(mockUserStore.storeUserBasicInfo).not.toHaveBeenCalled();
        expect(mockServices.revokeToken).not.toHaveBeenCalled();

        mockServices.refreshToken.mockReturnValueOnce(malformed());
        await expect(store.refreshTokenAndRevokeOldToken()).rejects.toEqual({
            message: 'Token refresh failed', route: 'tokens/refresh'
        });

        mockServices.refreshToken.mockRejectedValueOnce('network down');
        await expect(store.refreshTokenAndRevokeOldToken()).rejects.toEqual({
            message: 'Token refresh failed', route: 'tokens/refresh'
        });

        mockServices.refreshToken.mockRejectedValueOnce({
            code: 'ERR_NETWORK', status: 504, noRefreshToken: true
        });
        const failure = await store.refreshTokenAndRevokeOldToken().catch(reason => reason);
        expect(failure).toEqual({
            message: 'Token refresh failed', route: 'tokens/refresh',
            code: 'ERR_NETWORK', status: 504, noRefreshToken: true
        });
        expect(Object.isFrozen(failure)).toBe(true);

        const throwing = new Proxy({}, { get: () => { throw new Error('hostile getter'); } });
        mockServices.refreshToken.mockRejectedValueOnce(throwing);
        await expect(store.refreshTokenAndRevokeOldToken()).rejects.toEqual({
            message: 'Token refresh failed', route: 'tokens/refresh'
        });
    });

    test('generates both token kinds and rejects unsupported, malformed and failed requests', async () => {
        const store = useTokensStore();
        mockServices.generateAPIToken.mockReturnValueOnce(success({ token: 'api' }));
        await expect(store.generateToken({ type: 'api', expiresInSeconds: 60, password: 'secret' }))
            .resolves.toEqual({ token: 'api' });
        mockServices.generateMCPToken.mockReturnValueOnce(success({ token: 'mcp' }));
        await expect(store.generateToken({ type: 'mcp', expiresInSeconds: 120, password: 'secret' }))
            .resolves.toEqual({ token: 'mcp' });

        await expect(store.generateToken({
            type: 'unsupported' as 'api', expiresInSeconds: 1, password: 'secret'
        })).rejects.toEqual({ message: 'An error occurred' });

        mockServices.generateAPIToken.mockReturnValueOnce(malformed());
        await expect(store.generateToken({ type: 'api', expiresInSeconds: 60, password: 'secret' }))
            .rejects.toEqual({ message: 'Unable to generate token' });
        mockServices.generateAPIToken.mockRejectedValueOnce({ response: { data: { message: 'denied' } } });
        await expect(store.generateToken({ type: 'api', expiresInSeconds: 60, password: 'secret' }))
            .rejects.toEqual({ error: { message: 'denied' } });
        mockServices.generateAPIToken.mockRejectedValueOnce({ processed: false });
        await expect(store.generateToken({ type: 'api', expiresInSeconds: 60, password: 'secret' }))
            .rejects.toEqual({ message: 'Unable to generate token' });
        const processed = { processed: true };
        mockServices.generateAPIToken.mockRejectedValueOnce(processed);
        await expect(store.generateToken({ type: 'api', expiresInSeconds: 60, password: 'secret' }))
            .rejects.toBe(processed);
    });

    test('revokes one token with a redacted, bounded failure contract', async () => {
        const store = useTokensStore();
        mockServices.revokeToken.mockReturnValueOnce(success(true));
        await expect(store.revokeToken({ tokenId: 'one' })).resolves.toBe(true);
        mockServices.revokeToken.mockReturnValueOnce(malformed());
        await expect(store.revokeToken({ tokenId: 'one' })).rejects.toEqual({
            message: 'Unable to logout from this session'
        });

        mockServices.revokeToken.mockRejectedValueOnce({ status: 401.5, response: { status: 401 } });
        await expect(store.revokeToken({ tokenId: 'one' })).rejects.toEqual({
            message: 'Token revoke failed', route: 'tokens/:id', status: 401
        });
        mockServices.revokeToken.mockRejectedValueOnce({ code: 'NOT_ALLOWED', status: 99 });
        await expect(store.revokeToken({ tokenId: 'one' })).rejects.toEqual({
            message: 'Token revoke failed', route: 'tokens/:id'
        });
        mockServices.revokeToken.mockRejectedValueOnce({ response: null });
        await expect(store.revokeToken({ tokenId: 'one' })).rejects.toEqual({
            message: 'Token revoke failed', route: 'tokens/:id'
        });
    });

    test('revokes all other sessions and distinguishes every error channel', async () => {
        const store = useTokensStore();
        mockServices.revokeAllTokens.mockReturnValueOnce(success(true));
        await expect(store.revokeAllTokens()).resolves.toBe(true);
        mockServices.revokeAllTokens.mockReturnValueOnce(malformed());
        await expect(store.revokeAllTokens()).rejects.toEqual({ message: 'Unable to logout all other sessions' });
        mockServices.revokeAllTokens.mockRejectedValueOnce({ response: { data: { message: 'backend detail' } } });
        await expect(store.revokeAllTokens()).rejects.toEqual({ error: { message: 'backend detail' } });
        mockServices.revokeAllTokens.mockRejectedValueOnce({ processed: false });
        await expect(store.revokeAllTokens()).rejects.toEqual({ message: 'Unable to logout all other sessions' });
        const processed = { processed: true };
        mockServices.revokeAllTokens.mockRejectedValueOnce(processed);
        await expect(store.revokeAllTokens()).rejects.toBe(processed);
    });
});
