<template src="./list-page/ListPage.template.html"></template>

<script setup lang="ts">import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import type { Router } from 'framework7/types';

import { useI18n } from '@/locales/helpers.ts';
import {
    type Framework7Dom,
    useI18nUIComponents,
    showLoading,
    hideLoading,
    onSwipeoutDeleted,
    scrollToSelectedItem,
    onInfiniteScrolling
} from '@/lib/ui/mobile.ts';
import { TransactionListPageType, useTransactionListPageBase } from '@/views/base/transactions/TransactionListPageBase.ts';
import MobileTransactionMonthBlock from './components/MobileTransactionMonthBlock.vue';
import { useMobileTransactionMonthList } from './useMobileTransactionMonthList.ts';
import { buildMobileTransactionAddPath } from './list-page/addRoute.ts';
import { getCategoryListItemCheckedClass } from './list-page/filterDisplay.ts';

import { useEnvironmentsStore } from '@/stores/environment.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import {
    type TransactionListPartialFilter,
    type TransactionMonthList,
    useTransactionsStore
} from '@/stores/transaction.ts';

import { type TypeAndDisplayName, keys } from '@/core/base.ts';
import { TextDirection } from '@/core/text.ts';
import {
    type Year0BasedMonth,
    type TimeRangeAndDateType,
    DateRangeScene,
    DateRange
} from '@/core/datetime.ts';
import { AmountFilterType } from '@/core/numeral.ts';
import { TransactionType } from '@/core/transaction.ts';
import type { Transaction } from '@/models/transaction.ts';

import {
    getCurrentUnixTime,
    parseDateTimeFromUnixTime,
    getBrowserTimezoneOffsetMinutes,
    getActualUnixTimeForStore,
    getDayFirstUnixTimeBySpecifiedUnixTime,
    getYearMonthFirstUnixTime,
    getYearMonthLastUnixTime,
    getShiftedDateRangeAndDateType,
    getShiftedDateRangeAndDateTypeForBillingCycle,
    getDateTypeByDateRange,
    getDateTypeByBillingCycleDateRange,
    getDateRangeByDateType,
    getDateRangeByBillingCycleDateType,
    getFullMonthDateRange,
    getValidMonthDayOrCurrentDayShortDate
} from '@/lib/datetime.ts';
import {
    categoryTypeToTransactionType,
    transactionTypeToCategoryType
} from '@/lib/category.ts';
import logger from '@/lib/logger.ts';

const props = defineProps<{
    f7route: Router.Route;
    f7router: Router.Router;
}>();

const {
    tt,
    getCurrentLanguageTextDirection,
    getAllTransactionTagFilterTypes
} = useI18n();

const { showAlert, showToast, routeBackOnError } = useI18nUIComponents();

const {
    pageType,
    loading,
    customMinDatetime,
    customMaxDatetime,
    currentCalendarDate,
    currentTimezoneOffsetMinutes,
    firstDayOfWeek,
    fiscalYearStart,
    defaultCurrency,
    showTotalAmountInTransactionListPage,
    showTagInTransactionListPage,
    allDateRanges,
    allAccounts,
    allAccountsMap,
    allAvailableAccountsCount,
    allCategories,
    allPrimaryCategories,
    allAvailableCategoriesCount,
    allTransactionTags,
    allAvailableTagsCount,
    displayPageTypeName,
    query,
    queryDateRangeName,
    queryMinTime,
    queryMaxTime,
    queryMonthlyData,
    queryMonth,
    queryAllFilterCategoryIds,
    queryAllFilterAccountIds,
    queryAllFilterTagIds,
    queryAllFilterCategoryIdsCount,
    queryAllFilterAccountIdsCount,
    queryAllFilterTagIdsCount,
    queryAccountName,
    queryCategoryName,
    queryAmount,
    transactionCalendarMinDate,
    transactionCalendarMaxDate,
    currentMonthTransactionData,
    canAddTransaction,
    getDisplayTime,
    getDisplayLongYearMonth,
    getDisplayTimezone,
    getDisplayAmount,
    getDisplayMonthTotalAmount,
    getTransactionTypeName,
} = useTransactionListPageBase();

