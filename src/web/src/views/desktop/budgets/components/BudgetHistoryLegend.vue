<template>
    <div v-if="legendGroups.length > 0" class="budget-history-legend">
        <div v-for="group in legendGroups" :key="group.primaryKey" class="budget-history-legend-group">
            <button
                type="button"
                class="budget-history-legend-item budget-history-legend-item--primary"
                :class="{
                    'is-inactive': group.state === 'none',
                    'is-partial': group.state === 'partial'
                }"
                :aria-pressed="group.state !== 'none'"
                :aria-label="group.primaryLabel"
                :title="group.primaryLabel"
                @click="$emit('togglePrimary', group.primaryKey)"
            >
                <span class="budget-history-legend-swatch" :style="{ backgroundColor: group.color }"></span>
                <span class="budget-history-legend-label">{{ group.primaryLabel }}</span>
                <span v-if="historicalBudgetLevel === 'secondary'" class="budget-history-legend-count">
                    {{ group.secondaryItems.filter(item => item.selected).length }}/{{ group.secondaryItems.length }}
                </span>
            </button>

            <div v-if="historicalBudgetLevel === 'secondary'" class="budget-history-legend-secondary-list">
                <button
                    v-for="item in group.secondaryItems"
                    :key="item.key"
                    type="button"
                    class="budget-history-legend-item budget-history-legend-item--secondary"
                    :class="{ 'is-inactive': !item.selected }"
                    :aria-pressed="item.selected"
                    :aria-label="item.label"
                    :title="item.label"
                    @click="$emit('toggleSecondary', item.key)"
                >
                    <span class="budget-history-legend-swatch" :style="{ backgroundColor: item.color }"></span>
                    <span class="budget-history-legend-label">{{ item.label }}</span>
                </button>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import type { HistoricalLegendGroup } from '../historyPolarChart.ts';
import type { HistoricalBudgetLevel } from '../budgetPageTypes.ts';

defineProps<{
    legendGroups: HistoricalLegendGroup[];
    historicalBudgetLevel: HistoricalBudgetLevel;
}>();

defineEmits<{
    (e: 'togglePrimary', primaryKey: string): void;
    (e: 'toggleSecondary', secondaryKey: string): void;
}>();
</script>

<style scoped>
.budget-history-legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
}

.budget-history-legend-group {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    max-width: 100%;
}

.budget-history-legend-secondary-list {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 3px;
    min-width: 0;
    max-width: 100%;
}

.budget-history-legend-item {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
    border-radius: 999px;
    background-color: rgba(var(--v-theme-surface), 0.82);
    color: rgba(var(--v-theme-on-surface), 0.88);
    cursor: pointer;
    padding: 3px 8px;
    font-size: 0.78rem;
    line-height: 1.1;
    transition: all 0.18s ease;
    min-width: 0;
    max-width: min(100%, 18rem);
}

.budget-history-legend-item:hover {
    border-color: rgba(var(--v-theme-primary), 0.32);
    transform: translateY(-1px);
}

.budget-history-legend-item--primary {
    align-self: flex-start;
    font-weight: 600;
    background-color: rgba(var(--v-theme-primary), 0.07);
}

.budget-history-legend-item--secondary {
    padding: 2px 7px;
    font-size: 0.74rem;
    border-color: rgba(var(--v-theme-on-surface), 0.09);
}

.budget-history-legend-item--primary.is-partial {
    border-style: dashed;
}

.budget-history-legend-item.is-inactive {
    opacity: 0.46;
}

.budget-history-legend-swatch {
    width: 7px;
    height: 7px;
    border-radius: 999px;
    flex-shrink: 0;
}

.budget-history-legend-label {
    min-width: 0;
    max-width: 14rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.2;
}

.budget-history-legend-count {
    color: rgba(var(--v-theme-on-surface), 0.6);
    font-size: 0.75rem;
}
</style>
