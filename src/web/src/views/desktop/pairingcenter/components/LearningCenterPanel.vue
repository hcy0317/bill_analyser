<template src="./learning-center/LearningCenterPanel.template.html"></template>

<script setup lang="ts">
import { ref, computed, watch, useTemplateRef } from 'vue';

import SnackBar from '@/components/desktop/SnackBar.vue';
import SettingsJsonImportExportButton from '@/components/desktop/SettingsJsonImportExportButton.vue';
import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import { useI18n } from '@/locales/helpers.ts';
import { useLearningStore } from '@/stores/learning.ts';
import type { LearningRule } from '@/models/learning_center.ts';
import { getSuggestionFeatureChips, getRuleFeatureChips, getRuleFeatureSummary } from '@/models/learning_center.ts';
import services from '@/lib/services.ts';
import {
    extractPayloadMessage,
    getRequestErrorMessage,
} from './apiResultHelpers.ts';
import {
    confidenceColor,
    createLearningRuleMatchTypeOptions,
    createRuleAppliedFilterOptions,
    createRuleEnabledFilterOptions,
    filterLearningRules,
    getLearningStatusColor,
    getLearningStatusLabel,
    getTranslatedLearningStatusLabel,
    normalizeLearningPanelTab,
    translateLearningMatchType,
} from './learningCenterPanelModel.ts';
import { useLearningCenterLlmConfig } from './learning-center/useLearningCenterLlmConfig.ts';
import type { SelectOption } from './llmConfigHelpers.ts';

import {
    mdiRefresh,
    mdiCheck,
    mdiClose,
    mdiDelete,
    mdiPencil,
    mdiLightbulbOutline,
    mdiBookOpenPageVariant,
    mdiRobotOutline,
    mdiChevronDown,
    mdiFilterVariant,
} from '@mdi/js';

const props = defineProps<{
    initTab?: string;
    hideSectionTitle?: boolean;
    headerActionsTarget?: string;
}>();

type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();
const store = useLearningStore();
const snackbar = useTemplateRef<SnackBarType>('snackbar');

const activeTab = ref<string>(normalizeLearningPanelTab(props.initTab));
const statusFilter = ref<string>('');
const selectedIds = ref<number[]>([]);
const ruleMatchTypeFilter = ref<string>('');
const ruleFeatureFilter = ref<string>('');
const ruleLearnedActionFilter = ref<string>('');
const ruleEnabledFilter = ref<string>('all');
const ruleAppliedFilter = ref<string>('all');
const ruleMatchTypeFilterMenu = ref(false);
const ruleFeatureFilterMenu = ref(false);
const ruleLearnedActionFilterMenu = ref(false);
const ruleEnabledFilterMenu = ref(false);
const ruleAppliedFilterMenu = ref(false);

function setLearningError(message: string): void {
    store.error = message;
}

const {
    llmLoading,
    llmConfigLoading,
    llmSavedConfigs,
    llmStatusFilter,
    selectedLLMIds,
    llmProviderOptions,
    llmReasoningDepthOptions,
    llmCredentialModeOptions,
    addConfigDialog,
    addConfigSaving,
    autofillFieldsLocked,
    newConfigForm,
    selectedLLMProviderOption,
    baseUrlFieldLabel,
    llmConfigFieldNames,
    llmPendingCount,
    llmStatusOptions,
    filteredLLMCandidates,
    selectAllLLM,
    llmIndeterminate,
    unlockAutofillFields,
    closeAddConfigDialog,
    loadLLMConfigs,
    openAddConfigDialog,
    saveNewConfig,
    llmProviderLabel,
    handleActivateConfig,
    handleDeleteConfig,
    loadLLMCandidates,
    handleLLMGenerate,
    handleLLMBatchAccept,
    handleLLMAccept,
    handleLLMReject,
} = useLearningCenterLlmConfig({
    tt,
    setError: setLearningError,
    showInfoMessage,
    getPayloadErrorMessage,
    getRequestErrorMessage,
});

