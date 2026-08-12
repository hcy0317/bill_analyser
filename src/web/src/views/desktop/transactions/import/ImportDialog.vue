<template>
    <v-dialog :persistent="!!persistent" v-model="showState">
        <v-card class="pa-6 pa-sm-10 pa-md-12" data-testid="desktop.import.dialog">
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
                    <import-check-data-filter-button
                        v-if="currentStep === 'checkData' && importTransactionCheckDataTab?.filterMenus"
                        v-model:visible="showCheckDataFilterMenu"
                        v-model:opened="openedCheckDataFilterGroups"
                        :disabled="loading || submitting"
                        :filter-menus="importTransactionCheckDataTab.filterMenus"
                        :is-active-summary="isActiveCheckDataFilterGroup"
                    />
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
                                                 @click="menu.onClick?.()" />
                                </template>
                            </v-list>
                        </v-menu>
                    </v-btn>
                </div>
            </template>

            <import-flow-progress
                class="mt-4"
                :current-index="currentFlowProgressIndex"
                :current-item="currentFlowProgressItem"
                :current-label="tt('Current')"
                :detail="currentFlowProgressDetail"
                :import-label="tt('Import')"
                :items="importFlowProgressItems"
                :progress-value="currentFlowProgressValue"
                :trail-label="tt('Import Preview')"
            />

            <v-window class="disable-tab-transition" v-model="currentStep">
                <v-window-item value="uploadFile">
                    <v-row>

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

                        <v-col cols="12" md="12" v-if="showHandlingMethodSelector">
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
                                data-testid="desktop.import.file-name"
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
                    <decision-preview-replacement-bridge :on-reclassified="onReclassified" v-slot="{ handleReclassified }">
                        <import-transaction-check-data-tab
                            ref="importTransactionCheckDataTab"
                            :import-transactions="importTransactions"
                            :server-paged="serverPagedPreviewMode"
                            :total-import-transaction-count="previewTotalCount"
                            :preview-metadata="previewMetadata"
                            :disabled="loading || submitting"
                            :session-id="serverSessionId"
                            @reclassified="handleReclassified"
                            @invalidate-page-request="abortPendingPreviewPageRequest" @request-page="onCheckDataPageRequested"
                        />
                    </decision-preview-replacement-bridge>
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
                       data-testid="desktop.import.action.next"
                       :disabled="loading || submitting || (!isImportDataFromTextbox && !importFile) || (isImportDataFromTextbox && !importData)"
                       :append-icon="!submitting ? mdiArrowRight : undefined" @click="parseData"
                       v-if="currentStep === 'defineColumn' || currentStep === 'executeCustomScript' || currentStep === 'uploadFile'">
                    {{ tt('Next') }}
                    <v-progress-circular indeterminate size="22" class="ms-2" v-if="submitting"></v-progress-circular>
                </v-btn>
                <v-btn class="button-icon-with-direction" color="teal"
                       data-testid="desktop.import.action.confirm"
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
    <input ref="fileInput" data-testid="desktop.import.file-input" type="file" multiple style="display: none" :accept="supportedImportFileExtensions" @change="setImportFile($event)" />
</template>

<script setup lang="ts">
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import ImportTransactionDefineColumnTab from './tabs/ImportTransactionDefineColumnTab.vue';
import ImportTransactionExecuteCustomScriptTab from './tabs/ImportTransactionExecuteCustomScriptTab.vue';
import ImportTransactionCheckDataTab from './tabs/ImportTransactionCheckDataTab.vue';
import DecisionPreviewReplacementBridge from './DecisionPreviewReplacementBridge';
import ImportFlowProgress from './import-dialog/ImportFlowProgress.vue';
import { applyNonServerPagedReplacement, applyServerPagedReclassification } from './decisionPreviewReplacement';
import ImportCheckDataFilterButton from './import-dialog/ImportCheckDataFilterButton.vue';
import {
    resolveImportPreviewCategoryPath,
    type ImportPreviewRecord
} from './importPreview.ts';
import {
    buildImportTransactionFromPreviewRecord,
    type ImportPreviewTransactionDraft
} from './importPreviewTransaction.ts';
import {
    buildImportPreviewUpdateFromTransaction,
    getPreviewIdFromImportTransaction,
    getPreviewUpdateId
} from './importPreviewUpdates.ts';
import type {
    ImportPreviewMetadata,
    PreviewPageRequestOptions
} from './importPreviewIndex.ts';
import {
    buildImportPreviewHistoryRewriteAcknowledgement,
    buildImportPreviewHistoryRewriteOperationAcknowledgement,
    type ImportPreviewHistoryRewriteAcknowledgement,
    type ImportPreviewHistoryRewriteAcknowledgementOperation
} from './checkDataMatching.ts';
import {
    extractApiErrorMessage,
    fetchImportStage,
    isAbortError
} from './importDialogApi.ts';
import {
    getImportConfigDisplayDescription,
    getMatchedImportConfigMessage,
    normalizeImportConfigMatchResult,
    resolveActiveImportSource,
    resolveImportConfigFileFormat,
    resolveUnmatchedImportFiles,
    type ImportStageUnmatchedFile
} from './import-dialog/importConfigHelpers.ts';
import {
    appendPreviewPageFilters,
    buildCanonicalPreviewPageRequestKey,
    normalizePreviewPageSortBy,
    normalizePreviewPageSortDirection
} from './import-dialog/previewPageQuery.ts';
import { PreviewPageRequestCoordinator } from './import-dialog/previewPageRequestCoordinator.ts';
import {
    ImportDSVProcessMethod,
    type ImportConfigMatchResult,
    type ImportConfigSuggestionResult,
    type ImportFieldMappings,
    type ImportFilePreviewResult,
    type ImportTransactionDialogStep,
    type UnmatchedFileInfo
} from './import-dialog/types.ts';
import { createImportFlowMilestoneLogger } from './import-dialog/importFlowProfiler.ts';
import { useImportFlowProgress } from './import-dialog/useImportFlowProgress.ts';
import { useImportCheckDataFilterMenu } from './import-dialog/useImportCheckDataFilterMenu.ts';
import { useImportSourceSelection } from './import-dialog/useImportSourceSelection.ts';
import { ref, computed, nextTick, useTemplateRef } from 'vue';
import { useI18n } from '@/locales/helpers.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';
import { useOverviewStore } from '@/stores/overview.ts';
import { useStatisticsStore } from '@/stores/statistics.ts';
import { useSettingsStore } from '@/stores/setting.ts';
import { useUserStore } from '@/stores/user.ts';

