import { ref, computed } from 'vue';
import { defineStore } from 'pinia';

import { useI18n } from '@/locales/helpers.ts';

import { useSettingsStore } from './setting.ts';
import { useUserStore } from './user.ts';
import { useAccountsStore } from './account.ts';
import { useTransactionCategoriesStore } from './transactionCategory.ts';
import { useExchangeRatesStore } from './exchangeRates.ts';

import { entries, values } from '@/core/base.ts';
import { type DateTime, type TimeRangeAndDateType, DateRange, DateRangeScene } from '@/core/datetime.ts';
import { TimezoneTypeForStatistics } from '@/core/timezone.ts';
import { CategoryType } from '@/core/category.ts';
import {
    TransactionRelatedAccountType,
    TransactionTagFilterType
} from '@/core/transaction.ts';
import {
    StatisticsAnalysisType,
    CategoricalChartType,
    TrendChartType,
    ChartDataType,
    ChartSortingType,
    DEFAULT_CATEGORICAL_CHART_DATA_RANGE,
    DEFAULT_TREND_CHART_DATA_RANGE,
    DEFAULT_ASSET_TRENDS_CHART_DATA_RANGE
} from '@/core/statistics.ts';
import { DEFAULT_ACCOUNT_ICON, DEFAULT_CATEGORY_ICON } from '@/consts/icon.ts';
import { DEFAULT_ACCOUNT_COLOR, DEFAULT_CATEGORY_COLOR, DEFAULT_CHART_COLORS } from '@/consts/color.ts';

import {
    type TransactionStatisticResponse,
    type TransactionStatisticResponseItem,
    type TransactionStatisticTrendsResponseItem,
    type TransactionStatisticAssetTrendsResponseItem,
    type TransactionStatisticAssetTrendsResponseDataItem,
    type TransactionStatisticResponseItemWithInfo,
    type TransactionStatisticResponseWithInfo,
    type TransactionStatisticTrendsResponseItemWithInfo,
    type TransactionStatisticAssetTrendsResponseItemWithInfo,
    type TransactionStatisticDataItemBase,
    type TransactionCategoricalOverviewAnalysisData,
    type TransactionCategoricalOverviewAnalysisDataItem,
    type TransactionCategoricalAnalysisData,
    type TransactionCategoricalAnalysisDataItem,
    type TransactionTrendsAnalysisData,
    type TransactionTrendsAnalysisDataItem,
    type TransactionAssetTrendsAnalysisData,
    type TransactionAssetTrendsAnalysisDataItem,
    type TransactionAssetTrendsAnalysisDataAmount,
    TransactionCategoricalOverviewAnalysisDataItemType
} from '@/models/transaction.ts';

import {
    isEquals,
    isNumber,
    isString,
    isObject,
    isInteger,
    isYearMonth,
    isYearMonthEquals
} from '@/lib/common.ts';
import {
    getYearMonthDayDateTime,
    getGregorianCalendarYearAndMonthFromUnixTime,
    getDayDifference,
    getDateRangeByDateType
} from '@/lib/datetime.ts';
import { sortStatisticsItems } from '@/lib/statistics.ts';
import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';
import type {
    TransactionStatisticsFilter,
    TransactionStatisticsPartialFilter,
    WritableTransactionAssetTrendsAnalysisDataItem,
    WritableTransactionCategoricalAnalysisData,
    WritableTransactionCategoricalAnalysisDataItem,
    WritableTransactionTrendsAnalysisDataItem
} from './statistics/types.ts';
import {
    buildTransactionListPageParams,
    buildTransactionStatisticsPageParams
} from './statistics/pageParams.ts';

export type {
    TransactionStatisticsFilter,
    TransactionStatisticsPartialFilter
} from './statistics/types.ts';

