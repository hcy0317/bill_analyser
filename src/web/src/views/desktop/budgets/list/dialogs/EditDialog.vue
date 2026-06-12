<template>
    <v-dialog class="budget-edit-dialog" width="800" :persistent="true" v-model="showState">
        <v-card class="pa-2 pa-sm-4 pa-md-8">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center">
                        <h4 class="text-h4">{{ isNew ? tt('Add Budget') : tt('Edit Budget') }}</h4>
                    </div>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" @click="close">
                        <v-icon :icon="mdiClose" size="24" />
                    </v-btn>
                </div>
            </template>

            <v-card-text class="d-flex flex-column mt-md-4 pt-0">
                <v-form ref="form" @submit.prevent="save">
                    <v-row>
                        <!-- 预算类型 + 周期类型 -->
                        <v-col cols="12" md="6">
                            <v-select
                                v-model="budget.type"
                                :label="tt('Budget Type')"
                                :items="budgetTypeOptions"
                                item-title="text"
                                item-value="value"
                                persistent-placeholder
                                :disabled="saving || !isNew"
                            />
                        </v-col>

                        <v-col cols="12" md="6">
                            <v-select
                                v-model="budget.periodType"
                                :label="tt('Period Type')"
                                :items="periodTypeOptions"
                                item-title="text"
                                item-value="value"
                                persistent-placeholder
                                :disabled="saving"
                            />
                        </v-col>

                        <!-- 分类选择 -->
                        <v-col cols="12" md="6">
                            <!-- 只选择一级分类 -->
                            <v-select
                                v-if="usePrimaryCategoryOnly"
                                v-model="budget.categoryId"
                                :items="availablePrimaryCategories"
                                item-title="name"
                                item-value="id"
                                :label="tt('Category')"
                                :placeholder="tt('None')"
                                :disabled="saving"
                                persistent-placeholder
                                @update:model-value="onCategoryChange"
                            >
                                <template #item="{ props: itemProps, item }">
                                    <v-list-item v-bind="itemProps">
                                        <template #prepend>
                                            <item-icon class="me-2" icon-type="category"
                                                        :icon-id="item.raw.icon"
                                                        :color="item.raw.color" />
                                        </template>
                                    </v-list-item>
                                </template>
                                <!-- 选中后只显示文字，不显示图标 -->
                                <template #selection="{ item }">
                                    <span>{{ item.title }}</span>
                                </template>
                            </v-select>

                            <!-- 选择一级+二级分类 -->
                            <two-column-select
                                v-else
                                primary-key-field="id"
                                primary-value-field="id"
                                primary-title-field="name"
                                primary-icon-field="icon"
                                primary-icon-type="category"
                                primary-color-field="color"
                                primary-hidden-field="hidden"
                                primary-sub-items-field="subCategories"
                                secondary-key-field="id"
                                secondary-value-field="id"
                                secondary-title-field="name"
                                secondary-icon-field="icon"
                                secondary-icon-type="category"
                                secondary-color-field="color"
                                secondary-hidden-field="hidden"
                                :disabled="saving"
                                :enable-filter="true"
                                :filter-placeholder="tt('Find category')"
                                :filter-no-items-text="tt('No available category')"
                                :show-selection-primary-text="true"
                                :custom-selection-primary-text="selectedPrimaryCategoryName"
                                :custom-selection-secondary-text="selectedSecondaryCategoryName"
                                :label="tt('Category')"
                                :placeholder="tt('Select Category')"
                                :items="availableCategories as unknown as Record<string, unknown>[]"
                                v-model="budget.categoryId"
                                @update:model-value="(value: unknown) => onCategoryChange(String(value))"
                            />
                        </v-col>

                        <!-- 预算金额（带货币符号，加粗显示）-->
                        <v-col cols="12" md="6">
                            <amount-input
                                class="budget-amount-input font-weight-bold"
                                color="primary"
                                currency="CNY"
                                :show-currency="true"
                                :readonly="false"
                                :disabled="saving"
                                :persistent-placeholder="true"
                                :label="tt('Budget Amount')"
                                :placeholder="tt('Budget Amount')"
                                :enable-formula="true"
                                :enable-rules="true"
                                v-model="budgetAmountInCents"
                            />
                        </v-col>

                        <!-- 一级分类开关 -->
                        <v-col cols="12" md="6">
                            <v-switch
                                v-model="usePrimaryCategoryOnly"
                                :label="tt('Primary Category Only')"
                                color="primary"
                                hide-details
                            />
                        </v-col>

                        <!-- 预警阈值（滑块设计）-->
                        <v-col cols="12" md="6">
                            <div class="alert-threshold-container">
                                <div class="d-flex justify-space-between align-center mb-1">
                                    <span class="text-body-2">{{ tt('Alert Threshold') }}</span>
                                    <span class="text-body-2 font-weight-bold">{{ budget.alertThreshold }}%</span>
                                </div>
                                <v-slider
                                    v-model="budget.alertThreshold"
                                    :min="0"
                                    :max="100"
                                    :step="1"
                                    :disabled="saving"
                                    color="primary"
                                    track-color="grey-lighten-2"
                                    thumb-label
                                    hide-details
                                >
                                    <template #thumb-label="{ modelValue }">
                                        {{ modelValue }}%
                                    </template>
                                </v-slider>
                            </div>
                        </v-col>

                        <!-- 开始日期 -->
                        <v-col cols="12" md="6">
                            <date-only-select
                                :disabled="saving"
                                :label="tt('Start Date')"
                                v-model="startDateTime"
                            />
                        </v-col>

                        <!-- 结束日期（可选，支持清除）-->
                        <v-col cols="12" md="6">
                            <date-only-select
                                :disabled="saving"
                                :label="tt('End Date')"
                                :placeholder="tt('Optional')"
                                :clearable="true"
                                v-model="endDateTime"
                            />
                        </v-col>

                        <!-- 备注（富文本框，移到日期下方）-->
                        <v-col cols="12">
                            <v-textarea
                                v-model="budget.name"
                                :label="tt('Remarks')"
                                :placeholder="tt('Enter remarks...')"
                                persistent-placeholder
                                :disabled="saving"
                                rows="3"
                                auto-grow
                                variant="outlined"
                            />
                        </v-col>
                    </v-row>
                </v-form>
            </v-card-text>

            <!-- 按钮区域 - 参考交易对话框的间距和样式 -->
            <v-card-text class="overflow-y-visible">
                <div class="w-100 d-flex justify-center mt-2 mt-sm-4 mt-md-6 gap-4">
                    <v-btn color="primary" variant="elevated" size="large"
                           min-width="120" :loading="saving" @click="save">
                        {{ tt('Save') }}
                    </v-btn>
                    <v-btn color="secondary" variant="tonal" size="large"
                           min-width="120" @click="close">
                        {{ tt('Cancel') }}
                    </v-btn>
                </div>
            </v-card-text>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import TwoColumnSelect from '@/components/desktop/TwoColumnSelect.vue';
