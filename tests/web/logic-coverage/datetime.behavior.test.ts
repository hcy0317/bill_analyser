import { afterAll, beforeAll, describe, expect, jest, test } from '@jest/globals';
import moment from 'moment-timezone';

import {
    DateRange,
    DateRangeScene,
    KnownDateTimeFormat,
    LongDateFormat,
    MeridiemIndicator,
    ShortDateFormat,
    WeekDay,
    type DateTimeFormatOptions,
    type LocalizedRecentMonthDateRange,
    type TextualMonthDay,
    type TextualYearMonth,
    type TextualYearMonthDay
} from '@/core/datetime.ts';
import { FiscalYearStart } from '@/core/fiscalyear.ts';
import { NumeralSystem } from '@/core/numeral.ts';
import * as datetime from '@/lib/datetime.ts';

const OPTIONS = {} as DateTimeFormatOptions;
const FISCAL_APRIL = 0x0401;

function unix(value: string): number {
    return moment(value).unix();
}

beforeAll(() => {
    jest.useFakeTimers();
    jest.setSystemTime(new Date('2026-07-15T12:34:56Z'));
    moment.tz.setDefault('UTC');
});

afterAll(() => {
    moment.tz.setDefault();
    jest.useRealTimers();
});

describe('datetime scalar conversion and parsing', () => {
    test('reports the planning year range and validates zero-based months', () => {
        expect(datetime.getAllowedYearRange()).toStrictEqual([2000, 2031]);
        expect(datetime.isYear0BasedMonthValid(2026, 0)).toBe(true);
        expect(datetime.isYear0BasedMonthValid(2026, 11)).toBe(true);
        expect(datetime.isYear0BasedMonthValid(0, 1)).toBe(false);
        expect(datetime.isYear0BasedMonthValid(2026, -1)).toBe(false);
        expect(datetime.isYear0BasedMonthValid(2026, 12)).toBe(false);
        expect(datetime.isYear0BasedMonthValid('2026' as unknown as number, 1)).toBe(false);
        expect(datetime.isYear0BasedMonthValid(2026, null as unknown as number)).toBe(false);
    });

    test('converts year-month values among unix, strings and both object bases', () => {
        const timestamp = unix('2026-07-15T00:00:00Z');
        expect(datetime.getYear0BasedMonthObjectFromUnixTime(timestamp)).toStrictEqual({ year: 2026, month0base: 6 });
        expect(datetime.getYear0BasedMonthObjectFromString('2026-07')).toStrictEqual({ year: 2026, month0base: 6 });
        expect(datetime.getYear0BasedMonthObjectFromString('' as TextualYearMonth)).toBeNull();
        expect(datetime.getYear0BasedMonthObjectFromString('2026' as TextualYearMonth)).toBeNull();
        expect(datetime.getYear0BasedMonthObjectFromString('2026-13')).toBeNull();
        expect(datetime.getYear0BasedMonthObjectFromString(null as unknown as TextualYearMonth)).toBeNull();
        expect(datetime.getYearMonthStringFromYear0BasedMonthObject({ year: 2026, month0base: 6 })).toBe('2026-7');
        expect(datetime.getYearMonthStringFromYear0BasedMonthObject(null)).toBe('');
        expect(datetime.getYearMonthStringFromYear0BasedMonthObject({ year: 2026, month0base: 12 })).toBe('');

        const first = datetime.getYearMonthFirstUnixTime('2026-07');
        expect(first).toBe(unix('2026-07-01T00:00:00Z'));
        expect(datetime.getYearMonthFirstUnixTime({ year: 2026, month0base: 6 })).toBe(first);
        expect(datetime.getYearMonthFirstUnixTime({ year: 2026, month1base: 7 })).toBe(first);
        expect(datetime.getYearMonthFirstUnixTime('')).toBe(0);
        expect(datetime.getYearMonthFirstUnixTime({ year: 2026, month0base: 20 })).toBe(0);
        expect(datetime.getYearMonthFirstUnixTime({ year: 2026, month1base: 20 })).toBe(0);
        expect(datetime.getYearMonthLastUnixTime('2026-07')).toBe(unix('2026-07-31T23:59:59Z'));
    });

    test('handles meridiem, equality, timezones and local Date conversions', () => {
        const noon = unix('2026-07-15T12:00:00Z');
        expect(datetime.isPM(11)).toBe(false);
        expect(datetime.isPM(12)).toBe(true);
        expect(datetime.getAMOrPM(0)).toBe(MeridiemIndicator.AM.name);
        expect(datetime.getAMOrPM(23)).toBe(MeridiemIndicator.PM.name);
        expect(datetime.isUnixTimeYearMonthDayEquals(noon, noon + 11 * 3600)).toBe(true);
        expect(datetime.isUnixTimeYearMonthDayEquals(noon, noon + 24 * 3600)).toBe(false);
        expect(datetime.isUnixTimeYearMonthDayHourEquals(noon, noon + 30 * 60)).toBe(true);
        expect(datetime.isUnixTimeYearMonthDayHourEquals(noon, noon + 3600)).toBe(false);
        expect(datetime.getTimezoneOffsetMinutes('Asia/Shanghai')).toBe(480);
        expect(datetime.getTimezoneOffset('Asia/Shanghai')).toBe('+08:00');
        expect(datetime.getTimezoneOffsetMinutes()).toBe(0);
        expect(datetime.getTimezoneOffset()).toBe('+00:00');
        expect(datetime.getBrowserTimezoneOffsetMinutes()).toBe(-new Date().getTimezoneOffset());
        expect(datetime.getBrowserTimezoneOffset()).toMatch(/^[+-]\d{2}:\d{2}$/);
        expect(datetime.getUnixTimeFromLocalDatetime(datetime.getLocalDatetimeFromUnixTime(noon))).toBe(noon);
        expect(datetime.getActualUnixTimeForStore(1_000, 480, 0)).toBe(1_000 - 480 * 60);
        expect(datetime.getDummyUnixTimeForLocalUsage(1_000, 480, 0)).toBe(1_000 + 480 * 60);
    });

    test('creates, parses and formats DateTime values with optional offset adjustment', () => {
        expect(datetime.getCurrentUnixTime()).toBe(unix('2026-07-15T12:34:56Z'));
        expect(datetime.getCurrentDateTime().getUnixTime()).toBe(datetime.getCurrentUnixTime());
        expect(datetime.getYearMonthDayDateTime(2024, 2, 29).getGregorianCalendarYearDashMonthDashDay()).toBe('2024-02-29');

        const base = unix('2026-07-15T00:00:00Z');
        expect(datetime.parseDateTimeFromUnixTime(base).getUnixTime()).toBe(base);
        expect(datetime.parseDateTimeFromUnixTime(base, 480, 0).getUnixTime()).toBe(base + 480 * 60);
        expect(datetime.parseDateTimeFromUnixTime(base, 480).getUnixTime()).toBe(base + 480 * 60);
        expect(datetime.parseDateTimeFromKnownDateTimeFormat('2026-07-15', KnownDateTimeFormat.DefaultDate)?.getGregorianCalendarDay()).toBe(15);
        expect(datetime.parseDateTimeFromKnownDateTimeFormat('invalid', KnownDateTimeFormat.DefaultDate)).toBeUndefined();
        expect(datetime.parseDateTimeFromString('2026/07/15', 'YYYY/MM/DD')?.getGregorianCalendarMonth()).toBe(7);
        expect(datetime.parseDateTimeFromString('invalid', 'YYYY/MM/DD')).toBeUndefined();

        expect(datetime.formatUnixTime(base, 'YYYY-MM-DD HH:mm:ss Z', OPTIONS, 0, 0)).toBe('2026-07-15 00:00:00 +00:00');
        expect(datetime.formatCurrentTime('YYYY-MM-DD', OPTIONS)).toBe('2026-07-15');
        expect(datetime.formatGregorianCalendarYearDashMonthDashDay('2026-07-15', 'YYYY/MM/DD', OPTIONS)).toBe('2026/07/15');
        expect(datetime.formatGregorianCalendarMonthDashDay('07-15' as TextualMonthDay, 'MM/DD', OPTIONS)).toBe('07/15');
    });

    test('strictly validates local Gregorian date strings and formats local dates', () => {
        const leapDay = datetime.getLocalDateFromYearDashMonthDashDay('2024-02-29');
        expect(leapDay).toBeInstanceOf(Date);
        expect(datetime.getGregorianCalendarYearAndMonthFromLocalDate(leapDay!)).toBe('2024-02-29');
        expect(datetime.getGregorianCalendarYearAndMonthFromLocalDate(null as unknown as Date)).toBe('');

        for (const invalid of [null, '2024-02', '0999-01-01', '10000-01-01', '2024-00-01', '2024-13-01', '2024-01-00', '2024-01-32', '2023-02-29']) {
            expect(datetime.getLocalDateFromYearDashMonthDashDay(invalid as TextualYearMonthDay)).toBeNull();
        }
        expect(datetime.getGregorianCalendarYearAndMonthFromUnixTime(unix('2026-07-15T00:00:00Z'))).toBe('2026-07');
        expect(datetime.getGregorianCalendarYearAndMonthFromUnixTime(0)).toBe('');
        expect(datetime.getGregorianCalendarMonthDays({ year: 2024, month1base: 2 })).toBe(29);
    });

    test('computes durations, day differences and before/after arithmetic', () => {
        const base = unix('2026-07-15T00:00:00Z');
        expect(datetime.getUnixTimeBeforeUnixTime(base, 2, 'days')).toBe(unix('2026-07-13T00:00:00Z'));
        expect(datetime.getUnixTimeAfterUnixTime(base, 2, 'days')).toBe(unix('2026-07-17T00:00:00Z'));
        expect(datetime.getDayDifference({ year: 2024, month: 2, day: 28 }, { year: 2024, month: 3, day: 1 })).toBe(2);
        expect(datetime.getTimeDifferenceHoursAndMinutes(-155)).toStrictEqual({ offsetHours: 2, offsetMinutes: 35 });
    });
});

