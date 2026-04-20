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
                        <v-list density="compact" nav class="mt-2">
                            <v-list-item :active="activeTab === 'suggestions'"
                                         :prepend-icon="mdiLightbulbOutline"
                                         class="mb-1"
                                         @click="switchTab('suggestions')">
                                <v-list-item-title>{{ tt('Learning Suggestions') }}</v-list-item-title>
                            </v-list-item>
                            <v-list-item :active="activeTab === 'rules'"
                                         :prepend-icon="mdiBookOpenPageVariant"
                                         class="mb-1"
                                         @click="switchTab('rules')">
                                <v-list-item-title>{{ tt('Learning Rules') }}</v-list-item-title>
                            </v-list-item>
                            <v-list-item :active="activeTab === 'llm'"
                                         :prepend-icon="mdiRobotOutline"
                                         class="mb-1"
                                         @click="switchTab('llm')">
                                <v-list-item-title>{{ tt('LLM Induction') }}</v-list-item-title>
                                <template #append v-if="llmPendingCount > 0">
                                    <v-badge :content="llmPendingCount" color="warning" inline />
                                </template>
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
                        <div class="mx-6 mt-4" v-if="activeTab === 'llm'">
                            <v-btn block variant="outlined" color="secondary"
                                   :disabled="loading || llmAnalyzing"
                                   @click="handleLLMAnalyze">
                                <v-icon start :icon="mdiAutoFix" />
                                {{ tt('Analyze Uncategorized') }}
                            </v-btn>
                            <v-btn block variant="tonal" color="primary" class="mt-2"
                                   :disabled="loading"
                                   @click="loadLLMCandidates">
                                <v-icon start :icon="mdiRefresh" />
                                {{ tt('Refresh Candidates') }}
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
                                                <div class="text-body-2 font-weight-medium">{{ translateMatchType(item.matchType) }}</div>
                                                <div class="text-caption text-grey">{{ item.matchValue }}</div>
                                            </td>
                                            <td>
                                                <div class="d-flex flex-wrap ga-1">
                                                    <v-chip v-for="(feat, idx) in getFeatureSummary(item).split(' · ').filter(Boolean)"
                                                            :key="idx" size="x-small" variant="tonal" color="secondary">
                                                        {{ feat }}
                                                    </v-chip>
                                                </div>
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
                                               :icon="mdiLightbulbOutline"
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
                                                <div class="text-body-2 font-weight-medium">{{ translateMatchType(rule.matchType) }}</div>
                                                <div class="text-caption text-grey">{{ rule.matchValue }}</div>
                                            </td>
                                            <td>
                                                <div class="d-flex flex-wrap ga-1">
                                                    <v-chip v-for="(feat, idx) in getRuleFeatSummary(rule).split(' · ').filter(Boolean)"
                                                            :key="idx" size="x-small" variant="tonal" color="secondary">
                                                        {{ feat }}
                                                    </v-chip>
                                                </div>
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
                                                <v-btn size="small" variant="text" color="primary"
                                                       :icon="true" @click="openEditRuleDialog(rule)">
                                                    <v-icon :icon="mdiPencil" />
                                                    <v-tooltip activator="parent">{{ tt('Edit') }}</v-tooltip>
                                                </v-btn>
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
                                               :icon="mdiBookOpenPageVariant"
                                               :headline="tt('No Rules')"
                                               :text="tt('Accept suggestions to create learning rules that auto-classify future imports.')" />
                            </template>

                            <!-- ── LLM 归纳 Tab ── -->
                            <template v-if="activeTab === 'llm'">
                                <!-- LLM 多配置管理 -->
                                <v-card variant="outlined" class="mb-4">
                                    <v-card-title class="text-subtitle-1 d-flex align-center">
                                        <v-icon start :icon="mdiCog" size="small" />
                                        {{ tt('LLM Config') }}
                                        <v-spacer />
                                        <v-btn size="small" variant="tonal" color="primary"
                                               @click="openAddConfigDialog">
                                            {{ tt('Add Config') }}
                                        </v-btn>
                                    </v-card-title>
                                    <v-card-text>
                                        <v-table v-if="llmSavedConfigs.length > 0" density="compact" hover>
                                            <thead>
                                                <tr>
                                                    <th>{{ tt('Name') }}</th>
                                                    <th>{{ tt('Provider') }}</th>
                                                    <th>{{ tt('Model') }}</th>
                                                    <th class="text-center">{{ tt('Active') }}</th>
                                                    <th class="text-center">{{ tt('Actions') }}</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                <tr v-for="cfg in llmSavedConfigs" :key="cfg.id">
                                                    <td>{{ cfg.name }}</td>
                                                    <td>{{ cfg.provider }}</td>
                                                    <td>{{ cfg.model }}</td>
                                                    <td class="text-center">
                                                        <v-icon v-if="cfg.is_active" :icon="mdiCheck" color="success" size="small" />
                                                    </td>
                                                    <td class="text-center">
                                                        <v-btn v-if="!cfg.is_active" size="x-small" variant="text" color="primary"
                                                               @click="handleActivateConfig(cfg.id)">
                                                            {{ tt('Activate') }}
                                                        </v-btn>
                                                        <v-btn size="x-small" variant="text" color="error"
                                                               @click="handleDeleteConfig(cfg.id)">
                                                            <v-icon :icon="mdiDelete" size="small" />
                                                        </v-btn>
                                                    </td>
                                                </tr>
                                            </tbody>
                                        </v-table>
                                        <div v-else class="text-center text-caption text-medium-emphasis py-3">
                                            {{ tt('No saved configs. Click Add Config to create one.') }}
                                        </div>
                                    </v-card-text>
                                </v-card>

                                <!-- 分析结果提示 -->
                                <v-alert v-if="llmAnalyzeResult" type="info" closable class="mb-4"
                                         @click:close="llmAnalyzeResult = null">
                                    {{ tt('Analysis complete') }}: {{ tt('Generated') }} {{ llmAnalyzeResult.candidates_created || 0 }} {{ tt('candidate rules') }}
                                </v-alert>

                                <!-- 候选列表 -->
                                <div class="d-flex align-center mb-2">
                                    <span class="text-subtitle-2">{{ tt('Candidate Rules') }}</span>
                                    <v-spacer />
                                    <v-chip-group v-model="llmStatusFilter" mandatory>
                                        <v-chip value="" variant="tonal" size="small">{{ tt('All') }}</v-chip>
                                        <v-chip value="pending" variant="tonal" color="warning" size="small">{{ tt('Pending') }}</v-chip>
                                        <v-chip value="accepted" variant="tonal" color="success" size="small">{{ tt('Accepted') }}</v-chip>
                                        <v-chip value="rejected" variant="tonal" color="error" size="small">{{ tt('Rejected') }}</v-chip>
                                    </v-chip-group>
                                </div>

                                <v-progress-linear v-if="llmAnalyzing" indeterminate color="secondary" class="mb-2" />

                                <v-table v-if="filteredLLMCandidates.length > 0" hover density="comfortable">
                                    <thead>
                                        <tr>
                                            <th>{{ tt('Type') }}</th>
                                            <th>{{ tt('Rule Content') }}</th>
                                            <th>{{ tt('Target Category') }}</th>
                                            <th>{{ tt('Confidence') }}</th>
                                            <th>{{ tt('Status') }}</th>
                                            <th class="text-center">{{ tt('Actions') }}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr v-for="candidate in filteredLLMCandidates" :key="candidate.id">
                                            <td>
                                                <v-chip size="x-small" variant="tonal" color="info">
                                                    {{ candidate.rule_type || candidate.type || 'keyword' }}
                                                </v-chip>
                                            </td>
                                            <td>
                                                <div class="text-body-2 font-weight-medium">{{ candidate.rule_content || candidate.expression || '-' }}</div>
                                                <div v-if="candidate.reason" class="text-caption text-grey">{{ candidate.reason }}</div>
                                            </td>
                                            <td>{{ candidate.category_name || candidate.target_category || '-' }}</td>
                                            <td>
                                                <v-chip size="x-small" :color="confidenceColor(candidate.confidence ?? 0)">
                                                    {{ ((candidate.confidence || 0) * 100).toFixed(0) }}%
                                                </v-chip>
                                            </td>
                                            <td>
                                                <v-chip size="small" :color="llmStatusColor(candidate.status)">
                                                    {{ llmStatusLabel(candidate.status) }}
                                                </v-chip>
                                            </td>
                                            <td class="text-center">
                                                <template v-if="candidate.status === 'pending'">
                                                    <v-btn size="small" variant="text" color="success"
                                                           :icon="true" @click="handleLLMAccept(candidate.id)">
                                                        <v-icon :icon="mdiCheck" />
                                                        <v-tooltip activator="parent">{{ tt('Accept') }}</v-tooltip>
                                                    </v-btn>
                                                    <v-btn size="small" variant="text" color="error"
                                                           :icon="true" @click="handleLLMReject(candidate.id)">
                                                        <v-icon :icon="mdiClose" />
                                                        <v-tooltip activator="parent">{{ tt('Reject') }}</v-tooltip>
                                                    </v-btn>
                                                </template>
                                                <span v-else class="text-grey text-caption">—</span>
                                            </td>
                                        </tr>
                                    </tbody>
                                </v-table>

                                <v-empty-state v-if="!llmAnalyzing && filteredLLMCandidates.length === 0"
                                               :icon="mdiRobotOutline"
                                               :headline="tt('No Candidate Rules')"
                                               :text="tt('Click Analyze Uncategorized to let LLM induce classification rules.')" />
                            </template>
                        </v-card-text>
                    </v-main>
                </v-layout>
            </v-card>
        </v-col>
    </v-row>

    <!-- Edit Rule Dialog -->
    <v-dialog v-model="editRuleDialog" max-width="500" persistent>
        <v-card>
            <v-card-title>{{ tt('Edit Rule') }}</v-card-title>
            <v-card-text>
                <v-text-field v-model="editRuleForm.matchValue" :label="tt('Match Value')"
                              density="compact" class="mb-3" />
                <v-text-field v-model="editRuleForm.learnedType" :label="tt('Learned Type')"
                              density="compact" class="mb-3" />
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn variant="text" @click="editRuleDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="primary" variant="tonal" :loading="editRuleSaving"
                       @click="saveEditRule">{{ tt('Save') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- Add Config Dialog -->
    <v-dialog v-model="addConfigDialog" max-width="500" persistent>
        <v-card>
            <v-card-title>{{ tt('Add Config') }}</v-card-title>
            <v-card-text>
                <v-text-field v-model="newConfigForm.name" :label="tt('Name')"
                              density="compact" class="mb-3" placeholder="My OpenAI Config" />
                <v-select v-model="newConfigForm.provider" :label="tt('Provider')"
                          :items="['openai', 'anthropic', 'deepseek', 'ollama']"
                          density="compact" class="mb-3" />
                <v-text-field v-model="newConfigForm.model" :label="tt('Model')"
                              density="compact" class="mb-3" placeholder="gpt-4o-mini" />
                <v-text-field v-model="newConfigForm.api_key" label="API Key"
                              density="compact" class="mb-3" type="password" placeholder="sk-..." />
                <v-text-field v-model="newConfigForm.base_url" :label="tt('Base URL (optional)')"
                              density="compact" placeholder="https://api.openai.com/v1" />
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn variant="text" @click="addConfigDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="primary" variant="tonal" :loading="addConfigSaving"
                       @click="saveNewConfig">{{ tt('Save') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import axios from 'axios';
import { ref, computed, watch } from 'vue';
import { useDisplay } from 'vuetify';

import { useI18n } from '@/locales/helpers.ts';
import { useLearningStore } from '@/stores/learning.ts';
import type { LearningSuggestion, LearningRule, GenerateSuggestionsResponse } from '@/models/learning_center.ts';
import { getSuggestionFeatureSummary, getRuleFeatureSummary } from '@/models/learning_center.ts';
import services from '@/lib/services.ts';

import {
    mdiRefresh,
    mdiAutoFix,
    mdiCheckAll,
    mdiBrain,
    mdiCheck,
    mdiClose,
    mdiDelete,
    mdiPencil,
    mdiLightbulbOutline,
    mdiBookOpenPageVariant,
    mdiRobotOutline,
    mdiCog
} from '@mdi/js';

interface LLMConfigItem {
    id: number;
    name: string;
    provider: string;
    model: string;
    api_key?: string;
    base_url?: string;
    is_active?: boolean;
    created_at?: string;
    updated_at?: string;
}

interface LLMAnalyzeResult {
    candidates_created?: number;
}

interface LLMCandidateItem {
    id: number;
    type?: string;
    status: string;
    confidence?: number;
    rule_type?: string;
    rule_content?: string;
    expression?: string;
    reason?: string;
    category_name?: string;
    target_category?: string;
}

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

function toLLMConfigs(result: unknown): LLMConfigItem[] {
    return Array.isArray(result) ? result as LLMConfigItem[] : [];
}

function toLLMCandidates(result: unknown): LLMCandidateItem[] {
    if (Array.isArray(result)) {
        return result as LLMCandidateItem[];
    }

    if (result && typeof result === 'object' && 'candidates' in result) {
        const candidates = (result as { candidates?: unknown }).candidates;
        return Array.isArray(candidates) ? candidates as LLMCandidateItem[] : [];
    }

    return [];
}

function extractPayloadMessage(payload: unknown, depth = 0): string | null {
    if (depth > 2) {
        return null;
    }

    if (typeof payload === 'string' && payload) {
        return payload;
    }

    if (!payload || typeof payload !== 'object') {
        return null;
    }

    const errorMessage = 'error' in payload ? extractPayloadMessage(payload.error, depth + 1) : null;
    if (errorMessage) {
        return errorMessage;
    }

    const message = 'message' in payload ? extractPayloadMessage(payload.message, depth + 1) : null;
    if (message) {
        return message;
    }

    return null;
}

function getPayloadErrorMessage(payload: unknown, fallback: string): string {
    return extractPayloadMessage(payload) || fallback;
}

function getRequestErrorMessage(error: unknown, fallback: string): string {
    if (axios.isAxiosError(error)) {
        return getPayloadErrorMessage(error.response?.data, fallback);
    }

    if (error instanceof Error && error.message) {
        return error.message;
    }

    return fallback;
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

function translateMatchType(type: string): string {
    const map: Record<string, string> = {
        'composite': tt('Composite Rule'),
        'counterparty': tt('Counterparty'),
        'description': tt('Description'),
        'keyword': tt('Keyword'),
        'amount': tt('Amount'),
        'payment_method': tt('Payment Method'),
    };
    return map[type] || type;
}

function switchTab(tab: string) {
    activeTab.value = tab;
}

async function refreshCurrentTab() {
    if (activeTab.value === 'suggestions') {
        await store.loadSuggestions();
    } else if (activeTab.value === 'llm') {
        await loadLLMCandidates();
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

// ── Edit Rule Dialog ──────────
const editRuleDialog = ref(false);
const editRuleSaving = ref(false);
const editRuleForm = ref({ id: 0, matchValue: '', learnedType: '' });

function openEditRuleDialog(rule: LearningRule) {
    editRuleForm.value = {
        id: rule.id,
        matchValue: rule.matchValue || '',
        learnedType: rule.learnedType || '',
    };
    editRuleDialog.value = true;
}

async function saveEditRule() {
    editRuleSaving.value = true;
    try {
        const resp = await services.updateLearningRule({
            ruleId: editRuleForm.value.id,
            matchValue: editRuleForm.value.matchValue,
            learnedType: editRuleForm.value.learnedType,
        });
        if (resp.data?.success) {
            editRuleDialog.value = false;
            await store.loadRules();
        } else {
            store.error = getPayloadErrorMessage(resp.data?.result, 'Failed to update rule');
        }
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to update rule');
    } finally {
        editRuleSaving.value = false;
    }
}

// ── LLM 归纳 state ──────────
const llmSavedConfigs = ref<LLMConfigItem[]>([]);
const llmAnalyzing = ref(false);
const llmAnalyzeResult = ref<LLMAnalyzeResult | null>(null);
const llmCandidates = ref<LLMCandidateItem[]>([]);
const llmStatusFilter = ref<string>('');

// Add Config Dialog
const addConfigDialog = ref(false);
const addConfigSaving = ref(false);
const newConfigForm = ref({ name: '', provider: 'openai', model: '', api_key: '', base_url: '' });

const llmPendingCount = computed(() =>
    llmCandidates.value.filter(c => c.status === 'pending').length
);

const filteredLLMCandidates = computed(() => {
    if (!llmStatusFilter.value) return llmCandidates.value;
    return llmCandidates.value.filter(c => c.status === llmStatusFilter.value);
});

function llmStatusColor(status: string): string {
    switch (status) {
        case 'pending': return 'warning';
        case 'accepted': return 'success';
        case 'rejected': return 'error';
        default: return 'grey';
    }
}

function llmStatusLabel(status: string): string {
    switch (status) {
        case 'pending': return tt('Pending');
        case 'accepted': return tt('Accepted');
        case 'rejected': return tt('Rejected');
        default: return status;
    }
}

function confidenceColor(confidence: number): string {
    if (confidence >= 0.8) return 'success';
    if (confidence >= 0.5) return 'warning';
    return 'error';
}

async function loadLLMConfigs() {
    try {
        const resp = await services.getLLMConfigs();
        if (resp.data?.success && resp.data.result) {
            llmSavedConfigs.value = toLLMConfigs(resp.data.result);
        }
    } catch { /* ignore config load errors */ }
}

function openAddConfigDialog() {
    newConfigForm.value = { name: '', provider: 'openai', model: '', api_key: '', base_url: '' };
    addConfigDialog.value = true;
}

async function saveNewConfig() {
    if (!newConfigForm.value.name.trim()) {
        store.error = 'Name is required';
        return;
    }
    addConfigSaving.value = true;
    try {
        const resp = await services.createLLMConfig({
            ...newConfigForm.value,
            is_active: llmSavedConfigs.value.length === 0, // auto-activate first config
        });
        if (resp.data?.success) {
            addConfigDialog.value = false;
            await loadLLMConfigs();
        } else {
            store.error = getPayloadErrorMessage(resp.data?.result, 'Failed to create config');
        }
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to create config');
    } finally {
        addConfigSaving.value = false;
    }
}

async function handleActivateConfig(configId: number) {
    try {
        await services.activateLLMConfig(configId);
        await loadLLMConfigs();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to activate config');
    }
}

async function handleDeleteConfig(configId: number) {
    try {
        await services.deleteLLMConfig(configId);
        await loadLLMConfigs();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to delete config');
    }
}

async function loadLLMCandidates() {
    try {
        const resp = await services.getLLMCandidates({ limit: 100 });
        if (resp.data?.success && resp.data.result) {
            llmCandidates.value = toLLMCandidates(resp.data.result);
        }
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to load LLM candidates');
    }
}

async function handleLLMAnalyze() {
    llmAnalyzing.value = true;
    llmAnalyzeResult.value = null;
    try {
        const resp = await services.analyzeLLMTransactions(undefined, 20);
        if (resp.data?.success && resp.data.result) {
            llmAnalyzeResult.value = resp.data.result;
        }
        await loadLLMCandidates();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'LLM analysis failed');
    } finally {
        llmAnalyzing.value = false;
    }
}

async function handleLLMAccept(id: number) {
    try {
        await services.acceptLLMCandidate(id);
        await loadLLMCandidates();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to accept candidate');
    }
}

async function handleLLMReject(id: number) {
    try {
        await services.rejectLLMCandidate(id);
        await loadLLMCandidates();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to reject candidate');
    }
}

watch(activeTab, (tab) => {
    if (tab === 'suggestions') {
        store.loadSuggestions();
    } else if (tab === 'llm') {
        loadLLMConfigs();
        loadLLMCandidates();
    } else {
        store.loadRules();
    }
}, { immediate: true });
</script>

<style scoped>
.v-table :deep(td) {
    padding-block: 12px;
    vertical-align: middle;
}

.v-table :deep(th) {
    white-space: nowrap;
}
</style>
