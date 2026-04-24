<template>
    <v-card variant="flat"
            class="investment-settings-card"
            :class="{ disabled: loadingRecognitionSettings || savingRecognitionSettings }">
        <template v-if="!props.hideHeader" #title>
            <div class="investment-settings-title d-flex flex-wrap align-center ga-2">
                <span>{{ tt('Investment Recognition Settings') }}</span>
                <v-btn density="compact"
                       color="default"
                       variant="text"
                       size="32"
                       :icon="true"
                       :loading="loadingRecognitionSettings"
                       @click="reloadRecognitionSettings(true)">
                    <template #loader>
                        <v-progress-circular indeterminate size="20"/>
                    </template>
                    <v-icon :icon="mdiRefresh" size="20" />
                    <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                </v-btn>
            </div>
        </template>

        <v-alert v-if="error" type="error" closable class="mx-4 mt-4 mb-0" @click:close="error = null">
            {{ error }}
        </v-alert>

        <v-card-text>
            <div class="investment-settings-form">
                <v-row dense>
                    <v-col cols="12">
                        <v-switch
                            v-model="recognitionSettings.importLearningEnabled"
                            color="primary"
                            density="compact"
                            hide-details
                            inset
                            :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                            :label="tt('Enable Import Learning')"
                        />
                    </v-col>

                    <v-col cols="12">
                        <v-combobox
                            v-model="recognitionSettings.investmentPlatformKeywords"
                            color="primary"
                            density="compact"
                            variant="outlined"
                            multiple
                            chips
                            closable-chips
                            hide-details
                            persistent-placeholder
                            :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                            :label="tt('Investment Platform Keywords')"
                            :placeholder="tt('Enter investment platform keywords')"
                        />
                    </v-col>

                    <v-col cols="12">
                        <v-combobox
                            v-model="recognitionSettings.investmentProductKeywords"
                            color="primary"
                            density="compact"
                            variant="outlined"
                            multiple
                            chips
                            closable-chips
                            hide-details
                            persistent-placeholder
                            :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                            :label="tt('Investment Product Keywords')"
                            :placeholder="tt('Enter investment product keywords')"
                        />
                    </v-col>

                    <v-col cols="12">
                        <v-combobox
                            v-model="recognitionSettings.investmentExcludeKeywords"
                            color="primary"
                            density="compact"
                            variant="outlined"
                            multiple
                            chips
                            closable-chips
                            hide-details
                            persistent-placeholder
                            :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                            :label="tt('Investment Exclude Keywords')"
                            :placeholder="tt('Enter investment exclude keywords')"
                        />
                    </v-col>
                </v-row>
            </div>
        </v-card-text>

        <v-card-actions class="investment-settings-actions px-4 pb-4">
            <v-btn color="primary"
                   variant="outlined"
                   size="small"
                   class="investment-settings-action"
                   :disabled="loadingRecognitionSettings || savingRecognitionSettings || !recognitionSettingsChanged"
                   @click="saveRecognitionSettings">
                {{ tt('Save Changes') }}
                <v-progress-circular v-if="savingRecognitionSettings" indeterminate size="20" class="ms-2" />
            </v-btn>
            <v-btn color="default"
                   variant="outlined"
                   size="small"
                   class="investment-settings-action"
                   :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                   @click="resetRecognitionSettings">
                {{ tt('Reset') }}
            </v-btn>
        </v-card-actions>
    </v-card>

    <snack-bar ref="snackbar" />
</template>

<script setup lang="ts">
import { computed, onMounted, ref, useTemplateRef } from 'vue';
import { mdiRefresh } from '@mdi/js';

import SnackBar from '@/components/desktop/SnackBar.vue';
import { useI18n } from '@/locales/helpers.ts';
import { isEquals } from '@/lib/common.ts';
import {
    EMPTY_PAIRING_CENTER_INVESTMENT_SETTINGS,
    normalizePairingCenterInvestmentSettings,
    toPairingCenterInvestmentSettingsUpdateRequest,
    type PairingCenterInvestmentSettings
} from '@/models/pairing_center.ts';
import { useUserStore } from '@/stores/user.ts';

type RecognitionSettingsState = PairingCenterInvestmentSettings;
type SnackBarType = InstanceType<typeof SnackBar>;

const props = defineProps<{
    hideHeader?: boolean;
}>();

