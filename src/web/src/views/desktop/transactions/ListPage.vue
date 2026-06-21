<template src="./list/ListPage.template.html"></template>

<script setup lang="ts">import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import { VMenu } from 'vuetify/components/VMenu';
import PaginationButtons from '@/components/desktop/PaginationButtons.vue';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import EditDialog from './list/dialogs/EditDialog.vue';
import BatchManualEntryDialog from './list/dialogs/BatchManualEntryDialog.vue';
import AIImageRecognitionDialog from './list/dialogs/AIImageRecognitionDialog.vue';
import ImportDialog from './import/ImportDialog.vue';
import AccountFilterSettingsCard from '@/views/desktop/common/cards/AccountFilterSettingsCard.vue';
import CategoryFilterSettingsCard from '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue';
import TransactionTagFilterSettingsCard from '@/views/desktop/common/cards/TransactionTagFilterSettingsCard.vue';
import { TransactionEditPageType } from '@/views/base/transactions/TransactionEditPageBase.ts';

import { ref, computed, useTemplateRef, watch, nextTick } from 'vue';
import { useRouter, onBeforeRouteUpdate } from 'vue-router';
import { useDisplay, useTheme } from 'vuetify';

import { useI18n } from '@/locales/helpers.ts';
import { TransactionListPageType, useTransactionListPageBase } from '@/views/base/transactions/TransactionListPageBase.ts';

import { useSettingsStore } from '@/stores/setting.ts';
import { useUserStore } from '@/stores/user.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';
import { useTransactionTemplatesStore } from '@/stores/transactionTemplate.ts';
import { useDesktopPageStore } from '@/stores/desktopPage.ts';

import {
    type NameNumeralValue,
    type TypeAndDisplayName,
    keys
} from '@/core/base.ts';
import {
    type Year0BasedMonth,
    type LocalizedRecentMonthDateRange,
    type TimeRangeAndDateType,
    DateRangeScene,
    DateRange
} from '@/core/datetime.ts';
import { type NumeralSystem, AmountFilterType } from '@/core/numeral.ts';
import { isDarkApplicationTheme } from '@/core/theme.ts';
import { TransactionType, TransactionTagFilterType } from '@/core/transaction.ts';
import { TemplateType }  from '@/core/template.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import type { Transaction } from '@/models/transaction.ts';
import type { TransactionTemplate } from '@/models/transaction_template.ts';

import {
    isObject,
    isString,
    isNumber
} from '@/lib/common.ts';
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
    getRecentDateRangeIndex,
    getFullMonthDateRange,
    getValidMonthDayOrCurrentDayShortDate
} from '@/lib/datetime.ts';
import {
    categoryTypeToTransactionType,
    transactionTypeToCategoryType
} from '@/lib/category.ts';
import { isDataExportingEnabled, isDataImportingEnabled, isTransactionFromAIImageRecognitionEnabled } from '@/lib/server_settings.ts';
import { startDownloadFile } from '@/lib/ui/common.ts';
import { scrollToSelectedItem } from '@/lib/ui/desktop.ts';
import logger from '@/lib/logger.ts';

import {
    mdiMagnify,
    mdiCheck,
    mdiViewGridOutline,
    mdiBorderNoneVariant,
    mdiVectorArrangeBelow,
    mdiRefresh,
    mdiMenu,
    mdiMenuDown,
    mdiPencilBoxOutline,
    mdiArrowLeft,
    mdiArrowRight,
    mdiPlusBoxMultipleOutline,
    mdiCheckboxMultipleOutline,
    mdiMinusBoxMultipleOutline,
    mdiCloseBoxMultipleOutline,
    mdiPound,
    mdiMagicStaff,
    mdiTextBoxOutline
} from '@mdi/js';

interface TransactionListProps {
    initPageType?: string;
    initDateType?: string,
    initMaxTime?: string,
    initMinTime?: string,
    initType?: string,
    initCategoryIds?: string,
    initAccountIds?: string,
    initTagIds?: string,
    initTagFilterType?: string,
    initAmountFilterCents?: string,
    initKeyword?: string
}

const props = defineProps<TransactionListProps>();

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;
type EditDialogType = InstanceType<typeof EditDialog>;
type BatchManualEntryDialogType = InstanceType<typeof BatchManualEntryDialog>;
type AIImageRecognitionDialogType = InstanceType<typeof AIImageRecognitionDialog>;
type ImportDialogType = InstanceType<typeof ImportDialog>;

interface TransactionTemplateWithIcon {
    type: number;
    displayName: string;
    icon: string;
}

interface TransactionListDisplayTotalAmount {
    incomeText: string;
    expenseText: string;
}

const router = useRouter();
const display = useDisplay();
const theme = useTheme();

const {
    tt,
    getAllRecentMonthDateRanges,
    getAllTransactionTagFilterTypes,
    getWeekdayLongName,
    getCurrentNumeralSystemType
} = useI18n();

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
    query,
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
    queryTagName,
    queryAmount,
    transactionCalendarMinDate,
    transactionCalendarMaxDate,
    currentMonthTransactionData,
    canAddTransaction,
    getDisplayTime,
    getDisplayLongDate,
    getDisplayTimezone,
    getDisplayTimeInDefaultTimezone,
    getDisplayAmount,
    getDisplayMonthTotalAmount,
    getTransactionTypeName,
} = useTransactionListPageBase();

