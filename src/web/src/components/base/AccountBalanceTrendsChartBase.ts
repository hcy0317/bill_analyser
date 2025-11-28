import { computed } from 'vue';

import { useI18n } from '@/locales/helpers.ts';

import {
    type UnixTimeRange,
    type YearUnixTime,
    type YearQuarterUnixTime,
    type YearMonthUnixTime,
    YearMonthDayUnixTime,
} from '@/core/datetime.ts';
import type { FiscalYearUnixTime } from '@/core/fiscalyear.ts';
import { ChartDateAggregationType } from '@/core/statistics.ts';
import type { AccountInfoResponse } from '@/models/account.ts';
import type { TransactionReconciliationStatementResponseItem } from '@/models/transaction.ts';

import { isArray } from '@/lib/common.ts';
import { sumAmounts } from '@/lib/numeral.ts';
import {
    getGregorianCalendarYearAndMonthFromUnixTime,
    getYearFirstUnixTimeBySpecifiedUnixTime,
    getQuarterFirstUnixTimeBySpecifiedUnixTime,
    getMonthFirstUnixTimeBySpecifiedUnixTime,
    getDayFirstUnixTimeBySpecifiedUnixTime,
    getAllDaysStartAndEndUnixTimes,
    getFiscalYearStartUnixTime
} from '@/lib/datetime.ts';
import { getAllDateRangesByYearMonthRange } from '@/lib/statistics.ts';

export interface AccountBalanceUnixTimeAndBalanceRange extends UnixTimeRange {
    minUnixTimeOpeningBalance: number;
    minUnixTimeClosingBalance: number;
    maxUnixTimeClosingBalance: number;
}

export interface AccountBalanceTrendsChartItem {
    displayDate: string;
    displayDateRange?: string; // 用于聚合后显示日期范围（如"11-6 ~ 11-12"）
    openingBalance: number;
    closingBalance: number;
    minimumBalance: number;
    maximumBalance: number;
    medianBalance: number;
    averageBalance: number;
}

export interface CommonAccountBalanceTrendsChartProps {
    items: TransactionReconciliationStatementResponseItem[] | undefined;
    dateAggregationType: number;
    fiscalYearStart: number;
    account: AccountInfoResponse;
}

