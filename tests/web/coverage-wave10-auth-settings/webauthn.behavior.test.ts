import { jest } from '@jest/globals';

const mockCborDecode = jest.fn();
const mockGenerateRandomString = jest.fn(() => 'challenge-123');
const mockLoggerDebug = jest.fn();

function toArrayBuffer(value: string): ArrayBuffer {
    const bytes = new TextEncoder().encode(value);
    return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

jest.mock('cbor-js', () => ({
    __esModule: true,
    default: { decode: (...args: unknown[]) => mockCborDecode(...args) }
}));
jest.mock('@/lib/misc.ts', () => ({
    generateRandomString: () => mockGenerateRandomString()
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: (...args: unknown[]) => mockLoggerDebug(...args) }
}));
jest.mock('@/lib/common.ts', () => ({
    isFunction: (value: unknown) => typeof value === 'function',
    stringToArrayBuffer: (value: string) => toArrayBuffer(value),
    arrayBufferToString: (value: ArrayBuffer) => new TextDecoder().decode(value),
    base64encode: (value: ArrayBuffer) => `encoded:${Array.from(new Uint8Array(value)).join(',')}`,
    base64decode: (value: string) => value === 'challenge-wire' ? 'challenge-123' : 'credential-id'
}));

import {
    isWebAuthnCompletelySupported,
    isWebAuthnSupported,
    registerWebAuthnCredential,
    verifyWebAuthnCredential
} from '@/lib/webauthn.ts';

const mockCreate = jest.fn<(...args: any[]) => Promise<any>>();
const mockGet = jest.fn<(...args: any[]) => Promise<any>>();
const mockPlatformAvailable = jest.fn<(...args: any[]) => Promise<boolean>>();

function clientData(type: string, challenge = 'challenge-wire'): ArrayBuffer {
    return toArrayBuffer(JSON.stringify({
        challenge,
        crossOrigin: false,
        origin: 'http://localhost',
        type
    }));
}

function installWebAuthn(options: { create?: boolean; get?: boolean } = { create: true, get: true }): void {
    (globalThis as any).window = {
        location: { hostname: 'localhost' },
        PublicKeyCredential: { isUserVerifyingPlatformAuthenticatorAvailable: mockPlatformAvailable }
    };
    Object.defineProperty(globalThis, 'navigator', {
        configurable: true,
        value: {
            credentials: {
                create: options.create ? mockCreate : undefined,
                get: options.get ? mockGet : undefined
            }
        }
    });
}

beforeEach(() => {
    jest.clearAllMocks();
    mockGenerateRandomString.mockReturnValue('challenge-123');
    mockPlatformAvailable.mockResolvedValue(true);
    installWebAuthn();
});

describe('WebAuthn capability detection', () => {
    test('requires the platform credential constructor, credentials API, and availability function', async () => {
        expect(isWebAuthnSupported()).toBe(true);
        await expect(isWebAuthnCompletelySupported()).resolves.toBe(true);

        Object.defineProperty(window, 'PublicKeyCredential', { configurable: true, value: undefined });
        expect(isWebAuthnSupported()).toBe(false);
        await expect(isWebAuthnCompletelySupported()).resolves.toBe(false);

        Object.defineProperty(window, 'PublicKeyCredential', { configurable: true, value: {} });
        expect(isWebAuthnSupported()).toBe(false);
    });
});

