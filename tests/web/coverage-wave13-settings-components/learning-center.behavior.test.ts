/* eslint-disable @typescript-eslint/no-explicit-any */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const actualVue = jest.requireActual('vue') as any;
const mockSnackbarMessage = jest.fn();
const mockLoadSuggestions = jest.fn<() => Promise<void>>();
const mockLoadRules = jest.fn<() => Promise<void>>();
const mockGenerateSuggestions = jest.fn<() => Promise<any>>();
const mockAcceptSuggestion = jest.fn<() => Promise<void>>();
const mockRejectSuggestion = jest.fn<() => Promise<void>>();
const mockBatchAccept = jest.fn<(...args: any[]) => Promise<void>>();
const mockToggleRule = jest.fn<(...args: any[]) => Promise<void>>();
const mockDeleteRule = jest.fn<(...args: any[]) => Promise<void>>();
const mockUpdateRule = jest.fn<(...args: any[]) => Promise<any>>();
const mockLoadLlmCandidates = jest.fn<() => Promise<void>>();
const mockLoadLlmConfigs = jest.fn<() => Promise<void>>();
const mockExternalBindings = jest.fn();

const mockStore = actualVue.reactive({
    error: null as string | null,
    suggestionsLoading: false,
    rulesLoading: false,
    suggestionsTotal: 3,
    rulesTotal: 2,
    suggestions: [
        { id: 1, status: 'pending' },
        { id: 2, status: 'pending' },
        { id: 3, status: 'accepted' },
    ],
    rules: [{ id: 10, matchType: 'merchant', matchValue: 'Cafe', learnedType: 'Expense', enabled: true }],
    loadSuggestions: mockLoadSuggestions,
    loadRules: mockLoadRules,
    generateSuggestions: mockGenerateSuggestions,
    acceptSuggestion: mockAcceptSuggestion,
    rejectSuggestion: mockRejectSuggestion,
    batchAcceptSuggestions: mockBatchAccept,
    toggleRule: mockToggleRule,
    deleteRule: mockDeleteRule,
});

jest.mock('vue', () => ({
    ...actualVue,
    useTemplateRef: () => actualVue.ref({ showMessage: mockSnackbarMessage }),
}));
jest.mock('@/locales/helpers.ts', () => ({ useI18n: () => ({ tt: (key: string) => `tt:${key}` }) }));
jest.mock('@/stores/learning.ts', () => ({ useLearningStore: () => mockStore }));
jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: (...args: any[]) => mockExternalBindings(...args),
}));
jest.mock('@/models/learning_center.ts', () => ({
    getSuggestionFeatureChips: jest.fn(),
    getRuleFeatureChips: jest.fn(),
    getRuleFeatureSummary: (rule: any) => `${rule.matchValue}:${rule.learnedType}`,
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: { updateLearningRule: mockUpdateRule },
}));
jest.mock('@/views/desktop/pairingcenter/components/apiResultHelpers.ts', () => ({
    extractPayloadMessage: (payload: any) => payload?.message ?? '',
    getRequestErrorMessage: (error: any, fallback: string) => error?.message ?? fallback,
}));
jest.mock('@/views/desktop/pairingcenter/components/learningCenterPanelModel.ts', () => ({
    confidenceColor: jest.fn(),
    createLearningRuleMatchTypeOptions: (rules: any[]) => rules.map(rule => ({ title: rule.matchType, value: rule.matchType })),
    createRuleAppliedFilterOptions: () => [{ title: 'All', value: 'all' }],
    createRuleEnabledFilterOptions: () => [{ title: 'All', value: 'all' }],
    filterLearningRules: (rules: any[], filters: any) => rules.filter(rule => (
        !filters.matchType || rule.matchType === filters.matchType
    )),
    getLearningStatusColor: (status: string) => `color:${status}`,
    getLearningStatusLabel: (status: string) => `label:${status}`,
    getTranslatedLearningStatusLabel: (status: string) => `translated:${status}`,
    normalizeLearningPanelTab: (tab?: string) => ['suggestions', 'rules', 'llm', 'llm-config'].includes(tab ?? '') ? tab : 'rules',
    translateLearningMatchType: (type: string) => `match:${type}`,
}));
jest.mock('@/views/desktop/pairingcenter/components/learning-center/useLearningCenterLlmConfig.ts', () => ({
    useLearningCenterLlmConfig: () => ({
        llmLoading: actualVue.ref(false),
        llmConfigLoading: actualVue.ref(false),
        llmSavedConfigs: actualVue.ref([]),
        llmStatusFilter: actualVue.ref(''),
        selectedLLMIds: actualVue.ref([]),
        llmProviderOptions: actualVue.computed(() => []),
        llmReasoningDepthOptions: actualVue.computed(() => []),
        llmCredentialModeOptions: actualVue.computed(() => []),
        llmOAuthCredentialModeOptions: actualVue.computed(() => []),
        llmConnectionMode: actualVue.ref('api'),
        addConfigDialog: actualVue.ref(false),
        addConfigSaving: actualVue.ref(false),
        testingConfigId: actualVue.ref(null),
        autofillFieldsLocked: actualVue.ref(false),
        newConfigForm: actualVue.ref({}),
        selectedLLMProviderOption: actualVue.computed(() => null),
        baseUrlFieldLabel: actualVue.computed(() => ''),
        llmConfigFieldNames: actualVue.computed(() => []),
        llmPendingCount: actualVue.computed(() => 0),
        llmStatusOptions: actualVue.computed(() => []),
        filteredLLMCandidates: actualVue.computed(() => []),
        selectAllLLM: actualVue.computed({ get: () => false, set: () => undefined }),
        llmIndeterminate: actualVue.computed(() => false),
        unlockAutofillFields: jest.fn(), closeAddConfigDialog: jest.fn(),
        loadLLMConfigs: mockLoadLlmConfigs, openAddConfigDialog: jest.fn(), saveNewConfig: jest.fn(),
        llmProviderLabel: jest.fn(), handleActivateConfig: jest.fn(), handleTestConfig: jest.fn(), handleDeleteConfig: jest.fn(),
        loadLLMCandidates: mockLoadLlmCandidates, handleLLMGenerate: jest.fn(), handleLLMBatchAccept: jest.fn(),
        handleLLMAccept: jest.fn(), handleLLMReject: jest.fn(),
    }),
}));
for (const path of ['@/components/desktop/SnackBar.vue', '@/components/desktop/SettingsJsonImportExportButton.vue']) {
    jest.mock(path, () => ({ __esModule: true, default: { name: 'LearningCenterStub' } }));
}
jest.mock('@mdi/js', () => new Proxy({}, { get: (_target, property) => String(property) }));