export function useAccountBalanceTrendsChartBase(props: CommonAccountBalanceTrendsChartProps) {
    console.log(`[AccountBalanceTrendsChartBase] 初始化 - fiscalYearStart=${props.fiscalYearStart}, dateAggregationType=${props.dateAggregationType}, account=${props.account.name}, items数量=${props.items?.length || 0}`);

    // 记录前3笔交易的原始数据，验证accountOpeningBalance字段
    if (props.items && props.items.length > 0) {
        console.log(`[AccountBalanceTrendsChartBase] API返回的前3笔交易数据:`);
        for (let i = 0; i < Math.min(3, props.items.length); i++) {
            const item = props.items[i];
            if (item) {
                console.log(`  #${i + 1}: time=${item.time}, accountOpeningBalance=${item.accountOpeningBalance}, accountClosingBalance=${item.accountClosingBalance}`);
            }
        }
    }

    const {
        formatUnixTimeToShortDate,
        formatUnixTimeToGregorianLikeShortYear,
        formatUnixTimeToGregorianLikeShortYearMonth,
        formatUnixTimeToGregorianLikeYearQuarter,
        formatUnixTimeToGregorianLikeFiscalYear
    } = useI18n();

    const dataDateRange = computed<AccountBalanceUnixTimeAndBalanceRange | null>(() => {
        if (!props.items || props.items.length < 1) {
            console.log(`[AccountBalanceTrendsChartBase] dataDateRange计算 - 无数据项`);
            return null;
        }

        console.log(`[AccountBalanceTrendsChartBase] dataDateRange计算 - 开始处理${props.items.length}条交易记录`);

        let minUnixTime = Number.MAX_SAFE_INTEGER, maxUnixTime = 0;
        let minUnixTimeOpeningBalance = 0;
        let minUnixTimeClosingBalance = 0;
        let maxUnixTimeClosingBalance = 0;

        for (const item of props.items) {
            if (item.time < minUnixTime) {
                minUnixTime = item.time;
                minUnixTimeOpeningBalance = item.accountOpeningBalance;
                minUnixTimeClosingBalance = item.accountClosingBalance;
            }

            if (item.time > maxUnixTime) {
                maxUnixTime = item.time;
                maxUnixTimeClosingBalance = item.accountClosingBalance;
            }
        }

        if (minUnixTime >= Number.MAX_SAFE_INTEGER || maxUnixTime <= 0) {
            console.log(`[AccountBalanceTrendsChartBase] dataDateRange计算 - 无效的时间范围`);
            return null;
        }

        const result = {
            minUnixTime: minUnixTime,
            maxUnixTime: maxUnixTime,
            minUnixTimeOpeningBalance: minUnixTimeOpeningBalance,
            minUnixTimeClosingBalance: minUnixTimeClosingBalance,
            maxUnixTimeClosingBalance: maxUnixTimeClosingBalance
        };
        console.log(`[AccountBalanceTrendsChartBase] dataDateRange计算完成 - 时间范围: ${new Date(minUnixTime * 1000).toISOString()} ~ ${new Date(maxUnixTime * 1000).toISOString()}, 期初余额: ${minUnixTimeOpeningBalance}, 期末余额: ${maxUnixTimeClosingBalance}`);
        return result;
    });

    const allDateRanges = computed<YearUnixTime[] | FiscalYearUnixTime[] | YearQuarterUnixTime[] | YearMonthUnixTime[] | YearMonthDayUnixTime[]>(() => {
        if (!dataDateRange.value) {
            console.log(`[AccountBalanceTrendsChartBase] allDateRanges计算 - dataDateRange为空`);
            return [];
        }

        console.log(`[AccountBalanceTrendsChartBase] allDateRanges计算 - dateAggregationType=${props.dateAggregationType}, fiscalYearStart=${props.fiscalYearStart}`);

        if (props.dateAggregationType === ChartDateAggregationType.Day.type) {
            const result = getAllDaysStartAndEndUnixTimes(dataDateRange.value.minUnixTime, dataDateRange.value.maxUnixTime);
            console.log(`[AccountBalanceTrendsChartBase] allDateRanges计算完成 - 按天聚合，共${result.length}天`);
            return result;
        } else {
            const startYearMonth = getGregorianCalendarYearAndMonthFromUnixTime(dataDateRange.value.minUnixTime);
            const endYearMonth = getGregorianCalendarYearAndMonthFromUnixTime(dataDateRange.value.maxUnixTime);
            const result = getAllDateRangesByYearMonthRange(startYearMonth, endYearMonth, props.fiscalYearStart, props.dateAggregationType);
            console.log(`[AccountBalanceTrendsChartBase] allDateRanges计算完成 - 按期间聚合(type=${props.dateAggregationType})，共${result.length}个期间`);
            return result;
        }
    });

    const allDataItems = computed<AccountBalanceTrendsChartItem[]>(() => {
        const ret: AccountBalanceTrendsChartItem[] = [];

        if (!dataDateRange.value || !allDateRanges.value || allDateRanges.value.length < 1 || !props.items || props.items.length < 1) {
            console.log(`[AccountBalanceTrendsChartBase] allDataItems计算 - 缺少必要数据，返回空数组`);
            return ret;
        }

        console.log(`[AccountBalanceTrendsChartBase] allDataItems计算开始 - 处理${allDateRanges.value.length}个日期范围，${props.items.length}条交易`);

        const dayDataItemsMap: Record<number, TransactionReconciliationStatementResponseItem[]> = {};

        for (const dateItem of props.items) {
            let dateRangeMinUnixTime = 0;

            if (props.dateAggregationType === ChartDateAggregationType.Year.type) {
                dateRangeMinUnixTime = getYearFirstUnixTimeBySpecifiedUnixTime(dateItem.time);
            } else if (props.dateAggregationType === ChartDateAggregationType.FiscalYear.type) {
                dateRangeMinUnixTime = getFiscalYearStartUnixTime(dateItem.time, props.fiscalYearStart);
            } else if (props.dateAggregationType === ChartDateAggregationType.Quarter.type) {
                dateRangeMinUnixTime = getQuarterFirstUnixTimeBySpecifiedUnixTime(dateItem.time);
            } else if (props.dateAggregationType === ChartDateAggregationType.Month.type) {
                dateRangeMinUnixTime = getMonthFirstUnixTimeBySpecifiedUnixTime(dateItem.time);
            } else if (props.dateAggregationType === ChartDateAggregationType.Day.type) {
                dateRangeMinUnixTime = getDayFirstUnixTimeBySpecifiedUnixTime(dateItem.time);
            } else {
                return ret;
            }

            const dataItems: TransactionReconciliationStatementResponseItem[] = dayDataItemsMap[dateRangeMinUnixTime] || [];
            dataItems.push(dateItem);

            dayDataItemsMap[dateRangeMinUnixTime] = dataItems;
        }

        let lastOpeningBalance = dataDateRange.value.minUnixTimeOpeningBalance;
        let lastClosingBalance = dataDateRange.value.minUnixTimeClosingBalance;
        let lastMinimumBalance = lastClosingBalance;
        let lastMaximumBalance = lastClosingBalance;
        let lastMedianBalance = lastClosingBalance;
        let lastAverageBalance = lastClosingBalance;
        let consecutiveNoChangeCount = 0;  // 连续无变动天数计数器
        let aggregatedStartDate = '';  // 聚合起始日期
        let aggregatedEndDate = '';    // 聚合结束日期

        for (const dateRange of allDateRanges.value) {
            const dataItems = dayDataItemsMap[dateRange.minUnixTime];

            let displayDate = '';

            if (props.dateAggregationType === ChartDateAggregationType.Year.type) {
                displayDate = formatUnixTimeToGregorianLikeShortYear(dateRange.minUnixTime);
            } else if (props.dateAggregationType === ChartDateAggregationType.FiscalYear.type) {
                displayDate = formatUnixTimeToGregorianLikeFiscalYear(dateRange.minUnixTime);
            } else if (props.dateAggregationType === ChartDateAggregationType.Quarter.type) {
                displayDate = formatUnixTimeToGregorianLikeYearQuarter(dateRange.minUnixTime);
            } else if (props.dateAggregationType === ChartDateAggregationType.Month.type) {
                displayDate = formatUnixTimeToGregorianLikeShortYearMonth(dateRange.minUnixTime);
            } else if (props.dateAggregationType === ChartDateAggregationType.Day.type) {
                displayDate = formatUnixTimeToShortDate(dateRange.minUnixTime);
            } else {
                return ret;
            }

            if (isArray(dataItems) && dataItems.length > 0) {
                // 重置连续无变动计数器
                consecutiveNoChangeCount = 0;

                dataItems.sort(function (data1: TransactionReconciliationStatementResponseItem, data2: TransactionReconciliationStatementResponseItem) {
                    return data1.time - data2.time;
                });

                const openingBalance = dataItems[0]!.accountOpeningBalance;
                const closingBalance = dataItems[dataItems.length - 1]!.accountClosingBalance;
                const minimumBalance = Math.min(...dataItems.map(item => item.accountClosingBalance));
                const maximumBalance = Math.max(...dataItems.map(item => item.accountClosingBalance));
                const medianBalance = dataItems[Math.floor(dataItems.length / 2)]!.accountClosingBalance;
                const averageBalance = Math.trunc(sumAmounts(dataItems.map(item => item.accountClosingBalance)) / dataItems.length);

                if (props.account.isAsset) {
                    lastOpeningBalance = openingBalance;
                    lastClosingBalance = closingBalance;
                    lastMinimumBalance = minimumBalance;
                    lastMaximumBalance = maximumBalance;
                    lastMedianBalance = medianBalance;
                    lastAverageBalance = averageBalance;
                } else if (props.account.isLiability) {
                    lastOpeningBalance = -openingBalance;
                    lastClosingBalance = -closingBalance;
                    lastMinimumBalance = -minimumBalance;
                    lastMaximumBalance = -maximumBalance;
                    lastMedianBalance = -medianBalance;
                    lastAverageBalance = -averageBalance;
                } else {
                    lastOpeningBalance = openingBalance;
                    lastClosingBalance = closingBalance;
                    lastMinimumBalance = minimumBalance;
                    lastMaximumBalance = maximumBalance;
                    lastMedianBalance = medianBalance;
                    lastAverageBalance = averageBalance;
                }

                // 添加日志记录数据点
                console.log(`[AccountBalanceTrends] ${displayDate} (有变动): 开盘=${lastOpeningBalance}, 收盘=${lastClosingBalance}, 最低=${lastMinimumBalance}, 最高=${lastMaximumBalance}`);

                ret.push({
                    displayDate: displayDate,
                    openingBalance: lastOpeningBalance,
                    closingBalance: lastClosingBalance,
                    minimumBalance: lastMinimumBalance,
                    maximumBalance: lastMaximumBalance,
                    medianBalance: lastMedianBalance,
                    averageBalance: lastAverageBalance
                });
            } else if (props.dateAggregationType === ChartDateAggregationType.Day.type) {
                // If there is no data for this day, use the last closing balance
                // 优化：聚合连续无变动的天到一个点
                consecutiveNoChangeCount++;

                // 只保留第一个无变动天的数据点
                if (consecutiveNoChangeCount === 1) {
                    aggregatedStartDate = displayDate;  // 记录聚合起始日期
                    aggregatedEndDate = displayDate;    // 初始化为起始日期
                    console.log(`[AccountBalanceTrends] ${displayDate} (无变动): 余额=${lastClosingBalance}`);
                    ret.push({
                        displayDate: displayDate,
                        displayDateRange: displayDate,  // 初始只有一天
                        openingBalance: lastClosingBalance,
                        closingBalance: lastClosingBalance,
                        minimumBalance: lastClosingBalance,
                        maximumBalance: lastClosingBalance,
                        medianBalance: lastClosingBalance,
                        averageBalance: lastClosingBalance
                    });
                } else {
                    // 更新聚合结束日期和displayDateRange
                    aggregatedEndDate = displayDate;
                    const lastItem = ret[ret.length - 1];
                    if (lastItem && consecutiveNoChangeCount > 1) {
                        // 更新最后一个数据点的日期范围显示
                        lastItem.displayDateRange = `${aggregatedStartDate} ~ ${aggregatedEndDate}`;
                        console.log(`[AccountBalanceTrends] ${displayDate} (无变动，更新范围): ${lastItem.displayDateRange}, 连续无变动天数=${consecutiveNoChangeCount}`);
                    } else {
                        console.log(`[AccountBalanceTrends] ${displayDate} (无变动，跳过): 连续无变动天数=${consecutiveNoChangeCount}`);
                    }
                }
            }
        }

        console.log(`[AccountBalanceTrendsChartBase] allDataItems计算完成 - 生成${ret.length}个数据点`);
        return ret;
    });

    const allDisplayDateRanges = computed<string[]>(() => {
        if (!allDataItems.value || allDataItems.value.length < 1) {
            return [];
        }

        // 优先使用displayDateRange（聚合后的日期范围），如果不存在则使用displayDate
        return allDataItems.value.map(item => item.displayDateRange || item.displayDate);
    });

    return {
        // computed states
        allDateRanges,
        allDataItems,
        allDisplayDateRanges
    };
}
