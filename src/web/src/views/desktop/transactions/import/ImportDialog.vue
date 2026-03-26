<template>
    <v-dialog :persistent="!!persistent" v-model="showState">
        <v-card class="pa-6 pa-sm-10 pa-md-12">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center">
                        <h4 class="text-h4">{{ tt('Import Transactions') }}</h4>
                        <v-progress-circular indeterminate size="22" class="ms-2" v-if="loading"></v-progress-circular>
                    </div>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" :disabled="loading || submitting"
                           v-if="currentStep === 'defineColumn' && importTransactionDefineColumnTab?.menus">
                        <v-icon :icon="mdiDotsVertical" />
                        <v-menu activator="parent" max-height="500">
                            <v-list>
                                <v-list-item :key="index"
                                             :prepend-icon="menu.prependIcon"
                                             :title="menu.title"
                                             :disabled="menu.disabled"
                                             @click="menu.onClick()"
                                             v-for="(menu, index) in importTransactionDefineColumnTab.menus"/>
                            </v-list>
                        </v-menu>
                    </v-btn>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" :disabled="loading || submitting || !parsedFileData"
                           v-if="currentStep === 'defineColumn'"
                              @click="openManageImportConfigDialog">
                           <v-icon :icon="mdiFolderOpenOutline" />
                          </v-btn>
                          <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                              :icon="true" :disabled="loading || submitting || !parsedFileData"
                              v-if="currentStep === 'defineColumn'"
                                    @click="openSaveImportConfigDialog">
                        <v-icon :icon="mdiContentSaveOutline" />
                    </v-btn>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" :disabled="loading || submitting"
                           v-if="currentStep === 'executeCustomScript' && importTransactionExecuteCustomScriptTab?.menus">
                        <v-icon :icon="mdiDotsVertical" />
                        <v-menu activator="parent" max-height="500">
                            <v-list>
                                <v-list-item :key="index"
                                             :prepend-icon="menu.prependIcon"
                                             :title="menu.title"
                                             :disabled="menu.disabled"
                                             @click="menu.onClick()"
                                             v-for="(menu, index) in importTransactionExecuteCustomScriptTab.menus"/>
                            </v-list>
                        </v-menu>
                    </v-btn>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" :disabled="loading || submitting"
                           v-if="currentStep === 'checkData' && importTransactionCheckDataTab?.filterMenus">
                        <v-icon :icon="mdiFilterOutline" />
                        <v-menu activator="parent" max-height="500">
                            <v-list>
                                <template :key="groupIndex" v-for="(group, groupIndex) in importTransactionCheckDataTab.filterMenus">
                                    <v-list-subheader :title="group.title" />
                                    <v-divider class="my-2" v-if="groupIndex > 0" />
                                    <v-list-item :key="`menu_${groupIndex}_${index}`"
                                                 :prepend-icon="menu.prependIcon"
                                                 :title="menu.title"
                                                 :subtitle="menu.subTitle"
                                                 :append-icon="menu.appendIcon"
                                                 :disabled="menu.disabled"
                                                 @click="menu.onClick()"
                                                 v-for="(menu, index) in group.items" />
                                </template>
                            </v-list>
                        </v-menu>
                    </v-btn>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" :disabled="loading || submitting"
                           v-if="currentStep === 'checkData' && importTransactionCheckDataTab?.toolMenus">
                        <v-icon :icon="mdiDotsVertical" />
                        <v-menu activator="parent" max-height="500">
                            <v-list>
                                <template :key="index" v-for="(menu, index) in importTransactionCheckDataTab.toolMenus">
                                    <v-divider class="my-2" v-if="menu.divider" />
                                    <v-list-item :prepend-icon="menu.prependIcon"
                                                 :title="menu.title"
                                                 :subtitle="menu.subTitle"
                                                 :append-icon="menu.appendIcon"
                                                 :disabled="menu.disabled"
                                                 @click="menu.onClick()" />
                                </template>
                            </v-list>
                        </v-menu>
                    </v-btn>
                </div>
            </template>

            <div class="mt-4 cursor-default">
                <steps-bar min-width="700" :clickable="false" :steps="allSteps" :current-step="currentStep" />
            </div>

            <v-window class="disable-tab-transition" v-model="currentStep">
                <v-window-item value="uploadFile">
                    <v-row>
                        <!-- v6.79: 移除文件类型选择器，因为解析器会自动识别文件格式 -->

                        <v-col cols="12" md="12" v-if="allFileSubTypes">
                            <v-select
                                item-title="displayName"
                                item-value="type"
                                :disabled="submitting"
                                :label="tt('Format')"
                                :placeholder="tt('Format')"
                                :items="allFileSubTypes"
                                v-model="fileSubType"
                            />
                        </v-col>

                        <v-col cols="12" md="12" v-if="importFiles.length <= 1 && (fileType === 'dsv' || fileType === 'dsv_data')">
                            <v-select
                                item-title="displayName"
                                item-value="type"
                                :disabled="submitting"
                                :label="tt('Handling Method')"
                                :placeholder="tt('Handling Method')"
                                :items="[
                                    { displayName: tt('Auto detect'), type: ImportDSVProcessMethod.AutoDetect },
                                    { displayName: tt('Column Mapping'), type: ImportDSVProcessMethod.ColumnMapping },
                                 ]"
                                v-model="processDSVMethod"
                            />
                        </v-col>

                        <v-col cols="12" md="12" v-if="!isImportDataFromTextbox">
                            <v-text-field
                                readonly
                                persistent-placeholder
                                type="text"
                                class="always-cursor-pointer"
                                :disabled="submitting"
                                :label="tt('Data File')"
                                :placeholder="tt('format.misc.clickToSelectedFile', { extensions: supportedImportFileExtensions })"
                                v-model="fileName"
                                @click="showOpenFileDialog"
                            />
                        </v-col>

                        <v-col cols="12" md="12" v-if="isImportDataFromTextbox">
                            <v-textarea
                                type="text"
                                persistent-placeholder
                                rows="5"
                                :disabled="submitting"
                                :placeholder="tt('Data to import')"
                                v-model="importData"
                            />
                        </v-col>

                        <v-col cols="12" md="12" class="mb-0 pb-0" v-if="exportFileGuideDocumentUrl">
                            <a :href="exportFileGuideDocumentUrl" :class="{ 'disabled': submitting }" target="_blank">
                                <v-icon :icon="mdiHelpCircleOutline" size="16" />
                                <span class="ms-1" v-if="fileType === 'dsv' || fileType === 'dsv_data'">{{ tt('How to import this file?') }}</span>
                                <span class="ms-1" v-if="fileType !== 'dsv' && fileType !== 'dsv_data'">{{ tt('How to export this file?') }}</span>
                                <span class="ms-1" v-if="exportFileGuideDocumentLanguageName">[{{ exportFileGuideDocumentLanguageName }}]</span>
                            </a>
                        </v-col>
                    </v-row>
                </v-window-item>
                <v-window-item value="defineColumn">
                    <import-transaction-define-column-tab
                        ref="importTransactionDefineColumnTab"
                        :parsed-file-data="parsedFileData"
                        :disabled="loading || submitting"
                    />
                </v-window-item>
                <v-window-item value="executeCustomScript">
                    <import-transaction-execute-custom-script-tab
                        ref="importTransactionExecuteCustomScriptTab"
                        :parsed-file-data="parsedFileData"
                        :disabled="loading || submitting"
                    />
                </v-window-item>
                <v-window-item value="checkData">
                    <import-transaction-check-data-tab
                        ref="importTransactionCheckDataTab"
                        :import-transactions="importTransactions"
                        :disabled="loading || submitting"
                        :session-id="serverSessionId"
                        @reclassified="onReclassified"
                    />
                </v-window-item>
                <v-window-item value="finalResult">
                    <h4 class="text-h4 mb-1">{{ tt('Data Import Completed') }}</h4>
                    <p class="my-5">{{ tt('format.misc.importTransactionResult', { count: getDisplayCount(importedCount || 0) }) }}</p>
                </v-window-item>
            </v-window>

            <div class="d-flex justify-sm-space-between gap-4 flex-wrap justify-center mt-10">
                <v-btn color="secondary" variant="tonal" :disabled="loading || submitting"
                       :prepend-icon="mdiClose" @click="close(false)"
                       v-if="currentStep !== 'finalResult'">{{ tt('Cancel') }}</v-btn>
                <v-btn class="button-icon-with-direction" color="primary"
                       :disabled="loading || submitting || (!isImportDataFromTextbox && !importFile) || (isImportDataFromTextbox && !importData)"
                       :append-icon="!submitting ? mdiArrowRight : undefined" @click="parseData"
                       v-if="currentStep === 'defineColumn' || currentStep === 'executeCustomScript' || currentStep === 'uploadFile'">
                    {{ tt('Next') }}
                    <v-progress-circular indeterminate size="22" class="ms-2" v-if="submitting"></v-progress-circular>
                </v-btn>
                <v-btn class="button-icon-with-direction" color="teal"
                       :disabled="submitting || importTransactionCheckDataTab?.isEditing || !importTransactionCheckDataTab?.canImport"
                       :append-icon="!submitting ? mdiArrowRight : undefined" @click="submit"
                       v-if="currentStep === 'checkData'">
                    {{ (submitting && importProcess > 0 ? tt('format.misc.importingTransactions', { process: formatNumberToLocalizedNumerals(importProcess, 2) }) : tt('Import')) }}
                    <v-progress-circular indeterminate size="22" class="ms-2" v-if="submitting"></v-progress-circular>
                </v-btn>
                <v-btn color="secondary" variant="tonal"
                       :append-icon="mdiCheck"
                       @click="close(true)"
                       v-if="currentStep === 'finalResult'">{{ tt('Close') }}</v-btn>
            </div>
        </v-card>
    </v-dialog>

    <confirm-dialog ref="confirmDialog"/>
    <snack-bar ref="snackbar" />
    <v-dialog v-model="showSaveImportConfigDialog" max-width="520">
        <v-card class="pa-4">
            <v-card-title>{{ tt('Save Data Mapping File') }}</v-card-title>
            <v-card-text>
                <v-text-field
                    v-model="saveImportConfigName"
                    :label="tt('Name')"
                    :placeholder="tt('Name')"
                    :disabled="submitting"
                    persistent-placeholder
                />
                <v-textarea
                    v-model="saveImportConfigDescription"
                    :label="tt('Description')"
                    :placeholder="tt('Description')"
                    :disabled="submitting"
                    rows="2"
                    auto-grow
                    persistent-placeholder
                />
                <v-checkbox
                    v-model="saveImportConfigIsDefault"
                    :label="tt('Default')"
                    :disabled="submitting"
                    hide-details
                />
                <div class="text-caption text-medium-emphasis mt-2" v-if="saveImportConfigRecommended">
                    {{ tt('No default template yet, recommend setting this template as default') }}
                </div>
            </v-card-text>
            <v-card-actions class="justify-end gap-2">
                <v-btn variant="text" :disabled="submitting" @click="showSaveImportConfigDialog = false">
                    {{ tt('Cancel') }}
                </v-btn>
                <v-btn color="primary" :disabled="submitting || !saveImportConfigName.trim()" @click="saveCurrentImportConfig">
                    {{ tt('Save') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
    <v-dialog v-model="showManageImportConfigDialog" max-width="760">
        <v-card class="pa-4">
            <v-card-title>{{ tt('Saved Templates') }}</v-card-title>
            <v-card-text>
                <v-list v-if="importConfigList.length > 0">
                    <v-list-item :key="config.id" v-for="config in importConfigList">
                        <template #prepend>
                            <v-icon :icon="matchedImportConfig?.id === config.id ? mdiCheck : mdiFolderOpenOutline" />
                        </template>
                        <v-list-item-title class="d-flex align-center ga-2">
                            <span>{{ config.name }}</span>
                            <v-chip
                                v-if="config.isDefault"
                                size="x-small"
                                color="primary"
                                variant="tonal"
                            >
                                {{ tt('Default') }}
                            </v-chip>
                            <v-chip
                                v-else-if="config.defaultRecommendation"
                                size="x-small"
                                color="secondary"
                                variant="tonal"
                            >
                                {{ tt('Recommended Default') }}
                            </v-chip>
                        </v-list-item-title>
                        <v-list-item-subtitle>
                            {{ getImportConfigDisplayDescription(config) || tt('No data to import') }}
                        </v-list-item-subtitle>
                        <template #append>
                            <div class="d-flex ga-2">
                                <v-btn size="small" variant="text" color="primary" @click="applyImportConfig(config)">
                                    {{ tt('Apply') }}
                                </v-btn>
                                <v-btn size="small" variant="text" color="secondary" @click="openEditImportConfigDialog(config)">
                                    {{ tt('Edit') }}
                                </v-btn>
                                <v-btn size="small" variant="text" color="error" @click="removeImportConfig(config)">
                                    {{ tt('Delete') }}
                                </v-btn>
                            </div>
                        </template>
                    </v-list-item>
                </v-list>
                <div class="text-medium-emphasis" v-else>{{ tt('No saved templates') }}</div>
            </v-card-text>
            <v-card-actions class="justify-end">
                <v-btn variant="text" @click="showManageImportConfigDialog = false">
                    {{ tt('Close') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
    <v-dialog v-model="showEditImportConfigDialog" max-width="520">
        <v-card class="pa-4">
            <v-card-title>{{ tt('Edit') }}</v-card-title>
            <v-card-text>
                <v-text-field
                    v-model="editImportConfigName"
                    :label="tt('Template Name')"
                    :placeholder="tt('Template Name')"
                    :disabled="submitting"
                    persistent-placeholder
                />
                <v-textarea
                    v-model="editImportConfigDescription"
                    :label="tt('Description')"
                    :placeholder="tt('Description')"
                    :disabled="submitting"
                    rows="2"
                    auto-grow
                    persistent-placeholder
                />
                <v-checkbox
                    v-model="editImportConfigIsDefault"
                    :label="tt('Default')"
                    :disabled="submitting"
                    hide-details
                />
                <div class="text-caption text-medium-emphasis mt-2" v-if="editImportConfigRecommended">
                    {{ tt('No default template yet, recommend setting this template as default') }}
                </div>
            </v-card-text>
            <v-card-actions class="justify-end gap-2">
                <v-btn variant="text" :disabled="submitting" @click="showEditImportConfigDialog = false">
                    {{ tt('Cancel') }}
                </v-btn>
                <v-btn color="primary" :disabled="submitting || !editImportConfigName.trim()" @click="saveEditedImportConfig">
                    {{ tt('Save') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
    <input ref="fileInput" type="file" multiple style="display: none" :accept="supportedImportFileExtensions" @change="setImportFile($event)" />
</template>

<script setup lang="ts">
import type { StepBarItem } from '@/components/desktop/StepsBar.vue';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import ImportTransactionDefineColumnTab from './tabs/ImportTransactionDefineColumnTab.vue';
import ImportTransactionExecuteCustomScriptTab from './tabs/ImportTransactionExecuteCustomScriptTab.vue';
import ImportTransactionCheckDataTab from './tabs/ImportTransactionCheckDataTab.vue';

import { ref, computed, nextTick, useTemplateRef } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { getTimezoneOffsetMinutes } from '@/lib/datetime.ts';

import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';
import { useOverviewStore } from '@/stores/overview.ts';
import { useStatisticsStore } from '@/stores/statistics.ts';
import { useSettingsStore } from '@/stores/setting.ts';

import { type NumeralSystem } from '@/core/numeral.ts';

import type { LocalizedImportFileTypeSubType } from '@/core/file.ts';
import { ImportTransaction, type ImportTransactionResponse } from '@/models/imported_transaction.ts';

import { getCurrentToken } from '@/lib/userstate.ts';
import services from '@/lib/services.ts';
import logger from '@/lib/logger.ts';

import {
    mdiFilterOutline,
    mdiCheck,
    mdiContentSaveOutline,
    mdiDotsVertical,
    mdiFolderOpenOutline,
    mdiHelpCircleOutline,
    mdiClose,
    mdiArrowRight
} from '@mdi/js';

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;
type ImportTransactionDefineColumnTabType = InstanceType<typeof ImportTransactionDefineColumnTab>;
type ImportTransactionExecuteCustomScriptTabType = InstanceType<typeof ImportTransactionExecuteCustomScriptTab>;
type ImportTransactionCheckDataTabType = InstanceType<typeof ImportTransactionCheckDataTab>;

type ImportTransactionDialogStep = 'uploadFile' | 'defineColumn' | 'executeCustomScript' | 'checkData' | 'finalResult';
enum ImportDSVProcessMethod {
    AutoDetect,
    ColumnMapping,
}

interface ImportConfigMatchResult {
    id: number;
    name: string;
    fileFormat?: string;
    description?: string;
    descriptionSummary?: string;
    fieldMappings: Record<string, unknown>;
    sampleHeaders?: string[];
    dateFormat?: string;
    delimiter?: string;
    encoding?: string;
    skipRows?: number;
    hasHeader?: boolean;
    customRules?: Record<string, unknown>;
    isDefault?: boolean;
    defaultRecommendation?: boolean;
    matchScore?: number;
    matchReason?: string;
}

interface ImportFilePreviewResult {
    headers: string[];
    sampleData: string[][];
    previewRows?: string[][];
    totalRows: number;
    encoding?: string;
    delimiter?: string;
}

interface ImportConfigSuggestionResult {
    includeHeader?: boolean;
    columnMapping?: Record<string, number>;
    transactionTypeMapping?: Record<string, number>;
    suggestions?: Array<{
        columnType: number;
        columnIndex: number;
        header: string;
        score: number;
    }>;
}

defineProps<{
    persistent?: boolean;
}>();

const {
    tt,
    getCurrentNumeralSystemType,
    formatNumberToLocalizedNumerals
} = useI18n();

const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const transactionTagsStore = useTransactionTagsStore();
const transactionsStore = useTransactionsStore();
const overviewStore = useOverviewStore();
const statisticsStore = useStatisticsStore();
const settingsStore = useSettingsStore();

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const importTransactionDefineColumnTab = useTemplateRef<ImportTransactionDefineColumnTabType>('importTransactionDefineColumnTab');
const importTransactionExecuteCustomScriptTab = useTemplateRef<ImportTransactionExecuteCustomScriptTabType>('importTransactionExecuteCustomScriptTab');
const importTransactionCheckDataTab = useTemplateRef<ImportTransactionCheckDataTabType>('importTransactionCheckDataTab');
const fileInput = useTemplateRef<HTMLInputElement>('fileInput');

const showState = ref<boolean>(false);
const serverSessionId = ref<string>('');  // v6.48: 后端三阶段导入的会话ID
const currentStep = ref<ImportTransactionDialogStep>('uploadFile');
const importProcess = ref<number>(0);
const selectedFileTypes = ref<string[]>(['auto']);
const importFiles = ref<File[]>([]);
const importData = ref<string>('');
const parsedFileData = ref<string[][] | undefined>(undefined);
const importTransactions = ref<ImportTransaction[] | undefined>(undefined);
const parsedFileDelimiter = ref<string>('');
const matchedImportConfig = ref<ImportConfigMatchResult | null>(null);

const fileSubType = ref<string>('');
const processDSVMethod = ref<ImportDSVProcessMethod>(ImportDSVProcessMethod.AutoDetect);
const showSaveImportConfigDialog = ref<boolean>(false);
const saveImportConfigName = ref<string>('');
const saveImportConfigDescription = ref<string>('');
const saveImportConfigIsDefault = ref<boolean>(false);
const saveImportConfigRecommended = ref<boolean>(false);
const showManageImportConfigDialog = ref<boolean>(false);
const importConfigList = ref<ImportConfigMatchResult[]>([]);
const showEditImportConfigDialog = ref<boolean>(false);
const editImportConfigName = ref<string>('');
const editImportConfigDescription = ref<string>('');
const editImportConfigIsDefault = ref<boolean>(false);
const editImportConfigRecommended = ref<boolean>(false);
const editingImportConfig = ref<ImportConfigMatchResult | null>(null);

const allSteps = computed<StepBarItem[]>(() => [
    { name: 'uploadFile', title: tt('Select File'), subTitle: tt('Select the file to import') },
    { name: 'defineColumn', title: tt('Define Columns'), subTitle: tt('Map columns to fields') },
    { name: 'executeCustomScript', title: tt('Custom Script'), subTitle: tt('Execute Custom Script') },
    { name: 'checkData', title: tt('Check Data'), subTitle: tt('Verify and edit data') },
    { name: 'finalResult', title: tt('Import Result'), subTitle: tt('View import result') }
]);

const fileType = computed<string>(() => {
    const type = selectedFileTypes.value[0];
    if (selectedFileTypes.value.length > 0 && type && type !== 'auto') {
        return type;
    }

    const currentFile = importFile.value;
    const lowerName = currentFile?.name.toLowerCase() || '';
    if (lowerName.endsWith('.csv') || lowerName.endsWith('.txt')) {
        return 'dsv';
    }

    return 'auto';
});

const allFileSubTypes = computed<LocalizedImportFileTypeSubType[] | undefined>(() => undefined);

const isImportDataFromTextbox = computed<boolean>(() => false);

const supportedImportFileExtensions = computed<string>(() => '.csv,.xls,.xlsx,.txt');

const exportFileGuideDocumentUrl = computed<string | undefined>(() => undefined);
const exportFileGuideDocumentLanguageName = computed<string | undefined>(() => undefined);

const importFile = computed<File | undefined>(() => {
    return importFiles.value.length > 0 ? importFiles.value[0] : undefined;
});

const importedCount = ref<number | null>(null);
const loading = ref<boolean>(true);
const submitting = ref<boolean>(false);

const shouldUseColumnMapping = computed<boolean>(() => {
    return importFiles.value.length === 1 &&
        isGenericImportFile() &&
        (processDSVMethod.value === ImportDSVProcessMethod.ColumnMapping ||
            processDSVMethod.value === ImportDSVProcessMethod.AutoDetect);
});

let resolveFunc: (() => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const numeralSystem = computed<NumeralSystem>(() => getCurrentNumeralSystemType());

// Simplified file type options
// const fileTypeOptions = [
//     { title: 'Auto', value: 'auto' },
//     { title: 'Alipay', value: 'alipay' },
//     { title: 'WeChat', value: 'wechat' },
//     { title: 'ICBC', value: 'icbc' },
//     { title: 'ABC', value: 'abc' },
//     { title: 'CMBC', value: 'cmbc' },
//     { title: 'CCB', value: 'ccb' },
//     { title: 'ABC Credit', value: 'abc_credit' },
//     { title: 'PSBC Credit', value: 'psbc_credit' },
//     { title: 'SPDB Credit', value: 'spdb_credit' }
// ];

const fileName = computed<string>(() => {
    if (importFiles.value.length === 0) return '';
    const file = importFiles.value[0];
    if (importFiles.value.length === 1 && file) return file.name;
    return `${importFiles.value.length} files selected`;
});

function getDisplayCount(count: number): string {
    return numeralSystem.value.formatNumber(count);
}

function open(): Promise<void> {
    // v6.52: 清理之前可能残留的导入会话数据
    // 确保每次打开导入对话框时 bills_parser_template 和 bills_preview 表都是干净的
    if (serverSessionId.value) {
        cleanupServerSession();
    }

    selectedFileTypes.value = ['auto'];
    currentStep.value = 'uploadFile';
    importProcess.value = 0;
    importFiles.value = [];
    importData.value = '';
    parsedFileData.value = undefined;
    parsedFileDelimiter.value = '';
    matchedImportConfig.value = null;
    showSaveImportConfigDialog.value = false;
    saveImportConfigName.value = '';
    saveImportConfigDescription.value = '';
    saveImportConfigIsDefault.value = false;
    saveImportConfigRecommended.value = false;
    showManageImportConfigDialog.value = false;
    showEditImportConfigDialog.value = false;
    editImportConfigName.value = '';
    editImportConfigDescription.value = '';
    editImportConfigIsDefault.value = false;
    editImportConfigRecommended.value = false;
    editingImportConfig.value = null;
    importConfigList.value = [];
    importTransactionDefineColumnTab.value?.reset();
    importTransactionExecuteCustomScriptTab.value?.reset();
    importTransactions.value = undefined;
    importTransactionCheckDataTab.value?.reset();
    showState.value = true;
    const promises = [
        accountsStore.loadAllAccounts({ force: false }),
        transactionCategoriesStore.loadAllCategories({ force: false }),
        transactionTagsStore.loadAllTags({ force: false })
    ];

    Promise.all(promises).then(() => {
        loading.value = false;
    }).catch(error => {
        logger.error('failed to load essential data for importing transaction', error);

        loading.value = false;
        showState.value = false;

        if (!error.processed) {
            if (rejectFunc) {
                rejectFunc(error);
            }
        }
    });

    return new Promise((resolve, reject) => {
        resolveFunc = resolve;
        rejectFunc = reject;
    });
}

function showOpenFileDialog(): void {
    if (submitting.value) {
        return;
    }

    fileInput.value?.click();
}

function setImportFile(event: Event): void {
    if (!event || !event.target) {
        return;
    }

    const el = event.target as HTMLInputElement;

    if (!el.files || !el.files.length) {
        return;
    }

    importFiles.value = Array.from(el.files);
    processDSVMethod.value = ImportDSVProcessMethod.AutoDetect;
    matchedImportConfig.value = null;
    parsedFileData.value = undefined;
    el.value = '';
}

function getImportConfigFileFormat(): string {
    const lowerName = importFile.value?.name.toLowerCase() || '';
    if (lowerName.endsWith('.csv') || lowerName.endsWith('.txt')) {
        return 'csv';
    }
    if (lowerName.endsWith('.xlsx') || lowerName.endsWith('.xls')) {
        return 'excel';
    }
    return 'csv';
}

function isGenericImportFile(): boolean {
    const lowerName = importFile.value?.name.toLowerCase() || '';
    return lowerName.endsWith('.csv') || lowerName.endsWith('.txt') ||
        lowerName.endsWith('.xlsx') || lowerName.endsWith('.xls');
}

function getMatchedImportConfigMessage(config: ImportConfigMatchResult): string {
    if (config.matchReason === 'default_template_fallback') {
        return `已自动回退到默认模板：${config.name}`;
    }

    return `已自动套用模板：${config.name}`;
}

function getImportConfigDisplayDescription(config: Partial<ImportConfigMatchResult> | null | undefined): string {
    if (!config) {
        return '';
    }

    return config.description || config.descriptionSummary || config.sampleHeaders?.join(' / ') || '';
}

async function prepareColumnMappingStep(): Promise<void> {
    if (!importFile.value) {
        snackbar.value?.showError('Please select at least one file');
        return;
    }

    if (importFiles.value.length !== 1) {
        snackbar.value?.showError('Column mapping currently supports one file at a time');
        return;
    }

    const previewResponse = await services.previewImportFile({
        importFile: importFile.value,
        delimiter: parsedFileDelimiter.value || undefined,
        fileEncoding: matchedImportConfig.value?.encoding || undefined
    });
    const preview = previewResponse.data?.result as ImportFilePreviewResult | undefined;
    const rows = preview?.sampleData || [];

    if (!rows.length) {
        snackbar.value?.showError('No data to import');
        return;
    }

    parsedFileDelimiter.value = preview?.delimiter || parsedFileDelimiter.value;
    parsedFileData.value = rows.slice(0, 300);
    currentStep.value = 'defineColumn';

    await nextTick();
    importTransactionDefineColumnTab.value?.reset();

    const headers = rows[0] || [];
    if (!headers.length) {
        return;
    }

    try {
        const response = await services.matchImportConfig({
            fileFormat: getImportConfigFileFormat(),
            headers
        });
        const result = response.data?.result;

        if (response.data?.success && result?.fieldMappings) {
            matchedImportConfig.value = result;
            parsedFileDelimiter.value = result.delimiter || parsedFileDelimiter.value;
            importTransactionDefineColumnTab.value?.applyFieldMappings(result.fieldMappings);
            if (typeof result.hasHeader === 'boolean' && result.hasHeader !== undefined) {
                importTransactionDefineColumnTab.value?.applyFieldMappings({
                    ...result.fieldMappings,
                    includeHeader: result.hasHeader
                });
            }
            snackbar.value?.showMessage(getMatchedImportConfigMessage(result));
            return;
        }
    } catch (error) {
        logger.warn('failed to match import config', error);
    }

    try {
        const suggestionResponse = await services.suggestImportConfig({
            fileFormat: getImportConfigFileFormat(),
            headers,
            sampleRows: rows.slice(1, 21)
        });
        const suggestion = suggestionResponse.data?.result as ImportConfigSuggestionResult | undefined;
        if (suggestion?.columnMapping && Object.keys(suggestion.columnMapping).length > 0) {
            importTransactionDefineColumnTab.value?.applyFieldMappings(suggestion as any);
            snackbar.value?.showMessage('已自动建议列映射');
        }
    } catch (error) {
        logger.warn('failed to suggest import config', error);
    }
}

async function loadImportConfigList(): Promise<void> {
    const response = await services.getImportConfigs({
        fileFormat: getImportConfigFileFormat()
    });
    const result = response.data?.result || [];
    importConfigList.value = result.map((config: any) => ({
        id: config.id,
        name: config.name,
        fileFormat: config.fileFormat,
        description: config.description,
        descriptionSummary: config.descriptionSummary,
        fieldMappings: config.fieldMappings,
        sampleHeaders: config.sampleHeaders || [],
        dateFormat: config.dateFormat,
        delimiter: config.delimiter,
        encoding: config.encoding,
        skipRows: config.skipRows,
        hasHeader: config.hasHeader,
        customRules: config.customRules,
        isDefault: config.isDefault,
        defaultRecommendation: config.defaultRecommendation,
        matchScore: config.matchScore
    }));
}

async function openManageImportConfigDialog(): Promise<void> {
    try {
        await loadImportConfigList();
        showManageImportConfigDialog.value = true;
    } catch (error) {
        logger.error('failed to load import config list', error);
        snackbar.value?.showError('Unable to load saved templates');
    }
}

function applyImportConfig(config: ImportConfigMatchResult): void {
    importTransactionDefineColumnTab.value?.applyFieldMappings(config.fieldMappings as any);
    matchedImportConfig.value = config;
    parsedFileDelimiter.value = config.delimiter || parsedFileDelimiter.value;
    showManageImportConfigDialog.value = false;
    snackbar.value?.showMessage(`已套用模板：${config.name}`);
}

function openEditImportConfigDialog(config: ImportConfigMatchResult): void {
    editingImportConfig.value = config;
    editImportConfigName.value = config.name || '';
    editImportConfigDescription.value = getImportConfigDisplayDescription(config);
    editImportConfigRecommended.value = !config.isDefault && !!config.defaultRecommendation;
    editImportConfigIsDefault.value = !!config.isDefault || editImportConfigRecommended.value;
    showEditImportConfigDialog.value = true;
}

async function saveEditedImportConfig(): Promise<void> {
    if (!editingImportConfig.value || !editImportConfigName.value.trim()) {
        return;
    }

    try {
        const targetConfig = editingImportConfig.value;
        const response = await services.saveImportConfig({
            id: targetConfig.id,
            name: editImportConfigName.value.trim(),
            fileFormat: targetConfig.fileFormat || getImportConfigFileFormat(),
            description: editImportConfigDescription.value.trim(),
            fieldMappings: targetConfig.fieldMappings,
            dateFormat: targetConfig.dateFormat || '',
            encoding: targetConfig.encoding || 'utf-8',
            delimiter: targetConfig.delimiter,
            skipRows: targetConfig.skipRows || 0,
            hasHeader: targetConfig.hasHeader ?? true,
            customRules: targetConfig.customRules || {},
            sampleHeaders: targetConfig.sampleHeaders || [],
            isDefault: editImportConfigIsDefault.value
        });

        if (!response.data?.success) {
            throw new Error('edit failed');
        }

        if (matchedImportConfig.value?.id === targetConfig.id) {
            matchedImportConfig.value = {
                ...matchedImportConfig.value,
                name: editImportConfigName.value.trim(),
                description: editImportConfigDescription.value.trim(),
                isDefault: editImportConfigIsDefault.value
            };
        }

        await loadImportConfigList();
        showEditImportConfigDialog.value = false;
        snackbar.value?.showMessage('模板已更新');
    } catch (error) {
        logger.error('failed to update import config', error);
        snackbar.value?.showError('Unable to update saved template');
    }
}

function removeImportConfig(config: ImportConfigMatchResult): void {
    confirmDialog.value?.open('format.misc.confirmDelete', {
        name: config.name
    }).then(async () => {
        try {
            const response = await services.deleteImportConfig({ id: config.id });
            if (!response.data?.success) {
                throw new Error('delete failed');
            }
            if (matchedImportConfig.value?.id === config.id) {
                matchedImportConfig.value = null;
            }
            await loadImportConfigList();
            snackbar.value?.showMessage('模板已删除');
        } catch (error) {
            logger.error('failed to delete import config', error);
            snackbar.value?.showError('Unable to delete saved template');
        }
    });
}

function buildImportTransactionsFromParsedItems(items: ImportTransaction[] | undefined): ImportTransaction[] | undefined {
    if (!items) {
        return undefined;
    }

    for (const transaction of items) {
        if (transaction.valid) {
            transaction.selected = true;
        }
    }

    return items;
}

async function executeColumnMappingImport(): Promise<void> {
    if (!importFile.value || !importTransactionDefineColumnTab.value) {
        snackbar.value?.showError('Please select at least one file');
        return;
    }

    const mapping = importTransactionDefineColumnTab.value.generateResult();
    if (!mapping) {
        return;
    }

    const parseResult = await transactionsStore.parseImportTransaction({
        fileType: getImportConfigFileFormat(),
        importFile: importFile.value,
        columnMapping: mapping.columnMapping,
        transactionTypeMapping: mapping.transactionTypeMapping,
        hasHeaderLine: mapping.includeHeader,
        timeFormat: mapping.timeFormat,
        timezoneFormat: mapping.timezoneFormat,
        amountDecimalSeparator: mapping.amountDecimalSeparator,
        amountDigitGroupingSymbol: mapping.amountDigitGroupingSymbol,
        geoSeparator: mapping.geoLocationSeparator,
        geoOrder: mapping.geoLocationOrder,
        tagSeparator: mapping.tagSeparator,
        delimiter: parsedFileDelimiter.value
    });

    importTransactions.value = buildImportTransactionsFromParsedItems(
        parseResult.items.map((item, idx) => ImportTransaction.of(item, idx))
    );
    serverSessionId.value = '';
    currentStep.value = 'checkData';
}

function openSaveImportConfigDialog(): void {
    if (!parsedFileData.value || !importTransactionDefineColumnTab.value) {
        return;
    }

    saveImportConfigName.value = matchedImportConfig.value?.name ||
        `${importFile.value?.name || 'import'} 模板`;
    saveImportConfigDescription.value = getImportConfigDisplayDescription(matchedImportConfig.value);
    loadImportConfigList().then(() => {
        const hasDefaultTemplate = importConfigList.value.some(config => !!config.isDefault);
        const isFirstTemplate = importConfigList.value.length === 0;
        saveImportConfigRecommended.value = !hasDefaultTemplate && (
            isFirstTemplate || !!matchedImportConfig.value?.defaultRecommendation
        );
        saveImportConfigIsDefault.value = !!matchedImportConfig.value?.isDefault || saveImportConfigRecommended.value;
        showSaveImportConfigDialog.value = true;
    }).catch(error => {
        logger.error('failed to load import config list before saving', error);
        snackbar.value?.showError('Unable to load saved templates');
    });
}

async function saveCurrentImportConfig(): Promise<void> {
    if (!importTransactionDefineColumnTab.value || !parsedFileData.value?.length) {
        return;
    }

    const mapping = importTransactionDefineColumnTab.value.generateResult();
    if (!mapping) {
        return;
    }

    try {
        const response = await services.saveImportConfig({
            id: matchedImportConfig.value?.id,
            name: saveImportConfigName.value.trim(),
            fileFormat: getImportConfigFileFormat(),
            description: saveImportConfigDescription.value.trim(),
            fieldMappings: mapping,
            delimiter: parsedFileDelimiter.value,
            hasHeader: mapping.includeHeader,
            sampleHeaders: parsedFileData.value[0] || [],
            isDefault: saveImportConfigIsDefault.value,
            encoding: matchedImportConfig.value?.encoding || 'utf-8'
        });

        const savedId = response.data?.result?.id;
        matchedImportConfig.value = {
            id: savedId || matchedImportConfig.value?.id || 0,
            name: saveImportConfigName.value.trim(),
            fileFormat: getImportConfigFileFormat(),
            description: saveImportConfigDescription.value.trim(),
            descriptionSummary: saveImportConfigDescription.value.trim(),
            fieldMappings: mapping as unknown as Record<string, unknown>,
            sampleHeaders: parsedFileData.value[0] || [],
            delimiter: parsedFileDelimiter.value,
            encoding: matchedImportConfig.value?.encoding || 'utf-8',
            hasHeader: mapping.includeHeader,
            isDefault: saveImportConfigIsDefault.value,
            defaultRecommendation: !saveImportConfigIsDefault.value && saveImportConfigRecommended.value
        };
        await loadImportConfigList();
        showSaveImportConfigDialog.value = false;
        snackbar.value?.showMessage('模板已保存');
    } catch (error) {
        logger.error('failed to save import config', error);
        snackbar.value?.showError('Unable to save data mapping file');
    }
}

/**
 * v6.48: 三阶段导入 - 解析并去重
 *
 * 阶段1: 上传所有文件到后端，写入 bills_parser_template 表
 * 阶段2: 执行去重处理，结果写入 bills_preview 表，返回预览数据
 */
async function parseData(): Promise<void> {
    if (currentStep.value === 'uploadFile' && shouldUseColumnMapping.value) {
        try {
            await prepareColumnMappingStep();
        } catch (error) {
            logger.error('failed to prepare column mapping step', error);
            snackbar.value?.showError('Unable to prepare column mapping data');
        }
        return;
    }

    if (currentStep.value === 'uploadFile' && importFiles.value.length > 1) {
        logger.info(`[导入] 检测到多文件导入 ${importFiles.value.length} 个文件，将直接使用三阶段并行解析`);
    }

    if (currentStep.value === 'defineColumn') {
        submitting.value = true;
        try {
            await executeColumnMappingImport();
        } catch (error) {
            logger.error('failed to parse import file by column mapping', error);
            snackbar.value?.showError('Unable to parse import file');
        } finally {
            submitting.value = false;
        }
        return;
    }

    if (importFiles.value.length === 0) {
        snackbar.value?.showError('Please select at least one file');
        return;
    }

    submitting.value = true;
    importProcess.value = 0;

    try {
        // ========== 阶段1: 上传并解析所有文件 ==========
        logger.info(`[三阶段导入-阶段1] 开始上传 ${importFiles.value.length} 个文件`);

        const formData = new FormData();
        for (const file of importFiles.value) {
            formData.append('files', file);
        }

        // v6.79: 直接使用 'auto'，解析器会自动识别文件格式
        formData.append('parser_type', 'auto');

        const token = getCurrentToken();
        const headers: Record<string, string> = {};
        if (token) {
            headers['Authorization'] = `Bearer ${token}`;
        }

        const stage1Response = await fetch('/api/bills/import/v2/parse', {
            method: 'POST',
            headers: headers,
            body: formData
        });

        if (!stage1Response.ok) {
            const errorText = await stage1Response.text();
            throw new Error(`阶段1失败: ${errorText}`);
        }

        const stage1Result = await stage1Response.json();
        logger.info(`[三阶段导入-阶段1] 完成: success=${stage1Result.success}, session_id=${stage1Result.data?.session_id}, parsed_count=${stage1Result.data?.parsed_count}`);

        if (!stage1Result.success || !stage1Result.data?.session_id) {
            throw new Error(stage1Result.error || '解析失败：未获取到session_id');
        }

        serverSessionId.value = stage1Result.data.session_id;
        importProcess.value = 30;

        // ========== 阶段2: 去重并获取预览 ==========
        logger.info(`[三阶段导入-阶段2] 开始去重处理, session_id=${serverSessionId.value}`);

        const stage2Response = await fetch('/api/bills/import/v2/dedup', {
            method: 'POST',
            headers: {
                ...headers,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ session_id: serverSessionId.value })
        });

        if (!stage2Response.ok) {
            const errorText = await stage2Response.text();
            throw new Error(`阶段2失败: ${errorText}`);
        }

        const stage2Result = await stage2Response.json();
        logger.info(`[三阶段导入-阶段2] 完成: success=${stage2Result.success}, preview_count=${stage2Result.data?.preview_count || stage2Result.data?.preview?.length || 0}`);

        if (!stage2Result.success) {
            throw new Error(stage2Result.error || '去重预览失败');
        }

        logger.info(`[三阶段导入-阶段2] 去重统计: ${JSON.stringify(stage2Result.data?.dedup_stats || {})}`);

        // 转换预览数据为前端ImportTransaction格式
        const previewData = stage2Result.data?.preview || [];
        const transactions = previewData.map((item: any, idx: number) => {
            return convertPreviewToImportTransaction(item, idx);
        });

        logger.info(`[三阶段导入-阶段2] 转换完成: ${transactions.length} 条交易`);

        importTransactions.value = transactions;
        currentStep.value = 'checkData';
        importProcess.value = 100;

    } catch (error) {
        logger.error('[三阶段导入] 失败:', error);
        snackbar.value?.showError(`导入失败: ${error}`);
        // 如果失败，清理后端会话
        if (serverSessionId.value) {
            cleanupServerSession();
        }
    } finally {
        submitting.value = false;
    }
}

/**
 * 将后端预览数据转换为前端 ImportTransaction 格式
 */
function convertPreviewToImportTransaction(item: any, index: number): ImportTransaction {
    // 类型映射: 后端中文类型 -> 前端数字类型
    const typeMap: Record<string, number> = {
        '支出': 3,    // Expense
        '收入': 2,    // Income
        '转账': 4,    // Transfer
        '投资': 5,    // Investment
        '退款': 2     // 退款视为收入
    };
    const type = typeMap[item.preview_type] || 3;

    // 解析时间
    const timeStr = item.preview_date || '';
    const time = new Date(timeStr).getTime() / 1000;

    // 金额转换：后端是元，前端期望分
    const amountInCents = Math.round(Math.abs(item.preview_amount || 0) * 100);
    const destAmountInCents = Math.round(Math.abs(item.preview_destination_amount || 0) * 100);

    // 根据分类名称查找categoryId
    let categoryId = '';
    const mainCat = item.preview_main_category || '';
    const subCat = item.preview_sub_category || '';
    if (mainCat || subCat) {
        const categoriesMap = transactionCategoriesStore.allTransactionCategoriesMap;
        for (const [catId, cat] of Object.entries(categoriesMap)) {
            if (subCat && cat.name === subCat) {
                categoryId = catId;
                break;
            }
            if (!subCat && mainCat && cat.name === mainCat && !cat.parentId) {
                categoryId = catId;
                break;
            }
        }
    }

    // 账户ID
    const sourceAccountId = item.preview_source_account_id ? String(item.preview_source_account_id) : '';
    const destAccountId = item.preview_destination_account_id ? String(item.preview_destination_account_id) : '';

    // 时区
    const currentTimezone = settingsStore.appSettings.timeZone;
    const defaultUtcOffset = getTimezoneOffsetMinutes(currentTimezone);

    const responseItem: ImportTransactionResponse = {
        type: type,
        categoryId: categoryId,
        originalCategoryName: subCat || mainCat || '',
        time: isNaN(time) ? Date.now() / 1000 : time,
        utcOffset: defaultUtcOffset,
        sourceAccountId: sourceAccountId,
        originalSourceAccountName: item.preview_payment_method || '',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: destAccountId,
        sourceAmount: amountInCents,
        // v6.55: 转账和投资类型都使用目标金额
        destinationAmount: (type === 4 || type === 5) ? destAmountInCents || amountInCents : amountInCents,
        tagIds: [],
        originalTagNames: [],
        comment: item.preview_description || '',
        counterparty: item.preview_counterparty || '',
        paymentMethod: item.preview_payment_method || '',
        suggestedType: typeMap[item.suggested_preview_type] || undefined,
        transferSuggestionScore: Number(item.transfer_suggestion_score || 0),
        transferSuggestionLevel: item.transfer_suggestion_level || '',
        transferSuggestionReason: item.transfer_suggestion_reason || '',
        investmentSignalScore: Number(item.investment_signal_score || 0),
        investmentSignalLevel: item.investment_signal_level || '',
        investmentSignalReason: item.investment_signal_reason || '',
        investmentPlatform: item.investment_platform || '',
        investmentProduct: item.investment_product || '',
        recurringTemplateId: item.preview_recurring_id ? String(item.preview_recurring_id) : '',
        recurringTemplateName: item.preview_recurring_name || '',
        recurringCandidateCount: Number(item.preview_recurring_candidate_count || 0),
        recurringMatchScore: Number(item.preview_recurring_match_score || 0),
        recurringMatchReasons: item.preview_recurring_match_reasons || '',
        recurringMatchedDate: item.preview_recurring_matched_date || ''
    };

    // 添加预览表ID，用于阶段3确认导入
    const transaction = ImportTransaction.of(responseItem, index);
    (transaction as any)._previewId = item.id;  // 保存预览表记录ID

    return transaction;
}

/**
 * v6.55: 处理重新分类后的数据更新
 * @param previewData 后端返回的原始预览数据数组（preview_* 字段格式）
 */
function onReclassified(previewData: any[]): void {
    if (!previewData || previewData.length === 0) {
        return;
    }

    logger.info(`[三阶段导入] 收到重新分类结果: ${previewData.length} 条预览数据`);

    // 将后端预览数据转换为前端 ImportTransaction 格式
    const convertedTransactions = previewData.map((item, idx) => {
        return convertPreviewToImportTransaction(item, idx);
    });

    logger.info(`[三阶段导入] 转换完成: ${convertedTransactions.length} 条交易`);

    // 替换整个数组
    importTransactions.value = convertedTransactions;
}

/**
 * 清理后端导入会话
 */
async function cleanupServerSession(): Promise<void> {
    if (!serverSessionId.value) return;

    try {
        const token = getCurrentToken();
        const headers: Record<string, string> = {};
        if (token) {
            headers['Authorization'] = `Bearer ${token}`;
        }

        await fetch(`/api/bills/import/v2/session/${serverSessionId.value}`, {
            method: 'DELETE',
            headers: headers
        });
        logger.info(`[三阶段导入] 已清理会话: ${serverSessionId.value}`);
    } catch (e) {
        logger.warn('[三阶段导入] 清理会话失败:', e);
    }

    serverSessionId.value = '';
}

function submit(): void {
    if (importTransactionCheckDataTab.value?.isEditing) {
        return;
    }

    // 收集用户选中的交易
    const selectedTransactions: ImportTransaction[] = [];

    if (importTransactions.value) {
        for (const importTransaction of importTransactions.value) {
            if (importTransaction.valid && importTransaction.selected) {
                selectedTransactions.push(importTransaction);
            } else if (!importTransaction.valid && importTransaction.selected) {
                snackbar.value?.showError('Cannot import invalid transactions');
                return;
            }
        }
    }

    if (selectedTransactions.length < 1) {
        snackbar.value?.showError('No data to import');
        return;
    }

    confirmDialog.value?.open('format.misc.confirmImportTransactions', {
        count: getDisplayCount(selectedTransactions.length)
    }).then(async () => {
        submitting.value = true;
        importProcess.value = 0;

        try {
            if (!serverSessionId.value) {
                const clientSessionId = `import-${Date.now()}`;
                const token = getCurrentToken();
                const headers: Record<string, string> = {
                    'Content-Type': 'application/json'
                };
                if (token) {
                    headers['Authorization'] = `Bearer ${token}`;
                }

                const response = await fetch('/api/bills/batch', {
                    method: 'POST',
                    headers,
                    body: JSON.stringify({
                        clientSessionId,
                        transactions: selectedTransactions.map(transaction => ({
                            ...transaction.toCreateRequest(),
                            clientSessionId
                        }))
                    })
                });

                if (!response.ok) {
                    const errorText = await response.text();
                    throw new Error(`导入失败: ${errorText}`);
                }

                const result = await response.json();
                if (!result.success) {
                    throw new Error(result.error || '导入失败');
                }

                importedCount.value = result.result?.items?.length || selectedTransactions.length;
                currentStep.value = 'finalResult';

                accountsStore.updateAccountListInvalidState(true);
                transactionsStore.updateTransactionListInvalidState(true);
                overviewStore.updateTransactionOverviewInvalidState(true);
                statisticsStore.updateTransactionStatisticsInvalidState(true);
                return;
            }

            // v6.48+: 使用三阶段确认API作为唯一主链
            logger.info(`[三阶段导入-阶段3] 开始确认导入, session_id=${serverSessionId.value}`);

            // 收集用户编辑后的数据
            const previewUpdates = selectedTransactions.map(t => {
                // 类型反向映射
                const typeReverseMap: Record<number, string> = {
                    2: '收入',
                    3: '支出',
                    4: '转账',
                    5: '投资'
                };

                return {
                    id: (t as any)._previewId,
                    preview_type: typeReverseMap[t.type] || '支出',
                    preview_amount: t.sourceAmount / 100,  // 分转元
                    preview_destination_amount: t.destinationAmount / 100,
                    preview_source_account_id: t.sourceAccountId ? parseInt(t.sourceAccountId) : null,
                    preview_destination_account_id: t.destinationAccountId ? parseInt(t.destinationAccountId) : null,
                    preview_recurring_id: t.recurringTemplateId ? parseInt(t.recurringTemplateId) : null,
                    preview_recurring_name: t.recurringTemplateName || '',
                    preview_recurring_candidate_count: t.recurringCandidateCount || 0,
                    preview_recurring_match_score: t.recurringMatchScore || 0,
                    preview_recurring_match_reasons: t.recurringMatchReasons || '',
                    preview_recurring_matched_date: t.recurringMatchedDate || '',
                    category_id: t.categoryId ? parseInt(t.categoryId) : null,
                    selected: t.selected
                };
            });

            const token = getCurrentToken();
            const headers: Record<string, string> = {
                'Content-Type': 'application/json'
            };
            if (token) {
                headers['Authorization'] = `Bearer ${token}`;
            }

            const response = await fetch('/api/bills/import/v2/confirm', {
                method: 'POST',
                headers: headers,
                body: JSON.stringify({
                    session_id: serverSessionId.value,
                    preview_updates: previewUpdates
                })
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(`确认导入失败: ${errorText}`);
            }

            const result = await response.json();
            logger.info(`[三阶段导入-阶段3] 完成: imported=${result.data?.imported_count}`);

            if (!result.success) {
                throw new Error(result.error || '确认导入失败');
            }

            importedCount.value = result.data?.imported_count || selectedTransactions.length;
            currentStep.value = 'finalResult';

            // v6.61: 清理服务器会话（虽然后端 import_stage3_confirm 已经清理了临时表，
            // 但为了健壮性，在成功时也显式调用清理以确保数据被删除）
            await cleanupServerSession();
            serverSessionId.value = '';

            // 刷新相关store
            accountsStore.updateAccountListInvalidState(true);
            transactionsStore.updateTransactionListInvalidState(true);
            overviewStore.updateTransactionOverviewInvalidState(true);
            statisticsStore.updateTransactionStatisticsInvalidState(true);

        } catch (error) {
            logger.error('[三阶段导入-阶段3] 失败:', error);
            snackbar.value?.showError(`导入失败: ${error}`);
        } finally {
            submitting.value = false;
        }
    });
}

function close(completed: boolean): void {
    if (completed) {
        if (resolveFunc) {
            resolveFunc();
        }
    } else {
        // v6.51: 用户点击取消时，清理后端会话（清空 parser_template 和 preview 表）
        if (serverSessionId.value) {
            cleanupServerSession();
        }
        if (rejectFunc) {
            rejectFunc();
        }
    }

    showState.value = false;
}



defineExpose({
    open
});
</script>
