<template>
    <v-card variant="flat" class="pa-3 bill-matching-card">
        <div class="d-flex justify-space-between align-center flex-wrap ga-3">
            <div class="flex-grow-1 min-w-0">
                <div class="text-caption text-medium-emphasis mb-1">{{ tt('Historical Matching') }}</div>
                <div class="text-body-1 font-weight-medium text-truncate">{{ headlineText }}</div>
                <div class="text-caption text-medium-emphasis mt-1" v-if="sublineText">
                    {{ sublineText }}
                </div>
            </div>
            <div class="d-flex flex-wrap ga-2 align-center">
                <v-progress-circular v-if="loading" indeterminate size="20" />
                <v-btn size="small" color="warning" variant="tonal"
                       v-if="viewState.showDeletePairAction && linkedPair"
                       :disabled="isBusy"
                       @click="confirmDeletePair">
                    {{ tt('Clear Pair') }}
                </v-btn>
            </div>
        </div>

        <v-list class="rounded border-sm mt-3" v-if="sortedCandidates.length">
            <v-list-item v-for="candidate in sortedCandidates"
                         :key="candidate.candidateId"
                         class="bill-matching-candidate-item">
                <template #prepend>
                    <v-chip size="small" :color="getCandidateChipColor(candidate.kind)" variant="tonal">
                        {{ getCandidateKindLabel(candidate.kind) }}
                    </v-chip>
                </template>
                <template #title>
                    <div class="d-flex align-center flex-wrap ga-2">
                        <span>{{ getCandidateTitle(candidate) }}</span>
                        <v-chip size="x-small" variant="outlined">
                            {{ tt('Match Score') }} {{ formatScore(candidate.score) }}
                        </v-chip>
                        <v-chip size="x-small" variant="outlined" v-if="candidate.level">
                            {{ candidate.level }}
                        </v-chip>
                    </div>
                </template>
                <template #subtitle>
                    <div class="mt-1">{{ getCandidateSubtitle(candidate) }}</div>
                    <div class="text-caption text-medium-emphasis mt-1" v-if="getCandidateMeta(candidate)">
                        {{ getCandidateMeta(candidate) }}
                    </div>
                </template>
                <template #append>
                    <div v-if="isBillMatchingCandidateReviewActionSupported(candidate)"
                         class="d-flex flex-wrap ga-2 align-center justify-end">
                        <v-progress-circular v-if="actionCandidateId === candidate.candidateId" indeterminate size="18" />
                        <v-btn size="small" color="primary" variant="outlined"
                               :disabled="isBusy"
                               @click="handleCandidateAction('accept', candidate)">
                            {{ tt('Accept') }}
                        </v-btn>
                        <v-btn size="small" color="warning" variant="tonal"
                               :disabled="isBusy"
                               @click="handleCandidateAction('reject', candidate)">
                            {{ tt('Reject') }}
                        </v-btn>
                    </div>
                </template>
            </v-list-item>
        </v-list>

        <div class="text-body-2 text-medium-emphasis mt-3" v-else-if="!loading && !viewState.hasLinkedPair">
            {{ tt('No Historical Matching Candidates') }}
        </div>

        <template v-if="feedbackResponse.events.length">
            <v-divider class="my-3" />
            <div class="text-caption text-medium-emphasis mb-2">{{ tt('Matching Feedback') }}</div>
            <v-list class="rounded border-sm">
                <v-list-item v-for="event in feedbackResponse.events"
                             :key="`feedback-${event.id}`">
                    <template #prepend>
                        <v-chip size="x-small" :color="getFeedbackActionColor(event.action)" variant="tonal">
                            {{ event.action }}
                        </v-chip>
                    </template>
                    <template #title>
                        <div class="d-flex align-center flex-wrap ga-2">
                            <span>{{ getFeedbackTitle(event) }}</span>
                            <span class="text-caption text-medium-emphasis" v-if="event.createdAt">{{ event.createdAt }}</span>
                        </div>
                    </template>
                    <template #subtitle>
                        <div class="mt-1 text-medium-emphasis">{{ getFeedbackSubtitle(event) }}</div>
                    </template>
                </v-list-item>
            </v-list>
        </template>
    </v-card>

    <confirm-dialog ref="confirmDialog" />
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';