import { type NumeralSystem } from '@/core/numeral.ts';
import { CategoryType } from '@/core/category.ts';

import { ImportTransaction } from '@/models/imported_transaction.ts';

import { getCurrentToken } from '@/lib/userstate.ts';
import { openImportFileDialog } from '@/lib/importFileDialog.ts';
import services from '@/lib/services.ts';
import logger from '@/lib/logger.ts';
import { DEFAULT_IMPORT_API_TIMEOUT, DEFAULT_IMPORT_PARSE_API_TIMEOUT } from '@/consts/api.ts';

import {
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
const userStore = useUserStore();

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const importTransactionDefineColumnTab = useTemplateRef<ImportTransactionDefineColumnTabType>('importTransactionDefineColumnTab');
const importTransactionExecuteCustomScriptTab = useTemplateRef<ImportTransactionExecuteCustomScriptTabType>('importTransactionExecuteCustomScriptTab');
const importTransactionCheckDataTab = useTemplateRef<ImportTransactionCheckDataTabType>('importTransactionCheckDataTab');
const fileInput = useTemplateRef<HTMLInputElement>('fileInput');

const showState = ref<boolean>(false);
const serverSessionId = ref<string>('');
const currentStep = ref<ImportTransactionDialogStep>('uploadFile');
const importProcess = ref<number>(0);
const selectedFileTypes = ref<string[]>(['auto']);
const importFiles = ref<File[]>([]);
const importData = ref<string>('');
const parsedFileData = ref<string[][] | undefined>(undefined);
const importTransactions = ref<ImportTransaction[] | undefined>(undefined);
const previewTotalCount = ref<number>(0);
const previewMetadata = ref<ImportPreviewMetadata | null>(null);
const serverPagedPreviewMode = ref<boolean>(false);
const previewPageSortBy = ref<string>('');
const previewPageSortDirection = ref<'asc' | 'desc'>('asc');
const pendingInitialCheckDataPageRequest = ref<{
    page: number;
    pageSize: number;
    sortBy: string;
    sortDirection: 'asc' | 'desc';
} | null>(null);
const previewPageRequests = new PreviewPageRequestCoordinator();
const importDialogOpenedAt = ref<number | null>(null);
const importSubmitStartedAt = ref<number | null>(null);
const firstOperablePreviewLogged = ref<boolean>(false);
const parsedFileDelimiter = ref<string>('');
const parsedFileEncoding = ref<string>('auto');
const matchedImportConfig = ref<ImportConfigMatchResult | null>(null);

const unmatchedFilesQueue = ref<UnmatchedFileInfo[]>([]);
const currentUnmatchedIndex = ref<number>(0);

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
const showCheckDataFilterMenu = ref<boolean>(false);
const openedCheckDataFilterGroups = ref<string[]>([]);

const importFile = computed<File | undefined>(() => {
    return importFiles.value.length > 0 ? importFiles.value[0] : undefined;
});
const activeImportSource = computed(() => resolveActiveImportSource(
    importFile.value,
    unmatchedFilesQueue.value[currentUnmatchedIndex.value]
));

const {
    allFileSubTypes,
    exportFileGuideDocumentLanguageName,
    exportFileGuideDocumentUrl,
    fileType,
    isImportDataFromTextbox,
    showHandlingMethodSelector,
    supportedImportFileExtensions
} = useImportSourceSelection({
    importFile,
    importFiles,
    selectedFileTypes
});

const importedCount = ref<number | null>(null);
const loading = ref<boolean>(true);
const submitting = ref<boolean>(false);

let resolveFunc: (() => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const numeralSystem = computed<NumeralSystem>(() => getCurrentNumeralSystemType());

const logImportFlowMilestone = createImportFlowMilestoneLogger({
    importDialogOpenedAt,
    importSubmitStartedAt,
    logger
});

const fileName = computed<string>(() => {
    if (importFiles.value.length === 0) return '';
    const file = importFiles.value[0];
    if (importFiles.value.length === 1 && file) return file.name;
    return `${importFiles.value.length} files selected`;
});

function getDisplayCount(count: number): string {
    return numeralSystem.value.formatNumber(count);
}

const {
    currentFlowProgressDetail,
    currentFlowProgressIndex,
    currentFlowProgressItem,
    currentFlowProgressValue,
    importFlowProgressItems
} = useImportFlowProgress({
    currentStep,
    submitting,
    importProcess,
    unmatchedFilesQueue,
    currentUnmatchedIndex,
    matchedImportConfig,
    previewTotalCount,
    importedCount,
    fileName,
    supportedImportFileExtensions,
    translate: tt,
    formatNumber: formatNumberToLocalizedNumerals,
    formatCount: getDisplayCount
});

const { isActiveCheckDataFilterGroup } = useImportCheckDataFilterMenu({
    currentStep,
    openedGroups: openedCheckDataFilterGroups,
    showMenu: showCheckDataFilterMenu,
    getFilterMenus: () => importTransactionCheckDataTab.value?.filterMenus || [],
    translate: tt
});

function open(): Promise<void> {
    abortPendingPreviewPageRequest();
    importDialogOpenedAt.value = Date.now();
    importSubmitStartedAt.value = null;
    firstOperablePreviewLogged.value = false;
    logImportFlowMilestone('import_dialog_opened_at');
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
    parsedFileEncoding.value = 'auto';
    matchedImportConfig.value = null;
    unmatchedFilesQueue.value = [];
    currentUnmatchedIndex.value = 0;
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
    showCheckDataFilterMenu.value = false;
    openedCheckDataFilterGroups.value = [];
    importConfigList.value = [];
    importTransactionDefineColumnTab.value?.reset();
    importTransactionExecuteCustomScriptTab.value?.reset();
    importTransactions.value = undefined;
    previewTotalCount.value = 0;
    previewMetadata.value = null;
    serverPagedPreviewMode.value = false;
    previewPageSortBy.value = '';
    previewPageSortDirection.value = 'asc';
    pendingInitialCheckDataPageRequest.value = null;
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

async function showOpenFileDialog(): Promise<void> {
    if (submitting.value) {
        return;
    }

    await openImportFileDialog({
        accept: supportedImportFileExtensions.value,
        fileInput: fileInput.value,
        onFilesSelected: setSelectedImportFiles
    });
}

function setImportFile(event: Event): void {
    if (!event || !event.target) {
        return;
    }

    const el = event.target as HTMLInputElement;

    if (!el.files || !el.files.length) {
        return;
    }

    setSelectedImportFiles(Array.from(el.files));
    el.value = '';
}

function setSelectedImportFiles(files: readonly File[]): void {
    importFiles.value = Array.from(files);
    processDSVMethod.value = ImportDSVProcessMethod.AutoDetect;
    matchedImportConfig.value = null;
    parsedFileData.value = undefined;
    parsedFileEncoding.value = 'auto';
    logImportFlowMilestone('files_selected_at', {
        file_count: importFiles.value.length,
        total_size_bytes: importFiles.value.reduce((sum, file) => sum + file.size, 0)
    });
}

async function loadImportConfigList(): Promise<void> {
    const response = await services.getImportConfigs({
        fileFormat: activeImportSource.value.fileFormat
    });
    const result = Array.isArray(response.data?.result) ? response.data.result : [];
    importConfigList.value = result
        .map((config: Partial<ImportConfigMatchResult>) => normalizeImportConfigMatchResult(config))
        .filter((config): config is ImportConfigMatchResult => config !== null);
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
    importTransactionDefineColumnTab.value?.applyFieldMappings(config.fieldMappings);
    matchedImportConfig.value = config;
    parsedFileDelimiter.value = config.delimiter || parsedFileDelimiter.value;
    parsedFileEncoding.value = config.encoding || parsedFileEncoding.value;
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
            fileFormat: targetConfig.fileFormat || activeImportSource.value.fileFormat,
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

async function executeColumnMappingImport(): Promise<void> {
    if (!importTransactionDefineColumnTab.value) {
        snackbar.value?.showError('Column mapping tab not ready');
        return;
    }

    const mapping = importTransactionDefineColumnTab.value.generateResult();
    if (!mapping) {
        return;
    }

    const currentFile = unmatchedFilesQueue.value[currentUnmatchedIndex.value];
    if (!currentFile) {
        snackbar.value?.showError('No unmatched file to process');
        return;
    }

    // 使用新的 v2/parse_generic 接口，将通用解析结果写入同一个会话
    const response = await services.parseGenericIntoSession({
        sessionId: serverSessionId.value,
        tempPath: currentFile.tempPath,
        columnMapping: mapping.columnMapping as Record<string, number>,
        transactionTypeMapping: mapping.transactionTypeMapping as Record<string, number> | undefined,
        hasHeaderLine: mapping.includeHeader,
        timeFormat: mapping.timeFormat,
        timezoneFormat: mapping.timezoneFormat,
        amountDecimalSeparator: mapping.amountDecimalSeparator,
        amountDigitGroupingSymbol: mapping.amountDigitGroupingSymbol,
        fileEncoding: parsedFileEncoding.value || undefined,
        delimiter: parsedFileDelimiter.value || undefined
    });

    if (!response.data?.success) {
        throw new Error(extractApiErrorMessage(response.data, `解析文件 ${currentFile.originalName} 失败`));
    }

    logger.info(`[列映射导入] 文件 ${currentFile.originalName} 解析完成, parsed_count=${response.data?.result?.parsed_count || 0}`);

    // 检查是否还有更多未匹配文件
    const nextIndex = currentUnmatchedIndex.value + 1;
    if (nextIndex < unmatchedFilesQueue.value.length) {
        // 还有下一个未匹配文件，准备列映射
        currentUnmatchedIndex.value = nextIndex;
        await prepareColumnMappingForUnmatchedFile(unmatchedFilesQueue.value[nextIndex]!);
        return;
    }

    // 所有列映射都完成了，进入阶段2去重
    await executeStage2Dedup();
}

function openSaveImportConfigDialog(): void {
    if (!parsedFileData.value || !importTransactionDefineColumnTab.value) {
        return;
    }

    saveImportConfigName.value = matchedImportConfig.value?.name ||
        `${activeImportSource.value.fileName} 模板`;
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
            fileFormat: activeImportSource.value.fileFormat,
            description: saveImportConfigDescription.value.trim(),
            fieldMappings: mapping,
            dateFormat: mapping.timeFormat || '',
            delimiter: parsedFileDelimiter.value,
            skipRows: 0,
            hasHeader: mapping.includeHeader,
            customRules: {},
            sampleHeaders: parsedFileData.value[0] || [],
            isDefault: saveImportConfigIsDefault.value,
            encoding: parsedFileEncoding.value || 'auto'
        });

        const savedId = response.data?.result?.id;
        matchedImportConfig.value = {
            id: savedId || matchedImportConfig.value?.id || 0,
            name: saveImportConfigName.value.trim(),
            fileFormat: activeImportSource.value.fileFormat,
            description: saveImportConfigDescription.value.trim(),
            descriptionSummary: saveImportConfigDescription.value.trim(),
            fieldMappings: mapping,
            dateFormat: mapping.timeFormat || '',
            sampleHeaders: parsedFileData.value[0] || [],
            delimiter: parsedFileDelimiter.value,
            encoding: parsedFileEncoding.value || 'auto',
            skipRows: 0,
            hasHeader: mapping.includeHeader,
            customRules: {},
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
 */
async function prepareColumnMappingForUnmatchedFile(fileInfo: UnmatchedFileInfo): Promise<void> {
    logger.info(`[列映射] 准备文件: ${fileInfo.originalName} (${currentUnmatchedIndex.value + 1}/${unmatchedFilesQueue.value.length})`);
    parsedFileData.value = undefined;
    parsedFileDelimiter.value = '';
    parsedFileEncoding.value = 'auto';
    matchedImportConfig.value = null;
    const sessionId = serverSessionId.value;
    if (!sessionId) {
        throw new Error('Import session is unavailable for temporary file preview');
    }

    const previewResponse = await services.previewImportFileFromTemp({
        sessionId,
        tempPath: fileInfo.tempPath,
        fileEncoding: parsedFileEncoding.value || undefined,
        delimiter: parsedFileDelimiter.value || undefined
    });
    const preview = previewResponse.data?.result as ImportFilePreviewResult | undefined;
    const rows = preview?.sampleData || [];

    if (!rows.length) {
        snackbar.value?.showError(`文件 ${fileInfo.originalName} 没有可导入的数据`);
        return;
    }

    parsedFileDelimiter.value = preview?.delimiter || parsedFileDelimiter.value;
    parsedFileEncoding.value = preview?.encoding || parsedFileEncoding.value;
    parsedFileData.value = rows.slice(0, 300);
    currentStep.value = 'defineColumn';

    await nextTick();
    importTransactionDefineColumnTab.value?.reset();

    const headers = rows[0] || [];
    if (!headers.length) return;

    // 尝试自动匹配和建议列映射
    const fileFormat = resolveImportConfigFileFormat({ name: fileInfo.originalName });
    try {
        const response = await services.matchImportConfig({ fileFormat, headers });
        const result = response.data?.result;
        if (response.data?.success && result?.fieldMappings) {
            matchedImportConfig.value = result;
            parsedFileDelimiter.value = result.delimiter || parsedFileDelimiter.value;
            parsedFileEncoding.value = result.encoding || parsedFileEncoding.value;
            importTransactionDefineColumnTab.value?.applyFieldMappings(result.fieldMappings);
            snackbar.value?.showMessage(getMatchedImportConfigMessage(result));
            return;
        }
    } catch (error) {
        logger.warn('failed to match import config', error);
    }

    try {
        const suggestionResponse = await services.suggestImportConfig({
            fileFormat,
            headers,
            sampleRows: rows.slice(1, 21)
        });
        const suggestion = suggestionResponse.data?.result as ImportConfigSuggestionResult | undefined;
        if (suggestion?.columnMapping && Object.keys(suggestion.columnMapping).length > 0) {
            importTransactionDefineColumnTab.value?.applyFieldMappings(suggestion as ImportFieldMappings);
            snackbar.value?.showMessage('已自动建议列映射');
        }
    } catch (error) {
        logger.warn('failed to suggest import config', error);
    }
}

/**
 */
function abortPendingPreviewPageRequest(): void {
    previewPageRequests.abort();
}

async function fetchPreviewPage(
    page: number = 1,
    pageSize: number = 10,
    sortOptions: PreviewPageRequestOptions = {}
): Promise<void> {
    if (!serverSessionId.value) {
        return;
    }

    const normalizedPage = Math.max(page || 1, 1);
    const normalizedPageSize = Math.max(pageSize || 10, 1);
    const normalizedSortBy = normalizePreviewPageSortBy(sortOptions.sortBy ?? previewPageSortBy.value);
    const normalizedSortDirection = normalizePreviewPageSortDirection(
        sortOptions.sortDirection ?? previewPageSortDirection.value
    );
    const token = getCurrentToken();
    const headers: Record<string, string> = { 'Content-Type': 'application/json' };
    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }

    previewPageSortBy.value = normalizedSortBy;
    previewPageSortDirection.value = normalizedSortDirection;

    const searchParams = new URLSearchParams({
        page: String(normalizedPage),
        page_size: String(normalizedPageSize)
    });
    if (normalizedSortBy) {
        searchParams.set('sort_by', normalizedSortBy);
        searchParams.set('sort_direction', normalizedSortDirection);
    }
    appendPreviewPageFilters(searchParams, sortOptions.filters);

    const sessionId = serverSessionId.value;
    const requestKey = `${sessionId}?${buildCanonicalPreviewPageRequestKey(
        normalizedPage,
        normalizedPageSize,
        normalizedSortBy,
        normalizedSortDirection,
        sortOptions.filters
    )}`;
    const requestHandle = previewPageRequests.begin(requestKey, {
        replaceActive: sortOptions.replaceActive
    });
    if (!requestHandle) {
        return;
    }
    const controller = requestHandle.controller;

    try {
        const response = await fetchImportStage(
            `/api/bills/import/v2/preview/${encodeURIComponent(sessionId)}?${searchParams.toString()}`,
            {
                method: 'GET',
                headers,
                signal: controller.signal,
            },
            '预览分页加载'
        );

        if (!previewPageRequests.isCurrent(requestHandle)
            || sessionId !== serverSessionId.value) {
            return;
        }

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`获取预览分页失败: ${errorText}`);
        }

        const result = await response.json();
        if (!previewPageRequests.isCurrent(requestHandle)
            || sessionId !== serverSessionId.value) {
            return;
        }
        if (!result.success) {
            throw new Error(result.error || '获取预览分页失败');
        }
        logImportFlowMilestone('preview_response_received_at', {
            page: normalizedPage,
            page_size: normalizedPageSize,
            signal_filter: sortOptions.filters?.signal || null
        });

        const previewData = Array.isArray(result.data?.preview) ? result.data.preview as ImportPreviewRecord[] : [];
        importTransactions.value = previewData.map((item, idx) => convertPreviewToImportTransaction(item, idx));
        previewTotalCount.value = Number(result.data?.total || 0);
        previewMetadata.value = (result.data?.metadata || null) as ImportPreviewMetadata | null;
        if (!firstOperablePreviewLogged.value && normalizedPage === 1) {
            firstOperablePreviewLogged.value = true;
            logImportFlowMilestone('first_operable_preview_at', {
                preview_table_mounted_at: new Date().toISOString(),
                row_count: previewData.length,
                total: previewTotalCount.value
            });
        }
        logger.info(
            `[三阶段导入-预览分页] 加载 page=${normalizedPage}, page_size=${normalizedPageSize}, sort_by=${normalizedSortBy || 'default'}, sort_direction=${normalizedSortDirection}, filters=${Object.keys(sortOptions.filters || {}).length}, rows=${previewData.length}, total=${previewTotalCount.value}`
        );
    } catch (error) {
        if (isAbortError(error)) {
            return;
        }
        throw error;
    } finally {
        previewPageRequests.finish(requestHandle);
    }
}

async function onCheckDataPageRequested(
    page: number = 1,
    pageSize: number = 10,
    sortOptions?: PreviewPageRequestOptions
): Promise<void> {
    const normalizedPage = Math.max(page || 1, 1);
    const normalizedPageSize = Math.max(pageSize || 10, 1);
    const normalizedSortBy = normalizePreviewPageSortBy(sortOptions?.sortBy ?? previewPageSortBy.value);
    const normalizedSortDirection = normalizePreviewPageSortDirection(
        sortOptions?.sortDirection ?? previewPageSortDirection.value
    );
    const pendingRequest = pendingInitialCheckDataPageRequest.value;

    if (pendingRequest
        && pendingRequest.page === normalizedPage
        && pendingRequest.pageSize === normalizedPageSize
        && pendingRequest.sortBy === normalizedSortBy
        && pendingRequest.sortDirection === normalizedSortDirection) {
        pendingInitialCheckDataPageRequest.value = null;
    }

    try {
        await fetchPreviewPage(normalizedPage, normalizedPageSize, {
            sortBy: normalizedSortBy,
            sortDirection: normalizedSortDirection,
            filters: sortOptions?.filters,
            replaceActive: sortOptions?.replaceActive,
        });
    } catch (error) {
        if (isAbortError(error)) {
            return;
        }
        logger.error('[三阶段导入-预览分页] Check Data 加载失败:', error);
        snackbar.value?.showError(`导入失败: ${error}`);
    }
}

async function executeStage2Dedup(): Promise<void> {
    importProcess.value = 60;
    logger.info(`[三阶段导入-阶段2] 开始去重处理, session_id=${serverSessionId.value}`);
    logImportFlowMilestone('stage_status_poll_started_at', {
        session_id: serverSessionId.value
    });

    const token = getCurrentToken();
    const headers: Record<string, string> = { 'Content-Type': 'application/json' };
    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }

    const stage2Response = await fetchImportStage('/api/bills/import/v2/dedup', {
        method: 'POST',
        headers,
        body: JSON.stringify({
            session_id: serverSessionId.value,
            include_preview: false
        })
    }, '阶段2去重', DEFAULT_IMPORT_API_TIMEOUT);

    if (!stage2Response.ok) {
        const errorText = await stage2Response.text();
        throw new Error(`阶段2失败: ${errorText}`);
    }

    const stage2Result = await stage2Response.json();
    logImportFlowMilestone('stage2_response_received_at', {
        session_id: serverSessionId.value
    });
    logger.info(`[三阶段导入-阶段2] 完成: success=${stage2Result.success}, preview_count=${stage2Result.data?.after_dedup || stage2Result.data?.preview_count || stage2Result.data?.preview?.length || 0}`);

    if (!stage2Result.success) {
        throw new Error(stage2Result.error || '去重预览失败');
    }

    logger.info(`[三阶段导入-阶段2] 去重统计: ${JSON.stringify(stage2Result.data?.dedup_stats || {})}`);
    previewTotalCount.value = Number(stage2Result.data?.after_dedup || stage2Result.data?.preview_count || 0);
    previewMetadata.value = null;
    serverPagedPreviewMode.value = true;
    previewPageSortBy.value = '';
    previewPageSortDirection.value = 'asc';
    importTransactions.value = [];
    pendingInitialCheckDataPageRequest.value = {
        page: 1,
        pageSize: 10,
        sortBy: previewPageSortBy.value,
        sortDirection: previewPageSortDirection.value
    };

    currentStep.value = 'checkData';
    importProcess.value = 100;
}

