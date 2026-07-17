import moment from 'moment-timezone';

import { CalendarType, type ChineseCalendarLocaleData } from '@/core/calendar.ts';
import { type DateTimeFormatOptions } from '@/core/datetime.ts';
import { NumeralSystem } from '@/core/numeral.ts';
import {
    getHourIn12HourFormat,
    getUtcOffsetByUtcOffsetMinutes,
    MomentDateTime
} from '@/lib/datetime/moment_datetime.ts';

const chineseCalendarLocaleData: ChineseCalendarLocaleData = {
    numerals: ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'],
    monthNames: Array.from({ length: 12 }, (_, index) => `${index + 1}月`),
    dayNames: Array.from({ length: 30 }, (_, index) => `${index + 1}日`),
    leapMonthPrefix: '闰',
    solarTermNames: Array.from({ length: 24 }, (_, index) => `节气${index + 1}`)
};

const gregorianOptions: DateTimeFormatOptions = {
    numeralSystem: NumeralSystem.WesternArabicNumerals,
    calendarType: CalendarType.Gregorian,
    localeData: moment.localeData('en'),
    chineseCalendarLocaleData,
    persianCalendarLocaleData: {
        monthNames: Array.from({ length: 12 }, (_, index) => `Persian month ${index + 1}`),
        monthShortNames: Array.from({ length: 12 }, (_, index) => `P${index + 1}`)
    }
};

function dateTime(value = '2024-03-20T13:05:09.000'): MomentDateTime {
    return MomentDateTime.of(moment.tz(value, 'Asia/Kolkata')) as MomentDateTime;
}

