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

        <v-col cols="12">
            <v-card :class="{ 'disabled': loadingImportLearningRules }">
                <template #title>
                    <div class="d-flex align-center">
                        <span>{{ tt('Import Learning Rules') }}</span>
                        <v-btn density="compact" color="default" variant="text" size="24"
                               class="ms-2" :icon="true" :loading="loadingImportLearningRules"
                               @click="reloadImportLearningRules(true)">
                            <template #loader>
                                <v-progress-circular indeterminate size="20"/>
                            </template>
                            <v-icon :icon="mdiRefresh" size="24" />
                            <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                        </v-btn>
                    </div>
                </template>

                <v-card-text v-if="loadingImportLearningRules">
                    <v-skeleton-loader type="table-row-divider@4" />
                </v-card-text>

                <v-card-text v-else-if="!importLearningRules.length">
                    <span class="text-body-2 text-medium-emphasis">{{ tt('No import learning rules yet') }}</span>
                </v-card-text>

                <template v-else>
                    <v-card-text class="pt-0 pb-2 d-flex align-center flex-wrap gap-2">
                        <v-checkbox-btn
                            density="compact"
                            color="primary"
                            :disabled="loadingImportLearningRules || bulkUpdatingImportLearningRules"
                            :indeterminate="anyButNotAllImportLearningRulesSelected"
                            v-model="allImportLearningRulesSelected"
                        />
                        <span class="text-body-2 text-medium-emphasis">
                            {{ tt('Selected import learning rules: {count}', { count: selectedImportLearningRuleIds.length }) }}
                        </span>
                        <span class="text-body-2 text-medium-emphasis ms-2">
                            {{ tt('Showing Count', { visible: importLearningRules.length, total: importLearningRulesTotalCount }) }}
                        </span>
                        <v-btn size="small" density="comfortable" variant="text" color="primary"
                               :disabled="!hasSelectedImportLearningRules || bulkUpdatingImportLearningRules"
                               @click="setSelectedImportLearningRulesEnabled(true)">
                            {{ tt('Enable Selected') }}
                        </v-btn>
                        <v-btn size="small" density="comfortable" variant="text" color="secondary"
                               :disabled="!hasSelectedImportLearningRules || bulkUpdatingImportLearningRules"
                               @click="setSelectedImportLearningRulesEnabled(false)">
                            {{ tt('Disable Selected') }}
                        </v-btn>
                        <v-btn size="small" density="comfortable" variant="text" color="error"
                               :disabled="!hasSelectedImportLearningRules || bulkUpdatingImportLearningRules"
                               @click="deleteSelectedImportLearningRules">
                            {{ tt('Delete Selected') }}
                        </v-btn>
                    </v-card-text>

                    <v-table density="compact">
                    <thead>
                        <tr>
                            <th class="text-center" style="width: 48px">
                                <v-checkbox-btn
                                    density="compact"
                                    color="primary"
                                    :disabled="loadingImportLearningRules || bulkUpdatingImportLearningRules"
                                    :indeterminate="anyButNotAllImportLearningRulesSelected"
                                    v-model="allImportLearningRulesSelected"
                                />
                            </th>
                            <th>{{ tt('Match Field') }}</th>
                            <th>{{ tt('Match Value') }}</th>
                            <th>{{ tt('Learned Result') }}</th>
                            <th>{{ tt('Hits') }}</th>
                            <th>{{ tt('Status') }}</th>
                            <th>{{ tt('Actions') }}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr v-for="rule in importLearningRules" :key="rule.id">
                            <td class="text-center">
                                <v-checkbox-btn
                                    density="compact"
                                    color="primary"
                                    :disabled="bulkUpdatingImportLearningRules"
                                    :model-value="isImportLearningRuleSelected(rule.id)"
                                    @update:model-value="toggleImportLearningRuleSelection(rule.id, $event)"
                                />
                            </td>
                            <td>{{ getImportLearningMatchTypeName(rule.matchType) }}</td>
                            <td style="max-width: 260px">
                                <div class="text-truncate" :title="getImportLearningMatchValue(rule)">{{ getImportLearningMatchValue(rule) }}</div>
                            </td>
                            <td style="max-width: 320px">
                                <div class="text-truncate" :title="getImportLearningRuleSummary(rule)">
                                    {{ getImportLearningRuleSummary(rule) }}
                                </div>
                            </td>
                            <td>{{ rule.appliedCount }}</td>
                            <td>
                                <v-chip size="small" :color="rule.enabled ? 'success' : 'default'" variant="tonal">
                                    {{ rule.enabled ? tt('Enabled') : tt('Disabled') }}
                                </v-chip>
                            </td>
                            <td class="text-no-wrap">
                                <v-btn size="small" density="comfortable" variant="text" color="primary"
                                       :disabled="updatingImportLearningRuleId === rule.id"
                                       @click="toggleImportLearningRule(rule)">
                                    {{ rule.enabled ? tt('Disable') : tt('Enable') }}
                                </v-btn>
                                <v-btn size="small" density="comfortable" variant="text" color="error"
                                       :disabled="updatingImportLearningRuleId === rule.id"
                                       @click="deleteImportLearningRule(rule)">
                                    {{ tt('Delete') }}
                                </v-btn>
                            </td>
                        </tr>
                    </tbody>
                    </v-table>

                    <v-card-text class="pt-2 pb-4 d-flex align-center flex-wrap gap-2">
                        <span v-if="importLearningRulesTotalCount > 10">{{ tt('Transactions Per Page') }}</span>
                        <v-select class="ms-2"
                                  density="compact"
                                  max-width="100"
                                  item-title="name"
                                  item-value="value"
                                  :disabled="loadingImportLearningRules || bulkUpdatingImportLearningRules"
                                  :items="importLearningRulesPageOptions"
                                  v-model="importLearningRulesCountPerPage"
                                  v-if="importLearningRulesTotalCount > 10"
                        />
                        <pagination-buttons density="compact"
                                            :disabled="loadingImportLearningRules || bulkUpdatingImportLearningRules"
                                            :totalPageCount="importLearningRulesTotalPageCount"
                                            v-model="importLearningRulesCurrentPage"
                                            v-if="importLearningRulesTotalPageCount > 1" />
                    </v-card-text>
                </template>
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
import PaginationButtons from '@/components/desktop/PaginationButtons.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';

