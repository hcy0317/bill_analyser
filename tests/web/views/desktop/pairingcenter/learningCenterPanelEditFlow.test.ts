import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockLoadRules = jest.fn<() => Promise<void>>();
const mockUpdateLearningRule = jest.fn<(...args: Array<unknown>) => Promise<any>>();

const mockLearningStore = {
    error: null as string | null,
    suggestionsLoading: false,
    rulesLoading: false,
    suggestionsTotal: 0,
    rulesTotal: 1,
    suggestions: [],
    rules: [],
    loadSuggestions: jest.fn<() => Promise<void>>(),
    loadRules: mockLoadRules,
    generateSuggestions: jest.fn<() => Promise<null>>(),
    acceptSuggestion: jest.fn<() => Promise<void>>(),
    rejectSuggestion: jest.fn<() => Promise<void>>(),
    batchAcceptSuggestions: jest.fn<() => Promise<void>>(),
    toggleRule: jest.fn<() => Promise<void>>(),
    deleteRule: jest.fn<() => Promise<void>>()
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null)
    };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key
    })
}));

jest.mock('@/stores/learning.ts', () => ({
    useLearningStore: () => mockLearningStore
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        updateLearningRule: mockUpdateLearningRule
    }
}));

jest.mock('@/views/desktop/pairingcenter/components/learning-center/useLearningCenterLlmConfig.ts', () => ({
    useLearningCenterLlmConfig: () => {
        const { computed, ref } = jest.requireActual('vue') as any;
        return {
            llmLoading: ref(false),
            llmConfigLoading: ref(false),
            llmSavedConfigs: ref([]),
            llmStatusFilter: ref(''),
            selectedLLMIds: ref([]),
            llmProviderOptions: computed(() => []),
            llmReasoningDepthOptions: computed(() => []),
            llmCredentialModeOptions: computed(() => []),
            addConfigDialog: ref(false),
            addConfigSaving: ref(false),
            autofillFieldsLocked: ref(false),
            newConfigForm: ref({}),
            selectedLLMProviderOption: computed(() => null),
            baseUrlFieldLabel: computed(() => ''),
            llmConfigFieldNames: computed(() => []),
            llmPendingCount: computed(() => 0),
            llmStatusOptions: computed(() => []),
            filteredLLMCandidates: computed(() => []),
            selectAllLLM: computed({ get: () => false, set: () => undefined }),
            llmIndeterminate: computed(() => false),
            unlockAutofillFields: jest.fn(),
            closeAddConfigDialog: jest.fn(),
            loadLLMConfigs: jest.fn<() => Promise<void>>(),
            openAddConfigDialog: jest.fn(),
            saveNewConfig: jest.fn<() => Promise<void>>(),
            llmProviderLabel: jest.fn(),
            handleActivateConfig: jest.fn<() => Promise<void>>(),
            handleDeleteConfig: jest.fn<() => Promise<void>>(),
            loadLLMCandidates: jest.fn<() => Promise<void>>(),
            handleLLMGenerate: jest.fn<() => Promise<void>>(),
            handleLLMBatchAccept: jest.fn<() => Promise<void>>(),
            handleLLMAccept: jest.fn<() => Promise<void>>(),
            handleLLMReject: jest.fn<() => Promise<void>>()
        };
    }
}));

for (const componentPath of [
    '@/components/desktop/SnackBar.vue',
    '@/components/desktop/SettingsJsonImportExportButton.vue'
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: { name: 'LearningCenterEditFlowStub' }
    }));
}

import LearningCenterPanel from '@/views/desktop/pairingcenter/components/LearningCenterPanel.vue';

beforeEach(() => {
    jest.clearAllMocks();
    mockLearningStore.error = null;
    mockLoadRules.mockResolvedValue(undefined);
    mockUpdateLearningRule.mockResolvedValue({
        data: {
            success: true,
            result: {
                id: 42,
                matchValue: 'coffee shop',
                learnedType: 'Expense'
            }
        }
    });
});

describe('LearningCenterPanel edit flow', () => {
    test('submits the live update payload, closes the dialog, and reloads rules', async () => {
        const bindings = (LearningCenterPanel as any).setup({
            initTab: 'rules',
            hideSectionTitle: false,
            headerActionsTarget: ''
        }, { emit: jest.fn(), expose: jest.fn() });
        mockLoadRules.mockClear();

        bindings.openEditRuleDialog({
            id: 42,
            matchValue: 'coffee',
            learnedType: 'Expense'
        });
        expect(bindings.editRuleDialog.value).toBe(true);
        expect(bindings.editRuleForm.value).toStrictEqual({
            id: 42,
            matchValue: 'coffee',
            learnedType: 'Expense'
        });

        bindings.editRuleForm.value.matchValue = 'coffee shop';
        const savePromise = bindings.saveEditRule();
        expect(bindings.editRuleSaving.value).toBe(true);
        await savePromise;

        expect(mockUpdateLearningRule).toHaveBeenCalledTimes(1);
        expect(mockUpdateLearningRule).toHaveBeenCalledWith({
            ruleId: 42,
            matchValue: 'coffee shop',
            learnedType: 'Expense'
        });
        expect(bindings.editRuleDialog.value).toBe(false);
        expect(mockLoadRules).toHaveBeenCalledTimes(1);
        expect(mockLearningStore.error).toBeNull();
        expect(bindings.editRuleSaving.value).toBe(false);
    });
});
