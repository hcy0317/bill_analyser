import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockBeforeUnmountCallbacks: Array<() => void> = [];
const mockSetError = jest.fn<(message: string) => void>();
const mockShowInfoMessage = jest.fn<(message: string, options?: Record<string, unknown>) => void>();
const mockGetPayloadErrorMessage = jest.fn<(payload: unknown, fallback: string) => string>();
const mockGetRequestErrorMessage = jest.fn<(error: unknown, fallback: string) => string>();

const mockServices = {
    acceptLLMCandidate: jest.fn<(...args: any[]) => Promise<any>>(),
    activateLLMConfig: jest.fn<(...args: any[]) => Promise<any>>(),
    createLLMConfig: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteLLMConfig: jest.fn<(...args: any[]) => Promise<any>>(),
    generateLLMRuleSynthesis: jest.fn<(...args: any[]) => Promise<any>>(),
    getLLMCandidates: jest.fn<(...args: any[]) => Promise<any>>(),
    getLLMConfigs: jest.fn<(...args: any[]) => Promise<any>>(),
    rejectLLMCandidate: jest.fn<(...args: any[]) => Promise<any>>(),
    testLLMConfig: jest.fn<(...args: any[]) => Promise<any>>(),
};

let mockCredentialShouldThrow = false;
let mockCredentialFailure: unknown;
let mockNow = Date.parse('2026-07-15T00:00:00Z');
let mockNextTimerId = 1;
const mockTimers = new Map<number, () => void>();
const mockOriginalWindowSetTimeout = window.setTimeout;
const mockOriginalWindowClearTimeout = window.clearTimeout;

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onBeforeUnmount: (callback: () => void) => mockBeforeUnmountCallbacks.push(callback),
    };
});
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/views/desktop/pairingcenter/components/llmConfigHelpers.ts', () => {
    const actual = jest.requireActual(
        '@/views/desktop/pairingcenter/components/llmConfigHelpers.ts',
    ) as Record<string, any>;
    return {
        ...actual,
        buildCredentialConfigPayload: (form: unknown) => {
            if (mockCredentialShouldThrow) throw mockCredentialFailure;
            return actual['buildCredentialConfigPayload'](form);
        },
    };
});

const { useLearningCenterLlmConfig } = require(
    '@/views/desktop/pairingcenter/components/learning-center/useLearningCenterLlmConfig.ts',
) as typeof import(
    '@/views/desktop/pairingcenter/components/learning-center/useLearningCenterLlmConfig.ts'
);

function successResponse(result: unknown = undefined): any {
    return { data: { success: true, result } };
}

function failedResponse(result: unknown = { message: 'payload rejected' }): any {
    return { data: { success: false, result } };
}

function rawConfig(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        id: 1,
        name: 'Primary',
        provider: 'openai',
        model: 'gpt-test',
        api_key: 'masked-placeholder-from-server',
        is_active: true,
        ...overrides,
    };
}

function rawCandidate(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        id: 1,
        status: 'pending',
        type: 'rule',
        confidence: 0.9,
        suggested_main_category: 'Food',
        suggested_sub_category: 'Coffee',
        suggested_rule_expression: 'OR={coffee}',
        ...overrides,
    };
}

function createComposable(): ReturnType<typeof useLearningCenterLlmConfig> {
    return useLearningCenterLlmConfig({
        tt: key => `tt:${key}`,
        setError: mockSetError,
        showInfoMessage: mockShowInfoMessage,
        getPayloadErrorMessage: mockGetPayloadErrorMessage,
        getRequestErrorMessage: mockGetRequestErrorMessage,
    });
}

