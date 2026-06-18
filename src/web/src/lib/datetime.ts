import moment from 'moment-timezone';
import { type unitOfTime } from 'moment/moment';

import {
    itemAndIndex
} from '@/core/base.ts';
import {
    type DateTime,
    type DateTimeFormatOptions,
    type TextualYearMonthDay,
    type TextualYearMonth,
    type TextualMonthDay,
    type YearUnixTime,
    type YearQuarter,
    type Year0BasedMonth,
    type Year1BasedMonth,
    type YearMonthRange,
    type YearMonthDay,
    type TimeRange,
    type TimeRangeAndDateType,
    type TimeDifference,
    type RecentMonthDateRange,
    type LocalizedRecentMonthDateRange,
    type WeekDayValue,
    type DateFormat,
    type TimeFormat,
    YearQuarterUnixTime,
    YearMonthUnixTime,
    YearMonthDayUnixTime,
    MeridiemIndicator,
    KnownDateTimeFormat,
    DateRangeScene,
    DateRange,
    LANGUAGE_DEFAULT_DATE_TIME_FORMAT_VALUE
} from '@/core/datetime.ts';
import {
    type FiscalYearUnixTime,
    FiscalYearStart
} from '@/core/fiscalyear.ts';
import {
    NumeralSystem
} from '@/core/numeral.ts';

import {
    isObject,
    isString,
    isNumber
} from './common.ts';

import {
    MomentDateTime,
    getHourIn12HourFormat,
    getUtcOffsetByUtcOffsetMinutes
} from './datetime/moment_datetime.ts';
import {
    getFiscalYearFromUnixTime,
    getFiscalYearStartUnixTime,
    getFiscalYearEndUnixTime,
    getCurrentFiscalYear,
    getFiscalYearTimeRangeFromUnixTime,
    getFiscalYearTimeRangeFromYear
} from './datetime/fiscal_year.ts';

export {
    getHourIn12HourFormat,
    getUtcOffsetByUtcOffsetMinutes,
    getFiscalYearFromUnixTime,
    getFiscalYearStartUnixTime,
    getFiscalYearEndUnixTime,
    getCurrentFiscalYear,
    getFiscalYearTimeRangeFromUnixTime,
    getFiscalYearTimeRangeFromYear
};


export function getAllowedYearRange(): number[] {
    // 年份范围：从2000年到今年+5年（支持预算的长期规划）
    return [2000, moment().year() + 5];
}

export function isYear0BasedMonthValid(year: number, month0base: number): boolean {
    if (!isNumber(year) || !isNumber(month0base)) {
        return false;
    }

    return year > 0 && month0base >= 0 && month0base <= 11;
}

export function getYear0BasedMonthObjectFromUnixTime(unixTime: number): Year0BasedMonth {
    const datetime = moment.unix(unixTime);

    return {
        year: datetime.year(),
        month0base: datetime.month()
    };
}

export function getYear0BasedMonthObjectFromString(yearMonth: TextualYearMonth | ''): Year0BasedMonth | null {
    if (!isString(yearMonth)) {
        return null;
    }

    const items = yearMonth.split('-');

    if (items.length !== 2) {
        return null;
    }

    const year = parseInt(items[0] as string);
    const month0base = parseInt(items[1] as string) - 1;

    if (!isYear0BasedMonthValid(year, month0base)) {
        return null;
    }

    return {
        year: year,
        month0base: month0base
    };
}

export function getYearMonthStringFromYear0BasedMonthObject(yearMonth: Year0BasedMonth | null): TextualYearMonth | '' {
    if (!yearMonth || !isYear0BasedMonthValid(yearMonth.year, yearMonth.month0base)) {
        return '';
    }

    return (`${yearMonth.year}-${yearMonth.month0base + 1}`) as TextualYearMonth;
}

export function isPM(hour: number): boolean {
    if (hour > 11) {
        return true;
    } else {
        return false;
    }
}

export function isUnixTimeYearMonthDayEquals(unixTime1: number, unixTime2: number): boolean {
    const date1 = moment.unix(unixTime1);
    const date2 = moment.unix(unixTime2);

    return date1.year() === date2.year() && date1.month() === date2.month() && date1.date() === date2.date();
}

export function isUnixTimeYearMonthDayHourEquals(unixTime1: number, unixTime2: number): boolean {
    const date1 = moment.unix(unixTime1);
    const date2 = moment.unix(unixTime2);

    return date1.year() === date2.year() && date1.month() === date2.month() && date1.date() === date2.date() && date1.hour() === date2.hour();
}

export function getTimezoneOffset(timezone?: string): string {
    return getUtcOffsetByUtcOffsetMinutes(getTimezoneOffsetMinutes(timezone));
}

export function getTimezoneOffsetMinutes(timezone?: string): number {
    if (timezone) {
        return moment().tz(timezone).utcOffset();
    } else {
        return moment().utcOffset();
    }
}

export function getBrowserTimezoneOffset(): string {
    return getUtcOffsetByUtcOffsetMinutes(getBrowserTimezoneOffsetMinutes());
}

export function getBrowserTimezoneOffsetMinutes(): number {
    return -new Date().getTimezoneOffset();
}

export function getLocalDatetimeFromUnixTime(unixTime: number): Date {
    return new Date(unixTime * 1000);
}

export function getUnixTimeFromLocalDatetime(datetime: Date): number {
    return Math.floor(datetime.getTime() / 1000);
}

export function getActualUnixTimeForStore(unixTime: number, utcOffset: number, currentUtcOffset: number): number {
    return unixTime - (utcOffset - currentUtcOffset) * 60;
}

export function getDummyUnixTimeForLocalUsage(unixTime: number, utcOffset: number, currentUtcOffset: number): number {
    return unixTime + (utcOffset - currentUtcOffset) * 60;
}

export function getCurrentDateTime(): DateTime {
    return MomentDateTime.now();
}

export function getCurrentUnixTime(): number {
    return moment().unix();
}

