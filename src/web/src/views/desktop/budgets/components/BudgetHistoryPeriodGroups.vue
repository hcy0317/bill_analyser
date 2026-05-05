<template>
    <div v-if="historicalBudgetGroups.length > 0 || selectedHistoricalPeriodKey" class="mt-6">
        <div class="d-flex align-center justify-space-between flex-wrap ga-3 mb-3">
            <div class="text-subtitle-1 font-weight-medium">{{ tt('Historical Budgets') }}</div>
            <div class="d-flex align-center ga-2">
                <v-chip v-if="selectedHistoricalPeriodKey"
                        size="small"
                        color="primary"
                        variant="outlined"
                        closable
                        @click:close="$emit('clearSelectedPeriod')">
                    {{ selectedHistoricalPeriodKey }}
                </v-chip>
                <v-btn density="compact" color="default" variant="text" @click="$emit('toggleAllHistoricalGroups')">
                    {{ areAllHistoricalGroupsExpanded ? tt('Collapse All') : tt('Expand All') }}
                </v-btn>
            </div>
        </div>

        <v-expansion-panels v-model="expandedGroupKeysModel" multiple variant="accordion">
            <v-expansion-panel
                v-for="group in visibleHistoricalBudgetGroups"
                :key="group.key"
                :value="group.key"
                class="budget-history-period-panel"
            >
                <v-expansion-panel-title>
                    <div class="d-flex align-center flex-wrap ga-4 w-100">
                        <span class="text-subtitle-2 font-weight-medium">{{ group.label }}</span>
                        <span class="text-caption text-medium-emphasis">{{ group.startDate }} - {{ group.endDate }}</span>
                        <v-btn density="compact" color="default" variant="text" size="x-small"
                               class="budget-history-focus-btn"
                               :disabled="historicalBudgetGroups.length <= 1"
                               @click.stop="$emit('toggleHistoricalPeriodFocus', group.key)">
                            {{ selectedHistoricalPeriodKey === group.key ? tt('Clear Selection') : tt('Filter by Period') }}
                        </v-btn>
                        <span class="ms-auto text-caption text-medium-emphasis">{{ tt('Items') }}: {{ group.itemCount }}</span>
                        <span class="text-caption text-medium-emphasis">{{ tt('Budget') }}: {{ formatAmount(group.totalBudget / 100) }}</span>
                        <span class="text-caption text-medium-emphasis">{{ tt('Spent') }}: {{ formatAmount(group.totalSpent / 100) }}</span>
                        <span class="text-caption font-weight-medium" :class="getExecutionRateColorClass(group.totalExecutionRate)">
                            {{ group.totalExecutionRate.toFixed(1) }}%
                        </span>
                    </div>
                </v-expansion-panel-title>
                <v-expansion-panel-text>
                    <v-table density="comfortable" class="budget-table budget-history-period-table" :hover="false">
                        <tbody>
                        <template v-for="row in group.rows" :key="row.key">
                            <tr class="budget-list-row budget-history-row">
                                <td class="pa-0">
                                    <div class="budget-item budget-primary d-flex px-4 py-3"
                                         :class="{ 'bg-grey-lighten-4': !isDarkMode, 'bg-grey-darken-3': isDarkMode }">
                                        <button
                                            type="button"
                                            class="budget-history-row-icon-button me-3"
                                            :aria-label="tt('Filter Categories')"
                                            @click.stop="$emit('toggleHistoricalPrimaryLegend', row.primaryCategory)"
                                        >
                                            <item-icon
                                                v-if="row.primaryIcon"
                                                icon-type="category"
                                                :icon-id="row.primaryIcon"
                                                :color="row.primaryIconColor"
                                                :size="36"
                                            />
                                            <span v-else class="budget-history-row-swatch" :style="{ backgroundColor: row.primaryColor }"></span>
                                            <v-tooltip activator="parent">{{ tt('Filter Categories') }}</v-tooltip>
                                        </button>
                                        <div class="d-flex flex-column flex-grow-1">
                                            <div class="d-flex align-center justify-space-between mb-1">
                                                <div class="d-flex align-center flex-grow-1">
                                                    <v-btn v-if="row.childRows.length > 0"
                                                           density="compact"
                                                           color="default"
                                                           variant="text"
                                                           size="24"
                                                           class="budget-history-primary-toggle me-1"
                                                           :icon="true"
                                                           @click.stop="$emit('toggleHistoricalPrimaryCollapse', group.key, row)">
                                                        <v-icon :icon="isHistoricalPrimaryCollapsed(group.key, row) ? mdiChevronRight : mdiChevronDown" size="18" />
                                                        <v-tooltip activator="parent">
                                                            {{ isHistoricalPrimaryCollapsed(group.key, row) ? tt('Expand All') : tt('Collapse All') }}
                                                        </v-tooltip>
                                                    </v-btn>
                                                    <span class="budget-category-name text-body-1 font-weight-bold">
                                                        {{ row.displayCategory }}
                                                    </span>
                                                    <span class="budget-percent text-body-2 ms-2"
                                                          :class="getExecutionRateTextClass(row.executionRate)">
                                                        {{ row.executionRate.toFixed(1) }}%
                                                    </span>
                                                </div>
                                                <div class="budget-amounts d-flex align-center justify-end ms-auto" style="min-width: 150px;">
                                                    <span class="budget-spent text-body-2">
                                                        {{ formatAmount(row.spentAmount / 100) }}
                                                    </span>
                                                    <span class="budget-separator text-body-2 text-medium-emphasis mx-1">/</span>
                                                    <span class="budget-total text-body-2 text-medium-emphasis">
                                                        {{ formatAmount(row.budgetAmount / 100) }}
                                                    </span>
                                                </div>
                                            </div>
                                            <div class="budget-progress-container">
                                                <v-progress-linear
                                                    :model-value="Math.min(row.executionRate, 100)"
                                                    :color="getProgressColorByRate(row.executionRate)"
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
                            <tr v-for="child in (isHistoricalPrimaryCollapsed(group.key, row) ? [] : row.childRows)" :key="child.key"
                                class="budget-list-row budget-sub-row budget-history-sub-row">
                                <td class="pa-0">
                                    <div class="budget-item budget-secondary d-flex px-4 py-2"
                                         style="padding-left: 56px !important;">
                                        <button
                                            type="button"
                                            class="budget-history-row-icon-button budget-history-row-icon-button--secondary me-3"
                                            :aria-label="tt('Filter Categories')"
                                            @click.stop="$emit('toggleHistoricalSecondaryLegend', `${child.primaryCategory}::${child.secondaryCategory}`)"
                                        >
                                            <item-icon
                                                v-if="child.secondaryIcon"
                                                icon-type="category"
                                                :icon-id="child.secondaryIcon"
                                                :color="child.secondaryIconColor"
                                                :size="28"
                                            />
                                            <span v-else class="budget-history-row-swatch budget-history-row-swatch--secondary" :style="{ backgroundColor: child.color }"></span>
                                            <v-tooltip activator="parent">{{ tt('Filter Categories') }}</v-tooltip>
                                        </button>
                                        <div class="d-flex flex-column flex-grow-1">
                                            <div class="d-flex align-center justify-space-between mb-1">
                                                <div class="d-flex align-center flex-grow-1">
                                                    <span class="budget-category-name text-body-2 font-weight-medium">
                                                        {{ child.displayCategory }}
                                                    </span>
                                                    <span class="budget-percent text-body-2 ms-2"
                                                          :class="getExecutionRateTextClass(child.executionRate)">
                                                        {{ child.executionRate.toFixed(1) }}%
                                                    </span>
                                                </div>
                                                <div class="budget-amounts d-flex align-center justify-end ms-auto" style="min-width: 150px;">
                                                    <span class="budget-spent text-body-2">
                                                        {{ formatAmount(child.spentAmount / 100) }}
                                                    </span>
                                                    <span class="budget-separator text-body-2 text-medium-emphasis mx-1">/</span>
                                                    <span class="budget-total text-body-2 text-medium-emphasis">
                                                        {{ formatAmount(child.budgetAmount / 100) }}
                                                    </span>
                                                </div>
                                            </div>
                                            <div class="budget-progress-container">
                                                <v-progress-linear
                                                    :model-value="Math.min(child.executionRate, 100)"
                                                    :color="getProgressColorByRate(child.executionRate)"
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
                        </tbody>
                    </v-table>
                </v-expansion-panel-text>
            </v-expansion-panel>
        </v-expansion-panels>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import { useI18n } from '@/locales/helpers.ts';