const environmentsStore = useEnvironmentsStore();
const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const transactionTagsStore = useTransactionTagsStore();
const transactionsStore = useTransactionsStore();

const loadingError = ref<unknown | null>(null);
const loadingMore = ref<boolean>(false);
const transactionToDelete = ref<Transaction | null>(null);
const showTransactionListPageTypePopover = ref<boolean>(false);
const showDatePopover = ref<boolean>(false);
const showCategoryPopover = ref<boolean>(false);
const showAccountPopover = ref<boolean>(false);
const showMorePopover = ref<boolean>(false);
const showCustomDateRangeSheet = ref<boolean>(false);
const showCustomMonthSheet = ref<boolean>(false);
const showDeleteActionSheet = ref<boolean>(false);

const textDirection = computed<TextDirection>(() => getCurrentLanguageTextDirection());
const isDarkMode = computed<boolean>(() => environmentsStore.framework7DarkMode || false);

const allTransactionTagFilterTypes = computed<TypeAndDisplayName[]>(() => getAllTransactionTagFilterTypes());

const transactions = computed<TransactionMonthList[]>(() => {
    if (loading.value) {
        return [];
    }

    if (pageType.value === TransactionListPageType.List.type) {
        return transactionsStore.transactions;
    } else if (pageType.value === TransactionListPageType.Calendar.type) {
        if (queryMonthlyData.value) {
            const transactionData = currentMonthTransactionData.value;

            if (!transactionData || !transactionData.items) {
                return [];
            }

            const transactions :Transaction[] = [];

            for (const transaction of transactionData.items) {
                if (transaction.gregorianCalendarYearDashMonthDashDay === currentCalendarDate.value) {
                    transactions.push(transaction);
                }
            }

            const dailyTransactionList: TransactionMonthList = {
                year: currentMonthTransactionData.value.year,
                month: currentMonthTransactionData.value.month,
                yearDashMonth: currentMonthTransactionData.value.yearDashMonth,
                opened: true,
                items: transactions,
                totalAmountCents: {
                    incomeCents: 0,
                    expenseCents: 0,
                    incompleteIncome: false,
                    incompleteExpense: false
                },
                dailyTotalAmountsCents: {}
            };

            return [dailyTransactionList];
        } else {
            return [];
        }
    } else {
        return [];
    }
});

const {
    transactionInvisibleYearMonths,
    resetTransactionMonthListState,
    getTransactionMonthTitleDomId,
    getTransactionMonthListDomId,
    getTransactionDomId,
    isTransactionMonthListInvisible,
    getTransactionMonthListHeight,
    setTransactionMonthListHeights,
    setTransactionInvisibleYearMonthList,
    getTransactionDateStyle
} = useMobileTransactionMonthList(transactions);

const noTransaction = computed<boolean>(() => {
    if (pageType.value === TransactionListPageType.List.type) {
        return transactionsStore.noTransaction;
    } else if (pageType.value === TransactionListPageType.Calendar.type) {
        return !transactions.value || !transactions.value.length || !transactions.value[0]!.items || !transactions.value[0]!.items.length;
    } else {
        return true;
    }
});

const hasMoreTransaction = computed<boolean>(() => transactionsStore.hasMoreTransaction);

