<template src="./list/ListPage.template.html"></template>

<script setup lang="ts">
import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import BtnHorizontalGroup from '@/components/desktop/BtnHorizontalGroup.vue';
import BtnVerticalGroup from '@/components/desktop/BtnVerticalGroup.vue';
import AmountInput from '@/components/desktop/AmountInput.vue';
import DateRangeSelectionDialog from '@/components/desktop/DateRangeSelectionDialog.vue';
import AccountFilterSettingsCard from '@/views/desktop/common/cards/AccountFilterSettingsCard.vue';
import TransactionTagFilterSettingsCard from '@/views/desktop/common/cards/TransactionTagFilterSettingsCard.vue';
import CategoryFilterSettingsCard from '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue';
import EditDialog from './list/dialogs/EditDialog.vue';
import BudgetForecastPanel from './components/BudgetForecastPanel.vue';
import BudgetForecastSettingsDialog from './components/BudgetForecastSettingsDialog.vue';
import BudgetHistoryPanel from './components/BudgetHistoryPanel.vue';
import BudgetListTable from './components/BudgetListTable.vue';
import type { BudgetGroup, HistoricalBudgetLevel } from './budgetPageTypes.ts';
import { buildBudgetDrilldownRouteQuery } from './categorySelection.ts';
import { filterAndSortForecasts, summarizeForecastRisks } from './forecastDisplay.ts';
import { buildBudgetForecastLoadRequest } from './forecastRequest.ts';
import {
    buildHistoricalBudgetPeriodGroups,
    filterHistoricalBudgetItemsByType,
    type HistoricalBudgetCategoryMeta,
    type HistoricalBudgetCategoryRow
} from './historyGrouping.ts';
import {
    buildHistoricalPolarChartModel,
    buildHistoricalPolarChartOption,
    createHistoricalLabelAnimationState,
    resetHistoricalCategoryAnimationState,
    resetHistoricalLabelAnimationState,
    syncHistoricalLegendSelection,
    toggleHistoricalPrimarySelection,
    toggleHistoricalSecondarySelection,
    type HistoricalCategoryChartPoint,
    type HistoricalLegendSelection
} from './historyPolarChart.ts';
import {
    getBudgetPeriodTypeFromFilter,
    getBudgetRelativeScopeFromFilter,
    toBudgetRelativePeriodFilter,
    type BudgetRelativePeriodScope
} from './periodFilters.ts';
import {
    type BudgetViewMode,
    type FilterPreset,
    type PeriodFilter,
    type RangeFilter,
    isBudgetViewMode
} from './list/ListPage.types.ts';
import {
    getAmountFilterParameterCount,
    isDateInRange,
    matchAmountFilterCents,
    parseAmountFilterCents
} from './list/budgetAmountFilters.ts';
import {
    HISTORICAL_FISCAL_YEAR,
    type HistoricalAggregationType,
    type HistoricalPeriodRange,
    addMonths,
    buildBudgetHistoryRequestSignature,
    isValidHistoricalUnixTime,
    normalizeHistoryAmountCents,
    parseDateOnly
} from './list/budgetHistoryPeriods.ts';
import {
    CATEGORY_CHART_PALETTE,
    getExecutionRateColor,
    getExecutionRateColorClass,
    getExecutionRateTextClass,
    getForecastConfidenceClass,
    getForecastConfidenceLabel,
    getProgressColorByRate,
    normalizeCategoryColor,
    toCssCategoryColor,
    formatAmount
} from './list/budgetPresentation.ts';

import { ref, computed, useTemplateRef, watch, onMounted, nextTick } from 'vue';
import { useDisplay, useTheme } from 'vuetify';
import { useRouter } from 'vue-router';

import { useI18n } from '@/locales/helpers.ts';
import { useSettingsStore } from '@/stores/setting.ts';
import { useBudgetStore } from '@/stores/budget.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useUserStore } from '@/stores/user.ts';
import { CategoryType } from '@/core/category.ts';
import { DateRange, DateRangeScene } from '@/core/datetime.ts';
import { FiscalYearStart } from '@/core/fiscalyear.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { AmountFilterType } from '@/core/numeral.ts';
import { getApplicationThemeDefinition, isDarkApplicationTheme } from '@/core/theme.ts';
import {
    getCurrentUnixTime,
    getTodayFirstUnixTime,
    getDateRangeByDateType,
    getShiftedDateRangeAndDateType,
    getDateTypeByDateRange
} from '@/lib/datetime.ts';
import logger from '@/lib/logger.ts';

import {
    Budget,
    BudgetType,
    BudgetPeriodType,
    BudgetForecastStrategy,
    type BudgetExecutionResponse,
    type BudgetForecastResponse,
    type BudgetHistoryResponse,
    type BudgetHistoryItem,
    type BudgetHistoryRequest
} from '@/models/budget.ts';

import {
    mdiRefresh,
    mdiMagnify,
    mdiCheck,
    mdiDotsVertical,
    mdiFilterRemoveOutline,
    mdiMenu,
    mdiUnfoldLessHorizontal,
    mdiUnfoldMoreHorizontal,
    mdiShapeOutline,
    mdiWalletOutline,
    mdiTagOutline,
    mdiChartDonut,
    mdiCashMinus,
    mdiCashPlus,
    mdiBookmarkOutline,
    mdiBookmark,
    mdiContentSaveOutline,
    mdiClose,
    mdiInformationOutline,
    mdiArrowLeft,
    mdiArrowRight
} from '@mdi/js';

// ============================================================================
// 类型定义
// ============================================================================

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;
type EditDialogType = InstanceType<typeof EditDialog>;
interface ResizableChartComponent {
    resize?: () => void;
    chart?: {
        resize?: () => void;
    };
}

// ============================================================================
// 属性
// ============================================================================

const props = defineProps<{
    initType?: string;
    initPeriodType?: string;
    initViewMode?: string;
}>();

// ============================================================================
// 组件引用
// ============================================================================

const { tt, getAllDateRanges, formatDateRange, formatAmountToLocalizedNumeralsWithCurrency } = useI18n();
const display = useDisplay();
const theme = useTheme();
const router = useRouter();
const budgetStore = useBudgetStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const accountsStore = useAccountsStore();
const transactionTagsStore = useTransactionTagsStore();
const settingsStore = useSettingsStore();
const userStore = useUserStore();

// 默认货币
const defaultCurrency = computed(() => String(settingsStore.appSettings['currency'] || 'CNY'));

// 暗色模式检测
const isDarkMode = computed<boolean>(() => isDarkApplicationTheme(theme.global.name.value));

// 是否始终显示导航栏（桌面端）
const alwaysShowNav = computed(() => display.mdAndUp.value);
const showNav = ref<boolean>(true);

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const editDialog = useTemplateRef<EditDialogType>('editDialog');
const fileInput = useTemplateRef<HTMLInputElement>('fileInput');
const historicalChartRef = useTemplateRef<ResizableChartComponent>('historicalChartRef');

// ============================================================================
// 响应式状态
// ============================================================================

const loading = ref<boolean>(false);
const updating = ref<boolean>(false);
const searchKeyword = ref<string>('');
const filterKeyword = ref<string>('');
const reloadRequestId = ref<number>(0);

// 视图模式：'budget' 或 'forecast'
const activeViewMode = ref<BudgetViewMode>('budget');
const forecastStrategy = ref<BudgetForecastStrategy>(BudgetForecastStrategy.HistoricalAverage);
const forecastMonthsHistory = ref<number>(6);
const forecastSortBy = ref<string>('backtest');
const forecastOnlyLowConfidence = ref<boolean>(false);
const forecastOnlyOverBudget = ref<boolean>(false);
const historicalBudgetLevel = ref<HistoricalBudgetLevel>('secondary');
const historicalLegendSelection = ref<HistoricalLegendSelection>({});
const historicalDateType = ref<number>(DateRange.RecentTwelveMonths.type);
const showHistoricalDateDialog = ref<boolean>(false);
const historicalMinDatetime = ref<number>(0);
const historicalMaxDatetime = ref<number>(0);
const historicalExpandedGroupKeys = ref<string[]>([]);
const historicalCollapsedPrimaryKeys = ref<string[]>([]);
const selectedHistoricalPeriodKey = ref<string | null>(null);

// 预算类型：支出或投资
const activeBudgetType = ref<BudgetType>(BudgetType.Expense);

// 周期筛选器
const activePeriodFilter = ref<string>('thisMonth');

// 自定义日期范围 - 使用日期范围选择对话框组件
const showCustomDateDialog = ref<boolean>(false);
const customMinDatetime = ref<number>(getTodayFirstUnixTime());  // Unix时间戳（秒）
const customMaxDatetime = ref<number>(getCurrentUnixTime());      // Unix时间戳（秒）
const customStartDate = ref<string>('');  // 保留用于reload()计算
const customEndDate = ref<string>('');    // 保留用于reload()计算

// 删除状态追踪
const budgetRemoving = ref<Record<string, boolean>>({});

// 筛选器值
const categoryFilter = ref<string | null>(null);
const accountFilter = ref<string[]>([]);
const tagFilter = ref<string[]>([]);
const executionRateFilter = ref<RangeFilter | null>(null);
const spentAmountFilterCents = ref<string>('');  // 格式: 'filterType:value1:value2'
const budgetAmountFilterCents = ref<string>('');  // 格式: 'filterType:value1:value2'

// 筛选对话框
const showFilterAccountDialog = ref<boolean>(false);
const showFilterTagDialog = ref<boolean>(false);
const showFilterCategoryDialog = ref<boolean>(false);
const showForecastSettingsDialog = ref<boolean>(false);

// 筛选预设
const showSavePresetDialog = ref<boolean>(false);
const presetName = ref<string>('');

const filterPresets = ref<FilterPreset[]>([]);

// 从本地存储加载预设
function loadPresetsFromStorage(): void {
    const stored = localStorage.getItem('budgetFilterPresets');
    if (stored) {
        try {
            filterPresets.value = JSON.parse(stored);
        } catch {
            filterPresets.value = [];
        }
    }
}

// 将预设保存到本地存储
function savePresetsToStorage(): void {
    localStorage.setItem('budgetFilterPresets', JSON.stringify(filterPresets.value));
}

// 金额筛选临时值 (已花费)
const currentSpentFilterType = ref<string>('');
const currentSpentFilterValue1 = ref<number>(0);
const currentSpentFilterValue2 = ref<number>(0);

// 金额筛选临时值 (总预算)
const currentBudgetFilterType = ref<string>('');
const currentBudgetFilterValue1 = ref<number>(0);
const currentBudgetFilterValue2 = ref<number>(0);

// 排序
const sortBy = ref<string>('category');
const sortDesc = ref<boolean>(false);

// 折叠状态：记录哪些一级分类是折叠的
const collapsedCategories = ref<Set<string>>(new Set());

// ============================================================================
// 计算属性
// ============================================================================

const budgetPeriodTypeButtons = computed(() => [
    { name: tt('Monthly'), value: BudgetPeriodType.Monthly },
    { name: tt('Quarterly'), value: BudgetPeriodType.Quarterly },
    { name: tt('Yearly'), value: BudgetPeriodType.Yearly }
]);

const activeBudgetRelativeScope = computed<BudgetRelativePeriodScope>({
    get() {
        return getBudgetRelativeScopeFromFilter(activePeriodFilter.value);
    },
    set(scope: BudgetRelativePeriodScope) {
        const nextFilter = toBudgetRelativePeriodFilter(activeBudgetPeriodType.value, scope);
        if (nextFilter === activePeriodFilter.value) {
            return;
        }

        activePeriodFilter.value = nextFilter;
        reload(false);
    }
});

const activeBudgetPeriodType = computed<BudgetPeriodType>({
    get() {
        return getBudgetPeriodTypeFromFilter(activePeriodFilter.value);
    },
    set(periodType: BudgetPeriodType) {
        const nextFilter = toBudgetRelativePeriodFilter(periodType, activeBudgetRelativeScope.value);
        if (nextFilter === activePeriodFilter.value) {
            return;
        }

        activePeriodFilter.value = nextFilter;
        reload(false);
    }
});

