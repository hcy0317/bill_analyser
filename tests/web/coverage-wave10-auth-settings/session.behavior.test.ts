import { jest } from '@jest/globals';

const mockUaParser = jest.fn();

jest.mock('ua-parser-js', () => ({
    __esModule: true,
    default: (...args: unknown[]) => mockUaParser(...args)
}));

import { parseSessionInfo } from '@/lib/session.ts';
import {
    SessionDeviceType,
    TOKEN_TYPE_API,
    TOKEN_TYPE_MCP,
    type TokenInfoResponse
} from '@/models/token.ts';

function token(overrides: Partial<TokenInfoResponse> = {}): TokenInfoResponse {
    return {
        tokenId: 'token-1',
        tokenType: 1,
        userAgent: 'synthetic-agent',
        lastSeen: 123456,
        isCurrent: false,
        ...overrides
    };
}

function parsed(overrides: Record<string, unknown> = {}) {
    return {
        device: {},
        os: {},
        browser: {},
        ...overrides
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockUaParser.mockReturnValue(parsed());
});

describe('parseSessionInfo', () => {
    test('identifies API and MCP tokens without treating their labels as browser agents', () => {
        expect(parseSessionInfo(token({
            tokenId: 'api-id',
            tokenType: TOKEN_TYPE_API,
            userAgent: 'curl/8',
            isCurrent: true
        }))).toEqual(expect.objectContaining({
            tokenId: 'api-id',
            isCurrent: true,
            deviceType: SessionDeviceType.Api,
            deviceInfo: 'curl/8',
            deviceName: 'API Token'
        }));

        expect(parseSessionInfo(token({
            tokenId: 'mcp-id',
            tokenType: TOKEN_TYPE_MCP,
            userAgent: 'Codex MCP'
        }))).toEqual(expect.objectContaining({
            deviceType: SessionDeviceType.MCP,
            deviceInfo: 'Codex MCP',
            deviceName: 'MCP Token'
        }));
    });

    test.each([
        ['mobile', SessionDeviceType.Phone],
        ['wearable', SessionDeviceType.Wearable],
        ['tablet', SessionDeviceType.Tablet],
        ['smarttv', SessionDeviceType.TV],
        ['console', SessionDeviceType.Default]
    ])('maps %s agents to the corresponding device type', (type, expected) => {
        mockUaParser.mockReturnValue(parsed({
            device: { type, model: 'Synthetic Device' },
            browser: { name: 'Browser', version: '1.0' }
        }));

        const result = parseSessionInfo(token());

        expect(result.deviceType).toBe(expected);
        expect(result.deviceInfo).toBe('Synthetic Device (Browser 1.0)');
        expect(result.deviceName).toBe('Other Device');
    });

    test('uses operating-system and browser details when no model is reported', () => {
        mockUaParser.mockReturnValue(parsed({
            os: { name: 'SyntheticOS', version: '42' },
            browser: { name: 'SyntheticBrowser', version: '9' }
        }));

        const result = parseSessionInfo(token({ isCurrent: true }));

        expect(result.deviceType).toBe(SessionDeviceType.Default);
        expect(result.deviceInfo).toBe('SyntheticOS 42 (SyntheticBrowser 9)');
        expect(result.deviceName).toBe('Current');
    });

    test('handles partial and absent user-agent details', () => {
        mockUaParser
            .mockReturnValueOnce(parsed({ os: { name: 'SyntheticOS' } }))
            .mockReturnValueOnce(parsed({ browser: { name: 'BrowserOnly' } }))
            .mockReturnValueOnce(parsed());

        expect(parseSessionInfo(token()).deviceInfo).toBe('SyntheticOS');
        expect(parseSessionInfo(token()).deviceInfo).toBe('BrowserOnly');
        expect(parseSessionInfo(token()).deviceInfo).toBe('Unknown Device');
    });
});