function deferred<T>() {
    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

function runPendingTimers(): void {
    for (const [timerId, callback] of [...mockTimers.entries()]) {
        mockTimers.delete(timerId);
        callback();
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockNow = Date.parse('2026-07-15T00:00:00Z');
    mockNextTimerId = 1;
    mockTimers.clear();
    jest.spyOn(Date, 'now').mockImplementation(() => mockNow);
    window.setTimeout = ((callback: TimerHandler) => {
        const timerId = mockNextTimerId++;
        mockTimers.set(timerId, callback as () => void);
        return timerId;
    }) as typeof window.setTimeout;
    window.clearTimeout = ((timerId?: number) => {
        if (timerId !== undefined) mockTimers.delete(timerId);
    }) as typeof window.clearTimeout;
    mockBeforeUnmountCallbacks.length = 0;
    mockCredentialShouldThrow = false;
    mockCredentialFailure = undefined;
    mockGetPayloadErrorMessage.mockImplementation((_payload, fallback) => `payload:${fallback}`);
    mockGetRequestErrorMessage.mockImplementation((_error, fallback) => `request:${fallback}`);
    mockServices.getLLMConfigs.mockResolvedValue(successResponse([rawConfig()]));
    mockServices.createLLMConfig.mockResolvedValue(successResponse({ id: 2 }));
    mockServices.activateLLMConfig.mockResolvedValue(successResponse());
    mockServices.deleteLLMConfig.mockResolvedValue(successResponse());
    mockServices.getLLMCandidates.mockResolvedValue(successResponse({ candidates: [rawCandidate()] }));
    mockServices.generateLLMRuleSynthesis.mockResolvedValue(successResponse({ candidates_created: 2 }));
    mockServices.acceptLLMCandidate.mockResolvedValue(successResponse());
    mockServices.rejectLLMCandidate.mockResolvedValue(successResponse());
});

afterEach(() => {
    jest.restoreAllMocks();
    window.setTimeout = mockOriginalWindowSetTimeout;
    window.clearTimeout = mockOriginalWindowClearTimeout;
});

describe('useLearningCenterLlmConfig state and computed contracts', () => {
    test('initializes provider, status, field-name, selection, and label projections', () => {
        const state = createComposable();

        expect(state.llmLoading.value).toBe(false);
        expect(state.llmConfigLoading.value).toBe(false);
        expect(state.llmSavedConfigs.value).toStrictEqual([]);
        expect(state.llmCandidates.value).toStrictEqual([]);
        expect(state.llmProviderOptions.map(option => option.value)).toContain('openai_compatible');
        expect(state.llmReasoningDepthOptions).toContainEqual({ title: 'tt:High', value: 'high' });
        expect(state.llmCredentialModeOptions).toContainEqual({ title: 'tt:API Key', value: 'api_key' });
        expect(state.llmOAuthCredentialModeOptions).not.toContainEqual(expect.objectContaining({ value: 'api_key' }));
        expect(state.llmConnectionMode.value).toBe('api');
        expect(state.selectedLLMProviderOption.value.value).toBe('openai');
        expect(state.baseUrlFieldLabel.value).toBe('tt:Base URL (optional)');
        expect(state.llmConfigFieldNames.value).toMatchObject({
            name: expect.stringMatching(/^llm-config-label-/),
            apiKey: expect.stringMatching(/^llm-config-credential-/),
            rulePrompt: expect.stringMatching(/^llm-config-rule-prompt-/),
        });
        expect(new Set(Object.values(state.llmConfigFieldNames.value)).size).toBe(11);
        expect(state.llmStatusOptions.value).toStrictEqual([
            { title: 'tt:All', value: '' },
            { title: 'tt:Pending', value: 'pending' },
            { title: 'tt:Accepted', value: 'accepted' },
            { title: 'tt:Rejected', value: 'rejected' },
        ]);
        expect(state.llmProviderLabel('openai')).toBe('OpenAI');
        expect(state.llmProviderLabel('anthropic')).toBe('Claude (Anthropic)');
        expect(state.llmProviderLabel('custom-provider')).toBe('custom-provider');

        state.newConfigForm.value.provider = 'openai_compatible';
        expect(state.selectedLLMProviderOption.value.requiresBaseUrl).toBe(true);
        expect(state.baseUrlFieldLabel.value).toBe('tt:Base URL');
        state.newConfigForm.value.provider = 'missing-provider';
        expect(state.selectedLLMProviderOption.value.value).toBe('openai');

        state.llmConnectionMode.value = 'oauth';
        expect(state.newConfigForm.value.credential_mode).toBe('session_json');
        expect(state.llmConnectionMode.value).toBe('oauth');
        state.newConfigForm.value.credential_mode = 'access_token';
        state.llmConnectionMode.value = 'api';
        expect(state.newConfigForm.value.credential_mode).toBe('api_key');
    });

    test('filters candidates and implements writable select-all and indeterminate state', () => {
        const state = createComposable();
        state.llmCandidates.value = [
            { id: 1, status: 'pending' },
            { id: 2, status: 'pending' },
            { id: 3, status: 'accepted' },
        ];

        expect(state.llmPendingCount.value).toBe(2);
        expect(state.filteredLLMCandidates.value).toHaveLength(3);
        expect(state.selectAllLLM.value).toBe(false);
        expect(state.llmIndeterminate.value).toBe(false);

        state.selectedLLMIds.value = [1];
        expect(state.llmIndeterminate.value).toBe(true);
        expect(state.selectAllLLM.value).toBe(false);
        state.selectAllLLM.value = true;
        expect(state.selectedLLMIds.value).toStrictEqual([1, 2]);
        expect(state.selectAllLLM.value).toBe(true);
        expect(state.llmIndeterminate.value).toBe(false);

        state.selectAllLLM.value = false;
        expect(state.selectedLLMIds.value).toStrictEqual([]);
        state.llmStatusFilter.value = 'accepted';
        expect(state.filteredLLMCandidates.value).toStrictEqual([{ id: 3, status: 'accepted' }]);
        expect(state.selectAllLLM.value).toBe(false);
        state.selectAllLLM.value = true;
        expect(state.selectedLLMIds.value).toStrictEqual([]);
    });

    test('resets config form and covers autofill lock, timer, close, focus unlock, and unmount cleanup', () => {
        const state = createComposable();
        const initialFieldNames = state.llmConfigFieldNames.value;
        state.newConfigForm.value.name = 'stale';
        state.newConfigForm.value.api_key = 'not-a-real-key';

        mockNow += 1_000;
        state.openAddConfigDialog();
        expect(state.addConfigDialog.value).toBe(true);
        expect(state.autofillFieldsLocked.value).toBe(true);
        expect(state.newConfigForm.value.name).toBe('');
        expect(state.newConfigForm.value.api_key).toBe('');
        expect(state.llmConfigFieldNames.value.name).not.toBe(initialFieldNames.name);

        runPendingTimers();
        expect(state.autofillFieldsLocked.value).toBe(false);
        expect(mockTimers.size).toBe(0);

        state.unlockAutofillFields();
        expect(state.autofillFieldsLocked.value).toBe(false);
        state.openAddConfigDialog();
        expect(mockTimers.size).toBe(1);
        state.unlockAutofillFields();
        expect(mockTimers.size).toBe(0);

        state.openAddConfigDialog();
        state.closeAddConfigDialog();
        expect(state.addConfigDialog.value).toBe(false);
        expect(state.autofillFieldsLocked.value).toBe(false);
        expect(mockTimers.size).toBe(0);

        state.openAddConfigDialog();
        expect(mockBeforeUnmountCallbacks).toHaveLength(1);
        mockBeforeUnmountCallbacks[0]?.();
        expect(mockTimers.size).toBe(0);
    });
});

describe('useLearningCenterLlmConfig saved configuration actions', () => {
    test('loads sanitized configurations and keeps loading truthful for pending requests', async () => {
        const state = createComposable();
        const pending = deferred<any>();
        mockServices.getLLMConfigs.mockReturnValueOnce(pending.promise);

        const loadingPromise = state.loadLLMConfigs();
        expect(state.llmConfigLoading.value).toBe(true);
        pending.resolve(successResponse([rawConfig()]));
        await loadingPromise;

        expect(state.llmConfigLoading.value).toBe(false);
        expect(state.llmSavedConfigs.value).toStrictEqual([{
            id: 1,
            name: 'Primary',
            provider: 'openai',
            model: 'gpt-test',
            is_active: true,
        }]);
        expect(state.llmSavedConfigs.value[0]).not.toHaveProperty('api_key');
        expect(JSON.stringify(state.llmSavedConfigs.value)).not.toContain('masked-placeholder-from-server');
    });

    test('ignores failed or thrown config loads and maps malformed results to an empty safe list', async () => {
        const state = createComposable();
        state.llmSavedConfigs.value = [{ id: 9, name: 'unchanged', provider: 'openai', model: 'm' }];

        mockServices.getLLMConfigs.mockResolvedValueOnce(failedResponse());
        await state.loadLLMConfigs();
        expect(state.llmSavedConfigs.value[0]?.name).toBe('unchanged');

        mockServices.getLLMConfigs.mockResolvedValueOnce({});
        await state.loadLLMConfigs();
        expect(state.llmSavedConfigs.value[0]?.name).toBe('unchanged');

        mockServices.getLLMConfigs.mockResolvedValueOnce(successResponse({ malformed: true }));
        await state.loadLLMConfigs();
        expect(state.llmSavedConfigs.value).toStrictEqual([]);

        state.llmSavedConfigs.value = [{ id: 10, name: 'still-safe', provider: 'openai', model: 'm' }];
        mockServices.getLLMConfigs.mockRejectedValueOnce(new Error('load failed'));
        await state.loadLLMConfigs();
        expect(state.llmSavedConfigs.value[0]?.name).toBe('still-safe');
        expect(state.llmConfigLoading.value).toBe(false);
        expect(mockSetError).not.toHaveBeenCalled();
    });

    test('creates a trimmed active config with a strict payload and clears key material afterward', async () => {
        const state = createComposable();
        state.openAddConfigDialog();
        Object.assign(state.newConfigForm.value, {
            name: '  Primary config  ',
            provider: 'openai',
            model: '  gpt-test  ',
            api_key: 'not-a-real-key',
            base_url: '  https://api.example.test/v1  ',
            credential_mode: 'api_key',
            advancedMode: true,
            reasoning_depth: 'high',
            temperature: '0.2',
            max_tokens: '1024',
            system_prompt: '  safe system prompt  ',
        });
        mockServices.getLLMConfigs.mockResolvedValueOnce(successResponse([rawConfig({ id: 2 })]));
        const pending = deferred<any>();
        mockServices.createLLMConfig.mockReturnValueOnce(pending.promise);

        const savePromise = state.saveNewConfig();
        expect(state.addConfigSaving.value).toBe(true);
        pending.resolve(successResponse({ id: 2 }));
        await savePromise;

        expect(mockServices.createLLMConfig).toHaveBeenCalledTimes(1);
        const payload = mockServices.createLLMConfig.mock.calls[0]?.[0] as Record<string, unknown>;
        expect(Object.keys(payload).sort()).toStrictEqual([
            'advanced_settings', 'api_key', 'base_url', 'credential_config',
            'is_active', 'model', 'name', 'provider',
        ]);
        expect(payload).toStrictEqual({
            name: 'Primary config',
            provider: 'openai',
            model: 'gpt-test',
            api_key: 'not-a-real-key',
            base_url: 'https://api.example.test/v1',
            credential_config: { credential_mode: 'api_key' },
            advanced_settings: {
                reasoning_depth: 'high',
                temperature: 0.2,
                max_tokens: 1024,
                system_prompt: 'safe system prompt',
            },
            is_active: true,
        });
        expect(payload).not.toHaveProperty('token');
        expect(payload).not.toHaveProperty('authorization');
        expect(state.addConfigDialog.value).toBe(false);
        expect(state.addConfigSaving.value).toBe(false);
        expect(state.newConfigForm.value.api_key).toBe('');
        expect(state.llmSavedConfigs.value[0]).not.toHaveProperty('api_key');
        expect(JSON.stringify(state.llmSavedConfigs.value)).not.toContain('not-a-real-key');
    });

    test('keeps API key and OAuth credential payloads isolated when saving', async () => {
        const state = createComposable();
        Object.assign(state.newConfigForm.value, {
            name: 'OAuth config',
            provider: 'openai',
            model: 'gpt-test',
            api_key: 'stale-api-key',
            credential_mode: 'refresh_token',
            credential_json: '{"refresh_token":"refresh-secret","token_endpoint":"https://auth.example.test/token"}',
        });

        await state.saveNewConfig();

        expect(mockServices.createLLMConfig).toHaveBeenCalledWith(expect.objectContaining({
            api_key: '',
            credential_config: {
                credential_mode: 'refresh_token',
                credential_json: {
                    refresh_token: 'refresh-secret',
                    token_endpoint: 'https://auth.example.test/token',
                },
            },
        }));
    });

    test('marks later configs inactive and enforces name, base-url, and credential early returns', async () => {
        const state = createComposable();
        state.llmSavedConfigs.value = [{ id: 1, name: 'existing', provider: 'openai', model: 'm' }];
        Object.assign(state.newConfigForm.value, {
            name: 'Later',
            provider: 'openai_compatible',
            model: 'model',
            api_key: '',
            base_url: 'https://provider.example.test/v1',
        });
        await state.saveNewConfig();
        expect(mockServices.createLLMConfig).toHaveBeenCalledWith(expect.objectContaining({
            api_key: '',
            is_active: false,
            base_url: 'https://provider.example.test/v1',
        }));

        mockServices.createLLMConfig.mockClear();
        state.newConfigForm.value.name = '   ';
        await state.saveNewConfig();
        expect(mockSetError).toHaveBeenCalledWith('Name is required');
        expect(mockServices.createLLMConfig).not.toHaveBeenCalled();

        state.newConfigForm.value.name = 'Requires URL';
        state.newConfigForm.value.provider = 'openai_compatible';
        state.newConfigForm.value.base_url = '';
        await state.saveNewConfig();
        expect(mockSetError).toHaveBeenCalledWith('OpenAI-compatible requires a Base URL');
        expect(mockServices.createLLMConfig).not.toHaveBeenCalled();

        state.newConfigForm.value.provider = 'openai';
        state.newConfigForm.value.credential_mode = 'session_json';
        state.newConfigForm.value.credential_json = '[]';
        await state.saveNewConfig();
        expect(mockSetError).toHaveBeenCalledWith('Credential JSON must be a JSON object');

        state.newConfigForm.value.credential_json = '';
        mockCredentialShouldThrow = true;
        mockCredentialFailure = { malformed: true };
        await state.saveNewConfig();
        expect(mockSetError).toHaveBeenCalledWith('Invalid credential config');
        expect(state.addConfigSaving.value).toBe(false);
    });

    test('reports unsuccessful, malformed, and rejected creates with supplied safe mappers', async () => {
        const state = createComposable();
        Object.assign(state.newConfigForm.value, {
            name: 'Config',
            provider: 'openai',
            model: 'model',
            api_key: '',
        });

        mockServices.createLLMConfig.mockResolvedValueOnce(failedResponse({ code: 'invalid' }));
        await state.saveNewConfig();
        expect(mockGetPayloadErrorMessage).toHaveBeenCalledWith({ code: 'invalid' }, 'Failed to create config');
        expect(mockSetError).toHaveBeenCalledWith('payload:Failed to create config');

        mockServices.createLLMConfig.mockResolvedValueOnce({});
        await state.saveNewConfig();
        expect(mockGetPayloadErrorMessage).toHaveBeenCalledWith(undefined, 'Failed to create config');

        const requestFailure = { privateCredential: 'credential-canary' };
        mockServices.createLLMConfig.mockRejectedValueOnce(requestFailure);
        await state.saveNewConfig();
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(requestFailure, 'Failed to create config');
        expect(mockSetError).toHaveBeenCalledWith('request:Failed to create config');
        expect(mockSetError.mock.calls.flat().join(' ')).not.toContain('credential-canary');
        expect(state.addConfigSaving.value).toBe(false);
    });

    test('activates and deletes configs, refreshes success, and maps request failures', async () => {
        const state = createComposable();
        await state.handleActivateConfig(4);
        expect(mockServices.activateLLMConfig).toHaveBeenCalledWith(4);
        expect(mockServices.getLLMConfigs).toHaveBeenCalled();

        mockServices.getLLMConfigs.mockClear();
        await state.handleDeleteConfig(5);
        expect(mockServices.deleteLLMConfig).toHaveBeenCalledWith(5);
        expect(mockServices.getLLMConfigs).toHaveBeenCalled();

        const activationFailure = new Error('activate failed');
        mockServices.activateLLMConfig.mockRejectedValueOnce(activationFailure);
        await state.handleActivateConfig(6);
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(activationFailure, 'Failed to activate config');
        expect(mockSetError).toHaveBeenCalledWith('request:Failed to activate config');

        const deletionFailure = new Error('delete failed');
        mockServices.deleteLLMConfig.mockRejectedValueOnce(deletionFailure);
        await state.handleDeleteConfig(7);
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(deletionFailure, 'Failed to delete config');
        expect(mockSetError).toHaveBeenCalledWith('request:Failed to delete config');
    });

    test('tests a saved config without activating it and reports sanitized failures', async () => {
        const state = createComposable();
        mockServices.testLLMConfig.mockResolvedValueOnce(successResponse({ latency_ms: 42 }));

        await state.handleTestConfig(8);

        expect(mockServices.testLLMConfig).toHaveBeenCalledWith(8);
        expect(mockServices.activateLLMConfig).not.toHaveBeenCalled();
        expect(mockShowInfoMessage).toHaveBeenCalledWith(
            'LLM connection test succeeded',
            { latency: '42 ms' },
        );
        expect(state.testingConfigId.value).toBeNull();

        mockServices.testLLMConfig.mockResolvedValueOnce(failedResponse({ code: 'invalid-model' }));
        await state.handleTestConfig(9);
        expect(mockGetPayloadErrorMessage).toHaveBeenCalledWith(
            expect.objectContaining({ success: false }),
            'Failed to test LLM config',
        );
        expect(mockSetError).toHaveBeenCalledWith('payload:Failed to test LLM config');

        const requestFailure = { secret: 'credential-canary' };
        mockServices.testLLMConfig.mockRejectedValueOnce(requestFailure);
        await state.handleTestConfig(10);
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(
            requestFailure,
            'Failed to test LLM config',
        );
        expect(mockSetError).toHaveBeenCalledWith('request:Failed to test LLM config');
        expect(mockSetError.mock.calls.flat().join(' ')).not.toContain('credential-canary');
    });
});

describe('useLearningCenterLlmConfig candidate loading and review actions', () => {
    test('loads normalized candidates, prunes stale selections, and tracks pending requests', async () => {
        const state = createComposable();
        state.selectedLLMIds.value = [1, 2, 99];
        const pending = deferred<any>();
        mockServices.getLLMCandidates.mockReturnValueOnce(pending.promise);

        const loadingPromise = state.loadLLMCandidates();
        expect(state.llmLoading.value).toBe(true);
        expect(mockServices.getLLMCandidates).toHaveBeenCalledWith({ type: 'rule_synthesis', limit: 100 });
        pending.resolve(successResponse({
            candidates: [
                rawCandidate({ id: 1, status: 'pending' }),
                rawCandidate({ id: 2, status: 'accepted' }),
            ],
        }));
        await loadingPromise;

        expect(state.llmLoading.value).toBe(false);
        expect(state.llmCandidates.value).toHaveLength(2);
        expect(state.llmCandidates.value[0]).toMatchObject({
            id: 1,
            status: 'pending',
            category_name: 'Food/Coffee',
            rule_content: 'OR={coffee}',
        });
        expect(state.selectedLLMIds.value).toStrictEqual([1]);
    });

    test('handles missing, malformed, and rejected candidate responses without leaking errors', async () => {
        const state = createComposable();
        state.llmCandidates.value = [{ id: 9, status: 'pending' }];

        mockServices.getLLMCandidates.mockResolvedValueOnce(failedResponse());
        await state.loadLLMCandidates();
        expect(state.llmCandidates.value).toStrictEqual([{ id: 9, status: 'pending' }]);

        mockServices.getLLMCandidates.mockResolvedValueOnce({});
        await state.loadLLMCandidates();
        expect(state.llmCandidates.value).toStrictEqual([{ id: 9, status: 'pending' }]);

        mockServices.getLLMCandidates.mockResolvedValueOnce(successResponse({ candidates: 'malformed' }));
        await state.loadLLMCandidates();
        expect(state.llmCandidates.value).toStrictEqual([]);

        const requestFailure = { authorization: 'credential-canary' };
        mockServices.getLLMCandidates.mockRejectedValueOnce(requestFailure);
        await state.loadLLMCandidates();
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(
            requestFailure,
            'Failed to load LLM candidates',
        );
        expect(mockSetError).toHaveBeenCalledWith('request:Failed to load LLM candidates');
        expect(mockSetError.mock.calls.flat().join(' ')).not.toContain('credential-canary');
        expect(state.llmLoading.value).toBe(false);
    });

    test('generates candidates with reported and default counts, reloads, and maps failures', async () => {
        const state = createComposable();
        const generatePromise = state.handleLLMGenerate();
        expect(state.llmLoading.value).toBe(true);
        await generatePromise;
        expect(mockServices.generateLLMRuleSynthesis).toHaveBeenCalledWith({ limit: 8 });
        expect(mockShowInfoMessage).toHaveBeenCalledWith(
            'Generated LLM Rule Candidates Summary',
            { count: 2 },
        );
        expect(mockServices.getLLMCandidates).toHaveBeenCalled();
        expect(state.llmLoading.value).toBe(false);

        mockServices.generateLLMRuleSynthesis.mockResolvedValueOnce({});
        await state.handleLLMGenerate();
        expect(mockShowInfoMessage).toHaveBeenLastCalledWith(
            'Generated LLM Rule Candidates Summary',
            { count: 0 },
        );

        const failure = new Error('generation failed');
        mockServices.generateLLMRuleSynthesis.mockRejectedValueOnce(failure);
        await state.handleLLMGenerate();
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(
            failure,
            'Failed to generate LLM rule candidates',
        );
        expect(mockSetError).toHaveBeenCalledWith('request:Failed to generate LLM rule candidates');
        expect(state.llmLoading.value).toBe(false);
    });

    test('batch accepts selected ids in order, clears selection, reloads, and handles early/error exits', async () => {
        const state = createComposable();
        await state.handleLLMBatchAccept();
        expect(mockServices.acceptLLMCandidate).not.toHaveBeenCalled();

        state.selectedLLMIds.value = [3, 4];
        const acceptPromise = state.handleLLMBatchAccept();
        expect(state.llmLoading.value).toBe(true);
        await acceptPromise;
        expect(mockServices.acceptLLMCandidate.mock.calls.map(call => call[0])).toStrictEqual([3, 4]);
        expect(state.selectedLLMIds.value).toStrictEqual([]);
        expect(mockServices.getLLMCandidates).toHaveBeenCalled();
        expect(state.llmLoading.value).toBe(false);

        state.selectedLLMIds.value = [5, 6];
        const failure = new Error('batch denied');
        mockServices.acceptLLMCandidate
            .mockResolvedValueOnce(successResponse())
            .mockRejectedValueOnce(failure);
        await state.handleLLMBatchAccept();
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(
            failure,
            'Failed to batch accept LLM rule candidates',
        );
        expect(mockSetError).toHaveBeenCalledWith('request:Failed to batch accept LLM rule candidates');
        expect(state.selectedLLMIds.value).toStrictEqual([5, 6]);
        expect(state.llmLoading.value).toBe(false);
    });

    test('accepts and rejects one candidate, removes only that selection, reloads, and maps failures', async () => {
        const state = createComposable();
        mockServices.getLLMCandidates.mockResolvedValue(successResponse({ candidates: [
            rawCandidate({ id: 2, status: 'pending' }),
            rawCandidate({ id: 3, status: 'pending' }),
        ] }));
        state.selectedLLMIds.value = [1, 2, 3];

        await state.handleLLMAccept(1);
        expect(mockServices.acceptLLMCandidate).toHaveBeenCalledWith(1);
        expect(state.selectedLLMIds.value).toStrictEqual([2, 3]);
        expect(mockServices.getLLMCandidates).toHaveBeenCalled();

        await state.handleLLMReject(2);
        expect(mockServices.rejectLLMCandidate).toHaveBeenCalledWith(2);
        expect(state.selectedLLMIds.value).toStrictEqual([3]);

        state.selectedLLMIds.value = [3];
        const acceptFailure = new Error('accept denied');
        mockServices.acceptLLMCandidate.mockRejectedValueOnce(acceptFailure);
        await state.handleLLMAccept(3);
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(acceptFailure, 'Failed to accept candidate');
        expect(state.selectedLLMIds.value).toStrictEqual([3]);

        const rejectFailure = new Error('reject denied');
        mockServices.rejectLLMCandidate.mockRejectedValueOnce(rejectFailure);
        await state.handleLLMReject(3);
        expect(mockGetRequestErrorMessage).toHaveBeenCalledWith(rejectFailure, 'Failed to reject candidate');
        expect(mockSetError).toHaveBeenCalledWith('request:Failed to reject candidate');
        expect(state.selectedLLMIds.value).toStrictEqual([3]);
        expect(state.llmLoading.value).toBe(false);
    });
});