const forecastPeriodFilters = computed<PeriodFilter[]>(() => [
    { name: tt('Monthly Forecast'), value: 'thisMonth' },
    { name: tt('Quarterly Forecast'), value: 'thisQuarter' },
    { name: tt('Yearly Forecast'), value: 'thisYear' }
]);

const visiblePeriodFilters = computed<PeriodFilter[]>(() => {
    return activeViewMode.value === 'forecast'
        ? forecastPeriodFilters.value
        : [
            { name: tt('Current Period'), value: toBudgetRelativePeriodFilter(activeBudgetPeriodType.value, 'current') },
            { name: tt('Previous Period'), value: toBudgetRelativePeriodFilter(activeBudgetPeriodType.value, 'previous') }
        ];
});

// 当前选中的周期筛选索引（用于标签页）
const activePeriodFilterIndex = computed<number>(() => {
    return visiblePeriodFilters.value.findIndex(f => f.value === activePeriodFilter.value);
});

const allBudgets = computed<Budget[]>(() => budgetStore.allBudgets);

const filteredBudgets = computed<Budget[]>(() => {
    let budgets = allBudgets.value;

    // 按类型筛选
    budgets = budgets.filter(b => b.type === activeBudgetType.value);

    // 按周期筛选
    budgets = filterByPeriod(budgets);

    // 按关键词筛选
    if (searchKeyword.value) {
        const keyword = searchKeyword.value.toLowerCase();
        budgets = budgets.filter(b =>
            b.name.toLowerCase().includes(keyword) ||
            b.category.toLowerCase().includes(keyword) ||
            b.subCategory.toLowerCase().includes(keyword)
        );
    }

    // 按分类筛选（使用分类ID）
    if (categoryFilter.value) {
        const selectedCategory = allCategoriesMap.value[categoryFilter.value];
        if (selectedCategory) {
            budgets = budgets.filter(b => {
                // 如果选中的是一级分类，匹配该分类及其所有子分类
                if (!selectedCategory.parentId) {
                    return b.categoryId === categoryFilter.value ||
                           (selectedCategory.subCategories &&
                            selectedCategory.subCategories.some(sub => sub.id === b.categoryId));
                }
                // 如果选中的是二级分类，精确匹配
                return b.categoryId === categoryFilter.value;
            });
        }
    }

    // 按执行度筛选
    if (executionRateFilter.value) {
        const { min, max } = executionRateFilter.value;
        budgets = budgets.filter(b => {
            if (min !== undefined && b.executionRate < min) return false;
            if (max !== undefined && b.executionRate > max) return false;
            return true;
        });
    }

    // 按已花费筛选（filterType 后的值均为整数分）
    if (spentAmountFilterCents.value) {
        const parsed = parseAmountFilterCents(spentAmountFilterCents.value);
        if (parsed) {
            budgets = budgets.filter(b => matchAmountFilterCents(b.spentAmountCents, parsed));
        }
    }

    // 按预算金额筛选（filterType 后的值均为整数分）
    if (budgetAmountFilterCents.value) {
        const parsed = parseAmountFilterCents(budgetAmountFilterCents.value);
        if (parsed) {
            budgets = budgets.filter(b => matchAmountFilterCents(b.amountCents, parsed));
        }
    }

    return budgets;
});

const sortedBudgets = computed<Budget[]>(() => {
    const budgets = [...filteredBudgets.value];

    budgets.sort((a, b) => {
        let comparison = 0;

        switch (sortBy.value) {
            case 'category':
                comparison = (a.fullCategoryName || a.name).localeCompare(b.fullCategoryName || b.name);
                break;
            case 'executionRate':
                comparison = a.executionRate - b.executionRate;
                break;
            case 'spent':
                comparison = a.spentAmountInYuan - b.spentAmountInYuan;
                break;
            case 'budget':
                comparison = a.amountInYuan - b.amountInYuan;
                break;
        }

        return sortDesc.value ? -comparison : comparison;
    });

    return budgets;
});

/**
 * 层级预算组：按一级分类分组
 */
/**
 * 分组后的预算列表（层级显示用）
 */
const groupedBudgets = computed<BudgetGroup[]>(() => {
    const groups: Map<string, BudgetGroup> = new Map();

    // 构建一级分类名称到分类对象的映射（用于获取正确的图标和颜色）
    const primaryCategoryByName: Record<string, TransactionCategory> = {};
    for (const cat of budgetPrimaryCategories.value) {
        primaryCategoryByName[cat.name] = cat;
    }

    for (const budget of sortedBudgets.value) {
        const categoryKey = budget.category || budget.name;

        if (!groups.has(categoryKey)) {
            // 从分类配置中获取正确的图标和颜色（而非从预算数据中获取）
            const primaryCategory = primaryCategoryByName[categoryKey];
            const categoryIcon = primaryCategory?.icon || budget.categoryIcon || '';
            const categoryColor = normalizeCategoryColor(primaryCategory?.color || budget.categoryColor);

            groups.set(categoryKey, {
                category: categoryKey,
                categoryIcon: categoryIcon,
                categoryColor: categoryColor,
                primaryBudgets: [],
                subBudgets: [],
                totalAmountCents: 0,
                totalSpentCents: 0,
                subTotalAmountCents: 0,
                subTotalSpentCents: 0,
                primaryAmountCents: 0,
                primarySpentCents: 0,
                isCollapsed: collapsedCategories.value.has(categoryKey)
            });
        }

        const group = groups.get(categoryKey)!;

        // 判断是一级分类预算还是二级分类预算
        if (!budget.subCategory) {
            group.primaryBudgets.push(budget);
            group.primaryAmountCents += budget.amountCents;
            group.primarySpentCents += budget.spentAmountCents;
        } else {
            // 二级分类预算
            group.subBudgets.push(budget);
            // 累加二级分类的金额和花费
            group.subTotalAmountCents += budget.amountCents;
            group.subTotalSpentCents += budget.spentAmountCents;
        }
    }

    // 计算显示的总金额和总花费
    // 规则：
    // 1. 如果有一级分类预算，使用一级分类的预算金额（用户设置的总预算）
    // 2. 如果没有一级分类预算，使用二级分类之和
    // 3. 已花费金额：优先使用一级分类的已花费，否则使用二级之和
    for (const group of groups.values()) {
        if (group.primaryBudgets.length > 1) {
            group.totalAmountCents = group.primaryAmountCents + group.subTotalAmountCents;
            group.totalSpentCents = group.primarySpentCents + group.subTotalSpentCents;
        } else if (group.primaryBudgets.length > 0) {
            // 有一级分类预算：使用一级分类的预算金额
            group.totalAmountCents = group.primaryAmountCents;
            // 已花费：如果一级分类有值就用一级的，否则用二级之和
            group.totalSpentCents = group.primarySpentCents > 0 ? group.primarySpentCents : group.subTotalSpentCents;
        } else if (group.subBudgets.length > 0) {
            // 只有二级分类预算：使用二级之和
            group.totalAmountCents = group.subTotalAmountCents;
            group.totalSpentCents = group.subTotalSpentCents;
        } else {
            // 既没有一级也没有二级：金额为0
            group.totalAmountCents = 0;
            group.totalSpentCents = 0;
        }
    }

    // 转换为数组并按分类名称排序
    return Array.from(groups.values()).sort((a, b) => a.category.localeCompare(b.category));
});

function getPrimaryBudgetForHeader(group: BudgetGroup): Budget | null {
    return group.primaryBudgets[0] || null;
}

function getExpandedPrimaryBudgets(group: BudgetGroup): Budget[] {
    return group.primaryBudgets.length > 1 ? group.primaryBudgets : [];
}

function getBudgetCategoryIcon(budget: Budget, group: BudgetGroup): string {
    return budget.categoryIcon || group.categoryIcon;
}

function getBudgetCategoryColor(budget: Budget, group: BudgetGroup): string {
    return normalizeCategoryColor(budget.categoryColor) || group.categoryColor;
}

function groupHasExpandedRows(group: BudgetGroup): boolean {
    return getExpandedPrimaryBudgets(group).length > 0 || group.subBudgets.length > 0;
}

/**
 * 切换分类折叠状态
 */
function toggleCategoryCollapse(category: string): void {
    if (collapsedCategories.value.has(category)) {
        collapsedCategories.value.delete(category);
    } else {
        collapsedCategories.value.add(category);
    }
    // 强制更新
    collapsedCategories.value = new Set(collapsedCategories.value);
}

/**
 * 检查是否全部展开
 */
const isAllExpanded = computed<boolean>(() => {
    if (groupedBudgets.value.length === 0) return true;
    return collapsedCategories.value.size === 0;
});

/**
 * 切换全部展开/折叠（带动画）
 */
function toggleAllCategories(): void {
    if (isAllExpanded.value) {
        // 全部折叠
        const allCategories = groupedBudgets.value.map(g => g.category);
        collapsedCategories.value = new Set(allCategories);
    } else {
        // 全部展开
        collapsedCategories.value = new Set();
    }
}

// 根据预算类型获取对应的分类类型
const currentCategoryType = computed<number>(() => {
    return activeBudgetType.value === BudgetType.Expense
        ? CategoryType.Expense
        : CategoryType.Investment;
});

// 获取当前允许的分类类型字符串（用于分类筛选对话框）
const allowedCategoryTypes = computed<string>(() => {
    return String(currentCategoryType.value);
});

// 获取当前预算类型的一级分类列表
const budgetPrimaryCategories = computed<TransactionCategory[]>(() => {
    const categories = transactionCategoriesStore.allTransactionCategories[currentCategoryType.value];
    return categories || [];
});

// 分类ID到分类对象的映射
const allCategoriesMap = computed<Record<string, TransactionCategory>>(() => {
    const map: Record<string, TransactionCategory> = {};

    for (const primaryCategory of budgetPrimaryCategories.value) {
        map[primaryCategory.id] = primaryCategory;
        if (primaryCategory.subCategories) {
            for (const subCategory of primaryCategory.subCategories) {
                map[subCategory.id] = subCategory;
            }
        }
    }

    return map;
});

const executionRateOptions = computed<RangeFilter[]>(() => [
    { label: tt('Under 50%'), value: 'under50', min: 0, max: 50 },
    { label: tt('50% - 80%'), value: '50to80', min: 50, max: 80 },
    { label: tt('80% - 100%'), value: '80to100', min: 80, max: 100 },
    { label: tt('Over Budget'), value: 'over100', min: 100, max: undefined }
]);

// 所有账户列表
const allAccounts = computed(() => accountsStore.allAccounts);

// 所有标签列表
const allTransactionTags = computed(() => transactionTagsStore.allTransactionTags);

const hasActiveFilters = computed<boolean>(() => {
    return !!(categoryFilter.value || executionRateFilter.value ||
              spentAmountFilterCents.value || budgetAmountFilterCents.value ||
              accountFilter.value.length > 0 || tagFilter.value.length > 0);
});

