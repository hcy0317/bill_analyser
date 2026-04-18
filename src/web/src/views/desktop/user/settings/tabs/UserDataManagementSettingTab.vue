<template>
    <v-row>
        <v-col cols="12">
            <v-card :class="{ 'disabled': loadingDataStatistics }">
                <template #title>
                    <div class="d-flex align-center">
                        <span>{{ tt('Data Management') }}</span>
                        <v-btn density="compact" color="default" variant="text" size="24"
                               class="ms-2" :icon="true" :loading="loadingDataStatistics" @click="reloadUserDataStatistics(true)">
                            <template #loader>
                                <v-progress-circular indeterminate size="20"/>
                            </template>
                            <v-icon :icon="mdiRefresh" size="24" />
                            <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                        </v-btn>
                    </div>
                </template>

                <v-card-text>
                    <v-row>
                        <v-col cols="6" sm="3" :key="idx" v-for="(item, idx) in [
                            {
                                title: 'Transactions',
                                count: displayDataStatistics ? displayDataStatistics.totalTransactionCount : '-',
                                icon: mdiListBoxOutline,
                                color: 'info-darken-1'
                            },
                            {
                                title: 'Accounts',
                                count: displayDataStatistics ? displayDataStatistics.totalAccountCount : '-',
                                icon: mdiCreditCardOutline,
                                color: 'primary'
                            },
                            {
                                title: 'Transaction Categories',
                                count: displayDataStatistics ? displayDataStatistics.totalTransactionCategoryCount : '-',
                                icon: mdiViewDashboardOutline,
                                color: 'teal'
                            },
                            {
                                title: 'Transaction Tags',
                                count: displayDataStatistics ? displayDataStatistics.totalTransactionTagCount : '-',
                                icon: mdiTagOutline,
                                color: 'grey'
                            },
                            {
                                title: 'Transaction Pictures',
                                count: displayDataStatistics ? displayDataStatistics.totalTransactionPictureCount : '-',
                                icon: mdiImage,
                                color: 'error-darken-1'
                            },
                            {
                                title: 'Transaction Templates',
                                count: displayDataStatistics ? displayDataStatistics.totalTransactionTemplateCount : '-',
                                icon: mdiClipboardTextOutline,
                                color: 'secondary-darken-1'
                            },
                            {
                                title: 'Scheduled Transactions',
                                count: displayDataStatistics ? displayDataStatistics.totalScheduledTransactionCount : '-',
                                icon: mdiClipboardTextClockOutline,
                                color: 'success-darken-1'
                            }
                        ]">
                            <div class="d-flex align-center">
                                <div class="me-3">
                                    <v-avatar rounded :color="item.color" size="42" class="elevation-1">
                                        <v-icon size="24" :icon="item.icon"/>
                                    </v-avatar>
                                </div>

                                <div class="d-flex flex-column">
                                    <span class="text-caption">{{ tt(item.title) }}</span>
                                    <v-skeleton-loader class="skeleton-no-margin pt-2 pb-2" type="text" style="width: 60px" :loading="true" v-if="loadingDataStatistics"></v-skeleton-loader>
                                    <span class="text-xl" v-if="!loadingDataStatistics">{{ item.count }}</span>
                                </div>
                            </div>
                        </v-col>
                    </v-row>
                </v-card-text>
            </v-card>
        </v-col>

        <v-col cols="12">
            <v-card :class="{ 'disabled': loadingRecognitionSettings || savingRecognitionSettings }">
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

                <v-card-text>
                    <v-row>
                        <v-col cols="12" md="6">
                            <v-switch
                                color="primary"
                                hide-details
                                inset
                                :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                                :label="tt('Enable Import Learning')"
                                v-model="recognitionSettings.importLearningEnabled"
                            />
                        </v-col>

                        <v-col cols="12" md="12">
                            <v-combobox
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
                                v-model="recognitionSettings.investmentPlatformKeywords"
                            />
                        </v-col>

                        <v-col cols="12" md="12">
                            <v-combobox
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
                                v-model="recognitionSettings.investmentProductKeywords"
                            />
                        </v-col>

                        <v-col cols="12" md="12">
                            <v-combobox
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
                                v-model="recognitionSettings.investmentExcludeKeywords"
                            />
                        </v-col>
                    </v-row>
                </v-card-text>

                <v-card-actions class="px-4 pb-4">
                    <v-btn color="primary"
                           :disabled="loadingRecognitionSettings || savingRecognitionSettings || !recognitionSettingsChanged"
                           @click="saveRecognitionSettings">
                        {{ tt('Save Changes') }}
                        <v-progress-circular indeterminate size="20" class="ms-2" v-if="savingRecognitionSettings" />
                    </v-btn>
                    <v-btn color="default" variant="tonal"
                           :disabled="loadingRecognitionSettings || savingRecognitionSettings"
                           @click="resetRecognitionSettings">
                        {{ tt('Reset') }}
                    </v-btn>
                </v-card-actions>
            </v-card>
        </v-col>

        <v-col cols="12" v-if="isDataExportingEnabled()">
            <v-card :class="{ 'disabled': exportingData }" :title="tt('Export Data')">
                <v-card-text>
                    <span class="text-body-1">{{ tt('Export all transaction data to file.') }}&nbsp;{{ tt('It may take a long time, please wait for a few minutes.') }}</span>
                </v-card-text>

                <v-card-text class="d-flex flex-wrap gap-4">
                    <v-btn-group variant="elevated" density="comfortable" color="primary">
                        <v-btn :disabled="loadingDataStatistics || exportingData || !dataStatistics || !dataStatistics.totalTransactionCount || dataStatistics.totalTransactionCount === '0'">
                            {{ tt('Export Data') }}
                            <v-progress-circular indeterminate size="22" class="ms-2" v-if="exportingData"></v-progress-circular>
                            <v-menu activator="parent">
                                <v-list :disabled="loadingDataStatistics || exportingData || !dataStatistics || !dataStatistics.totalTransactionCount || dataStatistics.totalTransactionCount === '0'">
                                    <v-list-item @click="exportData('csv')">
                                        <v-list-item-title>{{ tt('CSV (Comma-separated values) File') }}</v-list-item-title>
                                    </v-list-item>
                                    <v-list-item @click="exportData('tsv')">
                                        <v-list-item-title>{{ tt('TSV (Tab-separated values) File') }}</v-list-item-title>
                                    </v-list-item>
                                </v-list>
                            </v-menu>
                        </v-btn>
                    </v-btn-group>
                </v-card-text>
            </v-card>
        </v-col>

        <v-col cols="12">
            <v-card :class="{ 'disabled': clearingData }">
                <template #title>
                    <span class="text-error">{{ tt('Danger Zone') }}</span>
                </template>

                <v-card-text class="py-0">
                    <span class="text-body-1 text-error">
                        <v-icon class="mt-n1" :icon="mdiAlert"/>
                        {{ tt('You CANNOT undo this action. "Clear All Transactions" will clear all your transactions data, and "Clear All Data" will clear your accounts, categories, tags and transactions data. Please enter your current password to confirm.') }}
                    </span>
                </v-card-text>

                <v-card-text class="pb-0">
                    <v-row class="mb-3">
                        <v-col cols="12" md="6">
                            <v-text-field
                                autocomplete="current-password"
                                ref="currentPasswordInput"
                                type="password"
                                variant="underlined"
                                color="error"
                                :disabled="loadingDataStatistics || clearingData"
                                :placeholder="tt('Current Password')"
                                v-model="currentPasswordForClearData"
                            />
                        </v-col>
                    </v-row>
                </v-card-text>

                <v-card-text class="d-flex flex-wrap gap-4">
                    <v-btn color="error" :disabled="loadingDataStatistics || !currentPasswordForClearData || clearingData">
                        {{ tt('Clear User Data') }}
                        <v-progress-circular indeterminate size="22" class="ms-2" v-if="clearingData"></v-progress-circular>
                        <v-menu activator="parent">
                            <v-list :disabled="loadingDataStatistics || !currentPasswordForClearData || clearingData">
                                <v-list-item @click="clearAllTransactions">
                                    <v-list-item-title>{{ tt('Clear All Transactions') }}</v-list-item-title>
                                </v-list-item>
                                <v-list-item @click="clearAllData">
                                    <v-list-item-title>{{ tt('Clear All Data') }}</v-list-item-title>
                                </v-list-item>
                            </v-list>
                        </v-menu>
                    </v-btn>
                </v-card-text>
            </v-card>
        </v-col>
    </v-row>

    <confirm-dialog ref="confirmDialog"/>
    <snack-bar ref="snackbar" />
