<template>
    <div>
        <div class="learning-section-header"
             :class="{ 'learning-section-header--empty': hideSectionTitle && hasHeaderActionsTarget }">
            <h3 v-if="!hideSectionTitle" class="learning-section-title">{{ tt('OCR Config') }}</h3>
            <Teleport :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">
                <div class="learning-section-actions"
                     :class="{ 'learning-section-actions--external': hasHeaderActionsTarget }">
                    <settings-json-import-export-button
                        section-key="ocrConfig"
                        filename-prefix="ocr-config"
                        :disabled="ocrConfigLoading || ocrConfigSaving"
                        password-required-for-export
                        @imported="loadOCRConfig"
                    />
                    <v-btn class="learning-panel-refresh"
                           variant="text"
                           color="default"
                           :icon="true"
                           :loading="ocrConfigLoading"
                           @click="loadOCRConfig">
                        <v-icon :icon="mdiRefresh" />
                        <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                    </v-btn>
                </div>
            </Teleport>
        </div>

        <v-card variant="outlined" class="mb-4">
            <v-card-title class="text-subtitle-1 d-flex align-center">
                <span>{{ tt('OCR Config') }}</span>
                <v-spacer />
                <v-chip size="small"
                        :color="ocrConfig.configured ? 'success' : 'warning'"
                        variant="tonal">
                    {{ tt(ocrConfig.configured ? 'Enabled' : 'Disabled') }}
                </v-chip>
            </v-card-title>
            <v-divider />
            <v-card-text class="pt-4">
                <v-row>
                    <v-col cols="12" md="5">
                        <v-select v-model="ocrConfigForm.provider"
                                  :items="ocrProviderOptions"
                                  item-title="title"
                                  item-value="value"
                                  :label="tt('Provider')"
                                  variant="outlined"
                                  density="comfortable"
                                  hide-details
                                  :loading="ocrConfigLoading"
                                  :disabled="ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="5">
                        <v-text-field v-model="ocrConfigForm.lang"
                                      :label="tt('OCR Language')"
                                      variant="outlined"
                                      density="comfortable"
                                      hide-details
                                      :disabled="ocrConfigForm.provider === 'disabled' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="6">
                        <v-text-field v-model="ocrConfigForm.model"
                                      :label="tt('Model')"
                                      variant="outlined"
                                      density="comfortable"
                                      hide-details
                                      :disabled="ocrConfigForm.provider === 'disabled' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="6">
                        <v-text-field v-model="ocrConfigForm.base_url"
                                      :label="tt('Base URL')"
                                      variant="outlined"
                                      density="comfortable"
                                      hide-details
                                      :disabled="ocrConfigForm.provider !== 'llm_vision' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="6">
                        <v-select v-model="ocrConfigForm.credential_mode"
                                  :label="tt('Credential Mode')"
                                  :items="credentialModeOptions"
                                  item-title="title"
                                  item-value="value"
                                  variant="outlined"
                                  density="comfortable"
                                  hide-details
                                  :disabled="ocrConfigForm.provider !== 'llm_vision' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="6">
                        <v-text-field v-model="ocrConfigForm.token_endpoint"
                                      :label="tt('Token Endpoint')"
                                      variant="outlined"
                                      density="comfortable"
                                      hide-details
                                      :disabled="ocrConfigForm.provider !== 'llm_vision' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="6">
                        <v-textarea v-model="ocrConfigForm.credential_json"
                                    :label="tt('Credential JSON')"
                                    variant="outlined"
                                    density="comfortable"
                                    rows="3"
                                    auto-grow
                                    hide-details
                                    :disabled="ocrConfigForm.provider !== 'llm_vision' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="6">
                        <v-textarea v-model="ocrConfigForm.parameters"
                                    :label="tt('Parameters JSON')"
                                    variant="outlined"
                                    density="comfortable"
                                    rows="3"
                                    auto-grow
                                    hide-details
                                    :disabled="ocrConfigForm.provider === 'disabled' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="4">
                        <v-textarea v-model="ocrConfigForm.refresh_headers"
                                    :label="tt('Refresh Headers')"
                                    variant="outlined"
                                    density="comfortable"
                                    rows="2"
                                    auto-grow
                                    hide-details
                                    :disabled="ocrConfigForm.provider !== 'llm_vision' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="4">
                        <v-textarea v-model="ocrConfigForm.refresh_body"
                                    :label="tt('Refresh Body')"
                                    variant="outlined"
                                    density="comfortable"
                                    rows="2"
                                    auto-grow
                                    hide-details
                                    :disabled="ocrConfigForm.provider !== 'llm_vision' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="4">
                        <v-textarea v-model="ocrConfigForm.refresh_params"
                                    :label="tt('Refresh Params')"
                                    variant="outlined"
                                    density="comfortable"
                                    rows="2"
                                    auto-grow
                                    hide-details
                                    :disabled="ocrConfigForm.provider !== 'llm_vision' || ocrConfigLoading || ocrConfigSaving" />
                    </v-col>
                    <v-col cols="12" md="2" class="d-flex align-center">
                        <v-btn class="learning-panel-action w-100"
                               variant="outlined"
                               color="default"
                               :loading="ocrConfigSaving"
                               :disabled="ocrConfigLoading"
                               @click="saveOCRConfig">
                            {{ tt('Save') }}
                        </v-btn>
                    </v-col>
                </v-row>
            </v-card-text>
        </v-card>

        <snack-bar ref="snackbar" />
    </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, useTemplateRef } from 'vue';
import axios from 'axios';

import SettingsJsonImportExportButton from '@/components/desktop/SettingsJsonImportExportButton.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import { useI18n } from '@/locales/helpers.ts';
import services, { type OCRConfigResponse } from '@/lib/services.ts';

import { mdiRefresh } from '@mdi/js';

interface SelectOption {
    title: string;
    value: string;
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

const ocrProviderOptions = computed<SelectOption[]>(() => {
    const providers = ocrConfig.value.available_providers.length
        ? ocrConfig.value.available_providers
        : ['disabled', 'tesseract', 'cloud_stub', 'local_json_ocr', 'llm_vision'];
    return providers.map(provider => ({
        title: ocrProviderLabel(provider),
        value: provider,
    }));
});

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
    const credentialJson = parseOptionalJsonObject(ocrConfigForm.value.credential_json, 'Credential JSON');
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
        const parameters = parseOptionalJsonObject(ocrConfigForm.value.parameters, 'Parameters JSON');
        const credentialConfig = buildOcrCredentialConfig();
        const resp = await services.updateOCRConfig({
            provider: ocrConfigForm.value.provider,
            lang: ocrConfigForm.value.lang.trim() || 'chi_sim+eng',
            model: ocrConfigForm.value.model.trim(),
            base_url: ocrConfigForm.value.base_url.trim(),
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

onMounted(() => {
    loadOCRConfig();
});
</script>

<style scoped>
.learning-section-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
}

.learning-section-header--empty {
    min-height: 0;
    margin-bottom: 0;
}

.learning-section-title {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 600;
}

.learning-section-actions {
    display: flex;
    flex: 1 1 auto;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
}

.learning-section-actions--external {
    min-width: 0;
    width: 100%;
}

.learning-panel-action {
    height: 38px;
    min-height: 38px;
    min-width: 136px;
}

.learning-panel-refresh {
    width: 38px;
    height: 38px;
    min-width: 38px;
}
</style>