/**
 *
 * 新流程：
 * 1. 所有文件先送入 v2/parse，命中特定解析器的直接写入同一会话
 * 2. 未匹配的文件列表返回给前端，逐个进行人工列映射
 * 3. 列映射完成后统一进入 v2/dedup 去重预览
 */
async function parseData(): Promise<void> {
    if (currentStep.value === 'defineColumn') {
        // 来自列映射步骤：提交当前文件的列映射
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
    importSubmitStartedAt.value = Date.now();
    firstOperablePreviewLogged.value = false;
    logImportFlowMilestone('import_submit_at', {
        file_count: importFiles.value.length,
        total_size_bytes: importFiles.value.reduce((sum, file) => sum + file.size, 0)
    });

    try {
        // ========== 阶段1: 上传并解析所有文件 ==========
        logger.info(`[三阶段导入-阶段1] 开始上传 ${importFiles.value.length} 个文件`);

        const formData = new FormData();
        for (const file of importFiles.value) {
            formData.append('files', file);
        }

        formData.append('parser_type', 'auto');

        const token = getCurrentToken();
        const headers: Record<string, string> = {};
        if (token) {
            headers['Authorization'] = `Bearer ${token}`;
        }

        const stage1Response = await fetchImportStage('/api/bills/import/v2/parse', {
            method: 'POST',
            headers: headers,
            body: formData
        }, '阶段1解析', DEFAULT_IMPORT_PARSE_API_TIMEOUT);

        if (!stage1Response.ok) {
            const errorText = await stage1Response.text();
            throw new Error(`阶段1失败: ${errorText}`);
        }

        const stage1Result = await stage1Response.json();
        logImportFlowMilestone('upload_completed_at', {
            session_id: stage1Result.data?.session_id,
            parsed_count: stage1Result.data?.parsed_count
        });
        logger.info(`[三阶段导入-阶段1] 完成: success=${stage1Result.success}, session_id=${stage1Result.data?.session_id}, parsed_count=${stage1Result.data?.parsed_count}`);

        if (!stage1Result.success || !stage1Result.data?.session_id) {
            throw new Error(stage1Result.error || '解析失败：未获取到session_id');
        }

        serverSessionId.value = stage1Result.data.session_id;
        importProcess.value = 30;

        const unmatchedFiles = (stage1Result.data.unmatched_files || []) as ImportStageUnmatchedFile[];

        if (unmatchedFiles.length > 0) {
            const resolvedFiles = resolveUnmatchedImportFiles(unmatchedFiles);
            if (resolvedFiles.errorMessage) {
                snackbar.value?.showError(resolvedFiles.errorMessage);
                await cleanupServerSession();
                return;
            }

            // 有未匹配文件，进入逐文件列映射流程
            logger.info(`[三阶段导入] ${unmatchedFiles.length} 个文件未匹配特定解析器，进入列映射流程`);
            unmatchedFilesQueue.value = resolvedFiles.queue;
            currentUnmatchedIndex.value = 0;

            submitting.value = false;
            await prepareColumnMappingForUnmatchedFile(unmatchedFilesQueue.value[0]!);
            return;
        }

        // 所有文件都匹配了特定解析器，直接进入阶段2
        await executeStage2Dedup();

    } catch (error) {
        logger.error('[三阶段导入] 失败:', error);
        snackbar.value?.showError(`导入失败: ${error}`);
        if (serverSessionId.value) {
            cleanupServerSession();
        }
    } finally {
        submitting.value = false;
    }
}

/**
 * 将后端预览数据转换为前端导入交易格式
 */
function convertPreviewToImportTransaction(item: ImportPreviewRecord, index: number): ImportTransaction {
    return buildImportTransactionFromPreviewRecord(item, index, {
        categoriesById: transactionCategoriesStore.allTransactionCategoriesMap,
        transferCategories: transactionCategoriesStore.allTransactionCategories[CategoryType.Transfer],
        cashTransferCategoryId: userStore.currentUserCashTransferCategoryId,
        timeZone: settingsStore.appSettings.timeZone
    });
}

/**
 * @param previewData 后端返回的原始预览数据数组（使用 preview_* 字段）
 */
function onReclassified(previewData: ImportPreviewRecord[], removedPreviewIds: number[] = []): void {
    if (applyServerPagedReclassification(serverPagedPreviewMode.value, () => {
        const page = importTransactionCheckDataTab.value?.getCurrentPreviewPage?.() || 1;
        const pageSize = importTransactionCheckDataTab.value?.getCurrentPreviewPageSize?.() || 10;
        const requestOptions = importTransactionCheckDataTab.value?.getCurrentServerPagedRequestOptions?.() || {};
        void fetchPreviewPage(page, pageSize, { ...requestOptions, replaceActive: true });
    })) return;

    if (!previewData || previewData.length === 0) {
        return;
    }

    logger.info(`[三阶段导入] 收到重新分类结果: ${previewData.length} 条预览数据`);

    // 将后端预览数据转换为前端导入交易格式
    const convertedTransactions = previewData.map((item, idx) => {
        return convertPreviewToImportTransaction(item, idx);
    });

    logger.info(`[三阶段导入] 转换完成: ${convertedTransactions.length} 条交易`);

    if (removedPreviewIds.length > 0) {
        importTransactions.value = applyNonServerPagedReplacement(
            importTransactions.value || [],
            removedPreviewIds,
            convertedTransactions,
            getPreviewIdFromTransaction,
        );
    } else {
        // 普通重新分类响应仍代表完整预览集合。
        importTransactions.value = convertedTransactions;
    }
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
    previewMetadata.value = null;
}

function getPreviewIdFromTransaction(transaction: ImportTransaction): number | null {
    return getPreviewIdFromImportTransaction(transaction);
}

function getHistoryRewriteOperationFromTransaction(
    transaction: ImportTransaction
): ImportPreviewHistoryRewriteAcknowledgementOperation | null {
    return buildImportPreviewHistoryRewriteOperationAcknowledgement(getPreviewIdFromTransaction(transaction), {
        reconciliationPlannedOperation: transaction.matching?.reconciliation?.planned_operation,
        reconciliationHistoryBillId: transaction.matching?.reconciliation?.history_bill_id,
        reconciliationHistoryBillVersion: transaction.matching?.reconciliation?.history_bill_version,
        reconciliationHistorySummary: transaction.matching?.reconciliation?.history_summary,
        reconciliationOperationId: transaction.matching?.reconciliation?.operation_id,
        reconciliationAcknowledgementToken: transaction.matching?.reconciliation?.acknowledgement_token,
        reconciliationDestructiveAckRequired: !!transaction.matching?.reconciliation?.destructive_ack_required
    });
}

async function fetchSelectedPreviewTransactionsForConfirm(
    selectedCount: number
): Promise<ImportTransaction[]> {
    if (!serverSessionId.value) {
        return [];
    }

    const token = getCurrentToken();
    const headers: Record<string, string> = { 'Content-Type': 'application/json' };
    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }

    const searchParams = new URLSearchParams({
        page: '1',
        page_size: String(Math.max(selectedCount, 1)),
        selected_only: 'true'
    });
    const response = await fetchImportStage(
        `/api/bills/import/v2/preview/${encodeURIComponent(serverSessionId.value)}?${searchParams.toString()}`,
        {
            method: 'GET',
            headers
        },
        '预览选中历史改写确认'
    );
    if (!response.ok) {
        throw new Error(`获取选中预览失败: ${await response.text()}`);
    }

    const result = await response.json();
    if (!result.success) {
        throw new Error(result.error || '获取选中预览失败');
    }

    const previewData = Array.isArray(result.data?.preview) ? result.data.preview as ImportPreviewRecord[] : [];
    return previewData.map((item, idx) => convertPreviewToImportTransaction(item, idx));
}