describe('registerWebAuthnCredential', () => {
    test('rejects when credential creation is unavailable', async () => {
        installWebAuthn({ create: false, get: true });

        await expect(registerWebAuthnCredential(
            { username: 'alice', secret: 'lock-secret' } as any,
            { nickname: 'Alice' } as any
        )).rejects.toEqual({ notSupported: true });
    });

    test('builds platform creation options and returns validated credential material', async () => {
        const authData = new Uint8Array(61);
        authData[53] = 0;
        authData[54] = 2;
        authData.set([7, 8], 55);
        authData.set([21, 22, 23, 24], 57);
        mockCborDecode.mockReturnValue({ authData, fmt: 'none' });
        const rawCredential = {
            rawId: new Uint8Array([1, 2, 3]).buffer,
            response: {
                clientDataJSON: clientData('webauthn.create'),
                attestationObject: new Uint8Array([9]).buffer
            }
        };
        mockCreate.mockResolvedValue(rawCredential);

        const result = await registerWebAuthnCredential(
            { username: 'alice', secret: 'lock-secret' } as any,
            { nickname: 'Alice' } as any
        );

        const request = mockCreate.mock.calls[0]?.[0] as any;
        expect(new TextDecoder().decode(request.publicKey.challenge)).toBe('challenge-123');
        expect(request.publicKey.rp).toEqual({ name: 'localhost', id: 'localhost' });
        expect(new TextDecoder().decode(request.publicKey.user.id)).toBe('alice|lock-secret');
        expect(result.id).toBe('encoded:1,2,3');
        expect(Array.from(result.publicKey ?? [])).toEqual([21, 22, 23, 24]);
        expect(result.rawCredential).toBe(rawCredential);
    });

    test('rejects empty and challenge-mismatched creation responses', async () => {
        mockCreate
            .mockResolvedValueOnce(null)
            .mockResolvedValueOnce({
                rawId: new Uint8Array([1]).buffer,
                response: {
                    clientDataJSON: clientData('webauthn.get'),
                    attestationObject: new Uint8Array([9]).buffer
                }
            });
        mockCborDecode.mockReturnValue({ authData: new Uint8Array(60), fmt: 'none' });

        await expect(registerWebAuthnCredential({ username: 'alice', secret: 'secret' } as any, { nickname: 'Alice' } as any))
            .rejects.toEqual({ invalid: true });
        await expect(registerWebAuthnCredential({ username: 'alice', secret: 'secret' } as any, { nickname: 'Alice' } as any))
            .rejects.toEqual({ invalid: true });
    });
});

describe('verifyWebAuthnCredential', () => {
    test('rejects when credential lookup is unavailable', async () => {
        installWebAuthn({ create: true, get: false });

        await expect(verifyWebAuthnCredential({ username: 'alice' } as any, 'credential-b64'))
            .rejects.toEqual({ notSupported: true });
    });

    test('requests the saved credential and returns matching unlock material', async () => {
        const rawCredential = {
            rawId: new Uint8Array([4, 5]).buffer,
            response: {
                clientDataJSON: clientData('webauthn.get'),
                userHandle: toArrayBuffer('alice|user-secret')
            }
        };
        mockGet.mockResolvedValue(rawCredential);

        const result = await verifyWebAuthnCredential({ username: 'alice' } as any, 'credential-b64');

        const request = mockGet.mock.calls[0]?.[0] as any;
        expect(new TextDecoder().decode(request.publicKey.challenge)).toBe('challenge-123');
        expect(new TextDecoder().decode(request.publicKey.allowCredentials[0].id)).toBe('credential-id');
        expect(result).toEqual(expect.objectContaining({
            id: 'encoded:4,5',
            userName: 'alice',
            userSecret: 'user-secret',
            rawCredential
        }));
    });

    test.each([
        [null],
        [{ rawId: new Uint8Array([1]).buffer, response: { clientDataJSON: clientData('webauthn.create'), userHandle: toArrayBuffer('alice|secret') } }],
        [{ rawId: new Uint8Array([1]).buffer, response: { clientDataJSON: clientData('webauthn.get'), userHandle: toArrayBuffer('bob|secret') } }],
        [{ rawId: new Uint8Array([1]).buffer, response: { clientDataJSON: clientData('webauthn.get'), userHandle: toArrayBuffer('alice|secret|extra') } }]
    ])('rejects invalid assertion response %#', async (response) => {
        mockGet.mockResolvedValue(response);
        await expect(verifyWebAuthnCredential({ username: 'alice' } as any, 'credential-b64'))
            .rejects.toEqual({ invalid: true });
    });
});