const loading = computed(() => (
    store.suggestionsLoading
    || store.rulesLoading
    || llmLoading.value
    || llmConfigLoading.value
));
const error = computed(() => store.error);
const suggestionsTotal = computed(() => store.suggestionsTotal);
const rules = computed(() => store.rules);
const rulesTotal = computed(() => store.rulesTotal);
const hideSectionTitle = computed(() => Boolean(props.hideSectionTitle));
const hasHeaderActionsTarget = computed(() => Boolean(props.headerActionsTarget));
const headerActionsTarget = computed(() => props.headerActionsTarget || 'body');

const suggestionStatusOptions = computed<SelectOption[]>(() => [
    { title: `${tt('All')} (${suggestionsTotal.value})`, value: '' },
    { title: tt('Pending'), value: 'pending' },
    { title: tt('Accepted'), value: 'accepted' },
    { title: tt('Rejected'), value: 'rejected' },
]);

const filteredSuggestions = computed(() => {
    if (!statusFilter.value) return store.suggestions;
    return store.suggestions.filter(s => s.status === statusFilter.value);
});

const ruleMatchTypeOptions = computed<SelectOption[]>(() => createLearningRuleMatchTypeOptions(rules.value, tt));
const ruleEnabledFilterOptions = computed<SelectOption[]>(() => createRuleEnabledFilterOptions(tt));
const ruleAppliedFilterOptions = computed<SelectOption[]>(() => createRuleAppliedFilterOptions(tt));

const ruleFeatureFilterActive = computed(() => ruleFeatureFilter.value.trim().length > 0);
const ruleLearnedActionFilterActive = computed(() => ruleLearnedActionFilter.value.trim().length > 0);

function setRuleMatchTypeFilter(value: string): void {
    ruleMatchTypeFilter.value = value;
    ruleMatchTypeFilterMenu.value = false;
}

function clearRuleFeatureFilter(): void {
    ruleFeatureFilter.value = '';
}

function clearRuleLearnedActionFilter(): void {
    ruleLearnedActionFilter.value = '';
}

function setRuleEnabledFilter(value: string): void {
    ruleEnabledFilter.value = value;
    ruleEnabledFilterMenu.value = false;
}

function setRuleAppliedFilter(value: string): void {
    ruleAppliedFilter.value = value;
    ruleAppliedFilterMenu.value = false;
}

const filteredRules = computed(() => filterLearningRules(
    rules.value,
    {
        matchType: ruleMatchTypeFilter.value,
        featureText: ruleFeatureFilter.value,
        learnedAction: ruleLearnedActionFilter.value,
        enabledState: ruleEnabledFilter.value,
        appliedState: ruleAppliedFilter.value,
    },
    getRuleFeatureSummary
));

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

function showInfoMessage(message: string, options?: Record<string, unknown>): void {
    snackbar.value?.showMessage(message, options);
}

function getPayloadErrorMessage(payload: unknown, fallback: string): string {
    return extractPayloadMessage(payload) || fallback;
}

function statusColor(status: string): string {
    return getLearningStatusColor(status);
}

function statusLabel(status: string): string {
    return getLearningStatusLabel(status);
}

function translateMatchType(type: string): string {
    return translateLearningMatchType(type, tt);
}

/**
 * 按当前 tab 刷新对应数据源，避免学习规则、LLM 候选和配置列表互相触发。
 */
async function refreshCurrentTab() {
    if (activeTab.value === 'suggestions') {
        await store.loadSuggestions();
    } else if (activeTab.value === 'llm') {
        await loadLLMCandidates();
    } else if (activeTab.value === 'llm-config') {
        await loadLLMConfigs();
    } else {
        await store.loadRules();
    }
}

/**
 * 生成普通学习建议，并把新增/更新数量反馈到页面提示。
 */
async function handleGenerate() {
    const result = await store.generateSuggestions();
    if (result) {
        showInfoMessage('Generated Suggestions Summary', {
            created: result.created,
            updated: result.updated,
        });
    }
}