import LearningCenterPanel from '@/views/desktop/pairingcenter/components/LearningCenterPanel.vue';

function setup(initTab = 'rules'): any {
    return (LearningCenterPanel as any).setup(actualVue.reactive({
        initTab, hideSectionTitle: false, headerActionsTarget: '',
    }), { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() });
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockStore.error = null;
    mockStore.suggestionsLoading = false;
    mockStore.rulesLoading = false;
    mockStore.suggestions = [
        { id: 1, status: 'pending' }, { id: 2, status: 'pending' }, { id: 3, status: 'accepted' },
    ];
    mockStore.rules = [{ id: 10, matchType: 'merchant', matchValue: 'Cafe', learnedType: 'Expense', enabled: true }];
    mockLoadSuggestions.mockResolvedValue(undefined);
    mockLoadRules.mockResolvedValue(undefined);
    mockLoadLlmCandidates.mockResolvedValue(undefined);
    mockLoadLlmConfigs.mockResolvedValue(undefined);
    mockGenerateSuggestions.mockResolvedValue(null);
    mockAcceptSuggestion.mockResolvedValue(undefined);
    mockRejectSuggestion.mockResolvedValue(undefined);
    mockBatchAccept.mockResolvedValue(undefined);
    mockToggleRule.mockResolvedValue(undefined);
    mockDeleteRule.mockResolvedValue(undefined);
    mockUpdateRule.mockResolvedValue({ data: { success: true, result: {} } });
});

