<template>
    <v-table class="budget-table table-striped" density="comfortable" :hover="!loading">
        <tbody v-if="loading && filteredBudgets.length === 0">
        <tr :key="itemIdx" v-for="itemIdx in [1, 2, 3, 4, 5]">
            <td class="px-0" colspan="4">
                <v-skeleton-loader type="text" :loading="true"></v-skeleton-loader>
            </td>
        </tr>
        </tbody>

        <tbody v-if="!loading && filteredBudgets.length === 0">
        <tr>
            <td colspan="4">
                <v-empty-state :icon="mdiWalletOutline" :headline="tt('No budgets found')" />
            </td>
        </tr>
        </tbody>

        <tbody v-if="filteredBudgets.length > 0">
        <template v-for="(group, gIdx) in groupedBudgets" :key="group.category">
            <tr class="budget-group-header budget-list-row"
                :class="{ 'budget-group-last-row': gIdx < groupedBudgets.length - 1 && (group.isCollapsed || !groupHasExpandedRows(group)) }"
                @click="$emit('toggleCategoryCollapse', group.category)"
                tabindex="0">
                <td colspan="4" class="pa-0">
                    <div class="budget-item budget-primary d-flex px-4 py-3"
                         :class="{ 'bg-grey-lighten-4': !isDarkMode, 'bg-grey-darken-3': isDarkMode }">
                        <v-icon
                            :icon="group.isCollapsed ? mdiChevronRight : mdiChevronDown"
                            size="20"
                            class="me-2 text-grey flex-shrink-0"
                        />
                        <item-icon
                            class="me-3 flex-shrink-0"
                            icon-type="category"
                            :icon-id="group.categoryIcon"
                            :color="group.categoryColor"
                            :size="36"
                        />
                        <div class="d-flex flex-column flex-grow-1">
                            <div class="d-flex align-center justify-space-between mb-1">
                                <div class="d-flex align-center flex-grow-1">
                                    <span class="budget-category-name text-body-1 font-weight-bold">
                                        {{ group.category }}
                                    </span>
                                    <span class="budget-percent text-body-2 ms-2"
                                          :class="getExecutionRateTextClass(getGroupExecutionRate(group))">
                                        {{ getGroupExecutionRateText(group) }}
                                    </span>
                                </div>
                                <div class="budget-row-actions d-flex align-center">
                                    <v-btn v-if="group.primaryBudgets.length === 0"
                                           density="compact" color="default" variant="text" size="x-small"
                                           :icon="mdiPlusCircleOutline"
                                           :disabled="loading || updating"
                                           @click.stop="$emit('addPrimaryBudget', group)">
                                        <v-icon :icon="mdiPlusCircleOutline" size="16" />
                                        <v-tooltip activator="parent">{{ tt('Add Primary Budget') }}</v-tooltip>
                                    </v-btn>
                                    <v-btn v-if="group.primaryBudgets.length === 1"
                                           density="compact" color="default" variant="text" size="x-small"
                                           :icon="mdiPencilOutline"
                                           :disabled="loading || updating"
                                           @click.stop="$emit('edit', getPrimaryBudgetForHeader(group)!)">
                                        <v-icon :icon="mdiPencilOutline" size="16" />
                                        <v-tooltip activator="parent">{{ tt('Edit') }}</v-tooltip>
                                    </v-btn>
                                    <v-btn v-if="group.primaryBudgets.length === 1"
                                           density="compact" color="default" variant="text" size="x-small"
                                           :icon="mdiDeleteOutline"
                                           :loading="budgetRemoving[getPrimaryBudgetForHeader(group)!.id]"
                                           :disabled="loading || updating"
                                           @click.stop="$emit('remove', getPrimaryBudgetForHeader(group)!)">
                                        <template #loader>
                                            <v-progress-circular indeterminate size="14" width="2"/>
                                        </template>
                                        <v-icon :icon="mdiDeleteOutline" size="16" />
                                        <v-tooltip activator="parent">{{ tt('Delete') }}</v-tooltip>
                                    </v-btn>
                                </div>
                                <div class="budget-amounts d-flex align-center justify-end ms-auto" style="min-width: 150px;">
                                    <span class="budget-spent text-body-2">
                                        {{ formatAmount(group.totalSpentCents / 100) }}
                                    </span>
                                    <span class="budget-separator text-body-2 text-medium-emphasis mx-1">/</span>
                                    <span class="budget-total text-body-2 text-medium-emphasis">
                                        {{ formatAmount(group.totalAmountCents / 100) }}
                                    </span>
                                </div>
                            </div>
                            <div class="budget-progress-container cursor-pointer"
                                 @click.stop="$emit('navigateToTransactions', group.category, getPrimaryBudgetForHeader(group))"
                                 :title="tt('Click to view transactions')">
                                <v-progress-linear
                                    :model-value="Math.min(getGroupExecutionRate(group), 100)"
                                    :color="getGroupProgressColor(group)"
                                    :bg-color="isDarkMode ? '#444444' : '#f0f0f0'"
                                    :bg-opacity="1"
                                    :height="6"
                                    rounded
                                />
                            </div>
                        </div>
                    </div>
                </td>
            </tr>

            <template v-if="!group.isCollapsed">
                <tr v-for="(budget, pIdx) in getExpandedPrimaryBudgets(group)" :key="budget.id"
                    class="budget-list-row budget-sub-row budget-primary-row"
                    :class="{ 'budget-group-last-row': gIdx < groupedBudgets.length - 1 && pIdx === getExpandedPrimaryBudgets(group).length - 1 && group.subBudgets.length === 0 }"
                    @dblclick="$emit('edit', budget)" tabindex="0"
                    @keydown.delete="$emit('remove', budget)" @keydown.enter="$emit('edit', budget)">
                    <td colspan="4" class="pa-0">
                        <div class="budget-item budget-secondary d-flex px-4 py-2"
                             style="padding-left: 56px !important;">
                            <item-icon
                                class="me-3 flex-shrink-0"
                                icon-type="category"
                                :icon-id="getBudgetCategoryIcon(budget, group)"
                                :color="getBudgetCategoryColor(budget, group)"
                                :size="28"
                            />
                            <div class="d-flex flex-column flex-grow-1">
                                <div class="d-flex align-center justify-space-between mb-1">
                                    <div class="d-flex align-center flex-grow-1">
                                        <span class="budget-category-name text-body-2 font-weight-medium">
                                            {{ budget.name || group.category }}
                                        </span>
                                        <span class="budget-percent text-body-2 ms-2"
                                              :class="getExecutionRateTextClass(budget.executionRate)">
                                            {{ budget.executionRateText }}
                                        </span>
                                        <v-icon v-if="budget.alertTriggered && !budget.isOverBudget"
                                                :icon="mdiAlertCircle" color="warning" class="ms-1" size="14" />
                                        <v-icon v-if="budget.isOverBudget"
                                                :icon="mdiAlertOctagon" color="error" class="ms-1" size="14" />
                                    </div>
                                    <div class="budget-row-actions d-flex align-center">
                                        <v-btn density="compact" color="default" variant="text" size="x-small"
                                               :icon="mdiPencilOutline"
                                               :disabled="loading || updating"
                                               @click.stop="$emit('edit', budget)">
                                            <v-icon :icon="mdiPencilOutline" size="16" />
                                            <v-tooltip activator="parent">{{ tt('Edit') }}</v-tooltip>
                                        </v-btn>
                                        <v-btn density="compact" color="default" variant="text" size="x-small"
                                               :icon="mdiDeleteOutline"
                                               :loading="budgetRemoving[budget.id]"
                                               :disabled="loading || updating"
                                               @click.stop="$emit('remove', budget)">
                                            <template #loader>
                                                <v-progress-circular indeterminate size="14" width="2"/>
                                            </template>
                                            <v-icon :icon="mdiDeleteOutline" size="16" />
                                            <v-tooltip activator="parent">{{ tt('Delete') }}</v-tooltip>
                                        </v-btn>
                                    </div>
                                    <div class="budget-amounts d-flex align-center justify-end ms-auto" style="min-width: 150px;">
                                        <span class="budget-spent text-body-2"
                                              :class="{ 'text-error font-weight-bold': budget.isOverBudget }">
                                            {{ formatAmount(budget.spentAmountInYuan) }}
                                        </span>
                                        <span class="budget-separator text-body-2 text-medium-emphasis mx-1">/</span>
                                        <span class="budget-total text-body-2 text-medium-emphasis">
                                            {{ formatAmount(budget.amountInYuan) }}
                                        </span>
                                    </div>
                                </div>
                                <div class="budget-progress-container cursor-pointer"
                                     @click.stop="$emit('navigateToTransactions', budget.category, budget)"
                                     :title="tt('Click to view transactions')">
                                    <v-progress-linear
                                        :model-value="Math.min(budget.executionRate, 100)"
                                        :color="getBudgetProgressColor(budget)"
                                        :bg-color="isDarkMode ? '#444444' : '#f0f0f0'"
                                        :bg-opacity="1"
                                        :height="6"
                                        rounded
                                    />
                                </div>
                            </div>
                        </div>
                    </td>
                </tr>
            </template>
        </template>
        </tbody>
    </v-table>
