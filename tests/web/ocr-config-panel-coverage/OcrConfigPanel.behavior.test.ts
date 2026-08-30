import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockActualVue = jest.requireActual('vue') as any;
const mockOnMountedCallbacks: Array<() => void> = [];
const mockShowMessage = jest.fn<(...args: any[]) => void>();
const mockShowError = jest.fn<(...args: any[]) => void>();
const mockAxiosIsAxiosError = jest.fn<(error: unknown) => boolean>();
const mockTemplateBindings = jest.fn<(...args: any[]) => void>();
const mockSnackbar = {
    showMessage: mockShowMessage,
    showError: mockShowError,
};
const mockServices = {
    getOCRConfig: jest.fn<(...args: any[]) => Promise<any>>(),
    updateOCRConfig: jest.fn<(...args: any[]) => Promise<any>>(),
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => void) => mockOnMountedCallbacks.push(callback),
        useTemplateRef: (name: string) => actual.ref(name === 'snackbar' ? mockSnackbar : null),
    };
});
jest.mock('axios', () => ({
    __esModule: true,
    default: {
        isAxiosError: (error: unknown) => mockAxiosIsAxiosError(error),
    },
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: (...args: any[]) => mockTemplateBindings(...args),
}));
jest.mock('@/components/desktop/SettingsJsonImportExportButton.vue', () => ({
    __esModule: true,
    default: { name: 'SettingsJsonImportExportButtonStub' },
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: { name: 'SnackBarStub' },
}));

import OcrConfigPanelComponent from '@/views/desktop/pairingcenter/components/OcrConfigPanel.vue';

const OcrConfigPanel = OcrConfigPanelComponent as any;

function response(result: unknown, success = true): any {
    return { data: { success, result } };
}

function fullConfig(overrides: Record<string, unknown> = {}): any {
    return {
        provider: 'llm_vision',
        lang: 'eng',
        model: 'vision-model',
        base_url: 'https://ocr.example.invalid/v1',
        parameters: { temperature: 0 },
        credential_config: {
            credential_mode: 'refresh_token',
            credential_json: { account: 'synthetic' },
            token_endpoint: 'https://auth.example.invalid/token',
            refresh_headers: { 'x-client': 'synthetic' },
            refresh_body: { grant_type: 'refresh_token' },
            refresh_params: { tenant: 'synthetic' },
        },
        available_providers: ['llm_vision', 'custom-provider'],
        configured: true,
        ...overrides,
    };
}

function setup(props: Record<string, unknown> = {}): any {
    return OcrConfigPanel.setup(
        props,
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() },
    );
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

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockOnMountedCallbacks.length = 0;
    mockAxiosIsAxiosError.mockReturnValue(false);
    mockServices.getOCRConfig.mockResolvedValue(response(fullConfig()));
    mockServices.updateOCRConfig.mockResolvedValue(response(fullConfig()));
});