import { computed, onMounted, ref, useTemplateRef, watch } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { useDataManagementPageBase } from '@/views/base/users/DataManagementPageBase.ts';

import { useRootStore } from '@/stores/index.ts';
import { useUserStore } from '@/stores/user.ts';

import { isEquals } from '@/lib/common.ts';
import { isDataExportingEnabled } from '@/lib/server_settings.ts';
import { getCurrentToken } from '@/lib/userstate.ts';
import { startDownloadFile } from '@/lib/ui/common.ts';
import type { NameNumeralValue } from '@/core/base.ts';
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

interface ImportLearningRuleInfo {
    id: number;
    matchType: string;
    matchValue: string;
    matchFeatures?: Record<string, string>;
    learnedType: string;
    learnedCategoryName: string;
    learnedSourceAccountName: string;
    learnedDestinationAccountName: string;
    enabled: boolean;
    appliedCount: number;
}

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
const loadingImportLearningRules = ref<boolean>(true);
const exportingData = ref<boolean>(false);
const currentPasswordForClearData = ref<string>('');
const clearingData = ref<boolean>(false);
const updatingImportLearningRuleId = ref<number>(0);
const bulkUpdatingImportLearningRules = ref<boolean>(false);
const importLearningRules = ref<ImportLearningRuleInfo[]>([]);
const importLearningRulesTotalCount = ref<number>(0);
const importLearningRulesCurrentPage = ref<number>(1);
const importLearningRulesCountPerPage = ref<number>(20);
const selectedImportLearningRuleIds = ref<number[]>([]);
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

const selectedImportLearningRules = computed<ImportLearningRuleInfo[]>(() => {
    const selectedIdSet = new Set(selectedImportLearningRuleIds.value);
    return importLearningRules.value.filter(rule => selectedIdSet.has(rule.id));
});

