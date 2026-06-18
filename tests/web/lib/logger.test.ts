import { afterEach, describe, expect, jest, test } from '@jest/globals';

import logger from '@/lib/logger.ts';
import { isEnableDebug } from '@/lib/settings.ts';

jest.mock('@/lib/settings.ts', () => ({
    isEnableDebug: jest.fn()
}));

const mockedIsEnableDebug = isEnableDebug as jest.MockedFunction<typeof isEnableDebug>;

describe('frontend logger', () => {
    afterEach(() => {
        jest.restoreAllMocks();
        jest.useRealTimers();
        mockedIsEnableDebug.mockReset();
    });

    test('writes timestamped info logs with structured context', () => {
        jest.useFakeTimers().setSystemTime(new Date('2026-06-17T12:34:56.789Z'));
        const info = jest.spyOn(console, 'info').mockImplementation(() => undefined);

        logger.info('import preview received', { previewCount: 64 });

        expect(info).toHaveBeenCalledWith(
            '[bill analyser Info] 2026-06-17T12:34:56.789Z import preview received',
            { previewCount: 64 }
        );
    });

    test('keeps debug gated while preserving the timestamped format when enabled', () => {
        const debug = jest.spyOn(console, 'debug').mockImplementation(() => undefined);

        mockedIsEnableDebug.mockReturnValue(false);
        logger.debug('files selected', { fileCount: 64 });
        expect(debug).not.toHaveBeenCalled();

        jest.useFakeTimers().setSystemTime(new Date('2026-06-17T12:35:00.000Z'));
        mockedIsEnableDebug.mockReturnValue(true);
        logger.debug('files selected', { fileCount: 64 });

        expect(debug).toHaveBeenCalledWith(
            '[bill analyser Debug] 2026-06-17T12:35:00.000Z files selected',
            { fileCount: 64 }
        );
    });
});
