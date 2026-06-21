import CBOR from 'cbor-js';

import type { ApplicationLockState } from '@/core/setting.ts';
import type { UserBasicInfo } from '@/models/user.ts';

import {
    isFunction,
    stringToArrayBuffer,
    arrayBufferToString,
    base64encode,
    base64decode
} from './common.ts';
import {
    generateRandomString
} from './misc.ts';
import logger from './logger.ts';

interface ClientData {
    readonly challenge: string;
    readonly crossOrigin: boolean;
    readonly origin: string;
    readonly type: string;
}

interface AttestationData {
    readonly authData: Uint8Array;
    readonly fmt: string;
}

interface WebAuthnRegisterResponse {
    readonly id: string;
    readonly clientData: ClientData;
    readonly publicKey: Uint8Array | null;
    readonly rawCredential: Credential;
}

interface WebAuthnVerifyResponse {
    readonly id: string;
    readonly userName: string;
    readonly userSecret: string;
    readonly clientData: ClientData;
    readonly rawCredential: Credential;
}

const PUBLIC_KEY_CREDENTIAL_CREATION_OPTIONS_BASE_TEMPLATE = {
    attestation: "none",
    authenticatorSelection: {
        authenticatorAttachment: 'platform',
        requireResidentKey: false,
        userVerification: "discouraged"
    },
    pubKeyCredParams: [
        // https://www.iana.org/assignments/cose/cose.xhtml#algorithms
        {type: "public-key", alg: -7},   // ECDSA w/ SHA-256
        {type: "public-key", alg: -257}, // RSASSA-PKCS1-v1_5 using SHA-256
    ],
    timeout: 120000
};

const PUBLIC_KEY_CREDENTIAL_REQUEST_OPTIONS_BASE_TEMPLATE = {
    allowCredentials: [{
        type: 'public-key'
    }],
    userVerification: "discouraged",
    timeout: 120000
};

function parseClientData(credential: Credential): ClientData | null {
    const utf8Decoder = new TextDecoder('utf-8');
    const decodedClientData = utf8Decoder.decode(credential.response.clientDataJSON);
    return JSON.parse(decodedClientData) as ClientData;
}

function parsePublicKeyFromAttestationData(credential: Credential): Uint8Array {
    const decodedAttestationData = CBOR.decode(credential.response.attestationObject) as AttestationData;
    const authData = decodedAttestationData.authData;

    const dataView = new DataView(new ArrayBuffer(2));
    const idLenBytes = authData.slice(53, 55);
    idLenBytes.forEach((value, index) => dataView.setUint8(index, value));

    const credentialIdLength = dataView.getUint16(0);
    const publicKeyBytes = authData.slice(55 + credentialIdLength);

    return publicKeyBytes;
}

/** 中文说明：同步判断当前浏览器是否暴露 WebAuthn 基础 API。 */
export function isWebAuthnSupported(): boolean {
    return !!window.PublicKeyCredential
        && !!navigator.credentials
        && isFunction(window.PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable);
}

/** 中文说明：异步确认浏览器是否完整支持 WebAuthn 平台认证器能力。 */
export function isWebAuthnCompletelySupported(): Promise<boolean> {
    if (!isWebAuthnSupported()) {
        return Promise.resolve(false);
    }

    return window.PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
}

