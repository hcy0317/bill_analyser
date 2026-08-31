<template src="./ocr-config/OcrConfigPanel.template.html"></template>

<script setup lang="ts">
import { computed, onMounted, ref, useTemplateRef } from 'vue';
import axios from 'axios';

import SettingsJsonImportExportButton from '@/components/desktop/SettingsJsonImportExportButton.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import { useI18n } from '@/locales/helpers.ts';
import services, { type OCRConfigResponse } from '@/lib/services.ts';

import { mdiRefresh } from '@mdi/js';

interface SelectOption {
    title: string;
    value: string;
}

interface OCRSetupOption extends SelectOption {
    kind: 'off' | 'built_in' | 'local' | 'cloud';
    kindLabel: string;
    description: string;
}

const props = defineProps<{
    hideSectionTitle?: boolean;
    headerActionsTarget?: string;
}>();

type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();
const snackbar = useTemplateRef<SnackBarType>('snackbar');

const hideSectionTitle = computed(() => Boolean(props.hideSectionTitle));
const hasHeaderActionsTarget = computed(() => Boolean(props.headerActionsTarget));
const headerActionsTarget = computed(() => props.headerActionsTarget || 'body');
const ocrConfigLoading = ref(false);
const ocrConfigSaving = ref(false);
const error = ref('');
const ocrConfig = ref<OCRConfigResponse>({
    provider: 'disabled',
    lang: 'chi_sim+eng',
    model: '',
    base_url: '',
    parameters: {},
    credential_config: {},
    available_providers: ['disabled', 'tesseract', 'cloud_stub', 'local_json_ocr', 'llm_vision'],
    configured: false,
    server_setup: {},
});
const ocrConfigForm = ref({
    provider: 'disabled',
    lang: 'chi_sim+eng',
    model: '',
    base_url: '',
    parameters: '',
    credential_mode: 'api_key',
    credential_json: '',
    token_endpoint: '',
    refresh_headers: '',
    refresh_body: '',
    refresh_params: '',
    api_key: '',
    advancedMode: false,
});
const credentialModeOptions = [
    { title: tt('API Key'), value: 'api_key' },
    { title: tt('Session JSON'), value: 'session_json' },
    { title: tt('Auth JSON'), value: 'auth_json' },
    { title: tt('Account JSON'), value: 'account_json' },
    { title: tt('Sub2API JSON'), value: 'sub2api_json' },
    { title: tt('Access Token'), value: 'access_token' },
    { title: tt('Refresh Token'), value: 'refresh_token' },
];
const ocrLanguageOptions: SelectOption[] = [
    { title: tt('Chinese and English (recommended)'), value: 'chi_sim+eng' },
    { title: tt('Simplified Chinese'), value: 'chi_sim' },
    { title: tt('English'), value: 'eng' },
];
const DEFAULT_CLOUD_VISION_MODEL = 'gpt-4o-mini';
const DEFAULT_CLOUD_VISION_BASE_URL = 'https://api.openai.com/v1';

const ocrProviderOptions = computed<SelectOption[]>(() => {
    const providers = ocrConfig.value.available_providers.length
        ? ocrConfig.value.available_providers
        : ['disabled', 'tesseract', 'cloud_stub', 'local_json_ocr', 'llm_vision'];
    return providers.map(provider => ({
        title: ocrProviderLabel(provider),
        value: provider,
    }));
});

const serverLocalModel = computed(() => ({
    provider: ocrConfig.value.server_setup?.local_model?.provider || 'local_json_ocr',
    configured: ocrConfig.value.server_setup?.local_model?.configured === true,
    bundled: ocrConfig.value.server_setup?.local_model?.bundled === true,
    display_name: ocrConfig.value.server_setup?.local_model?.display_name || tt('Local OCR'),
    model: ocrConfig.value.server_setup?.local_model?.model || '',
}));
const serverQuickSetupAvailable = computed(() => (
    serverLocalModel.value.configured && serverLocalModel.value.bundled
));

