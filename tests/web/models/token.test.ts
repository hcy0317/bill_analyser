import { describe, expect, test } from '@jest/globals';

import {
    SessionDeviceType,
    SessionInfo,
    TOKEN_TYPE_API,
    TOKEN_TYPE_MCP
} from '@/models/token.ts';

describe('token model helpers', () => {
    test('token type constants remain stable', () => {
        expect(TOKEN_TYPE_API).toBe(8);
        expect(TOKEN_TYPE_MCP).toBe(5);
    });

    test('SessionInfo.of preserves token session metadata', () => {
        const session = SessionInfo.of(
            'token-1',
            true,
            SessionDeviceType.Tablet,
            'iPadOS 18',
            'iPad Pro',
            1711785600000
        );

        expect(session.tokenId).toBe('token-1');
        expect(session.isCurrent).toBe(true);
        expect(session.deviceType).toBe(SessionDeviceType.Tablet);
        expect(session.deviceInfo).toBe('iPadOS 18');
        expect(session.deviceName).toBe('iPad Pro');
        expect(session.lastSeen).toBe(1711785600000);
    });
});
