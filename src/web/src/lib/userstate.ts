import CryptoJS from 'crypto-js';
import axios from 'axios';

import type { ApplicationLockState, WebAuthnConfig } from '@/core/setting.ts';
import type { UserBasicInfo } from '@/models/user.ts';
import { normalizeUserBasicInfo } from '@/models/user.ts';
import type { TransactionDraft } from '@/models/transaction.ts';

import { isString, isObject } from './common.ts';
import { isEnableApplicationLock } from './settings.ts';
import logger from './logger.ts';

const appLockSecretBaseStringPrefix: string = 'EBK_LOCK_SECRET_';

const tokenLocalStorageKey: string = 'ebk_user_token';
const refreshTokenLocalStorageKey: string = 'ebk_user_refresh_token';
const webauthnConfigLocalStorageKey: string = 'ebk_user_webauthn_config';
const userInfoLocalStorageKey: string = 'ebk_user_info';
const transactionDraftLocalStorageKey: string = 'ebk_user_draft_transaction';

const tokenSessionStorageKey: string = 'ebk_user_session_token';
const encryptedTokenSessionStorageKey: string = 'ebk_user_session_encrypted_token';
const appLockStateSessionStorageKey: string = 'ebk_user_app_lock_state'; // { 'username': '', secret: '' }

function getAppLockSecret(pinCode: string): string {
    const hashedPinCode = CryptoJS.SHA256(appLockSecretBaseStringPrefix + pinCode).toString();
    return hashedPinCode.substring(0, 24); // 将 secret 放入 WebAuthn 的 user id（user id 总长度必须小于 64 字节）
}

function getEncryptedToken(token: string, appLockState: ApplicationLockState): string {
    const key = CryptoJS.SHA256(`${appLockSecretBaseStringPrefix}|${appLockState.username}|${appLockState.secret}`).toString();
    return CryptoJS.AES.encrypt(token, key).toString();
}

function getDecryptedToken(encryptedToken: string, appLockState: ApplicationLockState): string {
    const key = CryptoJS.SHA256(`${appLockSecretBaseStringPrefix}|${appLockState.username}|${appLockState.secret}`).toString();
    const bytes = CryptoJS.AES.decrypt(encryptedToken, key);
    return bytes.toString(CryptoJS.enc.Utf8);
}

export function isUserLogined(): boolean {
    return !!localStorage.getItem(tokenLocalStorageKey);
}

export function isUserUnlocked(): boolean {
    if (!isUserLogined()) {
        return false;
    }

    if (!isEnableApplicationLock()) {
        return true;
    }

    return !!sessionStorage.getItem(appLockStateSessionStorageKey) && !!sessionStorage.getItem(tokenSessionStorageKey);
}

export function hasUserAppLockState(): boolean {
    return !!getUserAppLockState();
}

export function getUserAppLockState(): ApplicationLockState | null {
    const data = sessionStorage.getItem(appLockStateSessionStorageKey);

    if (!data) {
        return null;
    }

    const appLockState = JSON.parse(data);

    if (!appLockState || !appLockState.username || !appLockState.secret) {
        return null;
    }

    return appLockState as ApplicationLockState;
}

export function unlockTokenByWebAuthn(credentialId: string, userName: string, userSecret: string): void {
    const webauthnConfigData = localStorage.getItem(webauthnConfigLocalStorageKey);

    if (!webauthnConfigData) {
        throw new Error('WebAuthn credential is not set');
    }

    const webauthnConfig = JSON.parse(webauthnConfigData) as WebAuthnConfig;

    if (webauthnConfig.credentialId !== credentialId) {
        throw new Error('WebAuthn credential is invalid');
    }

    const encryptedToken = localStorage.getItem(tokenLocalStorageKey);

    if (!encryptedToken) {
        throw new Error('No token in local storage');
    }

    const appLockState: ApplicationLockState = {
        username: userName,
        secret: userSecret
    };
    const token = getDecryptedToken(encryptedToken, appLockState);

    sessionStorage.setItem(appLockStateSessionStorageKey, JSON.stringify(appLockState));
    sessionStorage.setItem(encryptedTokenSessionStorageKey, encryptedToken);
    sessionStorage.setItem(tokenSessionStorageKey, token);
}

