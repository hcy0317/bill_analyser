import { defineStore } from 'pinia';

import { useSettingsStore } from './setting.ts';
import { useUserStore } from './user.ts';

import type {
    TokenGenerateAPIResponse,
    TokenGenerateMCPResponse,
    TokenRefreshResponse,
    TokenInfoResponse
} from '@/models/token.ts';

import { isObject } from '@/lib/common.ts';

import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';

const tokenFailureCodes = new Set([
    'ERR_BAD_RESPONSE',
    'ERR_CANCELED',
    'ERR_NETWORK',
    'ECONNABORTED',
    'ETIMEDOUT'
]);

interface TokenStoreFailure {
    message: 'Token refresh failed' | 'Token revoke failed';
    route: 'tokens/refresh' | 'tokens/:id';
    code?: string;
    status?: number;
    noRefreshToken?: true;
}

function getTokenStoreFailure(
    reason: unknown,
    operation: 'refresh' | 'revoke'
): Readonly<TokenStoreFailure> {
    const failure: TokenStoreFailure = operation === 'refresh'
        ? { message: 'Token refresh failed', route: 'tokens/refresh' }
        : { message: 'Token revoke failed', route: 'tokens/:id' };

    if (typeof reason !== 'object' || reason === null) {
        return Object.freeze(failure);
    }

    try {
        const errorRecord = reason as Record<string, unknown>;
        const code = errorRecord['code'];
        if (typeof code === 'string' && tokenFailureCodes.has(code)) {
            failure.code = code;
        }

        let status = errorRecord['status'];
        if (operation === 'revoke' && (typeof status !== 'number' || !Number.isInteger(status))) {
            const response = errorRecord['response'];
            if (typeof response === 'object' && response !== null) {
                status = (response as Record<string, unknown>)['status'];
            }
        }
        if (typeof status === 'number' && Number.isInteger(status) && status >= 100 && status <= 599) {
            failure.status = status;
        }

        if (operation === 'refresh' && errorRecord['noRefreshToken'] === true) {
            failure.noRefreshToken = true;
        }
    } catch {
        return Object.freeze(failure);
    }

    return Object.freeze(failure);
}

/** 中文说明：token store 负责列出、生成、撤销 API/MCP token 和登录 session。 */
export const useTokensStore = defineStore('tokens', () => {
    const settingsStore = useSettingsStore();
    const userStore = useUserStore();

    function getAllTokens(): Promise<TokenInfoResponse[]> {
        return new Promise((resolve, reject) => {
            services.getTokens().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to retrieve session list' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to load token list', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve session list' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function refreshTokenAndRevokeOldToken(): Promise<TokenRefreshResponse> {
        return new Promise((resolve, reject) => {
            logger.info('[TokenStore] Starting token refresh and revoke old token');

            services.refreshToken().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    const failure = getTokenStoreFailure(undefined, 'refresh');
                    logger.error('[TokenStore] Token refresh failed', failure);
                    reject(failure);
                    return;
                }

                // 更新应用设置和用户信息
                if (data.result.applicationCloudSettings) {
                    settingsStore.setApplicationSettingsFromCloudSettings(data.result.applicationCloudSettings);
                }

                if (data.result.user && isObject(data.result.user)) {
                    userStore.storeUserBasicInfo(data.result.user);
                }

                // 异步撤销被轮换的 Token（不影响主流程）
                if (data.result.oldTokenId) {
                    revokeToken({
                        tokenId: data.result.oldTokenId,
                        ignoreError: true
                    }).catch(error => {
                        logger.warn(
                            '[TokenStore] Failed to revoke old token',
                            getTokenStoreFailure(error, 'revoke')
                        );
                    });
                }

                logger.info('[TokenStore] Token refresh completed successfully');
                resolve(data.result);
            }).catch(error => {
                const failure = getTokenStoreFailure(error, 'refresh');
                logger.error('[TokenStore] Token refresh failed', failure);
                reject(failure);
            });
        });
    }

    function generateToken<T extends 'api' | 'mcp'>({ type, expiresInSeconds, password }: { type: T, expiresInSeconds: number, password: string }): Promise<{ 'api': TokenGenerateAPIResponse, 'mcp': TokenGenerateMCPResponse }[T]> {
        return new Promise((resolve, reject) => {
            let promise = null;

            if (type === 'api') {
                promise = services.generateAPIToken({ expiresInSeconds, password });
            } else if (type === 'mcp') {
                promise = services.generateMCPToken({ expiresInSeconds, password });
            } else {
                reject({ message: 'An error occurred' });
                return;
            }

            promise.then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to generate token' });
                    return;
                }

                resolve(data.result as { 'api': TokenGenerateAPIResponse, 'mcp': TokenGenerateMCPResponse }[T]);
            }).catch(error => {
                logger.error('failed to generate token', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to generate token' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function revokeToken({ tokenId, ignoreError }: { tokenId: string, ignoreError?: boolean }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.revokeToken({ tokenId, ignoreError }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to logout from this session' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                const failure = getTokenStoreFailure(error, 'revoke');
                logger.error('[TokenStore] Token revoke failed', failure);
                reject(failure);
            });
        });
    }

    function revokeAllTokens(): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.revokeAllTokens().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to logout all other sessions' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to revoke all tokens', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to logout all other sessions' });
                } else {
                    reject(error);
                }
            });
        });
    }

    return {
        // 函数
        getAllTokens,
        refreshTokenAndRevokeOldToken,
        generateToken,
        revokeToken,
        revokeAllTokens
    };
});
