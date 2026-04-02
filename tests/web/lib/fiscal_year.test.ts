// 财年函数的单元测试
import fs from 'fs';
import path from 'path';
import { describe, expect, test, beforeAll } from '@jest/globals';
import moment from 'moment-timezone';

// 从库中导入所有财年相关函数
import type { TextualYearMonth } from '@/core/datetime.ts';
import { FiscalYearStart, FiscalYearUnixTime } from '@/core/fiscalyear.ts';

import {
    getFiscalYearFromUnixTime,
    getFiscalYearStartUnixTime,
    getFiscalYearEndUnixTime,
    getFiscalYearTimeRangeFromUnixTime,
    getAllFiscalYearsStartAndEndUnixTimes,
    getFiscalYearTimeRangeFromYear
} from '@/lib/datetime.ts';

// 将测试环境时区设置为 UTC，因为测试数据常量使用的是 UTC
beforeAll(() => {
    moment.tz.setDefault('UTC');
});

// 工具函数
function importTestData(datasetName: string): unknown[] {
    const data = JSON.parse(
        fs.readFileSync(path.join(__dirname, 'fiscal_year.data.json'), 'utf8')
    );
    if (!data || typeof data[datasetName] === 'undefined') {
        throw new Error(`${datasetName} is undefined or missing in the data object.`);
    }
    return data[datasetName];
}

function formatUnixTimeISO(unixTime: number): string {
    return moment.unix(unixTime).format('YYYY-MM-DDTHH:mm:ssZ');
}

function getTestTitleFormatDate(testFiscalYearStartId: string, testCaseDateString: string): string {
    return `FY_START: ${testFiscalYearStartId.padStart(10, ' ')}; DATE: ${moment(testCaseDateString).format('MMMM D, YYYY')}`;
}

function getTestTitleFormatString(testFiscalYearStartId: string, testCaseString: string): string {
    return `FY_START: ${testFiscalYearStartId.padStart(10, ' ')}; ${testCaseString}`;
}

// 财年起始配置
type FiscalYearStartConfig = {
    id: string;
    monthDateString: string;
    value: number;
};

const TEST_FISCAL_YEAR_START_PRESETS: Record<string, FiscalYearStartConfig> = {
    'January 1': {
        id: 'January 1',
        monthDateString: '01-01',
        value: 0x0101,
    },
    'April 1': {
        id: 'April 1',
        monthDateString: '04-01',
        value: 0x0401,
    },
    'October 1': {
        id: 'October 1',
        monthDateString: '10-01',
        value: 0x0A01,
    },
};

// 校验财年起始预设
describe('validateFiscalYearStart', () => {
    Object.values(TEST_FISCAL_YEAR_START_PRESETS).forEach((testFiscalYearStart) => {
        test(`should return fiscal year start object if fiscal year start value (uint16) is valid: id: ${testFiscalYearStart.id}; value: 0x${testFiscalYearStart.value.toString(16)}`, () => {
            expect(FiscalYearStart.valueOf(testFiscalYearStart.value)).toBeDefined();
        });

        test(`returns same month-date string for valid fiscal year start value: id: ${testFiscalYearStart.id}; value: 0x${testFiscalYearStart.value.toString(16)}`, () => {
            const fiscalYearStart = FiscalYearStart.valueOf(testFiscalYearStart.value);
            expect(fiscalYearStart?.toMonthDashDayString()).toStrictEqual(testFiscalYearStart.monthDateString);
        });
    });
});


// 校验无效的财年起始值
const TestCase_invalidFiscalYearValues = [
    0x0000, // 无效：L0/0
    0x0D01, // 无效：月份 13
    0x0100, // 无效：日期 0
    0x0120, // 无效：1 月 32 日
    0x021D, // 无效：2 月 29 日（不允许）
    0x021E, // 无效：2 月 30 日
    0x041F, // 无效：4 月 31 日
    0x061F, // 无效：6 月 31 日
    0x091F, // 无效：9 月 31 日
    0x0B20, // 无效：11 月 32 日
    0xFFFF, // 无效：最大的 uint16 值
]

describe('validateFiscalYearStartInvalidValues', () => {
    TestCase_invalidFiscalYearValues.forEach((testCase) => {
        test(`should return undefined if fiscal year start value (uint16) is invalid: value: 0x${testCase.toString(16)}`, () => {
            expect(FiscalYearStart.valueOf(testCase)).not.toBeDefined();
        });
    });
});

