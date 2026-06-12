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
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" :disabled="loading || submitting"
                           v-if="currentStep === 'checkData' && importTransactionCheckDataTab?.filterMenus">
                        <v-icon :icon="mdiFilterOutline" />
                        <v-menu
                            activator="parent"
                            max-height="500"
                            min-width="360"
                            v-model="showCheckDataFilterMenu"
                            :close-on-content-click="false"
                        >
                            <v-list
                                density="compact"
                                class="py-1 import-check-data-filter-menu"
                                v-model:opened="openedCheckDataFilterGroups"
                                open-strategy="multiple"
                            >
                                <v-list-group
                                    v-for="group in importTransactionCheckDataTab.filterMenus"
                                    :key="group.title"
                                    :value="group.title"
                                >
                                    <template #activator="{ props: groupActivatorProps }">
                                        <v-list-item
                                            v-bind="groupActivatorProps"
                                            class="import-check-data-filter-menu__group"
                                        >
                                            <template #title>
                                                <span
                                                    :class="{
                                                        'import-check-data-filter-menu__group-title': true,
                                                        'import-check-data-filter-menu__group-title--active': isActiveCheckDataFilterGroup(group.summary)
                                                    }"
                                                >
                                                    {{ group.title }}
                                                </span>
                                            </template>
                                        </v-list-item>
                                    </template>

                                    <template
                                        v-for="(menu, index) in group.items"
                                        :key="`${group.title}_${index}`"
                                    >
                                        <v-list-group
                                            v-if="menu.items?.length"
                                            :value="`${group.title}_${menu.title}`"
                                        >
                                            <template #activator="{ props: childGroupActivatorProps }">
                                                <v-list-item
                                                    v-bind="childGroupActivatorProps"
                                                    :prepend-icon="menu.prependIcon"
                                                    :title="menu.title"
                                                    :subtitle="menu.subTitle"
                                                    :disabled="menu.disabled"
                                                    class="import-check-data-filter-menu__item"
                                                />
                                            </template>
                                            <v-list-item
                                                v-for="(childMenu, childIndex) in menu.items"
                                                :key="`${group.title}_${menu.title}_${childIndex}`"
                                                :prepend-icon="childMenu.prependIcon"
                                                :title="childMenu.title"
                                                :subtitle="childMenu.subTitle"
                                                :append-icon="childMenu.appendIcon"
                                                :disabled="childMenu.disabled"
                                                class="import-check-data-filter-menu__item"
                                                @click="childMenu.onClick?.()"
                                            />
                                        </v-list-group>
                                        <v-list-item
                                            v-else
                                            :prepend-icon="menu.prependIcon"
                                            :title="menu.title"
                                            :subtitle="menu.subTitle"
                                            :append-icon="menu.appendIcon"
                                            :disabled="menu.disabled"
                                            class="import-check-data-filter-menu__item"
                                            @click="menu.onClick?.()"
                                        />
                                    </template>
                                </v-list-group>
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
                                                 @click="menu.onClick?.()" />
                                </template>
                            </v-list>
                        </v-menu>
                    </v-btn>
                </div>
            </template>

            <div class="import-flow-progress mt-4 cursor-default" aria-live="polite">
                <div class="d-flex align-start justify-space-between gap-4 flex-wrap">
                    <div>
                        <div class="text-caption text-medium-emphasis">
                            {{ tt('Import') }} {{ currentFlowProgressIndex + 1 }}/{{ importFlowProgressItems.length }}
                        </div>
                        <h5 class="text-subtitle-1 mb-1">{{ currentFlowProgressItem.title }}</h5>
                        <div class="text-body-2 text-medium-emphasis">{{ currentFlowProgressDetail }}</div>
                    </div>
                    <v-chip density="comfortable" color="primary" variant="tonal">
                        {{ tt('Current') }}
                    </v-chip>
                </div>

                <v-progress-linear
                    class="mt-4"
                    color="primary"
                    bg-opacity="0.12"
                    rounded
                    height="8"
                    :model-value="currentFlowProgressValue"
                />

                <div class="import-flow-progress__trail mt-3" role="list" :aria-label="tt('Import Preview')">
                    <div
                        v-for="(item, index) in importFlowProgressItems"
                        :key="item.key"
                        role="listitem"
                        :aria-current="item.active ? 'step' : undefined"
                        :class="[
                            'import-flow-progress__step',
                            {
                                'import-flow-progress__step--active': item.active,
                                'import-flow-progress__step--complete': item.complete
                            }
                        ]"
                    >
                        <span class="import-flow-progress__marker">{{ index + 1 }}</span>
                        <span class="import-flow-progress__label">{{ item.title }}</span>
                    </div>
                </div>
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
                    <import-transaction-check-data-tab
                        ref="importTransactionCheckDataTab"
                        :import-transactions="importTransactions"
                        :server-paged="serverPagedPreviewMode"
                        :total-import-transaction-count="previewTotalCount"
                        :preview-metadata="previewMetadata"
                        :disabled="loading || submitting"
                        :session-id="serverSessionId"
                        @reclassified="onReclassified"
                        @request-page="onCheckDataPageRequested"
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
    ImportPreviewServerQueryFilters,
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

