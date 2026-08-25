<template>
    <f7-page ptr data-testid="mobile.action-center.page" @ptr:refresh="refresh" @page:afterin="load">
        <f7-navbar>
            <f7-nav-left :back-link="tt('Back')" />
            <f7-nav-title :title="tt('Action Center')" />
            <f7-nav-right>
                <f7-link :disabled="loading || detecting" @click="detectRecurring">
                    <f7-icon f7="sparkles" />
                </f7-link>
            </f7-nav-right>
        </f7-navbar>

        <f7-toolbar tabbar top class="action-center-recurring-toolbar">
            <f7-segmented strong>
                <f7-button
                    :text="`${tt('Pending')} (${recurringSuggestions.length})`"
                    :active="recurringView === 'pending'"
                    @click="recurringView = 'pending'"
                />
                <f7-button
                    :text="`${tt('Review History')} (${recurringHistory.length})`"
                    :active="recurringView === 'history'"
                    @click="recurringView = 'history'"
                />
            </f7-segmented>
        </f7-toolbar>

        <f7-block v-if="error" class="action-center-error">
            {{ tt(error) }}
        </f7-block>

        <f7-list strong inset dividers class="action-center-controls">
            <f7-list-item :title="tt('Pending Actions')" :after="String(summary.total)">
                <template #media><f7-icon f7="tray_full" /></template>
            </f7-list-item>
            <f7-list-item
                link="#"
                :title="tt('Analysis Range')"
                :after="tt('Month count', { count: months })"
                @click="showMonthSelection = true"
            >
                <list-item-selection-popup
                    v-model="months"
                    v-model:show="showMonthSelection"
                    value-type="item"
                    key-field="value"
                    value-field="value"
                    title-field="title"
                    :title="tt('Analysis Range')"
                    :items="monthOptions"
                />
            </f7-list-item>
        </f7-list>

        <f7-block v-if="detectResult" strong inset class="action-center-notice">
            {{ tt('Recurring detection result', {
                detected: detectResult.detected,
                created: detectResult.created,
                updated: detectResult.updated
            }) }}
        </f7-block>

        <f7-block v-if="!loading && !summary.hasWork" strong inset class="action-center-empty text-align-center">
            <f7-icon f7="checkmark_circle" color="green" size="28" />
            <p>{{ tt('No pending actions') }}</p>
        </f7-block>

        <f7-block-title>{{ tt('Anomaly Insights') }} · {{ summary.anomalyCount }}</f7-block-title>
        <f7-list v-if="anomalyData.items.length" strong inset dividers media-list>
            <f7-list-item
                v-for="anomaly in anomalyData.items"
                :key="anomaly.key"
                link="#"
                :title="tt(anomalyTitle(anomaly.type))"
                :subtitle="anomalyContext(anomaly)"
                :text="anomalyDetails(anomaly)"
                @click="reviewAnomaly(anomaly)"
            >
                <template #media>
                    <f7-icon :f7="anomalyIcon(anomaly.type)" />
                </template>
            </f7-list-item>
        </f7-list>
        <f7-block v-else-if="!loading" strong inset class="action-center-empty">
            {{ tt('No anomalies detected. Your finances look healthy!') }}
        </f7-block>

        <f7-block-title>
            {{ tt('Recurring Suggestions') }} · {{ recurringView === 'pending' ? recurringSuggestions.length : recurringHistory.length }}
        </f7-block-title>
        <f7-list v-if="recurringView === 'pending' && recurringSuggestions.length" strong inset dividers media-list>
            <f7-list-item
                v-for="suggestion in recurringSuggestions"
                :key="suggestion.id"
                :title="suggestion.name || suggestion.counterparty"
                :subtitle="suggestion.counterparty || suggestion.description"
                :text="recurringDetails(suggestion)"
            >
                <template #media><f7-icon f7="calendar_badge_clock" /></template>
                <template #footer>
                    <div class="action-center-mobile-actions">
                        <f7-button
                            small fill
                            color="green"
                            :preloader="mutatingSuggestionId === suggestion.id"
                            :disabled="mutatingSuggestionId !== null"
                            @click="acceptRecurring(suggestion.id)"
                        >
                            {{ tt('Accept') }}
                        </f7-button>
                        <f7-button
                            small
                            :disabled="mutatingSuggestionId !== null"
                            @click="rejectRecurring(suggestion.id)"
                        >
                            {{ tt('Ignore') }}
                        </f7-button>
                    </div>
                </template>
            </f7-list-item>
        </f7-list>
        <f7-block v-else-if="recurringView === 'pending' && !loading" strong inset class="action-center-empty">
            {{ tt('No pending recurring suggestions') }}
        </f7-block>
        <f7-list v-if="recurringView === 'history' && recurringHistory.length" strong inset dividers media-list>
            <f7-list-item
                v-for="suggestion in recurringHistory"
                :key="suggestion.id"
                :title="suggestion.name || suggestion.counterparty"
                :subtitle="suggestion.counterparty || suggestion.description"
                :text="recurringDetails(suggestion)"
                :after="recurringStatusLabel(suggestion)"
            >
                <template #media><f7-icon f7="clock_arrow_circlepath" /></template>
            </f7-list-item>
        </f7-list>
        <f7-block v-else-if="recurringView === 'history' && !loading" strong inset class="action-center-empty">
            {{ tt('No recurring suggestion history') }}
        </f7-block>

        <f7-preloader v-if="loading" class="action-center-preloader" />
    </f7-page>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { Router } from 'framework7/types';

