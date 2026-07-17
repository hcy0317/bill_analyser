import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { TOKEN_TYPE_API } from '@/models/token.ts';
import type { UserBasicInfo } from '@/models/user.ts';
import { EMPTY_USER_BASIC_INFO } from '@/models/user.ts';
import { useTokensStore } from '@/stores/token.ts';
import { useTwoFactorAuthStore } from '@/stores/twoFactorAuth.ts';
import { useUserExternalAuthStore } from '@/stores/userExternalAuth.ts';

type ApiResponse<T> = {
    data: {
        success: boolean;
        result: T;
    };
};

const mockSetApplicationSettingsFromCloudSettings = jest.fn();
const mockStoreUserBasicInfo = jest.fn();
const mockUpdateCurrentToken = jest.fn();
const mockUpdateCurrentRefreshToken = jest.fn();
const mockLoggerError = jest.fn();
const mockLoggerWarn = jest.fn();

const mockGetTokens = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockRefreshToken = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockRevokeToken = jest.fn<(req: unknown) => Promise<ApiResponse<boolean>>>();
const mockGenerateAPIToken = jest.fn<(req: unknown) => Promise<ApiResponse<unknown>>>();
const mockGet2FAStatus = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockEnable2FA = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockConfirmEnable2FA = jest.fn<(req: unknown) => Promise<ApiResponse<unknown>>>();
const mockDisable2FA = jest.fn<(req: unknown) => Promise<ApiResponse<unknown>>>();
const mockRegenerate2FARecoveryCode = jest.fn<(req: unknown) => Promise<ApiResponse<unknown>>>();
const mockGetExternalAuths = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockUnlinkExternalAuth = jest.fn<(req: unknown) => Promise<ApiResponse<boolean>>>();

jest.mock('@/stores/setting.ts', () => ({
    __esModule: true,
    useSettingsStore: () => ({
        setApplicationSettingsFromCloudSettings: mockSetApplicationSettingsFromCloudSettings
    })
}));

jest.mock('@/stores/user.ts', () => ({
    __esModule: true,
    useUserStore: () => ({
        storeUserBasicInfo: mockStoreUserBasicInfo
    })
}));

jest.mock('@/lib/userstate.ts', () => ({
    __esModule: true,
    updateCurrentToken: (token: string) => mockUpdateCurrentToken(token),
    updateCurrentRefreshToken: (refreshToken: string) => mockUpdateCurrentRefreshToken(refreshToken)
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getTokens: () => mockGetTokens(),
        refreshToken: () => mockRefreshToken(),
        revokeToken: (req: unknown) => mockRevokeToken(req),
        generateAPIToken: (req: unknown) => mockGenerateAPIToken(req),
        get2FAStatus: () => mockGet2FAStatus(),
        enable2FA: () => mockEnable2FA(),
        confirmEnable2FA: (req: unknown) => mockConfirmEnable2FA(req),
        disable2FA: (req: unknown) => mockDisable2FA(req),
        regenerate2FARecoveryCode: (req: unknown) => mockRegenerate2FARecoveryCode(req),
        getExternalAuths: () => mockGetExternalAuths(),
        unlinkExternalAuth: (req: unknown) => mockUnlinkExternalAuth(req)
    }
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: (...args: Array<unknown>) => mockLoggerWarn(...args),
        error: (...args: Array<unknown>) => mockLoggerError(...args)
    }
}));

function apiResponse<T>(result: T): ApiResponse<T> {
    return {
        data: {
            success: true,
            result
        }
    };
}

function user(overrides: Partial<UserBasicInfo> = {}): UserBasicInfo {
    return {
        ...EMPTY_USER_BASIC_INFO,
        id: 7,
        username: 'security-user',
        email: 'security@example.test',
        ...overrides
    };
}