</template>

<script setup lang="ts">
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import { useI18n } from '@/locales/helpers.ts';
import type { Budget } from '@/models/budget.ts';
import type { BudgetGroup } from '../budgetPageTypes.ts';
import {
    mdiAlertCircle,
    mdiAlertOctagon,
    mdiChevronDown,
    mdiChevronRight,
    mdiDeleteOutline,
    mdiPencilOutline,
    mdiPlusCircleOutline,
    mdiWalletOutline
} from '@mdi/js';

defineProps<{
    loading: boolean;
    updating: boolean;
    isDarkMode: boolean;
    filteredBudgets: Budget[];
    groupedBudgets: BudgetGroup[];
    budgetRemoving: Record<string, boolean>;
    groupHasExpandedRows: (group: BudgetGroup) => boolean;
    getPrimaryBudgetForHeader: (group: BudgetGroup) => Budget | null;
    getExpandedPrimaryBudgets: (group: BudgetGroup) => Budget[];
    getBudgetCategoryIcon: (budget: Budget, group: BudgetGroup) => string;
    getBudgetCategoryColor: (budget: Budget, group: BudgetGroup) => string;
    getGroupExecutionRate: (group: BudgetGroup) => number;
    getGroupExecutionRateText: (group: BudgetGroup) => string;
    getExecutionRateTextClass: (rate: number) => string;
    getGroupProgressColor: (group: BudgetGroup) => string;
    getBudgetProgressColor: (budget: Budget) => string;
    formatAmount: (amount: number) => string;
}>();