import type {
    HistoricalBudgetCategoryRow,
    HistoricalBudgetPeriodGroup
} from '../historyGrouping.ts';
import { mdiChevronDown, mdiChevronRight } from '@mdi/js';

const props = defineProps<{
    historicalBudgetGroups: HistoricalBudgetPeriodGroup[];
    visibleHistoricalBudgetGroups: HistoricalBudgetPeriodGroup[];
    selectedHistoricalPeriodKey: string | null;
    areAllHistoricalGroupsExpanded: boolean;
    historicalExpandedGroupKeys: string[];
    isDarkMode: boolean;
    formatAmount: (amount: number) => string;
    getExecutionRateColorClass: (rate: number) => string;
    getExecutionRateTextClass: (rate: number) => string;
    getProgressColorByRate: (rate: number) => string;
    isHistoricalPrimaryCollapsed: (groupKey: string, row: HistoricalBudgetCategoryRow) => boolean;
}>();

const emit = defineEmits<{
    (e: 'clearSelectedPeriod'): void;
    (e: 'toggleAllHistoricalGroups'): void;
    (e: 'toggleHistoricalPeriodFocus', groupKey: string): void;
    (e: 'toggleHistoricalPrimaryLegend', primaryKey: string): void;
    (e: 'toggleHistoricalSecondaryLegend', secondaryKey: string): void;
    (e: 'toggleHistoricalPrimaryCollapse', groupKey: string, row: HistoricalBudgetCategoryRow): void;
    (e: 'update:historicalExpandedGroupKeys', keys: string[]): void;
}>();

