<template>
    <v-card-text class="pt-4">
        <div v-if="loading && (!isHistoricalHistoryReady || historicalLegendGroups.length === 0)" class="py-10 d-flex flex-column align-center justify-center budget-history-loading-placeholder">
            <v-progress-linear indeterminate color="primary" class="budget-history-loading-bar mb-2" />
            <span class="text-caption text-medium-emphasis">{{ tt('Loading') }}...</span>
        </div>

        <div v-else-if="canShowHistoricalBudgetPanel" class="budget-history-panel">
            <div class="budget-history-chart-shell">
                <v-chart
                    v-if="historicalChartModel.primaryBands.length > 0"
                    :key="historicalChartRenderKey"
                    ref="historicalChartRef"
                    autoresize
                    class="budget-history-chart"
                    :option="historicalChartOptions"
                    :update-options="historicalChartUpdateOptions"
                />

                <div v-else class="d-flex align-center justify-center budget-history-chart budget-history-empty-state">
                    <span class="text-medium-emphasis">{{ tt('All categories hidden') }}</span>
                </div>
            </div>

            <budget-history-legend
                :legend-groups="historicalLegendGroups"
                :historical-budget-level="historicalBudgetLevel"
                @toggle-primary="$emit('toggleHistoricalPrimaryLegend', $event)"
                @toggle-secondary="$emit('toggleHistoricalSecondaryLegend', $event)"
            />

            <budget-history-period-groups
                :historical-budget-groups="historicalBudgetGroups"
                :visible-historical-budget-groups="visibleHistoricalBudgetGroups"
                :selected-historical-period-key="selectedHistoricalPeriodKey"
                :are-all-historical-groups-expanded="areAllHistoricalGroupsExpanded"
                :historical-expanded-group-keys="historicalExpandedGroupKeys"
                :is-dark-mode="isDarkMode"
                :format-amount="formatAmount"
                :get-execution-rate-color-class="getExecutionRateColorClass"
                :get-execution-rate-text-class="getExecutionRateTextClass"
                :get-progress-color-by-rate="getProgressColorByRate"
                :is-historical-primary-collapsed="isHistoricalPrimaryCollapsed"
                @update:historical-expanded-group-keys="$emit('update:historicalExpandedGroupKeys', $event)"
                @clear-selected-period="$emit('clearSelectedPeriod')"
                @toggle-all-historical-groups="$emit('toggleAllHistoricalGroups')"
                @toggle-historical-period-focus="$emit('toggleHistoricalPeriodFocus', $event)"
                @toggle-historical-primary-legend="$emit('toggleHistoricalPrimaryLegend', $event)"
                @toggle-historical-secondary-legend="$emit('toggleHistoricalSecondaryLegend', $event)"
                @toggle-historical-primary-collapse="(groupKey, row) => $emit('toggleHistoricalPrimaryCollapse', groupKey, row)"
            />
        </div>

        <div v-else class="d-flex flex-column align-center justify-center py-16">
            <v-icon :icon="mdiChartBoxOutline" size="48" color="grey-lighten-1" class="mb-3"/>
            <span class="text-body-1 text-medium-emphasis">{{ tt('No historical budget data') }}</span>
            <span class="text-caption text-disabled mt-1">{{ tt('Create budgets and wait for execution data') }}</span>
        </div>
    </v-card-text>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from '@/locales/helpers.ts';
import type {
    HistoricalBudgetCategoryRow,
    HistoricalBudgetPeriodGroup
} from '../historyGrouping.ts';
import type {
    HistoricalLegendGroup,
    HistoricalPolarChartModel
} from '../historyPolarChart.ts';
import type { HistoricalBudgetLevel } from '../budgetPageTypes.ts';
import BudgetHistoryLegend from './BudgetHistoryLegend.vue';
import BudgetHistoryPeriodGroups from './BudgetHistoryPeriodGroups.vue';
import { mdiChartBoxOutline } from '@mdi/js';

interface ResizableChartComponent {
    resize?: () => void;
    chart?: {
        resize?: () => void;
    };
}

defineProps<{
    loading: boolean;
    isHistoricalHistoryReady: boolean;
    canShowHistoricalBudgetPanel: boolean;
    historicalChartModel: HistoricalPolarChartModel;
    historicalChartRenderKey: string;
    historicalChartOptions: unknown;
    historicalChartUpdateOptions: Record<string, unknown>;
    historicalLegendGroups: HistoricalLegendGroup[];
    historicalBudgetLevel: HistoricalBudgetLevel;
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

defineEmits<{
    (e: 'update:historicalExpandedGroupKeys', keys: string[]): void;
    (e: 'clearSelectedPeriod'): void;
    (e: 'toggleAllHistoricalGroups'): void;
    (e: 'toggleHistoricalPeriodFocus', groupKey: string): void;
    (e: 'toggleHistoricalPrimaryLegend', primaryKey: string): void;
    (e: 'toggleHistoricalSecondaryLegend', secondaryKey: string): void;
    (e: 'toggleHistoricalPrimaryCollapse', groupKey: string, row: HistoricalBudgetCategoryRow): void;
}>();

const historicalChartRef = ref<ResizableChartComponent | null>(null);
const { tt } = useI18n();

function resize(): void {
    const chartComponent = historicalChartRef.value;
    chartComponent?.resize?.();
    chartComponent?.chart?.resize?.();
}

defineExpose({ resize });
</script>

<style scoped>
.budget-history-chart {
    width: 100%;
    height: 520px;
}

.budget-history-chart-shell {
    position: relative;
}

.budget-history-loading-placeholder {
    min-height: 440px;
}

.budget-history-loading-bar {
    max-width: 480px;
}

.budget-history-panel {
    background-color: transparent;
}
</style>