export function getYearMonthDayDateTime(year: number, month: number, day: number): DateTime {
    const date = moment().set({ year: year, month: month - 1, date: day, hour: 0, minute: 0, second: 0, millisecond: 0 });
    return MomentDateTime.of(date);
}

export function parseDateTimeFromUnixTime(unixTime: number, utcOffset?: number, currentUtcOffset?: number): DateTime {
    if (isNumber(utcOffset)) {
        if (!isNumber(currentUtcOffset)) {
            currentUtcOffset = getTimezoneOffsetMinutes();
        }

        unixTime = getDummyUnixTimeForLocalUsage(unixTime, utcOffset, currentUtcOffset);
    }

    return MomentDateTime.of(moment.unix(unixTime));
}

export function parseDateTimeFromKnownDateTimeFormat(dateTime: string, format: KnownDateTimeFormat): DateTime | undefined {
    const m = moment(dateTime, format.format);

    if (!m.isValid()) {
        return undefined;
    }

    return MomentDateTime.of(m);
}

export function parseDateTimeFromString(dateTime: string, format: string): DateTime | undefined {
    const m = moment(dateTime, format);

    if (!m.isValid()) {
        return undefined;
    }

    return MomentDateTime.of(m);
}

export function formatUnixTime(unixTime: number, format: string, options: DateTimeFormatOptions, utcOffset?: number, currentUtcOffset?: number): string {
    return parseDateTimeFromUnixTime(unixTime, utcOffset, currentUtcOffset).format(format, options);
}

export function formatCurrentTime(format: string, options: DateTimeFormatOptions): string {
    return MomentDateTime.now().format(format, options);
}

export function formatGregorianCalendarYearDashMonthDashDay(date: TextualYearMonthDay, format: string, options: DateTimeFormatOptions): string {
    return MomentDateTime.of(moment(date, 'YYYY-MM-DD')).format(format, options);
}

export function formatGregorianCalendarMonthDashDay(monthDay: TextualMonthDay, format: string, options: DateTimeFormatOptions): string {
    return MomentDateTime.of(moment(monthDay, 'MM-DD')).format(format, options);
}

export function getLocalDateFromYearDashMonthDashDay(date: TextualYearMonthDay): Date | null {
    if (!isString(date)) {
        return null;
    }

    const items = date.split('-');

    if (items.length !== 3) {
        return null;
    }

    const year = parseInt(items[0] as string);
    const month = parseInt(items[1] as string);
    const day = parseInt(items[2] as string);

    if (!isNumber(year) || !isNumber(month) || !isNumber(day)) {
        return null;
    }

    if (year < 1000 || year > 9999 || month < 1 || month > 12 || day < 1 || day > 31) {
        return null;
    }

    const dateObj = new Date(year, month - 1, day);

    if (dateObj.getFullYear() !== year || dateObj.getMonth() !== (month - 1) || dateObj.getDate() !== day) {
        return null;
    }

    return dateObj;
}

export function getGregorianCalendarYearAndMonthFromLocalDate(date: Date): TextualYearMonthDay | '' {
    if (!date) {
        return '';
    }

    const year = date.getFullYear().toString().padStart(4, NumeralSystem.WesternArabicNumerals.digitZero);
    const month = (date.getMonth() + 1).toString().padStart(2, NumeralSystem.WesternArabicNumerals.digitZero);
    const day = (date.getDate()).toString().padStart(2, NumeralSystem.WesternArabicNumerals.digitZero);

    return (`${year}-${month}-${day}`) as TextualYearMonthDay;
}

export function getGregorianCalendarYearAndMonthFromUnixTime(unixTime: number): TextualYearMonth | '' {
    if (!unixTime) {
        return '';
    }

    return parseDateTimeFromUnixTime(unixTime).getGregorianCalendarYearDashMonth();
}

export function getGregorianCalendarMonthDays(yearMonth: Year1BasedMonth): number {
    return moment().set({ year: yearMonth.year, month: yearMonth.month1base - 1 }).daysInMonth();
}

export function getAMOrPM(hour: number): string {
    return isPM(hour) ? MeridiemIndicator.PM.name : MeridiemIndicator.AM.name;
}

export function getUnixTimeBeforeUnixTime(unixTime: number, amount: number, unit: unitOfTime.DurationConstructor): number {
    return moment.unix(unixTime).subtract(amount, unit as moment.unitOfTime.Base).unix();
}

export function getUnixTimeAfterUnixTime(unixTime: number, amount: number, unit: unitOfTime.DurationConstructor): number {
    return moment.unix(unixTime).add(amount, unit as moment.unitOfTime.Base).unix();
}

export function getDayDifference(yearMonthDay1: YearMonthDay, yearMonthDay2: YearMonthDay): number {
    const date1 = moment().set({ year: yearMonthDay1.year, month: yearMonthDay1.month - 1, date: yearMonthDay1.day, hour: 0, minute: 0, second: 0, millisecond: 0 });
    const date2 = moment().set({ year: yearMonthDay2.year, month: yearMonthDay2.month - 1, date: yearMonthDay2.day, hour: 0, minute: 0, second: 0, millisecond: 0 });
    return date2.diff(date1, 'days');
}

export function getTimeDifferenceHoursAndMinutes(timeDifferenceInMinutes: number): TimeDifference {
    const offsetHours = Math.trunc(Math.abs(timeDifferenceInMinutes) / 60);
    const offsetMinutes = Math.abs(timeDifferenceInMinutes) - offsetHours * 60;

    return {
        offsetHours: offsetHours,
        offsetMinutes: offsetMinutes,
    };
}

export function getTodayFirstUnixTime(): number {
    return moment().set({ hour: 0, minute: 0, second: 0, millisecond: 0 }).unix();
}

export function getTodayLastUnixTime(): number {
    return moment.unix(getTodayFirstUnixTime()).add(1, 'days').subtract(1, 'seconds').unix();
}