const settingsStore = useSettingsStore();
const userStore = useUserStore();
const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const transactionTagsStore = useTransactionTagsStore();
const transactionsStore = useTransactionsStore();
const transactionTemplatesStore = useTransactionTemplatesStore();
const desktopPageStore = useDesktopPageStore();

const tagFilterIconMap: Record<number, string> = {
    [TransactionTagFilterType.HasAny.type]: mdiPlusBoxMultipleOutline,
    [TransactionTagFilterType.HasAll.type]: mdiCheckboxMultipleOutline,
    [TransactionTagFilterType.NotHasAny.type]: mdiMinusBoxMultipleOutline,
    [TransactionTagFilterType.NotHasAll.type]: mdiCloseBoxMultipleOutline
};

const timeFilterMenu = useTemplateRef<VMenu>('timeFilterMenu');
const categoryFilterMenu = useTemplateRef<VMenu>('categoryFilterMenu');
const amountFilterCentsMenu = useTemplateRef<VMenu>('amountFilterCentsMenu');
const accountFilterMenu = useTemplateRef<VMenu>('accountFilterMenu');
const tagFilterMenu = useTemplateRef<VMenu>('tagFilterMenu');

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const editDialog = useTemplateRef<EditDialogType>('editDialog');
const batchManualEntryDialog = useTemplateRef<BatchManualEntryDialogType>('batchManualEntryDialog');
const aiImageRecognitionDialog = useTemplateRef<AIImageRecognitionDialogType>('aiImageRecognitionDialog');
const importDialog = useTemplateRef<ImportDialogType>('importDialog');

const activeTab = ref<string>('transactionPage');
const currentPage = ref<number>(1);
const temporaryCountPerPage = ref<number | null>(null);
const totalCount = ref<number>(1);
const searchKeyword = ref<string>('');
const currentAmountFilterType = ref<string>('');
const currentAmountFilterValue1 = ref<number>(0);
const currentAmountFilterValue2 = ref<number>(0);
const currentPageTransactions = ref<Transaction[]>([]);
const categoryMenuState = ref<boolean>(false);
const amountMenuState = ref<boolean>(false);
const exportingData = ref<boolean>(false);
const alwaysShowNav = ref<boolean>(display.mdAndUp.value);
const showNav = ref<boolean>(display.mdAndUp.value);
const showCustomDateRangeDialog = ref<boolean>(false);
const showCustomMonthDialog = ref<boolean>(false);
const showFilterAccountDialog = ref<boolean>(false);
const showFilterCategoryDialog = ref<boolean>(false);
const showFilterTagDialog = ref<boolean>(false);

const isDarkMode = computed<boolean>(() => isDarkApplicationTheme(theme.global.name.value));
const numeralSystem = computed<NumeralSystem>(() => getCurrentNumeralSystemType());

const allPageCounts = computed<NameNumeralValue[]>(() => {
    const pageCounts: NameNumeralValue[] = [];
    const availableCountPerPage: number[] = [ 5, 10, 15, 20, 25, 30, 50 ];

    for (const count of availableCountPerPage) {
        pageCounts.push({ value: count, name: numeralSystem.value.replaceWesternArabicDigitsToLocalizedDigits(count.toString()) });
    }

    return pageCounts;
});

const recentMonthDateRanges = computed<LocalizedRecentMonthDateRange[]>(() => getAllRecentMonthDateRanges(pageType.value === TransactionListPageType.List.type, true));

const allTransactionTemplates = computed<TransactionTemplate[]>(() => {
    const allTemplates = transactionTemplatesStore.allVisibleTemplates;
    return allTemplates[TemplateType.Normal.type] || [];
});

const allTransactionTagFilterTypes = computed<TransactionTemplateWithIcon[]>(() => {
    const allTagFilterTypes: TypeAndDisplayName[] = getAllTransactionTagFilterTypes();
    const allTagFilterTypesWithIcon: TransactionTemplateWithIcon[] = [];

    for (const tagFilterType of allTagFilterTypes) {
        allTagFilterTypesWithIcon.push({
            type: tagFilterType.type,
            displayName: tagFilterType.displayName,
            icon: tagFilterIconMap[tagFilterType.type] ?? ''
        });
    }

    return allTagFilterTypesWithIcon;
});

const allowCategoryTypes = computed<string>(() => {
    if (TransactionType.Income <= query.value.type && query.value.type <= TransactionType.Transfer) {
        return transactionTypeToCategoryType(query.value.type)?.toString() ?? '';
    }

    return '';
});

const transactions = computed<Transaction[]>(() => {
    if (pageType.value === TransactionListPageType.List.type) {
        if (queryMonthlyData.value) {
            const transactionData = currentMonthTransactionData.value;

            if (!transactionData || !transactionData.items) {
                return [];
            }

            const firstIndex = (currentPage.value - 1) * countPerPage.value;
            const lastIndex = currentPage.value * countPerPage.value;

            const result = transactionData.items.slice(firstIndex, lastIndex);
            return result;
        } else {
            return currentPageTransactions.value;
        }
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

            return transactions;
        } else {
            return [];
        }
    } else {
        return [];
    }
});

