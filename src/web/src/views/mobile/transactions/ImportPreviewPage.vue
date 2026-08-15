<template>
    <f7-page :ptr="true" data-testid="mobile.import.preview.page" @ptr:refresh="reload">
        <f7-navbar>
            <f7-nav-left :back-link="tt('Back')"></f7-nav-left>
            <f7-nav-title :title="tt('Import Preview')"></f7-nav-title>
            <f7-nav-right>
                <f7-link icon-f7="slider_horizontal_3" @click="showFilterSheet = true"></f7-link>
            </f7-nav-right>
        </f7-navbar>

        <f7-list strong inset dividers media-list class="margin-vertical skeleton-text" v-if="loading">
            <f7-list-item :title="tt('Import Preview')" subtitle="Loading" after="0.00"></f7-list-item>
            <f7-list-item :title="tt('Signals')" subtitle="Parser / Learning / History Rewrite"></f7-list-item>
        </f7-list>

        <template v-else>
            <f7-block class="import-preview-summary" data-testid="mobile.import.preview.summary">
                <span>{{ tt('format.misc.selectedCount', { count: selectedRows.length, totalCount: filteredRows.length }) }}</span>
                <span class="history-summary" v-if="selectedHistoryRewriteCount > 0">
                    {{ tt('History Rewrite') }} {{ selectedHistoryRewriteCount }}
                </span>
            </f7-block>

            <f7-list strong inset dividers media-list class="margin-vertical" v-if="filteredRows.length > 0">
                <f7-list-item
                    v-for="row in filteredRows"
                    :key="row.id"
                    checkbox
                    :checked="row.selected"
                    :title="rowTitle(row)"
                    :subtitle="rowSubtitle(row)"
                    :after="formatAmount(row)"
                    @change="toggleRowSelection(row, $event)"
                >
                    <template #footer>
                        <div class="import-preview-footer">
                            <span
                                v-for="chip in signalChips(row)"
                                :key="`${row.id}-${chip.label}`"
                                :class="['signal-chip', `signal-chip--${chip.tone}`]"
                                @click.stop="chip.historyBillId && openHistoryBillDetail(chip.historyBillId)"
                            >
                                {{ tt(chip.label) }}
                            </span>
                            <f7-link class="row-action-link" @click.stop="openRowSheet(row)">{{ tt('Details') }}</f7-link>
                        </div>
                    </template>
                </f7-list-item>
            </f7-list>
            <f7-list strong inset dividers class="margin-vertical" v-else>
                <f7-list-item :title="tt('No data to import')"></f7-list-item>
            </f7-list>
        </template>

        <f7-toolbar bottom>
            <f7-link @click="selectAllVisible">{{ tt('Select All') }}</f7-link>
            <f7-link @click="selectNoneVisible">{{ tt('Select None') }}</f7-link>
            <f7-link data-testid="mobile.import.preview.action.confirm" :class="{ disabled: selectedRows.length < 1 || confirming }" @click="confirmSelected">
                {{ tt('Import') }}
            </f7-link>
        </f7-toolbar>

        <f7-sheet
            class="import-preview-filter-sheet"
            :opened="showFilterSheet"
            swipe-to-close
            @sheet:closed="showFilterSheet = false"
        >
            <f7-page-content>
                <f7-block-title>{{ tt('Signals') }}</f7-block-title>
                <f7-list strong inset dividers class="margin-vertical">
                    <f7-list-item
                        radio
                        radio-icon="start"
                        :title="tt(option.title)"
                        :checked="signalFilter === option.value"
                        :key="String(option.value)"
                        v-for="option in signalFilterOptions"
                        @change="signalFilter = option.value"
                    ></f7-list-item>
                </f7-list>
            </f7-page-content>
        </f7-sheet>

        <f7-sheet
            class="import-preview-detail-sheet"
            :opened="!!detailRow"
            swipe-to-close
            @sheet:closed="detailRow = null"
        >
            <f7-page-content v-if="detailRow">
                <f7-block-title>{{ rowTitle(detailRow) }}</f7-block-title>
                <f7-list strong inset dividers class="margin-vertical">
                    <f7-list-item :title="tt('Counterparty')" :after="detailRow.record.preview_counterparty || '-'"></f7-list-item>
                    <f7-list-item :title="tt('Payment Method')" :after="detailRow.record.preview_payment_method || '-'"></f7-list-item>
                    <f7-list-item :title="tt('Description')" :footer="detailRow.record.preview_description || '-'"></f7-list-item>
                    <f7-list-item
                        v-for="line in detailLines(detailRow)"
                        :key="`${detailRow.id}-${line}`"
                        :title="line"
                    ></f7-list-item>
                </f7-list>

                <f7-block class="grid grid-cols-2 grid-gap" v-if="detailRow.signal.transferSuggestion?.actions.length">
                    <f7-button
                        outline
                        :preloader="detailRow.busy"
                        :loading="detailRow.busy"
                        v-for="action in detailRow.signal.transferSuggestion.actions"
                        :key="`transfer-${action.decision}`"
                        @click="reviewTransfer(detailRow, action.decision)"
                    >
                        {{ tt(action.labelKey) }}
                    </f7-button>
                </f7-block>
                <f7-block class="grid grid-cols-2 grid-gap" v-if="detailRow.signal.learning?.actions.length">
                    <f7-button
                        outline
                        :preloader="detailRow.busy"
                        :loading="detailRow.busy"
                        v-for="action in detailRow.signal.learning.actions"
                        :key="`learning-${action.decision}`"
                        @click="reviewLearning(detailRow, action.decision)"
                    >
                        {{ tt(action.labelKey) }}
                    </f7-button>
                </f7-block>
                <f7-block class="grid grid-cols-2 grid-gap" v-if="detailRow.signal.llm?.actions.length">
                    <f7-button
                        outline
                        :preloader="detailRow.busy"
                        :loading="detailRow.busy"
                        v-for="action in detailRow.signal.llm.actions"
                        :key="`llm-${action.decision}`"
                        @click="reviewLlm(detailRow, action.decision)"
                    >
                        {{ tt(action.labelKey) }}
                    </f7-button>
                </f7-block>
            </f7-page-content>
        </f7-sheet>
    </f7-page>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import type { Router } from 'framework7/types';

