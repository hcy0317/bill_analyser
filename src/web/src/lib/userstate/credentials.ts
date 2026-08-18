import CryptoJS from 'crypto-js';
import axios from 'axios';

import type { ApplicationLockState } from '@/core/setting.ts';

import logger from '../logger.ts';

const appLockSecretBaseStringPrefix: string = 'EBK_LOCK_SECRET_';
const opensslSaltedCredentialPrefix: string = 'U2FsdGVkX1';
const opensslSaltedCredentialHexPrefix: string = '53616c7465645f5f';
const base64CredentialPattern: RegExp = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const authenticatedCredentialVersion: string = 'v1';
const authenticatedCredentialSeparator: string = `.${authenticatedCredentialVersion}.`;
const sha256HexPattern: RegExp = /^[a-f0-9]{64}$/;
const jwtPartPattern: RegExp = /^[A-Za-z0-9_-]+$/;

type StoredCredentialFormat = 'plaintext' | 'encrypted' | 'invalid';
type SessionCredentialKind = 'access' | 'refresh';

interface ParsedEncryptedCredential {
    ciphertext: string;
    integrityTag: string | null;
}

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

function getCredentialEncryptionKey(appLockState: ApplicationLockState): string {
    return CryptoJS.SHA256(`${appLockSecretBaseStringPrefix}|${appLockState.username}|${appLockState.secret}`).toString();
}

function getCredentialIntegrityKey(appLockState: ApplicationLockState): string {
    return CryptoJS.SHA256(`${appLockSecretBaseStringPrefix}|integrity|${appLockState.username}|${appLockState.secret}`).toString();
}

function buildCredentialIntegrityTag(ciphertext: string, appLockState: ApplicationLockState): string {
    return CryptoJS.HmacSHA256(
        `${authenticatedCredentialVersion}.${ciphertext}`,
        getCredentialIntegrityKey(appLockState)
    ).toString();
}

function hasMatchingIntegrityTag(actual: string, expected: string): boolean {
    let difference = 0;
    for (let index = 0; index < actual.length; index += 1) {
        difference |= actual.charCodeAt(index) ^ expected.charCodeAt(index);
    }

    return difference === 0;
}

function parseEncryptedCredential(credential: string): ParsedEncryptedCredential | null {
    const separatorIndex = credential.lastIndexOf(authenticatedCredentialSeparator);

    if (separatorIndex < 0) {
        return { ciphertext: credential, integrityTag: null };
    }

    const ciphertext = credential.slice(0, separatorIndex);
    const integrityTag = credential.slice(separatorIndex + authenticatedCredentialSeparator.length);

    if (!ciphertext || !sha256HexPattern.test(integrityTag)) {
        return null;
    }

    return { ciphertext, integrityTag };
}

function isOpenSslSaltedCiphertext(ciphertext: string): boolean {
    if (
        !ciphertext.startsWith(opensslSaltedCredentialPrefix)
        || ciphertext.length < 44
        || ciphertext.length % 4 !== 0
        || !base64CredentialPattern.test(ciphertext)
    ) {
        return false;
    }

    const payload = CryptoJS.enc.Base64.parse(ciphertext);

    return payload.sigBytes >= 32
        && (payload.sigBytes - 16) % 16 === 0
        && payload.toString(CryptoJS.enc.Hex).startsWith(opensslSaltedCredentialHexPrefix);
}

export function getEncryptedToken(token: string, appLockState: ApplicationLockState): string {
    const ciphertext = CryptoJS.AES.encrypt(token, getCredentialEncryptionKey(appLockState)).toString();
    const integrityTag = buildCredentialIntegrityTag(ciphertext, appLockState);
    return `${ciphertext}${authenticatedCredentialSeparator}${integrityTag}`;
}