import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import { useI18n } from '@/locales/helpers.ts';
import services from '@/lib/services.ts';
import { useAccountsStore } from '@/stores/account.ts';
import {
    buildBillMatchingViewState,
    getBillMatchingCandidateBillAmountCents,
    getBillMatchingCandidateBillSubtitleParts,
    getBillMatchingCandidateBillTitle,
    isBillMatchingCandidateReviewActionSupported,
    normalizeBillMatchingCandidatesResponse,
    normalizeBillMatchingFeedbackResponse,
    type BillMatchingCandidate,
    type BillMatchingCandidatesResponse,
    type BillMatchingFeedbackEvent,
    type BillMatchingFeedbackResponse,
    type BillMatchingPairSummary
} from '@/models/bill_matching.ts';

const props = defineProps<{
    billId: string | number;
    disabled?: boolean;
}>();

const emit = defineEmits<{
    notify: [message: string];
    error: [message: string];
    updated: [];
}>();

const { tt, formatAmountToLocalizedNumeralsWithCurrency } = useI18n();
const accountsStore = useAccountsStore();

const confirmDialog = ref<InstanceType<typeof ConfirmDialog> | null>(null);
const loading = ref<boolean>(false);
const actionCandidateId = ref<string>('');
const deletingPairId = ref<string>('');
const candidatesResponse = ref<BillMatchingCandidatesResponse>(
    normalizeBillMatchingCandidatesResponse({ billId: 0, linkedPair: null, candidates: [] })
);
const feedbackResponse = ref<BillMatchingFeedbackResponse>(
    normalizeBillMatchingFeedbackResponse({ billId: 0, events: [] })
);
let latestRefreshRequestId = 0;

const normalizedBillId = computed<number>(() => {
    const billId = Number(props.billId);
    return Number.isFinite(billId) && billId > 0 ? billId : 0;
});

const viewState = computed(() => buildBillMatchingViewState(candidatesResponse.value));
const linkedPair = computed<BillMatchingPairSummary | null>(() => candidatesResponse.value.linkedPair);
const reconciliationSignal = computed<string>(() => {
    const signal = candidatesResponse.value.reconciliation?.['signal_label'];
    return typeof signal === 'string' ? signal : '';
});
const sortedCandidates = computed<BillMatchingCandidate[]>(() => {
    return [...candidatesResponse.value.candidates].sort((left, right) => right.score - left.score);
});
const allAccountsMap = computed(() => accountsStore.allAccountsMap);
const isBusy = computed<boolean>(() => {
    return !!props.disabled || loading.value || !!actionCandidateId.value || !!deletingPairId.value;
});
const headlineText = computed<string>(() => {
    if (viewState.value.mode === 'linked' && linkedPair.value) {
        return `${tt('Linked Pair')} · ${getPairTypeLabel(linkedPair.value.pairType)} · #${linkedPair.value.otherBillId}`;
    }

    if (reconciliationSignal.value && !viewState.value.hasCandidates) {
        return reconciliationSignal.value;
    }

    if (viewState.value.mode === 'candidates') {
        return `${tt('Matching Candidates')} ${viewState.value.candidateCount}`;
    }

    return tt('No Historical Matching Candidates');
});
const sublineText = computed<string>(() => {
    if (viewState.value.mode === 'linked' && linkedPair.value) {
        return `${tt('Pair Source')}: ${linkedPair.value.source}`;
    }

    if (viewState.value.primaryCandidate) {
        return `${tt('Best Candidate')}: ${getCandidateKindLabel(viewState.value.primaryCandidate.kind)} · ${tt('Match Score')} ${formatScore(viewState.value.primaryCandidate.score)}`;
    }

    if (reconciliationSignal.value) {
        return reconciliationSignal.value;
    }

    return '';
});