const { tt } = useI18n();
const userStore = useUserStore();
const snackbar = useTemplateRef<SnackBarType>('snackbar');

const loadingRecognitionSettings = ref<boolean>(true);
const savingRecognitionSettings = ref<boolean>(false);
const error = ref<string | null>(null);
const recognitionSettings = ref<RecognitionSettingsState>(createEmptyRecognitionSettings());
const recognitionSettingsSnapshot = ref<RecognitionSettingsState>(createEmptyRecognitionSettings());

const recognitionSettingsChanged = computed<boolean>(() => {
    return recognitionSettings.value.importLearningEnabled !== recognitionSettingsSnapshot.value.importLearningEnabled
        || !isEquals(recognitionSettings.value.investmentPlatformKeywords, recognitionSettingsSnapshot.value.investmentPlatformKeywords)
        || !isEquals(recognitionSettings.value.investmentProductKeywords, recognitionSettingsSnapshot.value.investmentProductKeywords)
        || !isEquals(recognitionSettings.value.investmentExcludeKeywords, recognitionSettingsSnapshot.value.investmentExcludeKeywords);
});

function createEmptyRecognitionSettings(): RecognitionSettingsState {
    return normalizePairingCenterInvestmentSettings(EMPTY_PAIRING_CENTER_INVESTMENT_SETTINGS);
}

function resetRecognitionSettings(): void {
    recognitionSettings.value = normalizePairingCenterInvestmentSettings(recognitionSettingsSnapshot.value);
    error.value = null;
}

function showSuccessMessage(message: string): void {
    snackbar.value?.showMessage(message);
}

function extractPayloadMessage(payload: unknown, depth = 0): string | null {
    if (depth > 2) {
        return null;
    }

    if (typeof payload === 'string' && payload) {
        return payload;
    }

    if (payload instanceof Error && payload.message) {
        return payload.message;
    }

    if (!payload || typeof payload !== 'object') {
        return null;
    }

    const typedPayload = payload as {
        errorMessage?: unknown;
        message?: unknown;
        error?: unknown;
    };

    return extractPayloadMessage(
        typedPayload.errorMessage ?? typedPayload.message ?? typedPayload.error,
        depth + 1
    );
}

function getErrorMessage(error: unknown, fallback: string): string {
    return extractPayloadMessage(error) || fallback;
}

async function reloadRecognitionSettings(force: boolean): Promise<void> {
    loadingRecognitionSettings.value = true;
    error.value = null;

    try {
        const nextState = normalizePairingCenterInvestmentSettings(
            await userStore.getPairingInvestmentSettings()
        );

        if (force) {
            showSuccessMessage(isEquals(recognitionSettingsSnapshot.value, nextState)
                ? 'Data is up to date'
                : 'Data has been updated');
        }

        recognitionSettingsSnapshot.value = nextState;
        recognitionSettings.value = normalizePairingCenterInvestmentSettings(nextState);
    } catch (caughtError: unknown) {
        error.value = getErrorMessage(caughtError, tt('Failed to load investment recognition settings'));
    } finally {
        loadingRecognitionSettings.value = false;
    }
}

async function saveRecognitionSettings(): Promise<void> {
    if (!recognitionSettingsChanged.value || savingRecognitionSettings.value) {
        return;
    }

    savingRecognitionSettings.value = true;
    error.value = null;

    try {
        const response = await userStore.updatePairingInvestmentSettings(
            toPairingCenterInvestmentSettingsUpdateRequest(recognitionSettings.value)
        );
        recognitionSettingsSnapshot.value = normalizePairingCenterInvestmentSettings(response);
        recognitionSettings.value = normalizePairingCenterInvestmentSettings(recognitionSettingsSnapshot.value);
        showSuccessMessage('Investment recognition settings updated');
    } catch (caughtError: unknown) {
        error.value = getErrorMessage(caughtError, tt('Failed to update investment recognition settings'));
    } finally {
        savingRecognitionSettings.value = false;
    }
}

onMounted(() => {
    reloadRecognitionSettings(false);
});

defineExpose({
    refresh: () => reloadRecognitionSettings(true),
});
</script>

<style scoped>
.investment-settings-card {
    background: transparent;
}

.investment-settings-title {
    min-height: 32px;
}

.investment-settings-form {
    max-width: 760px;
}

.investment-settings-actions {
    justify-content: flex-start;
    max-width: 760px;
}

.investment-settings-action {
    min-width: 120px;
}
</style>