import DateOnlySelect from '@/components/desktop/DateOnlySelect.vue';
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import { findBudgetCategoryIdByNames, resolveBudgetCategorySelection } from '../../categorySelection.ts';

import { ref, computed, watch } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { useBudgetStore } from '@/stores/budget.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';

import {
    Budget,
    BudgetType,
    BudgetPeriodType
} from '@/models/budget.ts';
import { CategoryType } from '@/core/category.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';

import { mdiClose } from '@mdi/js';

import logger from '@/lib/logger.ts';

// ============================================================================
// 事件定义
// ============================================================================

const emit = defineEmits<{
    (e: 'budget:saved'): void;
}>();

// ============================================================================
// 组件引用
// ============================================================================

const { tt } = useI18n();
const budgetStore = useBudgetStore();
const categoryStore = useTransactionCategoriesStore();

// ============================================================================
// 响应式状态
// ============================================================================

const showState = ref<boolean>(false);
const saving = ref<boolean>(false);
const budget = ref<Budget>(new Budget());
// AmountInput组件返回的值已经是分(cents)，不是元(yuan)
// parseAmount函数会自动将用户输入的元乘以100转换为分
const budgetAmountInCents = ref<number>(0);
const originalBudget = ref<Budget | null>(null);

// 日期时间选择器的值（Unix时间戳毫秒）
const startDateTime = ref<number>(0);
const endDateTime = ref<number>(0);

// 分类选择模式：是否只选择一级分类
const usePrimaryCategoryOnly = ref<boolean>(false);

// ============================================================================
// 计算属性
// ============================================================================

const isNew = computed<boolean>(() => !budget.value.id);

const budgetTypeOptions = computed(() => [
    { text: tt('Expense'), value: BudgetType.Expense },
    { text: tt('Investment'), value: BudgetType.Investment }
]);

const periodTypeOptions = computed(() => [
    { text: tt('Monthly'), value: BudgetPeriodType.Monthly },
    { text: tt('Quarterly'), value: BudgetPeriodType.Quarterly },
    { text: tt('Yearly'), value: BudgetPeriodType.Yearly }
]);

// 根据预算类型获取对应的分类类型数字值
const categoryTypeValue = computed<number>(() => {
    return budget.value.type === BudgetType.Expense
        ? CategoryType.Expense
        : CategoryType.Investment;
});