describe('OcrConfigPanel state and mapping', () => {
    test('projects props, credential modes, providers, fallbacks, and unknown labels', () => {
        const bindings = setup();

        expect(bindings.hideSectionTitle.value).toBe(false);
        expect(bindings.hasHeaderActionsTarget.value).toBe(false);
        expect(bindings.headerActionsTarget.value).toBe('body');
        expect(bindings.credentialModeOptions).toHaveLength(7);
        expect(bindings.credentialModeOptions[0]).toStrictEqual({
            title: 'tt:API Key',
            value: 'api_key',
        });
        expect(bindings.ocrProviderOptions.value.map((item: any) => item.value)).toStrictEqual([
            'disabled', 'tesseract', 'cloud_stub', 'local_json_ocr', 'llm_vision',
        ]);
        expect(bindings.ocrProviderLabel('disabled')).toBe('tt:Disabled');
        expect(bindings.ocrProviderLabel('tesseract')).toBe('Tesseract');
        expect(bindings.ocrProviderLabel('cloud_stub')).toBe('Cloud Stub');
        expect(bindings.ocrProviderLabel('local_json_ocr')).toBe('Local JSON OCR');
        expect(bindings.ocrProviderLabel('llm_vision')).toBe('LLM Vision');
        expect(bindings.ocrProviderLabel('custom-provider')).toBe('custom-provider');
        expect(bindings.ocrSetupOptions.value.map((item: any) => [item.value, item.kind])).toStrictEqual([
            ['disabled', 'off'],
            ['tesseract', 'built_in'],
            ['local_json_ocr', 'local'],
            ['llm_vision', 'cloud'],
        ]);

        bindings.ocrConfig.value.available_providers = ['custom-provider'];
        expect(bindings.ocrProviderOptions.value).toStrictEqual([
            { title: 'custom-provider', value: 'custom-provider' },
        ]);
        bindings.ocrConfig.value.available_providers = [];
        expect(bindings.ocrProviderOptions.value).toHaveLength(5);

        const projected = setup({ hideSectionTitle: true, headerActionsTarget: '#toolbar' });
        expect(projected.hideSectionTitle.value).toBe(true);
        expect(projected.hasHeaderActionsTarget.value).toBe(true);
        expect(projected.headerActionsTarget.value).toBe('#toolbar');
        expect(mockTemplateBindings).toHaveBeenCalled();
        expect(mockOnMountedCallbacks).toHaveLength(2);
    });

    test('applies complete configuration and normalizes sparse backend responses', () => {
        const bindings = setup();
        bindings.applyOCRConfig(fullConfig());

        expect(bindings.ocrConfig.value).toMatchObject({
            provider: 'llm_vision',
            lang: 'eng',
            model: 'vision-model',
            configured: true,
        });
        expect(bindings.ocrConfigForm.value).toStrictEqual({
            provider: 'llm_vision',
            lang: 'eng',
            model: 'vision-model',
            base_url: 'https://ocr.example.invalid/v1',
            parameters: '{\n  "temperature": 0\n}',
            credential_mode: 'refresh_token',
            credential_json: '{\n  "account": "synthetic"\n}',
            token_endpoint: 'https://auth.example.invalid/token',
            refresh_headers: '{\n  "x-client": "synthetic"\n}',
            refresh_body: '{\n  "grant_type": "refresh_token"\n}',
            refresh_params: '{\n  "tenant": "synthetic"\n}',
            api_key: '',
            advancedMode: false,
        });

        bindings.applyOCRConfig({
            provider: '',
            lang: '',
            model: null,
            base_url: null,
            parameters: null,
            credential_config: null,
            available_providers: [],
            configured: 0,
        });
        expect(bindings.ocrConfig.value).toMatchObject({
            provider: 'disabled',
            lang: 'chi_sim+eng',
            model: '',
            base_url: '',
            parameters: {},
            credential_config: {},
            configured: false,
        });
        expect(bindings.ocrConfig.value.available_providers).toHaveLength(5);
        expect(bindings.ocrConfigForm.value).toMatchObject({
            credential_mode: 'api_key',
            credential_json: '{}',
            token_endpoint: '',
            refresh_headers: '{}',
            refresh_body: '{}',
            refresh_params: '{}',
        });

        bindings.applyOCRConfig(fullConfig({ available_providers: 'invalid' }));
        expect(bindings.ocrConfig.value.available_providers).toHaveLength(5);
    });
});

