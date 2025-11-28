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
                        <v-col cols="12" md="12">
                            <v-select
                                :items="fileTypeOptions"
                                item-title="title"
                                item-value="value"
                                :label="tt('File Type')"
                                v-model="selectedFileTypes"
                                multiple
                                chips
                                :disabled="submitting"
                            ></v-select>
                        </v-col>

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

                        <v-col cols="12" md="12" v-if="!isImportDataFromTextbox && allSupportedEncodings">
                            <v-select
                                item-title="displayName"
                                item-value="encoding"
                                :disabled="submitting"
                                :label="tt('File Encoding')"
                                :placeholder="tt('File Encoding')"
                                :items="allSupportedEncodings"
                                v-model="fileEncoding"
                            />
                        </v-col>

                        <v-col cols="12" md="12" v-if="fileType === 'dsv' || fileType === 'dsv_data'">
                            <v-select
                                item-title="displayName"
                                item-value="type"
                                :disabled="submitting"
                                :label="tt('Handling Method')"
                                :placeholder="tt('Handling Method')"
                                :items="[
                                    { displayName: tt('Column Mapping'), type: ImportDSVProcessMethod.ColumnMapping },
                                    { displayName: tt('Custom Script'), type: ImportDSVProcessMethod.CustomScript }
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
    <input ref="fileInput" type="file" multiple style="display: none" :accept="supportedImportFileExtensions" @change="setImportFile($event)" />
</template>

<script setup lang="ts">
import type { StepBarItem } from '@/components/desktop/StepsBar.vue';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import ImportTransactionDefineColumnTab from './tabs/ImportTransactionDefineColumnTab.vue';
import ImportTransactionExecuteCustomScriptTab from './tabs/ImportTransactionExecuteCustomScriptTab.vue';
import ImportTransactionCheckDataTab from './tabs/ImportTransactionCheckDataTab.vue';

import { ref, computed, useTemplateRef } from 'vue';

import { useI18n } from '@/locales/helpers.ts';

import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';
import { useOverviewStore } from '@/stores/overview.ts';
import { useStatisticsStore } from '@/stores/statistics.ts';

import { type NumeralSystem } from '@/core/numeral.ts';

import type { LocalizedImportFileTypeSubType, LocalizedImportFileTypeSupportedEncodings } from '@/core/file.ts';
import { ImportTransaction, type ImportTransactionResponse } from '@/models/imported_transaction.ts';

import { isNumber } from '@/lib/common.ts';
import { generateRandomUUID } from '@/lib/misc.ts';
import logger from '@/lib/logger.ts';