describe('auth security stores', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        mockSetApplicationSettingsFromCloudSettings.mockClear();
        mockStoreUserBasicInfo.mockClear();
        mockUpdateCurrentToken.mockClear();
        mockUpdateCurrentRefreshToken.mockClear();
        mockLoggerError.mockClear();
        mockLoggerWarn.mockClear();
        mockGetTokens.mockReset();
        mockRefreshToken.mockReset();
        mockRevokeToken.mockReset();
        mockGenerateAPIToken.mockReset();
        mockGet2FAStatus.mockReset();
        mockEnable2FA.mockReset();
        mockConfirmEnable2FA.mockReset();
        mockDisable2FA.mockReset();
        mockRegenerate2FARecoveryCode.mockReset();
        mockGetExternalAuths.mockReset();
        mockUnlinkExternalAuth.mockReset();
    });

    test('refreshTokenAndRevokeOldToken consumes coordinator-persisted tokens and updates profile state', async () => {
        mockRefreshToken.mockResolvedValue(apiResponse({
            newToken: 'new-access-token',
            refreshToken: 'new-refresh-token',
            oldTokenId: 'old-session',
            user: user({ username: 'rotated-user' }),
            applicationCloudSettings: [{ settingKey: 'theme', settingValue: 'dark' }]
        }));
        mockRevokeToken.mockResolvedValue(apiResponse(true));
        const store = useTokensStore();

        await expect(store.refreshTokenAndRevokeOldToken()).resolves.toMatchObject({
            newToken: 'new-access-token',
            refreshToken: 'new-refresh-token',
            oldTokenId: 'old-session'
        });
        await Promise.resolve();

        expect(mockUpdateCurrentToken).not.toHaveBeenCalled();
        expect(mockUpdateCurrentRefreshToken).not.toHaveBeenCalled();
        expect(mockSetApplicationSettingsFromCloudSettings).toHaveBeenCalledWith([
            { settingKey: 'theme', settingValue: 'dark' }
        ]);
        expect(mockStoreUserBasicInfo).toHaveBeenCalledWith(expect.objectContaining({
            username: 'rotated-user'
        }));
        expect(mockRevokeToken).toHaveBeenCalledWith({
            tokenId: 'old-session',
            ignoreError: true
        });
    });

    test('refreshTokenAndRevokeOldToken rejects malformed envelopes with a safe failure', async () => {
        mockRefreshToken.mockResolvedValue({
            data: {
                success: false,
                result: null
            }
        });
        const store = useTokensStore();

        const consumerFailure = await store.refreshTokenAndRevokeOldToken().then(
            () => { throw new Error('refresh unexpectedly resolved'); },
            reason => reason
        );

        expect(consumerFailure).toStrictEqual({
            message: 'Token refresh failed',
            route: 'tokens/refresh'
        });
        expect(Object.isFrozen(consumerFailure)).toBe(true);
        expect(mockLoggerError).toHaveBeenCalledWith('[TokenStore] Token refresh failed', consumerFailure);
    });

    test('refreshTokenAndRevokeOldToken never logs raw refresh rejection details', async () => {
        const sentinel = 'TOKEN-STORE-REFRESH-SENTINEL';
        const hostileFailure = {
            message: `refresh failed: ${sentinel}`,
            code: 'ERR_NETWORK',
            status: 503,
            noRefreshToken: true,
            config: {
                data: JSON.stringify({ refreshToken: sentinel })
            },
            request: { body: sentinel },
            response: { data: { refreshToken: sentinel } }
        };
        mockRefreshToken.mockRejectedValue(hostileFailure);
        const store = useTokensStore();

        const consumerFailure = await store.refreshTokenAndRevokeOldToken().then(
            () => { throw new Error('refresh unexpectedly resolved'); },
            reason => reason
        );

        expect(consumerFailure).not.toBe(hostileFailure);
        expect(consumerFailure).toStrictEqual({
            message: 'Token refresh failed',
            route: 'tokens/refresh',
            code: 'ERR_NETWORK',
            status: 503,
            noRefreshToken: true
        });
        expect(Object.keys(consumerFailure as Record<string, unknown>).sort()).toStrictEqual([
            'code',
            'message',
            'noRefreshToken',
            'route',
            'status'
        ]);
        expect(Object.isFrozen(consumerFailure)).toBe(true);
        expect(mockLoggerError).toHaveBeenCalledWith('[TokenStore] Token refresh failed', consumerFailure);
        const serializedLoggerCalls = JSON.stringify(mockLoggerError.mock.calls);
        const serializedConsumerFailure = JSON.stringify(consumerFailure);
        expect(serializedLoggerCalls).not.toContain(sentinel);
        expect(serializedConsumerFailure).not.toContain(sentinel);
        expect(serializedLoggerCalls).not.toContain('refreshToken');
        expect(serializedConsumerFailure).not.toContain('refreshToken');
        expect(serializedLoggerCalls).not.toContain('config');
        expect(serializedConsumerFailure).not.toContain('config');
        expect(serializedLoggerCalls).not.toContain('request');
        expect(serializedConsumerFailure).not.toContain('request');
        expect(serializedLoggerCalls).not.toContain('response');
        expect(serializedConsumerFailure).not.toContain('response');
        expect(serializedConsumerFailure).not.toContain('data');
    });

    test('refreshTokenAndRevokeOldToken sanitizes old-session revoke failures before logging', async () => {
        const sentinel = 'TOKEN-STORE-REVOKE-SENTINEL';
        const hostileRevokeFailure = {
            message: `revoke failed: ${sentinel}`,
            code: 'ERR_BAD_RESPONSE',
            config: {
                headers: { Authorization: `Bearer ${sentinel}` }
            },
            request: { body: sentinel },
            response: {
                status: 502,
                data: {
                    refreshToken: sentinel
                }
            }
        };
        mockRefreshToken.mockResolvedValue(apiResponse({
            newToken: 'new-access-token',
            refreshToken: 'new-refresh-token',
            oldTokenId: 'old-session'
        }));
        mockRevokeToken.mockRejectedValue(hostileRevokeFailure);
        const store = useTokensStore();

        await expect(store.refreshTokenAndRevokeOldToken()).resolves.toMatchObject({
            oldTokenId: 'old-session'
        });
        await Promise.resolve();
        await Promise.resolve();

        const expectedFailure = {
            message: 'Token revoke failed',
            route: 'tokens/:id',
            code: 'ERR_BAD_RESPONSE',
            status: 502
        };
        expect(mockLoggerError).toHaveBeenCalledWith('[TokenStore] Token revoke failed', expectedFailure);
        expect(mockLoggerWarn).toHaveBeenCalledWith('[TokenStore] Failed to revoke old token', expectedFailure);
        const serializedLogs = JSON.stringify([
            ...mockLoggerError.mock.calls,
            ...mockLoggerWarn.mock.calls
        ]);
        expect(serializedLogs).not.toContain(sentinel);
        expect(serializedLogs).not.toContain('refreshToken');
        expect(serializedLogs).not.toContain('Authorization');
        expect(serializedLogs).not.toContain('config');
        expect(serializedLogs).not.toContain('request');
        expect(serializedLogs).not.toContain('response');
    });

    test('token list and api token generation preserve response envelopes', async () => {
        mockGetTokens.mockResolvedValue(apiResponse([{
            tokenId: 'session-1',
            tokenType: TOKEN_TYPE_API,
            userAgent: 'Bill Analyser API Token',
            lastSeen: 1711785600000,
            isCurrent: true
        }]));
        mockGenerateAPIToken.mockResolvedValue(apiResponse({
            token: 'api-token',
            apiBaseUrl: 'http://127.0.0.1:5000/api'
        }));
        const store = useTokensStore();

        await expect(store.getAllTokens()).resolves.toMatchObject([{
            tokenId: 'session-1',
            tokenType: TOKEN_TYPE_API
        }]);
        await expect(store.generateToken({
            type: 'api',
            expiresInSeconds: 3600,
            password: 'current-password'
        })).resolves.toStrictEqual({
            token: 'api-token',
            apiBaseUrl: 'http://127.0.0.1:5000/api'
        });
        expect(mockGenerateAPIToken).toHaveBeenCalledWith({
            expiresInSeconds: 3600,
            password: 'current-password'
        });
    });

    test('2FA status validates boolean enable and confirm writes rotated tokens', async () => {
        mockGet2FAStatus.mockResolvedValue(apiResponse({ enable: true }));
        mockConfirmEnable2FA.mockResolvedValue(apiResponse({
            token: '2fa-access-token',
            refreshToken: '2fa-refresh-token',
            recoveryCodes: ['ABCD-1234']
        }));
        const store = useTwoFactorAuthStore();

        await expect(store.get2FAStatus()).resolves.toStrictEqual({ enable: true });
        await expect(store.confirmEnable2FA({
            secret: 'otp-secret',
            passcode: '123456'
        })).resolves.toMatchObject({
            token: '2fa-access-token',
            recoveryCodes: ['ABCD-1234']
        });
        expect(mockUpdateCurrentToken).toHaveBeenCalledWith('2fa-access-token');
        expect(mockUpdateCurrentRefreshToken).toHaveBeenCalledWith('2fa-refresh-token');
        expect(mockConfirmEnable2FA).toHaveBeenCalledWith({
            secret: 'otp-secret',
            passcode: '123456'
        });
    });

    test('2FA enable, disable and recovery actions preserve valid response contracts', async () => {
        mockEnable2FA.mockResolvedValue(apiResponse({
            qrcode: 'data:image/png;base64,qr',
            secret: 'otp-secret'
        }));
        mockConfirmEnable2FA.mockResolvedValue(apiResponse({
            token: 'access-without-refresh',
            refreshToken: 42,
            recoveryCodes: ['AAAA-BBBB']
        }));
        mockDisable2FA.mockResolvedValue(apiResponse(true));
        mockRegenerate2FARecoveryCode.mockResolvedValue(apiResponse({
            recoveryCodes: ['CCCC-DDDD']
        }));
        const store = useTwoFactorAuthStore();

        await expect(store.enable2FA()).resolves.toStrictEqual({
            qrcode: 'data:image/png;base64,qr',
            secret: 'otp-secret'
        });
        await expect(store.confirmEnable2FA({
            secret: 'otp-secret',
            passcode: '654321'
        })).resolves.toMatchObject({ token: 'access-without-refresh' });
        await expect(store.disable2FA({ password: 'current-password' })).resolves.toBe(true);
        await expect(store.regenerate2FARecoveryCode({
            password: 'current-password'
        })).resolves.toStrictEqual({
            recoveryCodes: ['CCCC-DDDD']
        });

        expect(mockUpdateCurrentToken).toHaveBeenCalledWith('access-without-refresh');
        expect(mockUpdateCurrentRefreshToken).not.toHaveBeenCalled();
        expect(mockDisable2FA).toHaveBeenCalledWith({ password: 'current-password' });
    });

    test('2FA actions reject malformed success envelopes', async () => {
        const malformed = {
            data: {
                success: false,
                result: null
            }
        } as ApiResponse<unknown>;
        const store = useTwoFactorAuthStore();

        mockGet2FAStatus.mockResolvedValueOnce(apiResponse({ enable: 'yes' }));
        await expect(store.get2FAStatus()).rejects.toStrictEqual({
            message: 'Unable to retrieve current two-factor authentication status'
        });

        mockEnable2FA.mockResolvedValueOnce(apiResponse({ qrcode: '', secret: '' }));
        await expect(store.enable2FA()).rejects.toStrictEqual({
            message: 'Unable to enable two-factor authentication'
        });

        mockConfirmEnable2FA.mockResolvedValueOnce(malformed);
        await expect(store.confirmEnable2FA({
            secret: 'otp-secret',
            passcode: '123456'
        })).rejects.toStrictEqual({
            message: 'Unable to enable two-factor authentication'
        });

        mockDisable2FA.mockResolvedValueOnce(apiResponse(false));
        await expect(store.disable2FA({ password: 'current-password' })).rejects.toStrictEqual({
            message: 'Unable to disable two-factor authentication'
        });

        mockRegenerate2FARecoveryCode.mockResolvedValueOnce(malformed);
        await expect(store.regenerate2FARecoveryCode({
            password: 'current-password'
        })).rejects.toStrictEqual({
            message: 'Unable to regenerate two-factor authentication backup codes'
        });
    });

    test('2FA actions route backend, unprocessed and processed failures distinctly', async () => {
        const store = useTwoFactorAuthStore();
        const actions = [
            {
                reject: (error: unknown) => mockGet2FAStatus.mockRejectedValueOnce(error),
                invoke: () => store.get2FAStatus(),
                fallbackMessage: 'Unable to retrieve current two-factor authentication status'
            },
            {
                reject: (error: unknown) => mockEnable2FA.mockRejectedValueOnce(error),
                invoke: () => store.enable2FA(),
                fallbackMessage: 'Unable to enable two-factor authentication'
            },
            {
                reject: (error: unknown) => mockConfirmEnable2FA.mockRejectedValueOnce(error),
                invoke: () => store.confirmEnable2FA({ secret: 'secret', passcode: '123456' }),
                fallbackMessage: 'Unable to enable two-factor authentication'
            },
            {
                reject: (error: unknown) => mockDisable2FA.mockRejectedValueOnce(error),
                invoke: () => store.disable2FA({ password: 'current-password' }),
                fallbackMessage: 'Unable to disable two-factor authentication'
            },
            {
                reject: (error: unknown) => mockRegenerate2FARecoveryCode.mockRejectedValueOnce(error),
                invoke: () => store.regenerate2FARecoveryCode({ password: 'current-password' }),
                fallbackMessage: 'Unable to regenerate two-factor authentication backup codes'
            }
        ];

        for (const [index, action] of actions.entries()) {
            const backendData = { message: `backend-${index}` };
            action.reject({ response: { data: backendData } });
            await expect(action.invoke()).rejects.toStrictEqual({ error: backendData });

            action.reject({ processed: false });
            await expect(action.invoke()).rejects.toStrictEqual({
                message: action.fallbackMessage
            });

            const processedError = { processed: true, marker: `processed-${index}` };
            action.reject(processedError);
            await expect(action.invoke()).rejects.toBe(processedError);
        }
    });

    test('2FA recovery regeneration rejects empty backup code lists', async () => {
        mockRegenerate2FARecoveryCode.mockResolvedValue(apiResponse({
            recoveryCodes: []
        }));
        const store = useTwoFactorAuthStore();

        await expect(store.regenerate2FARecoveryCode({
            password: 'current-password'
        })).rejects.toStrictEqual({
            message: 'Unable to regenerate two-factor authentication backup codes'
        });
    });

    test('external auth list and unlink preserve request and boolean response contract', async () => {
        mockGetExternalAuths.mockResolvedValue(apiResponse([{
            externalAuthCategory: 'oauth2',
            externalAuthType: 'github',
            linked: true,
            externalUsername: 'security-user',
            createdAt: 1711785600000
        }]));
        mockUnlinkExternalAuth.mockResolvedValue(apiResponse(true));
        const store = useUserExternalAuthStore();

        await expect(store.getExternalAuths()).resolves.toMatchObject([{
            externalAuthType: 'github',
            linked: true
        }]);
        await expect(store.unlinkExternalAuth({
            externalAuthType: 'github',
            password: 'current-password'
        })).resolves.toBe(true);
        expect(mockUnlinkExternalAuth).toHaveBeenCalledWith({
            externalAuthType: 'github',
            password: 'current-password'
        });
    });
});