describe('OcrConfigPanel JSON and error contracts', () => {
    test('parses optional objects and rejects arrays, nulls, primitives, and malformed JSON', () => {
        const bindings = setup();
        expect(bindings.parseOptionalJsonObject('   ', 'Optional')).toStrictEqual({});
        expect(bindings.parseOptionalJsonObject('{"enabled":true}', 'Optional')).toStrictEqual({
            enabled: true,
        });
        expect(() => bindings.parseOptionalJsonObject('[]', 'Optional')).toThrow(
            'Optional must be a JSON object',
        );
        expect(() => bindings.parseOptionalJsonObject('null', 'Optional')).toThrow(
            'Optional must be a JSON object',
        );
        expect(() => bindings.parseOptionalJsonObject('42', 'Optional')).toThrow(
            'Optional must be a JSON object',
        );
        expect(() => bindings.parseOptionalJsonObject('{', 'Optional')).toThrow(SyntaxError);
    });

    test('builds minimal and complete credential payloads with trimmed endpoints', () => {
        const bindings = setup();
        Object.assign(bindings.ocrConfigForm.value, {
            credential_mode: 'api_key',
            credential_json: '',
            token_endpoint: '   ',
            refresh_headers: '',
            refresh_body: '',
            refresh_params: '',
        });
        expect(bindings.buildOcrCredentialConfig()).toStrictEqual({ credential_mode: 'api_key' });

        bindings.ocrConfigForm.value.api_key = '  vision-secret  ';
        expect(bindings.buildOcrCredentialConfig()).toStrictEqual({
            credential_mode: 'api_key',
            credential_json: { api_key: 'vision-secret' },
        });

        bindings.ocrConfigForm.value.api_key = '';
        bindings.ocrConfig.value.credential_config = { access_token: '********' };
        expect(bindings.buildOcrCredentialConfig()).toStrictEqual({
            credential_mode: 'api_key',
            preserve_existing: true,
        });

        bindings.ocrConfigForm.value.credential_mode = 'refresh_token';
        bindings.ocrConfigForm.value.credential_json = '{"access_token":"********"}';
        expect(bindings.buildOcrCredentialConfig()).toStrictEqual({
            credential_mode: 'refresh_token',
            preserve_existing: true,
        });

        Object.assign(bindings.ocrConfigForm.value, {
            credential_mode: 'refresh_token',
            credential_json: '{"account":"synthetic"}',
            token_endpoint: '  https://auth.example.invalid/token  ',
            refresh_headers: '{"x-client":"synthetic"}',
            refresh_body: '{"grant_type":"refresh_token"}',
            refresh_params: '{"tenant":"synthetic"}',
        });
        expect(bindings.buildOcrCredentialConfig()).toStrictEqual({
            credential_mode: 'refresh_token',
            credential_json: { account: 'synthetic' },
            token_endpoint: 'https://auth.example.invalid/token',
            refresh_headers: { 'x-client': 'synthetic' },
            refresh_body: { grant_type: 'refresh_token' },
            refresh_params: { tenant: 'synthetic' },
        });
    });

    test('extracts bounded nested payload messages and maps request error classes safely', () => {
        const bindings = setup();
        expect(bindings.extractPayloadMessage('direct')).toBe('direct');
        expect(bindings.extractPayloadMessage('')).toBeNull();
        expect(bindings.extractPayloadMessage(null)).toBeNull();
        expect(bindings.extractPayloadMessage(42)).toBeNull();
        expect(bindings.extractPayloadMessage({ error: { message: 'nested' } })).toBe('nested');
        expect(bindings.extractPayloadMessage({ error: '', message: 'fallback-message' })).toBe(
            'fallback-message',
        );
        expect(bindings.extractPayloadMessage({
            error: { error: { error: { error: 'too-deep' } } },
        })).toBeNull();
        expect(bindings.getPayloadErrorMessage({}, 'fallback')).toBe('fallback');

        const axiosFailure = { response: { data: { error: { message: 'remote denial' } } } };
        mockAxiosIsAxiosError.mockReturnValueOnce(true);
        expect(bindings.getRequestErrorMessage(axiosFailure, 'fallback')).toBe('remote denial');

        mockAxiosIsAxiosError.mockReturnValueOnce(true);
        expect(bindings.getRequestErrorMessage({}, 'fallback')).toBe('fallback');
        expect(bindings.getRequestErrorMessage(new Error('local failure'), 'fallback')).toBe(
            'local failure',
        );
        expect(bindings.getRequestErrorMessage({ private: 'credential-canary' }, 'fallback')).toBe(
            'fallback',
        );
    });
});