describe('datetime boundaries and range enumeration', () => {
    test('builds current day, week, month and year boundaries', () => {
        expect(datetime.getTodayFirstUnixTime()).toBe(unix('2026-07-15T00:00:00Z'));
        expect(datetime.getTodayLastUnixTime()).toBe(unix('2026-07-15T23:59:59Z'));
        expect(datetime.getThisWeekFirstUnixTime(WeekDay.Monday.type)).toBe(unix('2026-07-13T00:00:00Z'));
        expect(datetime.getThisWeekFirstUnixTime(undefined as unknown as Parameters<typeof datetime.getThisWeekFirstUnixTime>[0])).toBe(unix('2026-07-12T00:00:00Z'));
        expect(datetime.getThisWeekFirstUnixTime(WeekDay.Thursday.type)).toBe(unix('2026-07-09T00:00:00Z'));
        expect(datetime.getThisWeekLastUnixTime(WeekDay.Monday.type)).toBe(unix('2026-07-19T23:59:59Z'));
        expect(datetime.getThisMonthFirstUnixTime()).toBe(unix('2026-07-01T00:00:00Z'));
        expect(datetime.getThisMonthLastUnixTime()).toBe(unix('2026-07-31T23:59:59Z'));
        expect(datetime.getThisMonthSpecifiedDayFirstUnixTime(20)).toBe(unix('2026-07-20T00:00:00Z'));
        expect(datetime.getThisMonthSpecifiedDayLastUnixTime(20)).toBe(unix('2026-07-20T23:59:59Z'));
        expect(datetime.getThisYearFirstUnixTime()).toBe(unix('2026-01-01T00:00:00Z'));
        expect(datetime.getThisYearLastUnixTime()).toBe(unix('2026-12-31T23:59:59Z'));
        expect(datetime.getThisYearFirstUnixTime(FISCAL_APRIL)).toBe(unix('2026-04-01T00:00:00Z'));
        expect(datetime.getThisYearLastUnixTime(FISCAL_APRIL)).toBe(unix('2027-03-31T23:59:59Z'));
    });

    test('builds specified day, month, quarter and year boundaries', () => {
        const value = unix('2024-05-17T11:22:33Z');
        expect(datetime.getYearFirstUnixTimeBySpecifiedUnixTime(value)).toBe(unix('2024-01-01T00:00:00Z'));
        expect(datetime.getYearLastUnixTimeBySpecifiedUnixTime(value)).toBe(unix('2024-12-31T23:59:59Z'));
        expect(datetime.getQuarterFirstUnixTimeBySpecifiedUnixTime(value)).toBe(unix('2024-04-01T00:00:00Z'));
        expect(datetime.getQuarterLastUnixTimeBySpecifiedUnixTime(value)).toBe(unix('2024-06-30T23:59:59Z'));
        expect(datetime.getMonthFirstUnixTimeBySpecifiedUnixTime(value)).toBe(unix('2024-05-01T00:00:00Z'));
        expect(datetime.getMonthLastUnixTimeBySpecifiedUnixTime(value)).toBe(unix('2024-05-31T23:59:59Z'));
        expect(datetime.getDayFirstUnixTimeBySpecifiedUnixTime(value)).toBe(unix('2024-05-17T00:00:00Z'));
        expect(datetime.getDayLastUnixTimeBySpecifiedUnixTime(value)).toBe(unix('2024-05-17T23:59:59Z'));
        expect(datetime.getYearFirstUnixTime(2024)).toBe(unix('2024-01-01T00:00:00Z'));
        expect(datetime.getYearLastUnixTime(2024)).toBe(unix('2024-12-31T23:59:59Z'));
        expect(datetime.getQuarterFirstUnixTime({ year: 2024, quarter: 3 })).toBe(unix('2024-07-01T00:00:00Z'));
        expect(datetime.getQuarterLastUnixTime({ year: 2024, quarter: 3 })).toBe(unix('2024-09-30T23:59:59Z'));
    });

    test('normalizes range object bases and rejects missing endpoints', () => {
        expect(datetime.getStartEndYearMonthRange('2024-02', '2025-03')).toStrictEqual({
            startYearMonth: { year: 2024, month0base: 1 },
            endYearMonth: { year: 2025, month0base: 2 }
        });
        expect(datetime.getStartEndYearMonthRange({ year: 2024, month0base: 1 }, { year: 2025, month0base: 2 })).toBeTruthy();
        expect(datetime.getStartEndYearMonthRange({ year: 2024, month1base: 2 }, { year: 2025, month1base: 3 })).toBeTruthy();
        expect(datetime.getStartEndYearMonthRange('', '2025-03')).toBeNull();
        expect(datetime.getStartEndYearMonthRange('2024-02', '')).toBeNull();
    });

    test('enumerates calendar years, quarters, months, days and fiscal years', () => {
        expect(datetime.getAllYearsStartAndEndUnixTimes('2024-02', '2026-03').map(item => item.year)).toStrictEqual([2024, 2025, 2026]);
        expect(datetime.getAllYearsStartAndEndUnixTimes('', '2026-03')).toStrictEqual([]);
        expect(datetime.getAllQuartersStartAndEndUnixTimes('2024-10', '2025-04').map(item => [item.year, item.quarter])).toStrictEqual([
            [2024, 4], [2025, 1], [2025, 2]
        ]);
        expect(datetime.getAllQuartersStartAndEndUnixTimes('', '2025-04')).toStrictEqual([]);
        expect(datetime.getAllMonthsStartAndEndUnixTimes('2024-11', '2025-02').map(item => [item.year, item.month0base])).toStrictEqual([
            [2024, 10], [2024, 11], [2025, 0], [2025, 1]
        ]);
        expect(datetime.getAllMonthsStartAndEndUnixTimes('', '2025-02')).toStrictEqual([]);
        expect(datetime.getAllDaysStartAndEndUnixTimes(unix('2024-02-28T10:00:00Z'), unix('2024-03-01T10:00:00Z')).map(item => [item.year, item.month, item.day])).toStrictEqual([
            [2024, 2, 28], [2024, 2, 29], [2024, 3, 1]
        ]);
        expect(datetime.getAllDaysStartAndEndUnixTimes(0, 1)).toStrictEqual([]);
        expect(datetime.getAllDaysStartAndEndUnixTimes(1, 0)).toStrictEqual([]);

        expect(datetime.getAllFiscalYearsStartAndEndUnixTimes('2024-01', '2026-12', FISCAL_APRIL).map(item => item.year)).toStrictEqual([2024, 2025, 2026, 2027]);
        expect(datetime.getAllFiscalYearsStartAndEndUnixTimes('2024-01', '2024-12', 999)).toHaveLength(1);
        expect(datetime.getAllFiscalYearsStartAndEndUnixTimes('', '2024-12', FISCAL_APRIL)).toStrictEqual([]);
        expect(FiscalYearStart.valueOf(FISCAL_APRIL)).toBeDefined();
    });
});