function init(): void { // 从路由 query、账期设置和缓存状态恢复移动交易列表。
    const initQuery = props.f7route.query;

    let dateRange: TimeRangeAndDateType | null = getDateRangeByDateType(initQuery['dateType'] ? parseInt(initQuery['dateType']) : undefined, firstDayOfWeek.value, fiscalYearStart.value);

    if (!dateRange && initQuery['dateType'] && initQuery['maxTime'] && initQuery['minTime'] &&
        (DateRange.isBillingCycle(parseInt(initQuery['dateType'])) || initQuery['dateType'] === DateRange.Custom.type.toString()) &&
        parseInt(initQuery['maxTime']) > 0 && parseInt(initQuery['minTime']) > 0) {
        dateRange = {
            dateType: parseInt(initQuery['dateType']),
            maxTime: parseInt(initQuery['maxTime']),
            minTime: parseInt(initQuery['minTime'])
        };
    }

    transactionsStore.initTransactionListFilter({
        dateType: dateRange ? dateRange.dateType : undefined,
        maxTime: dateRange ? dateRange.maxTime : undefined,
        minTime: dateRange ? dateRange.minTime : undefined,
        type: initQuery['type'] && parseInt(initQuery['type']) > 0 ? parseInt(initQuery['type']) : undefined,
        categoryIds: initQuery['categoryIds'],
        accountIds: initQuery['accountIds'],
        flowDirection: initQuery['flowDirection'],
        tagIds: initQuery['tagIds'],
        tagFilterType: initQuery['tagFilterType'] && parseInt(initQuery['tagFilterType']) >= 0 ? parseInt(initQuery['tagFilterType']) : undefined,
        keyword: initQuery['keyword']
    });

    reload();
}

function reload(done?: () => void): void { // 重新加载移动交易列表第一页，并重建按月分组和折叠高度。
    const force = !!done;

    if (!done) {
        loading.value = true;
    }

    resetTransactionMonthListState();

    Promise.all([
        accountsStore.loadAllAccounts({ force: false }),
        transactionCategoriesStore.loadAllCategories({ force: false }),
        transactionTagsStore.loadAllTags({ force: false })
    ]).then(() => {
        if (queryMonthlyData.value) {
            const currentMonthMinDate = parseDateTimeFromUnixTime(query.value.minTime);
            const currentYear = currentMonthMinDate.getGregorianCalendarYear();
            const currentMonth = currentMonthMinDate.getGregorianCalendarMonth();

            return transactionsStore.loadMonthlyAllTransactions({
                year: currentYear,
                month: currentMonth,
                autoExpand: true,
                defaultCurrency: defaultCurrency.value
            });
        } else {
            return transactionsStore.loadTransactions({
                reload: true,
                autoExpand: true,
                defaultCurrency: defaultCurrency.value
            });
        }
    }).then(() => {
        done?.();

        if (force) {
            showToast('Data has been updated');
        }

        loading.value = false;
        setTransactionMonthListHeights(true);
    }).catch(error => {
        if (error.processed || done) {
            loading.value = false;
        }

        done?.();

        if (!error.processed) {
            if (!done) {
                loadingError.value = error;
            }

            showToast(error.message || error);
        }
    });
}

function loadMore(autoExpand: boolean): void { // 加载下一页交易数据，并按月份追加到现有分组。
    if (!hasMoreTransaction.value) {
        return;
    }

    if (loadingMore.value || loading.value) {
        return;
    }

    loadingMore.value = true;

    transactionsStore.loadTransactions({
        reload: false,
        autoExpand: autoExpand,
        defaultCurrency: defaultCurrency.value
    }).then(() => {
        loadingMore.value = false;
        setTransactionMonthListHeights(false);
    }).catch(error => {
        loadingMore.value = false;

        if (!error.processed) {
            showToast(error.message || error);
        }
    });
}

function changePageType(type: number): void {
    pageType.value = type;
    currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(query.value.minTime, currentCalendarDate.value);

    if (pageType.value === TransactionListPageType.Calendar.type) {
        const dateRange = getFullMonthDateRange(query.value.minTime, query.value.maxTime, firstDayOfWeek.value, fiscalYearStart.value);

        if (dateRange) {
            const changed = transactionsStore.updateTransactionListFilter({
                dateType: dateRange.dateType,
                maxTime: dateRange.maxTime,
                minTime: dateRange.minTime
            });

            if (changed) {
                currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(query.value.minTime, currentCalendarDate.value);
                reload();
            }
        }
    }
}