describe('OcrConfigPanel loading and saving', () => {
    test('loads configuration on demand and through the mounted lifecycle', async () => {
        const bindings = setup();
        const pending = deferred<any>();
        mockServices.getOCRConfig.mockReturnValueOnce(pending.promise);

        const loadPromise = bindings.loadOCRConfig();
        expect(bindings.ocrConfigLoading.value).toBe(true);
        pending.resolve(response(fullConfig({ model: 'loaded-model' })));
        await loadPromise;
        expect(bindings.ocrConfigLoading.value).toBe(false);
        expect(bindings.ocrConfigForm.value.model).toBe('loaded-model');

        mockServices.getOCRConfig.mockResolvedValueOnce(response(undefined, false));
        await bindings.loadOCRConfig();
        expect(bindings.ocrConfigForm.value.model).toBe('loaded-model');

        mockServices.getOCRConfig.mockRejectedValueOnce(new Error('ignored load failure'));
        await bindings.loadOCRConfig();
        expect(bindings.ocrConfigLoading.value).toBe(false);

        mockServices.getOCRConfig.mockResolvedValueOnce(response(fullConfig({ model: 'mounted-model' })));
        expect(mockOnMountedCallbacks).toHaveLength(1);
        mockOnMountedCallbacks[0]?.();
        await flush();
        expect(bindings.ocrConfigForm.value.model).toBe('mounted-model');
    });

    test('saves a trimmed complete payload, applies response state, and reports success', async () => {
        const bindings = setup();
        Object.assign(bindings.ocrConfigForm.value, {
            provider: 'llm_vision',
            lang: '  eng  ',
            model: '  vision-next  ',
            base_url: '  https://ocr.example.invalid/v2  ',
            parameters: '{"temperature":0.1}',
            credential_mode: 'refresh_token',
            credential_json: '{"account":"synthetic"}',
            token_endpoint: '  https://auth.example.invalid/token  ',
            refresh_headers: '{"x-client":"synthetic"}',
            refresh_body: '{"grant_type":"refresh_token"}',
            refresh_params: '{"tenant":"synthetic"}',
        });
        const pending = deferred<any>();
        mockServices.updateOCRConfig.mockReturnValueOnce(pending.promise);

        const savePromise = bindings.saveOCRConfig();
        expect(bindings.ocrConfigSaving.value).toBe(true);
        expect(mockServices.updateOCRConfig).toHaveBeenCalledWith({
            provider: 'llm_vision',
            lang: 'eng',
            model: 'vision-next',
            base_url: 'https://ocr.example.invalid/v2',
            parameters: { temperature: 0.1 },
            credential_config: {
                credential_mode: 'refresh_token',
                credential_json: { account: 'synthetic' },
                token_endpoint: 'https://auth.example.invalid/token',
                refresh_headers: { 'x-client': 'synthetic' },
                refresh_body: { grant_type: 'refresh_token' },
                refresh_params: { tenant: 'synthetic' },
            },
        });
        pending.resolve(response(fullConfig({ model: 'saved-model' })));
        await savePromise;

        expect(bindings.ocrConfigSaving.value).toBe(false);
        expect(bindings.ocrConfigForm.value.model).toBe('saved-model');
        expect(mockShowMessage).toHaveBeenCalledWith('OCR Config Saved');
        expect(mockShowError).not.toHaveBeenCalled();
    });

    test('uses the language fallback and reports unsuccessful response payloads', async () => {
        const bindings = setup();
        bindings.ocrConfigForm.value.lang = '   ';
        bindings.ocrConfigForm.value.parameters = '';
        mockServices.updateOCRConfig.mockResolvedValueOnce(response({
            error: { message: 'configuration rejected' },
        }, false));

        await bindings.saveOCRConfig();
        expect(mockServices.updateOCRConfig).toHaveBeenCalledWith(expect.objectContaining({
            lang: 'chi_sim+eng',
            parameters: {},
        }));
        expect(bindings.error.value).toBe('configuration rejected');
        expect(mockShowError).toHaveBeenCalledWith({ message: 'configuration rejected' });

        mockServices.updateOCRConfig.mockResolvedValueOnce({});
        await bindings.saveOCRConfig();
        expect(bindings.error.value).toBe('Failed to save OCR config');
        expect(bindings.ocrConfigSaving.value).toBe(false);
    });

    test('maps validation, Axios, Error, and opaque save failures without leaking payloads', async () => {
        const bindings = setup();
        bindings.ocrConfigForm.value.parameters = '[]';
        await bindings.saveOCRConfig();
        expect(bindings.error.value).toBe('Parameters JSON must be a JSON object');
        expect(mockServices.updateOCRConfig).not.toHaveBeenCalled();

        bindings.ocrConfigForm.value.parameters = '{}';
        const axiosFailure = {
            response: { data: { error: { message: 'remote save denial' } } },
            privateCredential: 'credential-canary',
        };
        mockAxiosIsAxiosError.mockReturnValueOnce(true);
        mockServices.updateOCRConfig.mockRejectedValueOnce(axiosFailure);
        await bindings.saveOCRConfig();
        expect(bindings.error.value).toBe('remote save denial');

        mockServices.updateOCRConfig.mockRejectedValueOnce(new Error('local save failure'));
        await bindings.saveOCRConfig();
        expect(bindings.error.value).toBe('local save failure');

        mockServices.updateOCRConfig.mockRejectedValueOnce({ privateCredential: 'credential-canary' });
        await bindings.saveOCRConfig();
        expect(bindings.error.value).toBe('Failed to save OCR config');
        expect(mockShowError.mock.calls.flat().join(' ')).not.toContain('credential-canary');
        expect(bindings.ocrConfigSaving.value).toBe(false);
    });
});