export function unlockTokenByPinCode(userName: string, pinCode: string): void {
    const encryptedToken = localStorage.getItem(tokenLocalStorageKey);

    if (!encryptedToken) {
        throw new Error('No token in local storage');
    }

    const appLockState: ApplicationLockState = {
        username: userName,
        secret: getAppLockSecret(pinCode)
    };
    const token = getDecryptedToken(encryptedToken, appLockState);

    sessionStorage.setItem(appLockStateSessionStorageKey, JSON.stringify(appLockState));
    sessionStorage.setItem(encryptedTokenSessionStorageKey, encryptedToken);
    sessionStorage.setItem(tokenSessionStorageKey, token);
}

export function encryptToken(userName: string, pinCode: string): void {
    const token = localStorage.getItem(tokenLocalStorageKey);

    if (!token) {
        throw new Error('No token in local storage');
    }

    const appLockState: ApplicationLockState = {
        username: userName,
        secret: getAppLockSecret(pinCode)
    };
    const encryptedToken = getEncryptedToken(token, appLockState);

    sessionStorage.setItem(appLockStateSessionStorageKey, JSON.stringify(appLockState));
    sessionStorage.setItem(encryptedTokenSessionStorageKey, encryptedToken);
    sessionStorage.setItem(tokenSessionStorageKey, token);
    localStorage.setItem(tokenLocalStorageKey, encryptedToken);
}

export function decryptToken(): void {
    const token = sessionStorage.getItem(tokenSessionStorageKey);

    if (!token) {
        throw new Error('No token in session storage');
    }

    localStorage.setItem(tokenLocalStorageKey, token);
    sessionStorage.removeItem(tokenSessionStorageKey);
    sessionStorage.removeItem(encryptedTokenSessionStorageKey);
    sessionStorage.removeItem(appLockStateSessionStorageKey);
}

export function isCorrectPinCode(pinCode: string): boolean {
    const secret = getAppLockSecret(pinCode);
    const appLockState = getUserAppLockState();

    if (!appLockState) {
        return false;
    }

    return appLockState && secret === appLockState.secret;
}

export function getCurrentToken(): string | null {
    const enableAppLock = isEnableApplicationLock();

    if (enableAppLock) {
        const usedEncryptedToken = sessionStorage.getItem(encryptedTokenSessionStorageKey);
        const currentEncryptedToken = localStorage.getItem(tokenLocalStorageKey);
        const sessionToken = sessionStorage.getItem(tokenSessionStorageKey);

        logger.debug(`[getCurrentToken] AppLock mode: hasSessionEnc=${!!usedEncryptedToken}, hasLocalEnc=${!!currentEncryptedToken}, hasSessionPlain=${!!sessionToken}`);

        if (!usedEncryptedToken || !currentEncryptedToken) {
            // 兜底：如果应用锁已启用，但当前没有 session key，
            // 则检查 localStorage 中的 token 是否其实是明文 token（未加密）。
            // 这种情况可能发生在刚登录并强制将 settings 中的 applicationLock 设为 false 时，
            // 但 settings store 尚未更新，或者 isEnableApplicationLock() 读到了过期数据。
            // 也可能是因为当时还没有锁状态，所以保存的是明文 token。

            if (currentEncryptedToken) {
                // 改进后的启发式判断：如果它看起来像 JWT（以 eyJ 开头），那基本可以确定是明文 token。
                if (currentEncryptedToken.startsWith('eyJ')) {
                    logger.debug(`[getCurrentToken] Fallback: Returning JWT from localStorage (length=${currentEncryptedToken.length})`);
                    return currentEncryptedToken;
                }

                // 如果它看起来不像 JWT，那它可能是加密后的，也可能只是另一种格式。
                // 但直接返回 null 一定会失败（Missing Header）。
                // 返回原值至少还有成功机会（比如它是明文但不是 JWT，或者后端能处理这种格式）。
                logger.warn(`[getCurrentToken] Fallback: Returning non-JWT token from localStorage. AppLock=${enableAppLock}`);
                return currentEncryptedToken;
            }

            logger.error(`[getCurrentToken] CRITICAL: No token in localStorage! AppLock=${enableAppLock}`);
            return null;
        }

        if (usedEncryptedToken === currentEncryptedToken) {
            if (sessionToken) {
                logger.debug(`[getCurrentToken] Returning cached decrypted token from sessionStorage (length=${sessionToken.length})`);
                return sessionToken;
            } else {
                logger.warn('[getCurrentToken] Encrypted tokens match but no plain token in sessionStorage!');
            }
        }

        // 重新解密 token
        logger.debug('[getCurrentToken] Encrypted token changed, re-decrypting...');

        const appLockState = getUserAppLockState();

        if (!appLockState) {
            logger.error('[getCurrentToken] AppLockState missing during re-decrypt');
            return null;
        }

        const token = getDecryptedToken(currentEncryptedToken, appLockState);

        sessionStorage.setItem(encryptedTokenSessionStorageKey, currentEncryptedToken);
        sessionStorage.setItem(tokenSessionStorageKey, token);

        logger.debug(`[getCurrentToken] Token re-decrypted successfully (length=${token.length})`);
        return token;
    } else {
        const token = localStorage.getItem(tokenLocalStorageKey);
        if (!token) {
            logger.debug('[getCurrentToken] No token in localStorage (normal mode)');
        } else {
            logger.debug(`[getCurrentToken] Normal mode: Returning token from localStorage (length=${token.length})`);
        }
        return token;
    }
}