describe('datetime range classification and shifting', () => {
    test('selects explicit, language-default and system-default formats', () => {
        const all = LongDateFormat.values();
        const map = LongDateFormat.all();
        expect(datetime.getDateTimeFormatType(map, all, 1, LongDateFormat.Default.key, LongDateFormat.Default)).toBe(all[0]);
        expect(datetime.getDateTimeFormatType(map, all, 0, LongDateFormat.Default.key, LongDateFormat.Default)).toBe(LongDateFormat.Default);
        expect(datetime.getDateTimeFormatType(map, all, 999, 'missing', LongDateFormat.Default)).toBe(LongDateFormat.Default);
        expect(datetime.getDateTimeFormatType(ShortDateFormat.all(), ShortDateFormat.values(), 0, 'missing', ShortDateFormat.Default)).toBe(ShortDateFormat.Default);
    });

    test('shifts whole multi-month, year, month-like and arbitrary ranges', () => {
        const twoMonths = datetime.getShiftedDateRange(unix('2024-01-01T00:00:00Z'), unix('2024-02-29T23:59:59Z'), 1);
        expect(twoMonths).toStrictEqual({ minTime: unix('2024-03-01T00:00:00Z'), maxTime: unix('2024-04-30T23:59:59Z') });

        const year = datetime.getShiftedDateRange(unix('2024-03-15T00:00:00Z'), unix('2025-03-14T23:59:59Z'), 1);
        expect(year).toStrictEqual({ minTime: unix('2025-03-15T00:00:00Z'), maxTime: unix('2026-03-14T23:59:59Z') });

        const monthLike = datetime.getShiftedDateRange(unix('2024-01-15T00:00:00Z'), unix('2024-02-14T23:59:59Z'), -1);
        expect(monthLike).toStrictEqual({ minTime: unix('2023-12-15T00:00:00Z'), maxTime: unix('2024-01-14T23:59:59Z') });

        const arbitrary = datetime.getShiftedDateRange(100, 199, 2);
        expect(arbitrary).toStrictEqual({ minTime: 300, maxTime: 399 });
        expect(datetime.getShiftedDateRangeAndDateType(100, 199, 1, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal)).toMatchObject({ minTime: 200, maxTime: 299 });
    });

    test('returns all supported preset date ranges and rejects unsupported types', () => {
        for (const range of DateRange.values()) {
            const result = datetime.getDateRangeByDateType(range.type, WeekDay.Monday.type, FISCAL_APRIL);
            if (range.isBillingCycle || range === DateRange.Custom) {
                expect(result).toBeNull();
            } else {
                expect(result).toMatchObject({ dateType: range.type });
            }
        }
        expect(datetime.getDateRangeByDateType(undefined, WeekDay.Monday.type, FISCAL_APRIL)).toBeNull();
        expect(datetime.getDateRangeByDateType(999, WeekDay.Monday.type, FISCAL_APRIL)).toBeNull();
    });

    test('classifies preset ranges while respecting scene availability', () => {
        const thisMonth = datetime.getDateRangeByDateType(DateRange.ThisMonth.type, WeekDay.Monday.type, FISCAL_APRIL)!;
        expect(datetime.getDateTypeByDateRange(thisMonth.minTime, thisMonth.maxTime, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal)).toBe(DateRange.ThisMonth.type);
        expect(datetime.getDateTypeByDateRange(thisMonth.minTime, thisMonth.maxTime, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.TrendAnalysis)).toBe(DateRange.Custom.type);
        expect(datetime.getDateTypeByDateRange(1, 2, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal)).toBe(DateRange.Custom.type);
    });

    test('builds current and previous billing cycles with statement and fallback dates', () => {
        const current20 = datetime.getDateRangeByBillingCycleDateType(DateRange.CurrentBillingCycle.type, WeekDay.Monday.type, FISCAL_APRIL, 20)!;
        expect(current20).toStrictEqual({
            dateType: DateRange.CurrentBillingCycle.type,
            minTime: unix('2026-06-21T00:00:00Z'),
            maxTime: unix('2026-07-20T23:59:59Z')
        });
        const current10 = datetime.getDateRangeByBillingCycleDateType(DateRange.CurrentBillingCycle.type, WeekDay.Monday.type, FISCAL_APRIL, 10)!;
        expect(current10).toStrictEqual({
            dateType: DateRange.CurrentBillingCycle.type,
            minTime: unix('2026-07-11T00:00:00Z'),
            maxTime: unix('2026-08-10T23:59:59Z')
        });
        const previous = datetime.getDateRangeByBillingCycleDateType(DateRange.PreviousBillingCycle.type, WeekDay.Monday.type, FISCAL_APRIL, 20)!;
        expect(previous.maxTime).toBe(unix('2026-06-20T23:59:59Z'));
        expect(datetime.getDateRangeByBillingCycleDateType(DateRange.CurrentBillingCycle.type, WeekDay.Monday.type, FISCAL_APRIL, null)).toMatchObject({ dateType: DateRange.CurrentBillingCycle.type });
        expect(datetime.getDateRangeByBillingCycleDateType(DateRange.PreviousBillingCycle.type, WeekDay.Monday.type, FISCAL_APRIL, undefined)).toMatchObject({ dateType: DateRange.PreviousBillingCycle.type });
        expect(datetime.getDateRangeByBillingCycleDateType(999, WeekDay.Monday.type, FISCAL_APRIL, 20)).toBeNull();
    });

    test('classifies and shifts billing-cycle ranges in both directions', () => {
        const previous = datetime.getDateRangeByBillingCycleDateType(DateRange.PreviousBillingCycle.type, WeekDay.Monday.type, FISCAL_APRIL, 20)!;
        const current = datetime.getDateRangeByBillingCycleDateType(DateRange.CurrentBillingCycle.type, WeekDay.Monday.type, FISCAL_APRIL, 20)!;

        expect(datetime.getDateTypeByBillingCycleDateRange(previous.minTime, previous.maxTime, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal, 20)).toBe(DateRange.PreviousBillingCycle.type);
        expect(datetime.getDateTypeByBillingCycleDateRange(current.minTime, current.maxTime, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal, 20)).toBe(DateRange.CurrentBillingCycle.type);
        expect(datetime.getDateTypeByBillingCycleDateRange(1, 2, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal, 20)).toBeNull();
        expect(datetime.getDateTypeByBillingCycleDateRange(1, 2, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.TrendAnalysis, 20)).toBeNull();
        expect(datetime.getDateTypeByBillingCycleDateRange(1, 2, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal, null)).toBeNull();

        expect(datetime.getShiftedDateRangeAndDateTypeForBillingCycle(previous.minTime, previous.maxTime, 1, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal, 20)).toStrictEqual(current);
        expect(datetime.getShiftedDateRangeAndDateTypeForBillingCycle(current.minTime, current.maxTime, -1, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal, 20)).toStrictEqual(previous);
        expect(datetime.getShiftedDateRangeAndDateTypeForBillingCycle(
            datetime.getUnixTimeBeforeUnixTime(previous.minTime, 1, 'months'),
            datetime.getUnixTimeBeforeUnixTime(previous.maxTime, 1, 'months'),
            1,
            WeekDay.Monday.type,
            FISCAL_APRIL,
            DateRangeScene.Normal,
            20
        )).toStrictEqual(previous);
        expect(datetime.getShiftedDateRangeAndDateTypeForBillingCycle(
            datetime.getUnixTimeAfterUnixTime(current.minTime, 1, 'months'),
            datetime.getUnixTimeAfterUnixTime(current.maxTime, 1, 'months'),
            -1,
            WeekDay.Monday.type,
            FISCAL_APRIL,
            DateRangeScene.Normal,
            20
        )).toStrictEqual(current);
        expect(datetime.getShiftedDateRangeAndDateTypeForBillingCycle(1, 2, 1, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal, 20)).toBeNull();
        expect(datetime.getShiftedDateRangeAndDateTypeForBillingCycle(1, 2, 1, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.TrendAnalysis, 20)).toBeNull();
        expect(datetime.getShiftedDateRangeAndDateTypeForBillingCycle(1, 2, 1, WeekDay.Monday.type, FISCAL_APRIL, DateRangeScene.Normal, null)).toBeNull();
    });
});

