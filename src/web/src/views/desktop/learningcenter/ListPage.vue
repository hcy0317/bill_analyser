<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-layout>
                    <!-- 左侧操作面板 -->
                    <v-navigation-drawer :permanent="alwaysShowNav" v-model="showNav">
                        <div class="mx-6 mt-4">
                            <v-btn block variant="tonal" color="primary"
                                   :disabled="loading"
                                   @click="refreshCurrentTab">
                                <v-icon start :icon="mdiRefresh" />
                                {{ tt('Refresh') }}
                            </v-btn>
                        </div>
                        <v-divider class="mt-4" />
                        <v-list density="compact" nav>
                            <v-list-item :active="activeTab === 'suggestions'"
                                         @click="switchTab('suggestions')">
                                <v-list-item-title>{{ tt('Learning Suggestions') }}</v-list-item-title>
                            </v-list-item>
                            <v-list-item :active="activeTab === 'rules'"
                                         @click="switchTab('rules')">
                                <v-list-item-title>{{ tt('Learning Rules') }}</v-list-item-title>
                            </v-list-item>
                        </v-list>
                        <v-divider />
                        <div class="mx-6 mt-4" v-if="activeTab === 'suggestions'">
                            <v-btn block variant="outlined" color="secondary"
                                   :disabled="loading"
                                   @click="handleGenerate">
                                <v-icon start :icon="mdiAutoFix" />
                                {{ tt('Generate Suggestions') }}
                            </v-btn>
                            <v-btn block variant="tonal" color="success" class="mt-2"
                                   :disabled="loading || selectedIds.length === 0"
                                   @click="handleBatchAccept">
                                <v-icon start :icon="mdiCheckAll" />
                                {{ tt('Batch Accept') }} ({{ selectedIds.length }})
                            </v-btn>
                        </div>
                    </v-navigation-drawer>

                    <!-- 主内容区 -->
                    <v-main>
                        <v-card-text>
                            <div class="d-flex align-center mb-4">
                                <v-icon :icon="mdiBrain" class="mr-2" />
                                <span class="text-h6">{{ tt('Learning Center') }}</span>
                                <v-spacer />
                                <v-btn-toggle v-model="activeTab" mandatory density="compact"
                                              color="primary" variant="outlined">
                                    <v-btn value="suggestions" size="small">{{ tt('Suggestions') }}</v-btn>
                                    <v-btn value="rules" size="small">{{ tt('Rules') }}</v-btn>
                                </v-btn-toggle>
                            </div>

                            <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />

                            <v-alert v-if="error" type="error" closable class="mb-4"
                                     @click:close="clearError">
                                {{ error }}
                            </v-alert>

                            <v-alert v-if="lastGenerateResult" type="info" closable class="mb-4"
                                     @click:close="lastGenerateResult = null">
                                {{ tt('Generated') }}: {{ lastGenerateResult.created }} {{ tt('new') }},
                                {{ lastGenerateResult.updated }} {{ tt('updated') }}
                            </v-alert>

                            <!-- ── Suggestions Tab ── -->
                            <template v-if="activeTab === 'suggestions'">
                                <div class="d-flex align-center mb-2">
                                    <v-chip-group v-model="statusFilter" mandatory>
                                        <v-chip value="" variant="tonal">{{ tt('All') }} ({{ suggestionsTotal }})</v-chip>
                                        <v-chip value="pending" variant="tonal" color="warning">{{ tt('Pending') }}</v-chip>
                                        <v-chip value="accepted" variant="tonal" color="success">{{ tt('Accepted') }}</v-chip>
                                        <v-chip value="rejected" variant="tonal" color="error">{{ tt('Rejected') }}</v-chip>
                                    </v-chip-group>
                                </div>

                                <v-table v-if="!loading && filteredSuggestions.length > 0" hover density="comfortable">
                                    <thead>
                                        <tr>
                                            <th style="width:40px">
                                                <v-checkbox-btn v-model="selectAll" :indeterminate="indeterminate"
                                                                density="compact" hide-details />
                                            </th>
                                            <th>{{ tt('Match Pattern') }}</th>
                                            <th>{{ tt('Features') }}</th>
                                            <th>{{ tt('Suggested Action') }}</th>
                                            <th class="text-center">{{ tt('Samples') }}</th>
                                            <th>{{ tt('Status') }}</th>
                                            <th class="text-center">{{ tt('Actions') }}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr v-for="item in filteredSuggestions" :key="item.id">
                                            <td>
                                                <v-checkbox-btn v-model="selectedIds" :value="item.id"
                                                                density="compact" hide-details
                                                                :disabled="item.status !== 'pending'" />
                                            </td>
                                            <td>
                                                <div class="text-body-2 font-weight-medium">{{ item.matchType }}</div>
                                                <div class="text-caption text-grey">{{ item.matchValue }}</div>
                                            </td>
                                            <td>
                                                <div class="text-caption">{{ getFeatureSummary(item) }}</div>
                                            </td>
                                            <td>
                                                <div class="text-body-2">{{ item.summary || item.suggestedType }}</div>
                                            </td>
                                            <td class="text-center">
                                                <v-chip size="x-small" color="info" variant="tonal">{{ item.sampleCount }}</v-chip>
                                            </td>
                                            <td>
                                                <v-chip size="small" :color="statusColor(item.status)">
                                                    {{ tt(statusLabel(item.status)) }}
                                                </v-chip>
                                            </td>
                                            <td class="text-center">
                                                <template v-if="item.status === 'pending'">
                                                    <v-btn size="small" variant="text" color="success"
                                                           :icon="true" @click="handleAccept(item.id)">
                                                        <v-icon :icon="mdiCheck" />
                                                        <v-tooltip activator="parent">{{ tt('Accept') }}</v-tooltip>
                                                    </v-btn>
                                                    <v-btn size="small" variant="text" color="error"
                                                           :icon="true" @click="handleReject(item.id)">
                                                        <v-icon :icon="mdiClose" />
                                                        <v-tooltip activator="parent">{{ tt('Reject') }}</v-tooltip>
                                                    </v-btn>
                                                </template>
                                                <span v-else class="text-grey text-caption">—</span>
                                            </td>
                                        </tr>
                                    </tbody>
                                </v-table>

                                <v-empty-state v-if="!loading && filteredSuggestions.length === 0"
                                               :headline="tt('No Suggestions')"
                                               :text="tt('Click Generate Suggestions to mine patterns from your transaction history.')" />
                            </template>

                            <!-- ── Rules Tab ── -->
                            <template v-if="activeTab === 'rules'">
                                <v-table v-if="!loading && rules.length > 0" hover density="comfortable">
                                    <thead>
                                        <tr>
                                            <th>{{ tt('Match Pattern') }}</th>
                                            <th>{{ tt('Features') }}</th>
                                            <th>{{ tt('Learned Action') }}</th>
                                            <th class="text-center">{{ tt('Applied') }}</th>
                                            <th>{{ tt('Enabled') }}</th>
                                            <th class="text-center">{{ tt('Actions') }}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr v-for="rule in rules" :key="rule.id">
                                            <td>
                                                <div class="text-body-2 font-weight-medium">{{ rule.matchType }}</div>
                                                <div class="text-caption text-grey">{{ rule.matchValue }}</div>
                                            </td>
                                            <td>
                                                <div class="text-caption">{{ getRuleFeatSummary(rule) }}</div>
                                            </td>
                                            <td>
                                                <div class="text-body-2">{{ rule.learnedType }}</div>
                                            </td>
                                            <td class="text-center">
                                                <v-chip size="x-small" color="info" variant="tonal">{{ rule.appliedCount }}</v-chip>
                                            </td>
                                            <td>
                                                <v-switch density="compact" color="success" hide-details
                                                          :model-value="rule.enabled"
                                                          @update:model-value="(v: boolean | null) => handleToggleRule(rule.id, !!v)" />
                                            </td>
                                            <td class="text-center">
                                                <v-btn size="small" variant="text" color="error"
                                                       :icon="true" @click="handleDeleteRule(rule.id)">
                                                    <v-icon :icon="mdiDelete" />
                                                    <v-tooltip activator="parent">{{ tt('Delete') }}</v-tooltip>
                                                </v-btn>
                                            </td>
                                        </tr>
                                    </tbody>
                                </v-table>

                                <v-empty-state v-if="!loading && rules.length === 0"
                                               :headline="tt('No Rules')"
                                               :text="tt('Accept suggestions to create learning rules that auto-classify future imports.')" />
                            </template>
                        </v-card-text>
                    </v-main>
                </v-layout>
            </v-card>
        </v-col>
    </v-row>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useDisplay } from 'vuetify';

