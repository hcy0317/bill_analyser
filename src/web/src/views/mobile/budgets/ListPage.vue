<template>
    <f7-page ptr data-testid="mobile.budgets.page" @ptr:refresh="reload" @page:afterin="onPageAfterIn">
        <f7-navbar>
            <f7-nav-left :back-link="tt('Back')"></f7-nav-left>
            <f7-nav-title :title="tt('Budget Management')"></f7-nav-title>
            <f7-nav-right class="navbar-compact-icons">
                <f7-link data-testid="mobile.budgets.action.add" icon-f7="plus" @click="openCreateSheet"></f7-link>
            </f7-nav-right>
        </f7-navbar>

        <f7-toolbar tabbar top class="budget-type-toolbar">
            <f7-segmented strong>
                <f7-button
                    :text="tt('Expense')"
                    :active="activeBudgetType === BudgetType.Expense"
                    @click="switchBudgetType(BudgetType.Expense)"
                ></f7-button>
                <f7-button
                    :text="tt('Investment')"
                    :active="activeBudgetType === BudgetType.Investment"
                    @click="switchBudgetType(BudgetType.Investment)"
                ></f7-button>
            </f7-segmented>
        </f7-toolbar>

        <f7-list strong inset dividers class="margin-vertical skeleton-text" v-if="loading">
            <f7-list-item title="Budget Name">
                <template #text>
                    <div class="budget-progress-row">
                        <f7-progressbar :progress="50"></f7-progressbar>
                    </div>
                    <div class="budget-amount-row">0.00 / 0.00</div>
                </template>
            </f7-list-item>
        </f7-list>

        <f7-list strong inset dividers class="margin-vertical" v-else-if="!loading && filteredBudgets.length === 0">
            <f7-list-item :title="tt('No data')"></f7-list-item>
        </f7-list>

        <f7-list strong inset dividers media-list class="margin-vertical budget-list" v-else>
            <template :key="group.category" v-for="group in groupedBudgets">
                <f7-list-item
                    swipeout
                    class="budget-list-item budget-group-list-item"
                    @click="toggleBudgetGroup(group.category)"
                >
                    <template #media>
                        <div class="budget-group-media">
                            <f7-icon class="budget-collapse-icon" :f7="group.isCollapsed ? 'chevron_right' : 'chevron_down'"></f7-icon>
                            <item-icon
                                class="budget-category-icon"
                                icon-type="category"
                                :icon-id="group.categoryIcon"
                                :color="group.categoryColor"
                                size="28px"
                            ></item-icon>
                        </div>
                    </template>
                    <template #title>
                        <span class="budget-name">{{ group.category }}</span>
                    </template>
                    <template #after>
                        <span class="budget-execution-rate" :class="getBudgetGroupRateClass(group)">
                            {{ getBudgetGroupExecutionRate(group).toFixed(1) }}%
                        </span>
                    </template>
                    <template #text>
                        <div class="budget-progress-row">
                            <f7-progressbar
                                :progress="getBudgetGroupProgressPercent(group)"
                                :class="getBudgetGroupProgressClass(group)"
                            ></f7-progressbar>
                        </div>
                        <div class="budget-amount-row">
                            <span class="budget-amount-spent">{{ formatAmount(group.totalSpent / 100) }}</span>
                            <span class="budget-amount-divider"> / </span>
                            <span class="budget-amount-target">{{ formatAmount(group.totalAmount / 100) }}</span>
                        </div>
                    </template>
                    <f7-swipeout-actions right v-if="getPrimaryBudgetForHeader(group)">
                        <f7-swipeout-button color="orange" close :text="tt('Edit')" @click="openEditSheet(getPrimaryBudgetForHeader(group)!)"></f7-swipeout-button>
                        <f7-swipeout-button color="red" class="padding-horizontal" @click="requestDelete(getPrimaryBudgetForHeader(group)!)">
                            <f7-icon f7="trash"></f7-icon>
                        </f7-swipeout-button>
                    </f7-swipeout-actions>
                </f7-list-item>

                <template v-if="!group.isCollapsed">
                    <f7-list-item
                        swipeout
                        class="budget-list-item budget-sub-list-item"
                        :key="budget.id"
                        v-for="budget in getExpandedBudgetRows(group)"
                        @click="onBudgetTap(budget)"
                    >
                        <template #media>
                            <item-icon
                                class="budget-category-icon"
                                icon-type="category"
                                :icon-id="budget.categoryIcon || group.categoryIcon"
                                :color="budget.categoryColor || group.categoryColor"
                                size="24px"
                            ></item-icon>
                        </template>
                        <template #title>
                            <span class="budget-name">{{ budget.subCategory || budget.name || budget.fullCategoryName }}</span>
                        </template>
                        <template #after>
                            <span class="budget-execution-rate" :class="{ 'text-color-red': budget.isOverBudget, 'text-color-orange': !budget.isOverBudget && budget.alertTriggered }">
                                {{ budget.executionRateText }}
                            </span>
                        </template>
                        <template #text>
                            <div class="budget-progress-row">
                                <f7-progressbar
                                    :progress="getBudgetProgressPercent(budget)"
                                    :class="{ 'color-red': budget.isOverBudget, 'color-orange': !budget.isOverBudget && budget.alertTriggered }"
                                ></f7-progressbar>
                            </div>
                            <div class="budget-amount-row">
                                <span class="budget-amount-spent">{{ formatAmount(budget.spentAmountInYuan) }}</span>
                                <span class="budget-amount-divider"> / </span>
                                <span class="budget-amount-target">{{ formatAmount(budget.amountInYuan) }}</span>
                            </div>
                        </template>
                        <f7-swipeout-actions right>
                            <f7-swipeout-button color="orange" close :text="tt('Edit')" @click="openEditSheet(budget)"></f7-swipeout-button>
                            <f7-swipeout-button color="red" class="padding-horizontal" @click="requestDelete(budget)">
                                <f7-icon f7="trash"></f7-icon>
                            </f7-swipeout-button>
                        </f7-swipeout-actions>
                    </f7-list-item>
                </template>
            </template>
        </f7-list>

        <budget-edit-sheet
            ref="editSheetRef"
            v-model:show="showEditSheet"
            :budget="editingBudget"
            :default-type="activeBudgetType"
            @save="onSheetSave"
            @delete:request="requestDelete"
        ></budget-edit-sheet>

        <f7-actions
            close-by-outside-click
            close-on-escape
            :opened="showDeleteActionSheet"
            @actions:closed="showDeleteActionSheet = false"
        >
            <f7-actions-group>
                <f7-actions-label>{{ deleteConfirmLabel }}</f7-actions-label>
                <f7-actions-button color="red" @click="confirmDelete">{{ tt('Delete') }}</f7-actions-button>
            </f7-actions-group>
            <f7-actions-group>
                <f7-actions-button bold close>{{ tt('Cancel') }}</f7-actions-button>
            </f7-actions-group>
        </f7-actions>
    </f7-page>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { useI18nUIComponents, showLoading, hideLoading } from '@/lib/ui/mobile.ts';
