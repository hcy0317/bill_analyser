<template>
    <div class="import-preview-signal-cell d-flex flex-column ga-1" v-if="viewModel?.hasAnySignal">
        <div class="d-flex flex-wrap align-center ga-1">
            <v-chip
                v-if="viewModel.parser"
                size="x-small"
                variant="outlined"
                :color="viewModel.parser.color"
                :title="viewModel.parser.title">
                {{ viewModel.parser.label }}
            </v-chip>
            <v-chip
                v-if="viewModel.dedup"
                :color="viewModel.dedup.color"
                variant="outlined"
                size="x-small"
                :title="viewModel.dedup.title">
                {{ tt(viewModel.dedup.labelKey) }} · {{ viewModel.dedup.sourceCount }}
            </v-chip>
        </div>

        <div class="signal-group" v-if="viewModel.transferSuggestion">
            <v-chip
                :color="viewModel.transferSuggestion.color"
                variant="tonal"
                size="x-small"
                :prepend-icon="getStatusIcon(viewModel.transferSuggestion.status)"
                :title="viewModel.transferSuggestion.title">
                {{ tt(viewModel.transferSuggestion.labelKey) }}
            </v-chip>
            <div class="d-inline-flex flex-wrap ga-1 ms-1">
                <v-btn
                    v-for="action in viewModel.transferSuggestion.actions"
                    :key="`transfer-${action.decision}`"
                    variant="text"
                    :color="action.color"
                    size="x-small"
                    :disabled="disabled"
                    @click.stop="emit('reviewTransfer', action.decision)">
                    {{ tt(action.labelKey) }}
                </v-btn>
            </div>
        </div>

        <div class="signal-group investment-signal-group" v-if="viewModel.investment">
            <div class="investment-signal-stack">
                <v-chip
                    class="investment-signal-chip"
                    :color="viewModel.investment.color"
                    variant="tonal"
                    size="x-small"
                    :prepend-icon="getInvestmentIcon(viewModel.investment.status)"
                    :title="viewModel.investment.title">
                    {{ tt(viewModel.investment.labelKey) }}
                </v-chip>
            </div>
        </div>

        <div class="signal-group" v-if="viewModel.learning">
            <v-chip
                :color="viewModel.learning.color"
                variant="tonal"
                size="x-small"
                :prepend-icon="getLearningIcon(viewModel.learning.status)"
                :title="viewModel.learning.title">
                {{ tt(viewModel.learning.labelKey) }}
            </v-chip>
            <div class="d-inline-flex flex-wrap ga-1 ms-1">
                <v-btn
                    v-for="action in viewModel.learning.actions"
                    :key="`learning-${action.decision}`"
                    variant="text"
                    :color="action.color"
                    size="x-small"
                    :disabled="disabled"
                    @click.stop="emit('reviewLearning', action.decision)">
                    {{ tt(action.labelKey) }}
                </v-btn>
            </div>
            <div class="text-caption text-medium-emphasis ms-1" v-if="viewModel.learning.summary">
                {{ viewModel.learning.summary }}
            </div>
        </div>

        <div class="signal-group" v-if="viewModel.recurring">
            <v-chip
                v-if="viewModel.recurring.hasMatch"
                color="success"
                variant="tonal"
                size="x-small"
                :title="viewModel.recurring.title">
                {{ tt('Scheduled Match') }}
            </v-chip>
            <v-chip
                v-if="viewModel.recurring.candidateCount > 0"
                class="ms-1"
                color="info"
                variant="outlined"
                size="x-small">
                {{ tt('Scheduled Candidates') }} {{ viewModel.recurring.candidateCount }}
            </v-chip>
            <v-chip
                v-if="viewModel.recurring.candidateCount > 0 && viewModel.recurring.primaryReason"
                class="ms-1"
                color="amber"
                variant="tonal"
                size="x-small"
                :prepend-icon="mdiStar">
                {{ tt('Best Candidate') }} · {{ viewModel.recurring.primaryReason }}
            </v-chip>
            <v-btn
                class="ms-1"
                variant="text"
                color="success"
                size="x-small"
                :disabled="disabled || !hasSession"
                @click.stop="emit('openRecurring')">
                {{ tt('Choose Scheduled Match') }}
            </v-btn>
            <v-btn
                v-if="viewModel.recurring.hasMatch"
                class="ms-1"
                variant="text"
                color="warning"
                size="x-small"
                :disabled="disabled || !hasSession"
                @click.stop="emit('clearRecurring')">
                {{ tt('Clear') }}
            </v-btn>
        </div>
    </div>
</template>

<script setup lang="ts">
import { useI18n } from '@/locales/helpers.ts';
import type {
    ImportPreviewSignalStatus,
    ImportPreviewSignalViewModel
} from '../checkDataMatching.ts';
import {
    mdiAlertOutline,
    mdiChartLine,
    mdiCheck,
    mdiLightbulbOutline,
    mdiSchoolOutline,
    mdiStar
} from '@mdi/js';

defineProps<{
    viewModel?: ImportPreviewSignalViewModel;
    disabled?: boolean;
    hasSession?: boolean;
}>();

const emit = defineEmits<{
    (e: 'reviewTransfer', decision: 'accept' | 'reject' | 'clear'): void;
    (e: 'reviewLearning', decision: 'accept' | 'reject' | 'clear'): void;
    (e: 'openRecurring'): void;
    (e: 'clearRecurring'): void;
}>();

const { tt } = useI18n();

function getStatusIcon(status: ImportPreviewSignalStatus): string {
    if (status === 'accepted') {
        return mdiCheck;
    }

    if (status === 'rejected') {
        return mdiAlertOutline;
    }

    return mdiLightbulbOutline;
}

function getInvestmentIcon(status: ImportPreviewSignalStatus): string {
    if (status === 'pending') {
        return mdiChartLine;
    }

    return getStatusIcon(status);
}

function getLearningIcon(status: ImportPreviewSignalStatus): string {
    if (status === 'pending') {
        return mdiSchoolOutline;
    }

    return getStatusIcon(status);
}
</script>

<style scoped>
.investment-signal-stack {
    display: inline-flex;
    max-width: 100%;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
}

.investment-signal-chip {
    align-self: flex-start;
}

</style>