// 可用的分类列表
const availableCategories = computed<TransactionCategory[]>(() => {
    return categoryStore.allTransactionCategories[categoryTypeValue.value] || [];
});

// 可用的一级分类列表（从allTransactionCategories获取，已经是顶级分类列表）
const availablePrimaryCategories = computed<TransactionCategory[]>(() => {
    // allTransactionCategories[categoryType] 返回的就是一级分类列表
    // 每个一级分类的parentId为空或'0'，过滤掉隐藏的即可
    return availableCategories.value.filter(cat => !cat.hidden);
});

// 所有分类的平坦映射（用于查找选中的分类）
const allCategoriesMap = computed<Record<string, TransactionCategory>>(() => {
    const map: Record<string, TransactionCategory> = {};
    const categories = availableCategories.value;

    for (const category of categories) {
        map[category.id] = category;
        if (category.subCategories) {
            for (const subCategory of category.subCategories) {
                map[subCategory.id] = subCategory;
            }
        }
    }

    return map;
});

// 选中的主分类名称
const selectedPrimaryCategoryName = computed<string>(() => {
    if (!budget.value.categoryId) return '';

    return resolveBudgetCategorySelection(availableCategories.value, budget.value.categoryId)?.primaryCategoryName || '';
});

// 选中的子分类名称
const selectedSecondaryCategoryName = computed<string>(() => {
    if (!budget.value.categoryId) return '';

    return resolveBudgetCategorySelection(availableCategories.value, budget.value.categoryId)?.secondaryCategoryName || '';
});

// ============================================================================
// 方法
// ============================================================================

/**
 * 打开对话框
 */
function open({ budget: budgetData, type, usePrimaryCategoryOnly: usePrimaryOnly }: { budget: Budget, type?: BudgetType, usePrimaryCategoryOnly?: boolean }): void {
    // 设置初始化标志，防止watch清空分类
    isInitializing.value = true;

    // 复制预算对象
    budget.value = Object.assign(new Budget(), budgetData);
    originalBudget.value = budgetData;

    // 设置金额（budget.amountCents 已经是分，直接使用，AmountInput 会正确显示为元）
    budgetAmountInCents.value = budget.value.amountCents;

    // 如果是新建且指定了类型
    if (type !== undefined && !budget.value.id) {
        budget.value.type = type;
    }

    // 设置仅使用一级分类标志
    if (usePrimaryOnly !== undefined) {
        usePrimaryCategoryOnly.value = usePrimaryOnly;
    } else {
        // 如果subCategory为空，默认使用一级分类模式
        usePrimaryCategoryOnly.value = !budget.value.subCategory;
    }

    // 转换日期字符串为Unix时间戳
    if (budget.value.startDate) {
        startDateTime.value = new Date(budget.value.startDate).getTime();
    } else {
        // 默认为当月1号
        const now = new Date();
        startDateTime.value = new Date(now.getFullYear(), now.getMonth(), 1).getTime();
    }

    if (budget.value.endDate) {
        endDateTime.value = new Date(budget.value.endDate).getTime();
    } else {
        // 默认为当月最后一天
        const now = new Date();
        endDateTime.value = new Date(now.getFullYear(), now.getMonth() + 1, 0).getTime();
    }

    // 加载分类数据
    categoryStore.loadAllCategories({ force: false }).then(() => {
        // 【修复】根据category和subCategory名称反查categoryId
        if (budget.value.category && !budget.value.categoryId) {
            const foundCategoryId = findCategoryIdByName(budget.value.category, budget.value.subCategory);
            if (foundCategoryId) {
                budget.value.categoryId = foundCategoryId;
                logger.info(`[Budget Edit] 根据名称查找分类ID: ${budget.value.category}/${budget.value.subCategory} -> ${foundCategoryId}`);
            }
        }

        // 注意：不再覆盖usePrimaryCategoryOnly，因为open()已经设置了正确的值

        logger.info(`[Budget Edit] 打开对话框: id=${budget.value.id}, category=${budget.value.category}, subCategory=${budget.value.subCategory}, categoryId=${budget.value.categoryId}, usePrimaryOnly=${usePrimaryCategoryOnly.value}`);

        // 初始化完成，允许watch触发
        isInitializing.value = false;
    });

    showState.value = true;
}

/**
 * 根据分类名称查找分类ID
 */
function findCategoryIdByName(category: string, subCategory?: string): string {
    return findBudgetCategoryIdByNames(availableCategories.value, category, subCategory);
}

/**
 * 关闭对话框
 */
function close(): void {
    showState.value = false;
    budget.value = new Budget();
    originalBudget.value = null;
    budgetAmountInCents.value = 0;
    startDateTime.value = 0;
    endDateTime.value = 0;
}