export function getThisWeekFirstUnixTime(firstDayOfWeek: WeekDayValue): number {
    const today = moment.unix(getTodayFirstUnixTime());

    if (!isNumber(firstDayOfWeek)) {
        firstDayOfWeek = 0;
    }

    let dayOfWeek = today.day() - firstDayOfWeek;

    if (dayOfWeek < 0) {
        dayOfWeek += 7;
    }

    return today.subtract(dayOfWeek, 'days').unix();
}

export function getThisWeekLastUnixTime(firstDayOfWeek: WeekDayValue): number {
    return moment.unix(getThisWeekFirstUnixTime(firstDayOfWeek)).add(7, 'days').subtract(1, 'seconds').unix();
}

export function getThisMonthFirstUnixTime(): number {
    const today = moment.unix(getTodayFirstUnixTime());
    return today.subtract(today.date() - 1, 'days').unix();
}

export function getThisMonthLastUnixTime(): number {
    return moment.unix(getThisMonthFirstUnixTime()).add(1, 'months').subtract(1, 'seconds').unix();
}

export function getThisMonthSpecifiedDayFirstUnixTime(date: number): number {
    return moment().set({ date: date, hour: 0, minute: 0, second: 0, millisecond: 0 }).unix();
}

export function getThisMonthSpecifiedDayLastUnixTime(date: number): number {
    return moment.unix(getThisMonthSpecifiedDayFirstUnixTime(date)).add(1, 'days').subtract(1, 'seconds').unix();
}

export function getThisYearFirstUnixTime(fiscalYearStart?: number): number {
    if (fiscalYearStart && fiscalYearStart > 1) {
        return getFiscalYearStartUnixTime(getTodayFirstUnixTime(), fiscalYearStart);
    }
    const today = moment.unix(getTodayFirstUnixTime());
    return today.subtract(today.dayOfYear() - 1, 'days').unix();
}

export function getThisYearLastUnixTime(fiscalYearStart?: number): number {
    if (fiscalYearStart && fiscalYearStart > 1) {
        return getFiscalYearEndUnixTime(getTodayFirstUnixTime(), fiscalYearStart);
    }
    return moment.unix(getThisYearFirstUnixTime()).add(1, 'years').subtract(1, 'seconds').unix();
}

export function getYearFirstUnixTimeBySpecifiedUnixTime(unixTime: number): number {
    const date = moment.unix(unixTime).set({ hour: 0, minute: 0, second: 0, millisecond: 0 });
    return date.subtract(date.dayOfYear() - 1, 'days').unix();
}

export function getYearLastUnixTimeBySpecifiedUnixTime(unixTime: number): number {
    return moment.unix(getYearFirstUnixTimeBySpecifiedUnixTime(unixTime)).add(1, 'years').subtract(1, 'seconds').unix();
}

export function getQuarterFirstUnixTimeBySpecifiedUnixTime(unixTime: number): number {
    const date = moment.unix(unixTime).set({ hour: 0, minute: 0, second: 0, millisecond: 0 });
    const month = date.month();
    const quarterStartMonth = Math.floor(month / 3) * 3;
    return date.set({ month: quarterStartMonth, date: 1 }).unix();
}

export function getQuarterLastUnixTimeBySpecifiedUnixTime(unixTime: number): number {
    return moment.unix(getQuarterFirstUnixTimeBySpecifiedUnixTime(unixTime)).add(3, 'months').subtract(1, 'seconds').unix();
}

export function getMonthFirstUnixTimeBySpecifiedUnixTime(unixTime: number): number {
    const date = moment.unix(unixTime).set({ hour: 0, minute: 0, second: 0, millisecond: 0 });
    return date.subtract(date.date() - 1, 'days').unix();
}

export function getMonthLastUnixTimeBySpecifiedUnixTime(unixTime: number): number {
    return moment.unix(getMonthFirstUnixTimeBySpecifiedUnixTime(unixTime)).add(1, 'months').subtract(1, 'seconds').unix();
}

export function getDayFirstUnixTimeBySpecifiedUnixTime(unixTime: number): number {
    return moment.unix(unixTime).set({ hour: 0, minute: 0, second: 0, millisecond: 0 }).unix();
}

export function getDayLastUnixTimeBySpecifiedUnixTime(unixTime: number): number {
    return moment.unix(unixTime).set({ hour: 0, minute: 0, second: 0, millisecond: 0 }).add(1, 'days').subtract(1, 'seconds').unix();
}

export function getYearFirstUnixTime(year: number): number {
    return moment().set({ year: year, month: 0, date: 1, hour: 0, minute: 0, second: 0, millisecond: 0 }).unix();
}

export function getYearLastUnixTime(year: number): number {
    return moment.unix(getYearFirstUnixTime(year)).add(1, 'years').subtract(1, 'seconds').unix();
}

export function getQuarterFirstUnixTime(yearQuarter: YearQuarter): number {
    return moment().set({ year: yearQuarter.year, month: (yearQuarter.quarter - 1) * 3, date: 1, hour: 0, minute: 0, second: 0, millisecond: 0 }).unix();
}

export function getQuarterLastUnixTime(yearQuarter: YearQuarter): number {
    return moment.unix(getQuarterFirstUnixTime(yearQuarter)).add(3, 'months').subtract(1, 'seconds').unix();
}

export function getYearMonthFirstUnixTime(yearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | ''): number {
    let yearMonthObj: Year0BasedMonth | null = null;

    if (isString(yearMonth)) {
        yearMonthObj = getYear0BasedMonthObjectFromString(yearMonth);
    } else if (isObject(yearMonth) && ('month0base' in yearMonth) && isYear0BasedMonthValid(yearMonth.year, yearMonth.month0base)) {
        yearMonthObj = yearMonth;
    } else if (isObject(yearMonth) && ('month1base' in yearMonth) && isYear0BasedMonthValid(yearMonth.year, yearMonth.month1base - 1)) {
        yearMonthObj = {
            year: yearMonth.year,
            month0base: yearMonth.month1base - 1
        };
    }

    if (!yearMonthObj) {
        return 0;
    }

    return moment().set({ year: yearMonthObj.year, month: yearMonthObj.month0base, date: 1, hour: 0, minute: 0, second: 0, millisecond: 0 }).unix();
}