export function updateCurrentToken(token: string): void {
    if (!isString(token)) {
        logger.error(`[updateCurrentToken] Token is not a string: ${typeof token}`);
        return;
    }

    const enableAppLock = isEnableApplicationLock();
    const hasLockState = hasUserAppLockState();
    logger.debug(`[updateCurrentToken] Storing token: length=${token.length}, AppLock=${enableAppLock}, HasLockState=${hasLockState}`);

    if (enableAppLock && hasLockState) {
        const appLockState = getUserAppLockState();
        const encryptedToken = getEncryptedToken(token, appLockState as ApplicationLockState);

        sessionStorage.setItem(encryptedTokenSessionStorageKey, encryptedToken);
        sessionStorage.setItem(tokenSessionStorageKey, token);
        localStorage.setItem(tokenLocalStorageKey, encryptedToken);
        logger.debug('[updateCurrentToken] Token encrypted and stored in localStorage + sessionStorage');
    } else {
        localStorage.setItem(tokenLocalStorageKey, token);
        logger.debug('[updateCurrentToken] Plain token stored in localStorage');
    }

    // 验证存储成功
    const stored = localStorage.getItem(tokenLocalStorageKey);
    if (!stored) {
        logger.error('[updateCurrentToken] CRITICAL: Token not found in localStorage after storage!');
    } else {
        logger.debug(`[updateCurrentToken] Verification: token stored successfully, length=${stored.length}`);

        // 关键修复：同步更新axios.defaults.headers.common
        // 这确保浏览器的CORS预检能识别Authorization头
        axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
        logger.debug('[updateCurrentToken] Updated axios.defaults.headers.common with new token');
    }
}

export function getCurrentRefreshToken(): string | null {
    return localStorage.getItem(refreshTokenLocalStorageKey);
}

export function updateCurrentRefreshToken(refreshToken: string): void {
    if (isString(refreshToken)) {
        localStorage.setItem(refreshTokenLocalStorageKey, refreshToken);
    }
}

export function hasWebAuthnConfig(): boolean {
    return !!getWebAuthnCredentialId();
}

export function getWebAuthnCredentialId(): string | undefined {
    const webauthnConfigData = localStorage.getItem(webauthnConfigLocalStorageKey);

    if (!webauthnConfigData) {
        return undefined;
    }

    const webauthnConfig = JSON.parse(webauthnConfigData) as WebAuthnConfig;

    return webauthnConfig.credentialId;
}

export function saveWebAuthnConfig(credentialId: string): void {
    const webAuthnConfig: WebAuthnConfig = {
        credentialId: credentialId
    };

    localStorage.setItem(webauthnConfigLocalStorageKey, JSON.stringify(webAuthnConfig));
}

export function clearWebAuthnConfig(): void {
    localStorage.removeItem(webauthnConfigLocalStorageKey);
}