export const useStatisticsStore = defineStore('statistics', () => {
    const { tt } = useI18n();

    const settingsStore = useSettingsStore();
    const userStore = useUserStore();
    const accountsStore = useAccountsStore();
    const transactionCategoriesStore = useTransactionCategoriesStore();
    const exchangeRatesStore = useExchangeRatesStore();

    const transactionStatisticsFilter = ref<TransactionStatisticsFilter>({
        chartDataType: ChartDataType.Default.type,
        categoricalChartType: CategoricalChartType.Default.type,
        categoricalChartDateType: DEFAULT_CATEGORICAL_CHART_DATA_RANGE.type,
        categoricalChartStartTime: 0,
        categoricalChartEndTime: 0,
        trendChartType: TrendChartType.Default.type,
        trendChartDateType: DEFAULT_TREND_CHART_DATA_RANGE.type,
        trendChartStartYearMonth: '',
        trendChartEndYearMonth: '',
        assetTrendsChartType: TrendChartType.Default.type,
        assetTrendsChartDateType: DEFAULT_ASSET_TRENDS_CHART_DATA_RANGE.type,
        assetTrendsChartStartTime: 0,
        assetTrendsChartEndTime: 0,
        filterAccountIds: {},
        filterCategoryIds: {},
        tagIds: '',
        tagFilterType: TransactionTagFilterType.Default.type,
        keyword: '',
        sortingType: ChartSortingType.Default.type
    });

    const transactionCategoryStatisticsData = ref<TransactionStatisticResponse | null>(null);
    const transactionCategoryTrendsData = ref<TransactionStatisticTrendsResponseItem[]>([]);
    const transactionAssetTrendsData = ref<TransactionStatisticAssetTrendsResponseItem[]>([]);
    const transactionStatisticsStateInvalid = ref<boolean>(true);

    const categoricalAnalysisChartDataCategory = computed<string>(() => {
        if (transactionStatisticsFilter.value.chartDataType === ChartDataType.OutflowsByAccount.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.ExpenseByAccount.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.InflowsByAccount.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.IncomeByAccount.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.AccountTotalAssets.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.AccountTotalLiabilities.type) {
            return 'account';
        } else if (transactionStatisticsFilter.value.chartDataType === ChartDataType.ExpenseByPrimaryCategory.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.ExpenseBySecondaryCategory.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.IncomeByPrimaryCategory.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.IncomeBySecondaryCategory.type) {
            return 'category';
        } else {
            return '';
        }
    });

    const transactionCategoryStatisticsDataWithCategoryAndAccountInfo = computed<TransactionStatisticResponseWithInfo | null>(() => {
        const statistics = transactionCategoryStatisticsData.value;

        if (!statistics) {
            return null;
        }

        const finalStatistics: TransactionStatisticResponseWithInfo = {
            startTime: statistics.startTime,
            endTime: statistics.endTime,
            items: []
        };

        if (statistics && statistics.items && statistics.items.length) {
            finalStatistics.items.push(...assembleAccountAndCategoryInfo(statistics.items));
        }

        return finalStatistics;
    });

    const transactionCategoryTotalAmountAnalysisData = computed<WritableTransactionCategoricalAnalysisData | null>(() => {
        if (!transactionCategoryStatisticsDataWithCategoryAndAccountInfo.value || !transactionCategoryStatisticsDataWithCategoryAndAccountInfo.value.items) {
            return null;
        }

        return getCategoryTotalAmountItems(transactionCategoryStatisticsDataWithCategoryAndAccountInfo.value.items, transactionStatisticsFilter.value);
    });

    const categoricalOverviewAnalysisData = computed<TransactionCategoricalOverviewAnalysisData | null>(() => {
        if (!transactionCategoryStatisticsDataWithCategoryAndAccountInfo.value || !transactionCategoryStatisticsDataWithCategoryAndAccountInfo.value.items) {
            return null;
        }

        const allDataItemsMap: Record<string, TransactionCategoricalOverviewAnalysisDataItem> = {};
        const allIncomeByPrimaryCategoryDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [];
        const allIncomeBySecondaryCategoryDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [];
        const allIncomeByAccountDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [];
        const allExpenseByAccountDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [];
        const allExpenseBySecondaryCategoryDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [];
        const allExpenseByPrimaryCategoryDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [];
        const allOpeningBalanceDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [];
        const allNetCashFlowDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [];

        let totalIncomeCents: number = 0;
        let totalExpenseCents: number = 0;

        for (const item of transactionCategoryStatisticsDataWithCategoryAndAccountInfo.value.items) {
            if (!item.primaryAccount || !item.account || !item.primaryCategory || !item.category) {
                continue;
            }

            if (item.relatedAccount && item.relatedAccountType === TransactionRelatedAccountType.TransferFrom) {
                continue;
            }

            if (!isNumber(item.amountInDefaultCurrencyCents)) {
                continue;
            }

            if (transactionStatisticsFilter.value.filterAccountIds && transactionStatisticsFilter.value.filterAccountIds[item.account.id]) {
                continue;
            }

            if (transactionStatisticsFilter.value.filterCategoryIds && transactionStatisticsFilter.value.filterCategoryIds[item.category.id]) {
                continue;
            }

            if (item.category.type === CategoryType.Income) {
                totalIncomeCents += item.amountInDefaultCurrencyCents;
            } else if (item.category.type === CategoryType.Expense) {
                totalExpenseCents += item.amountInDefaultCurrencyCents;
            }

            const incomeByAccountKey = `${TransactionCategoricalOverviewAnalysisDataItemType.IncomeByAccount}:${item.account.id}`;
            const expenseByAccountKey = `${TransactionCategoricalOverviewAnalysisDataItemType.ExpenseByAccount}:${item.account.id}`;
            let incomeByAccountItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[incomeByAccountKey];
            let expenseByAccountItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[expenseByAccountKey];

            if (!incomeByAccountItem) {
                incomeByAccountItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                    item.account.id,
                    item.account.name,
                    TransactionCategoricalOverviewAnalysisDataItemType.IncomeByAccount,
                    [item.primaryAccount.category, item.primaryAccount.displayOrder, item.account.displayOrder],
                    item.primaryAccount.hidden || item.account.hidden);
                allDataItemsMap[incomeByAccountKey] = incomeByAccountItem;
                allIncomeByAccountDataItems.push(incomeByAccountItem);
            }

            if (!expenseByAccountItem) {
                expenseByAccountItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                    item.account.id,
                    item.account.name,
                    TransactionCategoricalOverviewAnalysisDataItemType.ExpenseByAccount,
                    [item.primaryAccount.category, item.primaryAccount.displayOrder, item.account.displayOrder],
                    item.primaryAccount.hidden || item.account.hidden);
                allDataItemsMap[expenseByAccountKey] = expenseByAccountItem;
                allExpenseByAccountDataItems.push(expenseByAccountItem);
            }

            if (item.category.type === CategoryType.Income) {
                const primaryCategoryItemKey = `${TransactionCategoricalOverviewAnalysisDataItemType.IncomeByPrimaryCategory}:${item.primaryCategory.id}`;
                const secondaryCategoryItemKey = `${TransactionCategoricalOverviewAnalysisDataItemType.IncomeBySecondaryCategory}:${item.category.id}`;

                let primaryCategoryDataItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[primaryCategoryItemKey];
                let secondaryCategoryDataItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[secondaryCategoryItemKey];

                if (!primaryCategoryDataItem) {
                    primaryCategoryDataItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                        item.primaryCategory.id,
                        item.primaryCategory.name,
                        TransactionCategoricalOverviewAnalysisDataItemType.IncomeByPrimaryCategory,
                        [item.primaryCategory.displayOrder],
                        item.primaryCategory.hidden);
                    allDataItemsMap[primaryCategoryItemKey] = primaryCategoryDataItem;
                    allIncomeByPrimaryCategoryDataItems.push(primaryCategoryDataItem);
                }

                if (!secondaryCategoryDataItem) {
                    secondaryCategoryDataItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                        item.category.id,
                        item.category.name,
                        TransactionCategoricalOverviewAnalysisDataItemType.IncomeBySecondaryCategory,
                        [item.primaryCategory.displayOrder, item.category.displayOrder],
                        item.primaryCategory.hidden || item.category.hidden);
                    allDataItemsMap[secondaryCategoryItemKey] = secondaryCategoryDataItem;
                    allIncomeBySecondaryCategoryDataItems.push(secondaryCategoryDataItem);
                }

                primaryCategoryDataItem.totalAmountCents += item.amountInDefaultCurrencyCents;
                primaryCategoryDataItem.totalNonNegativeAmountCents += item.amountInDefaultCurrencyCents > 0 ? item.amountInDefaultCurrencyCents : 0;
                primaryCategoryDataItem.includeInPercent = true;
                primaryCategoryDataItem.outflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: secondaryCategoryDataItem });

                secondaryCategoryDataItem.totalAmountCents += item.amountInDefaultCurrencyCents;
                secondaryCategoryDataItem.totalNonNegativeAmountCents += item.amountInDefaultCurrencyCents > 0 ? item.amountInDefaultCurrencyCents : 0;
                secondaryCategoryDataItem.includeInPercent = true;
                secondaryCategoryDataItem.inflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: primaryCategoryDataItem });
                secondaryCategoryDataItem.outflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: incomeByAccountItem });

                incomeByAccountItem.totalAmountCents += item.amountInDefaultCurrencyCents;
                incomeByAccountItem.totalNonNegativeAmountCents += item.amountInDefaultCurrencyCents > 0 ? item.amountInDefaultCurrencyCents : 0;
                incomeByAccountItem.includeInPercent = true;
                incomeByAccountItem.inflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: secondaryCategoryDataItem });
            } else if (item.category.type === CategoryType.Expense) {
                const primaryCategoryItemKey = `${TransactionCategoricalOverviewAnalysisDataItemType.ExpenseByPrimaryCategory}:${item.primaryCategory.id}`;
                const secondaryCategoryItemKey = `${TransactionCategoricalOverviewAnalysisDataItemType.ExpenseBySecondaryCategory}:${item.category.id}`;

                let primaryCategoryDataItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[primaryCategoryItemKey];
                let secondaryCategoryDataItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[secondaryCategoryItemKey];

                if (!primaryCategoryDataItem) {
                    primaryCategoryDataItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                        item.primaryCategory.id,
                        item.primaryCategory.name,
                        TransactionCategoricalOverviewAnalysisDataItemType.ExpenseByPrimaryCategory,
                        [item.primaryCategory.displayOrder],
                        item.primaryCategory.hidden);
                    allDataItemsMap[primaryCategoryItemKey] = primaryCategoryDataItem;
                    allExpenseByPrimaryCategoryDataItems.push(primaryCategoryDataItem);
                }

                if (!secondaryCategoryDataItem) {
                    secondaryCategoryDataItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                        item.category.id,
                        item.category.name,
                        TransactionCategoricalOverviewAnalysisDataItemType.ExpenseBySecondaryCategory,
                        [item.primaryCategory.displayOrder, item.category.displayOrder],
                        item.primaryCategory.hidden || item.category.hidden);
                    allDataItemsMap[secondaryCategoryItemKey] = secondaryCategoryDataItem;
                    allExpenseBySecondaryCategoryDataItems.push(secondaryCategoryDataItem);
                }

                expenseByAccountItem.totalAmountCents += item.amountInDefaultCurrencyCents;
                expenseByAccountItem.totalNonNegativeAmountCents += Math.abs(item.amountInDefaultCurrencyCents);
                expenseByAccountItem.includeInPercent = true;
                expenseByAccountItem.outflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: secondaryCategoryDataItem });

                secondaryCategoryDataItem.totalAmountCents += item.amountInDefaultCurrencyCents;
                secondaryCategoryDataItem.totalNonNegativeAmountCents += Math.abs(item.amountInDefaultCurrencyCents);
                secondaryCategoryDataItem.includeInPercent = true;
                secondaryCategoryDataItem.inflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: expenseByAccountItem });
                secondaryCategoryDataItem.outflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: primaryCategoryDataItem });

                primaryCategoryDataItem.totalAmountCents += item.amountInDefaultCurrencyCents;
                primaryCategoryDataItem.totalNonNegativeAmountCents += Math.abs(item.amountInDefaultCurrencyCents);
                primaryCategoryDataItem.includeInPercent = true;
                primaryCategoryDataItem.inflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: secondaryCategoryDataItem });
            } else if (item.category.type === CategoryType.Transfer && item.relatedPrimaryAccount && item.relatedAccount) {
                const transferToAccountKey = `${TransactionCategoricalOverviewAnalysisDataItemType.ExpenseByAccount}:${item.relatedAccount.id}`;
                let transferToAccountItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[transferToAccountKey];

                if (!transferToAccountItem) {
                    transferToAccountItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                        item.relatedAccount.id,
                        item.relatedAccount.name,
                        TransactionCategoricalOverviewAnalysisDataItemType.ExpenseByAccount,
                        [item.relatedPrimaryAccount.category, item.relatedPrimaryAccount.displayOrder, item.relatedAccount.displayOrder],
                        item.relatedPrimaryAccount.hidden || item.relatedAccount.hidden);
                    allDataItemsMap[transferToAccountKey] = transferToAccountItem;
                    allExpenseByAccountDataItems.push(transferToAccountItem);
                }

                incomeByAccountItem.outflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: transferToAccountItem });
                transferToAccountItem.inflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: incomeByAccountItem });
            } else if (item.category.type === CategoryType.Investment && item.relatedPrimaryAccount && item.relatedAccount) {
                // v6.70: 投资类型处理 - 与转账类似，从源账户流出到投资账户
                const investmentToAccountKey = `${TransactionCategoricalOverviewAnalysisDataItemType.ExpenseByAccount}:${item.relatedAccount.id}`;
                let investmentToAccountItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[investmentToAccountKey];

                if (!investmentToAccountItem) {
                    investmentToAccountItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                        item.relatedAccount.id,
                        item.relatedAccount.name,
                        TransactionCategoricalOverviewAnalysisDataItemType.ExpenseByAccount,
                        [item.relatedPrimaryAccount.category, item.relatedPrimaryAccount.displayOrder, item.relatedAccount.displayOrder],
                        item.relatedPrimaryAccount.hidden || item.relatedAccount.hidden);
                    allDataItemsMap[investmentToAccountKey] = investmentToAccountItem;
                    allExpenseByAccountDataItems.push(investmentToAccountItem);
                }

                incomeByAccountItem.outflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: investmentToAccountItem });
                investmentToAccountItem.inflows.push({ amountCents: item.amountInDefaultCurrencyCents, relatedItem: incomeByAccountItem });
            }
        }

        sortCategoricalOverviewAnalysisDataItems(allIncomeByPrimaryCategoryDataItems, transactionStatisticsFilter.value);
        sortCategoricalOverviewAnalysisDataItems(allIncomeBySecondaryCategoryDataItems, transactionStatisticsFilter.value);
        sortCategoricalOverviewAnalysisDataItems(allIncomeByAccountDataItems, transactionStatisticsFilter.value);
        sortCategoricalOverviewAnalysisDataItems(allExpenseByAccountDataItems, transactionStatisticsFilter.value);
        sortCategoricalOverviewAnalysisDataItems(allExpenseBySecondaryCategoryDataItems, transactionStatisticsFilter.value);
        sortCategoricalOverviewAnalysisDataItems(allExpenseByPrimaryCategoryDataItems, transactionStatisticsFilter.value);

        for (const item of allExpenseByAccountDataItems) {
            const incomeByAccountKey = `${TransactionCategoricalOverviewAnalysisDataItemType.IncomeByAccount}:${item.id}`;
            const incomeByAccountItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[incomeByAccountKey];

            let accountTotalInflowsAmount: number = 0;
            let accountTotalIncomeAmount: number = 0;
            let accountTotalTransferAmount: number = 0;
            let accountTotalOutflowsAmount: number = 0;

            if (incomeByAccountItem) {
                for (const inflow of incomeByAccountItem.inflows) {
                    accountTotalInflowsAmount += inflow.amountCents;
                    accountTotalIncomeAmount += inflow.amountCents;
                }

                for (const outflow of incomeByAccountItem.outflows) {
                    accountTotalTransferAmount += outflow.amountCents;
                }
            }

            for (const inflow of item.inflows) {
                if (inflow.relatedItem.type === item.type && inflow.relatedItem.id === item.id) {
                    continue;
                }

                accountTotalInflowsAmount += inflow.amountCents;
            }

            for (const outflow of item.outflows) {
                accountTotalOutflowsAmount += outflow.amountCents;
            }

            const accountBalance: number = accountTotalIncomeAmount - accountTotalTransferAmount - accountTotalOutflowsAmount;
            const accountNetCashFlow: number = accountTotalInflowsAmount - accountTotalTransferAmount - accountTotalOutflowsAmount;

            if (incomeByAccountItem && accountsStore.allAccountsMap[item.id]?.isAsset) {
                if (accountBalance > 0) { // has positive balance, transfer the amount from income account to expense account
                    incomeByAccountItem.outflows.push({ amountCents: accountBalance + accountTotalOutflowsAmount, relatedItem: item });
                    item.inflows.push({ amountCents: accountBalance + accountTotalOutflowsAmount, relatedItem: incomeByAccountItem });
                } else if (accountNetCashFlow < 0) { // has negative net cash flow, add the difference to income account
                    incomeByAccountItem.totalAmountCents += -accountNetCashFlow;
                    incomeByAccountItem.totalNonNegativeAmountCents += -accountNetCashFlow > 0 ? -accountNetCashFlow : 0;
                    incomeByAccountItem.outflows.push({ amountCents: -accountNetCashFlow, relatedItem: item });
                    item.inflows.push({ amountCents: -accountNetCashFlow, relatedItem: incomeByAccountItem });
                }
            }

            if (accountNetCashFlow > 0) {
                let netCashFlowItem: TransactionCategoricalOverviewAnalysisDataItem | undefined = allDataItemsMap[TransactionCategoricalOverviewAnalysisDataItemType.NetCashFlow];

                if (!netCashFlowItem) {
                    netCashFlowItem = createNewTransactionCategoricalOverviewAnalysisDataItem(
                        TransactionCategoricalOverviewAnalysisDataItemType.NetCashFlow,
                        'Net Cash Flow',
                        TransactionCategoricalOverviewAnalysisDataItemType.NetCashFlow,
                        [Number.MAX_SAFE_INTEGER],
                        false);
                    allDataItemsMap[TransactionCategoricalOverviewAnalysisDataItemType.NetCashFlow] = netCashFlowItem;
                    allNetCashFlowDataItems.push(netCashFlowItem);
                }

                item.outflows.push({ amountCents: accountNetCashFlow, relatedItem: netCashFlowItem });

                netCashFlowItem.totalAmountCents += accountNetCashFlow;
                netCashFlowItem.totalNonNegativeAmountCents += accountNetCashFlow > 0 ? accountNetCashFlow : 0;
                netCashFlowItem.inflows.push({ amountCents: accountNetCashFlow, relatedItem: item });
            }
        }

        const allDataItems: TransactionCategoricalOverviewAnalysisDataItem[] = [
            ...allIncomeByPrimaryCategoryDataItems,
            ...allIncomeBySecondaryCategoryDataItems,
            ...allIncomeByAccountDataItems,
            ...allOpeningBalanceDataItems,
            ...allExpenseByAccountDataItems,
            ...allExpenseBySecondaryCategoryDataItems,
            ...allNetCashFlowDataItems,
            ...allExpenseByPrimaryCategoryDataItems
        ];

        return {
            totalIncomeCents: totalIncomeCents,
            totalExpenseCents: totalExpenseCents,
            items: allDataItems
        };
    });

    const accountTotalAmountAnalysisData = computed<WritableTransactionCategoricalAnalysisData | null>(() => {
        if (!accountsStore.allPlainAccounts) {
            return null;
        }

        const allDataItems: Record<string, WritableTransactionCategoricalAnalysisDataItem> = {};
        let totalAmountCents = 0;
        let totalNonNegativeAmountCents = 0;

        for (const account of accountsStore.allPlainAccounts) {
            if (transactionStatisticsFilter.value.chartDataType === ChartDataType.AccountTotalAssets.type) {
                if (!account.isAsset) {
                    continue;
                }
            } else if (transactionStatisticsFilter.value.chartDataType === ChartDataType.AccountTotalLiabilities.type) {
                if (!account.isLiability) {
                    continue;
                }
            }

            if (transactionStatisticsFilter.value.filterAccountIds && transactionStatisticsFilter.value.filterAccountIds[account.id]) {
                continue;
            }

            let primaryAccount = accountsStore.allAccountsMap[account.parentId];

            if (!primaryAccount) {
                primaryAccount = account;
            }

            let amount = account.balanceCents;

            if (account.currency !== userStore.currentUserDefaultCurrency) {
                const finalAmount = exchangeRatesStore.getExchangedAmount(amount, account.currency, userStore.currentUserDefaultCurrency);

                if (!isNumber(finalAmount)) {
                    continue;
                }

                amount = Math.trunc(finalAmount);
            }

            if (account.isLiability) {
                amount = -amount;
            }

            const data: WritableTransactionCategoricalAnalysisDataItem = {
                name: account.name,
                type: 'account',
                id: account.id,
                icon: account.icon || DEFAULT_ACCOUNT_ICON.icon,
                color: account.color || DEFAULT_ACCOUNT_COLOR,
                hidden: primaryAccount.hidden || account.hidden,
                displayOrders: [primaryAccount.category, primaryAccount.displayOrder, account.displayOrder],
                totalAmountCents: amount
            };

            totalAmountCents += amount;

            if (amount > 0) {
                totalNonNegativeAmountCents += amount;
            }

            allDataItems[account.id] = data;
        }

        return {
            totalAmountCents: totalAmountCents,
            totalNonNegativeAmountCents: totalNonNegativeAmountCents,
            items: allDataItems
        };
    });

    const categoricalAnalysisData = computed<TransactionCategoricalAnalysisData>(() => {
        let combinedData: WritableTransactionCategoricalAnalysisData | null = null;

        if (transactionStatisticsFilter.value.chartDataType === ChartDataType.OutflowsByAccount.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.ExpenseByAccount.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.ExpenseByPrimaryCategory.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.ExpenseBySecondaryCategory.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.InflowsByAccount.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.IncomeByAccount.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.IncomeByPrimaryCategory.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.IncomeBySecondaryCategory.type) {
            combinedData = transactionCategoryTotalAmountAnalysisData.value;
        } else if (transactionStatisticsFilter.value.chartDataType === ChartDataType.AccountTotalAssets.type ||
            transactionStatisticsFilter.value.chartDataType === ChartDataType.AccountTotalLiabilities.type) {
            combinedData = accountTotalAmountAnalysisData.value;
        }

        const allStatisticsItems: TransactionCategoricalAnalysisDataItem[] = [];

        if (combinedData && combinedData.items) {
            let maxTotalAmountCents = 0;

            for (const dataItem of values(combinedData.items)) {
                if (Math.abs(dataItem.totalAmountCents) > maxTotalAmountCents) {
                    maxTotalAmountCents = Math.abs(dataItem.totalAmountCents);
                }
            }

            for (const dataItem of values(combinedData.items)) {
                let percent = 0;

                if (transactionStatisticsFilter.value.chartDataType === ChartDataType.OutflowsByAccount.type ||
                    transactionStatisticsFilter.value.chartDataType === ChartDataType.InflowsByAccount.type) {
                    if (maxTotalAmountCents > 0) {
                        percent = Math.abs(dataItem.totalAmountCents) * 100 / maxTotalAmountCents;
                    } else {
                        percent = 0;
                    }
                } else {
                    if (combinedData.totalNonNegativeAmountCents > 0) {
                        percent = Math.abs(dataItem.totalAmountCents) * 100 / combinedData.totalNonNegativeAmountCents;
                    } else {
                        percent = 0;
                    }
                }

                if (percent < 0) {
                    percent = 0;
                }

                const statisticDataItem: TransactionCategoricalAnalysisDataItem = {
                    name: dataItem.name,
                    type: dataItem.type,
                    id: dataItem.id,
                    icon: dataItem.icon,
                    color: dataItem.color,
                    hidden: dataItem.hidden,
                    displayOrders: dataItem.displayOrders,
                    totalAmountCents: dataItem.totalAmountCents,
                    percent: percent
                };

                allStatisticsItems.push(statisticDataItem);
            }
        }

        sortCategoryTotalAmountItems(allStatisticsItems, transactionStatisticsFilter.value);

        const statisticData: TransactionCategoricalAnalysisData = {
            totalAmountCents: combinedData?.totalAmountCents || 0,
            items: allStatisticsItems
        };

        return statisticData;
    });

    const transactionCategoryTrendsDataWithCategoryAndAccountInfo = computed<TransactionStatisticTrendsResponseItemWithInfo[]>(() => {
        const trendsData = transactionCategoryTrendsData.value;
        const finalTrendsData: TransactionStatisticTrendsResponseItemWithInfo[] = [];

        if (trendsData && trendsData.length) {
            for (const trendItem of trendsData) {
                const finalTrendItem: TransactionStatisticTrendsResponseItemWithInfo = {
                    year: trendItem.year,
                    month: trendItem.month,
                    items: []
                };

                if (trendItem && trendItem.items && trendItem.items.length) {
                    finalTrendItem.items.push(...assembleAccountAndCategoryInfo(trendItem.items));
                }

                finalTrendsData.push(finalTrendItem);
            }
        }

        return finalTrendsData;
    });

    const trendsAnalysisData = computed<TransactionTrendsAnalysisData | null>(() => {
        if (!transactionCategoryTrendsDataWithCategoryAndAccountInfo.value || !transactionCategoryTrendsDataWithCategoryAndAccountInfo.value.length) {
            return null;
        }

        const combinedDataMap: Record<string, WritableTransactionTrendsAnalysisDataItem> = {};

        for (const trendItem of transactionCategoryTrendsDataWithCategoryAndAccountInfo.value) {
            const totalAmountItems = getCategoryTotalAmountItems(trendItem.items, transactionStatisticsFilter.value);

            for (const [id, item] of entries(totalAmountItems.items)) {
                let combinedData = combinedDataMap[id];

                if (!combinedData) {
                    combinedData = {
                        name: item.name,
                        type: item.type,
                        id: item.id,
                        icon: item.icon,
                        color: item.color,
                        hidden: item.hidden,
                        displayOrders: item.displayOrders,
                        totalAmountCents: 0,
                        items: []
                    };
                }

                combinedData.items.push({
                    year: trendItem.year,
                    month1base: trendItem.month,
                    totalAmountCents: item.totalAmountCents
                });

                combinedData.totalAmountCents += item.totalAmountCents;
                combinedDataMap[id] = combinedData;
            }
        }

        const totalAmountsTrends: TransactionTrendsAnalysisDataItem[] = [];

        for (const trendData of values(combinedDataMap)) {
            totalAmountsTrends.push(trendData);
        }

        sortCategoryTotalAmountItems(totalAmountsTrends, transactionStatisticsFilter.value);

        const trendsData: TransactionTrendsAnalysisData = {
            items: totalAmountsTrends
        };

        return trendsData;
    });

    const assetTrendsDataWithAccountInfo = computed<TransactionStatisticAssetTrendsResponseItemWithInfo[]>(() => {
        const assetTrendsData = transactionAssetTrendsData.value;
        const finalAssetTrendsData: TransactionStatisticAssetTrendsResponseItemWithInfo[] = [];

        if (!assetTrendsData || !assetTrendsData.length) {
            return finalAssetTrendsData;
        }

        const firstAssetTrendItem: TransactionStatisticAssetTrendsResponseItem | undefined = assetTrendsData[0];

        if (!firstAssetTrendItem) {
            return finalAssetTrendsData;
        }

        const lastAssetTrendItemMap: Record<string, TransactionStatisticAssetTrendsResponseDataItem> = {};
        let lastAssetTrendItem: TransactionStatisticAssetTrendsResponseItem = firstAssetTrendItem;

        for (const item of firstAssetTrendItem.items) {
            lastAssetTrendItemMap[item.accountId] = item;
        }

        for (const assetTrendItem of assetTrendsData) {
            const statisticResponseItems: TransactionStatisticResponseItem[] = [];
            const existedAccountIds: Record<string, boolean> = {};
            const missingDays: number = getDayDifference(lastAssetTrendItem, assetTrendItem) - 1;
            const lastAssetTrendItemDate: DateTime = getYearMonthDayDateTime(lastAssetTrendItem.year, lastAssetTrendItem.month, lastAssetTrendItem.day);

            // 用已知的上一笔余额补齐缺失日期
            for (let i = 1; i <= missingDays; i++) {
                const missingStatisticResponseItems: TransactionStatisticResponseItem[] = [];
                const dateTime: DateTime = lastAssetTrendItemDate.getDateTimeAfterDays(i);

                for (const item of values(lastAssetTrendItemMap)) {
                    const statisticResponseItem: TransactionStatisticResponseItem = {
                        categoryId: '',
                        accountId: item.accountId,
                        amountCents: item.accountClosingBalanceCents,
                        openingAmountCents: item.accountClosingBalanceCents
                    };

                    missingStatisticResponseItems.push(statisticResponseItem);
                }

                const finalAssetTrendItem: TransactionStatisticAssetTrendsResponseItemWithInfo = {
                    year: dateTime.getGregorianCalendarYear(),
                    month: dateTime.getGregorianCalendarMonth(),
                    day: dateTime.getGregorianCalendarDay(),
                    items: assembleAccountAndCategoryInfo(missingStatisticResponseItems)
                };

                lastAssetTrendItem = assetTrendItem;
                finalAssetTrendsData.push(finalAssetTrendItem);
            }

            // 填充当天数据
            for (const item of assetTrendItem.items) {
                const statisticResponseItem: TransactionStatisticResponseItem = {
                    categoryId: '',
                    accountId: item.accountId,
                    amountCents: item.accountClosingBalanceCents,
                    openingAmountCents: item.accountOpeningBalanceCents
                };

                lastAssetTrendItemMap[item.accountId] = item;
                existedAccountIds[item.accountId] = true;
                statisticResponseItems.push(statisticResponseItem);
            }

            // 用已知的上一笔余额补齐缺失账户
            for (const item of values(lastAssetTrendItemMap)) {
                if (existedAccountIds[item.accountId]) {
                    continue;
                }

                const statisticResponseItem: TransactionStatisticResponseItem = {
                    categoryId: '',
                    accountId: item.accountId,
                    amountCents: item.accountClosingBalanceCents,
                    openingAmountCents: item.accountClosingBalanceCents
                };

                existedAccountIds[item.accountId] = true;
                statisticResponseItems.push(statisticResponseItem);
            }

            const finalAssetTrendItem: TransactionStatisticAssetTrendsResponseItemWithInfo = {
                year: assetTrendItem.year,
                month: assetTrendItem.month,
                day: assetTrendItem.day,
                items: assembleAccountAndCategoryInfo(statisticResponseItems)
            };

            lastAssetTrendItem = assetTrendItem;
            finalAssetTrendsData.push(finalAssetTrendItem);
        }

        return finalAssetTrendsData;
    });

    const assetTrendsData = computed<TransactionAssetTrendsAnalysisData | null>(() => {
        if (!assetTrendsDataWithAccountInfo.value || !assetTrendsDataWithAccountInfo.value.length) {
            return null;
        }

        const combinedDataMap: Record<string, WritableTransactionAssetTrendsAnalysisDataItem> = {};

        for (const dailyData of assetTrendsDataWithAccountInfo.value) {
              let dailyTotalAmount = 0;
              let dailyOpeningTotalAmount = 0;

            for (const item of dailyData.items) {
                if (!item.primaryAccount || !item.account) {
                    continue;
                }

                if (transactionStatisticsFilter.value.filterAccountIds && transactionStatisticsFilter.value.filterAccountIds[item.account.id]) {
                    continue;
                }

                if (!isNumber(item.amountInDefaultCurrencyCents)) {
                    continue;
                }

                let amount = item.amountInDefaultCurrencyCents;
                let openingAmount = item.openingAmountInDefaultCurrencyCents || 0;

                if (item.account.isLiability) {
                    amount = -amount;
                    openingAmount = -openingAmount;
                }

                if (transactionStatisticsFilter.value.chartDataType === ChartDataType.AccountTotalAssets.type ||
                    transactionStatisticsFilter.value.chartDataType === ChartDataType.AccountTotalLiabilities.type) {
                    let data = combinedDataMap[item.account.id];

                    if (data) {
                        data.totalAmountCents += amount;
                        data.totalOpeningAmountCents = (data.totalOpeningAmountCents || 0) + openingAmount;
                    } else {
                        data = {
                            name: item.account.name,
                            type: 'account',
                            id: item.account.id,
                            icon: item.account.icon || DEFAULT_ACCOUNT_ICON.icon,
                            color: item.account.color || DEFAULT_ACCOUNT_COLOR,
                            hidden: item.primaryAccount.hidden || item.account.hidden,
                            displayOrders: [item.primaryAccount.category, item.primaryAccount.displayOrder, item.account.displayOrder],
                            totalAmountCents: amount,
                            totalOpeningAmountCents: openingAmount,
                            items: []
                        };
                        combinedDataMap[item.account.id] = data;
                    }

                    data.items.push({
                        year: dailyData.year,
                        month: dailyData.month,
                        day: dailyData.day,
                        totalAmountCents: amount,
                        totalOpeningAmountCents: openingAmount
                    });
                } else if (transactionStatisticsFilter.value.chartDataType === ChartDataType.NetWorth.type) {
                    dailyTotalAmount += amount;
                    dailyOpeningTotalAmount += openingAmount;
                }
            }

            if (transactionStatisticsFilter.value.chartDataType === ChartDataType.NetWorth.type) {
                let data = combinedDataMap['netWorth'];

                if (data) {
                    data.totalAmountCents += dailyTotalAmount;
                    data.totalOpeningAmountCents = (data.totalOpeningAmountCents || 0) + dailyOpeningTotalAmount;
                } else {
                    data = {
                        name: tt('Net Worth'),
                        type: 'account',
                        id: 'netWorth',
                        icon: 'bank',
                        color: DEFAULT_CHART_COLORS[0] ?? DEFAULT_ACCOUNT_COLOR,
                        hidden: false,
                        displayOrders: [0],
                        totalAmountCents: dailyTotalAmount,
                        totalOpeningAmountCents: dailyOpeningTotalAmount,
                        items: []
                    };
                }

                const amountItem: TransactionAssetTrendsAnalysisDataAmount = {
                    year: dailyData.year,
                    month: dailyData.month,
                    day: dailyData.day,
                    totalAmountCents: dailyTotalAmount,
                    totalOpeningAmountCents: dailyOpeningTotalAmount
                };
                data.items.push(amountItem);
                combinedDataMap['netWorth'] = data;
            }
        }

        const allAssetTrendsDataItems: TransactionAssetTrendsAnalysisDataItem[] = [];

        for (const assetTrendsDataItem of values(combinedDataMap)) {
            // v6.71: 过滤没有数据的账户 - 只显示有非零数据的账户
            // 检查该账户是否有任何非零数据点
            const hasNonZeroData = assetTrendsDataItem.items.some(item => {
                // 检查 totalAmountCents 是否非零（使用小数精度容差）
                const hasAmount = Math.abs(item.totalAmountCents) > 0.001;
                // 也检查 totalOpeningAmountCents 是否非零（如果存在）
                const hasOpeningAmount = item.totalOpeningAmountCents !== undefined &&
                    Math.abs(item.totalOpeningAmountCents) > 0.001;
                return hasAmount || hasOpeningAmount;
            });

            // 只添加有非零数据的账户到图表中
            if (hasNonZeroData) {
                allAssetTrendsDataItems.push(assetTrendsDataItem);
            }
        }

        sortCategoryTotalAmountItems(allAssetTrendsDataItems, transactionStatisticsFilter.value);

        const assetTrendsData: TransactionAssetTrendsAnalysisData = {
            items: allAssetTrendsDataItems
        };

        return assetTrendsData;
    });

    function createNewTransactionCategoricalOverviewAnalysisDataItem(id: string, name: string, type: TransactionCategoricalOverviewAnalysisDataItemType, displayOrders: number[], hidden: boolean): TransactionCategoricalOverviewAnalysisDataItem {
        const dataItem: TransactionCategoricalOverviewAnalysisDataItem = {
            id: id,
            name: name,
            type: type,
            displayOrders: displayOrders,
            hidden: hidden,
            inflows: [],
            outflows: [],
            totalAmountCents: 0,
            totalNonNegativeAmountCents: 0
        };

        return dataItem;
    }

    function sortCategoricalOverviewAnalysisDataItems(items: TransactionCategoricalOverviewAnalysisDataItem[], transactionStatisticsFilter: TransactionStatisticsFilter): void {
        let totalNonNegativeAmountCents: number = 0;

        for (const item of items) {
            totalNonNegativeAmountCents += item.totalNonNegativeAmountCents;
        }

        if (totalNonNegativeAmountCents > 0) {
            for (const item of items) {
                if (!item.includeInPercent) {
                    continue;
                }

                item.percent = Math.abs(item.totalAmountCents) * 100 / totalNonNegativeAmountCents;
            }
        }

        sortStatisticsItems(items, transactionStatisticsFilter.sortingType);
    }

    function assembleAccountAndCategoryInfo(items: TransactionStatisticResponseItem[]): TransactionStatisticResponseItemWithInfo[] {
        const finalItems: TransactionStatisticResponseItemWithInfo[] = [];
        const defaultCurrency = userStore.currentUserDefaultCurrency;

        for (const dataItem of items) {
            const item: TransactionStatisticResponseItemWithInfo = {
                categoryId: dataItem.categoryId,
                accountId: dataItem.accountId,
                relatedAccountId: dataItem.relatedAccountId,
                relatedAccountType: dataItem.relatedAccountType,
                amountCents: dataItem.amountCents,
                openingAmountCents: dataItem.openingAmountCents,
                amountInDefaultCurrencyCents: null,
                openingAmountInDefaultCurrencyCents: null
            };

            if (item.accountId) {
                item.account = accountsStore.allAccountsMap[item.accountId];
            }

            if (item.account && item.account.parentId !== '0') {
                item.primaryAccount = accountsStore.allAccountsMap[item.account.parentId];
            } else {
                item.primaryAccount = item.account;
            }

            if (item.relatedAccountId) {
                item.relatedAccount = accountsStore.allAccountsMap[item.relatedAccountId];
            }

            if (item.relatedAccount && item.relatedAccount.parentId !== '0') {
                item.relatedPrimaryAccount = accountsStore.allAccountsMap[item.relatedAccount.parentId];
            } else {
                item.relatedPrimaryAccount = item.relatedAccount;
            }

            if (item.categoryId) {
                item.category = transactionCategoriesStore.allTransactionCategoriesMap[item.categoryId];
            }

            if (item.category && item.category.parentId !== '0') {
                item.primaryCategory = transactionCategoriesStore.allTransactionCategoriesMap[item.category.parentId];
            } else {
                item.primaryCategory = item.category;
            }

            if (item.account && item.account.currency !== defaultCurrency) {
                const amount = exchangeRatesStore.getExchangedAmount(item.amountCents, item.account.currency, defaultCurrency);
                const openingAmount = isNumber(item.openingAmountCents) ? exchangeRatesStore.getExchangedAmount(item.openingAmountCents, item.account.currency, defaultCurrency) : null;

                if (isNumber(amount)) {
                    item.amountInDefaultCurrencyCents = Math.trunc(amount);
                }

                if (isNumber(openingAmount)) {
                    item.openingAmountInDefaultCurrencyCents = Math.trunc(openingAmount);
                }
            } else if (item.account && item.account.currency === defaultCurrency) {
                item.amountInDefaultCurrencyCents = item.amountCents;
                item.openingAmountInDefaultCurrencyCents = item.openingAmountCents;
            } else {
                item.amountInDefaultCurrencyCents = null;
                item.openingAmountInDefaultCurrencyCents = null;
            }

            finalItems.push(item);
        }

        return finalItems;
    }

    function getCategoryTotalAmountItems(items: TransactionStatisticResponseItemWithInfo[], transactionStatisticsFilter: TransactionStatisticsFilter): WritableTransactionCategoricalAnalysisData {
        const allDataItems: Record<string, WritableTransactionCategoricalAnalysisDataItem> = {};
        let totalAmountCents = 0;
        let totalNonNegativeAmountCents = 0;

        for (const item of items) {
            if (!item.primaryAccount || !item.account || !item.primaryCategory || !item.category) {
                continue;
            }

            if (transactionStatisticsFilter.chartDataType === ChartDataType.OutflowsByAccount.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.TotalOutflows.type) {
                // v6.70: 流出统计包含支出、转账流出和投资类型
                if (item.category.type === CategoryType.Transfer) {
                    if (item.relatedAccountType !== TransactionRelatedAccountType.TransferTo) {
                        continue;
                    }
                } else if (item.category.type === CategoryType.Investment) {
                    // 投资类型视为资金流出（从源账户流出到投资账户）
                    if (item.relatedAccountType !== TransactionRelatedAccountType.TransferTo) {
                        continue;
                    }
                } else if (item.category.type !== CategoryType.Expense) {
                    continue;
                }
            } else if (transactionStatisticsFilter.chartDataType === ChartDataType.ExpenseByAccount.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.ExpenseByPrimaryCategory.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.ExpenseBySecondaryCategory.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.TotalExpense.type) {
                if (item.category.type !== CategoryType.Expense) {
                    continue;
                }
            } else if (transactionStatisticsFilter.chartDataType === ChartDataType.InflowsByAccount.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.TotalInflows.type) {
                // v6.70: 流入统计包含收入、转账流入和投资流入（投资账户收到资金）
                if (item.category.type === CategoryType.Transfer) {
                    if (item.relatedAccountType !== TransactionRelatedAccountType.TransferFrom) {
                        continue;
                    }
                } else if (item.category.type === CategoryType.Investment) {
                    // 投资账户收到资金视为流入
                    if (item.relatedAccountType !== TransactionRelatedAccountType.TransferFrom) {
                        continue;
                    }
                } else if (item.category.type !== CategoryType.Income) {
                    continue;
                }
            } else if (transactionStatisticsFilter.chartDataType === ChartDataType.IncomeByAccount.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.IncomeByPrimaryCategory.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.IncomeBySecondaryCategory.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.TotalIncome.type) {
                if (item.category.type !== CategoryType.Income) {
                    continue;
                }
            } else if (transactionStatisticsFilter.chartDataType === ChartDataType.NetCashFlow.type) {
                // 不做处理
            } else if (transactionStatisticsFilter.chartDataType === ChartDataType.NetIncome.type) {
                if (item.category.type === CategoryType.Transfer) {
                    continue;
                }
            } else {
                continue;
            }

            if (transactionStatisticsFilter.filterAccountIds && transactionStatisticsFilter.filterAccountIds[item.account.id]) {
                continue;
            }

            if (transactionStatisticsFilter.filterCategoryIds && transactionStatisticsFilter.filterCategoryIds[item.category.id]) {
                continue;
            }

            if (transactionStatisticsFilter.chartDataType === ChartDataType.OutflowsByAccount.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.ExpenseByAccount.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.InflowsByAccount.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.IncomeByAccount.type) {
                if (isNumber(item.amountInDefaultCurrencyCents)) {
                    let data = allDataItems[item.account.id];

                    if (data) {
                        data.totalAmountCents += item.amountInDefaultCurrencyCents;
                    } else {
                        data = {
                            name: item.account.name,
                            type: 'account',
                            id: item.account.id,
                            icon: item.account.icon || DEFAULT_ACCOUNT_ICON.icon,
                            color: item.account.color || DEFAULT_ACCOUNT_COLOR,
                            hidden: item.primaryAccount.hidden || item.account.hidden,
                            displayOrders: [item.primaryAccount.category, item.primaryAccount.displayOrder, item.account.displayOrder],
                            totalAmountCents: item.amountInDefaultCurrencyCents
                        };
                    }

                    let includeInTotal: boolean = true;

                    // 总流出 / 总流入不包含未被筛选账户之间的转账交易
                    if (transactionStatisticsFilter.chartDataType === ChartDataType.OutflowsByAccount.type ||
                        transactionStatisticsFilter.chartDataType === ChartDataType.InflowsByAccount.type) {
                        if (item.relatedAccount && (!transactionStatisticsFilter.filterAccountIds || !transactionStatisticsFilter.filterAccountIds[item.relatedAccount.id])) {
                            includeInTotal = false;
                        }
                    }

                    if (includeInTotal) {
                        totalAmountCents += item.amountInDefaultCurrencyCents;
                        totalNonNegativeAmountCents += Math.abs(item.amountInDefaultCurrencyCents);
                    }

                    allDataItems[item.account.id] = data;
                }
            } else if (transactionStatisticsFilter.chartDataType === ChartDataType.ExpenseByPrimaryCategory.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.IncomeByPrimaryCategory.type) {
                if (isNumber(item.amountInDefaultCurrencyCents)) {
                    let data = allDataItems[item.primaryCategory.id];

                    if (data) {
                        data.totalAmountCents += item.amountInDefaultCurrencyCents;
                    } else {
                        data = {
                            name: item.primaryCategory.name,
                            type: 'category',
                            id: item.primaryCategory.id,
                            icon: item.primaryCategory.icon || DEFAULT_CATEGORY_ICON.icon,
                            color: item.primaryCategory.color || DEFAULT_CATEGORY_COLOR,
                            hidden: item.primaryCategory.hidden,
                            displayOrders: [item.primaryCategory.type, item.primaryCategory.displayOrder],
                            totalAmountCents: item.amountInDefaultCurrencyCents
                        };
                    }

                    totalAmountCents += item.amountInDefaultCurrencyCents;
                    totalNonNegativeAmountCents += Math.abs(item.amountInDefaultCurrencyCents);

                    allDataItems[item.primaryCategory.id] = data;
                }
            } else if (transactionStatisticsFilter.chartDataType === ChartDataType.ExpenseBySecondaryCategory.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.IncomeBySecondaryCategory.type) {
                if (isNumber(item.amountInDefaultCurrencyCents)) {
                    let data = allDataItems[item.category.id];

                    if (data) {
                        data.totalAmountCents += item.amountInDefaultCurrencyCents;
                    } else {
                        data = {
                            name: item.category.name,
                            type: 'category',
                            id: item.category.id,
                            icon: item.category.icon || DEFAULT_CATEGORY_ICON.icon,
                            color: item.category.color || DEFAULT_CATEGORY_COLOR,
                            hidden: item.primaryCategory.hidden || item.category.hidden,
                            displayOrders: [item.primaryCategory.type, item.primaryCategory.displayOrder, item.category.displayOrder],
                            totalAmountCents: item.amountInDefaultCurrencyCents
                        };
                    }

                    totalAmountCents += item.amountInDefaultCurrencyCents;
                    totalNonNegativeAmountCents += Math.abs(item.amountInDefaultCurrencyCents);

                    allDataItems[item.category.id] = data;
                }
            } else if (transactionStatisticsFilter.chartDataType === ChartDataType.TotalOutflows.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.TotalExpense.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.TotalInflows.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.TotalIncome.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.NetCashFlow.type ||
                transactionStatisticsFilter.chartDataType === ChartDataType.NetIncome.type) {
                if (isNumber(item.amountInDefaultCurrencyCents)) {
                    let data = allDataItems['total'];
                    let amount = item.amountInDefaultCurrencyCents;
                    let includeInTotal: boolean = true;

                    if (transactionStatisticsFilter.chartDataType === ChartDataType.NetCashFlow.type &&
                        (item.category.type === CategoryType.Expense ||
                         (item.category.type === CategoryType.Transfer && item.relatedAccountType === TransactionRelatedAccountType.TransferTo) ||
                         (item.category.type === CategoryType.Investment && item.relatedAccountType === TransactionRelatedAccountType.TransferTo))) {
                        // v6.70: 支出、转账流出、投资流出都视为负数
                        amount = -amount;
                    } else if (transactionStatisticsFilter.chartDataType === ChartDataType.NetIncome.type &&
                        item.category.type === CategoryType.Expense) {
                        amount = -amount;
                    }

                    // 总流出 / 总流入不包含未被筛选账户之间的转账交易
                    if (transactionStatisticsFilter.chartDataType === ChartDataType.TotalOutflows.type ||
                        transactionStatisticsFilter.chartDataType === ChartDataType.TotalInflows.type ||
                        transactionStatisticsFilter.chartDataType === ChartDataType.NetCashFlow.type) {
                        if (item.relatedAccount && (!transactionStatisticsFilter.filterAccountIds || !transactionStatisticsFilter.filterAccountIds[item.relatedAccount.id])) {
                            includeInTotal = false;
                        }
                    }

                    if (!data) {
                        let name = '';

                        if (transactionStatisticsFilter.chartDataType === ChartDataType.TotalOutflows.type) {
                            name = ChartDataType.TotalOutflows.name;
                        } else if (transactionStatisticsFilter.chartDataType === ChartDataType.TotalExpense.type) {
                            name = ChartDataType.TotalExpense.name;
                        } else if (transactionStatisticsFilter.chartDataType === ChartDataType.TotalInflows.type) {
                            name = ChartDataType.TotalInflows.name;
                        } else if (transactionStatisticsFilter.chartDataType === ChartDataType.TotalIncome.type) {
                            name = ChartDataType.TotalIncome.name;
                        } else if (transactionStatisticsFilter.chartDataType === ChartDataType.NetCashFlow.type) {
                            name = ChartDataType.NetCashFlow.name;
                        } else if (transactionStatisticsFilter.chartDataType === ChartDataType.NetIncome.type) {
                            name = ChartDataType.NetIncome.name;
                        }

                        data = {
                            name: name,
                            type: 'total',
                            id: 'total',
                            icon: '',
                            color: '',
                            hidden: false,
                            displayOrders: [1],
                            totalAmountCents: 0
                        };
                    }

                    if (includeInTotal) {
                        data.totalAmountCents += amount;

                        totalAmountCents += amount;

                        if (item.amountInDefaultCurrencyCents > 0) {
                            totalNonNegativeAmountCents += amount;
                        }
                    }

                    allDataItems['total'] = data;
                }
            }
        }

        return {
            totalAmountCents: totalAmountCents,
            totalNonNegativeAmountCents: totalNonNegativeAmountCents,
            items: allDataItems
        };
    }

    function sortCategoryTotalAmountItems(items: TransactionStatisticDataItemBase[], transactionStatisticsFilter: TransactionStatisticsFilter): void {
        sortStatisticsItems(items, transactionStatisticsFilter.sortingType);
    }

    function updateTransactionStatisticsInvalidState(invalidState: boolean): void {
        transactionStatisticsStateInvalid.value = invalidState;
    }

    function resetTransactionStatistics(): void {
        transactionStatisticsFilter.value.chartDataType = ChartDataType.Default.type;
        transactionStatisticsFilter.value.categoricalChartType = CategoricalChartType.Default.type;
        transactionStatisticsFilter.value.categoricalChartDateType = DEFAULT_CATEGORICAL_CHART_DATA_RANGE.type;
        transactionStatisticsFilter.value.categoricalChartStartTime = 0;
        transactionStatisticsFilter.value.categoricalChartEndTime = 0;
        transactionStatisticsFilter.value.trendChartType = TrendChartType.Default.type;
        transactionStatisticsFilter.value.trendChartDateType = DEFAULT_TREND_CHART_DATA_RANGE.type;
        transactionStatisticsFilter.value.trendChartStartYearMonth = '';
        transactionStatisticsFilter.value.trendChartEndYearMonth = '';
        transactionStatisticsFilter.value.assetTrendsChartType = TrendChartType.Default.type;
        transactionStatisticsFilter.value.assetTrendsChartDateType = DEFAULT_ASSET_TRENDS_CHART_DATA_RANGE.type;
        transactionStatisticsFilter.value.assetTrendsChartStartTime = 0;
        transactionStatisticsFilter.value.assetTrendsChartEndTime = 0;
        transactionStatisticsFilter.value.filterAccountIds = {};
        transactionStatisticsFilter.value.filterCategoryIds = {};
        transactionStatisticsFilter.value.tagIds = '';
        transactionStatisticsFilter.value.tagFilterType = TransactionTagFilterType.Default.type;
        transactionStatisticsFilter.value.keyword = '';
        transactionCategoryStatisticsData.value = null;
        transactionCategoryTrendsData.value = [];
        transactionStatisticsStateInvalid.value = true;
    }

    function initTransactionStatisticsFilter(analysisType: StatisticsAnalysisType, filter?: TransactionStatisticsPartialFilter): void {
        if (filter && isInteger(filter.chartDataType)) {
            transactionStatisticsFilter.value.chartDataType = filter.chartDataType;
        } else {
            transactionStatisticsFilter.value.chartDataType = settingsStore.appSettings.statistics.defaultChartDataType;
        }

        if (analysisType === StatisticsAnalysisType.CategoricalAnalysis || analysisType === StatisticsAnalysisType.TrendAnalysis) {
            if (!ChartDataType.isAvailableForAnalysisType(transactionStatisticsFilter.value.chartDataType, analysisType)) {
                transactionStatisticsFilter.value.chartDataType = ChartDataType.Default.type;
            }
        } else if (analysisType === StatisticsAnalysisType.AssetTrends) {
            if (!ChartDataType.isAvailableForAnalysisType(transactionStatisticsFilter.value.chartDataType, analysisType)) {
                transactionStatisticsFilter.value.chartDataType = ChartDataType.DefaultForAssetTrends.type;
            }
        }

        // 分类分析筛选条件初始化
        if (filter && isInteger(filter.categoricalChartType)) {
            transactionStatisticsFilter.value.categoricalChartType = filter.categoricalChartType;
        } else {
            transactionStatisticsFilter.value.categoricalChartType = settingsStore.appSettings.statistics.defaultCategoricalChartType;
        }

        if (!CategoricalChartType.isValidType(transactionStatisticsFilter.value.categoricalChartType)) {
            transactionStatisticsFilter.value.categoricalChartType = CategoricalChartType.Default.type;
        }

        if (filter && isInteger(filter.categoricalChartDateType)) {
            transactionStatisticsFilter.value.categoricalChartDateType = filter.categoricalChartDateType;
        } else {
            transactionStatisticsFilter.value.categoricalChartDateType = settingsStore.appSettings.statistics.defaultCategoricalChartDataRangeType;
        }

        let categoricalChartDateTypeValid = true;

        if (!DateRange.isAvailableForScene(transactionStatisticsFilter.value.categoricalChartDateType, DateRangeScene.Normal)) {
            transactionStatisticsFilter.value.categoricalChartDateType = DEFAULT_CATEGORICAL_CHART_DATA_RANGE.type;
            categoricalChartDateTypeValid = false;
        }

        if (categoricalChartDateTypeValid && transactionStatisticsFilter.value.categoricalChartDateType === DateRange.Custom.type) {
            if (filter && isInteger(filter.categoricalChartStartTime)) {
                transactionStatisticsFilter.value.categoricalChartStartTime = filter.categoricalChartStartTime;
            } else {
                transactionStatisticsFilter.value.categoricalChartStartTime = 0;
            }

            if (filter && isInteger(filter.categoricalChartEndTime)) {
                transactionStatisticsFilter.value.categoricalChartEndTime = filter.categoricalChartEndTime;
            } else {
                transactionStatisticsFilter.value.categoricalChartEndTime = 0;
            }
        } else {
            const categoricalChartDateRange = getDateRangeByDateType(transactionStatisticsFilter.value.categoricalChartDateType, userStore.currentUserFirstDayOfWeek, userStore.currentUserFiscalYearStart);

            if (categoricalChartDateRange) {
                transactionStatisticsFilter.value.categoricalChartDateType = categoricalChartDateRange.dateType;
                transactionStatisticsFilter.value.categoricalChartStartTime = categoricalChartDateRange.minTime;
                transactionStatisticsFilter.value.categoricalChartEndTime = categoricalChartDateRange.maxTime;
            }
        }

        // 趋势分析筛选条件初始化
        if (filter && isInteger(filter.trendChartType)) {
            transactionStatisticsFilter.value.trendChartType = filter.trendChartType;
        } else {
            transactionStatisticsFilter.value.trendChartType = settingsStore.appSettings.statistics.defaultTrendChartType;
        }

        if (!TrendChartType.isValidType(transactionStatisticsFilter.value.trendChartType)) {
            transactionStatisticsFilter.value.trendChartType = TrendChartType.Default.type;
        }

        if (filter && isInteger(filter.trendChartDateType)) {
            transactionStatisticsFilter.value.trendChartDateType = filter.trendChartDateType;
        } else {
            transactionStatisticsFilter.value.trendChartDateType = settingsStore.appSettings.statistics.defaultTrendChartDataRangeType;
        }

        let trendChartDateTypeValid = true;

        if (!DateRange.isAvailableForScene(transactionStatisticsFilter.value.trendChartDateType, DateRangeScene.TrendAnalysis)) {
            transactionStatisticsFilter.value.trendChartDateType = DEFAULT_TREND_CHART_DATA_RANGE.type;
            trendChartDateTypeValid = false;
        }

        if (trendChartDateTypeValid && transactionStatisticsFilter.value.trendChartDateType === DateRange.Custom.type) {
            if (filter && isYearMonth(filter.trendChartStartYearMonth)) {
                transactionStatisticsFilter.value.trendChartStartYearMonth = filter.trendChartStartYearMonth;
            } else {
                transactionStatisticsFilter.value.trendChartStartYearMonth = '';
            }

            if (filter && isYearMonth(filter.trendChartEndYearMonth)) {
                transactionStatisticsFilter.value.trendChartEndYearMonth = filter.trendChartEndYearMonth;
            } else {
                transactionStatisticsFilter.value.trendChartEndYearMonth = '';
            }
        } else {
            const trendChartDateRange = getDateRangeByDateType(transactionStatisticsFilter.value.trendChartDateType, userStore.currentUserFirstDayOfWeek, userStore.currentUserFiscalYearStart);

            if (trendChartDateRange) {
                transactionStatisticsFilter.value.trendChartDateType = trendChartDateRange.dateType;
                    if (trendChartDateRange.dateType === DateRange.All.type) {
                        transactionStatisticsFilter.value.trendChartStartYearMonth = '';
                        transactionStatisticsFilter.value.trendChartEndYearMonth = '';
                    } else {
                        transactionStatisticsFilter.value.trendChartStartYearMonth = getGregorianCalendarYearAndMonthFromUnixTime(trendChartDateRange.minTime);
                        transactionStatisticsFilter.value.trendChartEndYearMonth = getGregorianCalendarYearAndMonthFromUnixTime(trendChartDateRange.maxTime);
                    }
            }
        }

        // 资产趋势筛选条件初始化
        if (filter && isInteger(filter.assetTrendsChartType)) {
            transactionStatisticsFilter.value.assetTrendsChartType = filter.assetTrendsChartType;
        } else {
            transactionStatisticsFilter.value.assetTrendsChartType = settingsStore.appSettings.statistics.defaultAssetTrendsChartType;
        }

        if (!TrendChartType.isValidType(transactionStatisticsFilter.value.assetTrendsChartType)) {
            transactionStatisticsFilter.value.assetTrendsChartType = TrendChartType.Default.type;
        }

        if (filter && isInteger(filter.assetTrendsChartDateType)) {
            transactionStatisticsFilter.value.assetTrendsChartDateType = filter.assetTrendsChartDateType;
        } else {
            transactionStatisticsFilter.value.assetTrendsChartDateType = settingsStore.appSettings.statistics.defaultAssetTrendsChartDataRangeType;
        }

        let assetTrendsChartDateTypeValid = true;

        if (!DateRange.isAvailableForScene(transactionStatisticsFilter.value.assetTrendsChartDateType, DateRangeScene.AssetTrends)) {
            transactionStatisticsFilter.value.assetTrendsChartDateType = DEFAULT_ASSET_TRENDS_CHART_DATA_RANGE.type;
            assetTrendsChartDateTypeValid = false;
        }

        if (assetTrendsChartDateTypeValid && transactionStatisticsFilter.value.assetTrendsChartDateType === DateRange.Custom.type) {
            if (filter && isInteger(filter.assetTrendsChartStartTime)) {
                transactionStatisticsFilter.value.assetTrendsChartStartTime = filter.assetTrendsChartStartTime;
            } else {
                transactionStatisticsFilter.value.assetTrendsChartStartTime = 0;
            }

            if (filter && isInteger(filter.assetTrendsChartEndTime)) {
                transactionStatisticsFilter.value.assetTrendsChartEndTime = filter.assetTrendsChartEndTime;
            } else {
                transactionStatisticsFilter.value.assetTrendsChartEndTime = 0;
            }
        } else {
            const assetTrendsChartDateRange = getDateRangeByDateType(transactionStatisticsFilter.value.assetTrendsChartDateType, userStore.currentUserFirstDayOfWeek, userStore.currentUserFiscalYearStart);

            if (assetTrendsChartDateRange) {
                transactionStatisticsFilter.value.assetTrendsChartDateType = assetTrendsChartDateRange.dateType;
                transactionStatisticsFilter.value.assetTrendsChartStartTime = assetTrendsChartDateRange.minTime;
                transactionStatisticsFilter.value.assetTrendsChartEndTime = assetTrendsChartDateRange.maxTime;
            }
        }

        // 其他筛选条件初始化
        if (filter && isObject(filter.filterAccountIds)) {
            transactionStatisticsFilter.value.filterAccountIds = filter.filterAccountIds;
        } else {
            transactionStatisticsFilter.value.filterAccountIds = settingsStore.appSettings.statistics.defaultAccountFilter || {};
        }

        if (filter && isObject(filter.filterCategoryIds)) {
            transactionStatisticsFilter.value.filterCategoryIds = filter.filterCategoryIds;
        } else {
            transactionStatisticsFilter.value.filterCategoryIds = settingsStore.appSettings.statistics.defaultTransactionCategoryFilter || {};
        }

        if (filter && isString(filter.tagIds)) {
            transactionStatisticsFilter.value.tagIds = filter.tagIds;
        } else {
            transactionStatisticsFilter.value.tagIds = '';
        }

        if (filter && isInteger(filter.tagFilterType)) {
            transactionStatisticsFilter.value.tagFilterType = filter.tagFilterType;
        } else {
            transactionStatisticsFilter.value.tagFilterType = TransactionTagFilterType.Default.type;
        }

        if (filter && isString(filter.keyword)) {
            transactionStatisticsFilter.value.keyword = filter.keyword;
        } else {
            transactionStatisticsFilter.value.keyword = '';
        }

        if (filter && isInteger(filter.sortingType)) {
            transactionStatisticsFilter.value.sortingType = filter.sortingType;
        } else {
            transactionStatisticsFilter.value.sortingType = settingsStore.appSettings.statistics.defaultSortingType;
        }

        if (transactionStatisticsFilter.value.sortingType < ChartSortingType.Amount.type || transactionStatisticsFilter.value.sortingType > ChartSortingType.Name.type) {
            transactionStatisticsFilter.value.sortingType = ChartSortingType.Default.type;
        }
    }

    function updateTransactionStatisticsFilter(filter: TransactionStatisticsPartialFilter): boolean {
        let changed = false;

        if (filter && isInteger(filter.chartDataType) && transactionStatisticsFilter.value.chartDataType !== filter.chartDataType) {
            transactionStatisticsFilter.value.chartDataType = filter.chartDataType;
            changed = true;
        }

        // 分类分析筛选条件更新
        if (filter && isInteger(filter.categoricalChartType) && transactionStatisticsFilter.value.categoricalChartType !== filter.categoricalChartType) {
            transactionStatisticsFilter.value.categoricalChartType = filter.categoricalChartType;
            changed = true;
        }

        if (filter && isInteger(filter.categoricalChartDateType) && transactionStatisticsFilter.value.categoricalChartDateType !== filter.categoricalChartDateType) {
            transactionStatisticsFilter.value.categoricalChartDateType = filter.categoricalChartDateType;
            changed = true;

            if (filter.categoricalChartDateType !== DateRange.Custom.type) {
                const categoricalChartDateRange = getDateRangeByDateType(filter.categoricalChartDateType, userStore.currentUserFirstDayOfWeek, userStore.currentUserFiscalYearStart);

                if (categoricalChartDateRange) {
                    transactionStatisticsFilter.value.categoricalChartStartTime = categoricalChartDateRange.minTime;
                    transactionStatisticsFilter.value.categoricalChartEndTime = categoricalChartDateRange.maxTime;
                }
            }
        }

        if (filter && isInteger(filter.categoricalChartStartTime) && transactionStatisticsFilter.value.categoricalChartStartTime !== filter.categoricalChartStartTime) {
            transactionStatisticsFilter.value.categoricalChartStartTime = filter.categoricalChartStartTime;
            changed = true;
        }

        if (filter && isInteger(filter.categoricalChartEndTime) && transactionStatisticsFilter.value.categoricalChartEndTime !== filter.categoricalChartEndTime) {
            transactionStatisticsFilter.value.categoricalChartEndTime = filter.categoricalChartEndTime;
            changed = true;
        }

        // 趋势分析筛选条件更新
        if (filter && isInteger(filter.trendChartType) && transactionStatisticsFilter.value.trendChartType !== filter.trendChartType) {
            transactionStatisticsFilter.value.trendChartType = filter.trendChartType;
            changed = true;
        }

        if (filter && isInteger(filter.trendChartDateType) && transactionStatisticsFilter.value.trendChartDateType !== filter.trendChartDateType) {
            transactionStatisticsFilter.value.trendChartDateType = filter.trendChartDateType;
            changed = true;

            if (filter.trendChartDateType !== DateRange.Custom.type) {
                const trendChartDateRange = getDateRangeByDateType(filter.trendChartDateType, userStore.currentUserFirstDayOfWeek, userStore.currentUserFiscalYearStart);

                if (trendChartDateRange) {
                    if (filter.trendChartDateType === DateRange.All.type) {
                        transactionStatisticsFilter.value.trendChartStartYearMonth = '';
                        transactionStatisticsFilter.value.trendChartEndYearMonth = '';
                    } else {
                        transactionStatisticsFilter.value.trendChartStartYearMonth = getGregorianCalendarYearAndMonthFromUnixTime(trendChartDateRange.minTime);
                        transactionStatisticsFilter.value.trendChartEndYearMonth = getGregorianCalendarYearAndMonthFromUnixTime(trendChartDateRange.maxTime);
                    }
                }
            }
        }

        if (filter && (isYearMonth(filter.trendChartStartYearMonth) || filter.trendChartStartYearMonth === '') && !isYearMonthEquals(transactionStatisticsFilter.value.trendChartStartYearMonth, filter.trendChartStartYearMonth)) {
            transactionStatisticsFilter.value.trendChartStartYearMonth = filter.trendChartStartYearMonth;
            changed = true;
        }

        if (filter && (isYearMonth(filter.trendChartEndYearMonth) || filter.trendChartEndYearMonth === '') && !isYearMonthEquals(transactionStatisticsFilter.value.trendChartEndYearMonth, filter.trendChartEndYearMonth)) {
            transactionStatisticsFilter.value.trendChartEndYearMonth = filter.trendChartEndYearMonth;
            changed = true;
        }

        // 资产趋势筛选条件更新
        if (filter && isInteger(filter.assetTrendsChartType) && transactionStatisticsFilter.value.assetTrendsChartType !== filter.assetTrendsChartType) {
            transactionStatisticsFilter.value.assetTrendsChartType = filter.assetTrendsChartType;
            changed = true;
        }

        if (filter && isInteger(filter.assetTrendsChartDateType) && transactionStatisticsFilter.value.assetTrendsChartDateType !== filter.assetTrendsChartDateType) {
            transactionStatisticsFilter.value.assetTrendsChartDateType = filter.assetTrendsChartDateType;
            changed = true;

            if (filter.assetTrendsChartDateType !== DateRange.Custom.type) {
                const assetTrendsChartDateRange = getDateRangeByDateType(filter.assetTrendsChartDateType, userStore.currentUserFirstDayOfWeek, userStore.currentUserFiscalYearStart);

                if (assetTrendsChartDateRange) {
                    transactionStatisticsFilter.value.assetTrendsChartStartTime = assetTrendsChartDateRange.minTime;
                    transactionStatisticsFilter.value.assetTrendsChartEndTime = assetTrendsChartDateRange.maxTime;
                }
            }
        }

        if (filter && isInteger(filter.assetTrendsChartStartTime) && transactionStatisticsFilter.value.assetTrendsChartStartTime !== filter.assetTrendsChartStartTime) {
            transactionStatisticsFilter.value.assetTrendsChartStartTime = filter.assetTrendsChartStartTime;
            changed = true;
        }

        if (filter && isInteger(filter.assetTrendsChartEndTime) && transactionStatisticsFilter.value.assetTrendsChartEndTime !== filter.assetTrendsChartEndTime) {
            transactionStatisticsFilter.value.assetTrendsChartEndTime = filter.assetTrendsChartEndTime;
            changed = true;
        }

        // 其他筛选条件更新
        if (filter && isObject(filter.filterAccountIds) && !isEquals(transactionStatisticsFilter.value.filterAccountIds, filter.filterAccountIds)) {
            transactionStatisticsFilter.value.filterAccountIds = filter.filterAccountIds;
            changed = true;
        }

        if (filter && isObject(filter.filterCategoryIds) && !isEquals(transactionStatisticsFilter.value.filterCategoryIds, filter.filterCategoryIds)) {
            transactionStatisticsFilter.value.filterCategoryIds = filter.filterCategoryIds;
            changed = true;
        }

        if (filter && isString(filter.tagIds) && transactionStatisticsFilter.value.tagIds !== filter.tagIds) {
            transactionStatisticsFilter.value.tagIds = filter.tagIds;
            changed = true;
        }

        if (filter && isInteger(filter.tagFilterType) && transactionStatisticsFilter.value.tagFilterType !== filter.tagFilterType) {
            transactionStatisticsFilter.value.tagFilterType = filter.tagFilterType;
            changed = true;
        }

        if (filter && isString(filter.keyword) && transactionStatisticsFilter.value.keyword !== filter.keyword) {
            transactionStatisticsFilter.value.keyword = filter.keyword;
            changed = true;
        }

        if (filter && isInteger(filter.sortingType) && transactionStatisticsFilter.value.sortingType !== filter.sortingType) {
            transactionStatisticsFilter.value.sortingType = filter.sortingType;
            changed = true;
        }

        return changed;
    }

    function getTransactionStatisticsPageParams(analysisType: StatisticsAnalysisType, trendDateAggregationType: number, assetTrendsDateAggregationType: number): string {
        return buildTransactionStatisticsPageParams(
            transactionStatisticsFilter.value,
            analysisType,
            trendDateAggregationType,
            assetTrendsDateAggregationType
        );
    }

    function getTransactionListPageParams(analysisType: StatisticsAnalysisType, itemId: string, dateRange?: TimeRangeAndDateType): string {
        return buildTransactionListPageParams({
            filter: transactionStatisticsFilter.value,
            accountsMap: accountsStore.allAccountsMap,
            categoriesMap: transactionCategoriesStore.allTransactionCategoriesMap,
            analysisType,
            itemId,
            dateRange
        });
    }

    function loadCategoricalAnalysis({ force }: { force: boolean }): Promise<TransactionStatisticResponse> {
        return new Promise((resolve, reject) => {
            services.getTransactionStatistics({
                startTime: transactionStatisticsFilter.value.categoricalChartStartTime,
                endTime: transactionStatisticsFilter.value.categoricalChartEndTime,
                tagIds: transactionStatisticsFilter.value.tagIds,
                tagFilterType: transactionStatisticsFilter.value.tagFilterType,
                keyword: transactionStatisticsFilter.value.keyword,
                useTransactionTimezone: settingsStore.appSettings.statistics.defaultTimezoneType === TimezoneTypeForStatistics.TransactionTimezone.type
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                    return;
                }

                if (transactionStatisticsStateInvalid.value) {
                    updateTransactionStatisticsInvalidState(false);
                }

                if (force && data.result && isEquals(transactionCategoryStatisticsData.value, data.result)) {
                    reject({ message: 'Data is up to date', isUpToDate: true });
                    return;
                }

                transactionCategoryStatisticsData.value = data.result;

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to retrieve transaction statistics', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function loadTrendAnalysis({ force }: { force: boolean }): Promise<TransactionStatisticTrendsResponseItem[]> {
        return new Promise((resolve, reject) => {
            const isAllDateRange = transactionStatisticsFilter.value.trendChartDateType === DateRange.All.type;

            services.getTransactionStatisticsTrends({
                startYearMonth: isAllDateRange ? '197001' : transactionStatisticsFilter.value.trendChartStartYearMonth,
                endYearMonth: isAllDateRange ? '197001' : transactionStatisticsFilter.value.trendChartEndYearMonth,
                tagIds: transactionStatisticsFilter.value.tagIds,
                tagFilterType: transactionStatisticsFilter.value.tagFilterType,
                keyword: transactionStatisticsFilter.value.keyword,
                useTransactionTimezone: settingsStore.appSettings.statistics.defaultTimezoneType === TimezoneTypeForStatistics.TransactionTimezone.type
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                    return;
                }

                if (transactionStatisticsStateInvalid.value) {
                    updateTransactionStatisticsInvalidState(false);
                }

                if (force && data.result && isEquals(transactionCategoryTrendsData.value, data.result)) {
                    reject({ message: 'Data is up to date', isUpToDate: true });
                    return;
                }

                transactionCategoryTrendsData.value = data.result;

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to retrieve transaction statistics', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function loadAssetTrends({ force }: { force: boolean }): Promise<TransactionStatisticAssetTrendsResponseItem[]> {
        return new Promise((resolve, reject) => {
            services.getTransactionStatisticsAssetTrends({
                startTime: transactionStatisticsFilter.value.assetTrendsChartStartTime,
                endTime: transactionStatisticsFilter.value.assetTrendsChartEndTime
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                    return;
                }

                if (transactionStatisticsStateInvalid.value) {
                    updateTransactionStatisticsInvalidState(false);
                }

                if (force && data.result && isEquals(transactionAssetTrendsData.value, data.result)) {
                    reject({ message: 'Data is up to date', isUpToDate: true });
                    return;
                }

                transactionAssetTrendsData.value = data.result;

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to retrieve transaction statistics', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                } else {
                    reject(error);
                }
            });
        });
    }

    return {
        // 状态
        transactionStatisticsFilter,
        transactionCategoryStatisticsData,
        transactionCategoryTrendsData,
        transactionStatisticsStateInvalid,
        // 计算状态
        categoricalAnalysisChartDataCategory,
        categoricalOverviewAnalysisData,
        categoricalAnalysisData,
        trendsAnalysisData,
        assetTrendsData,
        // 函数
        updateTransactionStatisticsInvalidState,
        resetTransactionStatistics,
        initTransactionStatisticsFilter,
        updateTransactionStatisticsFilter,
        getTransactionStatisticsPageParams,
        getTransactionListPageParams,
        loadCategoricalAnalysis,
        loadTrendAnalysis,
        loadAssetTrends
    };
});
