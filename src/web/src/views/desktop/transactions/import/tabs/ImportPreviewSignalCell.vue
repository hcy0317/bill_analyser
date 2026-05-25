<template>
    <div class="import-preview-signal-cell d-flex flex-column ga-1" v-if="viewModel?.hasAnySignal">
        <div class="d-flex flex-wrap align-center ga-1">
            <template v-if="viewModel.parser">
                <v-menu
                    v-if="hasSignalDetails(viewModel.parser.detailLines)"
                    open-on-hover
                    location="bottom start"
                    :close-on-content-click="false">
                    <template #activator="{ props: menuProps }">
                        <v-chip
                            v-bind="menuProps"
                            size="x-small"
                            variant="outlined"
                            :color="viewModel.parser.color">
                            {{ viewModel.parser.label }}
                        </v-chip>
                    </template>
                    <v-card class="signal-detail-card" variant="outlined">
                        <v-card-text class="pa-2">
                            <div
                                v-for="line in viewModel.parser.detailLines"
                                :key="`parser-${line}`"
                                class="text-caption">
                                {{ line }}
                            </div>
                        </v-card-text>
                    </v-card>
                </v-menu>
                <v-chip
                    v-else
                    size="x-small"
                    variant="outlined"
                    :color="viewModel.parser.color">
                    {{ viewModel.parser.label }}
                </v-chip>
            </template>
            <template v-if="viewModel.dedup">
                <v-menu
                    v-if="hasSignalDetails(viewModel.dedup.detailLines)"
                    open-on-hover
                    location="bottom start"
                    :close-on-content-click="false">
                    <template #activator="{ props: menuProps }">
                        <v-chip
                            v-bind="menuProps"
                            :color="viewModel.dedup.color"
                            variant="outlined"
                            size="x-small">
                            {{ viewModel.dedup.label }}
                        </v-chip>
                    </template>
                    <v-card class="signal-detail-card" variant="outlined">
                        <v-card-text class="pa-2">
                            <div
                                v-for="line in viewModel.dedup.detailLines"
                                :key="`dedup-${line}`"
                                class="text-caption">
                                {{ line }}
                            </div>
                        </v-card-text>
                    </v-card>
                </v-menu>
                <v-chip
                    v-else
                    :color="viewModel.dedup.color"
                    variant="outlined"
                    size="x-small">
                    {{ viewModel.dedup.label }}
                </v-chip>
            </template>
        </div>

        <div class="signal-group" v-if="viewModel.transferSuggestion">
            <div class="signal-stack">
                <v-menu
                    v-if="hasSignalDetails(viewModel.transferSuggestion.detailLines)"
                    open-on-hover
                    location="bottom start"
                    :close-on-content-click="false">
                    <template #activator="{ props: menuProps }">
                        <v-chip
                            v-bind="menuProps"
                            class="signal-chip"
                            :color="viewModel.transferSuggestion.color"
                            variant="tonal"
                            size="x-small"
                            :prepend-icon="getStatusIcon(viewModel.transferSuggestion.status)">
                            {{ tt(viewModel.transferSuggestion.labelKey) }}
                        </v-chip>
                    </template>
                    <v-card class="signal-detail-card" variant="outlined">
                        <v-card-text class="pa-2">
                            <div
                                v-for="line in viewModel.transferSuggestion.detailLines"
                                :key="`transfer-${line}`"
                                class="text-caption">
                                {{ line }}
                            </div>
                        </v-card-text>
                    </v-card>
                </v-menu>
                <v-chip
                    v-else
                    class="signal-chip"
                    :color="viewModel.transferSuggestion.color"
                    variant="tonal"
                    size="x-small"
                    :prepend-icon="getStatusIcon(viewModel.transferSuggestion.status)">
                    {{ tt(viewModel.transferSuggestion.labelKey) }}
                </v-chip>
                <div
                    v-if="viewModel.transferSuggestion.actions.length > 0"
                    :class="getActionRowClass(viewModel.transferSuggestion.actions.length)">
                    <v-btn
                        v-for="action in viewModel.transferSuggestion.actions"
                        :key="`transfer-${action.decision}`"
                        class="signal-action-btn"
                        variant="text"
                        :color="action.color"
                        size="x-small"
                        :loading="transferBusy"
                        :disabled="disabled || rowBusy"
                        @click.stop="emit('reviewTransfer', action.decision)">
                        <template #loader>
                            <v-progress-circular indeterminate :size="12" :width="2" class="signal-action-loader" />
                        </template>
                        {{ tt(action.labelKey) }}
                    </v-btn>
                </div>
            </div>
        </div>

        <div class="signal-group" v-if="viewModel.learning">
            <div class="signal-stack signal-stack--learning">
                <v-menu
                    v-if="hasSignalDetails(viewModel.learning.detailLines)"
                    open-on-hover
                    location="bottom start"
                    :close-on-content-click="false">
                    <template #activator="{ props: menuProps }">
                        <v-chip
                            v-bind="menuProps"
                            class="signal-chip"
                            :color="viewModel.learning.color"
                            variant="tonal"
                            size="x-small"
                            :prepend-icon="getLearningIcon(viewModel.learning.status)">
                            {{ tt(viewModel.learning.labelKey) }}
                        </v-chip>
                    </template>
                    <v-card class="signal-detail-card" variant="outlined">
                        <v-card-text class="pa-2">
                            <div
                                v-for="line in viewModel.learning.detailLines"
                                :key="`learning-${line}`"
                                class="text-caption">
                                {{ line }}
                            </div>
                        </v-card-text>
                    </v-card>
                </v-menu>
                <v-chip
                    v-else
                    class="signal-chip"
                    :color="viewModel.learning.color"
                    variant="tonal"
                    size="x-small"
                    :prepend-icon="getLearningIcon(viewModel.learning.status)">
                    {{ tt(viewModel.learning.labelKey) }}
                </v-chip>
                <div
                    v-if="viewModel.learning.actions.length > 0"
                    :class="getActionRowClass(viewModel.learning.actions.length)">
                    <v-btn
                        v-for="action in viewModel.learning.actions"
                        :key="`learning-${action.decision}`"
                        class="signal-action-btn"
                        variant="text"
                        :color="action.color"
                        size="x-small"
                        :loading="learningBusy"
                        :disabled="disabled || rowBusy"
                        @click.stop="emit('reviewLearning', action.decision)">
                        <template #loader>
                            <v-progress-circular indeterminate :size="12" :width="2" class="signal-action-loader" />
                        </template>
                        {{ tt(action.labelKey) }}
                    </v-btn>
                </div>
            </div>
        </div>

        <div class="signal-group" v-if="viewModel.llm">
            <div class="signal-stack signal-stack--learning">
                <v-menu
                    v-if="hasSignalDetails(viewModel.llm.detailLines)"
                    open-on-hover
                    location="bottom start"
                    :close-on-content-click="false">
                    <template #activator="{ props: menuProps }">
                        <v-chip
                            v-bind="menuProps"
                            class="signal-chip"
                            :color="viewModel.llm.color"
                            variant="tonal"
                            size="x-small"
                            :prepend-icon="getStatusIcon(viewModel.llm.status)">
                            {{ tt(viewModel.llm.labelKey) }}
                        </v-chip>
                    </template>
                    <v-card class="signal-detail-card" variant="outlined">
                        <v-card-text class="pa-2">
                            <div
                                v-for="line in viewModel.llm.detailLines"
                                :key="`llm-${line}`"
                                class="text-caption">
                                {{ line }}
                            </div>
                        </v-card-text>
                    </v-card>
                </v-menu>
                <v-chip
                    v-else
                    class="signal-chip"
                    :color="viewModel.llm.color"
                    variant="tonal"
                    size="x-small"
                    :prepend-icon="getStatusIcon(viewModel.llm.status)">
                    {{ tt(viewModel.llm.labelKey) }}
                </v-chip>
                <div
                    v-if="viewModel.llm.actions.length > 0"
                    :class="getActionRowClass(viewModel.llm.actions.length)">
                    <v-btn
                        v-for="action in viewModel.llm.actions"
                        :key="`llm-${action.decision}`"
                        class="signal-action-btn"
                        variant="text"
                        :color="action.color"
                        size="x-small"
                        :loading="llmBusy"
                        :disabled="disabled || rowBusy"
                        @click.stop="emitLLMReview(action.decision)">
                        <template #loader>
                            <v-progress-circular indeterminate :size="12" :width="2" class="signal-action-loader" />
                        </template>
                        {{ tt(action.labelKey) }}
                    </v-btn>
                </div>
            </div>
        </div>

        <div class="signal-group" v-if="viewModel.recurring">
            <v-chip
                v-if="viewModel.recurring.hasMatch"
                color="success"
                variant="tonal"
                size="x-small">
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
                :disabled="disabled || rowBusy || !hasSession"
                @click.stop="emit('openRecurring')">
                {{ tt('Choose Scheduled Match') }}
            </v-btn>
            <v-btn
                v-if="viewModel.recurring.hasMatch"
                class="ms-1"
                variant="text"
                color="warning"
                size="x-small"
                :disabled="disabled || rowBusy || !hasSession"
                @click.stop="emit('clearRecurring')">
                {{ tt('Clear') }}
            </v-btn>
        </div>
    </div>