import { useI18n } from '@/locales/helpers.ts';
import { hideLoading, showLoading, useI18nUIComponents } from '@/lib/ui/mobile.ts';
import services from '@/lib/services.ts';
import {
    type ImportPreviewRecord
} from '@/views/desktop/transactions/import/importPreview.ts';
import {
    buildImportPreviewHistoryRewriteAcknowledgement,
    buildImportPreviewHistoryRewriteOperationAcknowledgement,
    buildImportPreviewSignalViewModel,
    matchesImportPreviewSignalFilter,
    type ImportPreviewHistoryRewriteAcknowledgement,
    type ImportPreviewHistoryRewriteAcknowledgementOperation,
    type ImportPreviewSignalDecision,
    type ImportPreviewSignalViewModel,
    type ImportPreviewVisibleSignalFilterValue
} from '@/views/desktop/transactions/import/checkDataMatching.ts';

const props = defineProps<{
    f7route: Router.Route;
    f7router: Router.Router;
}>();

type MobileSignalFilter = ImportPreviewVisibleSignalFilterValue | null;

interface MobileImportPreviewRow {
    id: number;
    record: ImportPreviewRecord;
    selected: boolean;
    signal: ImportPreviewSignalViewModel;
    busy: boolean;
}

const { tt, formatAmountToLocalizedNumeralsWithCurrency } = useI18n();
const { showAlert, showConfirm, showToast, routeBackOnError } = useI18nUIComponents();

const sessionId = String(props.f7route.query['sessionId'] || props.f7route.query['session_id'] || '').trim();
const loading = ref<boolean>(false);
const confirming = ref<boolean>(false);
const loadingError = ref<unknown | null>(null);
const rows = ref<MobileImportPreviewRow[]>([]);
const signalFilter = ref<MobileSignalFilter>(null);
const showFilterSheet = ref<boolean>(false);
const detailRow = ref<MobileImportPreviewRow | null>(null);

const signalFilterOptions: Array<{ title: string; value: MobileSignalFilter }> = [
    { title: 'All', value: null },
    { title: 'Parser', value: 'parser' },
    { title: 'Platform Duplicate', value: 'platform_duplicate' },
    { title: 'Transfer Match', value: 'transfer' },
    { title: 'History Rewrite', value: 'history' },
    { title: 'Learning Suggestion', value: 'learning' },
    { title: 'LLM Suggestion', value: 'llm' }
];