describe('LearningCenterPanel production behavior', () => {
    test('keeps external template bindings and defers same-tick header teleports', () => {
        const source = readFileSync(resolve(
            __dirname,
            '../../../src/web/src/views/desktop/pairingcenter/components/LearningCenterPanel.vue',
        ), 'utf8');
        const template = readFileSync(resolve(
            __dirname,
            '../../../src/web/src/views/desktop/pairingcenter/components/learning-center/LearningCenterPanel.template.html',
        ), 'utf8');

        expect(source).toContain('<template src="./learning-center/LearningCenterPanel.template.html"></template>');
        expect(source).toContain('<script setup lang="ts">');
        expect(source).toContain('useExternalTemplateBindings(');
        expect(template.match(/<Teleport defer/g)).toHaveLength(4);
        expect(template).not.toContain('<v-tabs v-model="llmConnectionMode"');
        expect(template).not.toContain('<v-window v-model="llmConnectionMode"');
        expect(template).toContain("tt('Other sign-in methods (expert)')");
        expect(template).toContain('v-model="newConfigForm.credential_mode"');
        expect(template).toContain('v-model="newConfigForm.credential_json"');
        expect(template).not.toContain('newConfigForm.token_endpoint');
        expect(template).not.toContain('newConfigForm.refresh_headers');
        expect(template).not.toContain('newConfigForm.refresh_body');
        expect(template).not.toContain('newConfigForm.refresh_params');
        expect(template).toContain('class="llm-config-dialog-actions');
    });

    test('scopes loading to the active tab and projects error, header, suggestion, and rule filter state', () => {
        const bindings = setup('rules');
        expect(bindings.activeTab.value).toBe('rules');
        expect(bindings.loading.value).toBe(false);
        mockStore.suggestionsLoading = true;
        expect(bindings.loading.value).toBe(false);
        mockStore.suggestionsLoading = false;
        mockStore.rulesLoading = true;
        expect(bindings.loading.value).toBe(true);
        mockStore.rulesLoading = false;

        bindings.activeTab.value = 'suggestions';
        mockStore.suggestionsLoading = true;
        expect(bindings.loading.value).toBe(true);
        mockStore.suggestionsLoading = false;

        bindings.activeTab.value = 'llm';
        bindings.llmLoading.value = true;
        expect(bindings.loading.value).toBe(true);
        bindings.llmLoading.value = false;

        bindings.activeTab.value = 'llm-config';
        bindings.llmConfigLoading.value = true;
        expect(bindings.loading.value).toBe(true);
        bindings.llmConfigLoading.value = false;
        mockStore.error = 'failed';
        expect(bindings.error.value).toBe('failed');
        expect(bindings.hideSectionTitle.value).toBe(false);
        expect(bindings.hasHeaderActionsTarget.value).toBe(false);
        expect(bindings.headerActionsTarget.value).toBe('body');
        expect(bindings.suggestionStatusOptions.value[0]).toEqual({ title: 'tt:All (3)', value: '' });
        expect(bindings.filteredSuggestions.value).toHaveLength(3);
        bindings.statusFilter.value = 'pending';
        expect(bindings.filteredSuggestions.value).toHaveLength(2);
        expect(bindings.ruleMatchTypeOptions.value).toEqual([{ title: 'merchant', value: 'merchant' }]);
        expect(bindings.ruleEnabledFilterOptions.value).toHaveLength(1);
        expect(bindings.ruleAppliedFilterOptions.value).toHaveLength(1);
    });

    test('hides the visible loading indicator when the shell supplies an unsupported tab', () => {
        const template = readFileSync(resolve(
            __dirname,
            '../../../src/web/src/views/desktop/pairingcenter/components/learning-center/LearningCenterPanel.template.html',
        ), 'utf8');
        mockStore.rulesLoading = true;
        const bindings = setup('rules');

        expect(template).toContain('<v-progress-linear v-if="loading"');
        expect(bindings.loading.value).toBe(true);
        bindings.activeTab.value = 'unsupported';
        expect(bindings.loading.value).toBe(false);
    });

    test('updates rule filters and selection states including indeterminate branches', () => {
        const bindings = setup('rules');
        bindings.ruleMatchTypeFilterMenu.value = true;
        bindings.setRuleMatchTypeFilter('merchant');
        expect(bindings.ruleMatchTypeFilter.value).toBe('merchant');
        expect(bindings.ruleMatchTypeFilterMenu.value).toBe(false);
        bindings.ruleFeatureFilter.value = ' cafe ';
        bindings.ruleLearnedActionFilter.value = ' expense ';
        expect(bindings.ruleFeatureFilterActive.value).toBe(true);
        expect(bindings.ruleLearnedActionFilterActive.value).toBe(true);
        bindings.clearRuleFeatureFilter();
        bindings.clearRuleLearnedActionFilter();
        expect(bindings.ruleFeatureFilterActive.value).toBe(false);
        expect(bindings.ruleLearnedActionFilterActive.value).toBe(false);
        bindings.setRuleEnabledFilter('enabled');
        bindings.setRuleAppliedFilter('applied');
        expect(bindings.ruleEnabledFilterMenu.value).toBe(false);
        expect(bindings.ruleAppliedFilterMenu.value).toBe(false);
        expect(bindings.filteredRules.value).toHaveLength(1);

        expect(bindings.selectAll.value).toBe(false);
        bindings.selectedIds.value = [1];
        expect(bindings.indeterminate.value).toBe(true);
        bindings.selectAll.value = true;
        expect(bindings.selectedIds.value).toEqual([1, 2]);
        expect(bindings.selectAll.value).toBe(true);
        expect(bindings.indeterminate.value).toBe(false);
        bindings.selectAll.value = false;
        expect(bindings.selectedIds.value).toEqual([]);
        mockStore.suggestions = [{ id: 3, status: 'accepted' }];
        expect(bindings.selectAll.value).toBe(false);
    });

    test('refreshes each data source and executes suggestion and rule actions', async () => {
        const bindings = setup('rules');
        mockLoadRules.mockClear();
        for (const tab of ['suggestions', 'llm', 'llm-config', 'rules']) {
            bindings.activeTab.value = tab;
            await bindings.refreshCurrentTab();
        }
        expect(mockLoadSuggestions).toHaveBeenCalled();
        expect(mockLoadLlmCandidates).toHaveBeenCalled();
        expect(mockLoadLlmConfigs).toHaveBeenCalled();
        expect(mockLoadRules).toHaveBeenCalled();

        await bindings.handleGenerate();
        expect(mockSnackbarMessage).not.toHaveBeenCalled();
        mockGenerateSuggestions.mockResolvedValueOnce({ created: 2, updated: 1 });
        await bindings.handleGenerate();
        expect(mockSnackbarMessage).toHaveBeenCalledWith('Generated Suggestions Summary', { created: 2, updated: 1 });
        await bindings.handleAccept(1);
        await bindings.handleReject(2);
        await bindings.handleBatchAccept();
        expect(mockBatchAccept).not.toHaveBeenCalled();
        bindings.selectedIds.value = [1, 2];
        await bindings.handleBatchAccept();
        expect(mockBatchAccept).toHaveBeenCalledWith([1, 2]);
        expect(bindings.selectedIds.value).toEqual([]);
        await bindings.handleToggleRule(10, false);
        await bindings.handleDeleteRule(10);
        expect(mockToggleRule).toHaveBeenCalledWith(10, false);
        expect(mockDeleteRule).toHaveBeenCalledWith(10);
    });

    test('maps labels, payload errors, info messages, and clears the store error', () => {
        const bindings = setup();
        bindings.showInfoMessage('hello', { color: 'info' });
        expect(mockSnackbarMessage).toHaveBeenCalledWith('hello', { color: 'info' });
        expect(bindings.getPayloadErrorMessage({ message: 'payload failed' }, 'fallback')).toBe('payload failed');
        expect(bindings.getPayloadErrorMessage({}, 'fallback')).toBe('fallback');
        expect(bindings.statusColor('pending')).toBe('color:pending');
        expect(bindings.statusLabel('pending')).toBe('label:pending');
        expect(bindings.translateMatchType('merchant')).toBe('match:merchant');
        expect(bindings.llmStatusColor('accepted')).toBe('color:accepted');
        expect(bindings.llmStatusLabel('accepted')).toBe('translated:accepted');
        mockStore.error = 'old';
        bindings.clearError();
        expect(mockStore.error).toBeNull();
    });

    test('saves edited rules across success, payload failure, and request failure paths', async () => {
        const bindings = setup();
        mockLoadRules.mockClear();
        bindings.openEditRuleDialog({ id: 10, matchValue: '', learnedType: undefined });
        expect(bindings.editRuleForm.value).toEqual({ id: 10, matchValue: '', learnedType: '' });
        bindings.editRuleForm.value = { id: 10, matchValue: 'Cafe', learnedType: 'Expense' };
        await bindings.saveEditRule();
        expect(mockUpdateRule).toHaveBeenCalledWith({ ruleId: 10, matchValue: 'Cafe', learnedType: 'Expense' });
        expect(bindings.editRuleDialog.value).toBe(false);
        expect(mockLoadRules).toHaveBeenCalled();
        expect(bindings.editRuleSaving.value).toBe(false);

        mockUpdateRule.mockResolvedValueOnce({ data: { success: false, result: { message: 'invalid rule' } } });
        await bindings.saveEditRule();
        expect(mockStore.error).toBe('invalid rule');

        mockUpdateRule.mockRejectedValueOnce({ message: 'network failed' });
        await bindings.saveEditRule();
        expect(mockStore.error).toBe('network failed');
        expect(bindings.editRuleSaving.value).toBe(false);
    });

    test('reacts to active tab changes and exposes external template bindings', async () => {
        const bindings = setup('suggestions');
        await flush();
        mockLoadSuggestions.mockClear();
        mockLoadRules.mockClear();
        mockLoadLlmCandidates.mockClear();
        mockLoadLlmConfigs.mockClear();
        bindings.activeTab.value = 'llm';
        await flush();
        bindings.activeTab.value = 'llm-config';
        await flush();
        bindings.activeTab.value = 'rules';
        await flush();
        bindings.activeTab.value = 'suggestions';
        await flush();
        expect(mockLoadLlmCandidates).toHaveBeenCalled();
        expect(mockLoadLlmConfigs).toHaveBeenCalled();
        expect(mockLoadRules).toHaveBeenCalled();
        expect(mockLoadSuggestions).toHaveBeenCalled();
        expect(mockExternalBindings).toHaveBeenCalled();
    });
});