import { ref, computed, nextTick, useTemplateRef, watch } from 'vue';

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

import type { LocalizedImportFileTypeSubType } from '@/core/file.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';

import { getCurrentToken } from '@/lib/userstate.ts';
import { openImportFileDialog } from '@/lib/importFileDialog.ts';
import services from '@/lib/services.ts';
import logger from '@/lib/logger.ts';
import { DEFAULT_IMPORT_API_TIMEOUT, DEFAULT_IMPORT_PARSE_API_TIMEOUT } from '@/consts/api.ts';

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
type ImportFlowProgressKey = 'selectSource' | 'parseStageRows' | 'reviewPreview' | 'confirmImport' | 'result';

interface ImportFlowProgressItem {
    complete: boolean;
    key: ImportFlowProgressKey;
    title: string;
    active: boolean;
}

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
    fieldMappings: ImportFieldMappings;
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

interface ImportFieldMappings {
    includeHeader?: boolean;
    columnMapping?: Record<number, number>;
    transactionTypeMapping?: Record<string, number>;
    timeFormat?: string;
    timezoneFormat?: string;
    amountDecimalSeparator?: string;
    amountDigitGroupingSymbol?: string;
    geoLocationSeparator?: string;
    geoLocationOrder?: string;
    tagSeparator?: string;
}

interface ImportTransactionCheckDataFilterMenuGroup {
    title: string;
    summary?: string;
}

const IMPORT_FLOW_PROGRESS_ORDER: ImportFlowProgressKey[] = [
    'selectSource',
    'parseStageRows',
    'reviewPreview',
    'confirmImport',
    'result'
];

const FALLBACK_IMPORT_FLOW_PROGRESS_ITEM: ImportFlowProgressItem = {
    active: true,
    complete: false,
    key: 'selectSource',
    title: ''
};

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
const serverSessionId = ref<string>('');  // v6.48: 后端三阶段导入的会话ID
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
let previewPageRequestSequence = 0;
let previewPageAbortController: AbortController | null = null;
const parsedFileDelimiter = ref<string>('');
const matchedImportConfig = ref<ImportConfigMatchResult | null>(null);

const SERVER_PAGED_PREVIEW_SORTABLE_COLUMNS = new Set<string>([
    'time',
    'type',
    'sourceAmount',
    'counterparty',
    'paymentMethod',
    'comment'
]);

// v7: 未匹配文件的逐文件列映射队列
interface UnmatchedFileInfo {
    originalName: string;
    tempPath: string;
}
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

const importFlowProgressTitleMap = computed<Record<ImportFlowProgressKey, string>>(() => ({
    selectSource: tt('Select File'),
    parseStageRows: `${tt('Parser')} / ${tt('Define Columns')}`,
    reviewPreview: tt('Check Data'),
    confirmImport: `${tt('Confirm')} ${tt('Import')}`,
    result: tt('Import Result')
}));