function changeDateFilter(dateType: number): void { // 切换移动端日期筛选，统一处理自然日期和账期日期范围。
    if (dateType === DateRange.Custom.type) { // Custom
        if (!query.value.minTime || !query.value.maxTime) {
            customMaxDatetime.value = getActualUnixTimeForStore(getCurrentUnixTime(), currentTimezoneOffsetMinutes.value, getBrowserTimezoneOffsetMinutes());
            customMinDatetime.value = getDayFirstUnixTimeBySpecifiedUnixTime(customMaxDatetime.value);
        } else {
            customMaxDatetime.value = query.value.maxTime;
            customMinDatetime.value = query.value.minTime;
        }

        if (pageType.value === TransactionListPageType.Calendar.type) {
            showCustomMonthSheet.value = true;
        } else {
            showCustomDateRangeSheet.value = true;
        }

        showDatePopover.value = false;
        return;
    } else if (query.value.dateType === dateType) {
        return;
    }

    let dateRange: TimeRangeAndDateType | null = null;

    if (DateRange.isBillingCycle(dateType)) {
        dateRange = getDateRangeByBillingCycleDateType(dateType, firstDayOfWeek.value, fiscalYearStart.value, accountsStore.getAccountStatementDate(query.value.accountIds));
    } else {
        dateRange = getDateRangeByDateType(dateType, firstDayOfWeek.value, fiscalYearStart.value);
    }

    if (!dateRange) {
        return;
    }

    if (pageType.value === TransactionListPageType.Calendar.type) {
        currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(dateRange.minTime, currentCalendarDate.value);
        const fullMonthDateRange = getFullMonthDateRange(dateRange.minTime, dateRange.maxTime, firstDayOfWeek.value, fiscalYearStart.value);

        if (fullMonthDateRange) {
            dateRange = fullMonthDateRange;
            currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(dateRange.minTime, currentCalendarDate.value);
        }
    }

    const changed = transactionsStore.updateTransactionListFilter({
        dateType: dateRange.dateType,
        maxTime: dateRange.maxTime,
        minTime: dateRange.minTime
    });

    showDatePopover.value = false;

    if (changed) {
        reload();
    }
}

function changeCustomDateFilter(minTime: number, maxTime: number): void {
    if (!minTime || !maxTime) {
        return;
    }

    let dateType: number | null = getDateTypeByBillingCycleDateRange(minTime, maxTime, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal, accountsStore.getAccountStatementDate(query.value.accountIds));

    if (!dateType) {
        dateType = getDateTypeByDateRange(minTime, maxTime, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal);
    }

    if (pageType.value === TransactionListPageType.Calendar.type) {
        currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(minTime, currentCalendarDate.value);
        const dateRange = getFullMonthDateRange(minTime, maxTime, firstDayOfWeek.value, fiscalYearStart.value);

        if (dateRange) {
            minTime = dateRange.minTime;
            maxTime = dateRange.maxTime;
            dateType = dateRange.dateType;
            currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(minTime, currentCalendarDate.value);
        }
    }

    const changed = transactionsStore.updateTransactionListFilter({
        dateType: dateType,
        maxTime: maxTime,
        minTime: minTime
    });

    showCustomDateRangeSheet.value = false;

    if (changed) {
        reload();
    }
}

function changeCustomMonthDateFilter(yearMonth: Year0BasedMonth): void {
    if (!yearMonth) {
        return;
    }

    const minTime = getYearMonthFirstUnixTime(yearMonth);
    const maxTime = getYearMonthLastUnixTime(yearMonth);
    const dateType = getDateTypeByDateRange(minTime, maxTime, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal);

    if (pageType.value === TransactionListPageType.Calendar.type) {
        currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(minTime, currentCalendarDate.value);
    }

    const changed = transactionsStore.updateTransactionListFilter({
        dateType: dateType,
        maxTime: maxTime,
        minTime: minTime
    });

    showCustomMonthSheet.value = false;

    if (changed) {
        reload();
    }
}