const hasSelectedImportLearningRules = computed<boolean>(() => {
    return selectedImportLearningRuleIds.value.length > 0;
});

const importLearningRulesPageOptions = computed<NameNumeralValue[]>(() => {
    return getTablePageOptions(importLearningRulesTotalCount.value);
});

const importLearningRulesTotalPageCount = computed<number>(() => {
    const totalCount = importLearningRulesTotalCount.value;
    const pageSize = importLearningRulesCountPerPage.value;

    if (pageSize <= 0 || totalCount < 1) {
        return 1;
    }

    return Math.max(Math.ceil(totalCount / pageSize), 1);
});

const anyButNotAllImportLearningRulesSelected = computed<boolean>(() => {
    return hasSelectedImportLearningRules.value &&
        selectedImportLearningRuleIds.value.length < importLearningRules.value.length;
});

const allImportLearningRulesSelected = computed<boolean>({
    get(): boolean {
        return importLearningRules.value.length > 0 &&
            selectedImportLearningRuleIds.value.length === importLearningRules.value.length;
    },
    set(value: boolean): void {
        if (value) {
            selectedImportLearningRuleIds.value = importLearningRules.value.map(rule => rule.id);
        } else {
            selectedImportLearningRuleIds.value = [];
        }
    }
});

function getImportLearningHeaders(): Record<string, string> {
    const headers: Record<string, string> = {
        'Content-Type': 'application/json'
    };
    const token = getCurrentToken();
    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }
    return headers;
}

function getDisplayCount(count: number): string {
    return String(count);
}

function getTablePageOptions(linesCount?: number): NameNumeralValue[] {
    const pageOptions: NameNumeralValue[] = [];

    if (!linesCount || linesCount < 1) {
        pageOptions.push({ value: -1, name: tt('All') });
        return pageOptions;
    }

    for (const count of [10, 20, 50, 100]) {
        if (linesCount < count) {
            break;
        }

        pageOptions.push({ value: count, name: getDisplayCount(count) });
    }

    pageOptions.push({ value: -1, name: tt('All') });

    return pageOptions;
}

function getImportLearningMatchTypeName(matchType: string): string {
    if (matchType === 'composite') {
        return tt('Composite Rule');
    }
    if (matchType === 'counterparty') {
        return tt('Counterparty');
    }
    if (matchType === 'payment_method') {
        return tt('Payment Method');
    }
    if (matchType === 'description') {
        return tt('Description');
    }
    return matchType;
}

function getImportLearningMatchValue(rule: ImportLearningRuleInfo): string {
    if (rule.matchType !== 'composite' || !rule.matchFeatures) {
        return rule.matchValue;
    }

    const labelMap: Record<string, string> = {
        parser_id: tt('Parser'),
        counterparty: tt('Counterparty'),
        description: tt('Description'),
        payment_method: tt('Payment Method')
    };

    const orderedKeys = ['parser_id', 'counterparty', 'description', 'payment_method'];
    const parts = orderedKeys
        .filter(key => !!rule.matchFeatures?.[key])
        .map(key => `${labelMap[key] || key}: ${rule.matchFeatures?.[key] || ''}`);

    return parts.length > 0 ? parts.join(' | ') : rule.matchValue;
}

function getImportLearningRuleSummary(rule: ImportLearningRuleInfo): string {
    const parts: string[] = [];

    if (rule.learnedType) {
        parts.push(rule.learnedType);
    }
    if (rule.learnedCategoryName) {
        parts.push(rule.learnedCategoryName);
    }
    if (rule.learnedSourceAccountName || rule.learnedDestinationAccountName) {
        parts.push(`${rule.learnedSourceAccountName || '-'} → ${rule.learnedDestinationAccountName || '-'}`);
    }

    return parts.join(' | ');
}

function getErrorMessage(error: unknown): string {
    if (error instanceof Error) {
        return error.message;
    }

    return String(error || 'Unknown error');
}