export function getYearMonthLastUnixTime(yearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | ''): number {
    return moment.unix(getYearMonthFirstUnixTime(yearMonth)).add(1, 'months').subtract(1, 'seconds').unix();
}

export function getStartEndYearMonthRange(startYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | '', endYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | ''): YearMonthRange | null {
    let startYearMonthObj: Year0BasedMonth | null = null;
    let endYearMonthObj: Year0BasedMonth | null = null;

    if (isString(startYearMonth)) {
        startYearMonthObj = getYear0BasedMonthObjectFromString(startYearMonth);
    } else if (isObject(startYearMonth) && ('month0base' in startYearMonth)) {
        startYearMonthObj = startYearMonth;
    } else if (isObject(startYearMonth) && ('month1base' in startYearMonth)) {
        startYearMonthObj = {
            year: startYearMonth.year,
            month0base: startYearMonth.month1base - 1
        };
    }

    if (isString(endYearMonth)) {
        endYearMonthObj = getYear0BasedMonthObjectFromString(endYearMonth);
    } else if (isObject(endYearMonth) && ('month0base' in endYearMonth)) {
        endYearMonthObj = endYearMonth;
    } else if (isObject(endYearMonth) && ('month1base' in endYearMonth)) {
        endYearMonthObj = {
            year: endYearMonth.year,
            month0base: endYearMonth.month1base - 1
        };
    }

    if (!startYearMonthObj || !endYearMonthObj) {
        return null;
    }

    return {
        startYearMonth: startYearMonthObj,
        endYearMonth: endYearMonthObj
    };
}

export function getAllYearsStartAndEndUnixTimes(startYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | '', endYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | ''): YearUnixTime[] {
    const allYearTimes: YearUnixTime[] = [];
    const range = getStartEndYearMonthRange(startYearMonth, endYearMonth);

    if (!range) {
        return allYearTimes;
    }

    for (let year = range.startYearMonth.year; year <= range.endYearMonth.year; year++) {
        const yearTime: YearUnixTime = {
            year: year,
            minUnixTime: getYearFirstUnixTime(year),
            maxUnixTime: getYearLastUnixTime(year),
        };

        allYearTimes.push(yearTime);
    }

    return allYearTimes;
}

export function getAllFiscalYearsStartAndEndUnixTimes(startYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | '', endYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | '', fiscalYearStartValue: number): FiscalYearUnixTime[] {
    // 用户选择的日期范围：start=2024-01，end=2026-12
    // 结果应拆分为 4 个 FiscalYearUnixTime：
    // - 2024-01->2024-06（FY 24）- 输入起始年月到其所在财年的结束
    // - 2024-07->2025-06（FY 25）- 完整财年
    // - 2025-07->2026-06（FY 26）- 完整财年
    // - 2026-07->2026-12（FY 27）- 该财年的起始到输入结束年月

    const allFiscalYearTimes: FiscalYearUnixTime[] = [];
    const range = getStartEndYearMonthRange(startYearMonth, endYearMonth);

    if (!range) {
        return allFiscalYearTimes;
    }

    const inputStartUnixTime = getYearMonthFirstUnixTime(range.startYearMonth);
    const inputEndUnixTime = getYearMonthLastUnixTime(range.endYearMonth);
    let fiscalYearStart = FiscalYearStart.valueOf(fiscalYearStartValue);

    if (!fiscalYearStart) {
        fiscalYearStart = FiscalYearStart.Default;
    }

    // 在输入日期范围前后各多遍历 1 年，
    // 以覆盖那些起始于上一公历年的财年。
    for (let year = range.startYearMonth.year - 1; year <= range.endYearMonth.year + 1; year++) {
        const thisYearMonthUnixTime = getYearMonthFirstUnixTime({ year: year, month1base: fiscalYearStart.month });
        const fiscalStartTime = getFiscalYearStartUnixTime(thisYearMonthUnixTime, fiscalYearStart.value);
        const fiscalEndTime = getFiscalYearEndUnixTime(thisYearMonthUnixTime, fiscalYearStart.value);

        const fiscalYear = getFiscalYearFromUnixTime(fiscalStartTime, fiscalYearStart.value);

        if (fiscalStartTime <= inputEndUnixTime && fiscalEndTime >= inputStartUnixTime) {
            const fiscalYearTime: FiscalYearUnixTime = {
                year: fiscalYear,
                minUnixTime: fiscalStartTime,
                maxUnixTime: fiscalEndTime,
            };

            allFiscalYearTimes.push(fiscalYearTime);
        }

        if (fiscalStartTime > inputEndUnixTime) {
            break;
        }
    }

    return allFiscalYearTimes;
}

export function getAllQuartersStartAndEndUnixTimes(startYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | '', endYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | ''): YearQuarterUnixTime[] {
    const allYearQuarterTimes: YearQuarterUnixTime[] = [];
    const range = getStartEndYearMonthRange(startYearMonth, endYearMonth);

    if (!range) {
        return allYearQuarterTimes;
    }

    for (let year = range.startYearMonth.year, month0base = range.startYearMonth.month0base; year < range.endYearMonth.year || (year === range.endYearMonth.year && (Math.floor(month0base / 3) <= Math.floor(range.endYearMonth.month0base / 3))); ) {
        const yearQuarter: YearQuarter = {
            year: year,
            quarter: Math.floor((month0base / 3)) + 1
        };

        const minUnixTime = getQuarterFirstUnixTime(yearQuarter);
        const maxUnixTime = getQuarterLastUnixTime(yearQuarter);

        allYearQuarterTimes.push(YearQuarterUnixTime.of(yearQuarter, minUnixTime, maxUnixTime));

        if (year === range.endYearMonth.year && month0base >= range.endYearMonth.month0base) {
            break;
        }

        if (month0base >= 9) {
            year++;
            month0base = 0;
        } else {
            month0base += 3;
        }
    }

    return allYearQuarterTimes;
}