const currentExecution = computed<BudgetExecutionResponse | null>(() => budgetStore.currentExecution);
const currentForecast = computed<BudgetForecastResponse | null>(() => budgetStore.currentForecast);
const currentHistory = computed<BudgetHistoryResponse | null>(() => budgetStore.currentHistory);
const currentHistoryRequestSignature = computed<string>(() => budgetStore.currentHistoryRequestSignature);
const forecastLoading = computed<boolean>(() => budgetStore.forecastLoading);
const firstDayOfWeek = computed(() => userStore.currentUserFirstDayOfWeek);
const viewModeButtons = computed(() => [
    { name: tt('Budget Management'), value: 'budget' },
    { name: tt('Period Forecast'), value: 'forecast' },
    { name: tt('Historical Budgets'), value: 'history' }
]);
const currentViewTitle = computed(() => {
    if (activeViewMode.value === 'forecast') {
        return tt('Period Forecast');
    }
    if (activeViewMode.value === 'history') {
        return tt('Historical Budget Execution');
    }
    return tt('Budget Management');
});
const historicalLevelButtons = computed(() => [
    { name: tt('Primary Category Budget'), value: 'primary' },
    { name: tt('Secondary Category Budget'), value: 'secondary' }
]);
const allHistoricalDateRanges = computed(() => getAllDateRanges(DateRangeScene.AssetTrends, true, false));
const historicalDateRangeName = computed(() => {
    if (!historicalMinDatetime.value || !historicalMaxDatetime.value) {
        return tt('Recent 12 months');
    }
    return formatDateRange(historicalDateType.value, historicalMinDatetime.value, historicalMaxDatetime.value);
});
const historicalDateStartText = computed(() => historicalMinDatetime.value ? formatDateOnly(new Date(historicalMinDatetime.value * 1000)) : '');
const historicalDateEndText = computed(() => historicalMaxDatetime.value ? formatDateOnly(new Date(historicalMaxDatetime.value * 1000)) : '');
const canShiftHistoricalDateRange = computed(() => {
    return historicalDateType.value !== DateRange.All.type && !!historicalMinDatetime.value && !!historicalMaxDatetime.value;
});
const forecastStrategies = computed(() => [
    { name: tt('Historical Average Strategy'), value: BudgetForecastStrategy.HistoricalAverage },
    { name: tt('Moving Average Strategy'), value: BudgetForecastStrategy.MovingAverage }
]);
const historyPeriodOptions = computed<number[]>(() => [3, 6, 9, 12]);
const forecastSortOptions = computed(() => [
    { name: tt('Sort by Backtest MAPE'), value: 'backtest' },
    { name: tt('Sort by Confidence'), value: 'confidence' },
    { name: tt('Sort by Projected Total'), value: 'projected_total' },
    { name: tt('Sort by Category'), value: 'category' }
]);

const forecastRiskSummary = computed(() => {
    return summarizeForecastRisks(currentForecast.value?.forecasts || [], displayForecasts.value.length);
});

const displayForecasts = computed(() => {
    return filterAndSortForecasts(currentForecast.value?.forecasts || [], {
        sortBy: forecastSortBy.value as 'backtest' | 'confidence' | 'projected_total' | 'category',
        onlyLowConfidence: forecastOnlyLowConfidence.value,
        onlyOverBudget: forecastOnlyOverBudget.value
    });
});

/**
 * 抬头汇总：按当前筛选后的可见分组聚合，避免全量预算总和
 */
const filteredSummary = computed(() => {
    let totalBudgetCents = 0;
    let totalSpentCents = 0;

    for (const group of groupedBudgets.value) {
        totalBudgetCents += group.totalAmountCents;
        totalSpentCents += group.totalSpentCents;
    }

    const totalExecutionRate = totalBudgetCents > 0 ? (totalSpentCents / totalBudgetCents) * 100 : 0;

    return {
        totalBudgetCents,
        totalSpentCents,
        totalExecutionRate
    };
});

/**
 * 往期预算趋势数据（一级分类）
 */
const historicalAggregationType = ref<HistoricalAggregationType>(BudgetPeriodType.Monthly);
const historyAggregationButtons = computed(() => [
    { name: tt('Monthly'), value: BudgetPeriodType.Monthly },
    { name: tt('Quarterly'), value: BudgetPeriodType.Quarterly },
    { name: tt('Yearly'), value: BudgetPeriodType.Yearly }
]);

const activeHistoricalAggregationType = computed<BudgetPeriodType>({
    get() {
        return historicalAggregationType.value === HISTORICAL_FISCAL_YEAR
            ? BudgetPeriodType.Yearly
            : historicalAggregationType.value;
    },
    set(nextType: BudgetPeriodType) {
        if (historicalAggregationType.value === nextType) {
            return;
        }

        historicalAggregationType.value = nextType;
        if (activeViewMode.value === 'history') {
            reload(false);
        }
    }
});

const fiscalYearStartValue = computed<number>(() => userStore.currentUserFiscalYearStart);

const fiscalYearStartInfo = computed(() => {
    return FiscalYearStart.valueOf(fiscalYearStartValue.value) || FiscalYearStart.Default;
});

function getDefaultHistoricalDateRange(): { dateType: number; minTime: number; maxTime: number } | null {
    return getDateRangeByDateType(
        historicalDateType.value,
        firstDayOfWeek.value,
        fiscalYearStartValue.value
    ) ?? getDateRangeByDateType(
        DateRange.RecentTwelveMonths.type,
        firstDayOfWeek.value,
        fiscalYearStartValue.value
    );
}

function getHistoricalDateRangeSnapshot(): { dateType: number; minTime: number; maxTime: number } {
    if (
        isValidHistoricalUnixTime(historicalMinDatetime.value)
        && isValidHistoricalUnixTime(historicalMaxDatetime.value)
        && historicalMinDatetime.value <= historicalMaxDatetime.value
    ) {
        return {
            dateType: historicalDateType.value,
            minTime: historicalMinDatetime.value,
            maxTime: historicalMaxDatetime.value
        };
    }

    const fallbackRange = getDefaultHistoricalDateRange();
    if (fallbackRange) {
        return fallbackRange;
    }

    const now = getCurrentUnixTime();
    return {
        dateType: DateRange.RecentTwelveMonths.type,
        minTime: now,
        maxTime: now
    };
}

function getHistoricalBudgetQueryRange(): { startDate: string; endDate: string } {
    const range = getHistoricalDateRangeSnapshot();

    return {
        startDate: formatDateOnly(new Date(range.minTime * 1000)),
        endDate: formatDateOnly(new Date(range.maxTime * 1000))
    };
}

function getHistoricalRequestPeriodType(): BudgetPeriodType {
    return historicalAggregationType.value === HISTORICAL_FISCAL_YEAR
        ? BudgetPeriodType.Yearly
        : historicalAggregationType.value;
}

const activeHistoricalHistoryRequestSignature = computed<string>(() => {
    const historyRange = getHistoricalBudgetQueryRange();

    return buildBudgetHistoryRequestSignature({
        type: activeBudgetType.value,
        periodType: getHistoricalRequestPeriodType(),
        startDate: historyRange.startDate,
        endDate: historyRange.endDate,
        categoryId: categoryFilter.value || undefined,
        accountIds: accountFilter.value.length ? [...accountFilter.value] : undefined,
        tagIds: tagFilter.value.length ? [...tagFilter.value] : undefined
    });
});

const isHistoricalHistoryReady = computed<boolean>(() => (
    !!currentHistory.value
    && currentHistoryRequestSignature.value === activeHistoricalHistoryRequestSignature.value
));

function buildFiscalYearPeriod(targetDate: Date): HistoricalPeriodRange {
    const fiscalStart = fiscalYearStartInfo.value;
    const currentYearFiscalStart = new Date(targetDate.getFullYear(), fiscalStart.month - 1, fiscalStart.day);
    const fiscalStartYear = targetDate >= currentYearFiscalStart
        ? targetDate.getFullYear()
        : targetDate.getFullYear() - 1;
    const startDate = new Date(fiscalStartYear, fiscalStart.month - 1, fiscalStart.day);
    const endDate = new Date(fiscalStartYear + 1, fiscalStart.month - 1, fiscalStart.day - 1);
    const endYear = endDate.getFullYear();

    return {
        key: `FY${endYear}`,
        label: `FY${endYear}`,
        startDate: formatDateOnly(startDate),
        endDate: formatDateOnly(endDate)
    };
}

function buildHistoricalAggregationPeriods(
    aggregationType: HistoricalAggregationType,
    startDate: string,
    endDate: string
): HistoricalPeriodRange[] {
    const start = parseDateOnly(startDate);
    const end = parseDateOnly(endDate);

    if (!start || !end || start > end) {
        return [];
    }

    const periods: HistoricalPeriodRange[] = [];

    if (aggregationType === BudgetPeriodType.Monthly) {
        let cursor = new Date(start.getFullYear(), start.getMonth(), 1);
        while (cursor <= end) {
            const periodEnd = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 0);
            const key = `${cursor.getFullYear()}-${String(cursor.getMonth() + 1).padStart(2, '0')}`;
            periods.push({
                key,
                label: key,
                startDate: formatDateOnly(cursor),
                endDate: formatDateOnly(periodEnd)
            });
            cursor = addMonths(cursor, 1);
        }
        return periods;
    }

    if (aggregationType === BudgetPeriodType.Quarterly) {
        let cursor = new Date(start.getFullYear(), Math.floor(start.getMonth() / 3) * 3, 1);
        while (cursor <= end) {
            const quarter = Math.floor(cursor.getMonth() / 3) + 1;
            const periodEnd = new Date(cursor.getFullYear(), cursor.getMonth() + 3, 0);
            const key = `${cursor.getFullYear()}-Q${quarter}`;
            periods.push({
                key,
                label: key,
                startDate: formatDateOnly(cursor),
                endDate: formatDateOnly(periodEnd)
            });
            cursor = addMonths(cursor, 3);
        }
        return periods;
    }

    if (aggregationType === BudgetPeriodType.Yearly) {
        let cursor = new Date(start.getFullYear(), 0, 1);
        while (cursor <= end) {
            const periodEnd = new Date(cursor.getFullYear(), 11, 31);
            const key = `${cursor.getFullYear()}`;
            periods.push({
                key,
                label: key,
                startDate: formatDateOnly(cursor),
                endDate: formatDateOnly(periodEnd)
            });
            cursor = new Date(cursor.getFullYear() + 1, 0, 1);
        }
        return periods;
    }

    let cursor = parseDateOnly(buildFiscalYearPeriod(start).startDate);
    while (cursor && cursor <= end) {
        const fiscalPeriod = buildFiscalYearPeriod(cursor);
        periods.push(fiscalPeriod);
        cursor = new Date(cursor.getFullYear() + 1, cursor.getMonth(), cursor.getDate());
    }

    return periods;
}

function resolveHistoricalPeriodByDate(
    aggregationType: HistoricalAggregationType,
    targetDate: Date
): HistoricalPeriodRange {
    if (aggregationType === BudgetPeriodType.Monthly) {
        const key = `${targetDate.getFullYear()}-${String(targetDate.getMonth() + 1).padStart(2, '0')}`;
        return {
            key,
            label: key,
            startDate: formatDateOnly(new Date(targetDate.getFullYear(), targetDate.getMonth(), 1)),
            endDate: formatDateOnly(new Date(targetDate.getFullYear(), targetDate.getMonth() + 1, 0))
        };
    }

    if (aggregationType === BudgetPeriodType.Quarterly) {
        const quarterStartMonth = Math.floor(targetDate.getMonth() / 3) * 3;
        const quarter = Math.floor(targetDate.getMonth() / 3) + 1;
        return {
            key: `${targetDate.getFullYear()}-Q${quarter}`,
            label: `${targetDate.getFullYear()}-Q${quarter}`,
            startDate: formatDateOnly(new Date(targetDate.getFullYear(), quarterStartMonth, 1)),
            endDate: formatDateOnly(new Date(targetDate.getFullYear(), quarterStartMonth + 3, 0))
        };
    }

    if (aggregationType === BudgetPeriodType.Yearly) {
        return {
            key: `${targetDate.getFullYear()}`,
            label: `${targetDate.getFullYear()}`,
            startDate: `${targetDate.getFullYear()}-01-01`,
            endDate: `${targetDate.getFullYear()}-12-31`
        };
    }

    return buildFiscalYearPeriod(targetDate);
}

