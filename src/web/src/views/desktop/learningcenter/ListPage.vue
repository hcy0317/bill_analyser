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
                                <v-list-item-title>LLM 归纳</v-list-item-title>
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
                                分析未分类交易
                            </v-btn>
                            <v-btn block variant="tonal" color="primary" class="mt-2"
                                   :disabled="loading"
                                   @click="loadLLMCandidates">
                                <v-icon start :icon="mdiRefresh" />
                                刷新候选
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
                                    <v-btn value="llm" size="small">LLM 归纳</v-btn>
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
                                                <div class="text-body-2 font-weight-medium">{{ rule.matchType }}</div>
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
                                <!-- LLM 配置 -->
                                <v-card variant="outlined" class="mb-4">
                                    <v-card-title class="text-subtitle-1">
                                        <v-icon start :icon="mdiCog" size="small" />
                                        LLM 配置
                                    </v-card-title>
                                    <v-card-text>
                                        <v-row dense>
                                            <v-col cols="12" sm="4">
                                                <v-select v-model="llmConfig.provider" label="提供商"
                                                           :items="['openai', 'anthropic', 'deepseek', 'ollama']"
                                                           density="compact" hide-details />
                                            </v-col>
                                            <v-col cols="12" sm="4">
                                                <v-text-field v-model="llmConfig.model" label="模型"
                                                              density="compact" hide-details
                                                              placeholder="gpt-4o-mini" />
                                            </v-col>
                                            <v-col cols="12" sm="4">
                                                <v-text-field v-model="llmConfig.api_key" label="API Key"
                                                              density="compact" hide-details
                                                              type="password" placeholder="sk-..." />
                                            </v-col>
                                        </v-row>
                                        <v-row dense class="mt-2">
                                            <v-col cols="12" sm="6">
                                                <v-text-field v-model="llmConfig.base_url" label="Base URL (可选)"
                                                              density="compact" hide-details
                                                              placeholder="https://api.openai.com/v1" />
                                            </v-col>
                                            <v-col cols="12" sm="3">
                                                <v-switch v-model="llmConfig.enabled" label="启用"
                                                          density="compact" hide-details color="success" />
                                            </v-col>
                                            <v-col cols="12" sm="3" class="d-flex align-center">
                                                <v-btn variant="tonal" color="primary" size="small"
                                                       :loading="llmConfigSaving"
                                                       @click="saveLLMConfig">
                                                    保存配置
                                                </v-btn>
                                            </v-col>
                                        </v-row>
                                    </v-card-text>
                                </v-card>

                                <!-- 分析结果提示 -->
                                <v-alert v-if="llmAnalyzeResult" type="info" closable class="mb-4"
                                         @click:close="llmAnalyzeResult = null">
                                    分析完成：生成 {{ llmAnalyzeResult.candidates_created || 0 }} 条候选规则
                                </v-alert>

                                <!-- 候选列表 -->
                                <div class="d-flex align-center mb-2">
                                    <span class="text-subtitle-2">候选规则</span>
                                    <v-spacer />
                                    <v-chip-group v-model="llmStatusFilter" mandatory>
                                        <v-chip value="" variant="tonal" size="small">全部</v-chip>
                                        <v-chip value="pending" variant="tonal" color="warning" size="small">待审核</v-chip>
                                        <v-chip value="accepted" variant="tonal" color="success" size="small">已采纳</v-chip>
                                        <v-chip value="rejected" variant="tonal" color="error" size="small">已拒绝</v-chip>
                                    </v-chip-group>
                                </div>

                                <v-progress-linear v-if="llmAnalyzing" indeterminate color="secondary" class="mb-2" />

                                <v-table v-if="filteredLLMCandidates.length > 0" hover density="comfortable">
                                    <thead>
                                        <tr>
                                            <th>类型</th>
                                            <th>规则内容</th>
                                            <th>目标分类</th>
                                            <th>置信度</th>
                                            <th>状态</th>
                                            <th class="text-center">操作</th>
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
                                                <v-chip size="x-small" :color="confidenceColor(candidate.confidence)">
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
                                                        <v-tooltip activator="parent">采纳</v-tooltip>
                                                    </v-btn>
                                                    <v-btn size="small" variant="text" color="error"
                                                           :icon="true" @click="handleLLMReject(candidate.id)">
                                                        <v-icon :icon="mdiClose" />
                                                        <v-tooltip activator="parent">拒绝</v-tooltip>
                                                    </v-btn>
                                                </template>
                                                <span v-else class="text-grey text-caption">—</span>
                                            </td>
                                        </tr>
                                    </tbody>
                                </v-table>

                                <v-empty-state v-if="!llmAnalyzing && filteredLLMCandidates.length === 0"
                                               :icon="mdiRobotOutline"
                                               headline="暂无候选规则"
                                               text="点击左侧「分析未分类交易」按钮，让 LLM 自动归纳分类规则。" />
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
import services from '@/lib/services.ts';

