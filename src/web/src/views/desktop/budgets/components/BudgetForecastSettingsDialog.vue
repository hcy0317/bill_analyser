<template>
    <v-dialog :model-value="modelValue" max-width="640" @update:model-value="emit('update:modelValue', Boolean($event))">
        <v-card class="pa-2 pa-sm-4 pa-md-8">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center">
                        <h4 class="text-h4">{{ tt('Forecast Settings') }}</h4>
                    </div>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" @click="emit('update:modelValue', false)">
                        <v-icon :icon="mdiClose" size="24" />
                    </v-btn>
                </div>
            </template>
            <v-card-text class="mt-md-4 pt-0">
                <v-row>
                    <v-col cols="12">
                        <v-select class="budget-forecast-setting-control"
                                  density="compact"
                                  hide-details
                                  variant="outlined"
                                  :disabled="loading || forecastLoading"
                                  :label="tt('Forecast Sort')"
                                  :aria-label="tt('Forecast Sort')"
                                  :items="forecastSortOptions"
                                  item-title="name"
                                  item-value="value"
                                  :model-value="forecastSortBy"
                                  @update:model-value="emit('update:forecastSortBy', String($event))" />
                    </v-col>

                    <v-col cols="12">
                        <v-select class="budget-forecast-setting-control"
                                  density="compact"
                                  hide-details
                                  variant="outlined"
                                  :disabled="loading || forecastLoading"
                                  :label="tt('Forecast Strategy')"
                                  :aria-label="tt('Forecast Strategy')"
                                  :items="forecastStrategies"
                                  item-title="name"
                                  item-value="value"
                                  :model-value="forecastStrategy"
                                  @update:model-value="emit('update:forecastStrategy', $event as BudgetForecastStrategy)" />
                    </v-col>

                    <v-col cols="12">
                        <v-select class="budget-forecast-setting-control"
                                  density="compact"
                                  hide-details
                                  variant="outlined"
                                  :disabled="loading || forecastLoading"
                                  :label="tt('History Periods')"
                                  :aria-label="tt('History Periods')"
                                  :items="historyPeriodOptions"
                                  :model-value="forecastMonthsHistory"
                                  @update:model-value="emit('update:forecastMonthsHistory', Number($event))" />
                    </v-col>
                </v-row>
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="emit('update:modelValue', false)">{{ tt('Close') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import { useI18n } from '@/locales/helpers.ts';
import { BudgetForecastStrategy } from '@/models/budget.ts';
import { mdiClose } from '@mdi/js';

defineProps<{
    modelValue: boolean;
    loading: boolean;
    forecastLoading: boolean;
    forecastSortOptions: Array<{ name: string; value: string }>;
    forecastStrategies: Array<{ name: string; value: BudgetForecastStrategy }>;
    historyPeriodOptions: number[];
    forecastSortBy: string;
    forecastStrategy: BudgetForecastStrategy;
    forecastMonthsHistory: number;
}>();

const emit = defineEmits<{
    (e: 'update:modelValue', value: boolean): void;
    (e: 'update:forecastSortBy', value: string): void;
    (e: 'update:forecastStrategy', value: BudgetForecastStrategy): void;
    (e: 'update:forecastMonthsHistory', value: number): void;
}>();

const { tt } = useI18n();
</script>

<style scoped>
.budget-forecast-setting-control {
    width: 100%;
}

.budget-forecast-setting-control :deep(.v-field) {
    min-height: 48px;
}
</style>