const filteredHistoricalItems = computed<BudgetHistoryItem[]>(() => {
    if (!isHistoricalHistoryReady.value || !currentHistory.value?.items?.length) {
        return [];
    }

    const selectedCategory = categoryFilter.value ? allCategoriesMap.value[categoryFilter.value] : null;
    const selectedPrimaryCategory = selectedCategory?.parentId
        ? allCategoriesMap.value[selectedCategory.parentId]
        : selectedCategory;

    const typeFilteredItems = filterHistoricalBudgetItemsByType(
        currentHistory.value.items,
        activeBudgetType.value
    );

    return typeFilteredItems.filter((item: BudgetHistoryItem) => {
        if (!selectedCategory) {
            return true;
        }

        const selectedName = selectedCategory.name;
        const selectedPrimaryName = selectedPrimaryCategory?.name || selectedName;

        if (selectedCategory.parentId) {
            return item.category === selectedPrimaryName && item.subCategory === selectedName;
        }

        return item.category === selectedPrimaryName;
    });
});

const periodFilteredHistoricalItems = computed<BudgetHistoryItem[]>(() => {
    if (!selectedHistoricalPeriodKey.value) {
        return filteredHistoricalItems.value;
    }
    return filteredHistoricalItems.value.filter((item) => {
        const itemDate = parseDateOnly(item.periodStart);
        if (!itemDate) {
            return false;
        }
        const period = resolveHistoricalPeriodByDate(historicalAggregationType.value, itemDate);
        return period.key === selectedHistoricalPeriodKey.value;
    });
});

const historicalPeriods = computed<HistoricalPeriodRange[]>(() => {
    const range = getHistoricalBudgetQueryRange();
    return buildHistoricalAggregationPeriods(
        historicalAggregationType.value,
        range.startDate,
        range.endDate
    );
});

const historicalCategoryChartData = computed<{ categories: string[]; points: HistoricalCategoryChartPoint[] }>(() => {
    const periods = historicalPeriods.value;
    const items = periodFilteredHistoricalItems.value;

    if (!periods.length || !items.length) {
        return { categories: [], points: [] };
    }

    const periodMap = new Map(periods.map(period => [period.key, period]));

    const categoryColorMap: Record<string, string> = {};
    const categoryOrderMap: Record<string, number> = {};
    const subCategoryColorMap: Record<string, string> = {};
    const subCategoryOrderMap: Record<string, number> = {};
    for (const cat of budgetPrimaryCategories.value) {
        if (cat.color) {
            categoryColorMap[cat.name] = `#${cat.color}`;
        }
        categoryOrderMap[cat.name] = cat.displayOrder ?? 0;

        for (const subCategory of cat.subCategories || []) {
            if (subCategory.color) {
                subCategoryColorMap[`${cat.name}::${subCategory.name}`] = `#${subCategory.color}`;
            }
            subCategoryOrderMap[`${cat.name}::${subCategory.name}`] = subCategory.displayOrder ?? 0;
        }
    }

    const groupedByPeriodAndPrimary = new Map<string, {
        primaryCategory: string;
        groupOrder: number;
        primaryBudgetAmountCents: number;
        primarySpentAmountCents: number;
        secondaryBudgetAmountCents: number;
        secondarySpentAmountCents: number;
        secondaryItems: Map<string, {
            displayCategory: string;
            secondaryCategory: string;
            budgetAmountCents: number;
            spentAmountCents: number;
            itemOrder: number;
        }>;
    }>();

    for (const item of items) {
        const itemDate = parseDateOnly(item.periodStart);
        if (!itemDate) continue;

        const resolvedPeriod = resolveHistoricalPeriodByDate(historicalAggregationType.value, itemDate);
        if (!periodMap.has(resolvedPeriod.key)) continue;

        const primaryCategory = String(item.category || '').trim() || tt('Uncategorized');
        const secondaryCategory = String(item.subCategory || '').trim();
        const budgetAmountCents = normalizeHistoryAmountCents(item.budgetAmountCents);
        const spentAmountCents = normalizeHistoryAmountCents(item.spentAmountCents);
        const periodPrimaryKey = `${resolvedPeriod.key}::${primaryCategory}`;

        if (!groupedByPeriodAndPrimary.has(periodPrimaryKey)) {
            groupedByPeriodAndPrimary.set(periodPrimaryKey, {
                primaryCategory,
                groupOrder: categoryOrderMap[primaryCategory] ?? Number.MAX_SAFE_INTEGER,
                primaryBudgetAmountCents: 0,
                primarySpentAmountCents: 0,
                secondaryBudgetAmountCents: 0,
                secondarySpentAmountCents: 0,
                secondaryItems: new Map()
            });
        }

        const summary = groupedByPeriodAndPrimary.get(periodPrimaryKey)!;

        if (!secondaryCategory) {
            summary.primaryBudgetAmountCents += budgetAmountCents;
            summary.primarySpentAmountCents += spentAmountCents;
            continue;
        }

        const secondaryKey = `${primaryCategory}::${secondaryCategory}`;
        if (!summary.secondaryItems.has(secondaryKey)) {
            summary.secondaryItems.set(secondaryKey, {
                displayCategory: secondaryCategory,
                secondaryCategory,
                budgetAmountCents: 0,
                spentAmountCents: 0,
                itemOrder: subCategoryOrderMap[secondaryKey] ?? 0
            });
        }

        const secondarySummary = summary.secondaryItems.get(secondaryKey)!;
        secondarySummary.budgetAmountCents += budgetAmountCents;
        secondarySummary.spentAmountCents += spentAmountCents;
        summary.secondaryBudgetAmountCents += budgetAmountCents;
        summary.secondarySpentAmountCents += spentAmountCents;
    }

    const grouped = new Map<string, {
        displayCategory: string;
        primaryCategory: string;
        secondaryCategory: string;
        budgetAmountCents: number;
        spentAmountCents: number;
        groupOrder: number;
        itemOrder: number;
    }>();

    for (const summary of groupedByPeriodAndPrimary.values()) {
        if (historicalBudgetLevel.value === 'primary') {
            const groupKey = summary.primaryCategory;
            const hasPrimaryBudget = summary.primaryBudgetAmountCents > 0 || summary.primarySpentAmountCents > 0;
            const budgetAmountCents = hasPrimaryBudget ? summary.primaryBudgetAmountCents : summary.secondaryBudgetAmountCents;
            const spentAmountCents = hasPrimaryBudget
                ? (summary.primarySpentAmountCents > 0 ? summary.primarySpentAmountCents : summary.secondarySpentAmountCents)
                : summary.secondarySpentAmountCents;

            if (!grouped.has(groupKey)) {
                grouped.set(groupKey, {
                    displayCategory: summary.primaryCategory,
                    primaryCategory: summary.primaryCategory,
                    secondaryCategory: '',
                    budgetAmountCents: 0,
                    spentAmountCents: 0,
                    groupOrder: summary.groupOrder,
                    itemOrder: 0
                });
            }

            const primarySummary = grouped.get(groupKey)!;
            primarySummary.budgetAmountCents += budgetAmountCents;
            primarySummary.spentAmountCents += spentAmountCents;
            continue;
        }

        const visibleSecondaryEntries = Array.from(summary.secondaryItems.entries())
            .filter(([, secondarySummary]) => secondarySummary.budgetAmountCents > 0 || secondarySummary.spentAmountCents > 0);
        const secondaryEntries: Array<[string, {
            displayCategory: string;
            secondaryCategory: string;
            budgetAmountCents: number;
            spentAmountCents: number;
            itemOrder: number;
        }]> = visibleSecondaryEntries.length > 0
            ? visibleSecondaryEntries
            : [[
                `${summary.primaryCategory}::${summary.primaryCategory}`,
                {
                    displayCategory: summary.primaryCategory,
                    secondaryCategory: summary.primaryCategory,
                    budgetAmountCents: summary.primaryBudgetAmountCents,
                    spentAmountCents: summary.primarySpentAmountCents,
                    itemOrder: 0
                }
            ]];

        for (const [secondaryKey, secondarySummary] of secondaryEntries) {
            if (secondarySummary.budgetAmountCents <= 0 && secondarySummary.spentAmountCents <= 0) {
                continue;
            }

            if (!grouped.has(secondaryKey)) {
                grouped.set(secondaryKey, {
                    displayCategory: secondarySummary.displayCategory,
                    primaryCategory: summary.primaryCategory,
                    secondaryCategory: secondarySummary.secondaryCategory,
                    budgetAmountCents: 0,
                    spentAmountCents: 0,
                    groupOrder: summary.groupOrder,
                    itemOrder: secondarySummary.itemOrder
                });
            }

            const groupedSecondarySummary = grouped.get(secondaryKey)!;
            groupedSecondarySummary.budgetAmountCents += secondarySummary.budgetAmountCents;
            groupedSecondarySummary.spentAmountCents += secondarySummary.spentAmountCents;
        }
    }

    const points: HistoricalCategoryChartPoint[] = [];
    let colorIdx = 0;

    for (const [, summary] of grouped) {
        const budgetAmountCents = summary.budgetAmountCents;
        const spentAmountCents = summary.spentAmountCents;
        const executionRate = summary.budgetAmountCents > 0
            ? (summary.spentAmountCents / summary.budgetAmountCents) * 100
            : 0;
        const subCategoryKey = `${summary.primaryCategory}::${summary.secondaryCategory}`;
        const color: string = (historicalBudgetLevel.value === 'secondary'
            ? subCategoryColorMap[subCategoryKey]
            : undefined)
            ?? categoryColorMap[summary.primaryCategory]
            ?? CATEGORY_CHART_PALETTE[colorIdx % CATEGORY_CHART_PALETTE.length] ?? '#5470c6';
        colorIdx++;

        points.push({
            category: summary.displayCategory,
            primaryCategory: summary.primaryCategory,
            secondaryCategory: summary.secondaryCategory,
            budgetAmountCents,
            spentAmountCents,
            executionRate: Number(executionRate.toFixed(1)),
            color,
            groupOrder: summary.groupOrder,
            itemOrder: summary.itemOrder
        });
    }

    points.sort((a, b) => {
        if (a.groupOrder !== b.groupOrder) {
            return a.groupOrder - b.groupOrder;
        }
        if (historicalBudgetLevel.value === 'secondary' && a.itemOrder !== b.itemOrder) {
            return a.itemOrder - b.itemOrder;
        }
        if (a.primaryCategory !== b.primaryCategory) {
            return a.primaryCategory.localeCompare(b.primaryCategory, 'zh-CN');
        }
        return a.category.localeCompare(b.category, 'zh-CN');
    });

    return { categories: points.map(p => p.category), points };
});

watch(
    () => historicalCategoryChartData.value.points.map(point => `${point.primaryCategory}::${point.secondaryCategory || point.category}`).join('|'),
    () => {
        historicalLegendSelection.value = syncHistoricalLegendSelection(
            historicalCategoryChartData.value.points,
            historicalLegendSelection.value
        );
        if (activeViewMode.value === 'history') {
            scheduleHistoricalChartResize();
        }
    },
    { immediate: true }
);

watch(activeBudgetType, () => {
    historicalLegendSelection.value = {};
    selectedHistoricalPeriodKey.value = null;
    resetHistoricalLabelAnimationState(historicalLabelAnimationState);
    if (activeViewMode.value === 'history') {
        scheduleHistoricalChartResize();
    }
});

watch(historicalBudgetLevel, () => {
    historicalLegendSelection.value = {};
    selectedHistoricalPeriodKey.value = null;
    resetHistoricalCategoryAnimationState(historicalLabelAnimationState);
    if (activeViewMode.value === 'history') {
        scheduleHistoricalChartResize();
    }
});

watch(historicalAggregationType, () => {
    selectedHistoricalPeriodKey.value = null;
});

const historicalChartModel = computed(() => {
    return buildHistoricalPolarChartModel(
        historicalCategoryChartData.value.points,
        historicalLegendSelection.value
    );
});
const historicalLabelAnimationState = createHistoricalLabelAnimationState();