import {
    mdiRefresh,
    mdiAutoFix,
    mdiCheckAll,
    mdiBrain,
    mdiCheck,
    mdiClose,
    mdiDelete,
    mdiLightbulbOutline,
    mdiBookOpenPageVariant,
    mdiRobotOutline,
    mdiCog
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

// ── LLM 归纳 state ──────────
interface LLMConfig {
    provider: string;
    model: string;
    api_key: string;
    base_url: string;
    enabled: boolean;
}

const llmConfig = ref<LLMConfig>({
    provider: 'openai',
    model: '',
    api_key: '',
    base_url: '',
    enabled: false,
});
const llmConfigSaving = ref(false);
const llmAnalyzing = ref(false);
const llmAnalyzeResult = ref<any>(null);
const llmCandidates = ref<any[]>([]);
const llmStatusFilter = ref<string>('');

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
        case 'pending': return '待审核';
        case 'accepted': return '已采纳';
        case 'rejected': return '已拒绝';
        default: return status;
    }
}

function confidenceColor(confidence: number): string {
    if (confidence >= 0.8) return 'success';
    if (confidence >= 0.5) return 'warning';
    return 'error';
}

async function loadLLMConfig() {
    try {
        const resp = await services.getLLMConfig();
        if (resp.data?.success && resp.data.result) {
            const cfg = resp.data.result;
            llmConfig.value = {
                provider: cfg.provider || 'openai',
                model: cfg.model || '',
                api_key: cfg.api_key || '',
                base_url: cfg.base_url || '',
                enabled: !!cfg.enabled,
            };
        }
    } catch { /* ignore config load errors */ }
}

async function saveLLMConfig() {
    llmConfigSaving.value = true;
    try {
        await services.updateLLMConfig(llmConfig.value);
    } catch (e: any) {
        store.error = e.message || 'Failed to save LLM config';
    } finally {
        llmConfigSaving.value = false;
    }
}

async function loadLLMCandidates() {
    try {
        const resp = await services.getLLMCandidates({ limit: 100 });
        if (resp.data?.success && resp.data.result) {
            llmCandidates.value = Array.isArray(resp.data.result)
                ? resp.data.result
                : (resp.data.result.candidates || []);
        }
    } catch (e: any) {
        store.error = e.message || 'Failed to load LLM candidates';
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
    } catch (e: any) {
        store.error = e.message || 'LLM analysis failed';
    } finally {
        llmAnalyzing.value = false;
    }
}

async function handleLLMAccept(id: number) {
    try {
        await services.acceptLLMCandidate(id);
        await loadLLMCandidates();
    } catch (e: any) {
        store.error = e.message || 'Failed to accept candidate';
    }
}

async function handleLLMReject(id: number) {
    try {
        await services.rejectLLMCandidate(id);
        await loadLLMCandidates();
    } catch (e: any) {
        store.error = e.message || 'Failed to reject candidate';
    }
}

watch(activeTab, (tab) => {
    if (tab === 'suggestions') {
        store.loadSuggestions();
    } else if (tab === 'llm') {
        loadLLMConfig();
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