const ocrSetupOptions = computed<OCRSetupOption[]>(() => [
    ...(serverQuickSetupAvailable.value ? [{
        title: tt('Smart built-in recognition'),
        value: 'local_json_ocr',
        kind: 'local' as const,
        kindLabel: 'Recommended',
        description: tt('PP-OCRv6 small runs locally on this server for stronger Chinese receipt recognition.'),
    }] : []),
    {
        title: serverQuickSetupAvailable.value ? tt('Basic built-in recognition') : tt('Built-in recognition'),
        value: 'tesseract',
        kind: 'built_in',
        kindLabel: serverQuickSetupAvailable.value ? 'Fallback' : 'Recommended',
        description: tt('Runs inside this Bill Analyser server. No account, address, or API key is required.'),
    },
    ...(!serverQuickSetupAvailable.value ? [{
        title: tt('Self-hosted / local OCR'),
        value: 'local_json_ocr',
        kind: 'local' as const,
        kindLabel: 'Private',
        description: tt('Uses an OCR command installed by the server operator. Images stay on this server.'),
    }] : []),
    {
        title: tt('Cloud vision'),
        value: 'llm_vision',
        kind: 'cloud',
        kindLabel: 'May cost',
        description: tt('Sends receipt images to an OpenAI-compatible vision service. Provider charges may apply.'),
    },
    {
        title: tt('Turn off OCR'),
        value: 'disabled',
        kind: 'off',
        kindLabel: 'Off',
        description: tt('Do not recognize receipt images automatically.'),
    },
]);
const selectedOCRSetupOption = computed<OCRSetupOption>(() => (
    ocrSetupOptions.value.find(option => option.value === ocrConfigForm.value.provider)
        ?? ocrSetupOptions.value[0] as OCRSetupOption
));
const usesRemoteVision = computed(() => ocrConfigForm.value.provider === 'llm_vision');
const supportsLanguage = computed(() => ocrConfigForm.value.provider === 'tesseract');
const savedOCRSetupOption = computed<OCRSetupOption>(() => (
    ocrSetupOptions.value.find(option => option.value === ocrConfig.value.provider)
        ?? ocrSetupOptions.value.find(option => option.value === 'disabled') as OCRSetupOption
));
const ocrHasUnsavedChanges = computed(() => {
    const config = ocrConfig.value;
    const form = ocrConfigForm.value;
    const credentials = config.credential_config || {};
    return form.provider !== config.provider
        || form.lang.trim() !== config.lang
        || form.model.trim() !== (config.model || '')
        || form.base_url.trim() !== (config.base_url || '')
        || form.parameters.trim() !== JSON.stringify(config.parameters || {}, null, 2)
        || form.credential_mode !== String(credentials['credential_mode'] || 'api_key')
        || form.credential_json.trim() !== JSON.stringify(credentials['credential_json'] || {}, null, 2)
        || form.token_endpoint.trim() !== String(credentials['token_endpoint'] || '')
        || form.refresh_headers.trim() !== JSON.stringify(credentials['refresh_headers'] || {}, null, 2)
        || form.refresh_body.trim() !== JSON.stringify(credentials['refresh_body'] || {}, null, 2)
        || form.refresh_params.trim() !== JSON.stringify(credentials['refresh_params'] || {}, null, 2)
        || form.api_key.trim().length > 0;
});
const ocrSaveActionLabel = computed(() => (
    ocrConfigForm.value.provider === 'disabled'
        ? tt('Save and turn off OCR')
        : `${tt('Save and enable')} ${selectedOCRSetupOption.value.title}`
));

function ocrProviderLabel(provider: string): string {
    const labels: Record<string, string> = {
        disabled: tt('Disabled'),
        tesseract: 'Tesseract',
        cloud_stub: 'Cloud Stub',
        local_json_ocr: 'Local JSON OCR',
        llm_vision: 'LLM Vision',
    };
    return labels[provider] ?? provider;
}

/** 选择普通用户可理解的 OCR 模式，并只补齐该模式真正需要的安全默认值。 */
function selectOCRSetup(provider: string): void {
    ocrConfigForm.value.provider = provider;
    if (provider === 'tesseract' && !ocrConfigForm.value.lang.trim()) {
        ocrConfigForm.value.lang = 'chi_sim+eng';
    }
    if (provider === 'llm_vision') {
        ocrConfigForm.value.model = ocrConfigForm.value.model.trim() || DEFAULT_CLOUD_VISION_MODEL;
        ocrConfigForm.value.base_url = ocrConfigForm.value.base_url.trim() || DEFAULT_CLOUD_VISION_BASE_URL;
        ocrConfigForm.value.credential_mode = ocrConfigForm.value.credential_mode || 'api_key';
    }
}