export function getCurrentUserInfo(): UserBasicInfo | null {
    const data = localStorage.getItem(userInfoLocalStorageKey);

    if (!data) {
        logger.debug('[getCurrentUserInfo] 用户信息不存在于localStorage');
        return null;
    }

    // 🆕 记录原始localStorage数据，验证fiscalYearStart的来源
    logger.debug(`[getCurrentUserInfo] 原始localStorage数据: ${data.substring(0, 200)}...`);

    const userInfo = normalizeUserBasicInfo(JSON.parse(data) as UserBasicInfo);

    // 🆕 验证所有关键字段是否存在
    logger.debug('[getCurrentUserInfo] 解析后的用户信息:');
    logger.debug(`  - id: ${userInfo.id} (类型: ${typeof userInfo.id})`);
    logger.debug(`  - username: ${userInfo.username}`);
    logger.debug(`  - fiscalYearStart: ${userInfo.fiscalYearStart} (0x${userInfo.fiscalYearStart?.toString(16)}) (类型: ${typeof userInfo.fiscalYearStart})`);

    if (userInfo.fiscalYearStart) {
        const month = userInfo.fiscalYearStart >> 8;
        const day = userInfo.fiscalYearStart & 0xff;
        logger.debug(`  - 财年开始日期解码: ${month}月${day}日`);

        // fiscalYearStart 应为 month << 8 | day 的复合值；仅对明显异常值保留告警
        if (userInfo.fiscalYearStart === 1) {
            logger.warn(`[getCurrentUserInfo] ⚠️ fiscalYearStart=1异常！应该是复合值如513(0x201=2月1日)，可能是登录API未正确返回`);
        }
    } else {
        logger.warn(`[getCurrentUserInfo] ⚠️ fiscalYearStart字段不存在或为falsy值`);
    }

    return userInfo;
}

export function updateCurrentUserInfo(user: UserBasicInfo): void {
    if (isObject(user)) {
        localStorage.setItem(userInfoLocalStorageKey, JSON.stringify(normalizeUserBasicInfo(user)));
    }
}

export function clearCurrentUserInfo(): void {
    localStorage.removeItem(userInfoLocalStorageKey);
}

export function getUserTransactionDraft(): TransactionDraft | null {
    let data = localStorage.getItem(transactionDraftLocalStorageKey);

    if (!data) {
        return null;
    }

    if (isEnableApplicationLock()) {
        const appLockState = getUserAppLockState();

        if (!appLockState) {
            return null;
        }

        data = getDecryptedToken(data, appLockState);
    }

    return JSON.parse(data) as TransactionDraft;
}

export function updateUserTransactionDraft(transaction?: TransactionDraft | null): void {
    if (!isObject(transaction)) {
        return;
    }

    let data = JSON.stringify(transaction);

    if (isEnableApplicationLock()) {
        const appLockState = getUserAppLockState();

        if (!appLockState) {
            return;
        }

        data = getEncryptedToken(data, appLockState);
    }

    localStorage.setItem(transactionDraftLocalStorageKey, data);
}

export function clearUserTransactionDraft(): void {
    localStorage.removeItem(transactionDraftLocalStorageKey);
}

export function clearCurrentSessionToken(): void {
    sessionStorage.removeItem(tokenSessionStorageKey);
    sessionStorage.removeItem(encryptedTokenSessionStorageKey);
    sessionStorage.removeItem(appLockStateSessionStorageKey);
}

export function clearCurrentTokenAndUserInfo(clearAppLockState: boolean): void {
    logger.debug(`[clearCurrentTokenAndUserInfo] 开始清理: clearAppLockState=${clearAppLockState}`);

    if (clearAppLockState) {
        logger.debug('[clearCurrentTokenAndUserInfo] 清理AppLock状态');
        sessionStorage.removeItem(appLockStateSessionStorageKey);
    }

    logger.debug('[clearCurrentTokenAndUserInfo] 清理sessionStorage token');
    sessionStorage.removeItem(tokenSessionStorageKey);
    sessionStorage.removeItem(encryptedTokenSessionStorageKey);

    logger.debug('[clearCurrentTokenAndUserInfo] 清理localStorage token');
    localStorage.removeItem(tokenLocalStorageKey);
    localStorage.removeItem(refreshTokenLocalStorageKey);

    logger.debug('[clearCurrentTokenAndUserInfo] 清理用户草稿');
    clearUserTransactionDraft();

    logger.debug('[clearCurrentTokenAndUserInfo] 清理用户信息');
    clearCurrentUserInfo();

    // 关键修复：清除axios.defaults.headers.common中的Authorization
    logger.debug('[clearCurrentTokenAndUserInfo] 清理axios Authorization头');
    delete axios.defaults.headers.common['Authorization'];

    // 验证清理结果
    const tokenStillExists = localStorage.getItem(tokenLocalStorageKey);
    const axiosAuthStillExists = axios.defaults.headers.common['Authorization'];

    if (tokenStillExists || axiosAuthStillExists) {
        logger.error('[clearCurrentTokenAndUserInfo] ❌ 清理失败！', {
            tokenStillExists: !!tokenStillExists,
            axiosAuthStillExists: !!axiosAuthStillExists
        });
    } else {
        logger.debug('[clearCurrentTokenAndUserInfo] 所有token和用户信息已清理');
    }
}
