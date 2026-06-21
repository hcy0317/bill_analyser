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

const mockGetTokens = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockRefreshToken = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockRevokeToken = jest.fn<(req: unknown) => Promise<ApiResponse<boolean>>>();
const mockGenerateAPIToken = jest.fn<(req: unknown) => Promise<ApiResponse<unknown>>>();
const mockGet2FAStatus = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockConfirmEnable2FA = jest.fn<(req: unknown) => Promise<ApiResponse<unknown>>>();
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
        confirmEnable2FA: (req: unknown) => mockConfirmEnable2FA(req),
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
        warn: jest.fn(),
        error: jest.fn()
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
        mockGetTokens.mockReset();
        mockRefreshToken.mockReset();
        mockRevokeToken.mockReset();
        mockGenerateAPIToken.mockReset();
        mockGet2FAStatus.mockReset();
        mockConfirmEnable2FA.mockReset();
        mockRegenerate2FARecoveryCode.mockReset();
        mockGetExternalAuths.mockReset();
        mockUnlinkExternalAuth.mockReset();
    });

    test('refreshTokenAndRevokeOldToken writes rotated tokens, cloud settings and user profile', async () => {
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

        expect(mockUpdateCurrentToken).toHaveBeenCalledWith('new-access-token');
        expect(mockUpdateCurrentRefreshToken).toHaveBeenCalledWith('new-refresh-token');
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