export function getAllMonthsStartAndEndUnixTimes(startYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | '', endYearMonth: Year0BasedMonth | Year1BasedMonth | TextualYearMonth | ''): YearMonthUnixTime[] {
    const allYearMonthTimes: YearMonthUnixTime[] = [];
    const range = getStartEndYearMonthRange(startYearMonth, endYearMonth);

    if (!range) {
        return allYearMonthTimes;
    }

    for (let year = range.startYearMonth.year, month0base = range.startYearMonth.month0base; year <= range.endYearMonth.year || month0base <= range.endYearMonth.month0base; ) {
        const yearMonth: Year0BasedMonth = {
            year: year,
            month0base: month0base
        };

        const minUnixTime = getYearMonthFirstUnixTime(yearMonth);
        const maxUnixTime = getYearMonthLastUnixTime(yearMonth);

        allYearMonthTimes.push(YearMonthUnixTime.of(yearMonth, minUnixTime, maxUnixTime));

        if (year === range.endYearMonth.year && month0base === range.endYearMonth.month0base) {
            break;
        }

        if (month0base >= 11) {
            year++;
            month0base = 0;
        } else {
            month0base++;
        }
    }

    return allYearMonthTimes;
}

export function getAllDaysStartAndEndUnixTimes(startUnixTime: number, endUnixTime: number): YearMonthDayUnixTime[] {
    const allYearMonthDayTimes: YearMonthDayUnixTime[] = [];

    if (!startUnixTime || !endUnixTime) {
        return allYearMonthDayTimes;
    }

    let unixTime: number = startUnixTime;

    while (unixTime <= endUnixTime) {
        const currentDateTime = parseDateTimeFromUnixTime(unixTime);
        const currentDayMinUnixTime = getDayFirstUnixTimeBySpecifiedUnixTime(unixTime);
        const currentDayMaxUnixTime = getDayLastUnixTimeBySpecifiedUnixTime(unixTime);

        allYearMonthDayTimes.push(YearMonthDayUnixTime.of(currentDateTime.toGregorianCalendarYearMonthDay(), currentDayMinUnixTime, currentDayMaxUnixTime));
        unixTime = currentDayMaxUnixTime + 1;
    }

    return allYearMonthDayTimes;
}

export function getDateTimeFormatType<T extends DateFormat | TimeFormat>(allFormatMap: Record<string, T>, allFormatArray: T[], formatTypeValue: number, languageDefaultTypeName: string, systemDefaultFormatType: T): T {
    if (formatTypeValue > LANGUAGE_DEFAULT_DATE_TIME_FORMAT_VALUE && allFormatArray[formatTypeValue - 1] && allFormatArray[formatTypeValue - 1]!.key) {
        return allFormatArray[formatTypeValue - 1] as T;
    } else if (formatTypeValue === LANGUAGE_DEFAULT_DATE_TIME_FORMAT_VALUE && allFormatMap[languageDefaultTypeName] && allFormatMap[languageDefaultTypeName].key) {
        return allFormatMap[languageDefaultTypeName];
    } else {
        return systemDefaultFormatType;
    }
}

export function getShiftedDateRange(minTime: number, maxTime: number, scale: number): TimeRange {
    const minDateTime = moment.unix(parseDateTimeFromUnixTime(minTime).getUnixTime()).set({ millisecond: 0 });
    const maxDateTime = moment.unix(parseDateTimeFromUnixTime(maxTime).getUnixTime()).set({ millisecond: 999 });

    const firstDayOfMonth = minDateTime.clone().startOf('month');
    const lastDayOfMonth = maxDateTime.clone().endOf('month');

    // 检查日期范围是否正好覆盖整月
    if (firstDayOfMonth.unix() === minDateTime.unix() && lastDayOfMonth.unix() === maxDateTime.unix()) {
        const months = maxDateTime.year() * 12 + (maxDateTime.month() + 1) - minDateTime.year() * 12 - (minDateTime.month() + 1) + 1;
        const newMinDateTime = minDateTime.add(months * scale, 'months');
        const newMaxDateTime = newMinDateTime.clone().add(months, 'months').subtract(1, 'seconds');

        return {
            minTime: newMinDateTime.unix(),
            maxTime: newMaxDateTime.unix()
        };
    }

    // 检查日期范围是否正好覆盖完整一年
    if (minDateTime.clone().add(1, 'years').subtract(1, 'seconds').unix() === maxDateTime.unix() ||
        maxDateTime.clone().subtract(1, 'years').add(1, 'seconds').unix() === minDateTime.unix()) {
        const newMinDateTime = minDateTime.add(1 * scale, 'years');
        const newMaxDateTime = maxDateTime.add(1 * scale, 'years');

        return {
            minTime: newMinDateTime.unix(),
            maxTime: newMaxDateTime.unix()
        };
    }

    // 检查日期范围是否正好覆盖完整一个月
    if (minDateTime.clone().add(1, 'months').subtract(1, 'seconds').unix() === maxDateTime.unix() ||
        maxDateTime.clone().subtract(1, 'months').add(1, 'seconds').unix() === minDateTime.unix()) {
        const newMinDateTime = minDateTime.add(1 * scale, 'months');
        const newMaxDateTime = maxDateTime.add(1 * scale, 'months');

        return {
            minTime: newMinDateTime.unix(),
            maxTime: newMaxDateTime.unix()
        };
    }

    const range = (maxTime - minTime + 1) * scale;

    return {
        minTime: minTime + range,
        maxTime: maxTime + range
    };
}