async function buildHistoryRewriteConfirmAcknowledgement({
    selectedTransactions,
    selectedPreviewUpdates,
    selectedCount
}: {
    selectedTransactions: ImportTransaction[];
    selectedPreviewUpdates: Record<string, unknown>[];
    selectedCount: number;
}): Promise<ImportPreviewHistoryRewriteAcknowledgement | null> {
    const selectedPreviewIds = new Set<number>();
    const operations = new Map<number, ImportPreviewHistoryRewriteAcknowledgementOperation>();
    const addTransaction = (transaction: ImportTransaction): void => {
        const previewId = getPreviewIdFromTransaction(transaction);
        if (previewId === null || !transaction.selected) {
            return;
        }
        selectedPreviewIds.add(previewId);
        const operation = getHistoryRewriteOperationFromTransaction(transaction);
        if (operation) {
            operations.set(operation.preview_id, operation);
        }
    };

    if (serverSessionId.value && serverPagedPreviewMode.value) {
        const selectedServerTransactions = await fetchSelectedPreviewTransactionsForConfirm(selectedCount);
        selectedServerTransactions.forEach(addTransaction);
        (importTransactions.value || []).forEach(addTransaction);
        for (const operation of importTransactionCheckDataTab.value?.getSelectedHistoryRewriteOperations?.() || []) {
            operations.set(operation.preview_id, operation);
        }

        for (const update of selectedPreviewUpdates) {
            const previewId = getPreviewUpdateId(update);
            const selected = update['selected'];
            if (previewId === null || typeof selected !== 'boolean') {
                continue;
            }
            if (selected) {
                selectedPreviewIds.add(previewId);
            } else {
                selectedPreviewIds.delete(previewId);
                operations.delete(previewId);
            }
        }
    } else {
        selectedTransactions.forEach(addTransaction);
    }

    return buildImportPreviewHistoryRewriteAcknowledgement({
        selectedPreviewIds: Array.from(selectedPreviewIds),
        operations: Array.from(operations.values()).filter(operation => selectedPreviewIds.has(operation.preview_id)),
        selectionScope: {
            mode: serverPagedPreviewMode.value ? 'server-paged-selected-preview' : 'visible-preview',
            preserve_unpatched_selection: serverPagedPreviewMode.value,
            selected_visible_history_rewrite_count: importTransactionCheckDataTab.value?.getSelectedVisibleHistoryRewriteOperationCount?.() || 0
        }
    });
}