</template>

<script setup lang="ts">
import { useI18n } from '@/locales/helpers.ts';
import {
    type ImportPreviewSignalStatus,
    type ImportPreviewSignalViewModel
} from '../checkDataMatching.ts';
import {
    mdiAlertOutline,
    mdiCheck,
    mdiLightbulbOutline,
    mdiSchoolOutline,
    mdiStar
} from '@mdi/js';

defineProps<{
    viewModel?: ImportPreviewSignalViewModel;
    disabled?: boolean;
    hasSession?: boolean;
    rowBusy?: boolean;
    transferBusy?: boolean;
    learningBusy?: boolean;
    llmBusy?: boolean;
}>();

const emit = defineEmits<{
    (e: 'reviewTransfer', decision: 'accept' | 'reject' | 'clear'): void;
    (e: 'reviewLearning', decision: 'accept' | 'reject' | 'clear'): void;
    (e: 'reviewLlm', decision: 'accept' | 'reject'): void;
    (e: 'openRecurring'): void;
    (e: 'clearRecurring'): void;
}>();

const { tt } = useI18n();

function hasSignalDetails(detailLines: string[] | undefined): boolean {
    return (detailLines || []).length > 0;
}

function getActionRowClass(actionCount: number): string[] {
    return [
        'signal-action-row',
        actionCount <= 1 ? 'signal-action-row--single' : 'signal-action-row--split'
    ];
}

