<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card data-testid="desktop.action-center.page">
                <v-card-text>
                    <div class="title-and-toolbar d-flex flex-wrap align-center ga-2 mb-4">
                        <div class="d-flex align-center">
                            <v-icon :icon="mdiInboxArrowDownOutline" class="mr-2" />
                            <span class="text-h6">{{ tt('Action Center') }}</span>
                            <v-chip v-if="summary.hasWork" class="ml-2" color="warning" size="small" variant="tonal">
                                {{ summary.total }}
                            </v-chip>
                        </div>
                        <v-spacer />
                        <v-select
                            v-model="months"
                            :items="monthOptions"
                            :label="tt('Analysis Range')"
                            variant="outlined"
                            density="compact"
                            hide-details
                            class="action-center-months"
                        />
                        <v-btn variant="tonal" color="primary" :loading="detecting" :disabled="loading" @click="detectRecurring">
                            <v-icon start :icon="mdiMagnifyScan" />
                            {{ tt('Discover Patterns') }}
                        </v-btn>
                        <v-btn variant="outlined" :loading="loading" :disabled="detecting" @click="load">
                            <v-icon start :icon="mdiRefresh" />
                            {{ tt('Refresh') }}
                        </v-btn>
                    </div>

                    <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />
                    <v-alert v-if="error" type="error" closable class="mb-4" @click:close="error = null">
                        {{ tt(error) }}
                    </v-alert>
                    <v-alert v-if="detectResult" type="info" closable class="mb-4" @click:close="detectResult = null">
                        {{ tt('Recurring detection result', {
                            detected: detectResult.detected,
                            created: detectResult.created,
                            updated: detectResult.updated
                        }) }}
                    </v-alert>
                    <v-alert v-if="!loading && !summary.hasWork" type="success" class="mb-4">
                        {{ tt('No pending actions') }}
                    </v-alert>

                    <section aria-labelledby="anomaly-actions-title">
                        <div class="d-flex align-center mb-2">
                            <v-icon :icon="mdiAlertCircleOutline" size="small" class="mr-2" />
                            <h2 id="anomaly-actions-title" class="text-subtitle-1 font-weight-medium">
                                {{ tt('Anomaly Insights') }}
                            </h2>
                            <v-chip class="ml-2" size="x-small" variant="tonal">
                                {{ summary.anomalyCount }}
                            </v-chip>
                        </div>
                        <v-list v-if="anomalyData.items.length" lines="two" class="pa-0">
                            <template v-for="(anomaly, index) in anomalyData.items" :key="anomaly.key">
                                <v-divider v-if="index > 0" />
                                <v-list-item class="px-0 py-2">
                                    <template #prepend>
                                        <v-avatar :color="severityColor(anomaly.severity)" variant="tonal" size="36">
                                            <v-icon :icon="anomalyIcon(anomaly.type)" size="20" />
                                        </v-avatar>
                                    </template>
                                    <v-list-item-title class="font-weight-medium">
                                        {{ tt(anomalyTitle(anomaly.type)) }}
                                    </v-list-item-title>
                                    <v-list-item-subtitle class="action-center-subtitle">
                                        {{ anomalyContext(anomaly) }}
                                        <span v-if="anomaly.occurredOn"> · {{ anomaly.occurredOn }}</span>
                                        <span v-if="anomaly.amountCents"> · {{ formatAmount(anomaly.amountCents) }}</span>
                                    </v-list-item-subtitle>
                                    <template #append>
                                        <v-btn variant="text" color="primary" @click="reviewAnomaly(anomaly)">
                                            <v-icon start :icon="mdiArrowRight" />
                                            {{ tt('Review') }}
                                        </v-btn>
                                    </template>
                                </v-list-item>
                            </template>
                        </v-list>
                        <p v-else-if="!loading" class="text-body-2 text-medium-emphasis py-3 mb-0">
                            {{ tt('No anomalies detected. Your finances look healthy!') }}
                        </p>
                        <p v-if="anomalyData.analyzedBills > 0" class="text-caption text-medium-emphasis mt-2 mb-0">
                            {{ tt('Analysis coverage', {
                                bills: anomalyData.analyzedBills,
                                months: anomalyData.analyzedMonths,
                                start: anomalyData.startDate,
                                end: anomalyData.endDate
                            }) }}
                        </p>
                    </section>

                    <v-divider class="my-5" />

                    <section aria-labelledby="recurring-actions-title">
                        <div class="d-flex align-center mb-2">
                            <v-icon :icon="mdiCalendarSync" size="small" class="mr-2" />
                            <h2 id="recurring-actions-title" class="text-subtitle-1 font-weight-medium">
                                {{ tt('Recurring Suggestions') }}
                            </h2>
                            <v-chip class="ml-2" size="x-small" variant="tonal">
                                {{ summary.recurringCount }}
                            </v-chip>
                        </div>
                        <v-tabs v-model="recurringView" color="primary" density="compact" class="mb-2">
                            <v-tab value="pending">
                                {{ tt('Pending') }}
                                <v-chip class="ml-2" size="x-small" variant="tonal">
                                    {{ recurringSuggestions.length }}
                                </v-chip>
                            </v-tab>
                            <v-tab value="history">
                                {{ tt('Review History') }}
                                <v-chip class="ml-2" size="x-small" variant="tonal">
                                    {{ recurringHistory.length }}
                                </v-chip>
                            </v-tab>
                        </v-tabs>
                        <v-list v-if="recurringView === 'pending' && recurringSuggestions.length" lines="three" class="pa-0">
                            <template v-for="(suggestion, index) in recurringSuggestions" :key="suggestion.id">
                                <v-divider v-if="index > 0" />
                                <v-list-item class="px-0 py-2">
                                    <v-list-item-title class="font-weight-medium">
                                        {{ suggestion.name || suggestion.counterparty }}
                                    </v-list-item-title>
                                    <v-list-item-subtitle class="action-center-subtitle">
                                        {{ suggestion.counterparty || suggestion.description }}
                                        <span v-if="suggestion.suggestedNextDate"> · {{ tt('Next Date') }}: {{ suggestion.suggestedNextDate }}</span>
                                    </v-list-item-subtitle>
                                    <v-list-item-subtitle>
                                        {{ formatAmount(suggestion.amountCents) }} · {{ tt('Confidence') }} {{ formatPercent(suggestion.confidenceScore) }}
                                    </v-list-item-subtitle>
                                    <template #append>
                                        <div class="d-flex ga-1">
                                            <v-btn
                                                variant="tonal"
                                                color="success"
                                                :loading="mutatingSuggestionId === suggestion.id"
                                                :disabled="mutatingSuggestionId !== null"
                                                @click="acceptRecurring(suggestion.id)"
                                            >
                                                <v-icon start :icon="mdiCheck" />
                                                {{ tt('Accept') }}
                                            </v-btn>
                                            <v-btn
                                                variant="text"
                                                :disabled="mutatingSuggestionId !== null"
                                                @click="rejectRecurring(suggestion.id)"
                                            >
                                                <v-icon start :icon="mdiClose" />
                                                {{ tt('Ignore') }}
                                            </v-btn>
                                        </div>
                                    </template>
                                </v-list-item>
                            </template>
                        </v-list>
                        <p v-else-if="recurringView === 'pending' && !loading" class="text-body-2 text-medium-emphasis py-3 mb-0">
                            {{ tt('No pending recurring suggestions') }}
                        </p>
                        <v-list v-if="recurringView === 'history' && recurringHistory.length" lines="three" class="pa-0">
                            <template v-for="(suggestion, index) in recurringHistory" :key="suggestion.id">
                                <v-divider v-if="index > 0" />
                                <v-list-item class="px-0 py-2">
                                    <v-list-item-title class="font-weight-medium">
                                        {{ suggestion.name || suggestion.counterparty }}
                                    </v-list-item-title>
                                    <v-list-item-subtitle class="action-center-subtitle">
                                        {{ suggestion.counterparty || suggestion.description }}
                                        <span v-if="suggestion.suggestedNextDate"> · {{ tt('Next Date') }}: {{ suggestion.suggestedNextDate }}</span>
                                    </v-list-item-subtitle>
                                    <v-list-item-subtitle>
                                        {{ formatAmount(suggestion.amountCents) }} · {{ tt('Confidence') }} {{ formatPercent(suggestion.confidenceScore) }}
                                    </v-list-item-subtitle>
                                    <template #append>
                                        <v-chip
                                            :color="suggestion.status === 'accepted' ? 'success' : 'error'"
                                            size="small"
                                            variant="tonal"
                                        >
                                            {{ tt(suggestion.status === 'accepted' ? 'Accepted' : 'Rejected') }}
                                        </v-chip>
                                    </template>
                                </v-list-item>
                            </template>
                        </v-list>
                        <p v-else-if="recurringView === 'history' && !loading" class="text-body-2 text-medium-emphasis py-3 mb-0">
                            {{ tt('No recurring suggestion history') }}
                        </p>
                    </section>
                </v-card-text>
            </v-card>
        </v-col>
    </v-row>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import {
    mdiAlertCircleOutline,
    mdiArrowRight,
    mdiCalendarSync,
    mdiCheck,
    mdiClose,
    mdiContentDuplicate,
    mdiInboxArrowDownOutline,
    mdiMagnifyScan,
    mdiRefresh,
    mdiTrendingUp
} from '@mdi/js';

