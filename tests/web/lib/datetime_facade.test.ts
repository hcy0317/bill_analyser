import { afterAll, beforeAll, describe, expect, jest, test } from '@jest/globals';
import moment from 'moment-timezone';

import type { DateTimeFormatOptions } from '@/core/datetime.ts';
import {
    formatUnixTime,
    getCurrentFiscalYear,
    getFiscalYearFromUnixTime,
    getHourIn12HourFormat,
    getUtcOffsetByUtcOffsetMinutes
} from '@/lib/datetime.ts';

beforeAll(() => {
    jest.useFakeTimers();
    jest.setSystemTime(new Date('2026-05-05T00:00:00Z'));
    moment.tz.setDefault('UTC');
});

afterAll(() => {
    jest.useRealTimers();
    moment.tz.setDefault();
});

describe('datetime facade helpers', () => {
    test('keeps formatting helpers available from the datetime facade', () => {
        expect(formatUnixTime(0, 'YYYY-MM-DD HH:mm:ss Z', {} as DateTimeFormatOptions, 0, 0)).toBe('1970-01-01 00:00:00 +00:00');
    });

    test('keeps small time helpers available from the datetime facade', () => {
        expect(getHourIn12HourFormat(0)).toBe(12);
        expect(getHourIn12HourFormat(13)).toBe(1);
        expect(getUtcOffsetByUtcOffsetMinutes(330)).toBe('+05:30');
        expect(getUtcOffsetByUtcOffsetMinutes(-480)).toBe('-08:00');
    });

    test('keeps fiscal year helpers available from the datetime facade', () => {
        const aprilFirstFiscalYearStart = 0x0401;
        const fixedCurrentUnixTime = moment('2026-05-05T00:00:00Z').unix();

        expect(getCurrentFiscalYear(aprilFirstFiscalYearStart)).toBe(getFiscalYearFromUnixTime(fixedCurrentUnixTime, aprilFirstFiscalYearStart));
    });
});