function isImportLearningRuleSelected(ruleId: number): boolean {
    return selectedImportLearningRuleIds.value.includes(ruleId);
}

function toggleImportLearningRuleSelection(ruleId: number, checked: boolean | null): void {
    const nextChecked = Boolean(checked);

    if (nextChecked) {
        if (!selectedImportLearningRuleIds.value.includes(ruleId)) {
            selectedImportLearningRuleIds.value = [...selectedImportLearningRuleIds.value, ruleId];
        }
        return;
    }

    selectedImportLearningRuleIds.value = selectedImportLearningRuleIds.value.filter(id => id !== ruleId);
}

function syncSelectedImportLearningRules(rules: ImportLearningRuleInfo[]): void {
    const validRuleIds = new Set(rules.map(rule => rule.id));
    selectedImportLearningRuleIds.value = selectedImportLearningRuleIds.value.filter(id => validRuleIds.has(id));
}

function removeImportLearningRulesFromState(ruleIds: number[]): void {
    if (!ruleIds.length) {
        return;
    }

    const deletedRuleIds = new Set(ruleIds);
    importLearningRules.value = importLearningRules.value.filter(rule => !deletedRuleIds.has(rule.id));
    syncSelectedImportLearningRules(importLearningRules.value);
}

async function updateImportLearningRuleEnabledById(ruleId: number, enabled: boolean): Promise<void> {
    const response = await fetch(`/api/bills/import/learning-rules/${ruleId}`, {
        method: 'PUT',
        headers: getImportLearningHeaders(),
        body: JSON.stringify({ enabled })
    });

    if (!response.ok) {
        const errorText = await response.text();
        throw new Error(errorText || 'Failed to update import learning rule');
    }
}

async function deleteImportLearningRuleById(ruleId: number): Promise<void> {
    const response = await fetch(`/api/bills/import/learning-rules/${ruleId}`, {
        method: 'DELETE',
        headers: getImportLearningHeaders()
    });

    if (!response.ok) {
        const errorText = await response.text();
        throw new Error(errorText || 'Failed to delete import learning rule');
    }
}