/**
 * 分类变更处理
 */
function onCategoryChange(categoryId: string | unknown): void {
    const catId = String(categoryId || '');
    logger.info(`[Budget] onCategoryChange called with categoryId: "${catId}"`);
    logger.info(`[Budget] Current budgetType: ${budget.value.type}, categoryTypeValue: ${categoryTypeValue.value}`);
    logger.info(`[Budget] availableCategories count: ${availableCategories.value.length}`);
    logger.info(`[Budget] allCategoriesMap keys count: ${Object.keys(allCategoriesMap.value).length}`);

    if (catId) {
        budget.value.categoryId = catId;
    }

    const selectedCategory = allCategoriesMap.value[catId];
    logger.info(`[Budget] selectedCategory from allCategoriesMap:`, selectedCategory ? { id: selectedCategory.id, name: selectedCategory.name, parentId: selectedCategory.parentId } : 'not found');

    const resolvedSelection = resolveBudgetCategorySelection(availableCategories.value, catId);

    if (resolvedSelection) {
        budget.value.category = resolvedSelection.primaryCategoryName;
        budget.value.subCategory = resolvedSelection.secondaryCategoryName;
        logger.info(`[Budget] Resolved category selection: category=${budget.value.category}, subCategory=${budget.value.subCategory}`);
    } else if (catId) {
        logger.warn(`[Budget] Category not found anywhere for id: "${catId}"`);
        logger.info(`[Budget] Available keys in allCategoriesMap:`, Object.keys(allCategoriesMap.value).slice(0, 10));
        logger.info(`[Budget] availablePrimaryCategories:`, availablePrimaryCategories.value.map(c => ({ id: c.id, name: c.name })).slice(0, 5));
    }
}

/**
 * 保存预算
 */
async function save(): Promise<void> {
    // 确保分类信息被正确填充（必须在验证之前调用）
    if (budget.value.categoryId) {
        onCategoryChange(budget.value.categoryId);
    }

    // AmountInput返回的值已经是分(cents)，直接使用
    budget.value.amountCents = budgetAmountInCents.value;

    // 转换日期
    if (startDateTime.value) {
        budget.value.startDate = new Date(startDateTime.value).toISOString().split('T')[0] || '';
    }
    if (endDateTime.value) {
        budget.value.endDate = new Date(endDateTime.value).toISOString().split('T')[0] || '';
    }

    // 日志记录保存前的数据（用于调试）
    logger.info('[Budget Save] Before validation:', {
        categoryId: budget.value.categoryId,
        category: budget.value.category,
        subCategory: budget.value.subCategory
    });

    // 验证必填字段 - 必须选择分类
    if (!budget.value.category) {
        logger.warn('[Budget Save] Category is required');
        return;
    }

    // 日志记录保存的数据
    logger.info('[Budget Save] Data to save:', {
        id: budget.value.id,
        name: budget.value.name,
        category: budget.value.category,
        subCategory: budget.value.subCategory,
        categoryId: budget.value.categoryId,
        amountCents: budget.value.amountCents,
        periodType: budget.value.periodType,
        startDate: budget.value.startDate,
        endDate: budget.value.endDate
    });

    saving.value = true;

    try {
        await budgetStore.saveBudget({ budget: budget.value });
        emit('budget:saved');
        close();
    } catch (error: unknown) {
        logger.error('Failed to save budget:', error);
    } finally {
        saving.value = false;
    }
}

// ============================================================================
// 监听
// ============================================================================

// 标记是否正在初始化（避免watch触发清空分类）
const isInitializing = ref<boolean>(false);

// 监听预算类型变化，清空分类选择（仅非初始化时）
watch(() => budget.value.type, () => {
    if (!isInitializing.value) {
        budget.value.category = '';
        budget.value.subCategory = '';
        budget.value.categoryId = '';
    }
});

// 监听一级分类开关变化，切换时重置分类选择（仅非初始化时）
watch(usePrimaryCategoryOnly, () => {
    if (!isInitializing.value) {
        budget.value.category = '';
        budget.value.subCategory = '';
        budget.value.categoryId = '';
    }
});

// ============================================================================
// 暴露方法
// ============================================================================

defineExpose({
    open
});
</script>

<style scoped>
.budget-edit-dialog :deep(.v-card) {
    max-height: 90vh;
    overflow-y: auto;
}

/* 预算金额输入框加粗显示（包括货币符号和金额） */
.budget-amount-input :deep(.v-field__input),
.budget-amount-input :deep(.v-field__prepend-inner) {
    font-weight: bold !important;
}
</style>