const currentImportFlowProgressKey = computed<ImportFlowProgressKey>(() => {
    if (currentStep.value === 'finalResult') {
        return 'result';
    }

    if (currentStep.value === 'checkData') {
        return submitting.value ? 'confirmImport' : 'reviewPreview';
    }

    if (
        currentStep.value === 'defineColumn'
        || currentStep.value === 'executeCustomScript'
        || (currentStep.value === 'uploadFile' && submitting.value)
    ) {
        return 'parseStageRows';
    }

    return 'selectSource';
});

const currentFlowProgressIndex = computed<number>(() => {
    const index = IMPORT_FLOW_PROGRESS_ORDER.indexOf(currentImportFlowProgressKey.value);
    return index >= 0 ? index : 0;
});

const importFlowProgressItems = computed<ImportFlowProgressItem[]>(() => {
    const activeIndex = currentFlowProgressIndex.value;
    const titles = importFlowProgressTitleMap.value;

    return IMPORT_FLOW_PROGRESS_ORDER.map((key, index) => ({
        active: index === activeIndex,
        complete: index < activeIndex,
        key,
        title: titles[key]
    }));
});

const currentFlowProgressItem = computed<ImportFlowProgressItem>(() => {
    return importFlowProgressItems.value[currentFlowProgressIndex.value]
        ?? importFlowProgressItems.value[0]
        ?? FALLBACK_IMPORT_FLOW_PROGRESS_ITEM;
});

const currentFlowProgressValue = computed<number>(() => {
    return ((currentFlowProgressIndex.value + 1) / IMPORT_FLOW_PROGRESS_ORDER.length) * 100;
});

const currentFlowProgressDetail = computed<string>(() => {
    if (currentImportFlowProgressKey.value === 'parseStageRows') {
        if (unmatchedFilesQueue.value.length > 0) {
            const fileName = unmatchedFilesQueue.value[currentUnmatchedIndex.value]?.originalName || '';
            return `${fileName} (${currentUnmatchedIndex.value + 1}/${unmatchedFilesQueue.value.length})`;
        }

        return matchedImportConfig.value?.name || tt('Parser');
    }

    if (currentImportFlowProgressKey.value === 'reviewPreview') {
        return previewTotalCount.value > 0
            ? tt('format.misc.previewCount', { count: getDisplayCount(previewTotalCount.value) })
            : tt('Import Preview');
    }

    if (currentImportFlowProgressKey.value === 'confirmImport') {
        return importProcess.value > 0
            ? tt('format.misc.importingTransactions', { process: formatNumberToLocalizedNumerals(importProcess.value, 2) })
            : tt('Confirm');
    }

    if (currentImportFlowProgressKey.value === 'result') {
        return tt('format.misc.importTransactionResult', { count: getDisplayCount(importedCount.value || 0) });
    }

    return fileName.value || supportedImportFileExtensions.value;
});

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