// 校验闰日 2 月 29 日不合法
describe('validateFiscalYearStartLeapDay', () => {
    test(`should return undefined if fiscal year start value (uint16) for February 29 is invalid: value: 0x0229}`, () => {
        expect(FiscalYearStart.valueOf(0x021D)).not.toBeDefined();
    });

    test(`should return undefined if fiscal year month-day string "02-29" is used to create fiscal year start object`, () => {
        expect(FiscalYearStart.parse('02-29')).not.toBeDefined();
    });
});

// 根据 UNIX 时间获取财年
type TestCase_getFiscalYearFromUnixTime = {
    date: string;
    unixTime: number;
    expected: {
        [fiscalYearStartId: string]: number;
    };
};

const TEST_CASES_GET_FISCAL_YEAR_FROM_UNIX_TIME: TestCase_getFiscalYearFromUnixTime[] =
    importTestData('test_cases_getFiscalYearFromUnixTime') as TestCase_getFiscalYearFromUnixTime[];

describe('getFiscalYearFromUnixTime', () => {
    Object.values(TEST_FISCAL_YEAR_START_PRESETS).forEach((testFiscalYearStart) => {
        TEST_CASES_GET_FISCAL_YEAR_FROM_UNIX_TIME.forEach((testCase) => {
            test(`returns correct fiscal year for ${getTestTitleFormatDate(testFiscalYearStart.id, testCase.date)}`, () => {
                const testCaseUnixTime = moment(testCase.date).unix();
                const fiscalYear = getFiscalYearFromUnixTime(testCaseUnixTime, testFiscalYearStart.value);
                const expected = testCase.expected[testFiscalYearStart.id];
                expect(fiscalYear).toBe(expected);
            });
        });
    });
});


// 财年起始 UNIX 时间
type TestCase_getFiscalYearStartUnixTime = {
    date: string;
    expected: {
        [fiscalYearStart: string]: {
            unixTime: number;
            unixTimeISO: string;
        };
    };
}

const TEST_CASES_GET_FISCAL_YEAR_START_UNIX_TIME: TestCase_getFiscalYearStartUnixTime[] =
    importTestData('test_cases_getFiscalYearStartUnixTime') as TestCase_getFiscalYearStartUnixTime[];

describe('getFiscalYearStartUnixTime', () => {
    Object.values(TEST_FISCAL_YEAR_START_PRESETS).forEach((testFiscalYearStart) => {
        TEST_CASES_GET_FISCAL_YEAR_START_UNIX_TIME.forEach((testCase) => {
            test(`returns correct start unix time for ${getTestTitleFormatDate(testFiscalYearStart.id, testCase.date)}`, () => {
                const testCaseUnixTime = moment(testCase.date).unix();
                const startUnixTime = getFiscalYearStartUnixTime(testCaseUnixTime, testFiscalYearStart.value);
                const expected = testCase.expected[testFiscalYearStart.id];
                const unixTimeISO = formatUnixTimeISO(startUnixTime);

                expect({ unixTime: startUnixTime, ISO: unixTimeISO }).toStrictEqual({ unixTime: expected!.unixTime, ISO: expected!.unixTimeISO });
            });
        });
    });
});


// 财年结束 UNIX 时间
type TestCase_getFiscalYearEndUnixTime = {
    date: string;
    expected: {
        [fiscalYearStart: string]: {
            unixTime: number;
            unixTimeISO: string;
        };
    };
}

const TEST_CASES_GET_FISCAL_YEAR_END_UNIX_TIME: TestCase_getFiscalYearEndUnixTime[] =
    importTestData('test_cases_getFiscalYearEndUnixTime') as TestCase_getFiscalYearEndUnixTime[];

describe('getFiscalYearEndUnixTime', () => {
    Object.values(TEST_FISCAL_YEAR_START_PRESETS).forEach((testFiscalYearStart) => {
        TEST_CASES_GET_FISCAL_YEAR_END_UNIX_TIME.forEach((testCase) => {
            test(`returns correct end unix time for ${getTestTitleFormatDate(testFiscalYearStart.id, testCase.date)}`, () => {
                const testCaseUnixTime = moment(testCase.date).unix();
                const endUnixTime = getFiscalYearEndUnixTime(testCaseUnixTime, testFiscalYearStart.value);
                const expected = testCase.expected[testFiscalYearStart.id];
                const unixTimeISO = formatUnixTimeISO(endUnixTime);

                expect({ unixTime: endUnixTime, ISO: unixTimeISO }).toStrictEqual({ unixTime: expected!.unixTime, ISO: expected!.unixTimeISO });

            });
        });
    });
});

// 获取财年 UNIX 时间范围
type TestCase_getFiscalYearTimeRangeFromUnixTime = {
    date: string;
    expected: {
        [fiscalYearStart: string]: FiscalYearUnixTime[]
    }
}

