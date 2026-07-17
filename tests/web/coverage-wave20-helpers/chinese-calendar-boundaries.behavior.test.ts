import { describe, expect, test } from '@jest/globals';

import type { ChineseCalendarLocaleData } from '@/core/calendar.ts';
import { DEFAULT_CONTENT } from '@/locales/calendar/chinese/index.ts';
import {
    getChineseCalendarAlternateDisplayDate,
    getChineseYearMonthAllDayInfos,
    getChineseYearMonthDayInfo
} from '@/lib/calendar/chinese_calendar.ts';

function locale(overrides: Partial<ChineseCalendarLocaleData> = {}): ChineseCalendarLocaleData {
    return {
        ...DEFAULT_CONTENT,
        ...overrides
    };
}

describe('Chinese calendar public boundary behavior', () => {
    test.each([
        { year: 1899, month: 1, day: 1 },
        { year: 2101, month: 1, day: 1 },
        { year: 2026, month: 0, day: 1 },
        { year: 2026, month: 13, day: 1 }
    ])('rejects unsupported Gregorian date $year-$month-$day', date => {
        expect(getChineseYearMonthDayInfo(date, DEFAULT_CONTENT)).toBeUndefined();
    });

    test('rejects the supported minimum-year days that predate the first Chinese year', () => {
        expect(getChineseYearMonthDayInfo({ year: 1999, month: 1, day: 1 }, DEFAULT_CONTENT))
            .toBeUndefined();
        expect(getChineseYearMonthAllDayInfos({ year: 1999, month1base: 1 }, DEFAULT_CONTENT))
            .toBeUndefined();
    });

    test('allows locale data without solar-term names and empty day-name fallbacks', () => {
        const withoutSolarTerms = locale({
            solarTermNames: undefined as unknown as string[],
            dayNames: []
        });
        const info = getChineseYearMonthDayInfo({ year: 2026, month: 7, day: 1 }, withoutSolarTerms);

        expect(info).toMatchObject({
            gregorianYear: 2026,
            gregorianMonth: 7,
            gregorianDay: 1,
            displayDay: '',
            solarTermName: ''
        });
    });

    test('returns an empty solar-term label when both indexed translations are missing', () => {
        const july = getChineseYearMonthAllDayInfos({ year: 2026, month1base: 7 }, DEFAULT_CONTENT)!;
        const solarTermDates = july.filter(info => Boolean(info.solarTermName));

        expect(solarTermDates).toHaveLength(2);
        const emptySolarTerms = locale({ solarTermNames: [] });
        expect(solarTermDates.map(info => getChineseYearMonthDayInfo({
            year: info.gregorianYear,
            month: info.gregorianMonth,
            day: info.gregorianDay
        }, emptySolarTerms)?.solarTermName)).toEqual(['', '']);
    });

    test('applies alternate-display priority from day to month to solar term', () => {
        const base = {
            gregorianYear: 2026,
            gregorianMonth: 1,
            gregorianDay: 1,
            year: 2025,
            month: 12 as const,
            day: 2 as const,
            displayYear: '二〇二五',
            displayMonth: '十二月',
            displayDay: '初二',
            isLeapMonth: false,
            solarTermName: ''
        };

        expect(getChineseCalendarAlternateDisplayDate(base).displayDate).toBe('初二');
        expect(getChineseCalendarAlternateDisplayDate({ ...base, day: 1 }).displayDate).toBe('十二月');
        expect(getChineseCalendarAlternateDisplayDate({
            ...base,
            day: 1,
            solarTermName: '冬至'
        }).displayDate).toBe('冬至');
    });

    test('does not fabricate a complete month beyond the supported Chinese-year tail', () => {
        const december = getChineseYearMonthAllDayInfos({ year: 2100, month1base: 12 }, DEFAULT_CONTENT);

        expect(december === undefined || december.length === 31).toBe(true);
        if (december) {
            expect(december[0]?.gregorianYear).toBe(2100);
            expect(december.at(-1)?.gregorianMonth).toBe(12);
        }
    });
});