export function getShiftedDateRangeAndDateType(minTime: number, maxTime: number, scale: number, firstDayOfWeek: WeekDayValue, fiscalYearStart: number, scene: DateRangeScene): TimeRangeAndDateType {
    const newDateRange = getShiftedDateRange(minTime, maxTime, scale);
    const newDateType = getDateTypeByDateRange(newDateRange.minTime, newDateRange.maxTime, firstDayOfWeek, fiscalYearStart, scene);

    return {
        dateType: newDateType,
        minTime: newDateRange.minTime,
        maxTime: newDateRange.maxTime
    };
}

export function getShiftedDateRangeAndDateTypeForBillingCycle(minTime: number, maxTime: number, scale: number, firstDayOfWeek: WeekDayValue, fiscalYearStart: number, scene: number, statementDate: number | undefined | null): TimeRangeAndDateType | null {
    if (!statementDate || !DateRange.PreviousBillingCycle.isAvailableForScene(scene) || !DateRange.CurrentBillingCycle.isAvailableForScene(scene)) {
        return null;
    }

    const previousBillingCycleRange = getDateRangeByBillingCycleDateType(DateRange.PreviousBillingCycle.type, firstDayOfWeek, fiscalYearStart, statementDate);
    const currentBillingCycleRange = getDateRangeByBillingCycleDateType(DateRange.CurrentBillingCycle.type, firstDayOfWeek, fiscalYearStart, statementDate);

    if (previousBillingCycleRange && getUnixTimeBeforeUnixTime(previousBillingCycleRange.maxTime, 1, 'months') === maxTime && getUnixTimeBeforeUnixTime(previousBillingCycleRange.minTime, 1, 'months') === minTime && scale === 1) {
        return previousBillingCycleRange;
    } else if (previousBillingCycleRange && previousBillingCycleRange.maxTime === maxTime && previousBillingCycleRange.minTime === minTime && scale === 1) {
        return currentBillingCycleRange;
    } else if (currentBillingCycleRange && currentBillingCycleRange.maxTime === maxTime && currentBillingCycleRange.minTime === minTime && scale === -1) {
        return previousBillingCycleRange;
    } else if (currentBillingCycleRange && getUnixTimeAfterUnixTime(currentBillingCycleRange.maxTime, 1, 'months') === maxTime && getUnixTimeAfterUnixTime(currentBillingCycleRange.minTime, 1, 'months') === minTime && scale === -1) {
        return currentBillingCycleRange;
    }

    return null;
}

export function getDateTypeByDateRange(minTime: number, maxTime: number, firstDayOfWeek: WeekDayValue, fiscalYearStart: number, scene: DateRangeScene): number {
    const allDateRanges = DateRange.values();
    let newDateType = DateRange.Custom.type;

    for (const dateRange of allDateRanges) {
        if (!dateRange.isAvailableForScene(scene)) {
            continue;
        }

        const range = getDateRangeByDateType(dateRange.type, firstDayOfWeek, fiscalYearStart);

        if (range && range.minTime === minTime && range.maxTime === maxTime) {
            newDateType = dateRange.type;
            break;
        }
    }

    return newDateType;
}

export function getDateTypeByBillingCycleDateRange(minTime: number, maxTime: number, firstDayOfWeek: WeekDayValue, fiscalYearStart: number, scene: DateRangeScene, statementDate: number | undefined | null): number | null {
    if (!statementDate || !DateRange.PreviousBillingCycle.isAvailableForScene(scene) || !DateRange.CurrentBillingCycle.isAvailableForScene(scene)) {
        return null;
    }

    const previousBillingCycleRange = getDateRangeByBillingCycleDateType(DateRange.PreviousBillingCycle.type, firstDayOfWeek, fiscalYearStart, statementDate);
    const currentBillingCycleRange = getDateRangeByBillingCycleDateType(DateRange.CurrentBillingCycle.type, firstDayOfWeek, fiscalYearStart, statementDate);

    if (previousBillingCycleRange && previousBillingCycleRange.maxTime === maxTime && previousBillingCycleRange.minTime === minTime) {
        return previousBillingCycleRange.dateType;
    } else if (currentBillingCycleRange && currentBillingCycleRange.maxTime === maxTime && currentBillingCycleRange.minTime === minTime) {
        return currentBillingCycleRange.dateType;
    }

    return null;
}