const recentDateRangeIndex = computed<number>({
    get: () => getRecentDateRangeIndex(recentMonthDateRanges.value, query.value.dateType, query.value.minTime, query.value.maxTime, firstDayOfWeek.value, fiscalYearStart.value),
    set: (value) => {
        if (value < 0 || value >= recentMonthDateRanges.value.length) {
            value = 0;
        }

        changeDateFilter(recentMonthDateRanges.value[value] as LocalizedRecentMonthDateRange);
    }
});

const queryPageType = computed<number>({
    get: () => pageType.value,
    set: (value) => changePageType(value)
});

const queryType = computed<number>({
    get: () => query.value.type,
    set: (value) => changeTypeFilter(value)
});

const queryAllSelectedFilterCategoryIds = computed<string>(() => {
    if (queryAllFilterCategoryIdsCount.value === 0) {
        return '';
    } else if (queryAllFilterCategoryIdsCount.value === 1) {
        return query.value.categoryIds;
    } else { // queryAllFilterCategoryIdsCount.value > 1
        return 'multiple';
    }
});

const queryAllSelectedFilterAccountIds = computed<string>(() => {
    if (queryAllFilterAccountIdsCount.value === 0) {
        return '';
    } else if (queryAllFilterAccountIdsCount.value === 1) {
        return query.value.accountIds;
    } else { // queryAllFilterAccountIdsCount.value > 1
        return 'multiple';
    }
});

const queryAllSelectedFilterTagIds = computed<string>(() => {
    if (queryAllFilterTagIdsCount.value === 0) {
        return '';
    } else if (queryAllFilterTagIdsCount.value === 1) {
        return query.value.tagIds;
    } else { // queryAllFilterTagIdsCount.value > 1
        return 'multiple';
    }
});

const countPerPage = computed<number>({
    get: () => {
        if (temporaryCountPerPage.value) {
            return temporaryCountPerPage.value;
        }

        return settingsStore.appSettings.itemsCountInTransactionListPage;
    },
    set: (value) => {
        const newTotalPageCount = Math.ceil(totalCount.value / value);

        if (currentPage.value > newTotalPageCount) {
            currentPage.value = newTotalPageCount;
        }

        temporaryCountPerPage.value = value;

        if (!queryMonthlyData.value) {
            reload(false, false);
        }
    }
});

const totalPageCount = computed<number>(() => Math.ceil(totalCount.value / countPerPage.value));

const paginationCurrentPage = computed<number>({
    get: () => currentPage.value,
    set: (value) => {
        currentPage.value = value;

        if (!queryMonthlyData.value) {
            reload(false, false);
        }
    }
});

const skeletonData = computed<number[]>(() => {
    const data: number[] = [];
    const totalCount = (pageType.value === TransactionListPageType.List.type ? countPerPage.value : 3);

    for (let i = 0; i < totalCount; i++) {
        data.push(i);
    }

    return data;
});

const currentMonthTotalAmount = computed<TransactionListDisplayTotalAmount | null>(() => {
    if (queryMonthlyData.value) {
        const transactionData = currentMonthTransactionData.value;

        if (!transactionData) {
            return null;
        }

        return {
            incomeText: getDisplayMonthTotalAmount(transactionData.totalAmountCents.incomeCents, defaultCurrency.value, '', transactionData.totalAmountCents.incompleteIncome),
            expenseText: getDisplayMonthTotalAmount(transactionData.totalAmountCents.expenseCents, defaultCurrency.value, '', transactionData.totalAmountCents.incompleteExpense)
        };
    } else {
        return null;
    }
});

function getCategoryListItemCheckedClass(category: TransactionCategory, queryCategoryIds: Record<string, boolean>): Record<string, boolean> {
    if (queryCategoryIds && queryCategoryIds[category.id]) {
        return {
            'list-item-selected': true,
            'has-children-item-selected': true
        };
    }

    if (category.subCategories) {
        for (const subCategory of category.subCategories) {
            if (queryCategoryIds && queryCategoryIds[subCategory.id]) {
                return {
                    'list-item-selected': true,
                    'has-children-item-selected': true
                };
            }
        }
    }

    return {};
}

function getAmountFilterParameterCount(filterType: string): number {
    const amountFilterCentsType = AmountFilterType.valueOf(filterType);
    return amountFilterCentsType ? amountFilterCentsType.paramCount : 0;
}

function updateUrlWhenChanged(changed: boolean): void { // 交易列表筛选条件改变后统一刷新 URL，保持刷新页面和导出请求能复用同一查询合同。
    if (changed) {
        loading.value = true;
        currentPageTransactions.value = [];
        transactionsStore.clearTransactions();
        currentPage.value = 1; // 重置到第一页
        router.push(`/transaction/list?${transactionsStore.getTransactionListPageParams(pageType.value)}`);
    }
}

