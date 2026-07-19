import { computed, onBeforeUnmount, ref } from 'vue';

import services from '@/lib/services.ts';

import {
    buildAdvancedSettingsPayload,
    buildCredentialConfigPayload,
    createEmptyLLMConfigForm,
    createLLMCredentialModeOptions,
    createLLMProviderOptions,
    createLLMReasoningDepthOptions,
    getLLMProviderLabel,
    toLLMCandidates,
    toLLMConfigs,
    type LLMCandidateItem,
    type LLMConfigForm,
    type LLMConfigItem,
    type LLMProviderOption,
    type SelectOption,
} from '../llmConfigHelpers.ts';

interface LearningCenterLlmConfigOptions {
    tt: (key: string) => string;
    setError: (message: string) => void;
    showInfoMessage: (message: string, options?: Record<string, unknown>) => void;
    getPayloadErrorMessage: (payload: unknown, fallback: string) => string;
    getRequestErrorMessage: (requestError: unknown, fallback: string) => string;
}

/**
 * 管理学习中心 LLM 配置、候选审核和浏览器 autofill 防护状态。
 *
 * 该 composable 保持原面板使用的变量名和 action 语义不变，只把 LLM 子域状态
 * 从页面 facade 中下沉，避免页面组件继续承担 provider 配置和候选审核细节。
 */