export function getDateRangeByDateType(dateType: number | undefined, firstDayOfWeek: WeekDayValue, fiscalYearStart: number): TimeRangeAndDateType | null {
    let maxTime = 0;
    let minTime = 0;

    // 添加日志记录财年开始日期
    if (dateType === DateRange.ThisYear.type || dateType === DateRange.LastYear.type ||
        dateType === DateRange.ThisFiscalYear.type || dateType === DateRange.LastFiscalYear.type) {
    }

    if (dateType === DateRange.All.type) { // 全部
        maxTime = 0;
        minTime = 0;
    } else if (dateType === DateRange.ThisWeek.type) { // 本周
        maxTime = getThisWeekLastUnixTime(firstDayOfWeek);
        minTime = getThisWeekFirstUnixTime(firstDayOfWeek);
    } else if (dateType === DateRange.LastWeek.type) { // 上周
        maxTime = getUnixTimeBeforeUnixTime(getThisWeekLastUnixTime(firstDayOfWeek), 7, 'days');
        minTime = getUnixTimeBeforeUnixTime(getThisWeekFirstUnixTime(firstDayOfWeek), 7, 'days');
    } else if (dateType === DateRange.ThisMonth.type) { // 本月
        maxTime = getThisMonthLastUnixTime();
        minTime = getThisMonthFirstUnixTime();
    } else if (dateType === DateRange.LastMonth.type) { // 上月
        maxTime = getUnixTimeBeforeUnixTime(getThisMonthFirstUnixTime(), 1, 'seconds');
        minTime = getUnixTimeBeforeUnixTime(getThisMonthFirstUnixTime(), 1, 'months');
    } else if (dateType === DateRange.ThisYear.type) { // 今年 - 自然年（1月1日开始）
        const now = moment();
        maxTime = now.clone().endOf('year').unix();
        minTime = now.clone().startOf('year').unix();
    } else if (dateType === DateRange.LastYear.type) { // 去年 - 自然年
        const lastYear = moment().subtract(1, 'years');
        maxTime = lastYear.clone().endOf('year').unix();
        minTime = lastYear.clone().startOf('year').unix();
    } else if (dateType === DateRange.ThisFiscalYear.type) { // 本财年
        maxTime = getFiscalYearEndUnixTime(getTodayFirstUnixTime(), fiscalYearStart);
        minTime = getFiscalYearStartUnixTime(getTodayFirstUnixTime(), fiscalYearStart);
    } else if (dateType === DateRange.LastFiscalYear.type) { // 上一财年
        maxTime = getUnixTimeBeforeUnixTime(getFiscalYearEndUnixTime(getTodayFirstUnixTime(), fiscalYearStart), 1, 'years');
        minTime = getUnixTimeBeforeUnixTime(getFiscalYearStartUnixTime(getTodayFirstUnixTime(), fiscalYearStart), 1, 'years');
    } else if (dateType === DateRange.RecentTwelveMonths.type) { // 最近 12 个月
        maxTime = getThisMonthLastUnixTime();
        minTime = getUnixTimeBeforeUnixTime(getThisMonthFirstUnixTime(), 11, 'months');
    } else if (dateType === DateRange.RecentTwentyFourMonths.type) // 最近 24 个月
    {
        maxTime = getThisMonthLastUnixTime();
        minTime = getUnixTimeBeforeUnixTime(getThisMonthFirstUnixTime(), 23, 'months');
    } else if (dateType === DateRange.RecentThirtySixMonths.type) // 最近 36 个月
    {
        maxTime = getThisMonthLastUnixTime();
        minTime = getUnixTimeBeforeUnixTime(getThisMonthFirstUnixTime(), 35, 'months');
    } else if (dateType === DateRange.RecentTwoYears.type) // 最近 2 年 - 最近 2 个自然年
    {
        const now = moment();
        maxTime = now.clone().endOf('year').unix();
        minTime = now.clone().subtract(1, 'years').startOf('year').unix();
    } else if (dateType === DateRange.RecentThreeYears.type) // 最近 3 年 - 最近 3 个自然年
    {
        const now = moment();
        maxTime = now.clone().endOf('year').unix();
        minTime = now.clone().subtract(2, 'years').startOf('year').unix();
    } else if (dateType === DateRange.RecentFiveYears.type) // 最近 5 年 - 最近 5 个自然年
    {
        const now = moment();
        maxTime = now.clone().endOf('year').unix();
        minTime = now.clone().subtract(4, 'years').startOf('year').unix();
    } else {
        return null;
    }

    return {
        dateType: dateType,
        maxTime: maxTime,
        minTime: minTime
    };
}

export function getDateRangeByBillingCycleDateType(dateType: number, firstDayOfWeek: WeekDayValue, fiscalYearStart: number, statementDate: number | undefined | null): TimeRangeAndDateType | null {
    let maxTime = 0;
    let minTime = 0;

    if (dateType === DateRange.PreviousBillingCycle.type || dateType === DateRange.CurrentBillingCycle.type) { // 上一个账单周期 | 当前账单周期
        if (statementDate) {
            if (getCurrentDateTime().getGregorianCalendarDay() <= statementDate) {
                maxTime = getThisMonthSpecifiedDayLastUnixTime(statementDate);
                minTime = getUnixTimeBeforeUnixTime(getUnixTimeAfterUnixTime(getThisMonthSpecifiedDayFirstUnixTime(statementDate), 1, 'days'), 1, 'months');
            } else {
                maxTime = getUnixTimeAfterUnixTime(getThisMonthSpecifiedDayLastUnixTime(statementDate), 1, 'months');
                minTime = getUnixTimeAfterUnixTime(getThisMonthSpecifiedDayFirstUnixTime(statementDate), 1, 'days');
            }

            if (dateType === DateRange.PreviousBillingCycle.type) {
                maxTime = getUnixTimeBeforeUnixTime(maxTime, 1, 'months');
                minTime = getUnixTimeBeforeUnixTime(minTime, 1, 'months');
            }
        } else {
            let fallbackDateRange = null;

            if (dateType === DateRange.CurrentBillingCycle.type) { // 等同于本月
                fallbackDateRange = getDateRangeByDateType(DateRange.ThisMonth.type, firstDayOfWeek, fiscalYearStart);
            } else if (dateType === DateRange.PreviousBillingCycle.type) { // 等同于上月
                fallbackDateRange = getDateRangeByDateType(DateRange.LastMonth.type, firstDayOfWeek, fiscalYearStart);
            }

            if (fallbackDateRange) {
                maxTime = fallbackDateRange.maxTime;
                minTime = fallbackDateRange.minTime;
            }
        }
    } else {
        return null;
    }

    return {
        dateType: dateType,
        maxTime: maxTime,
        minTime: minTime
    };
}

export function getRecentMonthDateRanges(monthCount: number): RecentMonthDateRange[] {
    const recentDateRanges: RecentMonthDateRange[] = [];
    const thisMonthFirstUnixTime = getThisMonthFirstUnixTime();

    for (let i = 0; i < monthCount; i++) {
        let minTime = thisMonthFirstUnixTime;

        if (i > 0) {
            minTime = getUnixTimeBeforeUnixTime(thisMonthFirstUnixTime, i, 'months');
        }

        const maxTime = getUnixTimeBeforeUnixTime(getUnixTimeAfterUnixTime(minTime, 1, 'months'), 1, 'seconds');
        let dateType = DateRange.Custom.type;
        const year = parseDateTimeFromUnixTime(minTime).getGregorianCalendarYear();
        const month = parseDateTimeFromUnixTime(minTime).getGregorianCalendarMonth();

        if (i === 0) {
            dateType = DateRange.ThisMonth.type;
        } else if (i === 1) {
            dateType = DateRange.LastMonth.type;
        }

        recentDateRanges.push({
            dateType: dateType,
            minTime: minTime,
            maxTime: maxTime,
            year: year,
            month: month
        });
    }

    return recentDateRanges;
}