function shiftDateRange(minTime: number, maxTime: number, scale: number): void {
    if (query.value.dateType === DateRange.All.type) {
        return;
    }

    let newDateRange: TimeRangeAndDateType | null = null;

    if (DateRange.isBillingCycle(query.value.dateType) || query.value.dateType === DateRange.Custom.type) {
        newDateRange = getShiftedDateRangeAndDateTypeForBillingCycle(minTime, maxTime, scale, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal, accountsStore.getAccountStatementDate(query.value.accountIds));
    }

    if (!newDateRange) {
        newDateRange = getShiftedDateRangeAndDateType(minTime, maxTime, scale, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal);
    }

    if (pageType.value === TransactionListPageType.Calendar.type) {
        currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(newDateRange.minTime, currentCalendarDate.value);
        const fullMonthDateRange = getFullMonthDateRange(newDateRange.minTime, newDateRange.maxTime, firstDayOfWeek.value, fiscalYearStart.value);

        if (fullMonthDateRange) {
            newDateRange = fullMonthDateRange;
            currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(newDateRange.minTime, currentCalendarDate.value);
        }
    }

    const changed = transactionsStore.updateTransactionListFilter({
        dateType: newDateRange.dateType,
        maxTime: newDateRange.maxTime,
        minTime: newDateRange.minTime
    });

    if (changed) {
        reload();
    }
}

function changeTypeFilter(type: number): void { // 切换交易类型筛选并刷新移动列表。
    if (query.value.type === type) {
        return;
    }

    let newCategoryFilter = undefined;

    if (type && query.value.categoryIds) {
        newCategoryFilter = '';

        for (const categoryId of keys(queryAllFilterCategoryIds.value)) {
            const category = allCategories.value[categoryId];

            if (category && category.type === transactionTypeToCategoryType(type)) {
                if (newCategoryFilter.length > 0) {
                    newCategoryFilter += ',';
                }

                newCategoryFilter += categoryId;
            }
        }
    }

    const nextFilter: TransactionListPartialFilter = {
        type: type,
        categoryIds: newCategoryFilter
    };
    if (query.value.flowDirection) {
        nextFilter.flowDirection = '';
    }
    const changed = transactionsStore.updateTransactionListFilter(nextFilter);

    showMorePopover.value = false;

    if (changed) {
        reload();
    }
}

function changeCategoryFilter(categoryIds: string): void { // 切换分类筛选，保持移动端单选和多选入口共用同一 query 字段。
    if (query.value.categoryIds === categoryIds) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        categoryIds: categoryIds
    });

    showCategoryPopover.value = false;

    if (changed) {
        reload();
    }
}

function filterMultipleCategories(): void {
    let navigateUrl = '/settings/filter/category?type=transactionListCurrent';

    if (TransactionType.Income <= query.value.type && query.value.type <= TransactionType.Transfer) {
        navigateUrl += '&allowCategoryTypes=' + transactionTypeToCategoryType(query.value.type);
    }

    props.f7router.navigate(navigateUrl);
}

function changeAccountFilter(accountIds: string): void { // 切换账户筛选，保持隐藏账户展示与桌面端筛选语义一致。
    if (query.value.accountIds === accountIds) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        accountIds: accountIds
    });

    showAccountPopover.value = false;

    if (changed) {
        reload();
    }
}

function filterMultipleAccounts(): void {
    props.f7router.navigate('/settings/filter/account?type=transactionListCurrent');
}

function changeTagFilter(tagIds: string): void {
    if (query.value.tagIds === tagIds) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        tagIds: tagIds
    });

    showMorePopover.value = false;

    if (changed) {
        reload();
    }
}

function filterMultipleTags(): void {
    props.f7router.navigate('/settings/filter/tag?type=transactionListCurrent');
}

function changeTagFilterType(filterType: number): void {
    if (query.value.tagFilterType === filterType) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        tagFilterType: filterType
    });

    showMorePopover.value = false;

    if (changed) {
        reload();
    }
}

function changeKeywordFilter(keyword: string): void {
    if (query.value.keyword === keyword) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        keyword: keyword
    });

    if (changed) {
        reload();
    }
}