import { useBudgetStore } from '@/stores/budget.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { CategoryType } from '@/core/category.ts';
import { Budget, BudgetType, BudgetPeriodType } from '@/models/budget.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import {
    type MobileBudgetGroup,
    buildMobileBudgetGroups,
    formatBudgetAmount,
    getBudgetGroupExecutionRate,
    getBudgetGroupProgressPercent,
    getBudgetProgressPercent,
    selectBudgetsByType
} from '@/views/mobile/budgets/listPageHelpers.ts';
import BudgetEditSheet from '@/views/mobile/budgets/EditSheet.vue';
import logger from '@/lib/logger.ts';

const { tt } = useI18n();
const { showToast } = useI18nUIComponents();
const budgetStore = useBudgetStore();
const transactionCategoriesStore = useTransactionCategoriesStore();

const loading = ref<boolean>(true);
const activeBudgetType = ref<BudgetType>(BudgetType.Expense);
const collapsedBudgetGroups = ref<Set<string>>(new Set());

// Edit sheet state. `editingBudget` is null when creating; otherwise a clone of
// the selected list row that the EditSheet operates on without mutating the
// store-owned instance.
const showEditSheet = ref<boolean>(false);
const editingBudget = ref<Budget | null>(null);
const editSheetRef = ref<{ setSaving(v: boolean): void } | null>(null);