const historicalCategoryMeta = computed<Record<string, HistoricalBudgetCategoryMeta>>(() => {
    const meta: Record<string, HistoricalBudgetCategoryMeta> = {};
    for (const cat of budgetPrimaryCategories.value) {
        const subMeta: Record<string, HistoricalBudgetCategoryMeta> = {};
        for (const sub of cat.subCategories || []) {
            subMeta[sub.name] = {
                color: sub.color,
                icon: sub.icon,
                iconColor: sub.color,
                displayOrder: sub.displayOrder ?? Number.MAX_SAFE_INTEGER
            };
        }
        meta[cat.name] = {
            color: cat.color,
            icon: cat.icon,
            iconColor: cat.color,
            displayOrder: cat.displayOrder ?? Number.MAX_SAFE_INTEGER,
            subCategories: subMeta
        };
    }
    return meta;
});

const historicalBudgetGroups = computed(() => {
    return buildHistoricalBudgetPeriodGroups({
        items: filteredHistoricalItems.value,
        aggregationType: historicalAggregationType.value,
        fiscalYearStartMonth: fiscalYearStartInfo.value.month,
        fiscalYearStartDay: fiscalYearStartInfo.value.day,
        uncategorizedLabel: tt('Uncategorized'),
        levelMode: historicalBudgetLevel.value,
        legendSelection: historicalLegendSelection.value,
        categoryMeta: historicalCategoryMeta.value,
        fallbackPalette: CATEGORY_CHART_PALETTE
    });
});

const visibleHistoricalBudgetGroups = computed(() => {
    if (!selectedHistoricalPeriodKey.value) {
        return historicalBudgetGroups.value;
    }
    return historicalBudgetGroups.value.filter(group => group.key === selectedHistoricalPeriodKey.value);
});
const areAllHistoricalGroupsExpanded = computed(() => (
    historicalBudgetGroups.value.length > 0
    && historicalExpandedGroupKeys.value.length === historicalBudgetGroups.value.length
));

watch(
    () => historicalBudgetGroups.value.map(group => group.key),
    (groupKeys) => {
        if (groupKeys.length === 0) {
            historicalExpandedGroupKeys.value = [];
            selectedHistoricalPeriodKey.value = null;
            return;
        }

        const retainedKeys = historicalExpandedGroupKeys.value.filter(groupKey => groupKeys.includes(groupKey));
        historicalExpandedGroupKeys.value = retainedKeys.length > 0 ? retainedKeys : [groupKeys[0] || ''];

        if (selectedHistoricalPeriodKey.value && !groupKeys.includes(selectedHistoricalPeriodKey.value)) {
            selectedHistoricalPeriodKey.value = null;
        }
    },
    { immediate: true }
);

watch(
    () => historicalBudgetGroups.value.flatMap(group => group.rows.map(row => `${group.key}::${row.key}`)),
    (rowKeys) => {
        historicalCollapsedPrimaryKeys.value = historicalCollapsedPrimaryKeys.value.filter(rowKey => rowKeys.includes(rowKey));
    },
    { immediate: true }
);

const historicalLegendGroups = computed(() => historicalChartModel.value.legendGroups);
const canShowHistoricalBudgetPanel = computed(() => (
    historicalLegendGroups.value.length > 0
    || historicalBudgetGroups.value.length > 0
    || !!selectedHistoricalPeriodKey.value
));
const historicalChartUpdateOptions = {
    notMerge: false,
    lazyUpdate: false,
    replaceMerge: ['series']
};
// Bump this when the historical chart's graphic/custom transition contract changes.
// It forces one component remount so old ECharts instances cannot keep stale leaveTo internals.
const HISTORICAL_CHART_RENDER_REVISION = 'history-animation-restore-v6';
const historicalChartRenderKey = computed(() => `${HISTORICAL_CHART_RENDER_REVISION}:${activeBudgetType.value}`);

const historicalChartOptions = computed(() => {
    if (!historicalChartModel.value.primaryBands.length) {
        return {};
    }

    const accentColor = activeBudgetType.value === BudgetType.Investment ? '#ffb300' : '#5c6bc0';
    const animationScope = `${activeBudgetType.value}-${historicalBudgetLevel.value}`;
    return buildHistoricalPolarChartOption(historicalChartModel.value, {
        isDarkMode: isDarkMode.value, themePalette: getApplicationThemeDefinition(theme.global.name.value).semantic,
        accentColor,
        budgetAmountLabel: tt('Budget Amount'),
        spentAmountLabel: tt('Spent Amount'),
        executionRateLabel: tt('Execution Rate'),
        formatAmount,
        showPrimaryRing: historicalBudgetLevel.value === 'secondary',
        categoryAnimationScope: animationScope,
        amountAxisRenderScope: animationScope,
        labelAnimationState: historicalLabelAnimationState
    });
});

watch(activeViewMode, (mode) => {
    if (mode === 'history') {
        scheduleHistoricalChartResize();
    }
}, { flush: 'post' });

watch(historicalChartOptions, () => {
    if (activeViewMode.value === 'history') {
        scheduleHistoricalChartResize();
    }
}, { flush: 'post' });

function scheduleHistoricalChartResize(): void {
    if (typeof window === 'undefined') {
        return;
    }

    void nextTick(() => {
        window.requestAnimationFrame(() => {
            const chartComponent = historicalChartRef.value;
            chartComponent?.resize?.();
            chartComponent?.chart?.resize?.();
        });
    });
}

function toggleHistoricalPrimaryLegend(primaryKey: string): void {
    historicalLegendSelection.value = toggleHistoricalPrimarySelection(
        historicalLegendSelection.value,
        historicalChartModel.value,
        primaryKey
    );
}

function toggleHistoricalSecondaryLegend(secondaryKey: string): void {
    historicalLegendSelection.value = toggleHistoricalSecondarySelection(
        historicalLegendSelection.value,
        secondaryKey
    );
}

function buildHistoricalPrimaryCollapseKey(groupKey: string, row: HistoricalBudgetCategoryRow): string {
    return `${groupKey}::${row.key}`;
}

function isHistoricalPrimaryCollapsed(groupKey: string, row: HistoricalBudgetCategoryRow): boolean {
    return historicalCollapsedPrimaryKeys.value.includes(buildHistoricalPrimaryCollapseKey(groupKey, row));
}

function toggleHistoricalPrimaryCollapse(groupKey: string, row: HistoricalBudgetCategoryRow): void {
    const key = buildHistoricalPrimaryCollapseKey(groupKey, row);
    historicalCollapsedPrimaryKeys.value = historicalCollapsedPrimaryKeys.value.includes(key)
        ? historicalCollapsedPrimaryKeys.value.filter(item => item !== key)
        : [...historicalCollapsedPrimaryKeys.value, key];
}

function ensureHistoricalDateRangeInitialized(): void {
    if (
        isValidHistoricalUnixTime(historicalMinDatetime.value)
        && isValidHistoricalUnixTime(historicalMaxDatetime.value)
        && historicalMinDatetime.value <= historicalMaxDatetime.value
    ) {
        return;
    }

    const range = getDefaultHistoricalDateRange();

    if (!range) {
        return;
    }

    historicalDateType.value = range.dateType;
    historicalMinDatetime.value = range.minTime;
    historicalMaxDatetime.value = range.maxTime;
}

function canShowHistoricalCustomDateRange(dateType: number): boolean {
    return historicalDateType.value === DateRange.Custom.type && dateType === DateRange.Custom.type;
}

function setHistoricalDateFilter(dateType: number): void {
    if (dateType === DateRange.Custom.type) {
        ensureHistoricalDateRangeInitialized();
        showHistoricalDateDialog.value = true;
        return;
    }

    const range = getDateRangeByDateType(dateType, firstDayOfWeek.value, fiscalYearStartValue.value);
    if (!range) {
        return;
    }

    historicalDateType.value = range.dateType;
    historicalMinDatetime.value = range.minTime;
    historicalMaxDatetime.value = range.maxTime;
    reload(false);
}

function onHistoricalDateRangeChange(minUnixTime: number, maxUnixTime: number): void {
    historicalDateType.value = getDateTypeByDateRange(
        minUnixTime,
        maxUnixTime,
        firstDayOfWeek.value,
        fiscalYearStartValue.value,
        DateRangeScene.AssetTrends
    );
    historicalMinDatetime.value = minUnixTime;
    historicalMaxDatetime.value = maxUnixTime;
    showHistoricalDateDialog.value = false;
    reload(false);
}

function shiftHistoricalDateRange(scale: number): void {
    if (!canShiftHistoricalDateRange.value) {
        return;
    }

    const range = getShiftedDateRangeAndDateType(
        historicalMinDatetime.value,
        historicalMaxDatetime.value,
        scale,
        firstDayOfWeek.value,
        fiscalYearStartValue.value,
        DateRangeScene.AssetTrends
    );

    historicalDateType.value = range.dateType;
    historicalMinDatetime.value = range.minTime;
    historicalMaxDatetime.value = range.maxTime;
    reload(false);
}

async function loadHistoricalBudgetView(requestId: number): Promise<void> {
    const historyRange = getHistoricalBudgetQueryRange();
    const historyRequest: BudgetHistoryRequest = {
        type: activeBudgetType.value,
        periodType: getHistoricalRequestPeriodType(),
        startDate: historyRange.startDate,
        endDate: historyRange.endDate,
        categoryId: categoryFilter.value || undefined,
        accountIds: accountFilter.value.length ? [...accountFilter.value] : undefined,
        tagIds: tagFilter.value.length ? [...tagFilter.value] : undefined
    };

    try {
        await budgetStore.createBudgetHistorySnapshot({
            ...historyRequest,
            type: activeBudgetType.value
        });
    } catch (snapshotError) {
        logger.warn('[Budget List] Failed to create history snapshot during historical view load', snapshotError);
    }

    if (requestId !== reloadRequestId.value) {
        return;
    }

    await budgetStore.loadBudgetHistory(historyRequest);
    if (activeViewMode.value === 'history') {
        scheduleHistoricalChartResize();
    }
}

async function loadBudgetHistoryForExpired(requestId: number): Promise<void> {
    await loadHistoricalBudgetView(requestId);
}

// ============================================================================
// 方法
// ============================================================================

/**
 * 根据周期筛选预算
 */
