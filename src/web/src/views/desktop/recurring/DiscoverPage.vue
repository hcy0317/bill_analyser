<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-text>
                    <div class="d-flex align-center mb-4">
                        <v-icon :icon="mdiCalendarSync" class="mr-2" />
                        <span class="text-h6">{{ tt('Recurring Discovery') }}</span>
                        <v-spacer />
                        <v-btn variant="tonal" color="primary" class="mr-2"
                               :disabled="loading"
                               @click="handleDetect">
                            <v-icon start :icon="mdiMagnifyScan" />
                            {{ tt('Discover Patterns') }}
                        </v-btn>
                        <v-btn variant="outlined" :disabled="loading" @click="handleRefresh">
                            <v-icon start :icon="mdiRefresh" />
                            {{ tt('Refresh') }}
                        </v-btn>
                    </div>

                    <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />

                    <v-alert v-if="error" type="error" closable class="mb-4"
                             @click:close="error = null">
                        {{ error }}
                    </v-alert>

                    <v-alert v-if="lastDetectResult" type="info" closable class="mb-4"
                             @click:close="lastDetectResult = null">
                        {{ tt('Detected') }}: {{ lastDetectResult.detected }} {{ tt('patterns') }},
                        {{ lastDetectResult.created }} {{ tt('new') }},
                        {{ lastDetectResult.updated }} {{ tt('updated') }}
                    </v-alert>

                    <!-- Status Filter -->
                    <div class="d-flex align-center mb-3">
                        <v-chip-group v-model="statusFilter" mandatory>
                            <v-chip value="" variant="tonal">{{ tt('All') }} ({{ suggestionsTotal }})</v-chip>
                            <v-chip value="pending" variant="tonal" color="warning">{{ tt('Pending') }}</v-chip>
                            <v-chip value="accepted" variant="tonal" color="success">{{ tt('Accepted') }}</v-chip>
                            <v-chip value="rejected" variant="tonal" color="error">{{ tt('Rejected') }}</v-chip>
                        </v-chip-group>
                    </div>

                    <!-- Suggestions Table -->
                    <v-data-table
                        :headers="tableHeaders"
                        :items="filteredSuggestions"
                        :loading="loading"
                        item-value="id"
                        density="compact"
                        class="elevation-1"
                    >
                        <template #item.name="{ item }">
                            <div>
                                <strong>{{ item.name }}</strong>
                                <div class="text-caption text-medium-emphasis">{{ item.counterparty }}</div>
                            </div>
                        </template>

                        <template #item.type="{ item }">
                            <v-chip size="small" :color="item.type === 'expense' ? 'error' : 'success'" variant="tonal">
                                {{ item.type === 'expense' ? tt('Expense') : tt('Income') }}
                            </v-chip>
                        </template>

                        <template #item.amount="{ item }">
                            <span :class="item.type === 'expense' ? 'text-error' : 'text-success'">
                                {{ item.type === 'expense' ? '-' : '+' }}{{ formatAmount(item.amount) }}
                            </span>
                        </template>

                        <template #item.frequency="{ item }">
                            <v-chip size="small" variant="tonal" color="info">
                                {{ getFrequencyLabel(item.frequency) }}
                            </v-chip>
                        </template>

                        <template #item.confidenceScore="{ item }">
                            <v-chip size="small" :color="getConfidenceColor(item.confidenceScore)" variant="tonal">
                                {{ Math.round(item.confidenceScore * 100) }}%
                            </v-chip>
                        </template>

                        <template #item.sampleCount="{ item }">
                            {{ item.sampleCount }} {{ tt('transactions') }}
                        </template>

                        <template #item.period="{ item }">
                            <div class="text-caption">
                                {{ item.firstOccurrence }} ~ {{ item.lastOccurrence }}
                            </div>
                        </template>

                        <template #item.suggestedNextDate="{ item }">
                            <span class="text-primary">{{ item.suggestedNextDate }}</span>
                        </template>

                        <template #item.status="{ item }">
                            <v-chip size="small" :color="statusColor(item.status)" variant="tonal">
                                {{ statusLabel(item.status) }}
                            </v-chip>
                        </template>

                        <template #item.actions="{ item }">
                            <div v-if="item.status === 'pending'" class="d-flex ga-1">
                                <v-btn size="x-small" color="success" variant="tonal"
                                       @click="handleAccept(item.id)">
                                    <v-icon :icon="mdiCheck" />
                                </v-btn>
                                <v-btn size="x-small" color="error" variant="tonal"
                                       @click="handleReject(item.id)">
                                    <v-icon :icon="mdiClose" />
                                </v-btn>
                            </div>
                        </template>
                    </v-data-table>
                </v-card-text>
            </v-card>
        </v-col>
    </v-row>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { useI18n } from 'vue-i18n';

import {
    mdiCalendarSync,
    mdiMagnifyScan,
    mdiRefresh,
    mdiCheck,
    mdiClose,
} from '@mdi/js';

import { useRecurringStore } from '@/stores/recurring.ts';
import type { RecurringDetectResponse } from '@/models/recurring_suggestion.ts';
import { getFrequencyLabel, getConfidenceColor } from '@/models/recurring_suggestion.ts';

const { t: tt } = useI18n();
const store = useRecurringStore();

const statusFilter = ref('');
const lastDetectResult = ref<RecurringDetectResponse | null>(null);

const loading = computed(() => store.loading);
const error = computed({
    get: () => store.error,
    set: (val) => { store.error = val; }
});
const suggestions = computed(() => store.suggestions);
const suggestionsTotal = computed(() => store.suggestionsTotal);

const filteredSuggestions = computed(() => {
    if (!statusFilter.value) return suggestions.value;
    return suggestions.value.filter(s => s.status === statusFilter.value);
});

const tableHeaders = computed(() => [
    { title: tt('Name'), key: 'name', sortable: true },
    { title: tt('Type'), key: 'type', sortable: true, width: 80 },
    { title: tt('Amount'), key: 'amount', sortable: true, width: 100 },
    { title: tt('Frequency'), key: 'frequency', sortable: true, width: 100 },
    { title: tt('Confidence'), key: 'confidenceScore', sortable: true, width: 100 },
    { title: tt('Samples'), key: 'sampleCount', sortable: true, width: 100 },
    { title: tt('Period'), key: 'period', sortable: false, width: 180 },
    { title: tt('Next Date'), key: 'suggestedNextDate', sortable: true, width: 110 },
    { title: tt('Status'), key: 'status', sortable: true, width: 90 },
    { title: tt('Actions'), key: 'actions', sortable: false, width: 90 },
]);

function formatAmount(amount: number): string {
    return amount.toFixed(2);
}

function statusColor(status: string): string {
    if (status === 'accepted') return 'success';
    if (status === 'rejected') return 'error';
    return 'warning';
}

function statusLabel(status: string): string {
    const labels: Record<string, string> = {
        pending: tt('Pending'),
        accepted: tt('Accepted'),
        rejected: tt('Rejected'),
    };
    return labels[status] || status;
}

async function handleDetect(): Promise<void> {
    const result = await store.detectPatterns();
    if (result) {
        lastDetectResult.value = result;
    }
}

async function handleRefresh(): Promise<void> {
    await store.loadSuggestions(statusFilter.value || undefined);
}

async function handleAccept(id: number): Promise<void> {
    await store.acceptSuggestion(id);
}

async function handleReject(id: number): Promise<void> {
    await store.rejectSuggestion(id);
}

watch(statusFilter, () => {
    store.loadSuggestions(statusFilter.value || undefined);
});

onMounted(() => {
    store.loadSuggestions();
});
</script>
