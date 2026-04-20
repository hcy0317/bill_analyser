<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-text>
                    <div class="d-flex align-center mb-4">
                        <v-icon :icon="mdiLightbulbOn" class="mr-2" />
                        <span class="text-h6">{{ tt('Anomaly Insights') }}</span>
                        <v-chip v-if="data.totalCount > 0" class="ml-2" size="small"
                                variant="tonal" color="warning">
                            {{ data.totalCount }} {{ tt('anomalies found') }}
                        </v-chip>
                        <v-spacer />
                        <v-select v-model="months" :items="monthOptions" variant="outlined"
                                  density="compact" style="max-width: 160px;" class="mr-2"
                                  hide-details />
                        <v-btn variant="outlined" :disabled="loading" @click="fetchAnomalies">
                            <v-icon start :icon="mdiRefresh" />
                            {{ tt('Analyze') }}
                        </v-btn>
                    </div>

                    <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />

                    <v-alert v-if="error" type="error" closable class="mb-4" @click:close="error = null">
                        {{ error }}
                    </v-alert>

                    <v-alert v-if="!loading && data.anomalies.length === 0" type="success" class="mb-4">
                        {{ tt('No anomalies detected. Your finances look healthy!') }}
                    </v-alert>

                    <!-- Anomaly Cards -->
                    <div v-for="(anomaly, i) in data.anomalies" :key="i" class="mb-3">
                        <v-card variant="outlined" :color="severityColor(anomaly.severity)">
                            <v-card-text>
                                <div class="d-flex align-center mb-1">
                                    <v-icon :icon="anomalyIcon(anomaly.type)" size="small" class="mr-2"
                                            :color="severityColor(anomaly.severity)" />
                                    <v-chip :color="severityColor(anomaly.severity)" size="x-small"
                                            variant="tonal" class="mr-2">
                                        {{ anomaly.type === 'large_transaction' ? tt('Large Transaction') :
                                           anomaly.type === 'duplicate_charge' ? tt('Possible Duplicate') :
                                           tt('Category Spike') }}
                                    </v-chip>
                                    <span class="text-caption text-grey">
                                        {{ anomaly.date || anomaly.month || '' }}
                                    </span>
                                </div>
                                <div class="text-body-2">{{ anomaly.message }}</div>
                            </v-card-text>
                        </v-card>
                    </div>

                    <!-- Footer summary -->
                    <div v-if="data.analyzedBills > 0" class="text-caption text-grey mt-4">
                        {{ tt('Analyzed') }} {{ data.analyzedBills }} {{ tt('transactions over') }}
                        {{ data.analyzedMonths }} {{ tt('months') }}
                        ({{ data.startDate }} ~ {{ data.endDate }})
                    </div>
                </v-card-text>
            </v-card>
        </v-col>
    </v-row>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import {
    mdiLightbulbOn, mdiRefresh, mdiAlertCircle,
    mdiContentDuplicate, mdiTrendingUp
} from '@mdi/js';
import services from '@/lib/services.ts';

const tt = (key: string) => key;

const loading = ref(false);
const error = ref<string | null>(null);
const months = ref(6);

const monthOptions = [
    { title: '3 months', value: 3 },
    { title: '6 months', value: 6 },
    { title: '12 months', value: 12 },
];

interface AnomalyData {
    anomalies: any[];
    totalCount: number;
    analyzedBills: number;
    analyzedMonths: number;
    startDate: string;
    endDate: string;
}

const data = ref<AnomalyData>({
    anomalies: [], totalCount: 0,
    analyzedBills: 0, analyzedMonths: 0,
    startDate: '', endDate: '',
});

function severityColor(severity: string) {
    if (severity === 'error') return 'error';
    if (severity === 'warning') return 'warning';
    return 'info';
}

function anomalyIcon(type: string) {
    if (type === 'large_transaction') return mdiAlertCircle;
    if (type === 'duplicate_charge') return mdiContentDuplicate;
    return mdiTrendingUp;
}

async function fetchAnomalies() {
    loading.value = true;
    error.value = null;
    try {
        const resp = await services.getAnomalies({ months: months.value });
        if (resp.data.success && resp.data.result) {
            data.value = resp.data.result;
        }
    } catch (e: any) {
        error.value = e.message || 'Failed to analyze';
    } finally {
        loading.value = false;
    }
}

watch(months, () => fetchAnomalies());

onMounted(() => fetchAnomalies());
</script>