export function useLearningCenterLlmConfig(options: LearningCenterLlmConfigOptions) {
    const {
        tt,
        setError,
        showInfoMessage,
        getPayloadErrorMessage,
        getRequestErrorMessage,
    } = options;

    const llmLoading = ref(false);
    const llmConfigLoading = ref(false);
    const llmSavedConfigs = ref<LLMConfigItem[]>([]);
    const llmCandidates = ref<LLMCandidateItem[]>([]);
    const llmStatusFilter = ref<string>('');
    const selectedLLMIds = ref<number[]>([]);

    const llmProviderOptions: LLMProviderOption[] = createLLMProviderOptions(tt);
    const defaultLLMProviderOption = llmProviderOptions[0] as LLMProviderOption;
    const llmReasoningDepthOptions = createLLMReasoningDepthOptions(tt);
    const llmCredentialModeOptions = createLLMCredentialModeOptions(tt);
    const llmOAuthCredentialModeOptions = llmCredentialModeOptions.filter(option => option.value !== 'api_key');

    const addConfigDialog = ref(false);
    const addConfigSaving = ref(false);
    const testingConfigId = ref<number | null>(null);
    const autofillNonce = ref(Date.now());
    const autofillFieldsLocked = ref(false);
    const autofillUnlockTimer = ref<number | null>(null);
    const newConfigForm = ref<LLMConfigForm>(createEmptyLLMConfigForm());

    const selectedLLMProviderOption = computed<LLMProviderOption>(() => (
        llmProviderOptions.find(option => option.value === newConfigForm.value.provider) ?? defaultLLMProviderOption
    ));

    const llmConnectionMode = computed<'api' | 'oauth'>({
        get: () => newConfigForm.value.credential_mode === 'api_key' ? 'api' : 'oauth',
        set: (mode) => {
            if (mode === 'api') {
                newConfigForm.value.credential_mode = 'api_key';
            } else if (newConfigForm.value.credential_mode === 'api_key') {
                newConfigForm.value.credential_mode = 'session_json';
            }
        },
    });

    const baseUrlFieldLabel = computed(() => (
        selectedLLMProviderOption.value.requiresBaseUrl ? tt('Base URL') : tt('Base URL (optional)')
    ));

    const llmConfigFieldNames = computed(() => ({
        name: `llm-config-label-${autofillNonce.value}`,
        provider: `llm-config-provider-${autofillNonce.value}`,
        model: `llm-config-model-${autofillNonce.value}`,
        apiKey: `llm-config-credential-${autofillNonce.value}`,
        baseUrl: `llm-config-endpoint-${autofillNonce.value}`,
        credentialJson: `llm-config-credential-json-${autofillNonce.value}`,
        temperature: `llm-config-temperature-${autofillNonce.value}`,
        maxTokens: `llm-config-max-tokens-${autofillNonce.value}`,
        systemPrompt: `llm-config-system-prompt-${autofillNonce.value}`,
        classificationPrompt: `llm-config-classification-prompt-${autofillNonce.value}`,
        rulePrompt: `llm-config-rule-prompt-${autofillNonce.value}`,
    }));

    const llmPendingCount = computed(() =>
        llmCandidates.value.filter(candidate => candidate.status === 'pending').length
    );

    const llmStatusOptions = computed<SelectOption[]>(() => [
        { title: tt('All'), value: '' },
        { title: tt('Pending'), value: 'pending' },
        { title: tt('Accepted'), value: 'accepted' },
        { title: tt('Rejected'), value: 'rejected' },
    ]);

    const filteredLLMCandidates = computed(() => {
        if (!llmStatusFilter.value) return llmCandidates.value;
        return llmCandidates.value.filter(candidate => candidate.status === llmStatusFilter.value);
    });

    const selectableLLMCandidates = computed(() =>
        filteredLLMCandidates.value.filter(candidate => candidate.status === 'pending')
    );

    const selectAllLLM = computed({
        get() {
            return selectableLLMCandidates.value.length > 0
                && selectableLLMCandidates.value.every(candidate => selectedLLMIds.value.includes(candidate.id));
        },
        set(val: boolean) {
            if (val) {
                selectedLLMIds.value = selectableLLMCandidates.value.map(candidate => candidate.id);
            } else {
                selectedLLMIds.value = [];
            }
        }
    });

    const llmIndeterminate = computed(() => {
        const selectedCount = selectableLLMCandidates.value
            .filter(candidate => selectedLLMIds.value.includes(candidate.id))
            .length;
        return selectedCount > 0 && selectedCount < selectableLLMCandidates.value.length;
    });

    function clearAutofillUnlockTimer(): void {
        if (autofillUnlockTimer.value === null) {
            return;
        }
        window.clearTimeout(autofillUnlockTimer.value);
        autofillUnlockTimer.value = null;
    }

    function unlockAutofillFields(): void {
        autofillFieldsLocked.value = false;
        clearAutofillUnlockTimer();
    }

    function lockAutofillFieldsBriefly(): void {
        clearAutofillUnlockTimer();
        autofillFieldsLocked.value = true;
        autofillUnlockTimer.value = window.setTimeout(() => {
            autofillFieldsLocked.value = false;
            autofillUnlockTimer.value = null;
        }, 350);
    }

    function closeAddConfigDialog(): void {
        clearAutofillUnlockTimer();
        autofillFieldsLocked.value = false;
        addConfigDialog.value = false;
    }

    /**
     * 加载已保存 LLM 配置列表，进入组件状态前移除敏感 api_key 字段。
     */
    async function loadLLMConfigs() {
        llmConfigLoading.value = true;
        try {
            const resp = await services.getLLMConfigs();
            if (resp.data?.success && resp.data.result) {
                llmSavedConfigs.value = toLLMConfigs(resp.data.result);
            }
        } catch { /* ignore config load errors */ }
        finally {
            llmConfigLoading.value = false;
        }
    }

    function openAddConfigDialog() {
        autofillNonce.value = Date.now();
        newConfigForm.value = createEmptyLLMConfigForm();
        addConfigDialog.value = true;
        lockAutofillFieldsBriefly();
    }

    /**
     * 创建新的 LLM 配置，提交前校验凭据 JSON、必填名称和 provider base URL 约束。
     */
    async function saveNewConfig() {
        const form = newConfigForm.value;
        let credentialConfig: Record<string, unknown>;
        try {
            credentialConfig = buildCredentialConfigPayload(form);
        } catch (error: unknown) {
            setError(error instanceof Error ? error.message : 'Invalid credential config');
            return;
        }
        const payload = {
            name: form.name.trim(),
            provider: form.provider,
            model: form.model.trim(),
            api_key: form.credential_mode === 'api_key' ? form.api_key.trim() : '',
            base_url: form.base_url.trim(),
            credential_config: credentialConfig,
            advanced_settings: buildAdvancedSettingsPayload(form),
            is_active: llmSavedConfigs.value.length === 0,
        };

        if (!payload.name) {
            setError('Name is required');
            return;
        }

        if (selectedLLMProviderOption.value.requiresBaseUrl && !payload.base_url) {
            setError(`${selectedLLMProviderOption.value.title} requires a Base URL`);
            return;
        }

        addConfigSaving.value = true;
        try {
            const resp = await services.createLLMConfig(payload);
            if (resp.data?.success) {
                closeAddConfigDialog();
                newConfigForm.value = createEmptyLLMConfigForm();
                await loadLLMConfigs();
            } else {
                setError(getPayloadErrorMessage(resp.data?.result, 'Failed to create config'));
            }
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to create config'));
        } finally {
            addConfigSaving.value = false;
        }
    }

    function llmProviderLabel(provider: string): string {
        return getLLMProviderLabel(provider, llmProviderOptions);
    }

    /**
     * 激活指定 LLM 配置并刷新配置列表，保证当前 active 状态来自服务端。
     */
    async function handleActivateConfig(configId: number) {
        try {
            await services.activateLLMConfig(configId);
            await loadLLMConfigs();
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to activate config'));
        }
    }

    /** 使用服务端保存的密钥执行最小请求，验证端点、认证和模型是否共同可用。 */
    async function handleTestConfig(configId: number) {
        testingConfigId.value = configId;
        try {
            const resp = await services.testLLMConfig(configId);
            if (!resp.data?.success) {
                setError(getPayloadErrorMessage(resp.data, 'Failed to test LLM config'));
                return;
            }
            const latencyMs = Number(resp.data?.result?.latency_ms ?? 0);
            showInfoMessage('LLM connection test succeeded', {
                latency: latencyMs > 0 ? `${latencyMs} ms` : undefined,
            });
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to test LLM config'));
        } finally {
            testingConfigId.value = null;
        }
    }

    /**
     * 删除指定 LLM 配置并刷新配置列表。
     */
    async function handleDeleteConfig(configId: number) {
        try {
            await services.deleteLLMConfig(configId);
            await loadLLMConfigs();
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to delete config'));
        }
    }

    /**
     * 加载 LLM 规则候选并剔除已不再可选的本地选中 id。
     */
    async function loadLLMCandidates() {
        llmLoading.value = true;
        try {
            const resp = await services.getLLMCandidates({ type: 'rule_synthesis', limit: 100 });
            if (resp.data?.success && resp.data.result) {
                llmCandidates.value = toLLMCandidates(resp.data.result);
                selectedLLMIds.value = selectedLLMIds.value.filter(id =>
                    llmCandidates.value.some(candidate => candidate.id === id && candidate.status === 'pending')
                );
            }
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to load LLM candidates'));
        } finally {
            llmLoading.value = false;
        }
    }

    /**
     * 触发 LLM 规则候选生成，并在完成后刷新候选列表。
     */
    async function handleLLMGenerate() {
        llmLoading.value = true;
        try {
            const resp = await services.generateLLMRuleSynthesis({ limit: 8 });
            const created = resp.data?.result?.candidates_created ?? 0;
            showInfoMessage('Generated LLM Rule Candidates Summary', { count: created });
            await loadLLMCandidates();
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to generate LLM rule candidates'));
        } finally {
            llmLoading.value = false;
        }
    }

    /**
     * 逐条接受选中的 LLM 规则候选，全部完成后清空选择并刷新候选列表。
     */
    async function handleLLMBatchAccept() {
        const ids = [...selectedLLMIds.value];
        if (ids.length === 0) {
            return;
        }

        llmLoading.value = true;
        try {
            for (const id of ids) {
                await services.acceptLLMCandidate(id);
            }
            selectedLLMIds.value = [];
            await loadLLMCandidates();
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to batch accept LLM rule candidates'));
        } finally {
            llmLoading.value = false;
        }
    }

    /**
     * 接受单条 LLM 候选并移出本地选择集合。
     */
    async function handleLLMAccept(id: number) {
        llmLoading.value = true;
        try {
            await services.acceptLLMCandidate(id);
            selectedLLMIds.value = selectedLLMIds.value.filter(selectedId => selectedId !== id);
            await loadLLMCandidates();
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to accept candidate'));
        } finally {
            llmLoading.value = false;
        }
    }

    /**
     * 拒绝单条 LLM 候选并移出本地选择集合。
     */
    async function handleLLMReject(id: number) {
        llmLoading.value = true;
        try {
            await services.rejectLLMCandidate(id);
            selectedLLMIds.value = selectedLLMIds.value.filter(selectedId => selectedId !== id);
            await loadLLMCandidates();
        } catch (error: unknown) {
            setError(getRequestErrorMessage(error, 'Failed to reject candidate'));
        } finally {
            llmLoading.value = false;
        }
    }

    onBeforeUnmount(() => {
        clearAutofillUnlockTimer();
    });

    return {
        llmLoading,
        llmConfigLoading,
        llmSavedConfigs,
        llmCandidates,
        llmStatusFilter,
        selectedLLMIds,
        llmProviderOptions,
        llmReasoningDepthOptions,
        llmCredentialModeOptions,
        llmOAuthCredentialModeOptions,
        llmConnectionMode,
        addConfigDialog,
        addConfigSaving,
        testingConfigId,
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
        handleTestConfig,
        handleDeleteConfig,
        loadLLMCandidates,
        handleLLMGenerate,
        handleLLMBatchAccept,
        handleLLMAccept,
        handleLLMReject,
    };
}
