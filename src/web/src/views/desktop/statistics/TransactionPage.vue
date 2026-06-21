<template src="./transaction/TransactionPage.template.html"></template>

<script setup lang="ts">
import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import SnackBar from '@/components/desktop/SnackBar.vue';
import TrendsChart from '@/components/desktop/TrendsChart.vue';
import AccountFilterSettingsCard from '@/views/desktop/common/cards/AccountFilterSettingsCard.vue';
import CategoryFilterSettingsCard from '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue';
import TransactionTagFilterSettingsCard from '@/views/desktop/common/cards/TransactionTagFilterSettingsCard.vue';
import ExportDialog from '@/views/desktop/statistics/transaction/dialogs/ExportDialog.vue';
import {
    getFilterLinkUrl,
    getTransactionItemLinkUrl
} from '@/views/desktop/statistics/transaction/pageLinks.ts';

import { ref, computed, useTemplateRef, watch } from 'vue';
import { useRouter, onBeforeRouteUpdate } from 'vue-router';
import { useDisplay, useTheme } from 'vuetify';

import { useI18n } from '@/locales/helpers.ts';
import { useStatisticsTransactionPageBase } from '@/views/base/statistics/StatisticsTransactionPageBase.ts';

import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { type TransactionStatisticsPartialFilter, useStatisticsStore } from '@/stores/statistics.ts';

import type { TypeAndDisplayName } from '@/core/base.ts';
import { type TextualYearMonth, type TimeRangeAndDateType, DateRangeScene, DateRange } from '@/core/datetime.ts';
import { isDarkApplicationTheme } from '@/core/theme.ts';
import {
    ChartDataAggregationType,
    StatisticsAnalysisType,
    CategoricalChartType,
    ChartDataType,
    ChartSortingType,
    ChartDateAggregationType
} from '@/core/statistics.ts';

import {
    isDefined,
    isString,
    isNumber,
    arrayItemToObjectField
} from '@/lib/common.ts';
import {
    getGregorianCalendarYearAndMonthFromUnixTime,
    getYearMonthFirstUnixTime,
    getYearMonthLastUnixTime,
    getShiftedDateRangeAndDateType,
    getDateTypeByDateRange,
    getDateRangeByDateType
} from '@/lib/datetime.ts';

import {
    mdiCheck,
    mdiArrowLeft,
    mdiArrowRight,
    mdiCalendarRangeOutline,
    mdiRefresh,
    mdiSquareRounded,
    mdiMagnify,
    mdiMenu,
    mdiFilterOutline,
    mdiFilterCogOutline,
    mdiExport,
    mdiDotsVertical
} from '@mdi/js';

type SnackBarType = InstanceType<typeof SnackBar>;
type TrendsChartType = InstanceType<typeof TrendsChart>;
type ExportDialogType = InstanceType<typeof ExportDialog>;

interface TransactionStatisticsProps {
    initAnalysisType?: string,
    initChartDataType?: string,
    initChartType?: string,
    initChartDateType?: string,
    initStartTime?: TextualYearMonth | '',
    initEndTime?: TextualYearMonth | '',
    initFilterAccountIds?: string,
    initFilterCategoryIds?: string,
    initTagIds?: string,
    initTagFilterType?: string,
    initKeyword?: string;
    initSortingType?: string,
    initTrendDateAggregationType?: string
    initAssetTrendsDateAggregationType?: string
}

const props = defineProps<TransactionStatisticsProps>();

const router = useRouter();
const display = useDisplay();
const theme = useTheme();

const {
    tt,
    getAllCategoricalChartTypes,
    getAllTrendChartTypes,
    formatAmountToWesternArabicNumeralsWithoutDigitGrouping,
    formatPercentToLocalizedNumerals
} = useI18n();