function getErrorMessage(error: unknown, fallbackMessage: string): string {
    if (error instanceof Error && error.message) {
        return error.message;
    }

    if (typeof error === 'string' && error) {
        return error;
    }

    if (error && typeof error === 'object') {
        const responseError = (error as { response?: { data?: { error?: string } } }).response?.data?.error;
        if (typeof responseError === 'string' && responseError) {
            return responseError;
        }
    }

    return fallbackMessage;
}

function getPairTypeLabel(pairType: string): string {
    if (pairType === 'transfer') {
        return tt('Transfer');
    }
    if (pairType === 'investment') {
        return tt('Investment');
    }

    return pairType;
}

function getCandidateKindLabel(kind: string): string {
    if (kind === 'duplicate' || kind === 'reconciliation_duplicate') {
        return tt('Duplicate');
    }
    if (kind === 'reconciliation_transfer') {
        return tt('Transfer');
    }
    if (kind === 'transfer') {
        return tt('Transfer');
    }
    if (kind === 'investment') {
        return tt('Investment');
    }
    if (kind === 'learning') {
        return tt('Learning Suggestion');
    }

    return kind;
}

function getCandidateChipColor(kind: string): string {
    if (kind === 'duplicate' || kind === 'reconciliation_duplicate') {
        return 'secondary';
    }
    if (kind === 'reconciliation_transfer') {
        return 'primary';
    }
    if (kind === 'transfer') {
        return 'primary';
    }
    if (kind === 'investment') {
        return 'success';
    }
    if (kind === 'learning') {
        return 'info';
    }

    return 'default';
}

function formatScore(score: number): string {
    return Number(score || 0).toFixed(2);
}

function getCandidateTitle(candidate: BillMatchingCandidate): string {
    const billTitle = getBillMatchingCandidateBillTitle(candidate);

    if (billTitle) {
        return billTitle;
    }

    if (candidate.summary) {
        return candidate.summary;
    }
    if (candidate.reason) {
        return candidate.reason;
    }

    return `#${candidate.billId || candidate.ruleId || candidate.candidateId}`;
}

function getCandidateSubtitle(candidate: BillMatchingCandidate): string {
    const billParts = getBillMatchingCandidateBillSubtitleParts(candidate);

    if (billParts.length) {
        return billParts.join(' · ');
    }

    return [candidate.recommendedType, candidate.reason].filter(Boolean).join(' · ');
}

function getCandidateMeta(candidate: BillMatchingCandidate): string {
    const parts: string[] = [];
    const billAmountCents = getBillMatchingCandidateBillAmountCents(candidate);

    if (billAmountCents !== null) {
        parts.push(formatAmountToLocalizedNumeralsWithCurrency(billAmountCents, false));
    }

    const accountText = getCandidateBillAccountText(candidate);

    if (accountText) {
        parts.push(accountText);
    }

    if (candidate.bill?.paymentMethod) {
        parts.push(candidate.bill.paymentMethod);
    }

    return parts.join(' · ');
}

function getCandidateBillAccountText(candidate: BillMatchingCandidate): string {
    const bill = candidate.bill;

    if (!bill) {
        return '';
    }

    const sourceAccountName = bill.sourceAccountId > 0 ? allAccountsMap.value[String(bill.sourceAccountId)]?.name || '' : '';
    const destinationAccountName = bill.destinationAccountId > 0 ? allAccountsMap.value[String(bill.destinationAccountId)]?.name || '' : '';

    if (sourceAccountName && destinationAccountName && sourceAccountName !== destinationAccountName) {
        return `${sourceAccountName} -> ${destinationAccountName}`;
    }

    return sourceAccountName || destinationAccountName;
}

function getFeedbackActionColor(action: string): string {
    if (action === 'accept') {
        return 'success';
    }
    if (action === 'reject') {
        return 'warning';
    }

    return 'default';
}

function getFeedbackPairType(event: BillMatchingFeedbackEvent): string {
    const pair = event.payload['pair'];
    if (pair && typeof pair === 'object' && !Array.isArray(pair)) {
        const pairType = (pair as Record<string, unknown>)['pairType'];
        if (typeof pairType === 'string' && pairType) {
            return pairType;
        }
    }

    return '';
}