export function getRecentDateRangeIndexByDateType(allRecentMonthDateRanges: LocalizedRecentMonthDateRange[], dateType: number): number {
    for (const [recentDateRange, index] of itemAndIndex(allRecentMonthDateRanges)) {
        if (!recentDateRange.isPreset && recentDateRange.dateType === dateType) {
            return index;
        }
    }

    return -1;
}

export function getRecentDateRangeIndex(allRecentMonthDateRanges: LocalizedRecentMonthDateRange[], dateType: number, minTime: number, maxTime: number, firstDayOfWeek: WeekDayValue, fiscalYearStart: number): number {
    let dateRange = getDateRangeByDateType(dateType, firstDayOfWeek, fiscalYearStart);

    if (dateRange && dateRange.dateType === DateRange.All.type) {
        return getRecentDateRangeIndexByDateType(allRecentMonthDateRanges, DateRange.All.type);
    }

    if (!dateRange && (!maxTime || !minTime)) {
        return getRecentDateRangeIndexByDateType(allRecentMonthDateRanges, DateRange.Custom.type);
    }

    if (!dateRange) {
        dateRange = {
            dateType: DateRange.Custom.type,
            maxTime: maxTime,
            minTime: minTime
        };
    }

    for (const [recentDateRange, index] of itemAndIndex(allRecentMonthDateRanges)) {
        if (recentDateRange.isPreset && recentDateRange.minTime === dateRange.minTime && recentDateRange.maxTime === dateRange.maxTime) {
            return index;
        }
    }

    return getRecentDateRangeIndexByDateType(allRecentMonthDateRanges, DateRange.Custom.type);
}

export function getFullMonthDateRange(minTime: number, maxTime: number, firstDayOfWeek: WeekDayValue, fiscalYearStart: number): TimeRangeAndDateType | null {
    if (isDateRangeMatchOneMonth(minTime, maxTime)) {
        return null;
    }

    if (!minTime) {
        return getDateRangeByDateType(DateRange.ThisMonth.type, firstDayOfWeek, fiscalYearStart);
    }

    const monthFirstUnixTime = getMonthFirstUnixTimeBySpecifiedUnixTime(minTime);
    const monthLastUnixTime = getMonthLastUnixTimeBySpecifiedUnixTime(minTime);
    const dateType = getDateTypeByDateRange(monthFirstUnixTime, monthLastUnixTime, firstDayOfWeek, fiscalYearStart, DateRangeScene.Normal);

    const dateRange: TimeRangeAndDateType = {
        dateType: dateType,
        maxTime: monthLastUnixTime,
        minTime: monthFirstUnixTime
    };

    return dateRange;
}

export function getCombinedDateAndTimeValues(date: Date, numeralSystem: NumeralSystem, hour: string, minute: string, second: string, meridiemIndicator: string, is24Hour: boolean): Date {
    const newDateTime = new Date(date.valueOf());
    let hours = numeralSystem.parseInt(hour);
    const minutes = numeralSystem.parseInt(minute);
    const seconds = numeralSystem.parseInt(second);

    if (!is24Hour) {
        if (hours === 12) {
            hours = 0;
        }

        if (meridiemIndicator === MeridiemIndicator.PM.name) {
            hours += 12;
        }
    }

    newDateTime.setHours(hours);
    newDateTime.setMinutes(minutes);
    newDateTime.setSeconds(seconds);

    return newDateTime;
}

export function getValidMonthDayOrCurrentDayShortDate(unixTime: number, currentShortDate: string): TextualYearMonthDay {
    const currentTime = moment();
    const monthLastTime = moment.unix(getMonthLastUnixTimeBySpecifiedUnixTime(unixTime));

    if (currentShortDate) {
        const yearMonthDay = currentShortDate.split('-');

        if (yearMonthDay.length === 3) {
            const currentDay = parseInt(yearMonthDay[2] as string);

            if (currentDay < monthLastTime.date()) {
                return MomentDateTime.of(monthLastTime.set({ date: currentDay })).getGregorianCalendarYearDashMonthDashDay();
            }
        }
    }

    if (monthLastTime.year() === currentTime.year() && monthLastTime.month() === currentTime.month()) {
        return MomentDateTime.of(currentTime).getGregorianCalendarYearDashMonthDashDay();
    }

    return MomentDateTime.of(monthLastTime).getGregorianCalendarYearDashMonthDashDay();
}

export function isDateRangeMatchFullYears(minTime: number, maxTime: number): boolean {
    const minDateTime = parseDateTimeFromUnixTime(minTime);
    const maxDateTime = parseDateTimeFromUnixTime(maxTime);
    return MomentDateTime.isGregorianCalendarYearFirstTime(minDateTime as MomentDateTime) && MomentDateTime.isGregorianCalendarYearLastTime(maxDateTime as MomentDateTime);
}

export function isDateRangeMatchFullMonths(minTime: number, maxTime: number): boolean {
    const minDateTime = parseDateTimeFromUnixTime(minTime);
    const maxDateTime = parseDateTimeFromUnixTime(maxTime);
    return MomentDateTime.isGregorianCalendarMonthFirstTime(minDateTime as MomentDateTime) && MomentDateTime.isGregorianCalendarMonthLastTime(maxDateTime as MomentDateTime);
}

export function isDateRangeMatchOneMonth(minTime: number, maxTime: number): boolean {
    const minDateTime = parseDateTimeFromUnixTime(minTime);
    const maxDateTime = parseDateTimeFromUnixTime(maxTime);

    if (minDateTime.getGregorianCalendarYear() !== maxDateTime.getGregorianCalendarYear() || minDateTime.getGregorianCalendarMonth() !== maxDateTime.getGregorianCalendarMonth()) {
        return false;
    }

    return isDateRangeMatchFullMonths(minTime, maxTime);
}
