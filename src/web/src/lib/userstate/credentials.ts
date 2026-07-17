import CryptoJS from 'crypto-js';
import axios from 'axios';

import type { ApplicationLockState } from '@/core/setting.ts';

import logger from '../logger.ts';

const appLockSecretBaseStringPrefix: string = 'EBK_LOCK_SECRET_';
const opensslSaltedCredentialPrefix: string = 'U2FsdGVkX1';
const opensslSaltedCredentialHexPrefix: string = '53616c7465645f5f';
const base64CredentialPattern: RegExp = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;

type StoredCredentialFormat = 'plaintext' | 'encrypted' | 'invalid';

export const tokenLocalStorageKey: string = 'ebk_user_token';
export const refreshTokenLocalStorageKey: string = 'ebk_user_refresh_token';
export const tokenSessionStorageKey: string = 'ebk_user_session_token';
export const encryptedTokenSessionStorageKey: string = 'ebk_user_session_encrypted_token';
export const refreshTokenSessionStorageKey: string = 'ebk_user_session_refresh_token';
export const encryptedRefreshTokenSessionStorageKey: string = 'ebk_user_session_encrypted_refresh_token';
export const appLockStateSessionStorageKey: string = 'ebk_user_app_lock_state'; // { 'username': '', secret: '' }

export function getAppLockSecret(pinCode: string): string {
    const hashedPinCode = CryptoJS.SHA256(appLockSecretBaseStringPrefix + pinCode).toString();
    return hashedPinCode.substring(0, 24); // 将 secret 放入 WebAuthn 的 user id（user id 总长度必须小于 64 字节）
}

export function getEncryptedToken(token: string, appLockState: ApplicationLockState): string {
    const key = CryptoJS.SHA256(`${appLockSecretBaseStringPrefix}|${appLockState.username}|${appLockState.secret}`).toString();
    return CryptoJS.AES.encrypt(token, key).toString();
}

export function getDecryptedToken(encryptedToken: string, appLockState: ApplicationLockState): string {
    const key = CryptoJS.SHA256(`${appLockSecretBaseStringPrefix}|${appLockState.username}|${appLockState.secret}`).toString();
    const bytes = CryptoJS.AES.decrypt(encryptedToken, key);
    return bytes.toString(CryptoJS.enc.Utf8);
}

function isEncryptedCredential(credential: string): boolean {
    if (
        !credential.startsWith(opensslSaltedCredentialPrefix)
        || credential.length < 44
        || credential.length % 4 !== 0
        || !base64CredentialPattern.test(credential)
    ) {
        return false;
    }

    const payload = CryptoJS.enc.Base64.parse(credential);

    return payload.sigBytes >= 32
        && (payload.sigBytes - 16) % 16 === 0
        && payload.toString(CryptoJS.enc.Hex).startsWith(opensslSaltedCredentialHexPrefix);
}

function getStoredCredentialFormat(credential: string): StoredCredentialFormat {
    if (!credential.startsWith(opensslSaltedCredentialPrefix)) {
        return 'plaintext';
    }

    return isEncryptedCredential(credential) ? 'encrypted' : 'invalid';
}

export function getDecryptedCredentialOrNull(
    encryptedCredential: string,
    appLockState: ApplicationLockState
): string | null {
    if (!isEncryptedCredential(encryptedCredential)) {
        return null;
    }

    try {
        return getDecryptedToken(encryptedCredential, appLockState) || null;
    } catch {
        return null;
    }
}

function requireDecryptedCredential(
    encryptedCredential: string,
    appLockState: ApplicationLockState,
    errorMessage: string
): string {
    const credential = getDecryptedCredentialOrNull(encryptedCredential, appLockState);

    if (!credential) {
        throw new Error(errorMessage);
    }

    return credential;
}

export function discardLegacyPlaintextRefreshCredential(): void {
    const refreshToken = localStorage.getItem(refreshTokenLocalStorageKey);

    if (!refreshToken || getStoredCredentialFormat(refreshToken) === 'encrypted') {
        return;
    }

    localStorage.removeItem(refreshTokenLocalStorageKey);
    sessionStorage.removeItem(encryptedRefreshTokenSessionStorageKey);
    sessionStorage.removeItem(refreshTokenSessionStorageKey);
    logger.warn('[userstate] Removed an unsafe refresh credential from a locked session');
}

export function clearAxiosAuthorizationHeader(): void {
    delete axios.defaults.headers.common['Authorization'];
    delete axios.defaults.headers.common['authorization'];
}

export function restoreUnlockedCredentials(
    encryptedToken: string,
    appLockState: ApplicationLockState
): void {
    const token = requireDecryptedCredential(encryptedToken, appLockState, 'Unable to decrypt token');
    const storedRefreshToken = localStorage.getItem(refreshTokenLocalStorageKey);
    const storedRefreshTokenFormat = storedRefreshToken
        ? getStoredCredentialFormat(storedRefreshToken)
        : null;

    if (storedRefreshTokenFormat === 'invalid') {
        throw new Error('Unable to decrypt refresh credential');
    }

    const refreshToken = storedRefreshTokenFormat === 'encrypted' && storedRefreshToken
        ? requireDecryptedCredential(storedRefreshToken, appLockState, 'Unable to decrypt refresh credential')
        : storedRefreshToken;
    let encryptedRefreshToken = storedRefreshToken;

    if (storedRefreshTokenFormat === 'plaintext' && refreshToken) {
        // Upgrade an older app-lock session whose refresh credential was stored in plaintext.
        // The access credential has already proved that the supplied unlock secret is valid.
        encryptedRefreshToken = getEncryptedToken(refreshToken, appLockState);
    }

    if (storedRefreshTokenFormat === 'plaintext' && encryptedRefreshToken) {
        localStorage.setItem(refreshTokenLocalStorageKey, encryptedRefreshToken);
    }

    sessionStorage.setItem(appLockStateSessionStorageKey, JSON.stringify(appLockState));
    sessionStorage.setItem(encryptedTokenSessionStorageKey, encryptedToken);
    sessionStorage.setItem(tokenSessionStorageKey, token);

    if (!refreshToken || !encryptedRefreshToken) {
        sessionStorage.removeItem(encryptedRefreshTokenSessionStorageKey);
        sessionStorage.removeItem(refreshTokenSessionStorageKey);
    } else {
        sessionStorage.setItem(encryptedRefreshTokenSessionStorageKey, encryptedRefreshToken);
        sessionStorage.setItem(refreshTokenSessionStorageKey, refreshToken);
    }

    axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
}