function filterByPeriod(budgets: Budget[]): Budget[] {
    const now = new Date();
    const currentYear = now.getFullYear();
    const currentMonth = now.getMonth();
    const currentQuarter = Math.floor(currentMonth / 3);

    switch (activePeriodFilter.value) {
        case 'thisMonth': {
            const startDate = new Date(currentYear, currentMonth, 1);
            const endDate = new Date(currentYear, currentMonth + 1, 0);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Monthly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'lastMonth': {
            const startDate = new Date(currentYear, currentMonth - 1, 1);
            const endDate = new Date(currentYear, currentMonth, 0);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Monthly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'thisQuarter': {
            const startDate = new Date(currentYear, currentQuarter * 3, 1);
            const endDate = new Date(currentYear, currentQuarter * 3 + 3, 0);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Quarterly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'lastQuarter': {
            const prevQuarter = currentQuarter === 0 ? 3 : currentQuarter - 1;
            const year = currentQuarter === 0 ? currentYear - 1 : currentYear;
            const startDate = new Date(year, prevQuarter * 3, 1);
            const endDate = new Date(year, prevQuarter * 3 + 3, 0);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Quarterly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'thisYear': {
            const startDate = new Date(currentYear, 0, 1);
            const endDate = new Date(currentYear, 11, 31);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Yearly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'lastYear': {
            const startDate = new Date(currentYear - 1, 0, 1);
            const endDate = new Date(currentYear - 1, 11, 31);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Yearly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'expired': {
            const today = new Date();
            today.setHours(0, 0, 0, 0);
            return budgets.filter(b => {
                if (!b.endDate) return false;
                const endDate = new Date(b.endDate);
                return endDate < today;
            });
        }
        case 'custom': {
            if (!customStartDate.value || !customEndDate.value) return budgets;
            const startDate = new Date(customStartDate.value);
            const endDate = new Date(customEndDate.value);
            return budgets.filter(b => isDateInRange(b, startDate, endDate));
        }
        default:
            return budgets;
    }
}

function formatFilterAmountCents(amountCents: number): string {
    return formatAmountToLocalizedNumeralsWithCurrency(amountCents, defaultCurrency.value);
}

/**
 * 设置周期筛选器
 */
function setPeriodFilter(filter: string): void {
    activePeriodFilter.value = filter;
    reload(false);
}

function toggleAllHistoricalGroups(): void {
    if (areAllHistoricalGroupsExpanded.value) {
        historicalExpandedGroupKeys.value = [];
        return;
    }

    historicalExpandedGroupKeys.value = historicalBudgetGroups.value.map(group => group.key);
}

/**
 * 自定义日期范围变化处理
 */
function onCustomDateRangeChange(minUnixTime: number, maxUnixTime: number): void {
    // 更新时间戳
    customMinDatetime.value = minUnixTime;
    customMaxDatetime.value = maxUnixTime;

    // 转换为日期字符串（四位年-两位月-两位日）
    // 时间戳单位是秒，需要乘以 1000 转换为毫秒
    const minDate = new Date(minUnixTime * 1000);
    const maxDate = new Date(maxUnixTime * 1000);
    customStartDate.value = `${minDate.getFullYear()}-${String(minDate.getMonth() + 1).padStart(2, '0')}-${String(minDate.getDate()).padStart(2, '0')}`;
    customEndDate.value = `${maxDate.getFullYear()}-${String(maxDate.getMonth() + 1).padStart(2, '0')}-${String(maxDate.getDate()).padStart(2, '0')}`;

    // 设置为自定义模式并关闭对话框
    activePeriodFilter.value = 'custom';
    showCustomDateDialog.value = false;
    reload(false);
}

/**
 * 日期范围选择错误处理
 */
function onDateRangeError(message: string): void {
    snackbar.value?.showMessage(message);
}

/**
 * 获取账户筛选器显示名称
 */
function getAccountFilterDisplayName(): string {
    if (accountFilter.value.length === 0) {
        return tt('All Accounts');
    }
    if (accountFilter.value.length === 1) {
        const account = allAccounts.value.find(a => a.id === accountFilter.value[0]);
        return account ? account.name : tt('Selected Account');
    }
    return `${accountFilter.value.length} ${tt('Accounts')}`;
}

/**
 * 获取标签筛选器显示名称
 */
function getTagFilterDisplayName(): string {
    if (tagFilter.value.length === 0) {
        return tt('All Tags');
    }
    if (tagFilter.value.length === 1) {
        const tag = allTransactionTags.value.find(t => t.id === tagFilter.value[0]);
        return tag ? tag.name : tt('Selected Tag');
    }
    return `${tagFilter.value.length} ${tt('Tags')}`;
}

/**
 * 处理账户筛选变化
 */
function setAccountFilter(changed: boolean): void {
    showFilterAccountDialog.value = false;

    if (changed) {
        reload(false);
    }
}

/**
 * 处理标签筛选变化
 */
function setTagFilter(changed: boolean): void {
    showFilterTagDialog.value = false;

    if (changed) {
        reload(false);
    }
}

/**
 * 处理分类筛选对话框变化
 */
function onCategoryFilterDialogChange(changed: boolean): void {
    showFilterCategoryDialog.value = false;

    if (changed) {
        reload(false);
    }
}

/**
 * 设置关键词筛选
 */
function setKeywordFilter(keyword: string): void {
    searchKeyword.value = keyword;
}

// ============================================================================
// 已花费筛选相关函数
// ============================================================================

/**
 * 点击已花费筛选类型
 */
function onSpentFilterTypeClick(filterType: string): void {
    if (currentSpentFilterType.value === filterType) {
        currentSpentFilterType.value = '';
    } else {
        currentSpentFilterType.value = filterType;
    }
}

/**
 * 改变已花费筛选
 */
function changeSpentFilter(filterType: string): void {
    currentSpentFilterType.value = '';

    if (!filterType) {
        spentAmountFilterCents.value = '';
        return;
    }

    const amountCount = getAmountFilterParameterCount(filterType);
    if (!amountCount) return;

    let amountFilterCents = filterType;

    if (amountCount === 1) {
        amountFilterCents += ':' + currentSpentFilterValue1.value;
    } else if (amountCount === 2) {
        if (currentSpentFilterValue2.value < currentSpentFilterValue1.value) {
            snackbar.value?.showMessage(tt('Incorrect amount range'));
            return;
        }
        amountFilterCents += ':' + currentSpentFilterValue1.value + ':' + currentSpentFilterValue2.value;
    } else {
        return;
    }

    spentAmountFilterCents.value = amountFilterCents;
}

/**
 * 获取已花费筛选器的显示名称
 */
function getSpentFilterDisplayName(): string {
    if (!spentAmountFilterCents.value) return tt('Spent');

    const parsed = parseAmountFilterCents(spentAmountFilterCents.value);
    if (!parsed) return tt('Spent');

    const filterType = AmountFilterType.valueOf(parsed.type);
    if (!filterType) return tt('Spent');

    const typeName = tt(filterType.name);
    if (parsed.value2Cents !== undefined) {
        return `${typeName} ${formatFilterAmountCents(parsed.value1Cents)}~${formatFilterAmountCents(parsed.value2Cents)}`;
    }
    return `${typeName} ${formatFilterAmountCents(parsed.value1Cents)}`;
}

/**
 * 获取已花费筛选器的简短标签（用于更多设置菜单）
 */
function getSpentFilterLabel(): string {
    if (!spentAmountFilterCents.value) return '';
    const parsed = parseAmountFilterCents(spentAmountFilterCents.value);
    if (!parsed) return '';
    const filterType = AmountFilterType.valueOf(parsed.type);
    if (!filterType) return '';
    if (parsed.value2Cents !== undefined) {
        return `${formatFilterAmountCents(parsed.value1Cents)}~${formatFilterAmountCents(parsed.value2Cents)}`;
    }
    return `${tt(filterType.name)} ${formatFilterAmountCents(parsed.value1Cents)}`;
}

// ============================================================================
// 总预算筛选相关函数
// ============================================================================

/**
 * 点击总预算筛选类型
 */
function onBudgetFilterTypeClick(filterType: string): void {
    if (currentBudgetFilterType.value === filterType) {
        currentBudgetFilterType.value = '';
    } else {
        currentBudgetFilterType.value = filterType;
    }
}

/**
 * 改变总预算筛选
 */
function changeBudgetFilter(filterType: string): void {
    currentBudgetFilterType.value = '';

    if (!filterType) {
        budgetAmountFilterCents.value = '';
        return;
    }

    const amountCount = getAmountFilterParameterCount(filterType);
    if (!amountCount) return;

    let amountFilterCents = filterType;

    if (amountCount === 1) {
        amountFilterCents += ':' + currentBudgetFilterValue1.value;
    } else if (amountCount === 2) {
        if (currentBudgetFilterValue2.value < currentBudgetFilterValue1.value) {
            snackbar.value?.showMessage(tt('Incorrect amount range'));
            return;
        }
        amountFilterCents += ':' + currentBudgetFilterValue1.value + ':' + currentBudgetFilterValue2.value;
    } else {
        return;
    }

    budgetAmountFilterCents.value = amountFilterCents;
}

/**
 * 获取总预算筛选器的显示名称
 */
function getBudgetFilterDisplayName(): string {
    if (!budgetAmountFilterCents.value) return tt('Budget');

    const parsed = parseAmountFilterCents(budgetAmountFilterCents.value);
    if (!parsed) return tt('Budget');

    const filterType = AmountFilterType.valueOf(parsed.type);
    if (!filterType) return tt('Budget');

    const typeName = tt(filterType.name);
    if (parsed.value2Cents !== undefined) {
        return `${typeName} ${formatFilterAmountCents(parsed.value1Cents)}~${formatFilterAmountCents(parsed.value2Cents)}`;
    }
    return `${typeName} ${formatFilterAmountCents(parsed.value1Cents)}`;
}

/**
 * 获取总预算筛选器的简短标签（用于更多设置菜单）
 */
function getBudgetFilterLabel(): string {
    if (!budgetAmountFilterCents.value) return '';
    const parsed = parseAmountFilterCents(budgetAmountFilterCents.value);
    if (!parsed) return '';
    const filterType = AmountFilterType.valueOf(parsed.type);
    if (!filterType) return '';
    if (parsed.value2Cents !== undefined) {
        return `${formatFilterAmountCents(parsed.value1Cents)}~${formatFilterAmountCents(parsed.value2Cents)}`;
    }
    return `${tt(filterType.name)} ${formatFilterAmountCents(parsed.value1Cents)}`;
}

/**
 * 清除所有筛选器
 */
function clearAllFilters(): void {
    categoryFilter.value = null;
    executionRateFilter.value = null;
    spentAmountFilterCents.value = '';
    budgetAmountFilterCents.value = '';
    currentSpentFilterType.value = '';
    currentSpentFilterValue1.value = 0;
    currentSpentFilterValue2.value = 0;
    currentBudgetFilterType.value = '';
    currentBudgetFilterValue1.value = 0;
    currentBudgetFilterValue2.value = 0;
}

// ============================================================================
// 筛选预设相关函数
// ============================================================================

/**
 * 保存当前筛选为预设
 */
function savePreset(): void {
    if (!presetName.value.trim()) {
        snackbar.value?.showMessage(tt('Please enter a preset name'));
        return;
    }

    const preset: FilterPreset = {
        id: Date.now().toString(),
        name: presetName.value.trim(),
        categoryFilter: categoryFilter.value,
        accountFilter: [...accountFilter.value],
        tagFilter: [...tagFilter.value],
        executionRateFilter: executionRateFilter.value,
        spentAmountFilterCents: spentAmountFilterCents.value,
        budgetAmountFilterCents: budgetAmountFilterCents.value
    };

    filterPresets.value.push(preset);
    savePresetsToStorage();
    showSavePresetDialog.value = false;
    presetName.value = '';
    snackbar.value?.showMessage(tt('Preset saved successfully'));
}

/**
 * 加载筛选预设
 */
function loadPreset(preset: FilterPreset): void {
    categoryFilter.value = preset.categoryFilter;
    accountFilter.value = [...preset.accountFilter];
    tagFilter.value = [...preset.tagFilter];
    executionRateFilter.value = preset.executionRateFilter;
    spentAmountFilterCents.value = preset.spentAmountFilterCents;
    budgetAmountFilterCents.value = preset.budgetAmountFilterCents;
    snackbar.value?.showMessage(tt('Preset loaded'));
}

/**
 * 删除筛选预设
 */
function deletePreset(presetId: string): void {
    filterPresets.value = filterPresets.value.filter(p => p.id !== presetId);
    savePresetsToStorage();
    snackbar.value?.showMessage(tt('Preset deleted'));
}

/**
 * 获取分类筛选显示名称
 */
function getCategoryFilterDisplayName(): string {
    if (!categoryFilter.value) {
        return '';
    }

    const category = allCategoriesMap.value[categoryFilter.value];
    if (!category) {
        return categoryFilter.value;
    }

    // 如果是子分类，显示"父分类 > 子分类"
    if (category.parentId) {
        const parentCategory = allCategoriesMap.value[category.parentId];
        if (parentCategory) {
            return `${parentCategory.name} > ${category.name}`;
        }
    }

    return category.name;
}

/**
 * 设置执行度筛选
 */
function setExecutionRateFilter(option: RangeFilter | null): void {
    executionRateFilter.value = option;
}

/**
 * 获取预算进度条颜色（优先使用分类颜色，否则使用执行率颜色）
 */
function getBudgetProgressColor(budget: Budget): string {
    // 优先使用分类颜色
    if (budget.categoryColor) {
        const categoryColor = toCssCategoryColor(budget.categoryColor);
        if (categoryColor) {
            return categoryColor;
        }
    }
    // 降级使用执行率颜色
    return getExecutionRateColor(budget.executionRate);
}

/**
 * 获取分组进度条颜色（用于一级分类行）
 */
function getGroupProgressColor(group: BudgetGroup): string {
    // 优先使用分类颜色
    if (group.categoryColor) {
        const groupColor = toCssCategoryColor(group.categoryColor);
        if (groupColor) {
            return groupColor;
        }
    }
    // 如果有一级分类预算，使用其分类颜色
    const primaryBudget = getPrimaryBudgetForHeader(group);
    if (primaryBudget && primaryBudget.categoryColor) {
        const primaryBudgetColor = toCssCategoryColor(primaryBudget.categoryColor);
        if (primaryBudgetColor) {
            return primaryBudgetColor;
        }
    }
    // 降级使用执行率颜色
    return getExecutionRateColor(getGroupExecutionRate(group));
}

function toggleHistoricalPeriodFocus(periodKey: string): void {
    if (selectedHistoricalPeriodKey.value === periodKey) {
        selectedHistoricalPeriodKey.value = null;
        return;
    }
    selectedHistoricalPeriodKey.value = periodKey;
    historicalExpandedGroupKeys.value = [periodKey];
}

/**
 * 获取分组执行率（一级分类汇总）
 *
 * 计算规则：
 * 1. 如果有一级分类预算且有已花费金额，使用一级分类的执行率
 * 2. 如果有一级分类预算但没有已花费，则按“总已花费 / 一级预算金额”计算
 * 3. 如果没有一级分类预算，则按“总已花费 / 总预算金额”计算（二级分类之和）
 */
function getGroupExecutionRate(group: BudgetGroup): number {
    // 如果有一级分类预算
    const primaryBudget = getPrimaryBudgetForHeader(group);
    if (group.primaryBudgets.length > 1 && group.totalAmountCents > 0) {
        return (group.totalSpentCents / group.totalAmountCents) * 100;
    }
    if (primaryBudget) {
        // 如果一级分类预算有执行率（后端已计算），使用它
        if (group.primaryBudgets.length === 1 && primaryBudget.executionRate > 0) {
            return primaryBudget.executionRate;
        }
        // 否则按“总已花费 / 一级预算金额”计算
        if (group.primaryAmountCents > 0) {
            return (group.totalSpentCents / group.primaryAmountCents) * 100;
        }
    }

    // 没有一级分类预算，使用二级分类汇总计算
    if (group.totalAmountCents > 0) {
        return (group.totalSpentCents / group.totalAmountCents) * 100;
    }

    return 0;
}

/**
 * 获取分组执行率文本
 */
function getGroupExecutionRateText(group: BudgetGroup): string {
    const rate = getGroupExecutionRate(group);
    return rate.toFixed(1) + '%';
}

/**
 * 点击进度条跳转到对应分类和时间的账单列表
 * @param category 主分类名称
 * @param budget 预算对象（用于获取日期范围）
 */
function navigateToTransactions(category: string, budget: Budget | null): void {
    const drilldownDateRange = getBudgetDrilldownDateRange(budget);
    const query = buildBudgetDrilldownRouteQuery({
        categories: budgetPrimaryCategories.value,
        primaryCategoryName: category,
        secondaryCategoryName: budget?.subCategory || null,
        fallbackCategoryId: budget?.categoryId || '',
        startDate: drilldownDateRange?.startDate,
        endDate: drilldownDateRange?.endDate,
        transactionType: activeBudgetType.value === BudgetType.Expense ? 3 : 5,
        accountIds: accountFilter.value,
        tagIds: tagFilter.value
    });

    logger.debug(`[Budget] 跳转到账单列表: category=${category}, query=`, query);

    // 跳转到账单列表页面
    router.push({
        path: '/transaction/list',
        query: query
    });
}

function getBudgetDrilldownDateRange(budget: Budget | null): { startDate: string; endDate: string } | null {
    if (budget?.startDate && budget.endDate) {
        return {
            startDate: budget.startDate,
            endDate: budget.endDate
        };
    }

    const periodRequest = getCurrentPeriodRequest();

    if (periodRequest.startDate && periodRequest.endDate) {
        return {
            startDate: periodRequest.startDate,
            endDate: periodRequest.endDate
        };
    }

    if (periodRequest.periodType === BudgetPeriodType.Monthly && periodRequest.year && periodRequest.month) {
        return {
            startDate: formatDateOnly(new Date(periodRequest.year, periodRequest.month - 1, 1)),
            endDate: formatDateOnly(new Date(periodRequest.year, periodRequest.month, 0))
        };
    }

    if (periodRequest.periodType === BudgetPeriodType.Quarterly && periodRequest.year && periodRequest.quarter) {
        const quarterStartMonth = (periodRequest.quarter - 1) * 3;
        return {
            startDate: formatDateOnly(new Date(periodRequest.year, quarterStartMonth, 1)),
            endDate: formatDateOnly(new Date(periodRequest.year, quarterStartMonth + 3, 0))
        };
    }

    if (periodRequest.periodType === BudgetPeriodType.Yearly && periodRequest.year) {
        return {
            startDate: formatDateOnly(new Date(periodRequest.year, 0, 1)),
            endDate: formatDateOnly(new Date(periodRequest.year, 11, 31))
        };
    }

    return null;
}

/**
 * 切换视图模式
 */
function switchViewMode(mode: unknown): void {
    activeViewMode.value = mode as BudgetViewMode;
    if (mode === 'forecast') {
        if (!['thisMonth', 'thisQuarter', 'thisYear'].includes(activePeriodFilter.value)) {
            activePeriodFilter.value = 'thisMonth';
        }
        loadForecast();
    } else {
        reload(false);
        if (mode === 'history') {
            scheduleHistoricalChartResize();
        }
    }
}

/**
 * 切换预算类型
 */
function switchBudgetType(type: unknown): void {
    activeBudgetType.value = type as BudgetType;
    reload(false);
}

/**
 * 获取当前周期类型
 */
function getCurrentPeriodType(): BudgetPeriodType {
    if (['thisMonth', 'lastMonth'].includes(activePeriodFilter.value)) {
        return BudgetPeriodType.Monthly;
    } else if (['thisQuarter', 'lastQuarter'].includes(activePeriodFilter.value)) {
        return BudgetPeriodType.Quarterly;
    } else if (['thisYear', 'lastYear'].includes(activePeriodFilter.value)) {
        return BudgetPeriodType.Yearly;
    }
    return BudgetPeriodType.Monthly;
}

function formatDateOnly(date: Date): string {
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

function getCurrentPeriodRequest(): BudgetHistoryRequest {
    const now = new Date();
    const currentYear = now.getFullYear();
    const currentMonth = now.getMonth();
    const currentQuarter = Math.floor(currentMonth / 3) + 1;

    switch (activePeriodFilter.value) {
        case 'thisMonth':
            return {
                periodType: BudgetPeriodType.Monthly,
                year: currentYear,
                month: currentMonth + 1
            };
        case 'lastMonth': {
            const target = new Date(currentYear, currentMonth - 1, 1);
            return {
                periodType: BudgetPeriodType.Monthly,
                year: target.getFullYear(),
                month: target.getMonth() + 1
            };
        }
        case 'thisQuarter':
            return {
                periodType: BudgetPeriodType.Quarterly,
                year: currentYear,
                quarter: currentQuarter
            };
        case 'lastQuarter': {
            const target = new Date(currentYear, currentMonth - 3, 1);
            return {
                periodType: BudgetPeriodType.Quarterly,
                year: target.getFullYear(),
                quarter: Math.floor(target.getMonth() / 3) + 1
            };
        }
        case 'thisYear':
            return {
                periodType: BudgetPeriodType.Yearly,
                year: currentYear
            };
        case 'lastYear':
            return {
                periodType: BudgetPeriodType.Yearly,
                year: currentYear - 1
            };
        case 'custom':
            return {
                periodType: getCurrentPeriodType(),
                startDate: customStartDate.value || undefined,
                endDate: customEndDate.value || undefined
            };
        default:
            return {
                periodType: getCurrentPeriodType()
            };
    }
}

/**
 * 加载预算列表和执行数据
 */
async function reload(force: boolean): Promise<void> {
    const requestId = ++reloadRequestId.value;
    loading.value = true;

    try {
        if (activeViewMode.value === 'history') {
            await loadHistoricalBudgetView(requestId);
            return;
        }

        const periodRequest = getCurrentPeriodRequest();
        const categoryLoadPromise = transactionCategoriesStore.loadAllCategories({ force: false });
        const budgetLoadPromise = budgetStore.loadAllBudgets({
            force,
            type: activeBudgetType.value,
            periodType: periodRequest.periodType
        });

        const [categoryLoadResult, budgetLoadResult] = await Promise.allSettled([
            categoryLoadPromise,
            budgetLoadPromise
        ]);

        if (budgetLoadResult.status === 'rejected') {
            throw budgetLoadResult.reason;
        }

        if (categoryLoadResult.status === 'rejected') {
            const categoryLoadError = categoryLoadResult.reason as { isUpToDate?: boolean; message?: string };
            if (!categoryLoadError.isUpToDate) {
                snackbar.value?.showError(categoryLoadError.message || tt('Failed to load categories'));
            }
        }

        await budgetStore.loadBudgetExecution({
            type: activeBudgetType.value,
            periodType: periodRequest.periodType,
            year: periodRequest.year,
            month: periodRequest.month,
            quarter: periodRequest.quarter,
            startDate: periodRequest.startDate,
            endDate: periodRequest.endDate
        });

        if (requestId !== reloadRequestId.value) {
            return;
        }

        loading.value = false;

        if (activePeriodFilter.value === 'expired') {
            void loadBudgetHistoryForExpired(requestId);
        }

        if (activeViewMode.value === 'forecast') {
            void loadForecast();
        }
    } catch (error: unknown) {
        const err = error as { isUpToDate?: boolean; message?: string };
        if (!err.isUpToDate) {
            snackbar.value?.showError(err.message || tt('Failed to load budgets'));
        }
    } finally {
        if (requestId === reloadRequestId.value) {
            loading.value = false;
        }
    }
}

/**
 * 加载周期预计
 */
async function loadForecast(): Promise<void> {
    try {
        const periodRequest = getCurrentPeriodRequest();

        await budgetStore.loadBudgetForecast(
            buildBudgetForecastLoadRequest({
                budgetType: activeBudgetType.value,
                periodRequest,
                monthsHistory: forecastMonthsHistory.value,
                forecastStrategy: forecastStrategy.value
            })
        );
    } catch (error: unknown) {
        const err = error as { message?: string };
        snackbar.value?.showError(err.message || tt('Failed to load forecast'));
    }
}

/**
 * 添加预算
 */
function add(): void {
    const newBudget = Budget.createNew(activeBudgetType.value);
    newBudget.periodType = getCurrentPeriodType();
    editDialog.value?.open({ budget: newBudget, type: activeBudgetType.value });
}

/**
 * 添加一级分类预算（为已有子分类的分类组添加一级分类预算）
 */
function addPrimaryBudget(group: BudgetGroup): void {
    const newBudget = Budget.createNew(activeBudgetType.value);
    newBudget.periodType = getCurrentPeriodType();
    newBudget.category = group.category;
    newBudget.subCategory = '';  // 一级分类预算的子分类为空
    newBudget.amountCents = group.totalAmountCents;  // 初始金额设为当前汇总金额
    newBudget.categoryIcon = group.categoryIcon;
    newBudget.categoryColor = group.categoryColor;
    editDialog.value?.open({ budget: newBudget, type: activeBudgetType.value, usePrimaryCategoryOnly: true });
}

/**
 * 编辑预算
 */
function edit(budget: Budget): void {
    editDialog.value?.open({ budget, type: activeBudgetType.value });
}

/**
 * 删除预算
 */
function remove(budget: Budget): void {
    const budgetName = budget.name || budget.fullCategoryName;
    confirmDialog.value?.open(
        tt('Delete Budget'),
        tt('Are you sure you want to delete this budget "{name}"?', { name: budgetName }),
        { color: 'warning' }  // 橙色，与其他删除弹窗一致
    ).then(result => {
        if (result) {
            budgetRemoving.value[budget.id] = true;

            budgetStore.deleteBudget({ budgetId: budget.id }).then(() => {
                snackbar.value?.showMessage(tt('Budget deleted successfully'));
            }).catch((error: unknown) => {
                const err = error as { message?: string };
                snackbar.value?.showError(err.message || tt('Failed to delete budget'));
            }).finally(() => {
                budgetRemoving.value[budget.id] = false;
            });
        }
    });
}

/**
 * 预算保存回调
 */
function onBudgetSaved(): void {
    reload(true);
}

/**
 * 导出预算
 */
async function exportBudgets(): Promise<void> {
    try {
        const result = await budgetStore.exportBudgets();

        // 下载导出的预算文件
        const dataStr = JSON.stringify(result.budgets, null, 2);
        const blob = new Blob([dataStr], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = url;
        link.download = `budgets_export_${new Date().toISOString().split('T')[0]}.json`;
        link.click();
        URL.revokeObjectURL(url);

        snackbar.value?.showMessage('Budgets exported successfully');
    } catch (error: unknown) {
        const err = error as { message?: string };
        snackbar.value?.showError(err.message || tt('Failed to export budgets'));
    }
}

/**
 * 导入预算
 */
function importBudgets(): void {
    fileInput.value?.click();
}

/**
 * 文件选择处理
 */
async function onFileSelected(event: Event): Promise<void> {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];

    if (!file) return;

    try {
        const text = await file.text();
        const data = JSON.parse(text);

        // 验证数据格式
        if (!Array.isArray(data)) {
            throw new Error('Invalid file format');
        }

        // 确认导入
        confirmDialog.value?.open(
            tt('Import Budgets'),
            tt('Are you sure you want to import {count} budgets?', { count: data.length })
        ).then(async (result) => {
            if (result) {
                try {
                    const importResult = await budgetStore.importBudgets({ budgets: data, overwriteExisting: true });
                    snackbar.value?.showMessage(
                        tt('Imported {imported} budgets, updated {updated} budgets', {
                            imported: importResult.importedCount,
                            updated: importResult.updatedCount
                        })
                    );
                    reload(true);
                } catch (error: unknown) {
                    const err = error as { message?: string };
                    snackbar.value?.showError(err.message || tt('Failed to import budgets'));
                }
            }
        });
    } catch {
        snackbar.value?.showError(tt('Failed to parse file'));
    } finally {
        // 重置文件输入
        target.value = '';
    }
}

// ============================================================================
// 生命周期
// ============================================================================

onMounted(() => {
    // 应用初始参数
    if (props.initType) {
        activeBudgetType.value = parseInt(props.initType) as BudgetType;
    }
    if (props.initPeriodType) {
        // 映射周期类型到筛选器
        const periodMap: Record<string, string> = {
            [BudgetPeriodType.Monthly]: 'thisMonth',
            [BudgetPeriodType.Quarterly]: 'thisQuarter',
            [BudgetPeriodType.Yearly]: 'thisYear'
        };
        activePeriodFilter.value = periodMap[props.initPeriodType] || 'thisMonth';
    }
    if (props.initViewMode) {
        if (isBudgetViewMode(props.initViewMode)) {
            activeViewMode.value = props.initViewMode;
        }
    }

    // 加载筛选预设
    loadPresetsFromStorage();

    ensureHistoricalDateRangeInitialized();

    // 初始加载
    reload(false);
});

watch([forecastStrategy, forecastMonthsHistory], () => {
    if (activeViewMode.value === 'forecast') {
        loadForecast();
    }
});

// 监听筛选关键词变化
watch(filterKeyword, (newVal) => {
    // 实时搜索
    searchKeyword.value = newVal;
});
useExternalTemplateBindings(accountFilter, AccountFilterSettingsCard, accountsStore, activeBudgetPeriodType, activeBudgetRelativeScope, activeBudgetType, activeHistoricalAggregationType, activeHistoricalHistoryRequestSignature, activePeriodFilter, activePeriodFilterIndex, activeViewMode, add, addMonths, addPrimaryBudget, allAccounts, allBudgets, allCategoriesMap, allHistoricalDateRanges, allowedCategoryTypes, allTransactionTags, alwaysShowNav, AmountFilterType, AmountInput, areAllHistoricalGroupsExpanded, BtnHorizontalGroup, BtnVerticalGroup, Budget, budgetAmountFilterCents, BudgetForecastPanel, BudgetForecastSettingsDialog, BudgetForecastStrategy, BudgetHistoryPanel, BudgetListTable, BudgetPeriodType, budgetPeriodTypeButtons, budgetPrimaryCategories, budgetRemoving, budgetStore, BudgetType, buildBudgetDrilldownRouteQuery, buildBudgetForecastLoadRequest, buildBudgetHistoryRequestSignature, buildFiscalYearPeriod, buildHistoricalAggregationPeriods, buildHistoricalBudgetPeriodGroups, buildHistoricalPolarChartModel, buildHistoricalPolarChartOption, buildHistoricalPrimaryCollapseKey, canShiftHistoricalDateRange, canShowHistoricalBudgetPanel, canShowHistoricalCustomDateRange, CATEGORY_CHART_PALETTE, categoryFilter, CategoryFilterSettingsCard, CategoryType, changeBudgetFilter, changeSpentFilter, clearAllFilters, collapsedCategories, computed, confirmDialog, ConfirmDialog, createHistoricalLabelAnimationState, currentBudgetFilterType, currentBudgetFilterValue1, currentBudgetFilterValue2, currentCategoryType, currentExecution, currentForecast, currentHistory, currentHistoryRequestSignature, currentSpentFilterType, currentSpentFilterValue1, currentSpentFilterValue2, currentViewTitle, customEndDate, customMaxDatetime, customMinDatetime, customStartDate, DateRange, DateRangeScene, DateRangeSelectionDialog, defaultCurrency, deletePreset, display, displayForecasts, edit, editDialog, EditDialog, ensureHistoricalDateRangeInitialized, executionRateFilter, executionRateOptions, exportBudgets, fileInput, filterAndSortForecasts, filterByPeriod, filteredBudgets, filteredHistoricalItems, filteredSummary, filterHistoricalBudgetItemsByType, filterKeyword, filterPresets, firstDayOfWeek, FiscalYearStart, fiscalYearStartInfo, fiscalYearStartValue, forecastLoading, forecastMonthsHistory, forecastOnlyLowConfidence, forecastOnlyOverBudget, forecastPeriodFilters, forecastRiskSummary, forecastSortBy, forecastSortOptions, forecastStrategies, forecastStrategy, formatAmount, formatDateOnly, formatFilterAmountCents, getAccountFilterDisplayName, getAmountFilterParameterCount, getBudgetCategoryColor, getBudgetCategoryIcon, getBudgetDrilldownDateRange, getBudgetFilterDisplayName, getBudgetFilterLabel, getBudgetPeriodTypeFromFilter, getBudgetProgressColor, getBudgetRelativeScopeFromFilter, getCategoryFilterDisplayName, getCurrentPeriodRequest, getCurrentPeriodType, getCurrentUnixTime, getDateRangeByDateType, getDateTypeByDateRange, getDefaultHistoricalDateRange, getExecutionRateColor, getExecutionRateColorClass, getExecutionRateTextClass, getExpandedPrimaryBudgets, getForecastConfidenceClass, getForecastConfidenceLabel, getGroupExecutionRate, getGroupExecutionRateText, getGroupProgressColor, getHistoricalBudgetQueryRange, getHistoricalDateRangeSnapshot, getHistoricalRequestPeriodType, getPrimaryBudgetForHeader, getProgressColorByRate, getShiftedDateRangeAndDateType, getSpentFilterDisplayName, getSpentFilterLabel, getTagFilterDisplayName, getTodayFirstUnixTime, groupedBudgets, groupHasExpandedRows, hasActiveFilters, HISTORICAL_CHART_RENDER_REVISION, HISTORICAL_FISCAL_YEAR, historicalAggregationType, historicalBudgetGroups, historicalBudgetLevel, historicalCategoryChartData, historicalCategoryMeta, historicalChartModel, historicalChartOptions, historicalChartRef, historicalChartRenderKey, historicalChartUpdateOptions, historicalCollapsedPrimaryKeys, historicalDateEndText, historicalDateRangeName, historicalDateStartText, historicalDateType, historicalExpandedGroupKeys, historicalLabelAnimationState, historicalLegendGroups, historicalLegendSelection, historicalLevelButtons, historicalMaxDatetime, historicalMinDatetime, historicalPeriods, historyAggregationButtons, historyPeriodOptions, importBudgets, isAllExpanded, isBudgetViewMode, isDarkApplicationTheme, isDarkMode, isDateInRange, isHistoricalHistoryReady, isHistoricalPrimaryCollapsed, isValidHistoricalUnixTime, loadBudgetHistoryForExpired, loadForecast, loadHistoricalBudgetView, loading, loadPreset, loadPresetsFromStorage, logger, matchAmountFilterCents, mdiArrowLeft, mdiArrowRight, mdiBookmark, mdiBookmarkOutline, mdiCashMinus, mdiCashPlus, mdiChartDonut, mdiCheck, mdiClose, mdiContentSaveOutline, mdiDotsVertical, mdiFilterRemoveOutline, mdiInformationOutline, mdiMagnify, mdiMenu, mdiRefresh, mdiShapeOutline, mdiTagOutline, mdiUnfoldLessHorizontal, mdiUnfoldMoreHorizontal, mdiWalletOutline, navigateToTransactions, nextTick, normalizeCategoryColor, normalizeHistoryAmountCents, onBudgetFilterTypeClick, onBudgetSaved, onCategoryFilterDialogChange, onCustomDateRangeChange, onDateRangeError, onFileSelected, onHistoricalDateRangeChange, onMounted, onSpentFilterTypeClick, parseAmountFilterCents, parseDateOnly, periodFilteredHistoricalItems, presetName, props, ref, reload, reloadRequestId, remove, resetHistoricalCategoryAnimationState, resetHistoricalLabelAnimationState, resolveHistoricalPeriodByDate, router, savePreset, savePresetsToStorage, scheduleHistoricalChartResize, searchKeyword, selectedHistoricalPeriodKey, setAccountFilter, setExecutionRateFilter, setHistoricalDateFilter, setKeywordFilter, setPeriodFilter, setTagFilter, settingsStore, shiftHistoricalDateRange, showCustomDateDialog, showFilterAccountDialog, showFilterCategoryDialog, showFilterTagDialog, showForecastSettingsDialog, showHistoricalDateDialog, showNav, showSavePresetDialog, snackbar, SnackBar, sortBy, sortDesc, sortedBudgets, spentAmountFilterCents, summarizeForecastRisks, switchBudgetType, switchViewMode, syncHistoricalLegendSelection, tagFilter, theme, toBudgetRelativePeriodFilter, toCssCategoryColor, toggleAllCategories, toggleAllHistoricalGroups, toggleCategoryCollapse, toggleHistoricalPeriodFocus, toggleHistoricalPrimaryCollapse, toggleHistoricalPrimaryLegend, toggleHistoricalPrimarySelection, toggleHistoricalSecondaryLegend, toggleHistoricalSecondarySelection, transactionCategoriesStore, TransactionCategory, TransactionTagFilterSettingsCard, transactionTagsStore, updating, useAccountsStore, useBudgetStore, useDisplay, useI18n, useRouter, userStore, useSettingsStore, useTemplateRef, useTheme, useTransactionCategoriesStore, useTransactionTagsStore, useUserStore, viewModeButtons, visibleHistoricalBudgetGroups, visiblePeriodFilters, watch);
</script>

<style scoped src="./list/ListPage.css"></style>