async function reloadImportLearningRules(force: boolean): Promise<void> {
    loadingImportLearningRules.value = true;

    try {
        const page = importLearningRulesCurrentPage.value;
        const pageSize = importLearningRulesCountPerPage.value;
        const response = await fetch(`/api/bills/import/learning-rules?page=${page}&pageSize=${pageSize}`, {
            method: 'GET',
            headers: getImportLearningHeaders(),
            cache: 'no-store'
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(errorText || 'Failed to load import learning rules');
        }

        const data = await response.json();
        const newRules = (data.result || []) as ImportLearningRuleInfo[];
        const totalCount = Number(data.totalCount || 0);
        const totalPages = Number(data.totalPages || 1);
        const effectivePage = Number(data.page || 1);

        if (page > totalPages && totalPages > 0) {
            importLearningRulesCurrentPage.value = totalPages;
            loadingImportLearningRules.value = false;
            return;
        }

        if (force) {
            if (isEquals(importLearningRules.value, newRules)) {
                snackbar.value?.showMessage('Data is up to date');
            } else {
                snackbar.value?.showMessage('Data has been updated');
            }
        }

        importLearningRules.value = newRules;
        importLearningRulesTotalCount.value = totalCount;
        importLearningRulesCurrentPage.value = effectivePage;
        syncSelectedImportLearningRules(newRules);
        loadingImportLearningRules.value = false;
    } catch (error) {
        loadingImportLearningRules.value = false;
        snackbar.value?.showError(getErrorMessage(error));
    }
}

async function toggleImportLearningRule(rule: ImportLearningRuleInfo): Promise<void> {
    updatingImportLearningRuleId.value = rule.id;

    try {
        await updateImportLearningRuleEnabledById(rule.id, !rule.enabled);
        rule.enabled = !rule.enabled;
        snackbar.value?.showMessage(rule.enabled ? 'Rule enabled' : 'Rule disabled');
    } catch (error) {
        snackbar.value?.showError(getErrorMessage(error));
    } finally {
        updatingImportLearningRuleId.value = 0;
    }
}

function deleteImportLearningRule(rule: ImportLearningRuleInfo): void {
    confirmDialog.value?.open('Are you sure you want to delete this import learning rule?', {
        color: 'warning'
    }).then(async confirmed => {
        const isConfirmed = Boolean(confirmed);

        if (!isConfirmed) {
            return;
        }

        updatingImportLearningRuleId.value = rule.id;

        try {
            await deleteImportLearningRuleById(rule.id);
            removeImportLearningRulesFromState([rule.id]);
            await reloadImportLearningRules(false);
            snackbar.value?.showMessage('Import learning rule deleted');
        } catch (error) {
            snackbar.value?.showError(getErrorMessage(error));
        } finally {
            updatingImportLearningRuleId.value = 0;
        }
    });
}

async function setSelectedImportLearningRulesEnabled(enabled: boolean): Promise<void> {
    const selectedRules = [...selectedImportLearningRules.value];

    if (!selectedRules.length || bulkUpdatingImportLearningRules.value) {
        return;
    }

    bulkUpdatingImportLearningRules.value = true;

    try {
        const results = await Promise.allSettled(
            selectedRules.map(rule => updateImportLearningRuleEnabledById(rule.id, enabled))
        );

        let successCount = 0;
        const failedMessages: string[] = [];

        results.forEach((result, index) => {
            if (result.status === 'fulfilled') {
                selectedRules[index]!.enabled = enabled;
                successCount += 1;
            } else {
                failedMessages.push(getErrorMessage(result.reason));
            }
        });

        if (successCount > 0) {
            snackbar.value?.showMessage(
                enabled ? 'Enabled selected import learning rules: {count}' : 'Disabled selected import learning rules: {count}',
                { count: successCount }
            );
        }

        if (failedMessages.length > 0 && failedMessages[0]) {
            snackbar.value?.showError(failedMessages[0]);
        }
    } finally {
        bulkUpdatingImportLearningRules.value = false;
    }
}

function deleteSelectedImportLearningRules(): void {
    const selectedRules = [...selectedImportLearningRules.value];

    if (!selectedRules.length || bulkUpdatingImportLearningRules.value) {
        return;
    }

    confirmDialog.value?.open('Are you sure you want to delete selected import learning rules?', {
        color: 'warning'
    }).then(async confirmed => {
        if (!confirmed) {
            return;
        }

        bulkUpdatingImportLearningRules.value = true;

        try {
            const deletedIds = new Set<number>();
            const failedMessages: string[] = [];

            for (const rule of selectedRules) {
                try {
                    await deleteImportLearningRuleById(rule.id);
                    deletedIds.add(rule.id);
                } catch (error) {
                    failedMessages.push(getErrorMessage(error));
                }
            }

            if (deletedIds.size > 0) {
                removeImportLearningRulesFromState(Array.from(deletedIds));
                await reloadImportLearningRules(false);
                snackbar.value?.showMessage('Deleted selected import learning rules: {count}', { count: deletedIds.size });
            }

            if (failedMessages.length > 0 && failedMessages[0]) {
                snackbar.value?.showError(failedMessages[0]);
            }
        } finally {
            bulkUpdatingImportLearningRules.value = false;
        }
    });
}

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
    reloadImportLearningRules(false);
    reloadRecognitionSettings(false);
});

watch(importLearningRulesCurrentPage, (newPage, oldPage) => {
    if (newPage !== oldPage) {
        reloadImportLearningRules(false);
    }
});

watch(importLearningRulesCountPerPage, (newPageSize, oldPageSize) => {
    if (newPageSize === oldPageSize) {
        return;
    }

    if (importLearningRulesCurrentPage.value !== 1) {
        importLearningRulesCurrentPage.value = 1;
        return;
    }

    reloadImportLearningRules(false);
});

defineExpose({
    reloadUserDataStatistics,
    reloadImportLearningRules,
    reloadRecognitionSettings
});
</script>