export function getDecryptedToken(encryptedToken: string, appLockState: ApplicationLockState): string {
    const parsedCredential = parseEncryptedCredential(encryptedToken);

    if (!parsedCredential || !isOpenSslSaltedCiphertext(parsedCredential.ciphertext)) {
        throw new Error('Invalid encrypted credential');
    }

    if (parsedCredential.integrityTag) {
        const expectedIntegrityTag = buildCredentialIntegrityTag(parsedCredential.ciphertext, appLockState);
        if (!hasMatchingIntegrityTag(parsedCredential.integrityTag, expectedIntegrityTag)) {
            throw new Error('Invalid encrypted credential integrity');
        }
    }

    const bytes = CryptoJS.AES.decrypt(
        parsedCredential.ciphertext,
        getCredentialEncryptionKey(appLockState)
    );
    return bytes.toString(CryptoJS.enc.Utf8);
}

function isEncryptedCredential(credential: string): boolean {
    const parsedCredential = parseEncryptedCredential(credential);
    return parsedCredential !== null && isOpenSslSaltedCiphertext(parsedCredential.ciphertext);
}

function isAuthenticatedCredential(credential: string): boolean {
    const parsedCredential = parseEncryptedCredential(credential);
    return parsedCredential !== null && parsedCredential.integrityTag !== null;
}

function decodeJwtPart(part: string): unknown {
    const base64 = part.replace(/-/g, '+').replace(/_/g, '/');
    const paddedBase64 = base64.padEnd(Math.ceil(base64.length / 4) * 4, '=');

    try {
        return JSON.parse(CryptoJS.enc.Base64.parse(paddedBase64).toString(CryptoJS.enc.Utf8));
    } catch {
        return null;
    }
}

function isLegacySessionCredential(credential: string, kind: SessionCredentialKind): boolean {
    const parts = credential.split('.');
    const [headerPart, payloadPart, signaturePart] = parts;
    if (
        parts.length !== 3
        || !headerPart
        || !payloadPart
        || !signaturePart
        || !parts.every(part => jwtPartPattern.test(part))
    ) {
        return false;
    }

    const header = decodeJwtPart(headerPart);
    const payload = decodeJwtPart(payloadPart);
    if (
        !header
        || typeof header !== 'object'
        || !payload
        || typeof payload !== 'object'
    ) {
        return false;
    }

    const headerRecord = header as Record<string, unknown>;
    const payloadRecord = payload as Record<string, unknown>;
    return headerRecord['typ'] === 'JWT'
        && typeof headerRecord['alg'] === 'string'
        && payloadRecord['type'] === kind
        && typeof payloadRecord['exp'] === 'number';
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
    const tokenIsAuthenticated = isAuthenticatedCredential(encryptedToken);
    if (!tokenIsAuthenticated && !isLegacySessionCredential(token, 'access')) {
        throw new Error('Unable to decrypt token');
    }

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
    const refreshTokenIsAuthenticated = storedRefreshTokenFormat === 'encrypted' && storedRefreshToken
        ? isAuthenticatedCredential(storedRefreshToken)
        : false;

    if (
        storedRefreshTokenFormat === 'encrypted'
        && refreshToken
        && !refreshTokenIsAuthenticated
        && !isLegacySessionCredential(refreshToken, 'refresh')
    ) {
        throw new Error('Unable to decrypt refresh credential');
    }

    const upgradedEncryptedToken = tokenIsAuthenticated
        ? encryptedToken
        : getEncryptedToken(token, appLockState);
    let encryptedRefreshToken = storedRefreshToken;

    if (
        refreshToken
        && (storedRefreshTokenFormat === 'plaintext' || !refreshTokenIsAuthenticated)
    ) {
        // Upgrade plaintext and unauthenticated legacy refresh credentials only after every
        // credential has passed validation, so a failed unlock cannot partially mutate storage.
        encryptedRefreshToken = getEncryptedToken(refreshToken, appLockState);
    }

    if (upgradedEncryptedToken !== encryptedToken) {
        localStorage.setItem(tokenLocalStorageKey, upgradedEncryptedToken);
    }

    if (encryptedRefreshToken && encryptedRefreshToken !== storedRefreshToken) {
        localStorage.setItem(refreshTokenLocalStorageKey, encryptedRefreshToken);
    }

    sessionStorage.setItem(appLockStateSessionStorageKey, JSON.stringify(appLockState));
    sessionStorage.setItem(encryptedTokenSessionStorageKey, upgradedEncryptedToken);
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