function init(initProps: TransactionListProps): void { // 从路由 query 和用户设置恢复交易列表状态，是桌面列表的单一初始化入口。
    let dateRange: TimeRangeAndDateType | null = getDateRangeByDateType(initProps.initDateType ? parseInt(initProps.initDateType) : undefined, firstDayOfWeek.value, fiscalYearStart.value);

    if (!dateRange && initProps.initDateType && initProps.initMaxTime && initProps.initMinTime &&
        (DateRange.isBillingCycle(parseInt(initProps.initDateType)) || initProps.initDateType === DateRange.Custom.type.toString()) &&
        parseInt(initProps.initMaxTime) > 0 && parseInt(initProps.initMinTime) > 0) {
        dateRange = {
            dateType: parseInt(initProps.initDateType),
            maxTime: parseInt(initProps.initMaxTime),
            minTime: parseInt(initProps.initMinTime)
        };
    }

    transactionsStore.initTransactionListFilter({
        dateType: dateRange ? dateRange.dateType : undefined,
        maxTime: dateRange ? dateRange.maxTime : undefined,
        minTime: dateRange ? dateRange.minTime : undefined,
        type: initProps.initType && parseInt(initProps.initType) > 0 ? parseInt(initProps.initType) : undefined,
        categoryIds: initProps.initCategoryIds,
        accountIds: initProps.initAccountIds,
        tagIds: initProps.initTagIds,
        tagFilterType: initProps.initTagFilterType && parseInt(initProps.initTagFilterType) >= 0 ? parseInt(initProps.initTagFilterType) : undefined,
        amountFilterCents: initProps.initAmountFilterCents || '',
        keyword: initProps.initKeyword || ''
    });

    if (initProps.initPageType) {
        const type = TransactionListPageType.valueOf(parseInt(initProps.initPageType));

        if (type) {
            pageType.value = type.type;
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
                        updateUrlWhenChanged(changed);
                        return;
                    }
                }
            }
        }
    }

    searchKeyword.value = initProps.initKeyword || '';
    currentAmountFilterType.value = '';

    currentPage.value = 1;
    reload(false, true);

    transactionTemplatesStore.loadAllTemplates({
        templateType: TemplateType.Normal.type,
        force: false
    });
}