</template>

<script setup lang="ts">
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';

import { computed, onMounted, ref, useTemplateRef } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { useDataManagementPageBase } from '@/views/base/users/DataManagementPageBase.ts';

import { useRootStore } from '@/stores/index.ts';
import { useUserStore } from '@/stores/user.ts';

import { isEquals } from '@/lib/common.ts';
import { isDataExportingEnabled } from '@/lib/server_settings.ts';
import { startDownloadFile } from '@/lib/ui/common.ts';
import type { UserProfileUpdateRequest } from '@/models/user.ts';

import {
    mdiRefresh,
    mdiListBoxOutline,
    mdiCreditCardOutline,
    mdiViewDashboardOutline,
    mdiTagOutline,
    mdiClipboardTextOutline,
    mdiImage,
    mdiClipboardTextClockOutline,
    mdiAlert
} from '@mdi/js';

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;

interface RecognitionSettingsState {
    importLearningEnabled: boolean;
    investmentPlatformKeywords: string[];
    investmentProductKeywords: string[];
    investmentExcludeKeywords: string[];
}

const { tt } = useI18n();
const { dataStatistics, displayDataStatistics, getExportFileName } = useDataManagementPageBase();

const rootStore = useRootStore();
const userStore = useUserStore();

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');

const loadingDataStatistics = ref<boolean>(true);
const exportingData = ref<boolean>(false);
const currentPasswordForClearData = ref<string>('');
const clearingData = ref<boolean>(false);
const loadingRecognitionSettings = ref<boolean>(true);
const savingRecognitionSettings = ref<boolean>(false);
const recognitionSettings = ref<RecognitionSettingsState>({
    importLearningEnabled: false,
    investmentPlatformKeywords: [],
    investmentProductKeywords: [],
    investmentExcludeKeywords: []
});
const recognitionSettingsSnapshot = ref<RecognitionSettingsState>({
    importLearningEnabled: false,
    investmentPlatformKeywords: [],
    investmentProductKeywords: [],
    investmentExcludeKeywords: []
});