const showHandlingMethodSelector = computed<boolean>(() => {
    if (importFiles.value.length > 1) {
        return false;
    }

    if (fileType.value !== 'dsv' && fileType.value !== 'dsv_data') {
        return false;
    }

    return !looksLikeStructuredBillStatementFile(importFile.value);
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

let resolveFunc: (() => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const numeralSystem = computed<NumeralSystem>(() => getCurrentNumeralSystemType());

// 精简后的文件类型选项
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

function normalizeImportConfigMatchResult(config: Partial<ImportConfigMatchResult>): ImportConfigMatchResult | null {
    const id = Number(config.id);
    const name = typeof config.name === 'string' ? config.name.trim() : '';

    if (!Number.isFinite(id) || !name) {
        return null;
    }

    return {
        id,
        name,
        fileFormat: config.fileFormat,
        description: config.description,
        descriptionSummary: config.descriptionSummary,
        fieldMappings: config.fieldMappings || {},
        sampleHeaders: config.sampleHeaders || [],
        dateFormat: config.dateFormat,
        delimiter: config.delimiter,
        encoding: config.encoding,
        skipRows: config.skipRows,
        hasHeader: config.hasHeader,
        customRules: config.customRules,
        isDefault: config.isDefault,
        defaultRecommendation: config.defaultRecommendation,
        matchScore: config.matchScore,
        matchReason: config.matchReason
    };
}

function isActiveCheckDataFilterGroup(summary?: string): boolean {
    return !!summary && summary !== tt('All');
}

function getVisibleCheckDataFilterGroups(): ImportTransactionCheckDataFilterMenuGroup[] {
    return importTransactionCheckDataTab.value?.filterMenus || [];
}

function syncOpenedCheckDataFilterGroups(): void {
    const groups = getVisibleCheckDataFilterGroups();
    if (groups.length < 1) {
        openedCheckDataFilterGroups.value = [];
        return;
    }

    const validTitles = new Set(groups.map(group => group.title));
    const retainedTitles = openedCheckDataFilterGroups.value.filter(title => validTitles.has(title));

    if (retainedTitles.length > 0) {
        openedCheckDataFilterGroups.value = retainedTitles;
        return;
    }

    const activeTitles = groups
        .filter(group => isActiveCheckDataFilterGroup(group.summary))
        .map(group => group.title);
    const firstGroup = groups[0];

    openedCheckDataFilterGroups.value = activeTitles.length > 0
        ? activeTitles
        : (firstGroup ? [firstGroup.title] : []);
}

watch(showCheckDataFilterMenu, visible => {
    if (visible) {
        syncOpenedCheckDataFilterGroups();
    }
});

watch(currentStep, step => {
    if (step !== 'checkData') {
        showCheckDataFilterMenu.value = false;
    }
});

watch(
    () => getVisibleCheckDataFilterGroups().map(group => `${group.title}:${group.summary || ''}`).join('|'),
    () => {
        if (showCheckDataFilterMenu.value) {
            syncOpenedCheckDataFilterGroups();
        }
    }
);

function open(): Promise<void> {
    abortPendingPreviewPageRequest();
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
}

function looksLikeStructuredBillStatementFile(file?: File): boolean {
    const fileName = file?.name?.toLowerCase() || '';

    return /微信支付账单|wechat|wxpay|支付宝交易明细|alipay/.test(fileName);
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

async function loadImportConfigList(): Promise<void> {
    const response = await services.getImportConfigs({
        fileFormat: getImportConfigFileFormat()
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
            fieldMappings: mapping,
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
 * v7: 为未匹配的服务端临时文件准备列映射界面
 */
async function prepareColumnMappingForUnmatchedFile(fileInfo: UnmatchedFileInfo): Promise<void> {
    logger.info(`[列映射] 准备文件: ${fileInfo.originalName} (${currentUnmatchedIndex.value + 1}/${unmatchedFilesQueue.value.length})`);

    const previewResponse = await services.previewImportFileFromTemp({
        tempPath: fileInfo.tempPath,
        delimiter: parsedFileDelimiter.value || undefined
    });
    const preview = previewResponse.data?.result as ImportFilePreviewResult | undefined;
    const rows = preview?.sampleData || [];

    if (!rows.length) {
        snackbar.value?.showError(`文件 ${fileInfo.originalName} 没有可导入的数据`);
        return;
    }

    parsedFileDelimiter.value = preview?.delimiter || parsedFileDelimiter.value;
    parsedFileData.value = rows.slice(0, 300);
    currentStep.value = 'defineColumn';

    await nextTick();
    importTransactionDefineColumnTab.value?.reset();

    const headers = rows[0] || [];
    if (!headers.length) return;

    // 尝试自动匹配和建议列映射
    try {
        const response = await services.matchImportConfig({ fileFormat: 'csv', headers });
        const result = response.data?.result;
        if (response.data?.success && result?.fieldMappings) {
            matchedImportConfig.value = result;
            parsedFileDelimiter.value = result.delimiter || parsedFileDelimiter.value;
            importTransactionDefineColumnTab.value?.applyFieldMappings(result.fieldMappings);
            snackbar.value?.showMessage(getMatchedImportConfigMessage(result));
            return;
        }
    } catch (error) {
        logger.warn('failed to match import config', error);
    }

    try {
        const suggestionResponse = await services.suggestImportConfig({
            fileFormat: 'csv',
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
 * v7: 执行阶段2去重并显示预览
 */
function normalizePreviewPageSortBy(value: string | null | undefined): string {
    const normalizedValue = String(value || '').trim();
    return SERVER_PAGED_PREVIEW_SORTABLE_COLUMNS.has(normalizedValue) ? normalizedValue : '';
}

function normalizePreviewPageSortDirection(value: string | null | undefined): 'asc' | 'desc' {
    return String(value || '').toLowerCase() === 'desc' ? 'desc' : 'asc';
}

const PREVIEW_PAGE_FILTER_PARAM_NAMES: Record<keyof ImportPreviewServerQueryFilters, string> = {
    minDatetime: 'min_datetime',
    maxDatetime: 'max_datetime',
    transactionType: 'transaction_type',
    category: 'category',
    account: 'account',
    tag: 'tag',
    signal: 'signal',
    annotation: 'annotation',
    description: 'description',
};

function appendPreviewPageFilters(
    searchParams: URLSearchParams,
    filters: ImportPreviewServerQueryFilters | undefined
): void {
    if (!filters) {
        return;
    }

    for (const [key, value] of Object.entries(filters)) {
        if (typeof value !== 'string') {
            continue;
        }
        searchParams.set(PREVIEW_PAGE_FILTER_PARAM_NAMES[key as keyof ImportPreviewServerQueryFilters], value);
    }
}

function abortPendingPreviewPageRequest(): void {
    previewPageRequestSequence += 1;
    previewPageAbortController?.abort();
    previewPageAbortController = null;
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
    const requestSequence = previewPageRequestSequence + 1;
    previewPageRequestSequence = requestSequence;
    previewPageAbortController?.abort();
    const controller = new AbortController();
    previewPageAbortController = controller;

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

        if (requestSequence !== previewPageRequestSequence
            || controller.signal.aborted
            || sessionId !== serverSessionId.value) {
            return;
        }

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`获取预览分页失败: ${errorText}`);
        }

        const result = await response.json();
        if (requestSequence !== previewPageRequestSequence
            || controller.signal.aborted
            || sessionId !== serverSessionId.value) {
            return;
        }
        if (!result.success) {
            throw new Error(result.error || '获取预览分页失败');
        }

        const previewData = Array.isArray(result.data?.preview) ? result.data.preview as ImportPreviewRecord[] : [];
        importTransactions.value = previewData.map((item, idx) => convertPreviewToImportTransaction(item, idx));
        previewTotalCount.value = Number(result.data?.total || 0);
        previewMetadata.value = (result.data?.metadata || null) as ImportPreviewMetadata | null;
        logger.info(
            `[三阶段导入-预览分页] 加载 page=${normalizedPage}, page_size=${normalizedPageSize}, sort_by=${normalizedSortBy || 'default'}, sort_direction=${normalizedSortDirection}, filters=${Object.keys(sortOptions.filters || {}).length}, rows=${previewData.length}, total=${previewTotalCount.value}`
        );
    } catch (error) {
        if (isAbortError(error)) {
            return;
        }
        throw error;
    } finally {
        if (requestSequence === previewPageRequestSequence) {
            previewPageAbortController = null;
        }
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
    logger.info(`[三阶段导入-阶段2] 完成: success=${stage2Result.success}, preview_count=${stage2Result.data?.preview_count || stage2Result.data?.preview?.length || 0}`);

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
 * v7: 三阶段导入 - 解析并去重（含逐文件列映射）
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
        logger.info(`[三阶段导入-阶段1] 完成: success=${stage1Result.success}, session_id=${stage1Result.data?.session_id}, parsed_count=${stage1Result.data?.parsed_count}`);

        if (!stage1Result.success || !stage1Result.data?.session_id) {
            throw new Error(stage1Result.error || '解析失败：未获取到session_id');
        }

        serverSessionId.value = stage1Result.data.session_id;
        importProcess.value = 30;

        // v7: 检查是否有未匹配特定解析器的文件
        const unmatchedFiles: Array<{ original_name: string; temp_path: string }> = stage1Result.data.unmatched_files || [];

        if (unmatchedFiles.length > 0) {
            // 有未匹配文件，进入逐文件列映射流程
            logger.info(`[三阶段导入] ${unmatchedFiles.length} 个文件未匹配特定解析器，进入列映射流程`);
            unmatchedFilesQueue.value = unmatchedFiles.map(f => ({
                originalName: f.original_name,
                tempPath: f.temp_path
            }));
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
 * v6.55: 处理重新分类后的数据更新
 * @param previewData 后端返回的原始预览数据数组（使用 preview_* 字段）
 */
function onReclassified(previewData: ImportPreviewRecord[]): void {
    if (serverPagedPreviewMode.value) {
        const page = importTransactionCheckDataTab.value?.getCurrentPreviewPage?.() || 1;
        const pageSize = importTransactionCheckDataTab.value?.getCurrentPreviewPageSize?.() || 10;
        const requestOptions = importTransactionCheckDataTab.value?.getCurrentServerPagedRequestOptions?.() || {};
        void fetchPreviewPage(page, pageSize, requestOptions);
        return;
    }

    if (!previewData || previewData.length === 0) {
        return;
    }

    logger.info(`[三阶段导入] 收到重新分类结果: ${previewData.length} 条预览数据`);

    // 将后端预览数据转换为前端导入交易格式
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

            // v6.48+: 使用三阶段确认接口作为唯一主链
            logger.info(`[三阶段导入-阶段3] 开始确认导入, session_id=${serverSessionId.value}`);

            // 收集用户编辑后的数据
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

            // v6.61: 清理服务器会话（虽然后端 import_stage3_confirm 已经清理了临时表，
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
        // v6.51: 用户点击取消时，清理后端会话（清空 parser_template 和 preview 表）
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

<style scoped>
.import-flow-progress {
    border: 1px solid rgba(var(--v-theme-on-surface), 0.08);
    border-radius: 16px;
    padding: 16px;
    background: rgba(var(--v-theme-surface), 0.78);
}

.import-flow-progress__trail {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 10px;
}

.import-flow-progress__step {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    color: rgba(var(--v-theme-on-surface), 0.58);
    font-size: 0.78rem;
}

.import-flow-progress__marker {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 24px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px solid rgba(var(--v-theme-on-surface), 0.18);
    color: currentColor;
    font-weight: 700;
}

.import-flow-progress__label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.import-flow-progress__step--active {
    color: rgb(var(--v-theme-primary));
    font-weight: 700;
}

.import-flow-progress__step--complete {
    color: rgba(var(--v-theme-on-surface), 0.78);
}

.import-flow-progress__step--active .import-flow-progress__marker,
.import-flow-progress__step--complete .import-flow-progress__marker {
    border-color: rgb(var(--v-theme-primary));
    background: rgba(var(--v-theme-primary), 0.12);
}

@media (max-width: 700px) {
    .import-flow-progress__trail {
        grid-template-columns: 1fr;
    }
}

.import-check-data-filter-drawer :deep(.v-navigation-drawer__content) {
    display: flex;
    flex-direction: column;
}

.import-check-data-filter-menu__group-title {
    font-size: 0.98rem;
    font-weight: 700;
    letter-spacing: 0.01em;
}

.import-check-data-filter-menu__group-title--active {
    color: rgb(var(--v-theme-primary));
}

.import-check-data-filter-menu :deep(.v-list-group__items .v-list-item) {
    padding-inline-start: 28px;
}
</style>
