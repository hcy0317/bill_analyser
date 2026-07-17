/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

const mockGetServerVersion = jest.fn<(...args: unknown[]) => Promise<any>>();
const mockLoggerWarn = jest.fn();
const mockLoggerError = jest.fn();

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: { getServerVersion: (...args: unknown[]) => mockGetServerVersion(...args) }
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        warn: (...args: unknown[]) => mockLoggerWarn(...args),
        error: (...args: unknown[]) => mockLoggerError(...args)
    }
}));
jest.mock('@/lib/version.ts', () => ({
    getClientVersionInfo: () => ({ version: '1.0.0', commitHash: 'client', buildTime: '' })
}));

const { useSystemsStore } = require('@/stores/system.ts') as typeof import('@/stores/system.ts');

beforeEach(() => {
    setActivePinia(createPinia());
    jest.clearAllMocks();
});

describe('systems store version matching', () => {
    test('rejects malformed successful responses and resolves exact matches', async () => {
        for (const data of [undefined, {}, { success: false }, { success: true }]) {
            mockGetServerVersion.mockResolvedValueOnce({ data });
            await expect(useSystemsStore().checkIfClientVersionMatchServerVersion())
                .rejects.toEqual({ message: 'Unable to retrieve server version' });
        }

        mockGetServerVersion.mockResolvedValueOnce({
            data: { success: true, result: { version: '1.0.0', commitHash: 'client' } }
        });
        await expect(useSystemsStore().checkIfClientVersionMatchServerVersion()).resolves.toEqual({
            match: true,
            version: { version: '1.0.0', commitHash: 'client' }
        });
    });

    test('reports version and commit mismatches', async () => {
        mockGetServerVersion.mockResolvedValueOnce({
            data: { success: true, result: { version: '2.0.0', commitHash: 'client' } }
        });
        await expect(useSystemsStore().checkIfClientVersionMatchServerVersion()).resolves.toEqual({
            match: false,
            version: { version: '2.0.0', commitHash: 'client' }
        });
        expect(mockLoggerWarn).toHaveBeenCalledWith(expect.stringContaining('does not match'));

        mockGetServerVersion.mockResolvedValueOnce({
            data: { success: true, result: { version: '1.0.0', commitHash: 'server' } }
        });
        await expect(useSystemsStore().checkIfClientVersionMatchServerVersion()).resolves.toEqual({
            match: false,
            version: { version: '1.0.0', commitHash: 'server' }
        });
    });

    test('normalizes response, unprocessed, and processed failures', async () => {
        const responseError = { response: { data: { message: 'server failed' } } };
        mockGetServerVersion.mockRejectedValueOnce(responseError);
        await expect(useSystemsStore().checkIfClientVersionMatchServerVersion())
            .rejects.toEqual({ error: responseError.response.data });

        mockGetServerVersion.mockRejectedValueOnce({ processed: false });
        await expect(useSystemsStore().checkIfClientVersionMatchServerVersion())
            .rejects.toEqual({ message: 'Unable to retrieve server version' });

        const processed = { processed: true, message: 'known' };
        mockGetServerVersion.mockRejectedValueOnce(processed);
        await expect(useSystemsStore().checkIfClientVersionMatchServerVersion()).rejects.toBe(processed);
        expect(mockLoggerError).toHaveBeenCalledTimes(3);
    });
});