describe('recent ranges, date composition and full-range checks', () => {
    test('creates recent month ranges and finds preset/custom indices', () => {
        const ranges = datetime.getRecentMonthDateRanges(3);
        expect(ranges.map(item => item.dateType)).toStrictEqual([DateRange.ThisMonth.type, DateRange.LastMonth.type, DateRange.Custom.type]);
        expect(ranges.map(item => item.month)).toStrictEqual([7, 6, 5]);

        const localized: LocalizedRecentMonthDateRange[] = [
            { ...ranges[0]!, isPreset: true, displayName: 'July' },
            { ...ranges[1]!, isPreset: true, displayName: 'June' },
            { dateType: DateRange.All.type, minTime: 0, maxTime: 0, isPreset: false, displayName: 'All' },
            { dateType: DateRange.Custom.type, minTime: 0, maxTime: 0, isPreset: false, displayName: 'Custom' }
        ];

        expect(datetime.getRecentDateRangeIndexByDateType(localized, DateRange.All.type)).toBe(2);
        expect(datetime.getRecentDateRangeIndexByDateType(localized, 999)).toBe(-1);
        expect(datetime.getRecentDateRangeIndex(localized, DateRange.All.type, 0, 0, WeekDay.Monday.type, FISCAL_APRIL)).toBe(2);
        expect(datetime.getRecentDateRangeIndex(localized, 999, 0, 0, WeekDay.Monday.type, FISCAL_APRIL)).toBe(3);
        expect(datetime.getRecentDateRangeIndex(localized, DateRange.Custom.type, ranges[0]!.minTime, ranges[0]!.maxTime, WeekDay.Monday.type, FISCAL_APRIL)).toBe(0);
        expect(datetime.getRecentDateRangeIndex(localized, DateRange.Custom.type, 1, 2, WeekDay.Monday.type, FISCAL_APRIL)).toBe(3);
    });

    test('expands partial ranges to a full month but leaves full months unchanged', () => {
        const full = datetime.getDateRangeByDateType(DateRange.ThisMonth.type, WeekDay.Monday.type, FISCAL_APRIL)!;
        expect(datetime.getFullMonthDateRange(full.minTime, full.maxTime, WeekDay.Monday.type, FISCAL_APRIL)).toBeNull();
        expect(datetime.getFullMonthDateRange(0, 0, WeekDay.Monday.type, FISCAL_APRIL)).toStrictEqual(full);
        expect(datetime.getFullMonthDateRange(unix('2024-05-15T00:00:00Z'), unix('2024-05-20T00:00:00Z'), WeekDay.Monday.type, FISCAL_APRIL)).toStrictEqual({
            dateType: DateRange.Custom.type,
            minTime: unix('2024-05-01T00:00:00Z'),
            maxTime: unix('2024-05-31T23:59:59Z')
        });
    });

    test('combines localized time fields in 24-hour and 12-hour modes', () => {
        const date = new Date('2026-07-15T00:00:00Z');
        const twentyThree = datetime.getCombinedDateAndTimeValues(date, NumeralSystem.WesternArabicNumerals, '23', '05', '06', MeridiemIndicator.AM.name, true);
        const midnight = datetime.getCombinedDateAndTimeValues(date, NumeralSystem.WesternArabicNumerals, '12', '05', '06', MeridiemIndicator.AM.name, false);
        const afternoon = datetime.getCombinedDateAndTimeValues(date, NumeralSystem.WesternArabicNumerals, '01', '05', '06', MeridiemIndicator.PM.name, false);

        expect([twentyThree.getHours(), twentyThree.getMinutes(), twentyThree.getSeconds()]).toStrictEqual([23, 5, 6]);
        expect([midnight.getHours(), midnight.getMinutes(), midnight.getSeconds()]).toStrictEqual([0, 5, 6]);
        expect([afternoon.getHours(), afternoon.getMinutes(), afternoon.getSeconds()]).toStrictEqual([13, 5, 6]);
    });

    test('chooses a valid preserved day, current day or month end', () => {
        const february = unix('2024-02-10T00:00:00Z');
        expect(datetime.getValidMonthDayOrCurrentDayShortDate(february, '2026-07-20')).toBe('2024-02-20');
        expect(datetime.getValidMonthDayOrCurrentDayShortDate(february, '2026-07-31')).toBe('2024-02-29');
        expect(datetime.getValidMonthDayOrCurrentDayShortDate(february, 'invalid')).toBe('2024-02-29');
        expect(datetime.getValidMonthDayOrCurrentDayShortDate(datetime.getCurrentUnixTime(), '')).toBe('2026-07-15');
    });

    test('recognizes full year, full month and same-month boundaries', () => {
        const yearMin = unix('2024-01-01T00:00:00Z');
        const yearMax = unix('2024-12-31T23:59:59Z');
        const monthMin = unix('2024-02-01T00:00:00Z');
        const monthMax = unix('2024-02-29T23:59:59Z');

        expect(datetime.isDateRangeMatchFullYears(yearMin, yearMax)).toBe(true);
        expect(datetime.isDateRangeMatchFullYears(monthMin, monthMax)).toBe(false);
        expect(datetime.isDateRangeMatchFullMonths(monthMin, monthMax)).toBe(true);
        expect(datetime.isDateRangeMatchFullMonths(monthMin + 1, monthMax)).toBe(false);
        expect(datetime.isDateRangeMatchOneMonth(monthMin, monthMax)).toBe(true);
        expect(datetime.isDateRangeMatchOneMonth(monthMin, unix('2024-03-31T23:59:59Z'))).toBe(false);
    });
});