function changeAmountFilter(filterType: string): void { // 应用移动端金额筛选，并写回金额筛选类型和范围参数。
    if (query.value.amountFilterCents === filterType) {
        return;
    }

    if (filterType) {
        showMorePopover.value = false;
        props.f7router.navigate(`/transaction/filter/amount?type=${filterType}&value=${query.value.amountFilterCents}`);
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        amountFilterCents: filterType
    });

    showMorePopover.value = false;

    if (changed) {
        reload();
    }
}

function add(): void { // 进入移动端新增交易页，并把当前筛选上下文作为默认草稿来源。
    if (!canAddTransaction.value) {
        showToast('Select Account');
        showAccountPopover.value = true;
        return;
    }

    props.f7router.navigate(buildMobileTransactionAddPath(
        query.value,
        queryAllFilterCategoryIdsCount.value,
        queryAllFilterAccountIdsCount.value,
        getCurrentUnixTime()
    ));
}

function duplicate(transaction: Transaction): void { // 以当前交易为模板进入移动端复制页面。
    props.f7router.navigate(`/transaction/add?id=${transaction.id}&type=${transaction.type}`);
}

function edit(transaction: Transaction): void { // 进入移动端交易编辑页面。
    props.f7router.navigate(`/transaction/edit?id=${transaction.id}&type=${transaction.type}`);
}

function remove(transaction: Transaction | null, confirm: boolean): void { // 删除移动端交易，支持二次确认并在成功后刷新列表。
    if (!transaction) {
        showAlert('An error occurred');
        return;
    }

    if (!confirm) {
        transactionToDelete.value = transaction;
        showDeleteActionSheet.value = true;
        return;
    }

    showDeleteActionSheet.value = false;
    transactionToDelete.value = null;
    showLoading();

    transactionsStore.deleteTransaction({
        transaction: transaction,
        defaultCurrency: defaultCurrency.value,
        beforeResolve: (done) => {
            onSwipeoutDeleted(getTransactionDomId(transaction), done);
        }
    }).then(() => {
        hideLoading();
    }).catch(error => {
        hideLoading();

        if (!error.processed) {
            showToast(error.message || error);
        }
    });
}

function collapseTransactionMonthList(monthList: TransactionMonthList, collapse: boolean): void {
    transactionsStore.collapseMonthInTransactionList({
        monthList: monthList,
        collapse: collapse
    });

    if (!collapse && transactionInvisibleYearMonths.value[monthList.yearDashMonth]) {
        delete transactionInvisibleYearMonths.value[monthList.yearDashMonth];
    }
}

function onPopoverOpen(event: { $el: Framework7Dom }): void {
    scrollToSelectedItem(event.$el, '.popover-inner', 'li.list-item-selected');
}

function onPageAfterIn(): void {
    if (transactionsStore.transactionListStateInvalid && !loading.value) {
        reload();
    }

    routeBackOnError(props.f7router, loadingError);
}

function onResize(): void {
    setTransactionMonthListHeights(true)
        .then(() => {
            setTransactionMonthListHeights(false);
        });
}

function onScroll(): void { // 滚动时维护月份折叠可见性并触发无限加载。
    setTransactionInvisibleYearMonthList();
}

function onTransactionMonthListCollapseStateChanged(): void {
    setTransactionMonthListHeights(false)
        .then(() => {
            setTransactionInvisibleYearMonthList();
        });
}

onMounted(() => {
    window.addEventListener('resize', onResize);
    onInfiniteScrolling(onScroll);
});

onUnmounted(() => {
    window.removeEventListener('resize', onResize);
});

// 监听交易日历日期变化，自动重新加载数据
watch(currentCalendarDate, (newDate, oldDate) => {
    logger.debug('[ListPage] currentCalendarDate changed', { oldDate, newDate, pageType: pageType.value });
    // 日历模式下，日期变化时重新加载当天交易
    if (pageType.value === TransactionListPageType.Calendar.type && newDate !== oldDate) {
        logger.debug('[ListPage] Calendar date changed, reloading transactions', { date: newDate });
        reload();
    }
});

