<template>
    <v-card :class="{ disabled: loadingRecognitionSettings || savingRecognitionSettings }">
        <template #title>
            <div class="d-flex align-center">
                <span>{{ tt('Investment Recognition Settings') }}</span>
                <v-btn density="compact" color="default" variant="text" size="24"
                       class="ms-2" :icon="true" :loading="loadingRecognitionSettings"
                       @click="reloadRecognitionSettings(true)">
                    <template #loader>
                        <v-progress-circular indeterminate size="20"/>
                    </template>
                    <v-icon :icon="mdiRefresh" size="24" />
                    <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                </v-btn>
            </div>
        </template>

        <v-alert v-if="error" type="error" closable class="mx-4 mt-4 mb-0" @click:close="error = null">
            {{ error }}
        </v-alert>

        <v-alert v-if="successMessage" type="success" closable class="mx-4 mt-4 mb-0" @click:close="successMessage = null">
            {{ successMessage }}
        </v-alert>

        <v-card-text>
            <v-row>
                <v-col cols="12" md="6">
                    <v-switch
                        v-model="recognitionSettings.importLearningEnabled"
                        color="primary"
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
                        multiple
                        chips
                        closable-chips
                        persistent-placeholder
                        persistent-hint
                        :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                        :label="tt('Investment Platform Keywords')"
                        :placeholder="tt('Enter investment platform keywords')"
                        :hint="tt('Press Enter to add investment platform keywords for import recognition')"
                    />
                </v-col>

                <v-col cols="12">
                    <v-combobox
                        v-model="recognitionSettings.investmentProductKeywords"
                        color="primary"
                        multiple
                        chips
                        closable-chips
                        persistent-placeholder
                        persistent-hint
                        :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                        :label="tt('Investment Product Keywords')"
                        :placeholder="tt('Enter investment product keywords')"
                        :hint="tt('Press Enter to add investment product keywords for import recognition')"
                    />
                </v-col>

                <v-col cols="12">
                    <v-combobox
                        v-model="recognitionSettings.investmentExcludeKeywords"
                        color="primary"
                        multiple
                        chips
                        closable-chips
                        persistent-placeholder
                        persistent-hint
                        :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                        :label="tt('Investment Exclude Keywords')"
                        :placeholder="tt('Enter investment exclude keywords')"
                        :hint="tt('Press Enter to add keywords that should block investment recognition')"
                    />
                </v-col>
            </v-row>
        </v-card-text>

        <v-card-actions class="px-4 pb-4">
            <v-btn color="primary"
                   :disabled="loadingRecognitionSettings || savingRecognitionSettings || !recognitionSettingsChanged"
                   @click="saveRecognitionSettings">
                {{ tt('Save Changes') }}
                <v-progress-circular v-if="savingRecognitionSettings" indeterminate size="20" class="ms-2" />
            </v-btn>
            <v-btn color="default" variant="tonal"
                   :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                   @click="resetRecognitionSettings">
                {{ tt('Reset') }}
            </v-btn>
        </v-card-actions>
    </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { mdiRefresh } from '@mdi/js';

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

const { tt } = useI18n();
const userStore = useUserStore();

const loadingRecognitionSettings = ref<boolean>(true);
const savingRecognitionSettings = ref<boolean>(false);
const error = ref<string | null>(null);
const successMessage = ref<string | null>(null);
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
    successMessage.value = null;
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
            successMessage.value = isEquals(recognitionSettingsSnapshot.value, nextState)
                ? 'Data is up to date'
                : 'Data has been updated';
        }

        recognitionSettingsSnapshot.value = nextState;
        recognitionSettings.value = normalizePairingCenterInvestmentSettings(nextState);
    } catch (caughtError: unknown) {
        successMessage.value = null;
        error.value = getErrorMessage(caughtError, 'Failed to load investment recognition settings');
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
    successMessage.value = null;

    try {
        const response = await userStore.updatePairingInvestmentSettings(
            toPairingCenterInvestmentSettingsUpdateRequest(recognitionSettings.value)
        );
        recognitionSettingsSnapshot.value = normalizePairingCenterInvestmentSettings(response);
        recognitionSettings.value = normalizePairingCenterInvestmentSettings(recognitionSettingsSnapshot.value);
        successMessage.value = 'Investment recognition settings updated';
    } catch (caughtError: unknown) {
        error.value = getErrorMessage(caughtError, 'Failed to update investment recognition settings');
    } finally {
        savingRecognitionSettings.value = false;
    }
}

onMounted(() => {
    reloadRecognitionSettings(false);
});
</script>
