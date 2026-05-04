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
    available_providers: ['disabled', 'tesseract', 'cloud_stub'],
    configured: false,
});
const ocrConfigForm = ref<{ provider: string; lang: string }>({
    provider: 'disabled',
    lang: 'chi_sim+eng',
});

const ocrProviderOptions = computed<SelectOption[]>(() => {
    const providers = ocrConfig.value.available_providers.length
        ? ocrConfig.value.available_providers
        : ['disabled', 'tesseract', 'cloud_stub'];
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
    };
    return labels[provider] ?? provider;
}

function applyOCRConfig(config: OCRConfigResponse): void {
    ocrConfig.value = {
        provider: config.provider || 'disabled',
        lang: config.lang || 'chi_sim+eng',
        available_providers: Array.isArray(config.available_providers) && config.available_providers.length
            ? config.available_providers
            : ['disabled', 'tesseract', 'cloud_stub'],
        configured: !!config.configured,
    };
    ocrConfigForm.value = {
        provider: ocrConfig.value.provider,
        lang: ocrConfig.value.lang,
    };
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

function getRequestErrorMessage(requestError: unknown, fallback: string): string {
    if (axios.isAxiosError(requestError)) {
        return getPayloadErrorMessage(requestError.response?.data, fallback);
    }

    if (requestError instanceof Error && requestError.message) {
        return requestError.message;
    }

    return fallback;
}

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

async function saveOCRConfig() {
    ocrConfigSaving.value = true;
    try {
        const resp = await services.updateOCRConfig({
            provider: ocrConfigForm.value.provider,
            lang: ocrConfigForm.value.lang.trim() || 'chi_sim+eng',
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