init();
useExternalTemplateBindings(accountsStore, add, allAccounts, allAccountsMap, allAvailableAccountsCount, allAvailableCategoriesCount, allAvailableTagsCount, allCategories, allDateRanges, allPrimaryCategories, allTransactionTagFilterTypes, allTransactionTags, AmountFilterType, buildMobileTransactionAddPath, canAddTransaction, categoryTypeToTransactionType, changeAccountFilter, changeAmountFilter, changeCategoryFilter, changeCustomDateFilter, changeCustomMonthDateFilter, changeDateFilter, changeKeywordFilter, changePageType, changeTagFilter, changeTagFilterType, changeTypeFilter, collapseTransactionMonthList, computed, currentCalendarDate, currentMonthTransactionData, currentTimezoneOffsetMinutes, customMaxDatetime, customMinDatetime, DateRange, DateRangeScene, defaultCurrency, displayPageTypeName, duplicate, edit, environmentsStore, filterMultipleAccounts, filterMultipleCategories, filterMultipleTags, firstDayOfWeek, fiscalYearStart, getActualUnixTimeForStore, getAllTransactionTagFilterTypes, getBrowserTimezoneOffsetMinutes, getCategoryListItemCheckedClass, getCurrentLanguageTextDirection, getCurrentUnixTime, getDateRangeByBillingCycleDateType, getDateRangeByDateType, getDateTypeByBillingCycleDateRange, getDateTypeByDateRange, getDayFirstUnixTimeBySpecifiedUnixTime, getDisplayAmount, getDisplayLongYearMonth, getDisplayMonthTotalAmount, getDisplayTime, getDisplayTimezone, getFullMonthDateRange, getShiftedDateRangeAndDateType, getShiftedDateRangeAndDateTypeForBillingCycle, getTransactionDateStyle, getTransactionDomId, getTransactionMonthListDomId, getTransactionMonthListHeight, getTransactionMonthTitleDomId, getTransactionTypeName, getValidMonthDayOrCurrentDayShortDate, getYearMonthFirstUnixTime, getYearMonthLastUnixTime, hasMoreTransaction, hideLoading, init, isDarkMode, isTransactionMonthListInvisible, keys, loading, loadingError, loadingMore, loadMore, logger, MobileTransactionMonthBlock, noTransaction, onInfiniteScrolling, onMounted, onPageAfterIn, onPopoverOpen, onResize, onScroll, onSwipeoutDeleted, onTransactionMonthListCollapseStateChanged, onUnmounted, pageType, parseDateTimeFromUnixTime, props, query, queryAccountName, queryAllFilterAccountIds, queryAllFilterAccountIdsCount, queryAllFilterCategoryIds, queryAllFilterCategoryIdsCount, queryAllFilterTagIds, queryAllFilterTagIdsCount, queryAmount, queryCategoryName, queryDateRangeName, queryMaxTime, queryMinTime, queryMonth, queryMonthlyData, ref, reload, remove, resetTransactionMonthListState, routeBackOnError, scrollToSelectedItem, setTransactionInvisibleYearMonthList, setTransactionMonthListHeights, shiftDateRange, showAccountPopover, showAlert, showCategoryPopover, showCustomDateRangeSheet, showCustomMonthSheet, showDatePopover, showDeleteActionSheet, showLoading, showMorePopover, showTagInTransactionListPage, showToast, showTotalAmountInTransactionListPage, showTransactionListPageTypePopover, textDirection, TextDirection, transactionCalendarMaxDate, transactionCalendarMinDate, transactionCategoriesStore, transactionInvisibleYearMonths, TransactionListPageType, transactions, transactionsStore, transactionTagsStore, transactionToDelete, TransactionType, transactionTypeToCategoryType, tt, useAccountsStore, useEnvironmentsStore, useI18n, useI18nUIComponents, useMobileTransactionMonthList, useTransactionCategoriesStore, useTransactionListPageBase, useTransactionsStore, useTransactionTagsStore, watch);
</script>

<style src="./list-page/ListPage.css"></style>
