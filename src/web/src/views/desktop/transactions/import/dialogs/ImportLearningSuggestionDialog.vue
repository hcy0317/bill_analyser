<template>
    <v-dialog width="960" persistent v-model="showState">
        <v-card class="pa-2 pa-sm-4 pa-md-4">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center flex-wrap ga-2">
                        <h4 class="text-h4">{{ tt('Review Learning Suggestions') }}</h4>
                        <v-chip size="small" color="secondary" variant="tonal">
                            {{ tt('format.misc.selectedCount', { count: selectedSuggestions.length, totalCount: suggestions.length }) }}
                        </v-chip>
                    </div>
                </div>
            </template>
            <v-card-text class="my-md-4 w-100">
                <p class="text-body-2 text-medium-emphasis mb-4">
                    {{ tt('Select the learning suggestions to promote into long-term rules') }}
                </p>
                <v-alert type="info" variant="tonal" v-if="!suggestions.length">
                    {{ tt('No learning suggestions available for the selected preview rows') }}
                </v-alert>
                <div class="learning-suggestion-list" v-else>
                    <v-list lines="three">
                        <v-list-item
                            v-for="suggestion in suggestions"
                            :key="getImportLearningSuggestionKey(suggestion)"
                            @click="toggleSuggestionSelection(suggestion)">
                            <template #prepend>
                                <v-checkbox-btn
                                    density="compact"
                                    color="primary"
                                    :model-value="selectedSuggestionKeySet[getImportLearningSuggestionKey(suggestion)] === true"
                                    @click.stop
                                    @update:model-value="updateSelectedSuggestionState(suggestion, $event)" />
                            </template>
                            <v-list-item-title>
                                <div class="d-flex align-center ga-2 flex-wrap">
                                    <span>{{ suggestion.summary || suggestion.matchValue || tt('Learning Suggestion') }}</span>
                                    <v-chip size="x-small" color="secondary" variant="tonal">
                                        {{ suggestion.matchType }}
                                    </v-chip>
                                    <v-chip size="x-small" color="info" variant="outlined">
                                        {{ tt('Sample Count') }} {{ suggestion.sampleCount }}
                                    </v-chip>
                                </div>
                            </v-list-item-title>
                            <v-list-item-subtitle>
                                <div class="text-body-2">
                                    {{ getImportLearningSuggestionFeatureSummary(suggestion) || suggestion.matchValue || '-' }}
                                </div>
                                <div class="text-caption text-medium-emphasis mt-1">
                                    {{ tt('Preview Sources') }} #{{ suggestion.sourcePreviewIds.join(', #') }}
                                </div>
                                <div class="d-flex flex-wrap ga-1 mt-2">
                                    <v-chip v-if="suggestion.learnedType" size="x-small" color="primary" variant="tonal">
                                        {{ suggestion.learnedType }}
                                    </v-chip>
                                    <v-chip v-if="suggestion.learnedCategoryName" size="x-small" color="success" variant="outlined">
                                        {{ suggestion.learnedCategoryName }}
                                    </v-chip>
                                    <v-chip v-if="suggestion.learnedSourceAccountName" size="x-small" color="default" variant="outlined">
                                        {{ suggestion.learnedSourceAccountName }}
                                    </v-chip>
                                    <v-chip v-if="suggestion.learnedDestinationAccountName" size="x-small" color="default" variant="outlined">
                                        {{ suggestion.learnedDestinationAccountName }}
                                    </v-chip>
                                </div>
                            </v-list-item-subtitle>
                        </v-list-item>
                    </v-list>
                </div>
            </v-card-text>
            <v-card-text class="overflow-y-visible">
                <div class="w-100 d-flex justify-center gap-4 flex-wrap">
                    <v-btn :disabled="!selectedSuggestions.length" @click="confirm">
                        {{ tt('Promote Selected Suggestions') }}
                    </v-btn>
                    <v-btn color="secondary" variant="tonal" @click="cancel">
                        {{ tt('Cancel') }}
                    </v-btn>
                </div>
            </v-card-text>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { arrayItemToObjectField } from '@/lib/common.ts';
import {
    collectImportLearningSuggestionPreviewIds,
    getImportLearningSuggestionFeatureSummary,
    getImportLearningSuggestionKey,
    type ImportLearningSuggestion
} from '@/models/import_learning.ts';

interface ImportLearningSuggestionDialogResponse {
    suggestions: ImportLearningSuggestion[];
    previewIds: number[];
}

const { tt } = useI18n();

const showState = ref<boolean>(false);
const suggestions = ref<ImportLearningSuggestion[]>([]);
const selectedSuggestionKeys = ref<string[]>([]);

let resolveFunc: ((response: ImportLearningSuggestionDialogResponse) => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const selectedSuggestionKeySet = computed<Record<string, boolean>>(() => {
    return arrayItemToObjectField(selectedSuggestionKeys.value, true);
});

const selectedSuggestions = computed<ImportLearningSuggestion[]>(() => {
    return suggestions.value.filter(suggestion => {
        return selectedSuggestionKeySet.value[getImportLearningSuggestionKey(suggestion)] === true;
    });
});

function updateSelectedSuggestionState(
    suggestion: ImportLearningSuggestion,
    selected: boolean | null
): void {
    const suggestionKey = getImportLearningSuggestionKey(suggestion);
    const nextSelection = selectedSuggestionKeys.value.filter(key => key !== suggestionKey);

    if (selected) {
        nextSelection.push(suggestionKey);
    }

    selectedSuggestionKeys.value = nextSelection;
}

function toggleSuggestionSelection(suggestion: ImportLearningSuggestion): void {
    const suggestionKey = getImportLearningSuggestionKey(suggestion);
    const isSelected = selectedSuggestionKeySet.value[suggestionKey] === true;
    updateSelectedSuggestionState(suggestion, !isSelected);
}

function open(options: {
    suggestions: ImportLearningSuggestion[];
}): Promise<ImportLearningSuggestionDialogResponse> {
    suggestions.value = options.suggestions;
    selectedSuggestionKeys.value = options.suggestions.map(getImportLearningSuggestionKey);
    showState.value = true;

    return new Promise((resolve, reject) => {
        resolveFunc = resolve;
        rejectFunc = reject;
    });
}

function confirm(): void {
    resolveFunc?.({
        suggestions: selectedSuggestions.value,
        previewIds: collectImportLearningSuggestionPreviewIds(selectedSuggestions.value)
    });
    showState.value = false;
}

function cancel(): void {
    rejectFunc?.();
    showState.value = false;
}

defineExpose({
    open
});
</script>

<style scoped>
.learning-suggestion-list {
    max-height: 420px;
    overflow-y: auto;
    border: 1px solid rgba(var(--v-theme-outline), 0.24);
    border-radius: 8px;
}
</style>