// Delete confirmation state.
const showDeleteActionSheet = ref<boolean>(false);
const budgetToDelete = ref<Budget | null>(null);

const filteredBudgets = computed<Budget[]>(() => {
    return selectBudgetsByType(
        activeBudgetType.value,
        budgetStore.expenseBudgets,
        budgetStore.investmentBudgets
    );
});

const currentCategoryType = computed<CategoryType>(() => {
    return activeBudgetType.value === BudgetType.Investment
        ? CategoryType.Investment
        : CategoryType.Expense;
});

const budgetPrimaryCategories = computed<TransactionCategory[]>(() => {
    return transactionCategoriesStore.allTransactionCategories[currentCategoryType.value] || [];
});

const groupedBudgets = computed<MobileBudgetGroup[]>(() => buildMobileBudgetGroups({
    budgets: filteredBudgets.value,
    primaryCategories: budgetPrimaryCategories.value,
    collapsedCategories: collapsedBudgetGroups.value
}));

const deleteConfirmLabel = computed<string>(() => {
    const target = budgetToDelete.value;
    if (!target) {
        return tt('Are you sure you want to delete this budget?');
    }
    const name = target.name || target.fullCategoryName || '';
    return tt('Are you sure you want to delete this budget "{name}"?', { name });
});

function formatAmount(amount: number): string {
    return formatBudgetAmount(amount);
}

function switchBudgetType(type: BudgetType): void {
    if (activeBudgetType.value === type) {
        return;
    }
    activeBudgetType.value = type;
    collapsedBudgetGroups.value = new Set();
    void reload(false);
}

// Drilldown remains deferred: hooking the row tap to /transaction/list with a
// computed categoryIds + Custom date range requires reusing
// buildBudgetDrilldownRouteQuery from views/desktop/budgets/categorySelection.ts,
// which lives outside this slice's owned_paths. S5 keeps this a no-op and
// records the decision for S6/S9 to lift the helper into a shared budgets/
// path or duplicate it under views/mobile/budgets.
function onBudgetTap(budget: Budget): void {
    logger.info(`[MobileBudgets] Tap budget (drilldown still deferred): ${budget.id}`);
}

function toggleBudgetGroup(category: string): void {
    if (collapsedBudgetGroups.value.has(category)) {
        collapsedBudgetGroups.value.delete(category);
    } else {
        collapsedBudgetGroups.value.add(category);
    }
    collapsedBudgetGroups.value = new Set(collapsedBudgetGroups.value);
}

function getPrimaryBudgetForHeader(group: MobileBudgetGroup): Budget | null {
    return group.primaryBudgets[0] || null;
}

function getExpandedBudgetRows(group: MobileBudgetGroup): Budget[] {
    const rows = group.primaryBudgets.length > 1 ? [...group.primaryBudgets] : [];
    rows.push(...group.subBudgets);
    return rows;
}

function getBudgetGroupRateClass(group: MobileBudgetGroup): Record<string, boolean> {
    const rate = getBudgetGroupExecutionRate(group);
    return {
        'text-color-red': rate > 100,
        'text-color-orange': rate >= 80 && rate <= 100
    };
}

function getBudgetGroupProgressClass(group: MobileBudgetGroup): Record<string, boolean> {
    const rate = getBudgetGroupExecutionRate(group);
    return {
        'color-red': rate > 100,
        'color-orange': rate >= 80 && rate <= 100
    };
}

function openCreateSheet(): void {
    editingBudget.value = null;
    showEditSheet.value = true;
}