import { useI18n } from '@/locales/helpers.ts';
import type { RecurringSuggestion } from '@/models/recurring_suggestion.ts';
import {
    buildAnomalyActionTarget,
    type ActionCenterAnomaly,
    type ActionCenterAnomalyType
} from '@/views/base/action-center/actionCenterModel.ts';
import { useActionCenter } from '@/views/base/action-center/useActionCenter.ts';

const props = defineProps<{ f7router: Router.Router }>();
const { tt, formatAmountToLocalizedNumerals } = useI18n();
const showMonthSelection = ref(false);
const recurringView = ref<'pending' | 'history'>('pending');
const monthOptions = computed(() => [3, 6, 12].map(value => ({
    value,
    title: tt('Month count', { count: value })
})));
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

function anomalyTitle(type: ActionCenterAnomalyType): string {
    if (type === 'large_transaction') return 'Large Transaction';
    if (type === 'duplicate_charge') return 'Possible Duplicate';
    return 'Category Spike';
}

function anomalyIcon(type: ActionCenterAnomalyType): string {
    if (type === 'large_transaction') return 'exclamationmark_circle';
    if (type === 'duplicate_charge') return 'doc_on_doc';
    return 'chart_bar_alt_fill';
}

function anomalyContext(anomaly: ActionCenterAnomaly): string {
    return anomaly.counterparty || anomaly.description || anomaly.category || tt('Transaction');
}

function formatAmount(value: number): string {
    return formatAmountToLocalizedNumerals(value / 100);
}

function anomalyDetails(anomaly: ActionCenterAnomaly): string {
    return [anomaly.occurredOn, anomaly.amountCents ? formatAmount(anomaly.amountCents) : '']
        .filter(Boolean)
        .join(' · ');
}

function recurringDetails(suggestion: RecurringSuggestion): string {
    const amount = formatAmount(suggestion.amountCents);
    const confidence = `${tt('Confidence')} ${Math.round(suggestion.confidenceScore * 100)}%`;
    const nextDate = suggestion.suggestedNextDate
        ? `${tt('Next Date')}: ${suggestion.suggestedNextDate}`
        : '';
    return [amount, confidence, nextDate].filter(Boolean).join(' · ');
}

function recurringStatusLabel(suggestion: RecurringSuggestion): string {
    if (suggestion.status === 'accepted') return tt('Accepted');
    if (suggestion.status === 'rejected') return tt('Rejected');
    return '';
}

function reviewAnomaly(anomaly: ActionCenterAnomaly): void {
    props.f7router.navigate(buildAnomalyActionTarget(anomaly, 'mobile') as string);
}

async function refresh(done?: () => void): Promise<void> {
    await load();
    done?.();
}

watch(months, () => load());
</script>

<style scoped>
.action-center-error {
    color: var(--f7-color-red);
}

.action-center-notice {
    color: var(--f7-color-blue);
}

.action-center-controls :deep(.item-title) {
    flex-shrink: 0;
}

.action-center-controls :deep(.item-after) {
    min-width: 0;
}

.action-center-recurring-toolbar :deep(.segmented) {
    width: 100%;
}

.action-center-empty {
    overflow-wrap: anywhere;
}

.action-center-mobile-actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
}

.action-center-mobile-actions .button {
    flex: 1 1 0;
}

.action-center-preloader {
    display: block;
    margin: 24px auto;
}
</style>
