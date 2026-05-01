<template>
    <f7-page ptr @ptr:refresh="reload" @page:afterin="onPageAfterIn">
        <f7-navbar>
            <f7-nav-left :back-link="tt('Back')"></f7-nav-left>
            <f7-nav-title :title="tt('Budget Management')"></f7-nav-title>
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
            <f7-list-item
                class="budget-list-item"
                :key="budget.id"
                v-for="budget in filteredBudgets"
                @click="onBudgetTap(budget)"
            >
                <template #title>
                    <span class="budget-name">{{ budget.name || budget.fullCategoryName }}</span>
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
            </f7-list-item>
        </f7-list>
    </f7-page>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { useBudgetStore } from '@/stores/budget.ts';
import { Budget, BudgetType, BudgetPeriodType } from '@/models/budget.ts';
import {
    formatBudgetAmount,
    getBudgetProgressPercent,
    selectBudgetsByType
} from '@/views/mobile/budgets/listPageHelpers.ts';
import logger from '@/lib/logger.ts';

const { tt } = useI18n();
const budgetStore = useBudgetStore();

const loading = ref<boolean>(true);
const activeBudgetType = ref<BudgetType>(BudgetType.Expense);

const filteredBudgets = computed<Budget[]>(() => {
    return selectBudgetsByType(
        activeBudgetType.value,
        budgetStore.expenseBudgets,
        budgetStore.investmentBudgets
    );
});

function formatAmount(amount: number): string {
    return formatBudgetAmount(amount);
}

function switchBudgetType(type: BudgetType): void {
    if (activeBudgetType.value === type) {
        return;
    }
    activeBudgetType.value = type;
    void reload(false);
}

// Drilldown deferred to S5: the desktop /transaction/list?... query depends on
// buildBudgetDrilldownRouteQuery (categories context, fiscal-year scope, account/tag
// filters), which the mobile budgets MVP does not yet wire. Tap is a no-op for now.
function onBudgetTap(budget: Budget): void {
    logger.info(`[MobileBudgets] Tap budget (drilldown deferred to S5): ${budget.id}`);
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
    return budgetStore.loadAllBudgets({
        force: true,
        type: req.type,
        periodType: req.periodType
    }).then(() => {
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