import {
    mdiFilterOutline,
    mdiCheck,
    mdiDotsVertical,
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
    ColumnMapping,
    CustomScript
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

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const importTransactionDefineColumnTab = useTemplateRef<ImportTransactionDefineColumnTabType>('importTransactionDefineColumnTab');
const importTransactionExecuteCustomScriptTab = useTemplateRef<ImportTransactionExecuteCustomScriptTabType>('importTransactionExecuteCustomScriptTab');
const importTransactionCheckDataTab = useTemplateRef<ImportTransactionCheckDataTabType>('importTransactionCheckDataTab');
const fileInput = useTemplateRef<HTMLInputElement>('fileInput');

const showState = ref<boolean>(false);
const clientSessionId = ref<string>('');
const currentStep = ref<ImportTransactionDialogStep>('uploadFile');
const importProcess = ref<number>(0);
const selectedFileTypes = ref<string[]>(['auto']);
const importFiles = ref<File[]>([]);
const importData = ref<string>('');
const parsedFileData = ref<string[][] | undefined>(undefined);
const importTransactions = ref<ImportTransaction[] | undefined>(undefined);

const fileSubType = ref<string>('');
const fileEncoding = ref<string>('utf-8');
const processDSVMethod = ref<ImportDSVProcessMethod>(ImportDSVProcessMethod.ColumnMapping);

const allSteps = computed<StepBarItem[]>(() => [
    { name: 'uploadFile', title: tt('Select File'), subTitle: tt('Select the file to import') },
    { name: 'defineColumn', title: tt('Define Columns'), subTitle: tt('Map columns to fields') },
    { name: 'executeCustomScript', title: tt('Custom Script'), subTitle: tt('Execute custom script') },
    { name: 'checkData', title: tt('Check Data'), subTitle: tt('Verify and edit data') },
    { name: 'finalResult', title: tt('Import Result'), subTitle: tt('View import result') }
]);

const fileType = computed<string>(() => {
    const type = selectedFileTypes.value[0];
    if (selectedFileTypes.value.length > 0 && type) {
        return type;
    }
    return 'auto';
});

const allFileSubTypes = computed<LocalizedImportFileTypeSubType[] | undefined>(() => undefined);

const isImportDataFromTextbox = computed<boolean>(() => false);

const allSupportedEncodings = computed<LocalizedImportFileTypeSupportedEncodings[] | undefined>(() => [
    { displayName: 'UTF-8', encoding: 'utf-8' },
    { displayName: 'GBK', encoding: 'gbk' },
    { displayName: 'GB18030', encoding: 'gb18030' },
    { displayName: 'Big5', encoding: 'big5' }
]);

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

// Simplified file type options
const fileTypeOptions = [
    { title: 'Auto', value: 'auto' },
    { title: 'Alipay', value: 'alipay' },
    { title: 'WeChat', value: 'wechat' },
    { title: 'ICBC', value: 'icbc' },
    { title: 'ABC', value: 'abc' },
    { title: 'CMBC', value: 'cmbc' },
    { title: 'CCB', value: 'ccb' },
    { title: 'ABC Credit', value: 'abc_credit' },
    { title: 'PSBC Credit', value: 'psbc_credit' },
    { title: 'SPDB Credit', value: 'spdb_credit' }
];

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
    selectedFileTypes.value = ['auto'];
    currentStep.value = 'uploadFile';
    importProcess.value = 0;
    importFiles.value = [];
    importData.value = '';
    parsedFileData.value = undefined;
    importTransactionDefineColumnTab.value?.reset();
    importTransactionExecuteCustomScriptTab.value?.reset();
    importTransactions.value = undefined;
    importTransactionCheckDataTab.value?.reset();
    showState.value = true;
    clientSessionId.value = generateRandomUUID();

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
    el.value = '';
}

function parseData(): void {
    if (importFiles.value.length === 0) {
        snackbar.value?.showError('Please select at least one file');
        return;
    }

    submitting.value = true;
    importProcess.value = 0;

    const parserType = selectedFileTypes.value.includes('auto') ? 'auto' : selectedFileTypes.value.join(',');
    
    const processNextFile = async (index: number, accumulatedTransactions: ImportTransaction[]) => {
        if (index >= importFiles.value.length) {
            // All files processed
            importTransactions.value = accumulatedTransactions;
            currentStep.value = 'checkData';
            submitting.value = false;
            return;
        }
        
        const file = importFiles.value[index];

        if (!file) {
            processNextFile(index + 1, accumulatedTransactions);
            return;
        }

        const formData = new FormData();
        formData.append('file', file);
        formData.append('parser_type', parserType);
        formData.append('preview_only', 'true');
        
        try {
            const response = await fetch('/api/bills/import/upload', {
                method: 'POST',
                body: formData
            });
            
            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(`Failed to upload ${file.name}: ${errorText}`);
            }
            
            const result = await response.json();
            if (result.success && result.data && result.data.preview) {
                const transactions = result.data.preview.map((item: any, idx: number) => {
                    // Map backend item to ImportTransactionResponse
                    // Backend item keys: time, type, amount, description, counterparty, main_category, sub_category, account
                    
                    const typeMap: Record<string, number> = { '支出': 1, '收入': 2, '转账': 3 };
                    const type = typeMap[item.type] || 1;
                    
                    // Parse time string to timestamp (seconds)
                    const time = new Date(item.time).getTime() / 1000;

                    const responseItem: ImportTransactionResponse = {
                        type: type,
                        categoryId: '',
                        originalCategoryName: item.sub_category || item.main_category || '',
                        time: isNaN(time) ? Date.now() / 1000 : time,
                        utcOffset: 0,
                        sourceAccountId: '',
                        originalSourceAccountName: item.account || '',
                        originalSourceAccountCurrency: 'CNY',
                        destinationAccountId: '',
                        sourceAmount: Math.abs(item.amount),
                        destinationAmount: Math.abs(item.amount),
                        tagIds: [],
                        originalTagNames: [],
                        comment: item.description || item.counterparty || ''
                    };
                    
                    return ImportTransaction.of(responseItem, accumulatedTransactions.length + idx);
                });
                
                accumulatedTransactions.push(...transactions);
            }
            
            processNextFile(index + 1, accumulatedTransactions);
            
        } catch (error) {
            console.error(error);
            snackbar.value?.showError(`Error processing ${file.name}: ${error}`);
            submitting.value = false;
        }
    };
    
    processNextFile(0, []);
}


