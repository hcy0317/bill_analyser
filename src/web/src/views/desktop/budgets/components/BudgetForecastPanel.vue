<template>
    <v-table class="forecast-table table-striped" :hover="!forecastLoading">
        <thead>
        <tr>
            <th style="width: 20%;">{{ tt('Category') }}</th>
            <th style="width: 15%;">{{ tt('Historical Average') }}</th>
            <th style="width: 15%;">{{ tt('Current Spent') }}</th>
            <th style="width: 15%;">{{ tt('Projected Total') }}</th>
            <th style="width: 15%;">{{ tt('Budget') }}</th>
            <th style="width: 10%;">{{ tt('Trend') }}</th>
            <th style="width: 10%;">{{ tt('Status') }}</th>
        </tr>
        </thead>

        <tbody v-if="forecastLoading && (!currentForecast || currentForecast.forecasts.length === 0)">
        <tr :key="itemIdx" v-for="itemIdx in [1, 2, 3, 4, 5]">
            <td class="px-0" colspan="7">
                <v-skeleton-loader type="text" :loading="true"></v-skeleton-loader>
            </td>
        </tr>
        </tbody>

        <tbody v-if="!forecastLoading && (!currentForecast || currentForecast.forecasts.length === 0)">
        <tr>
            <td colspan="7">
                <div class="d-flex flex-column align-center justify-center py-12">
                    <v-icon :icon="mdiChartTimelineVariant" size="48" color="grey-lighten-1" class="mb-3"/>
                    <span class="text-body-1 text-medium-emphasis">{{ tt('No forecast data') }}</span>
                </div>
            </td>
        </tr>
        </tbody>

        <tbody v-if="displayForecasts.length > 0">
        <tr v-for="forecast in displayForecasts" :key="forecast.categoryId" class="text-sm">
            <td>
                <div>{{ forecast.categoryName }}</div>
                <div class="text-caption text-medium-emphasis" v-if="forecast.strategyExplanation">
                    {{ forecast.strategyExplanation }}
                </div>
                <div class="text-caption text-medium-emphasis" v-if="forecast.samplePeriods !== null && forecast.samplePeriods !== undefined">
                    {{ tt('Sample Periods') }}: {{ forecast.samplePeriods }}
                </div>
            </td>
            <td>{{ formatAmount(forecast.historicalAverage / 100) }}</td>
            <td>{{ formatAmount(forecast.currentSpent / 100) }}</td>
            <td :class="{ 'text-error': forecast.projectedOverBudget }">
                {{ formatAmount(forecast.projectedTotal / 100) }}
            </td>
            <td>{{ formatAmount(forecast.budgetAmount / 100) }}</td>
            <td>
                <v-icon v-if="forecast.trend === 'up'" :icon="mdiTrendingUp" color="error" size="20" />
                <v-icon v-else-if="forecast.trend === 'down'" :icon="mdiTrendingDown" color="success" size="20" />
                <v-icon v-else :icon="mdiTrendingNeutral" color="grey" size="20" />
            </td>
            <td>
                <div class="d-flex flex-column ga-1">
                    <v-chip v-if="forecast.projectedOverBudget" color="error" size="small">
                        {{ tt('Over Budget') }}
                    </v-chip>
                    <v-chip v-else color="success" size="small">
                        {{ tt('On Track') }}
                    </v-chip>
                    <span class="text-caption text-medium-emphasis" v-if="forecast.backtestMape !== null && forecast.backtestMape !== undefined">
                        {{ tt('Backtest MAPE') }}: {{ forecast.backtestMape.toFixed(2) }}%
                        <v-icon class="ms-1" :icon="mdiInformationOutline" size="14" />
                        <v-tooltip activator="parent" location="top">
                            {{ tt('Backtest MAPE Hint') }}
                        </v-tooltip>
                    </span>
                    <span class="text-caption" :class="getForecastConfidenceClass(forecast.confidence)" v-if="forecast.confidence">
                        {{ tt('Confidence') }}: {{ tt(getForecastConfidenceLabel(forecast.confidence)) }}
                        <v-icon class="ms-1" :icon="mdiInformationOutline" size="14" />
                        <v-tooltip activator="parent" location="top">
                            {{ tt('Forecast Confidence Hint') }}
                        </v-tooltip>
                    </span>
                </div>
            </td>
        </tr>
        </tbody>
    </v-table>

    <v-card-text v-if="currentForecast" class="border-t">
        <div class="budget-forecast-summary-row d-flex align-center flex-wrap ga-6 text-subtitle-2">
            <div>
                <span>{{ tt('Period') }}: </span>
                <span>{{ currentForecast.periodStart }} - {{ currentForecast.periodEnd }}</span>
            </div>
            <div>
                <span>{{ tt('Forecast Strategy') }}: </span>
                <span>{{ tt(currentForecast.forecastStrategy === 'moving_average' ? 'Moving Average Strategy' : 'Historical Average Strategy') }}</span>
            </div>
            <div>
                <span>{{ tt('History Periods') }}: </span>
                <span>{{ currentForecast.historyPeriods || forecastMonthsHistory }}</span>
            </div>
            <div>
                <span>{{ tt('Low Confidence Count', { count: forecastRiskSummary.lowConfidenceCount }) }}</span>
            </div>
            <div>
                <span>{{ tt('Over Budget Count', { count: forecastRiskSummary.overBudgetCount }) }}</span>
            </div>
            <div v-if="currentForecast.avgBacktestMape !== null && currentForecast.avgBacktestMape !== undefined">
                <span>{{ tt('Average Backtest MAPE') }}: </span>
                <span>{{ currentForecast.avgBacktestMape.toFixed(2) }}%</span>
                <v-icon class="ms-1" :icon="mdiInformationOutline" size="14" />
                <v-tooltip activator="parent" location="top">
                    {{ tt('Average Backtest MAPE Hint') }}
                </v-tooltip>
            </div>
        </div>
    </v-card-text>
</template>

<script setup lang="ts">
import { useI18n } from '@/locales/helpers.ts';
import type { BudgetForecastItem, BudgetForecastResponse } from '@/models/budget.ts';
import {
    mdiChartTimelineVariant,
    mdiInformationOutline,
    mdiTrendingDown,
    mdiTrendingNeutral,
    mdiTrendingUp
} from '@mdi/js';

defineProps<{
    forecastLoading: boolean;
    currentForecast: BudgetForecastResponse | null;
    displayForecasts: BudgetForecastItem[];
    forecastMonthsHistory: number;
    forecastRiskSummary: {
        lowConfidenceCount: number;
        overBudgetCount: number;
    };
    formatAmount: (amount: number) => string;
    getForecastConfidenceLabel: (confidence?: 'high' | 'medium' | 'low' | null) => string;
    getForecastConfidenceClass: (confidence?: 'high' | 'medium' | 'low' | null) => string;
}>();

const { tt } = useI18n();
</script>

<style scoped>
.forecast-table .text-sm {
    font-size: 0.875rem;
}

.budget-forecast-summary-row {
    column-gap: 28px;
    row-gap: 8px;
}
</style>