function buildHistoryRewriteConfirmDetails(
    acknowledgement: ImportPreviewHistoryRewriteAcknowledgement | null
): string[] {
    return (acknowledgement?.operations || []).map(operation => (
        `${operation.planned_operation} #${operation.history_bill_id} v${operation.history_bill_version}`
    ));
}

async function submit(): Promise<void> {
    if (importTransactionCheckDataTab.value?.isEditing) {
        return;
    }

    // 收集用户选中的交易
    const selectedTransactions: ImportTransaction[] = [];
    let selectedPreviewUpdates: Record<string, unknown>[] = [];
    let selectedCount = 0;

    if (serverSessionId.value && serverPagedPreviewMode.value) {
        selectedPreviewUpdates = importTransactionCheckDataTab.value?.getSelectedPreviewUpdates?.() || [];
        selectedCount = importTransactionCheckDataTab.value?.getSelectedPreviewCount?.() || selectedPreviewUpdates.length;
    } else if (importTransactions.value) {
        for (const importTransaction of importTransactions.value) {
            if (importTransaction.valid && importTransaction.selected) {
                selectedTransactions.push(importTransaction);
            } else if (!importTransaction.valid && importTransaction.selected) {
                snackbar.value?.showError('Cannot import invalid transactions');
                return;
            }
        }
        selectedCount = selectedTransactions.length;
    }

    if (selectedCount < 1) {
        snackbar.value?.showError('No data to import');
        return;
    }

    let historyRewriteAcknowledgement: ImportPreviewHistoryRewriteAcknowledgement | null = null;
    try {
        historyRewriteAcknowledgement = serverSessionId.value
            ? await buildHistoryRewriteConfirmAcknowledgement({
                selectedTransactions,
                selectedPreviewUpdates,
                selectedCount
            })
            : null;
    } catch (error) {
        snackbar.value?.showError(extractApiErrorMessage(error, 'Failed to prepare history rewrite acknowledgement'));
        return;
    }

    const historyRewriteDetails = buildHistoryRewriteConfirmDetails(historyRewriteAcknowledgement);
    confirmDialog.value?.open('format.misc.confirmImportTransactions', {
        count: getDisplayCount(selectedCount),
        warning: historyRewriteDetails.length > 0 ? 'History Rewrite' : undefined,
        details: historyRewriteDetails,
        color: historyRewriteDetails.length > 0 ? 'warning' : 'primary'
    }).then(async confirmed => {
        if (!confirmed) {
            return;
        }

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

            logger.info(`[三阶段导入-阶段3] 开始确认导入, session_id=${serverSessionId.value}`);

            // 收集用户编辑后的数据
            const validAccountIds = new Set(accountsStore.allVisiblePlainAccounts.map(account => String(account.id)));
            const previewUpdates = serverPagedPreviewMode.value
                ? selectedPreviewUpdates
                : selectedTransactions.map(t => {
                    const rawCategoryPath = resolveImportPreviewCategoryPath(
                        t.categoryId,
                        transactionCategoriesStore.allTransactionCategoriesMap
                    );
                    const categoryPath = rawCategoryPath
                        && (rawCategoryPath.type === null || rawCategoryPath.type === t.type)
                        ? rawCategoryPath
                        : null;

                    return buildImportPreviewUpdateFromTransaction(t, {
                        categoryPath,
                        validAccountIds,
                        clearTransferDecision: !!(t as ImportPreviewTransactionDraft)._shouldClearTransferDecision
                    });
                });

            const token = getCurrentToken();
            const headers: Record<string, string> = {
                'Content-Type': 'application/json'
            };
            if (token) {
                headers['Authorization'] = `Bearer ${token}`;
            }

            const confirmPayload: Record<string, unknown> = {
                session_id: serverSessionId.value,
                preserve_unpatched_selection: serverPagedPreviewMode.value,
                preview_updates: previewUpdates
            };
            if (historyRewriteAcknowledgement) {
                confirmPayload['history_rewrite_acknowledgement'] = historyRewriteAcknowledgement;
            }

            const response = await fetchImportStage('/api/bills/import/v2/confirm', {
                method: 'POST',
                headers: headers,
                body: JSON.stringify(confirmPayload)
            }, '阶段3确认导入', DEFAULT_IMPORT_API_TIMEOUT);

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(`确认导入失败: ${errorText}`);
            }

            const result = await response.json();
            logger.info(`[三阶段导入-阶段3] 完成: imported=${result.data?.imported_count}`);

            if (!result.success) {
                throw new Error(result.error || '确认导入失败');
            }

            importedCount.value = result.data?.imported_count || selectedCount;
            currentStep.value = 'finalResult';

            // 但为了健壮性，在成功时也显式调用清理以确保数据被删除）
            await cleanupServerSession();
            serverSessionId.value = '';
            serverPagedPreviewMode.value = false;
            previewTotalCount.value = 0;
            previewMetadata.value = null;
            previewPageSortBy.value = '';
            previewPageSortDirection.value = 'asc';

            // 刷新相关状态仓库
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
        if (serverSessionId.value) {
            cleanupServerSession();
        }
        if (rejectFunc) {
            rejectFunc();
        }
    }

    importTransactions.value = undefined;
    previewTotalCount.value = 0;
    previewMetadata.value = null;
    serverPagedPreviewMode.value = false;
    previewPageSortBy.value = '';
    previewPageSortDirection.value = 'asc';
    pendingInitialCheckDataPageRequest.value = null;
    showState.value = false;
}



defineExpose({
    open
});
</script>

<style scoped src="./import-dialog/ImportDialog.scss"></style>