const recognitionSettingsChanged = computed<boolean>(() => {
    return recognitionSettings.value.importLearningEnabled !== recognitionSettingsSnapshot.value.importLearningEnabled
        || !isEquals(recognitionSettings.value.investmentPlatformKeywords, recognitionSettingsSnapshot.value.investmentPlatformKeywords)
        || !isEquals(recognitionSettings.value.investmentProductKeywords, recognitionSettingsSnapshot.value.investmentProductKeywords)
        || !isEquals(recognitionSettings.value.investmentExcludeKeywords, recognitionSettingsSnapshot.value.investmentExcludeKeywords);
});

function reloadUserDataStatistics(force: boolean): void {
    loadingDataStatistics.value = true;

    userStore.getUserDataStatistics().then(dataStatisticsResponse => {
        if (force) {
            if (isEquals(dataStatistics.value, dataStatisticsResponse)) {
                snackbar.value?.showMessage('Data is up to date');
            } else {
                snackbar.value?.showMessage('Data has been updated');
            }
        }

        dataStatistics.value = dataStatisticsResponse;
        loadingDataStatistics.value = false;
    }).catch(error => {
        loadingDataStatistics.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function normalizeRecognitionSettings(profile: {
    importLearningEnabled?: boolean;
    investmentPlatformKeywords?: string[];
    investmentProductKeywords?: string[];
    investmentExcludeKeywords?: string[];
}): RecognitionSettingsState {
    return {
        importLearningEnabled: !!profile.importLearningEnabled,
        investmentPlatformKeywords: [...(profile.investmentPlatformKeywords || [])],
        investmentProductKeywords: [...(profile.investmentProductKeywords || [])],
        investmentExcludeKeywords: [...(profile.investmentExcludeKeywords || [])]
    };
}

function resetRecognitionSettings(): void {
    recognitionSettings.value = normalizeRecognitionSettings(recognitionSettingsSnapshot.value);
}

function reloadRecognitionSettings(force: boolean): void {
    loadingRecognitionSettings.value = true;

    userStore.getCurrentUserProfile().then(profile => {
        const nextState = normalizeRecognitionSettings(profile);

        if (force) {
            if (isEquals(recognitionSettingsSnapshot.value, nextState)) {
                snackbar.value?.showMessage('Data is up to date');
            } else {
                snackbar.value?.showMessage('Data has been updated');
            }
        }

        recognitionSettingsSnapshot.value = nextState;
        recognitionSettings.value = normalizeRecognitionSettings(nextState);
        loadingRecognitionSettings.value = false;
    }).catch(error => {
        loadingRecognitionSettings.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function saveRecognitionSettings(): void {
    if (!recognitionSettingsChanged.value || savingRecognitionSettings.value) {
        return;
    }

    savingRecognitionSettings.value = true;

    const request: UserProfileUpdateRequest = {
        importLearningEnabled: recognitionSettings.value.importLearningEnabled,
        investmentPlatformKeywords: [...recognitionSettings.value.investmentPlatformKeywords],
        investmentProductKeywords: [...recognitionSettings.value.investmentProductKeywords],
        investmentExcludeKeywords: [...recognitionSettings.value.investmentExcludeKeywords]
    };

    rootStore.updateUserProfile(request).then(response => {
        recognitionSettingsSnapshot.value = normalizeRecognitionSettings(response.user || recognitionSettings.value);
        recognitionSettings.value = normalizeRecognitionSettings(recognitionSettingsSnapshot.value);
        savingRecognitionSettings.value = false;
        snackbar.value?.showMessage('Your profile has been successfully updated');
    }).catch(error => {
        savingRecognitionSettings.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function exportData(fileType: string): void {
    if (exportingData.value) {
        return;
    }

    exportingData.value = true;

    userStore.getExportedUserData(fileType).then(data => {
        startDownloadFile(getExportFileName(fileType), data);
        exportingData.value = false;
    }).catch(error => {
        exportingData.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function clearAllTransactions(): void {
    if (!currentPasswordForClearData.value) {
        snackbar.value?.showMessage('Current password cannot be blank');
        return;
    }

    if (clearingData.value) {
        return;
    }

    confirmDialog.value?.open('Are you sure you want to clear all transactions?', { color: 'error' }).then(() => {
        clearingData.value = true;

        rootStore.clearAllUserTransactions({
            password: currentPasswordForClearData.value
        }).then(() => {
            clearingData.value = false;
            currentPasswordForClearData.value = '';

            snackbar.value?.showMessage('All transactions has been cleared');
            reloadUserDataStatistics(false);
        }).catch(error => {
            clearingData.value = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    });
}

function clearAllData(): void {
    if (!currentPasswordForClearData.value) {
        snackbar.value?.showMessage('Current password cannot be blank');
        return;
    }

    if (clearingData.value) {
        return;
    }

    confirmDialog.value?.open('Are you sure you want to clear all data?', { color: 'error' }).then(() => {
        clearingData.value = true;

        rootStore.clearAllUserData({
            password: currentPasswordForClearData.value
        }).then(() => {
            clearingData.value = false;
            currentPasswordForClearData.value = '';

            snackbar.value?.showMessage('All user data has been cleared');
            reloadUserDataStatistics(false);
        }).catch(error => {
            clearingData.value = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    });
}

onMounted(() => {
    reloadUserDataStatistics(false);
    reloadRecognitionSettings(false);
});

defineExpose({
    reloadUserDataStatistics,
    reloadRecognitionSettings
});
</script>