function reload(force: boolean, init: boolean): void { // 根据当前页模式、筛选和分页重新加载交易数据，并同步月度概览。
    loading.value = true;

    const page = currentPage.value;

    Promise.all([
        accountsStore.loadAllAccounts({ force: false }),
        transactionCategoriesStore.loadAllCategories({ force: false }),
        transactionTagsStore.loadAllTags({ force: false })
    ]).then(() => {
        if (init) {
            if (desktopPageStore.showAddTransactionDialogInTransactionList) {
                desktopPageStore.resetShowAddTransactionDialogInTransactionList();
                add();
            }
        }

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
                count: countPerPage.value,
                page: page,
                withCount: page <= 1,
                autoExpand: true,
                defaultCurrency: defaultCurrency.value
            });
        }
    }).then(data => {
        loading.value = false;
        currentPageTransactions.value = data && data.items && data.items.length ? data.items : [];

        if (page <= 1) {
            totalCount.value = data && data.totalCount ? data.totalCount : 1;
        }

        if (force) {
            snackbar.value?.showMessage('Data has been updated');
        }
    }).catch(error => {
        loading.value = false;
        currentPageTransactions.value = [];
        totalCount.value = 1;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function changePageType(type: number): void {
    pageType.value = type;
    currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(query.value.minTime, currentCalendarDate.value);

    if (pageType.value === TransactionListPageType.Calendar.type) {
        const dateRange = getFullMonthDateRange(query.value.minTime, query.value.maxTime, firstDayOfWeek.value, fiscalYearStart.value);

        if (dateRange) {
            transactionsStore.updateTransactionListFilter({
                dateType: dateRange.dateType,
                maxTime: dateRange.maxTime,
                minTime: dateRange.minTime
            });
            currentCalendarDate.value = getValidMonthDayOrCurrentDayShortDate(query.value.minTime, currentCalendarDate.value);
        }
    }

    updateUrlWhenChanged(true);
}

function changeDateFilter(dateRange: TimeRangeAndDateType | number | null): void { // 切换日期筛选并同步日历、账期和普通日期范围三类查询参数。
    if (dateRange === DateRange.Custom.type || (isObject(dateRange) && dateRange.dateType === DateRange.Custom.type && !dateRange.minTime && !dateRange.maxTime)) { // Custom
        if (!query.value.minTime || !query.value.maxTime) {
            customMaxDatetime.value = getActualUnixTimeForStore(getCurrentUnixTime(), currentTimezoneOffsetMinutes.value, getBrowserTimezoneOffsetMinutes());
            customMinDatetime.value = getDayFirstUnixTimeBySpecifiedUnixTime(customMaxDatetime.value);
        } else {
            customMaxDatetime.value = query.value.maxTime;
            customMinDatetime.value = query.value.minTime;
        }

        if (pageType.value === TransactionListPageType.Calendar.type) {
            showCustomMonthDialog.value = true;
        } else {
            showCustomDateRangeDialog.value = true;
        }

        return;
    }

    if (isNumber(dateRange)) {
        if (DateRange.isBillingCycle(dateRange)) {
            dateRange = getDateRangeByBillingCycleDateType(dateRange, firstDayOfWeek.value, fiscalYearStart.value, accountsStore.getAccountStatementDate(query.value.accountIds));
        } else {
            dateRange = getDateRangeByDateType(dateRange, firstDayOfWeek.value, fiscalYearStart.value);
        }
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

    if (query.value.dateType === dateRange.dateType && query.value.maxTime === dateRange.maxTime && query.value.minTime === dateRange.minTime) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        dateType: dateRange.dateType,
        maxTime: dateRange.maxTime,
        minTime: dateRange.minTime
    });

    // ⚠️ 关键修复: 时间筛选更改后立即重新加载数据
    if (changed) {
        reload(false, false);
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

    if (query.value.dateType === dateType && query.value.maxTime === maxTime && query.value.minTime === minTime) {
        showCustomDateRangeDialog.value = false;
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        dateType: dateType,
        maxTime: maxTime,
        minTime: minTime
    });

    showCustomDateRangeDialog.value = false;

    // ⚠️ 关键修复: 自定义日期筛选更改后立即重新加载数据
    if (changed) {
        reload(false, false);
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

    if (query.value.dateType === dateType && query.value.maxTime === maxTime && query.value.minTime === minTime) {
        showCustomMonthDialog.value = false;
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        dateType: dateType,
        maxTime: maxTime,
        minTime: minTime
    });

    showCustomMonthDialog.value = false;

    // ⚠️ 关键修复: 自定义月份筛选更改后立即重新加载数据
    if (changed) {
        reload(false, false);
    }
}

function shiftDateRange(startTime: number, endTime: number, scale: number): void {
    if (recentMonthDateRanges.value[recentDateRangeIndex.value]?.dateType === DateRange.All.type) {
        return;
    }

    let newDateRange: TimeRangeAndDateType | null = null;

    if (DateRange.isBillingCycle(query.value.dateType) || query.value.dateType === DateRange.Custom.type) {
        newDateRange = getShiftedDateRangeAndDateTypeForBillingCycle(startTime, endTime, scale, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal, accountsStore.getAccountStatementDate(query.value.accountIds));
    }

    if (!newDateRange) {
        newDateRange = getShiftedDateRangeAndDateType(startTime, endTime, scale, firstDayOfWeek.value, fiscalYearStart.value, DateRangeScene.Normal);
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

    updateUrlWhenChanged(changed);
}

function changeTypeFilter(type: number): void {
    let newCategoryFilter: string | undefined = undefined;

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

    const changed = transactionsStore.updateTransactionListFilter({
        type: type,
        categoryIds: newCategoryFilter
    });

    updateUrlWhenChanged(changed);
}

function changeCategoryFilter(categoryIds: string): void { // 切换分类筛选，处理多选状态和 URL 中的 categoryIds 合同。
    categoryMenuState.value = false;

    if (query.value.categoryIds === categoryIds) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        categoryIds: categoryIds
    });

    updateUrlWhenChanged(changed);
}

function changeMultipleCategoriesFilter(changed: boolean): void {
    categoryMenuState.value = false;
    showFilterCategoryDialog.value = false;
    updateUrlWhenChanged(changed);
}

function changeAccountFilter(accountIds: string): void { // 切换账户筛选，保持父子账户隐藏状态和多选 query 合同一致。
    if (query.value.accountIds === accountIds) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        accountIds: accountIds
    });

    updateUrlWhenChanged(changed);
}

function changeMultipleAccountsFilter(changed: boolean): void {
    showFilterAccountDialog.value = false;
    updateUrlWhenChanged(changed);
}

function changeTagFilter(tagIds: string): void {
    if (query.value.tagIds === tagIds) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        tagIds: tagIds
    });

    updateUrlWhenChanged(changed);
}

function changeMultipleTagsFilter(changed: boolean): void {
    showFilterTagDialog.value = false;

    updateUrlWhenChanged(changed);
}

function changeTagFilterType(filterType: number): void {
    if (query.value.tagFilterType === filterType) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        tagFilterType: filterType
    });

    updateUrlWhenChanged(changed);
}

function changeKeywordFilter(keyword: string): void {
    if (query.value.keyword === keyword) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        keyword: keyword
    });

    updateUrlWhenChanged(changed);
}

function onAmountFilterTypeClick(filterType: string): void {
    // 如果点击已选中的筛选类型，则切换显示输入框
    if (currentAmountFilterType.value === filterType) {
        currentAmountFilterType.value = '';
    } else {
        currentAmountFilterType.value = filterType;
    }
}