function getFeedbackTitle(event: BillMatchingFeedbackEvent): string {
    const pairType = getFeedbackPairType(event);

    if (pairType) {
        return `${getPairTypeLabel(pairType)} · ${event.action}`;
    }

    return event.action;
}

function getFeedbackSubtitle(event: BillMatchingFeedbackEvent): string {
    return event.candidateId || '';
}

async function refreshMatchingData(): Promise<void> {
    latestRefreshRequestId += 1;
    const requestId = latestRefreshRequestId;

    if (!normalizedBillId.value) {
        candidatesResponse.value = normalizeBillMatchingCandidatesResponse({ billId: 0, linkedPair: null, candidates: [] });
        feedbackResponse.value = normalizeBillMatchingFeedbackResponse({ billId: 0, events: [] });
        return;
    }

    loading.value = true;

    const [candidateResult, feedbackResult] = await Promise.allSettled([
        services.getMatchingBillCandidates({ billId: normalizedBillId.value }),
        services.getMatchingBillFeedback({ billId: normalizedBillId.value })
    ]);

    if (requestId !== latestRefreshRequestId) {
        return;
    }

    if (candidateResult.status === 'fulfilled') {
        candidatesResponse.value = normalizeBillMatchingCandidatesResponse(candidateResult.value.data.result);
    } else {
        candidatesResponse.value = normalizeBillMatchingCandidatesResponse({
            billId: normalizedBillId.value,
            linkedPair: null,
            candidates: []
        });
        emit('error', getErrorMessage(candidateResult.reason, tt('Load Historical Matching Failed')));
    }

    if (feedbackResult.status === 'fulfilled') {
        feedbackResponse.value = normalizeBillMatchingFeedbackResponse(feedbackResult.value.data.result);
    } else {
        feedbackResponse.value = normalizeBillMatchingFeedbackResponse({ billId: normalizedBillId.value, events: [] });
        emit('error', getErrorMessage(feedbackResult.reason, tt('Load Historical Matching Failed')));
    }

    loading.value = false;
}

async function handleCandidateAction(action: 'accept' | 'reject', candidate: BillMatchingCandidate): Promise<void> {
    if (!candidate.candidateId) {
        return;
    }

    actionCandidateId.value = candidate.candidateId;

    try {
        if (action === 'accept') {
            const response = await services.acceptMatchingCandidate({ candidateId: candidate.candidateId });
            emit('notify', tt('Matching Candidate Accepted'));

            if (response.data.result.bill) {
                emit('updated');
            }
        } else {
            await services.rejectMatchingCandidate({ candidateId: candidate.candidateId });
            emit('notify', tt('Matching Candidate Rejected'));
        }

        await refreshMatchingData();
    } catch (error) {
        emit('error', getErrorMessage(error, tt('Matching Candidate Action Failed')));
    } finally {
        actionCandidateId.value = '';
    }
}

async function deletePair(): Promise<void> {
    if (!linkedPair.value) {
        return;
    }

    deletingPairId.value = String(linkedPair.value.id);

    try {
        await services.deleteMatchingPair({ pairId: linkedPair.value.id });
        emit('notify', tt('Matching Pair Cleared'));
        await refreshMatchingData();
    } catch (error) {
        emit('error', getErrorMessage(error, tt('Delete Pair Failed')));
    } finally {
        deletingPairId.value = '';
    }
}

function confirmDeletePair(): void {
    confirmDialog.value?.open(tt('Clear Pair Confirmation')).then(() => {
        void deletePair();
    }).catch(() => {
        // 用户取消时不处理
    });
}

watch(normalizedBillId, () => {
    void refreshMatchingData();
}, { immediate: true });
</script>

<style scoped>
.bill-matching-card {
    background-color: rgba(var(--v-theme-on-surface), 0.08) !important;
    border: 1px solid rgba(var(--v-theme-on-surface), 0.10);
    border-radius: 8px;
}

.bill-matching-card :deep(.v-list-item__append) {
    align-self: center;
}

.bill-matching-candidate-item :deep(.v-list-item__append) {
    max-width: 240px;
}
</style>