const filteredRows = computed<MobileImportPreviewRow[]>(() => rows.value.filter(row => (
    matchesImportPreviewSignalFilter(row.signal, signalFilter.value)
)));
const selectedRows = computed<MobileImportPreviewRow[]>(() => rows.value.filter(row => row.selected));
const selectedHistoryRewriteCount = computed<number>(() => selectedRows.value.filter(row => !!historyOperation(row)).length);

function buildSignal(record: ImportPreviewRecord): ImportPreviewSignalViewModel {
    const matching = record.matching;
    const llm = matching?.llm || {};
    return buildImportPreviewSignalViewModel({
        parserId: record.preview_parser_id || matching?.parser?.id,
        parserTags: record.preview_parser_tags || matching?.parser?.tags,
        dedupType: record.dedup_type || matching?.dedup?.type,
        dedupSourceIds: Array.isArray(record.dedup_source_ids) ? record.dedup_source_ids : matching?.dedup?.source_ids,
        dedupSourceCount: matching?.dedup?.source_count,
        dedupSourceLabels: matching?.dedup?.source_labels,
        dedupSources: matching?.dedup?.sources,
        parserIdChain: matching?.parser?.source_chain,
        reconciliationType: matching?.reconciliation?.candidate_type,
        reconciliationStatus: matching?.reconciliation?.status,
        reconciliationTitle: matching?.reconciliation?.signal_label,
        reconciliationSourceChain: matching?.reconciliation?.source_chain,
        reconciliationPlannedOperation: matching?.reconciliation?.planned_operation,
        reconciliationHistoryBillId: matching?.reconciliation?.history_bill_id,
        reconciliationHistoryBillVersion: matching?.reconciliation?.history_bill_version,
        reconciliationHistorySummary: matching?.reconciliation?.history_summary,
        reconciliationOperationId: matching?.reconciliation?.operation_id,
        reconciliationAcknowledgementToken: matching?.reconciliation?.acknowledgement_token,
        reconciliationDestructiveAckRequired: !!matching?.reconciliation?.destructive_ack_required,
        reconciliationNotice: matching?.reconciliation?.notice || matching?.annotation?.history_rewrite_notice,
        transferStatus: normalizeSignalStatus(matching?.transfer?.review_status, record.transfer_suggestion_reason),
        transferTitle: record.transfer_suggestion_reason || matching?.transfer?.reason,
        transferCandidateType: matching?.transfer?.candidate_type,
        transferLearningLevel: matching?.transfer?.learning_level || matching?.transfer?.level || record.transfer_suggestion_level,
        transferSuppressed: matching?.transfer?.suppressed,
        transferPairOrder: matching?.transfer?.pair_order,
        transferSourceChain: matching?.transfer?.source_chain,
        learningStatus: normalizeSignalStatus(matching?.learning?.review_status, record.learning_recommendation_reason),
        learningTitle: record.learning_recommendation_reason || matching?.learning?.reason,
        learningSummary: record.learning_recommendation_summary || matching?.learning?.summary,
        learningMode: matching?.learning?.mode || '',
        learningSignalState: matching?.learning?.signal_state || '',
        learningAutoApplied: !!matching?.learning?.auto_apply,
        llmStatus: normalizeSignalStatus(llm.review_status, llm.reason),
        llmTitle: llm.reason || '',
        llmSummary: '',
        llmConfidence: Number(llm.confidence || 0),
        llmCategoryPath: [llm.suggested_main_category || '', llm.suggested_sub_category || ''].filter(Boolean).join('/'),
        llmSourceAccount: llm.suggested_source_account || '',
        llmDestinationAccount: llm.suggested_destination_account || '',
        recurringCandidateCount: Number(record.preview_recurring_candidate_count || matching?.recurring?.candidate_count || 0),
        recurringTitle: record.preview_recurring_name || matching?.recurring?.name || '',
        recurringPrimaryReason: record.preview_recurring_match_reasons || matching?.recurring?.match_reasons || ''
    }, {
        formatAmountWithCurrency: formatAmountToLocalizedNumeralsWithCurrency
    });
}