async function handleAccept(id: number) {
    await store.acceptSuggestion(id);
}

async function handleReject(id: number) {
    await store.rejectSuggestion(id);
}

/**
 * 批量接受当前选中的普通学习建议，成功后清空选择集合。
 */
async function handleBatchAccept() {
    if (selectedIds.value.length === 0) return;
    await store.batchAcceptSuggestions([...selectedIds.value]);
    selectedIds.value = [];
}

/**
 * 切换学习规则启用状态，具体持久化和错误处理由 learning store 负责。
 */
async function handleToggleRule(ruleId: number, enabled: boolean) {
    await store.toggleRule(ruleId, enabled);
}

/**
 * 删除指定学习规则，保持规则列表刷新逻辑集中在 learning store。
 */
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

/**
 * 保存学习规则编辑弹窗，成功后关闭弹窗并重新加载规则列表。
 */
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

function llmStatusColor(status: string): string {
    return getLearningStatusColor(status);
}

function llmStatusLabel(status: string): string {
    return getTranslatedLearningStatusLabel(status, tt);
}

useExternalTemplateBindings(
    SettingsJsonImportExportButton,
    getSuggestionFeatureChips,
    getRuleFeatureChips,
    confidenceColor,
    mdiRefresh,
    mdiCheck,
    mdiClose,
    mdiDelete,
    mdiPencil,
    mdiLightbulbOutline,
    mdiBookOpenPageVariant,
    mdiRobotOutline,
    mdiChevronDown,
    mdiFilterVariant,
    ruleFeatureFilterMenu,
    ruleLearnedActionFilterMenu,
    llmSavedConfigs,
    llmStatusFilter,
    selectedLLMIds,
    llmProviderOptions,
    llmReasoningDepthOptions,
    llmCredentialModeOptions,
    addConfigDialog,
    addConfigSaving,
    autofillFieldsLocked,
    newConfigForm,
    selectedLLMProviderOption,
    baseUrlFieldLabel,
    llmConfigFieldNames,
    llmPendingCount,
    llmStatusOptions,
    filteredLLMCandidates,
    selectAllLLM,
    llmIndeterminate,
    unlockAutofillFields,
    closeAddConfigDialog,
    openAddConfigDialog,
    saveNewConfig,
    llmProviderLabel,
    handleActivateConfig,
    handleDeleteConfig,
    handleLLMGenerate,
    handleLLMBatchAccept,
    handleLLMAccept,
    handleLLMReject,
    loading,
    error,
    rulesTotal,
    hideSectionTitle,
    hasHeaderActionsTarget,
    headerActionsTarget,
    suggestionStatusOptions,
    ruleMatchTypeOptions,
    ruleEnabledFilterOptions,
    ruleAppliedFilterOptions,
    ruleFeatureFilterActive,
    ruleLearnedActionFilterActive,
    setRuleMatchTypeFilter,
    clearRuleFeatureFilter,
    clearRuleLearnedActionFilter,
    setRuleEnabledFilter,
    setRuleAppliedFilter,
    filteredRules,
    selectAll,
    indeterminate,
    clearError,
    statusColor,
    statusLabel,
    translateMatchType,
    refreshCurrentTab,
    handleGenerate,
    handleAccept,
    handleReject,
    handleBatchAccept,
    handleToggleRule,
    handleDeleteRule,
    openEditRuleDialog,
    saveEditRule,
    llmStatusColor,
    llmStatusLabel,
);

watch(
    () => props.initTab,
    (initTab) => {
        activeTab.value = normalizeLearningPanelTab(initTab);
    },
    { immediate: true }
);

watch(activeTab, (tab) => {
    if (tab === 'suggestions') {
        store.loadSuggestions();
    } else if (tab === 'llm') {
        loadLLMCandidates();
    } else if (tab === 'llm-config') {
        loadLLMConfigs();
    } else {
        store.loadRules();
    }
}, { immediate: true });
</script>

<style scoped src="./learning-center/LearningCenterPanel.scss"></style>
