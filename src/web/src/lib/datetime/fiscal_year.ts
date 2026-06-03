import moment from 'moment-timezone';

import {
    type FiscalYearUnixTime,
    FiscalYearStart
} from '@/core/fiscalyear.ts';

export function getFiscalYearFromUnixTime(unixTime: number, fiscalYearStartValue: number): number {
    const date = moment.unix(unixTime);

    // 若财年从 1 月 1 日开始，则财年与公历年一致
    if (fiscalYearStartValue === FiscalYearStart.JanuaryFirstDay.value) {
        return date.year();
    }

    // 获取日期组成部分
    const month = date.month() + 1; // 从 1 开始计数
    const day = date.date();
    const year = date.year();

    let fiscalYearStart = FiscalYearStart.valueOf(fiscalYearStartValue);

    if (!fiscalYearStart) {
        fiscalYearStart = FiscalYearStart.Default;
    }

    // 对于其他财年起始日：
    // 如果输入时间早于该公历年的财年起始日，
    // 则它属于在当前公历年结束的那个财年
    if (month < fiscalYearStart.month || (month === fiscalYearStart.month && day < fiscalYearStart.day)) {
        return year;
    }

    // 如果输入时间在该公历年的财年起始日当天或之后，
    // 则它属于在下一公历年结束的那个财年
    return year + 1;
}

export function getFiscalYearStartUnixTime(unixTime: number, fiscalYearStartValue: number): number {
    const date = moment.unix(unixTime);

    // 若财年从 1 月 1 日开始，则财年起点总是输入公历年的 1 月 1 日
    // 注意：这里使用宽松相等，以处理潜在的类型不匹配（string vs number）

    if (fiscalYearStartValue == FiscalYearStart.JanuaryFirstDay.value) {
        return moment().year(date.year()).month(0).date(1).hour(0).minute(0).second(0).millisecond(0).unix();
    }

    let fiscalYearStart = FiscalYearStart.valueOf(fiscalYearStartValue);

    if (!fiscalYearStart) {
        fiscalYearStart = FiscalYearStart.Default;
    }

    const month = date.month() + 1; // 从 1 开始计数
    const day = date.date();
    const year = date.year();

    // 对于其他财年起始日：
    // 如果输入时间早于该公历年的财年起始日，
    // 则对应财年的开始日期位于“输入年份”，结束日期位于“输入年份 + 1”。
    // 如果输入时间在该公历年的财年起始日当天或之后，
    // 则对应财年的开始日期位于“输入年份 - 1”，结束日期位于“输入年份”。
    let startYear = year - 1;
    if (month > fiscalYearStart.month || (month === fiscalYearStart.month && day >= fiscalYearStart.day)) {
        startYear = year;
    }

    return moment().set({
        year: startYear,
        month: fiscalYearStart.month - 1, // 从 0 开始计数
        date: fiscalYearStart.day,
        hour: 0,
        minute: 0,
        second: 0,
        millisecond: 0,
    }).unix();
}

export function getFiscalYearEndUnixTime(unixTime: number, fiscalYearStart: number): number {
    const fiscalYearStartTime = moment.unix(getFiscalYearStartUnixTime(unixTime, fiscalYearStart));
    return fiscalYearStartTime.add(1, 'years').subtract(1, 'seconds').unix();
}

export function getCurrentFiscalYear(fiscalYearStart: number): number {
    const date = moment();
    return getFiscalYearFromUnixTime(date.unix(), fiscalYearStart);
}

export function getFiscalYearTimeRangeFromUnixTime(unixTime: number, fiscalYearStart: number): FiscalYearUnixTime {
    const start = getFiscalYearStartUnixTime(unixTime, fiscalYearStart);
    const end = getFiscalYearEndUnixTime(unixTime, fiscalYearStart);
    return {
        year: getFiscalYearFromUnixTime(unixTime, fiscalYearStart),
        minUnixTime: start,
        maxUnixTime: end,
    };
}

export function getFiscalYearTimeRangeFromYear(year: number, fiscalYearStartValue: number): FiscalYearUnixTime {
    const fiscalYear = year;
    let fiscalYearStart = FiscalYearStart.valueOf(fiscalYearStartValue);

    if (!fiscalYearStart) {
        fiscalYearStart = FiscalYearStart.Default;
    }

    // 对于指定财年（例如 2023），其开始日期位于前一个公历年，
    // 除非财年起始日就是 1 月 1 日
    const calendarStartYear = fiscalYearStartValue === FiscalYearStart.JanuaryFirstDay.value ? fiscalYear : fiscalYear - 1;

    // 生成财年起始时刻的时间戳
    const fiscalYearStartUnixTime = moment().set({
        year: calendarStartYear,
        month: fiscalYearStart.month - 1, // 从 0 开始计数
        date: fiscalYearStart.day,
        hour: 0,
        minute: 0,
        second: 0,
        millisecond: 0,
    }).unix();

    // 财年结束时刻 = 财年开始后一年再减 1 秒
    const fiscalYearEndUnixTime = moment.unix(fiscalYearStartUnixTime).add(1, 'years').subtract(1, 'seconds').unix();

    return {
        year: fiscalYear,
        minUnixTime: fiscalYearStartUnixTime,
        maxUnixTime: fiscalYearEndUnixTime,
    };
}