import { useI18n } from '@/locales/helpers.ts';
import { useLearningStore } from '@/stores/learning.ts';
import type { LearningSuggestion, LearningRule, GenerateSuggestionsResponse } from '@/models/learning_center.ts';
import { getSuggestionFeatureSummary, getRuleFeatureSummary } from '@/models/learning_center.ts';

import {
    mdiRefresh,
    mdiAutoFix,
    mdiCheckAll,
    mdiBrain,
    mdiCheck,
    mdiClose,
    mdiDelete
} from '@mdi/js';

const props = defineProps<{
    initTab?: string
}>();

const display = useDisplay();
const { tt } = useI18n();
const store = useLearningStore();

const activeTab = ref<string>(props.initTab || 'suggestions');
const statusFilter = ref<string>('');
const showNav = ref(true);
const selectedIds = ref<number[]>([]);
const lastGenerateResult = ref<GenerateSuggestionsResponse | null>(null);

const alwaysShowNav = computed(() => !display.mdAndDown.value);
const loading = computed(() => store.suggestionsLoading || store.rulesLoading);
const error = computed(() => store.error);
const suggestionsTotal = computed(() => store.suggestionsTotal);
const rules = computed(() => store.rules);

const filteredSuggestions = computed(() => {
    if (!statusFilter.value) return store.suggestions;
    return store.suggestions.filter(s => s.status === statusFilter.value);
});