function openEditSheet(budget: Budget): void {
    // Pass a shallow clone so the sheet's local form doesn't bind to the
    // store-owned instance (saveBudget will replace it on success).
    editingBudget.value = Object.assign(new Budget(), budget);
    showEditSheet.value = true;
}

function onSheetSave(updated: Budget): void {
    showLoading();
    budgetStore.saveBudget({ budget: updated }).then(() => {
        hideLoading();
        editSheetRef.value?.setSaving(false);
        showEditSheet.value = false;
        editingBudget.value = null;
        showToast('Budget has been saved');
        void reload(false);
    }).catch((error: unknown) => {
        hideLoading();
        editSheetRef.value?.setSaving(false);
        const message = (error as { message?: string })?.message;
        showToast(message || 'Failed to save budget');
        logger.error('[MobileBudgets] Failed to save budget', error);
    });
}

function requestDelete(budget: Budget): void {
    if (!budget || !budget.id) {
        return;
    }
    budgetToDelete.value = budget;
    showDeleteActionSheet.value = true;
}

function confirmDelete(): void {
    const target = budgetToDelete.value;
    if (!target) {
        return;
    }
    showDeleteActionSheet.value = false;
    showLoading();
    budgetStore.deleteBudget({ budgetId: target.id }).then(() => {
        hideLoading();
        budgetToDelete.value = null;
        // If the user was editing this budget, close the sheet.
        if (editingBudget.value && editingBudget.value.id === target.id) {
            showEditSheet.value = false;
            editingBudget.value = null;
        }
        showToast('Budget has been deleted');
        void reload(false);
    }).catch((error: unknown) => {
        hideLoading();
        const message = (error as { message?: string })?.message;
        showToast(message || 'Failed to delete budget');
        logger.error('[MobileBudgets] Failed to delete budget', error);
    });
}

function getCurrentPeriodRequest(): {
    type: BudgetType,
    periodType: BudgetPeriodType,
    year: number,
    month: number
} {
    const now = new Date();
    return {
        type: activeBudgetType.value,
        periodType: BudgetPeriodType.Monthly,
        year: now.getFullYear(),
        month: now.getMonth() + 1
    };
}

function reload(done: unknown): Promise<void> {
    loading.value = true;
    const req = getCurrentPeriodRequest();
    return Promise.all([
        transactionCategoriesStore.loadAllCategories({ force: false }),
        budgetStore.loadAllBudgets({
        force: true,
        type: req.type,
        periodType: req.periodType
        })
    ]).then(() => {
        return budgetStore.loadBudgetExecution({
            type: req.type,
            periodType: req.periodType,
            year: req.year,
            month: req.month
        });
    }).then(() => {
        loading.value = false;
    }).catch(error => {
        loading.value = false;
        logger.error('[MobileBudgets] Failed to load budgets', error);
    }).finally(() => {
        if (typeof done === 'function') {
            (done as () => void)();
        }
    });
}

function onPageAfterIn(): void {
    void reload(false);
}
</script>

<style>
.budget-type-toolbar {
    --f7-toolbar-bg-color: var(--f7-page-bg-color);
}

.budget-list .budget-list-item .item-title-row {
    align-items: center;
}

.budget-list .budget-group-list-item {
    --f7-list-item-title-font-weight: 600;
}

.budget-list .budget-sub-list-item {
    --f7-list-item-padding-left: 48px;
}

.budget-list .budget-group-media {
    display: flex;
    align-items: center;
    min-width: 52px;
}

.budget-list .budget-collapse-icon {
    color: var(--f7-text-color);
    opacity: 0.45;
    margin-inline-end: 5px;
}

.budget-list .budget-category-icon {
    flex-shrink: 0;
}

.budget-list .budget-progress-row {
    margin-top: 6px;
}

.budget-list .budget-amount-row {
    margin-top: 4px;
    font-size: 13px;
    color: var(--f7-text-color);
    opacity: 0.8;
}

.budget-list .budget-amount-divider {
    opacity: 0.5;
}

.budget-list .budget-execution-rate {
    font-variant-numeric: tabular-nums;
}
</style>