describe('MomentDateTime behavior coverage', () => {
    afterAll(() => {
        moment.tz.setDefault();
        jest.useRealTimers();
    });

    test('exposes Gregorian calendar, clock, timezone, and conversion values', () => {
        const value = dateTime();

        expect(value.getUnixTime()).toBe(moment.tz('2024-03-20T13:05:09', 'Asia/Kolkata').unix());
        expect(value.getGregorianCalendarYear()).toBe(2024);
        expect(value.getGregorianCalendarQuarter()).toBe(1);
        expect(value.getLocalizedCalendarQuarter(gregorianOptions)).toBe(1);
        expect(value.getGregorianCalendarMonth()).toBe(3);
        expect(value.getGregorianCalendarMonthDisplayName(gregorianOptions)).toBe('March');
        expect(value.getGregorianCalendarMonthDisplayShortName(gregorianOptions)).toBe('Mar');
        expect(value.getLocalizedCalendarMonth(gregorianOptions)).toBe('3');
        expect(value.getLocalizedCalendarMonthDisplayName(gregorianOptions)).toBe('March');
        expect(value.getLocalizedCalendarMonthDisplayShortName(gregorianOptions)).toBe('Mar');
        expect(value.getGregorianCalendarDay()).toBe(20);
        expect(value.getLocalizedCalendarDay(gregorianOptions)).toBe('20');
        expect(value.isLocalizedCalendarFirstDayOfMonth(gregorianOptions)).toBe(false);
        expect(value.getGregorianCalendarYearDashMonthDashDay()).toBe('2024-03-20');
        expect(value.getGregorianCalendarYearDashMonth()).toBe('2024-03');
        expect(value.getWeekDay().type).toBe(3);
        expect(value.getWeekDayDisplayName(gregorianOptions)).toBe('Wednesday');
        expect(value.getWeekDayDisplayShortName(gregorianOptions)).toBe('Wed');
        expect(value.getWeekDayDisplayMinName(gregorianOptions)).toBe('We');
        expect(value.getHour()).toBe(13);
        expect(value.getMinute()).toBe(5);
        expect(value.getSecond()).toBe(9);
        expect(value.getDisplayAMPM(gregorianOptions)).toBe('PM');
        expect(value.getTimezoneUtcOffsetMinutes()).toBe(330);
        expect(value.toGregorianCalendarYearMonthDay()).toEqual({ year: 2024, month: 3, day: 20 });
        expect(value.toGregorianCalendarYear0BasedMonth()).toEqual({ year: 2024, month0base: 2 });
        expect(value.getDateTimeAfterDays(2).getGregorianCalendarDay()).toBe(22);
    });

    test('returns empty localized labels when locale data is absent', () => {
        const value = dateTime();
        const noLocale = {} as DateTimeFormatOptions;

        expect(value.getGregorianCalendarMonthDisplayName(noLocale)).toBe('');
        expect(value.getGregorianCalendarMonthDisplayShortName(noLocale)).toBe('');
        expect(value.getLocalizedCalendarMonthDisplayName(noLocale)).toBe('');
        expect(value.getLocalizedCalendarMonthDisplayShortName(noLocale)).toBe('');
        expect(value.getWeekDayDisplayName(noLocale)).toBe('');
        expect(value.getWeekDayDisplayShortName(noLocale)).toBe('');
        expect(value.getWeekDayDisplayMinName(noLocale)).toBe('');
        expect(value.getDisplayAMPM(noLocale)).toBe('');
    });

    test('projects Buddhist, Persian, and Chinese localized calendar values', () => {
        const value = dateTime();
        const buddhist = { ...gregorianOptions, calendarType: CalendarType.Buddhist };
        const persian = { ...gregorianOptions, calendarType: CalendarType.Persian };
        const chinese = { ...gregorianOptions, calendarType: CalendarType.Chinese };

        expect(value.getLocalizedCalendarYear(buddhist)).toBe('2567');
        expect(value.getLocalizedCalendarYear(persian)).toBe('1403');
        expect(value.getLocalizedCalendarMonth(persian)).toBe('1');
        expect(value.getLocalizedCalendarMonthDisplayName(persian)).toBe('Persian month 1');
        expect(value.getLocalizedCalendarMonthDisplayShortName(persian)).toBe('P1');
        expect(value.getLocalizedCalendarDay(persian)).toBe('1');
        expect(value.isLocalizedCalendarFirstDayOfMonth(persian)).toBe(true);

        expect(value.getLocalizedCalendarYear(chinese)).not.toBe('');
        expect(value.getLocalizedCalendarMonth(chinese)).not.toBe('');
        expect(value.getLocalizedCalendarMonthDisplayName(chinese)).not.toBe('');
        expect(value.getLocalizedCalendarMonthDisplayShortName(chinese)).not.toBe('');
        expect(value.getLocalizedCalendarDay(chinese)).not.toBe('');
        expect(typeof value.isLocalizedCalendarFirstDayOfMonth(chinese)).toBe('boolean');

        expect(value.getLocalizedCalendarYear(gregorianOptions)).toBe('2024');
        expect(dateTime('2024-03-01T00:00:00').isLocalizedCalendarFirstDayOfMonth(gregorianOptions)).toBe(true);
    });

    test('formats every supported token, localized digits, and literal characters', () => {
        const value = dateTime();
        const formatted = value.format(
            'YYYY|YY|M|MM|MMM|MMMM|D|DD|dd|ddd|dddd|H|HH|h|hh|m|mm|s|ss|A|Z|literal',
            gregorianOptions
        );

        expect(formatted).toBe('2024|24|3|03|Mar|March|20|20|We|Wed|Wednesday|13|13|1|01|5|05|9|09|PM|+05:30|literal');

        const easternOptions = {
            ...gregorianOptions,
            numeralSystem: NumeralSystem.EasternArabicNumerals
        };
        expect(value.format('YYYY-MM-DD HH:mm:ss Z', easternOptions)).toBe('٢٠٢٤-٠٣-٢٠ ١٣:٠٥:٠٩ +٠٥:٣٠');
    });

    test('identifies exact Gregorian year and month boundaries', () => {
        const yearStart = dateTime('2024-01-01T00:00:00.000');
        const yearEnd = dateTime('2024-12-31T23:59:59.999');
        const monthStart = dateTime('2024-03-01T00:00:00.000');
        const monthEnd = dateTime('2024-03-31T23:59:59.999');
        const middle = dateTime('2024-03-20T13:05:09.000');

        expect(MomentDateTime.isGregorianCalendarYearFirstTime(yearStart)).toBe(true);
        expect(MomentDateTime.isGregorianCalendarYearFirstTime(middle)).toBe(false);
        expect(MomentDateTime.isGregorianCalendarYearLastTime(yearEnd)).toBe(true);
        expect(MomentDateTime.isGregorianCalendarYearLastTime(middle)).toBe(false);
        expect(MomentDateTime.isGregorianCalendarMonthFirstTime(monthStart)).toBe(true);
        expect(MomentDateTime.isGregorianCalendarMonthFirstTime(middle)).toBe(false);
        expect(MomentDateTime.isGregorianCalendarMonthLastTime(monthEnd)).toBe(true);
        expect(MomentDateTime.isGregorianCalendarMonthLastTime(middle)).toBe(false);
    });

    test('creates current values and covers 12-hour and signed offset helpers', () => {
        jest.useFakeTimers();
        jest.setSystemTime(new Date('2026-07-16T12:00:00Z'));

        expect(MomentDateTime.now().getGregorianCalendarYear()).toBe(2026);
        expect(getHourIn12HourFormat(0)).toBe(12);
        expect(getHourIn12HourFormat(12)).toBe(12);
        expect(getHourIn12HourFormat(23)).toBe(11);
        expect(getUtcOffsetByUtcOffsetMinutes(0)).toBe('+00:00');
        expect(getUtcOffsetByUtcOffsetMinutes(330)).toBe('+05:30');
        expect(getUtcOffsetByUtcOffsetMinutes(-480)).toBe('-08:00');
    });
});