/** 中文说明：为当前用户创建 WebAuthn credential，并把 token 解锁材料封装进注册响应。 */
export function registerWebAuthnCredential(lockState: ApplicationLockState, userInfo: UserBasicInfo): Promise<WebAuthnRegisterResponse> {
    if (!window.location || !window.location.hostname) {
        return Promise.reject({
            notSupported: true
        });
    }

    if (!isWebAuthnSupported() || !navigator.credentials.create) {
        return Promise.reject({
            notSupported: true
        });
    }

    const challenge = generateRandomString();
    const userId = `${lockState.username}|${lockState.secret}`; // username 32bytes(max) + secret 24bytes = 56bytes(max)

    const publicKeyCredentialCreationOptions: PublicKeyCredentialCreationOptions = Object.assign({}, PUBLIC_KEY_CREDENTIAL_CREATION_OPTIONS_BASE_TEMPLATE, {
        challenge: stringToArrayBuffer(challenge),
        rp: {
            name: window.location.hostname,
            id: window.location.hostname
        },
        user: {
            id: stringToArrayBuffer(userId),
            name: lockState.username,
            displayName: userInfo.nickname
        }
    }) as PublicKeyCredentialCreationOptions;

    logger.debug('webauthn create options', publicKeyCredentialCreationOptions);

    return navigator.credentials.create({
        publicKey: publicKeyCredentialCreationOptions
    }).then(rawCredential => {
        const clientData = rawCredential ? parseClientData(rawCredential) : null;
        const publicKey = rawCredential ? parsePublicKeyFromAttestationData(rawCredential) : null;

        const challengeFromClientData = clientData && clientData.challenge ? base64decode(clientData.challenge) : null;

        logger.debug('webauthn create raw response', rawCredential);

        if (rawCredential && rawCredential.rawId &&
            clientData && clientData.type === 'webauthn.create' && challengeFromClientData === challenge) {
            const ret: WebAuthnRegisterResponse = {
                id: base64encode(rawCredential.rawId),
                clientData: clientData,
                publicKey: publicKey,
                rawCredential: rawCredential
            };

            logger.debug('webauthn create response', ret);

            return ret;
        } else {
            return Promise.reject({
                invalid: true
            });
        }
    });
}

/** 中文说明：使用已保存的 credentialId 触发 WebAuthn 验证，成功后返回解锁所需的断言信息。 */
export function verifyWebAuthnCredential(userInfo: UserBasicInfo, credentialId: string): Promise<WebAuthnVerifyResponse> {
    if (!window.location || !window.location.hostname) {
        return Promise.reject({
            notSupported: true
        });
    }

    if (!isWebAuthnSupported() || !navigator.credentials.get) {
        return Promise.reject({
            notSupported: true
        });
    }

    const challenge = generateRandomString();
    const publicKeyCredentialRequestOptions: PublicKeyCredentialRequestOptions = Object.assign({}, PUBLIC_KEY_CREDENTIAL_REQUEST_OPTIONS_BASE_TEMPLATE, {
        challenge: stringToArrayBuffer(challenge),
        rpId: window.location.hostname
    }) as PublicKeyCredentialRequestOptions;

    if (publicKeyCredentialRequestOptions.allowCredentials && publicKeyCredentialRequestOptions.allowCredentials.length > 0) {
        publicKeyCredentialRequestOptions.allowCredentials[0]!.id = stringToArrayBuffer(base64decode(credentialId));
    }

    logger.debug('webauthn get options', publicKeyCredentialRequestOptions);

    return navigator.credentials.get({
        publicKey: publicKeyCredentialRequestOptions
    }).then(rawCredential => {
        const clientData = rawCredential ? parseClientData(rawCredential) : null;
        const challengeFromClientData = clientData && clientData.challenge ? base64decode(clientData.challenge) : null;
        const userIdParts = rawCredential && rawCredential.response && rawCredential.response.userHandle ? arrayBufferToString(rawCredential.response.userHandle).split('|') : null;

        logger.debug('webauthn get raw response', rawCredential);

        if (rawCredential && rawCredential.rawId &&
            clientData && clientData.type === 'webauthn.get' && challengeFromClientData === challenge &&
            userIdParts && userIdParts.length === 2 && userIdParts[0] === userInfo.username) {
            const ret: WebAuthnVerifyResponse = {
                id: base64encode(rawCredential.rawId),
                userName: userIdParts[0] as string,
                userSecret: userIdParts[1] as string,
                clientData: clientData,
                rawCredential: rawCredential
            };

            logger.debug('webauthn get response', ret);

            return ret;
        } else {
            return Promise.reject({
                invalid: true
            });
        }
    });
}