function normalizeSignalStatus(status: unknown, fallbackText: unknown): 'pending' | 'accepted' | 'rejected' | 'skipped' | null {
    const normalizedStatus = String(status || '').trim().toLowerCase();
    if (normalizedStatus === 'accepted' || normalizedStatus === 'rejected' || normalizedStatus === 'skipped') {
        return normalizedStatus;
    }
    if (normalizedStatus === 'pending' || String(fallbackText || '').trim()) {
        return 'pending';
    }
    return null;
}

function normalizeRows(preview: ImportPreviewRecord[]): MobileImportPreviewRow[] {
    return preview.map(record => ({
        id: Number(record.id),
        record,
        selected: !!(record.preview_selected ?? record.selected),
        signal: buildSignal(record),
        busy: false
    })).filter(row => Number.isFinite(row.id) && row.id > 0);
}

async function reload(done?: () => void): Promise<void> {
    if (!sessionId) {
        showAlert('Import session not found');
        props.f7router.back();
        done?.();
        return;
    }

    loading.value = true;
    showLoading(() => loading.value);
    try {
        const response = await services.getImportPreviewPage({
            sessionId,
            page: 1,
            pageSize: 500
        });
        rows.value = normalizeRows(((response.data.result as Record<string, unknown>)['preview'] || []) as ImportPreviewRecord[]);
    } catch (error) {
        loadingError.value = error;
        showToast(error instanceof Error && error.message ? error.message : 'Failed to load import preview');
    } finally {
        loading.value = false;
        hideLoading();
        done?.();
    }
}

function rowTitle(row: MobileImportPreviewRow): string {
    return row.record.preview_counterparty || row.record.preview_payment_method || tt('Imported Transaction');
}

function rowSubtitle(row: MobileImportPreviewRow): string {
    return [
        row.record.preview_date || '',
        row.record.preview_main_category || '',
        row.record.preview_sub_category || '',
        row.record.preview_description || ''
    ].filter(Boolean).join(' · ');
}

function formatAmount(row: MobileImportPreviewRow): string {
    const amount = Number(row.record.preview_amount_cents || 0) / 100;
    return Number.isFinite(amount) ? amount.toFixed(2) : '0.00';
}

function signalChips(row: MobileImportPreviewRow): Array<{ label: string; tone: string; historyBillId?: number }> {
    const chips: Array<{ label: string; tone: string; historyBillId?: number }> = [];
    if (row.signal.parser) chips.push({ label: 'Parser', tone: 'neutral' });
    if (row.signal.dedup) chips.push({ label: row.signal.dedup.labelKey, tone: 'neutral' });
    if (row.signal.historyRewrite) chips.push({
        label: row.signal.historyRewrite.labelKey,
        tone: 'warning',
        historyBillId: row.signal.historyRewrite.historyBillId
    });
    if (row.signal.transferSuggestion) chips.push({ label: row.signal.transferSuggestion.labelKey, tone: 'info' });
    if (row.signal.learning) chips.push({ label: row.signal.learning.labelKey, tone: row.signal.learning.color === 'success' ? 'success' : 'warning' });
    if (row.signal.llm) chips.push({ label: row.signal.llm.labelKey, tone: 'warning' });
    return chips;
}

function openHistoryBillDetail(historyBillId: number): void {
    if (historyBillId > 0) {
        props.f7router.navigate(`/transaction/detail?id=${encodeURIComponent(String(historyBillId))}`);
    }
}

function detailLines(row: MobileImportPreviewRow): string[] {
    return [
        ...(row.signal.historyRewrite?.detailLines || []),
        ...(row.signal.transferSuggestion?.detailLines || []),
        ...(row.signal.learning?.detailLines || []),
        ...(row.signal.llm?.detailLines || []),
        ...(row.signal.dedup?.detailLines || [])
    ];
}

function toggleRowSelection(row: MobileImportPreviewRow, event: Event): void {
    row.selected = !!(event.target as HTMLInputElement | null)?.checked;
}

function selectAllVisible(): void {
    filteredRows.value.forEach(row => row.selected = true);
}

function selectNoneVisible(): void {
    filteredRows.value.forEach(row => row.selected = false);
}

function openRowSheet(row: MobileImportPreviewRow): void {
    detailRow.value = row;
}