import { useI18n } from '@/locales/helpers.ts';
import {
    buildAnomalyActionTarget,
    type ActionCenterAnomaly,
    type ActionCenterAnomalyType,
    type ActionCenterSeverity,
    type DesktopActionTarget
} from '@/views/base/action-center/actionCenterModel.ts';
import { useActionCenter } from '@/views/base/action-center/useActionCenter.ts';

const route = useRoute();
const router = useRouter();
const { tt, formatAmountToLocalizedNumerals } = useI18n();
const {
    months,
    loading,
    detecting,
    mutatingSuggestionId,
    error,
    detectResult,
    anomalyData,
    recurringSuggestions,
    recurringHistory,
    summary,
    load,
    detectRecurring,
    acceptRecurring,
    rejectRecurring
} = useActionCenter();

const recurringView = ref<'pending' | 'history'>(
    route.query['recurring'] === 'history' ? 'history' : 'pending'
);

const monthOptions = computed(() => [3, 6, 12].map(value => ({
    title: tt('Month count', { count: value }),
    value
})));

function severityColor(value: ActionCenterSeverity): string {
    if (value === 'error') return 'error';
    if (value === 'warning') return 'warning';
    return 'info';
}

function anomalyIcon(type: ActionCenterAnomalyType): string {
    if (type === 'large_transaction') return mdiAlertCircleOutline;
    if (type === 'duplicate_charge') return mdiContentDuplicate;
    return mdiTrendingUp;
}