function getStatusIcon(status: ImportPreviewSignalStatus): string {
    if (status === 'accepted') {
        return mdiCheck;
    }

    if (status === 'rejected') {
        return mdiAlertOutline;
    }

    return mdiLightbulbOutline;
}

function getLearningIcon(status: ImportPreviewSignalStatus): string {
    if (status === 'pending') {
        return mdiSchoolOutline;
    }

    return getStatusIcon(status);
}

function emitLLMReview(decision: 'accept' | 'reject' | 'clear'): void {
    if (decision === 'clear') {
        return;
    }

    emit('reviewLlm', decision);
}
</script>

<style scoped>
.signal-stack {
    display: inline-flex;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
    max-width: 100%;
}

.signal-stack--learning {
    width: fit-content;
}

.signal-chip {
    align-self: flex-start;
    max-width: 100%;
}

.signal-action-row {
    display: grid;
    gap: 4px;
    width: 100%;
}

.signal-action-row--single {
    grid-template-columns: minmax(0, 1fr);
}

.signal-action-row--split {
    grid-template-columns: repeat(2, minmax(0, 1fr));
}

.signal-action-btn {
    min-width: 0;
    padding-inline: 8px;
}

.signal-action-loader {
    opacity: 0.9;
}

.signal-detail-card {
    width: max-content;
    max-width: min(480px, calc(100vw - 32px));
}
</style>