/**
 * 将后端 OCR 配置响应映射到展示状态和编辑表单，保留未配置 provider 的默认值。
 */
function applyOCRConfig(config: OCRConfigResponse): void {
    ocrConfig.value = {
        provider: config.provider || 'disabled',
        lang: config.lang || 'chi_sim+eng',
        model: config.model || '',
        base_url: config.base_url || '',
        parameters: config.parameters || {},
        credential_config: config.credential_config || {},
        available_providers: Array.isArray(config.available_providers) && config.available_providers.length
            ? config.available_providers
            : ['disabled', 'tesseract', 'cloud_stub', 'local_json_ocr', 'llm_vision'],
        configured: !!config.configured,
        server_setup: config.server_setup || {},
    };
    const credentialConfig = ocrConfig.value.credential_config || {};
    ocrConfigForm.value = {
        provider: ocrConfig.value.provider,
        lang: ocrConfig.value.lang,
        model: ocrConfig.value.model || '',
        base_url: ocrConfig.value.base_url || '',
        parameters: JSON.stringify(ocrConfig.value.parameters || {}, null, 2),
        credential_mode: String(credentialConfig['credential_mode'] || 'api_key'),
        credential_json: JSON.stringify(credentialConfig['credential_json'] || {}, null, 2),
        token_endpoint: String(credentialConfig['token_endpoint'] || ''),
        refresh_headers: JSON.stringify(credentialConfig['refresh_headers'] || {}, null, 2),
        refresh_body: JSON.stringify(credentialConfig['refresh_body'] || {}, null, 2),
        refresh_params: JSON.stringify(credentialConfig['refresh_params'] || {}, null, 2),
        api_key: '',
        advancedMode: false,
    };
}

/**
 * 解析可选 JSON 对象字段，空字符串表示不提交该配置段。
 */
function parseOptionalJsonObject(text: string, label: string): Record<string, unknown> {
    const trimmed = text.trim();
    if (!trimmed) {
        return {};
    }
    const parsed = JSON.parse(trimmed) as unknown;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        throw new Error(`${label} must be a JSON object`);
    }
    return parsed as Record<string, unknown>;
}

/**
 * 根据 OCR 表单构造 credential_config，仅包含用户实际填写的凭据和刷新字段。
 */
function buildOcrCredentialConfig(): Record<string, unknown> {
    const credentialConfig: Record<string, unknown> = {
        credential_mode: ocrConfigForm.value.credential_mode,
    };
    if (ocrConfigForm.value.credential_mode === 'api_key') {
        const apiKey = ocrConfigForm.value.api_key.trim();
        if (apiKey) {
            credentialConfig['credential_json'] = { api_key: apiKey };
            return credentialConfig;
        }
        if (containsRedactedSecret(ocrConfig.value.credential_config)) {
            credentialConfig['preserve_existing'] = true;
        }
        return credentialConfig;
    }
    const credentialJson = parseOptionalJsonObject(ocrConfigForm.value.credential_json, 'Credential JSON');
    if (containsRedactedSecret(credentialJson)
        || (Object.keys(credentialJson).length === 0 && containsRedactedSecret(ocrConfig.value.credential_config))) {
        credentialConfig['preserve_existing'] = true;
        return credentialConfig;
    }
    if (Object.keys(credentialJson).length > 0) {
        credentialConfig['credential_json'] = credentialJson;
    }
    if (ocrConfigForm.value.token_endpoint.trim()) {
        credentialConfig['token_endpoint'] = ocrConfigForm.value.token_endpoint.trim();
    }
    for (const [targetKey, formKey, label] of [
        ['refresh_headers', 'refresh_headers', 'Refresh Headers'],
        ['refresh_body', 'refresh_body', 'Refresh Body'],
        ['refresh_params', 'refresh_params', 'Refresh Params'],
    ] as const) {
        const value = parseOptionalJsonObject(ocrConfigForm.value[formKey], label);
        if (Object.keys(value).length > 0) {
            credentialConfig[targetKey] = value;
        }
    }
    return credentialConfig;
}