function historyOperation(row: MobileImportPreviewRow): ImportPreviewHistoryRewriteAcknowledgementOperation | null {
    const reconciliation = row.record.matching?.reconciliation;
    return buildImportPreviewHistoryRewriteOperationAcknowledgement(row.id, {
        reconciliationPlannedOperation: reconciliation?.planned_operation,
        reconciliationHistoryBillId: reconciliation?.history_bill_id,
        reconciliationHistoryBillVersion: reconciliation?.history_bill_version,
        reconciliationOperationId: reconciliation?.operation_id,
        reconciliationAcknowledgementToken: reconciliation?.acknowledgement_token,
        reconciliationDestructiveAckRequired: !!reconciliation?.destructive_ack_required
    });
}

function buildPreviewUpdates(): Record<string, unknown>[] {
    return rows.value.map(row => ({
        id: row.id,
        selected: row.selected
    }));
}

function buildHistoryAcknowledgement(): ImportPreviewHistoryRewriteAcknowledgement | null {
    return buildImportPreviewHistoryRewriteAcknowledgement({
        selectedPreviewIds: selectedRows.value.map(row => row.id),
        operations: selectedRows.value.map(row => historyOperation(row)).filter((operation): operation is ImportPreviewHistoryRewriteAcknowledgementOperation => !!operation),
        selectionScope: {
            mode: 'mobile-visible-preview',
            selected_visible_count: selectedRows.value.length
        }
    });
}

function confirmSelected(): void {
    if (selectedRows.value.length < 1 || confirming.value) {
        return;
    }

    const message = selectedHistoryRewriteCount.value > 0
        ? 'History Rewrite'
        : 'format.misc.confirmImportTransactions';
    showConfirm(message, () => {
        void confirmSelectedNow();
    });
}

async function confirmSelectedNow(): Promise<void> {
    confirming.value = true;
    try {
        await services.confirmImportPreview({
            sessionId,
            previewUpdates: buildPreviewUpdates(),
            preserveUnpatchedSelection: true,
            historyRewriteAcknowledgement: buildHistoryAcknowledgement()
        });
        showToast('Import completed');
        props.f7router.back();
    } catch (error) {
        showToast(error instanceof Error && error.message ? error.message : 'Failed to confirm import preview');
    } finally {
        confirming.value = false;
    }
}

async function reviewTransfer(row: MobileImportPreviewRow, decision: ImportPreviewSignalDecision['decision']): Promise<void> {
    row.busy = true;
    try {
        await services.reviewImportTransferDecision({
            previewId: row.id,
            decision,
            payload: { sessionId }
        });
        await reload();
    } finally {
        row.busy = false;
    }
}

async function reviewLearning(row: MobileImportPreviewRow, decision: ImportPreviewSignalDecision['decision']): Promise<void> {
    row.busy = true;
    const candidateId = `preview:${row.id}:learning`;
    try {
        if (decision === 'accept') {
            await services.acceptMatchingCandidate({ candidateId, payload: { sessionId, responseMode: 'preview-item' } });
        } else if (decision === 'reject') {
            await services.rejectMatchingCandidate({ candidateId, payload: { sessionId, responseMode: 'preview-item' } });
        } else {
            await services.clearMatchingCandidate({ candidateId, payload: { sessionId, responseMode: 'preview-item' } });
        }
        await reload();
    } catch (error) {
        showToast(error instanceof Error && error.message ? error.message : 'Learning decision failed');
        await reload();
    } finally {
        row.busy = false;
    }
}

async function reviewLlm(row: MobileImportPreviewRow, decision: ImportPreviewSignalDecision['decision']): Promise<void> {
    if (decision === 'clear') {
        return;
    }

    row.busy = true;
    const suggestion = row.record.matching?.llm || {};
    try {
        if (decision === 'accept') {
            await services.llmPreviewRecommendAccept({ sessionId, previewId: row.id, suggestion });
        } else {
            await services.llmPreviewRecommendReject({ sessionId, previewId: row.id, suggestion });
        }
        await reload();
    } catch (error) {
        showToast(error instanceof Error && error.message ? error.message : 'LLM recommendation decision failed');
        await reload();
    } finally {
        row.busy = false;
    }
}

onMounted(() => {
    routeBackOnError(props.f7router, loadingError);
    void reload();
});
</script>

<style scoped src="./import-preview/ImportPreviewPage.scss"></style>