function submit(): void {
    if (importTransactionCheckDataTab.value?.isEditing) {
        return;
    }

    const transactions: ImportTransaction[] = [];

    if (importTransactions.value) {
        for (const importTransaction of importTransactions.value) {
            if (importTransaction.valid && importTransaction.selected) {
                transactions.push(importTransaction);
            } else if (!importTransaction.valid && importTransaction.selected) {
                snackbar.value?.showError('Cannot import invalid transactions');
                return;
            }
        }
    }

    if (transactions.length < 1) {
        snackbar.value?.showError('No data to import');
        return;
    }

    confirmDialog.value?.open('format.misc.confirmImportTransactions', {
        count: getDisplayCount(transactions.length)
    }).then(() => {
        submitting.value = true;

        let showProcessTimer : number | undefined = undefined;

        if (transactions.length > 100) {
            setTimeout(() => {
                if (!submitting.value) {
                    logger.warn('transaction import is not submitting');
                    return;
                }

                // @ts-expect-error the return value of setInterval is number, but lint shows it as NodeJS.Timer
                showProcessTimer = setInterval(() => {
                    if (submitting.value) {
                        transactionsStore.getImportTransactionsProcess({
                            clientSessionId: clientSessionId.value
                        }).then(response => {
                            if (isNumber(response) && 0 <= response && response < 100) {
                                importProcess.value = response;
                            } else {
                                importProcess.value = 0;
                                clearInterval(showProcessTimer);
                                showProcessTimer = undefined;
                            }
                        }).catch(() => {
                            importProcess.value = 0;
                            clearInterval(showProcessTimer);
                            showProcessTimer = undefined;
                        });
                    }
                }, 2000);
            }, 2000);
        }

        transactionsStore.importTransactions({
            transactions: transactions,
            clientSessionId: clientSessionId.value
        }).then(response => {
            if (showProcessTimer) {
                importProcess.value = 0;
                clearInterval(showProcessTimer);
                showProcessTimer = undefined;
            }

            importedCount.value = response;
            currentStep.value = 'finalResult';

            accountsStore.updateAccountListInvalidState(true);
            transactionsStore.updateTransactionListInvalidState(true);
            overviewStore.updateTransactionOverviewInvalidState(true);
            statisticsStore.updateTransactionStatisticsInvalidState(true);

            submitting.value = false;
        }).catch(error => {
            if (showProcessTimer) {
                importProcess.value = 0;
                clearInterval(showProcessTimer);
                showProcessTimer = undefined;
            }

            submitting.value = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    });
}

function close(completed: boolean): void {
    if (completed) {
        if (resolveFunc) {
            resolveFunc();
        }
    } else {
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