defineEmits<{
    (e: 'toggleCategoryCollapse', category: string): void;
    (e: 'addPrimaryBudget', group: BudgetGroup): void;
    (e: 'edit', budget: Budget): void;
    (e: 'remove', budget: Budget): void;
    (e: 'navigateToTransactions', category: string, budget: Budget | null): void;
}>();

const { tt } = useI18n();
</script>

<style scoped>
.cursor-pointer {
    cursor: pointer;
}

.budget-list-row {
    cursor: pointer;
    transition: background-color 0.2s ease;
}

.budget-list-row:hover {
    background-color: rgba(var(--v-theme-primary), 0.04);
}

.budget-list-row:focus {
    outline: none;
    background-color: rgba(var(--v-theme-primary), 0.08);
}

.budget-item {
    width: 100%;
}

.budget-category-name {
    color: rgba(var(--v-theme-on-surface), 0.87);
}

.budget-percent {
    font-size: 0.875rem;
}

.budget-amounts {
    flex-shrink: 0;
    font-family: 'Roboto Mono', 'Consolas', monospace;
    text-align: right;
}

.budget-spent {
    color: rgba(var(--v-theme-on-surface), 0.87);
}

.budget-progress-container {
    width: 100%;
}

.budget-progress-container :deep(.v-progress-linear) {
    transition: all 0.3s ease;
}

.budget-list-row:hover .budget-row-actions,
.budget-list-row:focus .budget-row-actions {
    opacity: 1;
}

.budget-row-actions {
    opacity: 0;
    transition: opacity 0.2s ease;
}

.budget-table {
    background-color: rgb(var(--v-theme-surface)) !important;
}

.budget-table:deep(tbody tr) {
    background-color: transparent !important;
}

.budget-table:deep(tbody tr td) {
    border-bottom: none !important;
}

.budget-primary,
.budget-secondary {
    background-color: rgb(var(--v-theme-surface)) !important;
}

.budget-group-last-row td {
    position: relative;
}

.budget-group-last-row td::after {
    content: '';
    position: absolute;
    left: 32px;
    right: 32px;
    bottom: 0;
    height: 1px;
    background-color: rgba(var(--v-theme-on-surface), 0.15);
}

.text-orange {
    color: rgb(226, 182, 10) !important;
}
</style>