const expandedGroupKeysModel = computed({
    get: () => props.historicalExpandedGroupKeys,
    set: (value: string[]) => emit('update:historicalExpandedGroupKeys', value)
});

const { tt } = useI18n();
</script>

<style scoped>
.budget-list-row {
    cursor: pointer;
    transition: background-color 0.2s ease;
}

.budget-list-row:hover {
    background-color: rgba(var(--v-theme-primary), 0.04);
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

.budget-table {
    background-color: rgb(var(--v-theme-surface)) !important;
}

.budget-table:deep(tbody tr),
.budget-table:deep(tbody tr td) {
    background-color: transparent !important;
    border-bottom: none !important;
}

.budget-primary,
.budget-secondary {
    background-color: rgb(var(--v-theme-surface)) !important;
}

.budget-history-period-panel {
    background-color: rgba(var(--v-theme-surface), 0.78);
}

.budget-history-period-table :deep(th),
.budget-history-period-table :deep(td) {
    white-space: nowrap;
}

.budget-history-row-icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    border: 0;
    border-radius: 50%;
    background-color: transparent;
    cursor: pointer;
    padding: 0;
}

.budget-history-row-icon-button--secondary {
    width: 28px;
    height: 28px;
}

.budget-history-row-icon-button:hover {
    background-color: rgba(var(--v-theme-primary), 0.08);
}

.budget-history-row-swatch {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background-color: currentColor;
}

.budget-history-row-swatch--secondary {
    width: 18px;
    height: 18px;
}

.budget-history-primary-toggle {
    flex-shrink: 0;
}

.text-orange {
    color: rgb(226, 182, 10) !important;
}
</style>