const TEST_CASES_GET_FISCAL_YEAR_UNIX_TIME_RANGE: TestCase_getFiscalYearTimeRangeFromUnixTime[] =
    importTestData('test_cases_getFiscalYearTimeRangeFromUnixTime') as TestCase_getFiscalYearTimeRangeFromUnixTime[];

describe('getFiscalYearTimeRangeFromUnixTime', () => {
    Object.values(TEST_FISCAL_YEAR_START_PRESETS).forEach((testFiscalYearStart) => {
        TEST_CASES_GET_FISCAL_YEAR_UNIX_TIME_RANGE.forEach((testCase) => {
            test(`returns correct fiscal year unix time range for ${getTestTitleFormatDate(testFiscalYearStart.id, testCase.date)}`, () => {
                const testCaseUnixTime = moment(testCase.date).unix();
                const fiscalYearUnixTimeRange = getFiscalYearTimeRangeFromUnixTime(testCaseUnixTime, testFiscalYearStart.value);
                expect(fiscalYearUnixTimeRange).toStrictEqual(testCase.expected[testFiscalYearStart.id]);
            });
        });
    });
});

// 获取所有财年的起止 UNIX 时间
type TestCase_getAllFiscalYearsStartAndEndUnixTimes = {
    startYearMonth: TextualYearMonth;
    endYearMonth: TextualYearMonth;
    fiscalYearStart: string;
    fiscalYearStartId: string;
    expected: FiscalYearUnixTime[]
}

const TEST_CASES_GET_ALL_FISCAL_YEARS_START_AND_END_UNIX_TIMES: TestCase_getAllFiscalYearsStartAndEndUnixTimes[] =
    importTestData('test_cases_getAllFiscalYearsStartAndEndUnixTimes') as TestCase_getAllFiscalYearsStartAndEndUnixTimes[];

describe('getAllFiscalYearsStartAndEndUnixTimes', () => {
    TEST_CASES_GET_ALL_FISCAL_YEARS_START_AND_END_UNIX_TIMES.forEach((testCase) => {
        const fiscalYearStart = FiscalYearStart.parse(testCase.fiscalYearStart);
        test(`returns correct fiscal year start and end unix times for ${getTestTitleFormatString(testCase.fiscalYearStartId, `${testCase.startYearMonth} to ${testCase.endYearMonth}`)}`, () => {
            expect(fiscalYearStart).toBeDefined();

            const fiscalYearStartAndEndUnixTimes = getAllFiscalYearsStartAndEndUnixTimes(testCase.startYearMonth, testCase.endYearMonth, fiscalYearStart?.value || 0);

            // 将结果补充 ISO 字符串，便于测试输出更直观
            const resultWithISO = fiscalYearStartAndEndUnixTimes.map(data => ({
                ...data,
                minUnixTimeISO: formatUnixTimeISO(data.minUnixTime),
                maxUnixTimeISO: formatUnixTimeISO(data.maxUnixTime)
            }));

            // 为预期结果补充 ISO 字符串
            const expectedWithISO = testCase.expected.map(data => ({
                ...data,
                minUnixTimeISO: formatUnixTimeISO(data.minUnixTime),
                maxUnixTimeISO: formatUnixTimeISO(data.maxUnixTime)
            }));

            expect(resultWithISO).toStrictEqual(expectedWithISO);
        });
    });
});

// 根据年份获取财年范围
type TestCase_getFiscalYearTimeRangeFromYear = {
    year: number;
    fiscalYearStart: string;
    expected: FiscalYearUnixTime;
}

const TEST_CASES_GET_FISCAL_YEAR_RANGE_FROM_YEAR: TestCase_getFiscalYearTimeRangeFromYear[] =
    importTestData('test_cases_getFiscalYearTimeRangeFromYear') as TestCase_getFiscalYearTimeRangeFromYear[];

describe('getFiscalYearTimeRangeFromYear', () => {
    TEST_CASES_GET_FISCAL_YEAR_RANGE_FROM_YEAR.forEach((testCase) => {
        const fiscalYearStart = FiscalYearStart.parse(testCase.fiscalYearStart);
        test(`returns correct fiscal year unix time range for input year integer ${testCase.year} and FY_START: ${testCase.fiscalYearStart}`, () => {
            expect(fiscalYearStart).toBeDefined();
            const fiscalYearRange = getFiscalYearTimeRangeFromYear(testCase.year, fiscalYearStart?.value || 0);
            expect(fiscalYearRange).toStrictEqual(testCase.expected);
        });
    });
});