const selectAll = computed({
    get() {
        const pending = filteredSuggestions.value.filter(s => s.status === 'pending');
        return pending.length > 0 && pending.every(s => selectedIds.value.includes(s.id));
    },
    set(val: boolean) {
        if (val) {
            selectedIds.value = filteredSuggestions.value
                .filter(s => s.status === 'pending')
                .map(s => s.id);
        } else {
            selectedIds.value = [];
        }
    }
});

const indeterminate = computed(() => {
    const pending = filteredSuggestions.value.filter(s => s.status === 'pending');
    const selectedCount = pending.filter(s => selectedIds.value.includes(s.id)).length;
    return selectedCount > 0 && selectedCount < pending.length;
});

function clearError() {
    store.error = null;
}

function getFeatureSummary(item: LearningSuggestion): string {
    return getSuggestionFeatureSummary(item);
}

function getRuleFeatSummary(rule: LearningRule): string {
    return getRuleFeatureSummary(rule);
}

function statusColor(status: string): string {
    switch (status) {
        case 'pending': return 'warning';
        case 'accepted': return 'success';
        case 'rejected': return 'error';
        default: return 'grey';
    }
}

function statusLabel(status: string): string {
    switch (status) {
        case 'pending': return 'Pending';
        case 'accepted': return 'Accepted';
        case 'rejected': return 'Rejected';
        default: return status;
    }
}

function switchTab(tab: string) {
    activeTab.value = tab;
}

async function refreshCurrentTab() {
    if (activeTab.value === 'suggestions') {
        await store.loadSuggestions();
    } else {
        await store.loadRules();
    }
}

async function handleGenerate() {
    const result = await store.generateSuggestions();
    if (result) {
        lastGenerateResult.value = result;
    }
}

async function handleAccept(id: number) {
    await store.acceptSuggestion(id);
}

async function handleReject(id: number) {
    await store.rejectSuggestion(id);
}

async function handleBatchAccept() {
    if (selectedIds.value.length === 0) return;
    await store.batchAcceptSuggestions([...selectedIds.value]);
    selectedIds.value = [];
}

async function handleToggleRule(ruleId: number, enabled: boolean) {
    await store.toggleRule(ruleId, enabled);
}

async function handleDeleteRule(ruleId: number) {
    await store.deleteRule(ruleId);
}

watch(activeTab, (tab) => {
    if (tab === 'suggestions') {
        store.loadSuggestions();
    } else {
        store.loadRules();
    }
}, { immediate: true });
</script>