const {
    loading,
    analysisType,
    trendDateAggregationType,
    assetTrendsDateAggregationType,
    defaultCurrency,
    firstDayOfWeek,
    fiscalYearStart,
    allDateRanges,
    allSortingTypes,
    allTrendAnalysisDateAggregationTypes,
    allAssetTrendsDateAggregationTypes,
    query,
    queryChartDataCategory,
    queryDateType,
    queryStartTime,
    queryEndTime,
    queryDateRangeName,
    queryTrendDateAggregationTypeName,
    queryAssetTrendsDateAggregationTypeName,
    canChangeDateRange,
    canShiftDateRange,
    canUseCategoryFilter,
    canUseTagFilter,
    canUseKeywordFilter,
    showAmountInChart,
    totalAmountName,
    showPercentInCategoricalChart,
    showTotalAmountInTrendsChart,
    showStackedInTrendsChart,
    translateNameInTrendsChart,
    categoricalOverviewAnalysisData,
    categoricalAnalysisData,
    trendsAnalysisData,
    assetTrendsData,
    canShowCustomDateRange,
    getTransactionCategoricalAnalysisDataItemDisplayColor,
    getDisplayAmount
} = useStatisticsTransactionPageBase();

const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const statisticsStore = useStatisticsStore();

const snackbar = useTemplateRef<SnackBarType>('snackbar');
const monthlyTrendsChart = useTemplateRef<TrendsChartType>('monthlyTrendsChart');
const dailyTrendsChart = useTemplateRef<TrendsChartType>('dailyTrendsChart');
const exportDialog = useTemplateRef<ExportDialogType>('exportDialog');

const activeTab = ref<string>('statisticsPage');
const initing = ref<boolean>(true);
const filterKeyword = ref<string>('');
const alwaysShowNav = ref<boolean>(display.mdAndUp.value);
const showNav = ref<boolean>(display.mdAndUp.value);
const showCustomDateRangeDialog = ref<boolean>(false);
const showCustomMonthRangeDialog = ref<boolean>(false);
const showFilterAccountDialog = ref<boolean>(false);
const showFilterCategoryDialog = ref<boolean>(false);
const showFilterTagDialog = ref<boolean>(false);

const isDarkMode = computed<boolean>(() => isDarkApplicationTheme(theme.global.name.value));

const statisticsDataHasData = computed<boolean>(() => {
    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
        return !!categoricalAnalysisData.value && !!categoricalAnalysisData.value.items && categoricalAnalysisData.value.items.length > 0;
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
        return !!trendsAnalysisData.value && !!trendsAnalysisData.value.items && trendsAnalysisData.value.items.length > 0 && !!monthlyTrendsChart.value;
    } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
        return !!assetTrendsData.value && !!assetTrendsData.value.items && assetTrendsData.value.items.length > 0 && !!dailyTrendsChart.value;
    }

    return false;
});

const allChartTypes = computed<TypeAndDisplayName[]>(() => {
    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
        return getAllCategoricalChartTypes(true);
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
        return getAllTrendChartTypes();
    } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
        return getAllTrendChartTypes();
    } else {
        return [];
    }
});

const queryAnalysisType = computed<StatisticsAnalysisType>({
    get: () => analysisType.value,
    set: (value: number) => {
        setAnalysisType(value);
    }
});

const queryChartType = computed<number | undefined>({
    get: () => {
        if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
            return query.value.categoricalChartType;
        } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
            return query.value.trendChartType;
        } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
            return query.value.assetTrendsChartType;
        } else {
            return undefined;
        }
    },
    set: (value: number | undefined) => {
        setChartType(value);
    }
});

const queryChartDataType = computed<number>({
    get: () => query.value.chartDataType,
    set: (value: number) => {
        setChartDataType(value);
    }
});

const querySortingType = computed<number>({
    get: () => query.value.sortingType,
    set: (value: number) => {
        setSortingType(value);
    }
});

const isQuerySpecialChartType = computed<boolean>(() => {
    return ChartDataType.valueOf(queryChartDataType.value)?.specialChart ?? false;
});

const statisticsTextColor = computed<string>(() => {
    if (query.value.chartDataType === ChartDataType.OutflowsByAccount.type ||
        query.value.chartDataType === ChartDataType.ExpenseByAccount.type ||
        query.value.chartDataType === ChartDataType.ExpenseByPrimaryCategory.type ||
        query.value.chartDataType === ChartDataType.ExpenseBySecondaryCategory.type) {
        return 'text-expense';
    } else if (query.value.chartDataType === ChartDataType.InflowsByAccount.type ||
        query.value.chartDataType === ChartDataType.IncomeByAccount.type ||
        query.value.chartDataType === ChartDataType.IncomeByPrimaryCategory.type ||
        query.value.chartDataType === ChartDataType.IncomeBySecondaryCategory.type) {
        return 'text-income';
    } else {
        return 'text-default';
    }
});