function containsRedactedSecret(value: unknown): boolean {
    if (value === '********') {
        return true;
    }
    if (Array.isArray(value)) {
        return value.some(containsRedactedSecret);
    }
    if (value && typeof value === 'object') {
        return Object.values(value).some(containsRedactedSecret);
    }
    return false;
}

/**
 * 从嵌套错误 payload 中提取可读消息，限制递归深度避免异常响应导致死循环。
 */
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

    const nestedErrorText = 'error' in payload ? extractPayloadMessage(payload.error, depth + 1) : null;
    if (nestedErrorText) {
        return nestedErrorText;
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

function getRequestErrorMessage(requestError: unknown, fallback: string): string {
    if (axios.isAxiosError(requestError)) {
        return getPayloadErrorMessage(requestError.response?.data, fallback);
    }

    if (requestError instanceof Error && requestError.message) {
        return requestError.message;
    }

    return fallback;
}

/**
 * 加载 OCR 配置并填充表单；加载失败保持当前表单状态不打断规则中心页面。
 */
async function loadOCRConfig() {
    ocrConfigLoading.value = true;
    try {
        const resp = await services.getOCRConfig();
        if (resp.data?.success && resp.data.result) {
            applyOCRConfig(resp.data.result);
        }
    } catch { /* ignore config load errors */ }
    finally {
        ocrConfigLoading.value = false;
    }
}

/**
 * 保存 OCR 配置表单，提交前先校验 JSON 字段并组装 credential_config。
 */
async function saveOCRConfig() {
    ocrConfigSaving.value = true;
    try {
        const provider = ocrConfigForm.value.provider;
        const parameters = provider === 'disabled'
            ? {}
            : parseOptionalJsonObject(ocrConfigForm.value.parameters, 'Parameters JSON');
        const credentialConfig = provider === 'llm_vision' ? buildOcrCredentialConfig() : {};
        const resp = await services.updateOCRConfig({
            provider,
            lang: ocrConfigForm.value.lang.trim() || 'chi_sim+eng',
            model: provider === 'llm_vision' ? ocrConfigForm.value.model.trim() : '',
            base_url: provider === 'llm_vision' ? ocrConfigForm.value.base_url.trim() : '',
            parameters,
            credential_config: credentialConfig,
        });
        if (resp.data?.success && resp.data.result) {
            applyOCRConfig(resp.data.result);
            snackbar.value?.showMessage('OCR Config Saved');
        } else {
            error.value = getPayloadErrorMessage(resp.data?.result, 'Failed to save OCR config');
            snackbar.value?.showError({ message: error.value });
        }
    } catch (requestError: unknown) {
        error.value = getRequestErrorMessage(requestError, 'Failed to save OCR config');
        snackbar.value?.showError({ message: error.value });
    } finally {
        ocrConfigSaving.value = false;
    }
}

async function enableServerQuickSetup(): Promise<void> {
    if (!serverQuickSetupAvailable.value) {
        return;
    }
    selectOCRSetup(serverLocalModel.value.provider);
    await saveOCRConfig();
}

useExternalTemplateBindings(
    SettingsJsonImportExportButton,
    mdiRefresh,
    hideSectionTitle,
    hasHeaderActionsTarget,
    headerActionsTarget,
    ocrConfigLoading,
    credentialModeOptions,
    ocrProviderOptions,
    ocrSetupOptions,
    ocrLanguageOptions,
    serverLocalModel,
    serverQuickSetupAvailable,
    selectedOCRSetupOption,
    savedOCRSetupOption,
    ocrHasUnsavedChanges,
    ocrSaveActionLabel,
    usesRemoteVision,
    supportsLanguage,
    selectOCRSetup,
    enableServerQuickSetup,
    saveOCRConfig,
);

onMounted(() => {
    loadOCRConfig();
});
</script>

<style scoped src="./ocr-config/OcrConfigPanel.scss"></style>
