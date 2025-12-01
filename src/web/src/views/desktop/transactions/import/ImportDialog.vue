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

import { isNumber } from '@/lib/common.ts';
import { generateRandomUUID } from '@/lib/misc.ts';
import { getCurrentToken } from '@/lib/userstate.ts';
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
const settingsStore = useSettingsStore();

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const importTransactionDefineColumnTab = useTemplateRef<ImportTransactionDefineColumnTabType>('importTransactionDefineColumnTab');
const importTransactionExecuteCustomScriptTab = useTemplateRef<ImportTransactionExecuteCustomScriptTabType>('importTransactionExecuteCustomScriptTab');
const importTransactionCheckDataTab = useTemplateRef<ImportTransactionCheckDataTabType>('importTransactionCheckDataTab');
const fileInput = useTemplateRef<HTMLInputElement>('fileInput');

const showState = ref<boolean>(false);
const clientSessionId = ref<string>('');
const serverSessionId = ref<string>('');  // v6.48: 后端三阶段导入的会话ID
const currentStep = ref<ImportTransactionDialogStep>('uploadFile');
const importProcess = ref<number>(0);
const selectedFileTypes = ref<string[]>(['auto']);
const importFiles = ref<File[]>([]);
const importData = ref<string>('');
const parsedFileData = ref<string[][] | undefined>(undefined);
const importTransactions = ref<ImportTransaction[] | undefined>(undefined);
const dedupStats = ref<Record<string, number>>({});  // v6.48: 去重统计信息

const fileSubType = ref<string>('');
const processDSVMethod = ref<ImportDSVProcessMethod>(ImportDSVProcessMethod.ColumnMapping);

const allSteps = computed<StepBarItem[]>(() => [
    { name: 'uploadFile', title: tt('Select File'), subTitle: tt('Select the file to import') },
    { name: 'defineColumn', title: tt('Define Columns'), subTitle: tt('Map columns to fields') },
    { name: 'executeCustomScript', title: tt('Custom Script'), subTitle: tt('Execute Custom Script') },
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

/**
 * v6.48: 三阶段导入 - 解析并去重
 *
 * 阶段1: 上传所有文件到后端，写入 bills_parser_template 表
 * 阶段2: 执行去重处理，结果写入 bills_preview 表，返回预览数据
 */
async function parseData(): Promise<void> {
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

        const parserType = selectedFileTypes.value.includes('auto') ? 'auto' : selectedFileTypes.value.join(',');
        formData.append('parser_type', parserType);

        const token = getCurrentToken();
        const headers: Record<string, string> = {};
        if (token) {
            headers['Authorization'] = `Bearer ${token}`;
        }

        // 调用阶段1 API
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
        logger.info(`[三阶段导入-阶段1] 完成: success=${stage1Result.success}, ` +
                    `session_id=${stage1Result.data?.session_id}, ` +
                    `parsed_count=${stage1Result.data?.parsed_count}`);

        if (!stage1Result.success || !stage1Result.data?.session_id) {
            throw new Error(stage1Result.error || '解析失败：未获取到session_id');
        }

        serverSessionId.value = stage1Result.data.session_id;
        importProcess.value = 30;  // 更新进度

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
        logger.info(`[三阶段导入-阶段2] 完成: success=${stage2Result.success}, ` +
                    `total=${stage2Result.data?.total}, ` +
                    `after_dedup=${stage2Result.data?.after_dedup}, ` +
                    `preview_count=${stage2Result.data?.preview?.length || 0}`);

        if (!stage2Result.success) {
            throw new Error(stage2Result.error || '去重处理失败');
        }

        // 保存去重统计
        dedupStats.value = stage2Result.data?.dedup_stats || {};
        importProcess.value = 80;

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
        paymentMethod: item.preview_payment_method || ''
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
            // v6.48: 使用三阶段确认API
            if (serverSessionId.value) {
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

            } else {
                // 兼容旧的导入方式
                await legacySubmit(selectedTransactions);
            }

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

/**
 * 旧的导入提交方式（兼容）
 */
async function legacySubmit(transactions: ImportTransaction[]): Promise<void> {
    return new Promise((resolve, reject) => {
        let showProcessTimer: number | undefined = undefined;

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
            resolve();
        }).catch(error => {
            if (showProcessTimer) {
                importProcess.value = 0;
                clearInterval(showProcessTimer);
                showProcessTimer = undefined;
            }
            reject(error);
        });
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