function init(initProps: TransactionStatisticsProps): void {
    let needReload = !isDefined(initProps.initAnalysisType);

    const filter: TransactionStatisticsPartialFilter = {
        chartDataType: initProps.initChartDataType ? parseInt(initProps.initChartDataType) : undefined,
        filterAccountIds: initProps.initFilterAccountIds ? arrayItemToObjectField(initProps.initFilterAccountIds.split(','), true) : {},
        filterCategoryIds: initProps.initFilterCategoryIds ? arrayItemToObjectField(initProps.initFilterCategoryIds.split(','), true) : {},
        tagIds: initProps.initTagIds,
        tagFilterType: initProps.initTagFilterType && parseInt(initProps.initTagFilterType) >= 0 ? parseInt(initProps.initTagFilterType) : undefined,
        keyword: initProps.initKeyword,
        sortingType: initProps.initSortingType ? parseInt(initProps.initSortingType) : undefined
    };

    filterKeyword.value = filter.keyword || '';

    if (initProps.initAnalysisType === StatisticsAnalysisType.CategoricalAnalysis.toString()) {
        filter.categoricalChartType = initProps.initChartType ? parseInt(initProps.initChartType) : undefined;
        filter.categoricalChartDateType = initProps.initChartDateType ? parseInt(initProps.initChartDateType) : undefined;
        filter.categoricalChartStartTime = initProps.initStartTime ? parseInt(initProps.initStartTime) : undefined;
        filter.categoricalChartEndTime = initProps.initEndTime ? parseInt(initProps.initEndTime) : undefined;

        if (filter.categoricalChartDateType !== query.value.categoricalChartDateType) {
            needReload = true;
        } else if (filter.categoricalChartDateType === DateRange.Custom.type) {
            if (filter.categoricalChartStartTime !== query.value.categoricalChartStartTime
                || filter.categoricalChartEndTime !== query.value.categoricalChartEndTime) {
                needReload = true;
            }
        }

        if (initProps.initAnalysisType !== analysisType.value.toString()) {
            analysisType.value = StatisticsAnalysisType.CategoricalAnalysis;
            needReload = true;
        }
    } else if (initProps.initAnalysisType === StatisticsAnalysisType.TrendAnalysis.toString()) {
        filter.trendChartType = initProps.initChartType ? parseInt(initProps.initChartType) : undefined;
        filter.trendChartDateType = initProps.initChartDateType ? parseInt(initProps.initChartDateType) : undefined;
        filter.trendChartStartYearMonth = initProps.initStartTime;
        filter.trendChartEndYearMonth = initProps.initEndTime;

        if (filter.trendChartDateType !== query.value.trendChartDateType) {
            needReload = true;
        } else if (filter.trendChartDateType === DateRange.Custom.type) {
            if (filter.trendChartStartYearMonth !== query.value.trendChartStartYearMonth
                || filter.trendChartEndYearMonth !== query.value.trendChartEndYearMonth) {
                needReload = true;
            }
        }

        if (initProps.initAnalysisType !== analysisType.value.toString()) {
            analysisType.value = StatisticsAnalysisType.TrendAnalysis;
            needReload = true;
        }

        if (initProps.initTrendDateAggregationType) {
            trendDateAggregationType.value = parseInt(initProps.initTrendDateAggregationType);
        }
    } else if (initProps.initAnalysisType === StatisticsAnalysisType.AssetTrends.toString()) {
        filter.assetTrendsChartType = initProps.initChartType ? parseInt(initProps.initChartType) : undefined;
        filter.assetTrendsChartDateType = initProps.initChartDateType ? parseInt(initProps.initChartDateType) : undefined;
        filter.assetTrendsChartStartTime = initProps.initStartTime ? parseInt(initProps.initStartTime) : undefined;
        filter.assetTrendsChartEndTime = initProps.initEndTime ? parseInt(initProps.initEndTime) : undefined;

        if (filter.assetTrendsChartDateType !== query.value.assetTrendsChartDateType) {
            needReload = true;
        } else if (filter.assetTrendsChartDateType === DateRange.Custom.type) {
            if (filter.assetTrendsChartStartTime !== query.value.assetTrendsChartStartTime
                || filter.assetTrendsChartEndTime !== query.value.assetTrendsChartEndTime) {
                needReload = true;
            }
        }

        if (initProps.initAnalysisType !== analysisType.value.toString()) {
            analysisType.value = StatisticsAnalysisType.AssetTrends;
            needReload = true;
        }

        if (initProps.initAssetTrendsDateAggregationType) {
            assetTrendsDateAggregationType.value = parseInt(initProps.initAssetTrendsDateAggregationType);
        }
    }

    if (!isDefined(initProps.initAnalysisType)) {
        analysisType.value = StatisticsAnalysisType.CategoricalAnalysis;
        statisticsStore.initTransactionStatisticsFilter(analysisType.value);
    } else {
        statisticsStore.initTransactionStatisticsFilter(analysisType.value, filter);
    }

    if (!needReload && !statisticsStore.transactionStatisticsStateInvalid) {
        loading.value = false;
        initing.value = false;
        return;
    }

    loading.value = true;
    initing.value = true;

    Promise.all([
        accountsStore.loadAllAccounts({force: false}),
        transactionCategoriesStore.loadAllCategories({force: false})
    ]).then(() => {
        if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
            return statisticsStore.loadCategoricalAnalysis({
                force: false
            }) as Promise<unknown>;
        } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
            return statisticsStore.loadTrendAnalysis({
                force: false
            }) as Promise<unknown>;
        } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
            return statisticsStore.loadAssetTrends({
                force: false
            }) as Promise<unknown>;
        } else {
            return Promise.reject('An error occurred');
        }
    }).then(() => {
        loading.value = false;
        initing.value = false;
    }).catch(error => {
        loading.value = false;
        initing.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function reload(force: boolean): Promise<unknown> | null {
    let dispatchPromise: Promise<unknown> | null = null;

    loading.value = true;

    if (query.value.chartDataType === ChartDataType.Overview.type ||
        query.value.chartDataType === ChartDataType.OutflowsByAccount.type ||
        query.value.chartDataType === ChartDataType.ExpenseByAccount.type ||
        query.value.chartDataType === ChartDataType.ExpenseByPrimaryCategory.type ||
        query.value.chartDataType === ChartDataType.ExpenseBySecondaryCategory.type ||
        query.value.chartDataType === ChartDataType.InflowsByAccount.type ||
        query.value.chartDataType === ChartDataType.IncomeByAccount.type ||
        query.value.chartDataType === ChartDataType.IncomeByPrimaryCategory.type ||
        query.value.chartDataType === ChartDataType.IncomeBySecondaryCategory.type ||
        query.value.chartDataType === ChartDataType.TotalOutflows.type ||
        query.value.chartDataType === ChartDataType.TotalExpense.type ||
        query.value.chartDataType === ChartDataType.TotalInflows.type ||
        query.value.chartDataType === ChartDataType.TotalIncome.type ||
        query.value.chartDataType === ChartDataType.NetCashFlow.type ||
        query.value.chartDataType === ChartDataType.NetIncome.type ||
        query.value.chartDataType === ChartDataType.NetWorth.type) {
        if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
            dispatchPromise = statisticsStore.loadCategoricalAnalysis({
                force: force
            });
        } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
            dispatchPromise = statisticsStore.loadTrendAnalysis({
                force: force
            });
        } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
            dispatchPromise = statisticsStore.loadAssetTrends({
                force: force
            });
        }
    } else if (query.value.chartDataType === ChartDataType.AccountTotalAssets.type ||
        query.value.chartDataType === ChartDataType.AccountTotalLiabilities.type) {
        if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
            dispatchPromise = accountsStore.loadAllAccounts({
                force: force
            });
        } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
            dispatchPromise = statisticsStore.loadAssetTrends({
                force: force
            });
        }
    }

    if (dispatchPromise) {
        dispatchPromise.then(() => {
            loading.value = false;

            if (force) {
                snackbar.value?.showMessage('Data has been updated');
            }
        }).catch(error => {
            loading.value = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    }

    return dispatchPromise;
}

function setAnalysisType(type: StatisticsAnalysisType): void {
    if (analysisType.value === type) {
        return;
    }

    if (!ChartDataType.isAvailableForAnalysisType(query.value.chartDataType, type)) {
        let defaultChartDataType: ChartDataType = ChartDataType.Default;

        if (type === StatisticsAnalysisType.AssetTrends) {
            defaultChartDataType = ChartDataType.DefaultForAssetTrends;
        }

        statisticsStore.updateTransactionStatisticsFilter({
            chartDataType: defaultChartDataType.type
        });
    }

    if (analysisType.value !== StatisticsAnalysisType.TrendAnalysis && type === StatisticsAnalysisType.TrendAnalysis) {
        trendDateAggregationType.value = ChartDateAggregationType.Default.type;
    } else if (analysisType.value !== StatisticsAnalysisType.AssetTrends && type === StatisticsAnalysisType.AssetTrends) {
        assetTrendsDateAggregationType.value = ChartDateAggregationType.Default.type;
    }

    analysisType.value = type;
    loading.value = true;
    statisticsStore.updateTransactionStatisticsInvalidState(true);
    router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
}

function setChartType(type?: number): void {
    let changed = false;

    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
        changed = statisticsStore.updateTransactionStatisticsFilter({
            categoricalChartType: type
        });
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
        changed = statisticsStore.updateTransactionStatisticsFilter({
            trendChartType: type
        });
    } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
        changed = statisticsStore.updateTransactionStatisticsFilter({
            assetTrendsChartType: type
        });
    }

    if (changed) {
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setChartDataType(type: number): void {
    const changed = statisticsStore.updateTransactionStatisticsFilter({
        chartDataType: type
    });

    if (changed) {
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setSortingType(type: number): void {
    if (type < ChartSortingType.Amount.type || type > ChartSortingType.Name.type) {
        return;
    }

    const changed = statisticsStore.updateTransactionStatisticsFilter({
        sortingType: type
    });

    if (changed) {
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setTrendDateAggregationType(type: number): void {
    const changed = trendDateAggregationType.value !== type;
    trendDateAggregationType.value = type;

    if (changed) {
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setAssetTrendsDateAggregationType(type: number): void {
    const changed = assetTrendsDateAggregationType.value !== type;
    assetTrendsDateAggregationType.value = type;

    if (changed) {
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setDateFilter(dateType: number): void {
    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
        if (dateType === DateRange.Custom.type) { // Custom
            showCustomDateRangeDialog.value = true;
            return;
        } else if (query.value.categoricalChartDateType === dateType) {
            return;
        }
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
        if (dateType === DateRange.Custom.type) { // Custom
            showCustomMonthRangeDialog.value = true;
            return;
        } else if (query.value.trendChartDateType === dateType) {
            return;
        }
    } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
        if (dateType === DateRange.Custom.type) { // Custom
            showCustomDateRangeDialog.value = true;
            return;
        } else if (query.value.assetTrendsChartDateType === dateType) {
            return;
        }
    }

    const dateRange = getDateRangeByDateType(dateType, firstDayOfWeek.value, fiscalYearStart.value);

    if (!dateRange) {
        return;
    }

    let changed = false;

    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
        changed = statisticsStore.updateTransactionStatisticsFilter({
            categoricalChartDateType: dateRange.dateType,
            categoricalChartStartTime: dateRange.minTime,
            categoricalChartEndTime: dateRange.maxTime
        });
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
        changed = statisticsStore.updateTransactionStatisticsFilter({
            trendChartDateType: dateRange.dateType,
            trendChartStartYearMonth: dateType === DateRange.All.type ? '' : getGregorianCalendarYearAndMonthFromUnixTime(dateRange.minTime),
            trendChartEndYearMonth: dateType === DateRange.All.type ? '' : getGregorianCalendarYearAndMonthFromUnixTime(dateRange.maxTime)
        });
    } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
        changed = statisticsStore.updateTransactionStatisticsFilter({
            assetTrendsChartDateType: dateRange.dateType,
            assetTrendsChartStartTime: dateRange.minTime,
            assetTrendsChartEndTime: dateRange.maxTime
        });
    }

    if (changed) {
        loading.value = true;
        statisticsStore.updateTransactionStatisticsInvalidState(true);
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setCustomDateFilter(startTime: number | TextualYearMonth, endTime: number | TextualYearMonth): void {
    if (!startTime || !endTime) {
        return;
    }

    let changed = false;

    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis && isNumber(startTime) && isNumber(endTime)) {
        const chartDateType = getDateTypeByDateRange(startTime, endTime, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal);

        changed = statisticsStore.updateTransactionStatisticsFilter({
            categoricalChartDateType: chartDateType,
            categoricalChartStartTime: startTime,
            categoricalChartEndTime: endTime
        });

        showCustomDateRangeDialog.value = false;
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis && isString(startTime) && isString(endTime)) {
        const chartDateType = getDateTypeByDateRange(getYearMonthFirstUnixTime(startTime), getYearMonthLastUnixTime(endTime), firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.TrendAnalysis);

        changed = statisticsStore.updateTransactionStatisticsFilter({
            trendChartDateType: chartDateType,
            trendChartStartYearMonth: startTime,
            trendChartEndYearMonth: endTime
        });

        showCustomMonthRangeDialog.value = false;
    } else if (analysisType.value === StatisticsAnalysisType.AssetTrends && isNumber(startTime) && isNumber(endTime)) {
        const chartDateType = getDateTypeByDateRange(startTime, endTime, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.AssetTrends);

        changed = statisticsStore.updateTransactionStatisticsFilter({
            assetTrendsChartDateType: chartDateType,
            assetTrendsChartStartTime: startTime,
            assetTrendsChartEndTime: endTime
        });

        showCustomDateRangeDialog.value = false;
    }

    if (changed) {
        loading.value = true;
        statisticsStore.updateTransactionStatisticsInvalidState(true);
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function shiftDateRange(scale: number): void {
    let changed = false;

    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
        if (query.value.categoricalChartDateType === DateRange.All.type) {
            return;
        }

        const newDateRange = getShiftedDateRangeAndDateType(query.value.categoricalChartStartTime, query.value.categoricalChartEndTime, scale, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal);

        changed = statisticsStore.updateTransactionStatisticsFilter({
            categoricalChartDateType: newDateRange.dateType,
            categoricalChartStartTime: newDateRange.minTime,
            categoricalChartEndTime: newDateRange.maxTime
        });
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
        if (query.value.trendChartDateType === DateRange.All.type) {
            return;
        }

        const newDateRange = getShiftedDateRangeAndDateType(getYearMonthFirstUnixTime(query.value.trendChartStartYearMonth), getYearMonthLastUnixTime(query.value.trendChartEndYearMonth), scale, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.TrendAnalysis);

        changed = statisticsStore.updateTransactionStatisticsFilter({
            trendChartDateType: newDateRange.dateType,
            trendChartStartYearMonth: getGregorianCalendarYearAndMonthFromUnixTime(newDateRange.minTime),
            trendChartEndYearMonth: getGregorianCalendarYearAndMonthFromUnixTime(newDateRange.maxTime)
        });
    } else if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
        if (query.value.assetTrendsChartDateType === DateRange.All.type) {
            return;
        }

        const newDateRange = getShiftedDateRangeAndDateType(query.value.assetTrendsChartStartTime, query.value.assetTrendsChartEndTime, scale, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.AssetTrends);

        changed = statisticsStore.updateTransactionStatisticsFilter({
            assetTrendsChartDateType: newDateRange.dateType,
            assetTrendsChartStartTime: newDateRange.minTime,
            assetTrendsChartEndTime: newDateRange.maxTime
        });
    }

    if (changed) {
        loading.value = true;
        statisticsStore.updateTransactionStatisticsInvalidState(true);
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setAccountFilter(changed: boolean): void {
    showFilterAccountDialog.value = false;

    if (changed) {
        loading.value = true;
        statisticsStore.updateTransactionStatisticsInvalidState(true);
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setCategoryFilter(changed: boolean): void {
    showFilterCategoryDialog.value = false;

    if (changed) {
        loading.value = true;
        statisticsStore.updateTransactionStatisticsInvalidState(true);
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setTagFilter(changed: boolean): void {
    showFilterTagDialog.value = false;

    if (changed) {
        loading.value = true;
        statisticsStore.updateTransactionStatisticsInvalidState(true);
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function setKeywordFilter(keyword: string): void {
    if (analysisType.value === StatisticsAnalysisType.AssetTrends) {
        return;
    }

    if (query.value.keyword === keyword) {
        return;
    }

    let changed = false;

    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis) {
        changed = statisticsStore.updateTransactionStatisticsFilter({
            keyword: keyword
        });
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis) {
        changed = statisticsStore.updateTransactionStatisticsFilter({
            keyword: keyword
        });
    }

    if (changed) {
        loading.value = true;
        statisticsStore.updateTransactionStatisticsInvalidState(true);
        router.push(getFilterLinkUrl(statisticsStore, analysisType.value, trendDateAggregationType.value, assetTrendsDateAggregationType.value));
    }
}

function exportResults(): void {
    if (analysisType.value === StatisticsAnalysisType.CategoricalAnalysis && categoricalAnalysisData.value && categoricalAnalysisData.value.items) {
        exportDialog.value?.open({
            headers: [
                tt('Name'),
                tt('Amount') + ` (${defaultCurrency.value})`,
                tt('Proportion (%)')
            ],
            data: categoricalAnalysisData.value.items
                .filter(item => !item.hidden)
                .map(item => [
                    item.name,
                    formatAmountToWesternArabicNumeralsWithoutDigitGrouping(item.totalAmountCents),
                    item.percent.toFixed(4)
                ])
        });
    } else if (analysisType.value === StatisticsAnalysisType.TrendAnalysis && trendsAnalysisData.value && trendsAnalysisData.value.items && monthlyTrendsChart.value) {
        const exportData = monthlyTrendsChart.value.exportData();
        exportDialog.value?.open({
            headers: exportData.headers || [],
            data: exportData.data || []
        });
    } else if (analysisType.value === StatisticsAnalysisType.AssetTrends && assetTrendsData.value && assetTrendsData.value.items && dailyTrendsChart.value) {
        const exportData = dailyTrendsChart.value.exportData();
        exportDialog.value?.open({
            headers: exportData.headers || [],
            data: exportData.data || []
        });
    }
}

function onClickSankeyChartItem(sourceItemType: 'account' | 'category', sourceItemId: string, targetItemType?: 'account' | 'category', targetItemId?: string): void {
    if (sourceItemType === 'category' && targetItemType === 'category' && sourceItemId && targetItemId) {
        const sourceCategory = transactionCategoriesStore.allTransactionCategoriesMap[sourceItemId];
        const targetCategory = transactionCategoriesStore.allTransactionCategoriesMap[targetItemId];

        if (sourceCategory?.parentId === targetCategory?.id) {
            router.push(getTransactionItemLinkUrl(statisticsStore, analysisType.value, `${sourceItemType}:${sourceItemId}`));
            return;
        } else if (targetCategory?.parentId === sourceCategory?.id) {
            router.push(getTransactionItemLinkUrl(statisticsStore, analysisType.value, `${targetItemType}:${targetItemId}`));
            return;
        }
    }

    router.push(getTransactionItemLinkUrl(statisticsStore, analysisType.value, `${sourceItemType}:${sourceItemId}` + (targetItemType && targetItemId ? `-${targetItemType}:${targetItemId}` : '')));
}

function onClickPieChartItem(item: Record<string, unknown>): void {
    router.push(getTransactionItemLinkUrl(statisticsStore, analysisType.value, item['id'] as string));
}

function onClickTrendChartItem(item: { itemId: string, dateRange: TimeRangeAndDateType }): void {
    router.push(getTransactionItemLinkUrl(statisticsStore, analysisType.value, item.itemId, item.dateRange));
}

function onShowDateRangeError(message: string): void {
    snackbar.value?.showError(message);
}

onBeforeRouteUpdate((to) => {
    if (to.query) {
        init({
            initAnalysisType: (to.query['analysisType'] as string | null) || undefined,
            initChartDataType: (to.query['chartDataType'] as string | null) || undefined,
            initChartType: (to.query['chartType'] as string | null) || undefined,
            initChartDateType: (to.query['chartDateType'] as string | null) || undefined,
            initStartTime: (to.query['startTime'] as TextualYearMonth | null) || undefined,
            initEndTime: (to.query['endTime'] as TextualYearMonth | null) || undefined,
            initFilterAccountIds: (to.query['filterAccountIds'] as string | null) || undefined,
            initFilterCategoryIds: (to.query['filterCategoryIds'] as string | null) || undefined,
            initTagIds: (to.query['tagIds'] as string | null) || undefined,
            initTagFilterType: (to.query['tagFilterType'] as string | null) || undefined,
            initKeyword: (to.query['keyword'] as string | null) || undefined,
            initSortingType: (to.query['sortingType'] as string | null) || undefined,
            initTrendDateAggregationType: (to.query['trendDateAggregationType'] as string | null) || undefined,
            initAssetTrendsDateAggregationType: (to.query['assetTrendsDateAggregationType'] as string | null) || undefined
        });
    } else {
        init({});
    }
});

watch(() => display.mdAndUp.value, (newValue) => {
    alwaysShowNav.value = newValue;

    if (!showNav.value) {
        showNav.value = newValue;
    }
});

init(props);
useExternalTemplateBindings(AccountFilterSettingsCard, accountsStore, activeTab, allAssetTrendsDateAggregationTypes, allChartTypes, allDateRanges, allSortingTypes, allTrendAnalysisDateAggregationTypes, alwaysShowNav, analysisType, arrayItemToObjectField, assetTrendsData, assetTrendsDateAggregationType, canChangeDateRange, canShiftDateRange, canShowCustomDateRange, canUseCategoryFilter, canUseKeywordFilter, canUseTagFilter, categoricalAnalysisData, CategoricalChartType, categoricalOverviewAnalysisData, CategoryFilterSettingsCard, ChartDataAggregationType, ChartDataType, ChartDateAggregationType, ChartSortingType, computed, dailyTrendsChart, DateRange, DateRangeScene, defaultCurrency, display, exportDialog, ExportDialog, exportResults, filterKeyword, firstDayOfWeek, fiscalYearStart, formatAmountToWesternArabicNumeralsWithoutDigitGrouping, formatPercentToLocalizedNumerals, getAllCategoricalChartTypes, getAllTrendChartTypes, getDateRangeByDateType, getDateTypeByDateRange, getDisplayAmount, getFilterLinkUrl, getGregorianCalendarYearAndMonthFromUnixTime, getShiftedDateRangeAndDateType, getTransactionCategoricalAnalysisDataItemDisplayColor, getTransactionItemLinkUrl, getYearMonthFirstUnixTime, getYearMonthLastUnixTime, init, initing, isDarkApplicationTheme, isDarkMode, isDefined, isNumber, isQuerySpecialChartType, isString, loading, mdiArrowLeft, mdiArrowRight, mdiCalendarRangeOutline, mdiCheck, mdiDotsVertical, mdiExport, mdiFilterCogOutline, mdiFilterOutline, mdiMagnify, mdiMenu, mdiRefresh, mdiSquareRounded, monthlyTrendsChart, onBeforeRouteUpdate, onClickPieChartItem, onClickSankeyChartItem, onClickTrendChartItem, onShowDateRangeError, props, query, queryAnalysisType, queryAssetTrendsDateAggregationTypeName, queryChartDataCategory, queryChartDataType, queryChartType, queryDateRangeName, queryDateType, queryEndTime, querySortingType, queryStartTime, queryTrendDateAggregationTypeName, ref, reload, router, setAccountFilter, setAnalysisType, setAssetTrendsDateAggregationType, setCategoryFilter, setChartDataType, setChartType, setCustomDateFilter, setDateFilter, setKeywordFilter, setSortingType, setTagFilter, setTrendDateAggregationType, shiftDateRange, showAmountInChart, showCustomDateRangeDialog, showCustomMonthRangeDialog, showFilterAccountDialog, showFilterCategoryDialog, showFilterTagDialog, showNav, showPercentInCategoricalChart, showStackedInTrendsChart, showTotalAmountInTrendsChart, snackbar, SnackBar, StatisticsAnalysisType, statisticsDataHasData, statisticsStore, statisticsTextColor, theme, totalAmountName, transactionCategoriesStore, TransactionTagFilterSettingsCard, translateNameInTrendsChart, trendDateAggregationType, trendsAnalysisData, TrendsChart, tt, useAccountsStore, useDisplay, useI18n, useRouter, useStatisticsStore, useStatisticsTransactionPageBase, useTemplateRef, useTheme, useTransactionCategoriesStore, watch);
</script>

<style src="./transaction/TransactionPage.css"></style>