function changeAmountFilter(filterType: string): void { // 应用金额筛选，按筛选类型决定需要写入 URL 的金额参数数量。
    currentAmountFilterType.value = '';
    amountMenuState.value = false;

    if (query.value.amountFilterCents === filterType) {
        return;
    }

    let amountFilterCents = filterType;

    if (filterType) {
        const amountCount = getAmountFilterParameterCount(filterType);

        if (!amountCount) {
            return;
        }

        if (amountCount === 1) {
            amountFilterCents += ':' + currentAmountFilterValue1.value;
        } else if (amountCount === 2) {
            if (currentAmountFilterValue2.value < currentAmountFilterValue1.value) {
                snackbar.value?.showMessage('Incorrect amount range');
                return;
            }

            amountFilterCents += ':' + currentAmountFilterValue1.value + ':' + currentAmountFilterValue2.value;
        } else {
            return;
        }
    }

    if (query.value.amountFilterCents === amountFilterCents) {
        return;
    }

    const changed = transactionsStore.updateTransactionListFilter({
        amountFilterCents: amountFilterCents
    });

    updateUrlWhenChanged(changed);
}

function add(template?: TransactionTemplate): void { // 打开新增交易弹窗，并把模板字段投影到正式交易草稿。
    const currentUnixTime = getCurrentUnixTime();

    let newTransactionTime: number | undefined = undefined;

    if (query.value.maxTime && query.value.minTime) {
        if (query.value.maxTime < currentUnixTime) {
            newTransactionTime = query.value.maxTime;
        } else if (currentUnixTime < query.value.minTime) {
            newTransactionTime = query.value.minTime;
        }
    }

    editDialog.value?.open({
        time: newTransactionTime,
        type: query.value.type,
        categoryId: queryAllFilterCategoryIdsCount.value === 1 ? query.value.categoryIds : '',
        accountId: queryAllFilterAccountIdsCount.value === 1 ? query.value.accountIds : '',
        tagIds: query.value.tagIds || '',
        template: template
    }).then(result => {
        if (result && result.message) {
            snackbar.value?.showMessage(result.message);
        }

        reload(false, false);
    }).catch(error => {
        if (error) {
            snackbar.value?.showError(error);
        }
    });
}

function addByRecognizingImage(): void { // 通过图片 OCR 新建交易草稿，统一处理金额分、时间、分类、账户和标签候选。
    aiImageRecognitionDialog.value?.open().then(result => {
        // result is RecognizedReceiptImageResponse: { amount(yuan|null), tradeTime(ISO|null), description, provenance, confidence }
        // Convert yuan -> cents to match Bill Analyser frontend money convention; skip when backend cannot determine.
        const amountInCents = (typeof result.amount === 'number' && Number.isFinite(result.amount))
            ? Math.round(result.amount * 100)
            : undefined;

        let tradeTimeUnixSeconds: number | undefined = undefined;
        if (result.tradeTime) {
            const parsedMs = Date.parse(result.tradeTime);
            if (!Number.isNaN(parsedMs)) {
                tradeTimeUnixSeconds = Math.floor(parsedMs / 1000);
            }
        }

        editDialog.value?.open({
            time: tradeTimeUnixSeconds,
            sourceAmountCents: amountInCents,
            comment: result.description ?? undefined,
            noTransactionDraft: true
        }).then(result => {
            if (result && result.message) {
                snackbar.value?.showMessage(result.message);
            }

            reload(false, false);
        }).catch(error => {
            if (error) {
                snackbar.value?.showError(error);
            }
        });
    });
}

function batchAdd(): void { // 打开批量手工录入弹窗，并把当前筛选上下文作为批量录入默认值。
    const currentUnixTime = getCurrentUnixTime();

    let newTransactionTime: number | undefined = undefined;

    if (query.value.maxTime && query.value.minTime) {
        if (query.value.maxTime < currentUnixTime) {
            newTransactionTime = query.value.maxTime;
        } else if (currentUnixTime < query.value.minTime) {
            newTransactionTime = query.value.minTime;
        }
    }

    batchManualEntryDialog.value?.open({
        time: newTransactionTime,
        type: query.value.type,
        categoryId: queryAllFilterCategoryIdsCount.value === 1 ? query.value.categoryIds : '',
        accountId: queryAllFilterAccountIdsCount.value === 1 ? query.value.accountIds : '',
        tagIds: query.value.tagIds || ''
    }).then(result => {
        if (result && result.message) {
            snackbar.value?.showMessage(result.message);
        }

        reload(false, false);
    }).catch(error => {
        if (error) {
            snackbar.value?.showError(error);
        }
    });
}

function importTransaction(): void { // 打开导入入口；导入预览本身属于 D1 域，这里只负责交易页装配。
    importDialog.value?.open().then(() => {
        reload(false, false);
    }).catch(error => {
        if (error) {
            snackbar.value?.showError(error);
        }
    });
}