function anomalyTitle(type: ActionCenterAnomalyType): string {
    if (type === 'large_transaction') return 'Large Transaction';
    if (type === 'duplicate_charge') return 'Possible Duplicate';
    return 'Category Spike';
}

function anomalyContext(anomaly: ActionCenterAnomaly): string {
    return anomaly.counterparty || anomaly.description || anomaly.category || tt('Transaction');
}

function formatAmount(value: number): string {
    return formatAmountToLocalizedNumerals(value / 100);
}

function formatPercent(value: number): string {
    return `${Math.round(value * 100)}%`;
}

function reviewAnomaly(anomaly: ActionCenterAnomaly): void {
    router.push(buildAnomalyActionTarget(anomaly, 'desktop') as DesktopActionTarget);
}

watch(months, () => load());
watch(
    () => route.query['recurring'],
    value => { recurringView.value = value === 'history' ? 'history' : 'pending'; }
);
watch(recurringView, value => {
    const recurringQuery = route.query['recurring'];
    if (value === 'history') {
        if (recurringQuery !== 'history') {
            void router.replace({ query: { ...route.query, recurring: 'history' } });
        }
        return;
    }
    if (recurringQuery !== undefined) {
        const query = { ...route.query };
        delete query['recurring'];
        void router.replace({ query });
    }
});
onMounted(() => load());
</script>

<style scoped>
.action-center-months {
    flex: 0 1 170px;
    min-width: 150px;
}

.action-center-subtitle {
    white-space: normal;
}
</style>