function exportTransactions(fileExtension: string): void { // 使用当前交易列表筛选条件导出正式账单，避免导出结果与屏幕筛选不一致。
    if (exportingData.value) {
        return;
    }

    const nickname = userStore.currentUserNickname;
    let exportFileName = '';

    if (nickname) {
        exportFileName = tt('dataExport.exportFilename', {
            nickname: nickname
        }) + '.' + fileExtension;
    } else {
        exportFileName = tt('dataExport.defaultExportFilename') + '.' + fileExtension;
    }

    const exportTransactionReq = transactionsStore.getExportTransactionDataRequestByTransactionFilter();

    exportingData.value = true;

    userStore.getExportedUserData(fileExtension, exportTransactionReq).then(data => {
        startDownloadFile(exportFileName, data);
        exportingData.value = false;
    }).catch(error => {
        exportingData.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function show(transaction: Transaction): void { // 打开交易详情/编辑弹窗，并在保存后按返回动作刷新列表或进入编辑态。
    editDialog.value?.open({
        id: transaction.id,
        currentTransaction: transaction
    }).then(result => {
        if (result && result.message) {
            snackbar.value?.showMessage(result.message);
        }

        // 如果删除了交易，则强制刷新账户列表，然后重新加载交易列表
        if (result && result.deleted) {
            return accountsStore.loadAllAccounts({ force: true }).then(() => {
                reload(false, false);
            });
        }

        // 非删除操作，直接刷新
        reload(false, false);
        return Promise.resolve(); // 修复TypeScript警告：确保所有分支都有返回值
    }).catch(error => {
        if (error) {
            snackbar.value?.showError(error);
        }
    });
}

function scrollTimeMenuToSelectedItem(opened: boolean): void {
    if (opened) {
        scrollMenuToSelectedItem(timeFilterMenu.value);
    }
}

function scrollCategoryMenuToSelectedItem(opened: boolean): void {
    if (opened) {
        scrollMenuToSelectedItem(categoryFilterMenu.value);
    }
}

function scrollAmountMenuToSelectedItem(opened: boolean): void {
    if (opened) {
        currentAmountFilterType.value = '';

        let amount1 = 0, amount2 = 0;

        if (isString(query.value.amountFilterCents)) {
            try {
                const filterItems = query.value.amountFilterCents.split(':');
                const amountCount = getAmountFilterParameterCount(filterItems[0] as string);

                if (filterItems.length === 2 && amountCount === 1) {
                    amount1 = parseInt(filterItems[1] as string);
                } else if (filterItems.length === 3 && amountCount === 2) {
                    amount1 = parseInt(filterItems[1] as string);
                    amount2 = parseInt(filterItems[2] as string);
                }
            } catch (ex) {
                logger.warn('cannot parse amount from filter value, original value is ' + query.value.amountFilterCents, ex);
            }
        }

        currentAmountFilterValue1.value = amount1;
        currentAmountFilterValue2.value = amount2;

        scrollMenuToSelectedItem(amountFilterCentsMenu.value);
    }
}

function scrollAccountMenuToSelectedItem(opened: boolean): void {
    if (opened) {
        scrollMenuToSelectedItem(accountFilterMenu.value);
    }
}

function scrollTagMenuToSelectedItem(opened: boolean): void {
    if (opened) {
        scrollMenuToSelectedItem(tagFilterMenu.value);
    }
}

function scrollMenuToSelectedItem(menu: VMenu | null): void {
    nextTick(() => {
        scrollToSelectedItem(menu?.contentEl, 'div.v-list', 'div.v-list-item.list-item-selected');
    });
}

function onShowDateRangeError(message: string): void {
    snackbar.value?.showError(message);
}

onBeforeRouteUpdate((to) => {
    if (to.query) {
        init({
            initDateType: (to.query['dateType'] as string | null) || undefined,
            initMinTime: (to.query['minTime'] as string | null) || undefined,
            initMaxTime: (to.query['maxTime'] as string | null) || undefined,
            initType: (to.query['type'] as string | null) || undefined,
            initCategoryIds: (to.query['categoryIds'] as string | null) || undefined,
            initAccountIds: (to.query['accountIds'] as string | null) || undefined,
            initTagIds: (to.query['tagIds'] as string | null) || undefined,
            initTagFilterType: (to.query['tagFilterType'] as string | null) || undefined,
            initAmountFilterCents: (to.query['amountFilterCents'] as string | null) || undefined,
            initKeyword: (to.query['keyword'] as string | null) || undefined
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

watch(() => desktopPageStore.showAddTransactionDialogInTransactionList, (newValue) => {
    if (newValue) {
        desktopPageStore.resetShowAddTransactionDialogInTransactionList();
        add();
    }
});

// 监听交易日历日期变化，自动重新加载数据
watch(currentCalendarDate, (newDate, oldDate) => {
    logger.debug('[ListPage] currentCalendarDate changed', { oldDate, newDate, pageType: pageType.value });
    // 日历模式下，日期变化时重新加载当天交易
    if (pageType.value === TransactionListPageType.Calendar.type && newDate !== oldDate) {
        logger.debug('[ListPage] Calendar date changed, reloading transactions', { date: newDate });
        reload(false, false);
    }
});

init(props);
useExternalTemplateBindings(accountFilterMenu, AccountFilterSettingsCard, accountsStore, activeTab, add, addByRecognizingImage, aiImageRecognitionDialog, AIImageRecognitionDialog, allAccounts, allAccountsMap, allAvailableAccountsCount, allAvailableCategoriesCount, allAvailableTagsCount, allCategories, allDateRanges, allowCategoryTypes, allPageCounts, allPrimaryCategories, allTransactionTagFilterTypes, allTransactionTags, allTransactionTemplates, alwaysShowNav, amountFilterCentsMenu, AmountFilterType, amountMenuState, batchAdd, batchManualEntryDialog, BatchManualEntryDialog, canAddTransaction, categoryFilterMenu, CategoryFilterSettingsCard, categoryMenuState, categoryTypeToTransactionType, changeAccountFilter, changeAmountFilter, changeCategoryFilter, changeCustomDateFilter, changeCustomMonthDateFilter, changeDateFilter, changeKeywordFilter, changeMultipleAccountsFilter, changeMultipleCategoriesFilter, changeMultipleTagsFilter, changePageType, changeTagFilter, changeTagFilterType, changeTypeFilter, computed, confirmDialog, ConfirmDialog, countPerPage, currentAmountFilterType, currentAmountFilterValue1, currentAmountFilterValue2, currentCalendarDate, currentMonthTotalAmount, currentMonthTransactionData, currentPage, currentPageTransactions, currentTimezoneOffsetMinutes, customMaxDatetime, customMinDatetime, DateRange, DateRangeScene, defaultCurrency, desktopPageStore, display, editDialog, EditDialog, exportingData, exportTransactions, firstDayOfWeek, fiscalYearStart, getActualUnixTimeForStore, getAllRecentMonthDateRanges, getAllTransactionTagFilterTypes, getAmountFilterParameterCount, getBrowserTimezoneOffsetMinutes, getCategoryListItemCheckedClass, getCurrentNumeralSystemType, getCurrentUnixTime, getDateRangeByBillingCycleDateType, getDateRangeByDateType, getDateTypeByBillingCycleDateRange, getDateTypeByDateRange, getDayFirstUnixTimeBySpecifiedUnixTime, getDisplayAmount, getDisplayLongDate, getDisplayMonthTotalAmount, getDisplayTime, getDisplayTimeInDefaultTimezone, getDisplayTimezone, getFullMonthDateRange, getRecentDateRangeIndex, getShiftedDateRangeAndDateType, getShiftedDateRangeAndDateTypeForBillingCycle, getTransactionTypeName, getValidMonthDayOrCurrentDayShortDate, getWeekdayLongName, getYearMonthFirstUnixTime, getYearMonthLastUnixTime, importDialog, ImportDialog, importTransaction, init, isDarkApplicationTheme, isDarkMode, isDataExportingEnabled, isDataImportingEnabled, isNumber, isObject, isString, isTransactionFromAIImageRecognitionEnabled, keys, loading, logger, mdiArrowLeft, mdiArrowRight, mdiBorderNoneVariant, mdiCheck, mdiCheckboxMultipleOutline, mdiCloseBoxMultipleOutline, mdiMagicStaff, mdiMagnify, mdiMenu, mdiMenuDown, mdiMinusBoxMultipleOutline, mdiPencilBoxOutline, mdiPlusBoxMultipleOutline, mdiPound, mdiRefresh, mdiTextBoxOutline, mdiVectorArrangeBelow, mdiViewGridOutline, nextTick, numeralSystem, onAmountFilterTypeClick, onBeforeRouteUpdate, onShowDateRangeError, pageType, PaginationButtons, paginationCurrentPage, parseDateTimeFromUnixTime, props, query, queryAccountName, queryAllFilterAccountIds, queryAllFilterAccountIdsCount, queryAllFilterCategoryIds, queryAllFilterCategoryIdsCount, queryAllFilterTagIds, queryAllFilterTagIdsCount, queryAllSelectedFilterAccountIds, queryAllSelectedFilterCategoryIds, queryAllSelectedFilterTagIds, queryAmount, queryCategoryName, queryMaxTime, queryMinTime, queryMonth, queryMonthlyData, queryPageType, queryTagName, queryType, recentDateRangeIndex, recentMonthDateRanges, ref, reload, router, scrollAccountMenuToSelectedItem, scrollAmountMenuToSelectedItem, scrollCategoryMenuToSelectedItem, scrollMenuToSelectedItem, scrollTagMenuToSelectedItem, scrollTimeMenuToSelectedItem, scrollToSelectedItem, searchKeyword, settingsStore, shiftDateRange, show, showCustomDateRangeDialog, showCustomMonthDialog, showFilterAccountDialog, showFilterCategoryDialog, showFilterTagDialog, showNav, showTagInTransactionListPage, showTotalAmountInTransactionListPage, skeletonData, snackbar, SnackBar, startDownloadFile, tagFilterIconMap, tagFilterMenu, TemplateType, temporaryCountPerPage, theme, timeFilterMenu, totalCount, totalPageCount, transactionCalendarMaxDate, transactionCalendarMinDate, transactionCategoriesStore, TransactionEditPageType, TransactionListPageType, transactions, transactionsStore, TransactionTagFilterSettingsCard, TransactionTagFilterType, transactionTagsStore, transactionTemplatesStore, TransactionType, transactionTypeToCategoryType, tt, updateUrlWhenChanged, useAccountsStore, useDesktopPageStore, useDisplay, useI18n, useRouter, userStore, useSettingsStore, useTemplateRef, useTheme, useTransactionCategoriesStore, useTransactionListPageBase, useTransactionsStore, useTransactionTagsStore, useTransactionTemplatesStore, useUserStore, VMenu, watch);
</script>

<style src="./list/ListPage.css"></style>
