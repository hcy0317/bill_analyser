<template src="./edit-dialog/EditDialog.template.html"></template>

<script setup lang="ts">import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import MapView from '@/components/common/MapView.vue';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import BillMatchingPanel from './BillMatchingPanel.vue';
import TransactionPicturesPanel from './TransactionPicturesPanel.vue';

import { ref, computed, useTemplateRef, watch, nextTick, onMounted, onUnmounted } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import {
    TransactionEditPageMode,
    TransactionEditPageType,
    GeoLocationStatus,
    useTransactionEditPageBase
} from '@/views/base/transactions/TransactionEditPageBase.ts';

import { useSettingsStore } from '@/stores/setting.ts';
import { useUserStore } from '@/stores/user.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';
import { useTransactionTemplatesStore } from '@/stores/transactionTemplate.ts';

import type { Coordinate } from '@/core/coordinate.ts';
import { CategoryType } from '@/core/category.ts';
import { TransactionType, TransactionEditScopeType } from '@/core/transaction.ts';
import { TemplateType, ScheduledTemplateFrequencyType } from '@/core/template.ts';
import { KnownErrorCode } from '@/consts/api.ts';

import { TransactionTag } from '@/models/transaction_tag.ts';
import { TransactionTemplate } from '@/models/transaction_template.ts';
import type { TransactionPictureInfoBasicResponse } from '@/models/transaction_picture_info.ts';
import { Transaction } from '@/models/transaction.ts';
import type { RecognizedReceiptImageResponse, RecognizeReceiptImageError } from '@/models/large_language_model.ts';
import { RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD } from '@/models/large_language_model.ts';

import {
    getTimezoneOffsetMinutes,
    getCurrentUnixTime
} from '@/lib/datetime.ts';
import { categorizedArrayToPlainArray } from '@/lib/common.ts';
import { formatCoordinate } from '@/lib/coordinate.ts';
import { generateRandomUUID } from '@/lib/misc.ts';
import { getCurrentToken } from '@/lib/userstate.ts';
import {
    getTransactionPrimaryCategoryName,
    getTransactionSecondaryCategoryName,
    localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories
} from '@/lib/category.ts';
import { type SetTransactionOptions, setTransactionModelByTransaction } from '@/lib/transaction.ts';
import {
    type ReceiptDraftCandidateHint,
    applyReceiptDraftAutoFillToTransaction,
    applyReceiptDraftFieldToTransaction,
    buildReceiptDraftCandidateHints,
    getReceiptDraftCandidateDisplayValue
} from '@/lib/receiptDraft.ts';
import {
    createRecurringCandidateSubtitleFormatter,
    getRecurringCandidatePrimaryReason,
    type RecurringCandidateItem
} from './edit-dialog/recurringCandidateDisplay.ts';
import {
    isTransactionPicturesEnabled,
    getMapProvider
} from '@/lib/server_settings.ts';
import {
    isSupportGetGeoLocationByClick
} from '@/lib/map/index.ts';
import logger from '@/lib/logger.ts';

import {
    mdiDotsVertical,
    mdiEyeOffOutline,
    mdiEyeOutline,
    mdiSwapHorizontal,
    mdiMapMarkerOutline,
    mdiCheck,
    mdiPound,
    mdiMenuDown
} from '@mdi/js';

export interface TransactionEditOptions extends SetTransactionOptions {
    id?: string;
    templateType?: number;
    template?: TransactionTemplate;
    currentTransaction?: Transaction;
    currentTemplate?: TransactionTemplate;
    noTransactionDraft?: boolean;
}

interface TransactionEditResponse {
    message: string;
    deleted?: boolean;
}

type MapViewType = InstanceType<typeof MapView>;
type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;

const props = defineProps<{
    type: TransactionEditPageType;
    persistent?: boolean;
    show?: boolean;
}>();

const { tt, getCurrentLanguageTag, getAllTransactionDefaultCategories } = useI18n();
const formatRecurringCandidateSubtitle = createRecurringCandidateSubtitleFormatter(tt);

const {
    mode,
    isSupportGeoLocation,
    editId,
    addByTemplateId,
    duplicateFromId,
    clientSessionId,
    loading,
    submitting,
    uploadingPicture,
    geoLocationStatus,
    setGeoLocationByClickMap,
    transaction,
    defaultCurrency,
    defaultAccountId,
    coordinateDisplayType,
    allTimezones,
    allVisibleAccounts,
    allAccountsMap,
    allVisibleCategorizedAccounts,
    allCategories,
    allCategoriesMap,
    allTags,
    allTagsMap,
    firstVisibleAccountId,
    hasAvailableExpenseCategories,
    hasAvailableIncomeCategories,
    hasAvailableTransferCategories,
    hasAvailableInvestmentCategories,
    canAddTransactionPicture,
    title,
    saveButtonTitle,
    cancelButtonTitle,
    sourceAmountName,
    sourceAmountTitle,
    sourceAccountTitle,
    transferInAmountTitle,
    sourceAccountName,
    destinationAccountName,
    sourceAccountCurrency,
    destinationAccountCurrency,
    transactionDisplayTimezone,
    transactionTimezoneTimeDifference,
    geoLocationStatusInfo,
    inputEmptyProblemMessage,
    inputIsEmpty,
    createNewTransactionModel,
    swapTransactionData,
    getTransactionPictureUrl
} = useTransactionEditPageBase(props.type);

const settingsStore = useSettingsStore();
const userStore = useUserStore();
const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const transactionTagsStore = useTransactionTagsStore();
const transactionsStore = useTransactionsStore();
const transactionTemplatesStore = useTransactionTemplatesStore();

const map = useTemplateRef<MapViewType>('map');
const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');

const showState = ref<boolean>(false);
const activeTab = ref<string>('basicInfo');
const originalTransactionEditable = ref<boolean>(false);
const noTransactionDraft = ref<boolean>(false);
const geoMenuState = ref<boolean>(false);
const editableGeoMenuState = computed<boolean>({
    get: () => mode.value !== TransactionEditPageMode.View && geoMenuState.value,
    set: (value: boolean) => {
        geoMenuState.value = mode.value !== TransactionEditPageMode.View && value;
    }
});
const tagSearchContent = ref<string>('');
const removingPictureId = ref<string>('');
const recognizingPicture = ref<boolean>(false);
const receiptDraftCandidateHints = ref<ReceiptDraftCandidateHint[]>([]);
const addingDefaultCategories = ref<boolean>(false);
const loadingRecurringCandidates = ref<boolean>(false);
const recurringBindingSubmitting = ref<boolean>(false);
const showRecurringCandidateDialog = ref<boolean>(false);
const recurringCandidates = ref<RecurringCandidateItem[]>([]);
const selectedRecurringCandidateId = ref<string>('');
const linkedRecurringId = ref<string>('');
const linkedRecurringName = ref<string>('');
const recurringCandidateCount = ref<number>(0);

const initAmount = ref<number | undefined>(undefined);
const initCategoryId = ref<string | undefined>(undefined);
const initAccountId = ref<string | undefined>(undefined);
const initTagIds = ref<string | undefined>(undefined);

let resolveFunc: ((response?: TransactionEditResponse) => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const sourceAmountColor = computed<string | undefined>(() => {
    if (transaction.value.type === TransactionType.Expense) {
        return 'expense';
    } else if (transaction.value.type === TransactionType.Income) {
        return 'income';
    } else if (transaction.value.type === TransactionType.Transfer) {
        return 'primary';
    }

    return undefined;
});

const isAllFilteredTagHidden = computed<boolean>(() => {
    const lowerCaseTagSearchContent = tagSearchContent.value.toLowerCase();
    let hiddenCount = 0;

    for (const tag of allTags.value) {
        if (!lowerCaseTagSearchContent || tag.name.toLowerCase().indexOf(lowerCaseTagSearchContent) >= 0) {
            if (!tag.hidden) {
                return false;
            }

            hiddenCount++;
        }
    }

    return hiddenCount > 0;
});

const isTransactionModified = computed<boolean>(() => {
    if (mode.value === TransactionEditPageMode.Add) {
        return transactionsStore.isTransactionDraftModified(transaction.value, initAmount.value, initCategoryId.value, initAccountId.value, initTagIds.value, firstVisibleAccountId.value);
    } else if (mode.value === TransactionEditPageMode.Edit) {
        return true;
    } else {
        return false;
    }
});

const recurringMatchDisplayText = computed<string>(() => {
    if (linkedRecurringName.value) {
        return linkedRecurringName.value;
    }

    if (linkedRecurringId.value) {
        return `#${linkedRecurringId.value}`;
    }

    if (recurringCandidateCount.value > 0) {
        return `${tt('Scheduled Candidates')} ${recurringCandidateCount.value}`;
    }

    return tt('None');
});

const recurringMatchPrimaryReason = computed<string>(() => {
    const linkedCandidate = recurringCandidates.value.find(
        candidate => String(candidate.id) === linkedRecurringId.value
    );

    if (linkedCandidate) {
        return getRecurringCandidatePrimaryReason(linkedCandidate);
    }

    const bestCandidate = recurringCandidates.value[0];
    return bestCandidate ? getRecurringCandidatePrimaryReason(bestCandidate) : '';
});

function resetBillRecurringState(): void {
    loadingRecurringCandidates.value = false;
    recurringBindingSubmitting.value = false;
    showRecurringCandidateDialog.value = false;
    recurringCandidates.value = [];
    selectedRecurringCandidateId.value = '';
    linkedRecurringId.value = '';
    linkedRecurringName.value = '';
    recurringCandidateCount.value = 0;
}

function buildRecurringAuthHeaders(): Record<string, string> {
    const token = getCurrentToken();
    const headers: Record<string, string> = {};

    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }

    return headers;
}

function isBestRecurringCandidate(candidate: RecurringCandidateItem): boolean {
    const bestCandidate = recurringCandidates.value[0];
    if (!bestCandidate) {
        return false;
    }

    return String(bestCandidate.id) === String(candidate.id);
}

function showRecurringOperationError(error: unknown): void {
    if (error instanceof Error) {
        snackbar.value?.showError(error.message);
        return;
    }

    snackbar.value?.showError(String(error));
}

async function refreshBillRecurringCandidates(silent = false): Promise<void> { // 刷新当前账单可绑定的周期候选，并在非静默模式下反馈加载错误。
    if (props.type !== TransactionEditPageType.Transaction || !editId.value || mode.value === TransactionEditPageMode.Add) {
        resetBillRecurringState();
        return;
    }

    loadingRecurringCandidates.value = true;

    try {
        const response = await fetch(`/api/bills/${editId.value}/recurring-candidates?toleranceDays=3`, {
            method: 'GET',
            headers: buildRecurringAuthHeaders()
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Load recurring candidates failed: ${response.status} ${errorText}`);
        }

        const result = await response.json();
        if (!result.success) {
            throw new Error(result.error || 'Unknown error');
        }

        recurringCandidates.value = (result.result?.candidates || []) as RecurringCandidateItem[];
        recurringCandidateCount.value = recurringCandidates.value.length;
        linkedRecurringId.value = String(result.result?.linkedRecurringId || '');
        linkedRecurringName.value = String(result.result?.linkedRecurringName || '');

        if (linkedRecurringId.value && !linkedRecurringName.value) {
            const linkedCandidate = recurringCandidates.value.find(
                candidate => String(candidate.id) === linkedRecurringId.value
            );
            linkedRecurringName.value = linkedCandidate?.name || '';
        }

        selectedRecurringCandidateId.value = linkedRecurringId.value
            || (recurringCandidates.value[0] ? String(recurringCandidates.value[0].id) : '');
    } catch (error) {
        logger.error('failed to load recurring candidates for bill', error);
        if (!silent) {
            snackbar.value?.showMessage(tt('Load Scheduled Candidates Failed'));
        }
    } finally {
        loadingRecurringCandidates.value = false;
    }
}

async function openBillRecurringCandidateDialog(): Promise<void> {
    showRecurringCandidateDialog.value = true;
    await refreshBillRecurringCandidates();
}

function closeBillRecurringCandidateDialog(): void {
    showRecurringCandidateDialog.value = false;
}

async function applyBillRecurringCandidate(): Promise<void> { // 将用户选择的周期候选绑定到当前正式账单，并同步弹窗内周期状态。
    if (!editId.value || !selectedRecurringCandidateId.value) {
        return;
    }

    recurringBindingSubmitting.value = true;

    try {
        const response = await fetch(`/api/bills/${editId.value}/recurring-match`, {
            method: 'PUT',
            headers: {
                ...buildRecurringAuthHeaders(),
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ recurringId: selectedRecurringCandidateId.value })
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Bind recurring match failed: ${response.status} ${errorText}`);
        }

        const result = await response.json();
        if (!result.success) {
            throw new Error(result.error || 'Unknown error');
        }

        await refreshBillRecurringCandidates(true);
        closeBillRecurringCandidateDialog();
    } catch (error) {
        logger.error('failed to bind recurring match for bill', error);
        showRecurringOperationError(error);
    } finally {
        recurringBindingSubmitting.value = false;
    }
}

async function clearBillRecurringMatch(): Promise<void> { // 解除当前账单的周期绑定，并刷新候选列表以恢复可绑定状态。
    if (!editId.value || !linkedRecurringId.value) {
        return;
    }

    recurringBindingSubmitting.value = true;

    try {
        const response = await fetch(`/api/bills/${editId.value}/recurring-match`, {
            method: 'DELETE',
            headers: buildRecurringAuthHeaders()
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Clear recurring match failed: ${response.status} ${errorText}`);
        }

        const result = await response.json();
        if (!result.success) {
            throw new Error(result.error || 'Unknown error');
        }

        await refreshBillRecurringCandidates(true);
        closeBillRecurringCandidateDialog();
    } catch (error) {
        logger.error('failed to clear recurring match for bill', error);
        showRecurringOperationError(error);
    } finally {
        recurringBindingSubmitting.value = false;
    }
}

function clearBillRecurringMatchFromDialog(): void {
    clearBillRecurringMatch();
}

function setTransaction(newTransaction: Transaction | null, options: SetTransactionOptions, setContextData: boolean, convertContextTime: boolean): void { // 设置弹窗交易上下文，集中处理复制、只读、时间转换和背景展示状态。
    setTransactionModelByTransaction(
        transaction.value,
        newTransaction,
        allCategories.value,
        allCategoriesMap.value,
        allVisibleAccounts.value,
        allAccountsMap.value,
        allTagsMap.value,
        defaultAccountId.value,
        {
            time: options.time,
            type: options.type,
            categoryId: options.categoryId,
            accountId: options.accountId,
            destinationAccountId: options.destinationAccountId,
            sourceAmountCents: options.sourceAmountCents,
            destinationAmountCents: options.destinationAmountCents,
            tagIds: options.tagIds,
            comment: options.comment
        },
        setContextData,
        convertContextTime
    );
}

function open(options: TransactionEditOptions): Promise<TransactionEditResponse | undefined> { // 打开交易编辑弹窗，并用 Promise 向列表页返回保存、删除、编辑或取消结果。
    addByTemplateId.value = null;
    duplicateFromId.value = null;
    showState.value = true;
    activeTab.value = 'basicInfo';
    loading.value = true;
    submitting.value = false;
    geoLocationStatus.value = null;
    setGeoLocationByClickMap.value = false;
    originalTransactionEditable.value = false;
    noTransactionDraft.value = options.noTransactionDraft || false;
    receiptDraftCandidateHints.value = [];
    resetBillRecurringState();

    initAmount.value = options.sourceAmountCents;
    initCategoryId.value = options.categoryId;
    initAccountId.value = options.accountId;
    initTagIds.value = options.tagIds;

    const newTransaction = createNewTransactionModel(options.type);
    setTransaction(newTransaction, options, true, false);

    const promises: Promise<unknown>[] = [
        accountsStore.loadAllAccounts({ force: false }),
        transactionCategoriesStore.loadAllCategories({ force: false }),
        transactionTagsStore.loadAllTags({ force: true })  // 强制刷新标签列表，确保数据最新
    ];

    if (props.type === TransactionEditPageType.Transaction) {
        if (options && options.id) {
            if (options.currentTransaction) {
                setTransaction(options.currentTransaction, options, true, true);
            }

            mode.value = TransactionEditPageMode.View;
            editId.value = options.id;

            promises.push(transactionsStore.getTransaction({ transactionId: editId.value }));
        } else {
            mode.value = TransactionEditPageMode.Add;
            editId.value = null;

            if (options.template) {
                setTransaction(options.template, options, false, false);
                addByTemplateId.value = options.template.id;
            } else if (!options.noTransactionDraft && (settingsStore.appSettings.autoSaveTransactionDraft === 'enabled' || settingsStore.appSettings.autoSaveTransactionDraft === 'confirmation') && transactionsStore.transactionDraft) {
                setTransaction(Transaction.ofDraft(transactionsStore.transactionDraft), options, false, false);
            }

            if (settingsStore.appSettings.autoGetCurrentGeoLocation
                && !geoLocationStatus.value && !transaction.value.geoLocation) {
                updateGeoLocation(false);
            }
        }
    } else if (props.type === TransactionEditPageType.Template) {
        const template = TransactionTemplate.createNewTransactionTemplate(transaction.value);
        template.name = '';

        if (options && options.templateType) {
            template.templateType = options.templateType;
        }

        if (template.templateType === TemplateType.Schedule.type) {
            template.scheduledFrequencyType = ScheduledTemplateFrequencyType.Disabled.type;
            template.scheduledFrequency = '';
        }

        transaction.value = template;

        if (options && options.id) {
            if (options.currentTemplate) {
                setTransaction(options.currentTemplate, options, false, false);
                (transaction.value as TransactionTemplate).fillFrom(options.currentTemplate);
            }

            mode.value = TransactionEditPageMode.Edit;
            editId.value = options.id;
            transaction.value.id = options.id;

            promises.push(transactionTemplatesStore.getTemplate({
                templateId: editId.value,
                templateType: (transaction.value as TransactionTemplate).templateType
            }));
        } else {
            mode.value = TransactionEditPageMode.Add;
            editId.value = null;
            transaction.value.id = '';
        }
    }

    if (options.type &&
        options.type >= TransactionType.Income &&
        options.type <= TransactionType.Transfer) {
        transaction.value.type = options.type;
    }

    if (mode.value === TransactionEditPageMode.Add) {
        clientSessionId.value = generateRandomUUID();
    }

    Promise.all(promises).then(function (responses) {
        if (editId.value && !responses[3]) {
            if (rejectFunc) {
                if (props.type === TransactionEditPageType.Transaction) {
                    rejectFunc('Unable to retrieve transaction');
                } else if (props.type === TransactionEditPageType.Template) {
                    rejectFunc('Unable to retrieve template');
                }
            }

            return;
        }

        if (props.type === TransactionEditPageType.Transaction && options && options.id && responses[3] && responses[3] instanceof Transaction) {
            const transaction: Transaction = responses[3];

            // 添加日志追踪标签数据
            logger.debug(`[标签调试] 加载交易详情 ID=${transaction.id}, tagIds=${JSON.stringify(transaction.tagIds)}, tags数量=${transaction.tags?.length || 0}`);
            if (transaction.tags && transaction.tags.length > 0) {
                logger.debug(`[标签调试] tags内容: ${JSON.stringify(transaction.tags.map(t => ({id: t.id, idType: typeof t.id, name: t.name, nameType: typeof t.name})))}`);
            }
            logger.debug(`[标签调试] allTags数量=${allTags.value.length}, allTagsMap键数量=${Object.keys(allTagsMap.value).length}`);
            if (allTags.value.length > 0) {
                logger.debug(`[标签调试] allTags前5项: ${JSON.stringify(allTags.value.slice(0, 5).map(t => ({id: t.id, idType: typeof t.id, name: t.name, nameType: typeof t.name})))}`);
            }

            // 检查 tagIds 与 allTags 的匹配情况
            logger.debug(`[标签调试] transaction.tagIds类型: ${JSON.stringify(transaction.tagIds.map(id => ({id, type: typeof id})))}`);
            logger.debug(`[标签调试] allTags的id类型: ${JSON.stringify(allTags.value.map(t => ({id: t.id, type: typeof t.id})).slice(0, 5))}`);

            // 测试匹配
            for (const tagId of transaction.tagIds) {
                const foundInAllTags = allTags.value.find(t => t.id === tagId);
                logger.debug(`[标签调试] tagId=${tagId} (${typeof tagId}) 在allTags中匹配: ${foundInAllTags ? 'YES' : 'NO'}`);
                if (!foundInAllTags) {
                    const foundWithConversion = allTags.value.find(t => String(t.id) === String(tagId));
                    logger.debug(`[标签调试] 使用String()转换后匹配: ${foundWithConversion ? 'YES' : 'NO'}, 匹配到: ${foundWithConversion ? JSON.stringify({id: foundWithConversion.id, name: foundWithConversion.name}) : 'null'}`);
                }
            }

            // 【新增】添加分类调试日志
            logger.debug(`[分类调试] 加载交易详情 ID=${transaction.id}, type=${transaction.type}, categoryId=${transaction.categoryId}`);
            logger.debug(`[分类调试] transaction.category存在: ${!!transaction.category}`);
            if (transaction.category) {
                logger.debug(`[分类调试] transaction.category内容: ${JSON.stringify({id: transaction.category.id, name: transaction.category.name, type: transaction.category.type})}`);
            }
            logger.debug(`[分类调试] expenseCategoryId="${transaction.expenseCategoryId}", incomeCategoryId="${transaction.incomeCategoryId}", transferCategoryId="${transaction.transferCategoryId}", investmentCategoryId="${transaction.investmentCategoryId}"`);

            setTransaction(transaction, options, true, true);

            // 【修正】setTransaction后检查transaction.category (transaction是Transaction对象，不是ref)
            logger.debug(`[分类调试] setTransaction后 transaction.category存在: ${!!transaction.category}`);
            if (transaction.category) {
                logger.debug(`[分类调试] setTransaction后 transaction.category内容: ${JSON.stringify({id: transaction.category.id, name: transaction.category.name})}`);
            }

            originalTransactionEditable.value = transaction.editable;
            refreshBillRecurringCandidates(true);
        } else if (props.type === TransactionEditPageType.Template && options && options.id && responses[3] && responses[3] instanceof TransactionTemplate) {
            const template: TransactionTemplate = responses[3];
            setTransaction(template, options, false, false);

            if (!(transaction.value instanceof TransactionTemplate)) {
                transaction.value = TransactionTemplate.createNewTransactionTemplate(transaction.value);
            }

            (transaction.value as TransactionTemplate).fillFrom(template);
        } else {
            setTransaction(null, options, true, true);
        }

        loading.value = false;
    }).catch(error => {
        logger.error('failed to load essential data for editing transaction', error);

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

function save(): void { // 保存新增或编辑交易，统一处理标签新建、地理位置、周期状态和返回动作。
    const problemMessage = inputEmptyProblemMessage.value;

    if (problemMessage) {
        snackbar.value?.showMessage(problemMessage);
        return;
    }

    if (props.type === TransactionEditPageType.Transaction && (mode.value === TransactionEditPageMode.Add || mode.value === TransactionEditPageMode.Edit)) {
        const doSubmit = function () {
            submitting.value = true;

            transactionsStore.saveTransaction({
                transaction: transaction.value as Transaction,
                defaultCurrency: defaultCurrency.value,
                isEdit: mode.value === TransactionEditPageMode.Edit,
                clientSessionId: clientSessionId.value
            }).then(() => {
                submitting.value = false;

                if (resolveFunc) {
                    if (mode.value === TransactionEditPageMode.Add) {
                        resolveFunc({
                            message: 'You have added a new transaction'
                        });
                    } else if (mode.value === TransactionEditPageMode.Edit) {
                        resolveFunc({
                            message: 'You have saved this transaction'
                        });
                    }
                }

                if (mode.value === TransactionEditPageMode.Add && !noTransactionDraft.value && !addByTemplateId.value && !duplicateFromId.value) {
                    transactionsStore.clearTransactionDraft();
                }

                showState.value = false;
            }).catch(error => {
                submitting.value = false;

                if (error.error && (error.error.errorCode === KnownErrorCode.TransactionCannotCreateInThisTime || error.error.errorCode === KnownErrorCode.TransactionCannotModifyInThisTime)) {
                    confirmDialog.value?.open('You have set this time range to prevent editing transactions. Would you like to change the editable transaction range to All?').then(() => {
                        submitting.value = true;

                        userStore.updateUserTransactionEditScope({
                            transactionEditScope: TransactionEditScopeType.All.type
                        }).then(() => {
                            submitting.value = false;

                            snackbar.value?.showMessage('Your editable transaction range has been set to All');
                        }).catch(error => {
                            submitting.value = false;

                            if (!error.processed) {
                                snackbar.value?.showError(error);
                            }
                        });
                    });
                } else if (!error.processed) {
                    snackbar.value?.showError(error);
                }
            });
        };

        if (transaction.value.sourceAmountCents === 0) {
            confirmDialog.value?.open('Are you sure you want to save this transaction with a zero amount?').then(() => {
                doSubmit();
            });
        } else {
            doSubmit();
        }
    } else if (props.type === TransactionEditPageType.Template && (mode.value === TransactionEditPageMode.Add || mode.value === TransactionEditPageMode.Edit)) {
        submitting.value = true;

        transactionTemplatesStore.saveTemplateContent({
            template: transaction.value as TransactionTemplate,
            isEdit: mode.value === TransactionEditPageMode.Edit,
            clientSessionId: clientSessionId.value
        }).then(() => {
            submitting.value = false;

            if (resolveFunc) {
                if (mode.value === TransactionEditPageMode.Add) {
                    resolveFunc({
                        message: 'You have added a new template'
                    });
                } else if (mode.value === TransactionEditPageMode.Edit) {
                    resolveFunc({
                        message: 'You have saved this template'
                    });
                }
            }

            showState.value = false;
        }).catch(error => {
            submitting.value = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    }
}

async function addDefaultCategories(): Promise<boolean> {
    if (addingDefaultCategories.value) {
        return false;
    }

    const allPresetCategories = getAllTransactionDefaultCategories(0, getCurrentLanguageTag());
    const presetCategoriesArray = categorizedArrayToPlainArray(allPresetCategories);
    const submitCategories = localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories(presetCategoriesArray);

    if (!submitCategories.length) {
        snackbar.value?.showMessage('No available category');
        return false;
    }

    addingDefaultCategories.value = true;

    try {
        await transactionCategoriesStore.addPresetCategories({
            categories: submitCategories
        });

        await transactionCategoriesStore.loadAllCategories({ force: true });
        return true;
    } catch (error: any) {
        if (error && !error.processed) {
            snackbar.value?.showError(error);
        }

        return false;
    } finally {
        addingDefaultCategories.value = false;
    }
}

function onCategorySelectorClick(hasAvailableCategories: boolean): void {
    if (hasAvailableCategories || mode.value === TransactionEditPageMode.View || loading.value || submitting.value || addingDefaultCategories.value) {
        return;
    }

    confirmDialog.value?.open(`${tt('No available category')}. ${tt('Add Default Categories')}?`).then(async () => {
        const success = await addDefaultCategories();

        if (success) {
            snackbar.value?.showMessage('You have added preset categories');
        }
    }).catch(() => {
        // 用户取消时不处理
    });
}

function duplicate(withTime?: boolean, withGeoLocation?: boolean): void {
    if (props.type !== TransactionEditPageType.Transaction || mode.value !== TransactionEditPageMode.View) {
        return;
    }

    editId.value = null;
    duplicateFromId.value = transaction.value.id;
    clientSessionId.value = generateRandomUUID();
    activeTab.value = 'basicInfo';
    transaction.value.id = '';

    if (!withTime) {
        transaction.value.time = getCurrentUnixTime();
        transaction.value.timeZone = settingsStore.appSettings.timeZone;
        transaction.value.utcOffset = getTimezoneOffsetMinutes(transaction.value.timeZone);
    }

    if (!withGeoLocation) {
        transaction.value.removeGeoLocation();
    }

    transaction.value.clearPictures();
    mode.value = TransactionEditPageMode.Add;
}

function edit(): void {
    if (props.type !== TransactionEditPageType.Transaction || mode.value !== TransactionEditPageMode.View) {
        return;
    }

    mode.value = TransactionEditPageMode.Edit;
}

function remove(): void {
    if (props.type !== TransactionEditPageType.Transaction || mode.value !== TransactionEditPageMode.View) {
        return;
    }

    confirmDialog.value?.open('Are you sure you want to delete this transaction?').then(() => {
        submitting.value = true;

        transactionsStore.deleteTransaction({
            transaction: transaction.value as Transaction,
            defaultCurrency: defaultCurrency.value
        }).then(() => {
            submitting.value = false;

            // 触发列表刷新（标记为已删除）
            if (resolveFunc) {
                resolveFunc({ message: 'Transaction has been deleted successfully', deleted: true });
            }

            // 关闭详情页面
            showState.value = false;
        }).catch((error: any) => {
            submitting.value = false;

            if (error && !error.processed) {
                snackbar.value?.showError(error);
            }
        });
    });
}

function cancel(): void {
    const doClose = function () {
        if (rejectFunc) {
            rejectFunc();
        }

        showState.value = false;
    };

    if (props.type !== TransactionEditPageType.Transaction || mode.value !== TransactionEditPageMode.Add || noTransactionDraft.value || addByTemplateId.value || duplicateFromId.value) {
        doClose();
        return;
    }

    if (settingsStore.appSettings.autoSaveTransactionDraft === 'confirmation') {
        if (transactionsStore.isTransactionDraftModified(transaction.value, initAmount.value, initCategoryId.value, initAccountId.value, initTagIds.value, firstVisibleAccountId.value)) {
            confirmDialog.value?.open('Do you want to save this transaction draft?').then(() => {
                transactionsStore.saveTransactionDraft(transaction.value, initAmount.value, initCategoryId.value, initAccountId.value, initTagIds.value, firstVisibleAccountId.value);
                doClose();
            }).catch(() => {
                transactionsStore.clearTransactionDraft();
                doClose();
            });
        } else {
            transactionsStore.clearTransactionDraft();
            doClose();
        }
    } else if (settingsStore.appSettings.autoSaveTransactionDraft === 'enabled') {
        transactionsStore.saveTransactionDraft(transaction.value, initAmount.value, initCategoryId.value, initAccountId.value, initTagIds.value, firstVisibleAccountId.value);
        doClose();
    } else {
        doClose();
    }
}

function updateGeoLocation(forceUpdate: boolean): void { // 更新交易地理位置，支持强制刷新和浏览器定位失败提示。
    geoMenuState.value = false;

    if (!isSupportGeoLocation) {
        logger.warn('this browser does not support geo location');

        if (forceUpdate) {
            snackbar.value?.showMessage('Unable to retrieve current position');
        }
        return;
    }

    navigator.geolocation.getCurrentPosition(function (position) {
        if (!position || !position.coords) {
            logger.error('current position is null');
            geoLocationStatus.value = GeoLocationStatus.Error;

            if (forceUpdate) {
                snackbar.value?.showMessage('Unable to retrieve current position');
            }

            return;
        }

        geoLocationStatus.value = GeoLocationStatus.Success;

        transaction.value.setLatitudeAndLongitude(position.coords.latitude, position.coords.longitude);
    }, function (err) {
        logger.error('cannot retrieve current position', err);
        geoLocationStatus.value = GeoLocationStatus.Error;

        if (forceUpdate) {
            snackbar.value?.showMessage('Unable to retrieve current position');
        }
    });

    geoLocationStatus.value = GeoLocationStatus.Getting;
}

function updateSpecifiedGeoLocation(coordinate: Coordinate): void {
    if (mode.value === TransactionEditPageMode.View) {
        return;
    }

    if (isSupportGetGeoLocationByClick() && setGeoLocationByClickMap.value) {
        transaction.value.setLatitudeAndLongitude(coordinate.latitude, coordinate.longitude);
        map.value?.setMarkerPosition(transaction.value.geoLocation);
    }
}

function clearGeoLocation(): void {
    geoMenuState.value = false;
    geoLocationStatus.value = null;
    transaction.value.removeGeoLocation();
}

function saveNewTag(tagName: string): void {
    submitting.value = true;

    transactionTagsStore.saveTag({
        tag: TransactionTag.createNewTag(tagName)
    }).then(tag => {
        submitting.value = false;

        if (tag && tag.id) {
            transaction.value.tagIds.push(tag.id);

            // 【修复】刷新标签列表，确保新标签在v-autocomplete中正确显示名称
            transactionTagsStore.loadAllTags({ force: true }).then(() => {
                // 刷新成功，新标签现在可以正常显示
            }).catch(refreshError => {
                // 刷新失败不影响核心功能，只记录日志
                logger.warn('Failed to refresh tags after creating new tag:', refreshError);
            });
        }
    }).catch(error => {
        submitting.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function shouldRecognizeUploadedPicture(): boolean {
    return props.type === TransactionEditPageType.Transaction && mode.value === TransactionEditPageMode.Add;
}

function applyReceiptDraftCandidate(candidate: ReceiptDraftCandidateHint): void {
    if (!applyReceiptDraftFieldToTransaction(transaction.value, candidate.key, candidate.field)) {
        return;
    }

    receiptDraftCandidateHints.value = receiptDraftCandidateHints.value.filter(item => item.id !== candidate.id);
}

function applyReceiptRecognitionResult(result: RecognizedReceiptImageResponse): void { // 把图片 OCR 识别结果应用到交易草稿，保留候选证据并只覆盖可自动填充字段。
    receiptDraftCandidateHints.value = buildReceiptDraftCandidateHints(result.draft);
    applyReceiptDraftAutoFillToTransaction(transaction.value, result);

    if (result.confidence !== null && result.confidence < RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD) {
        snackbar.value?.showMessage('Low confidence recognition, please verify');
    }
}

function showReceiptRecognitionError(error: RecognizeReceiptImageError | unknown): void {
    const typed = error as RecognizeReceiptImageError;
    const errorCode = typed && typeof typed.errorCode === 'string' ? typed.errorCode : 'unknown';

    if (errorCode === 'provider_unconfigured') {
        snackbar.value?.showError('OCR recognition requires configuration in Rule Center');
        return;
    }

    if (errorCode === 'timeout') {
        snackbar.value?.showError('Recognition timed out, please try again');
        return;
    }

    if (errorCode === 'parse_error') {
        snackbar.value?.showError('Could not parse this image, please try a clearer one');
        return;
    }

    if (errorCode === 'rate_limited') {
        snackbar.value?.showError('Too many requests, please wait a moment');
        return;
    }

    snackbar.value?.showError('Unable to recognize image');
}

async function recognizeUploadedPicture(pictureFile: File): Promise<void> { // 上传后调用图片 OCR 识别，并将识别成功或失败状态回写到弹窗提示。
    if (!shouldRecognizeUploadedPicture()) {
        return;
    }

    recognizingPicture.value = true;
    try {
        const result = await transactionsStore.recognizeReceiptImage({ imageFile: pictureFile });
        applyReceiptRecognitionResult(result);
        activeTab.value = 'basicInfo';
        snackbar.value?.showMessage('Image recognized and filled');
    } catch (error) {
        showReceiptRecognitionError(error);
    } finally {
        recognizingPicture.value = false;
    }
}

async function uploadPicture(event: Event): Promise<void> { // 上传交易图片，成功后按配置触发 OCR，并维护当前图片列表。
    if (!event || !event.target) {
        return;
    }

    const el = event.target as HTMLInputElement;

    if (!el.files || !el.files.length || !el.files[0]) {
        return;
    }

    const pictureFile = el.files[0] as File;

    el.value = '';

    uploadingPicture.value = true;
    submitting.value = true;

    try {
        const response = await transactionsStore.uploadTransactionPicture({
            pictureFile,
            clientSessionId: clientSessionId.value
        });
        transaction.value.addPicture(response);
        uploadingPicture.value = false;

        await recognizeUploadedPicture(pictureFile);
    } catch (error: unknown) {
        const processed = !!(error && typeof error === 'object' && (error as { processed?: boolean }).processed);
        if (!processed) {
            snackbar.value?.showError('Unable to upload transaction picture');
        }
    } finally {
        uploadingPicture.value = false;
        submitting.value = false;
        recognizingPicture.value = false;
    }
}

function viewOrRemovePicture(pictureInfo: TransactionPictureInfoBasicResponse): void {
    if (mode.value !== TransactionEditPageMode.Add && mode.value !== TransactionEditPageMode.Edit) {
        window.open(getTransactionPictureUrl(pictureInfo), '_blank');
        return;
    }

    confirmDialog.value?.open('Are you sure you want to remove this transaction picture?').then(() => {
        removingPictureId.value = pictureInfo.pictureId;
        submitting.value = true;

        transactionsStore.removeUnusedTransactionPicture({ pictureInfo }).then(response => {
            if (response) {
                transaction.value.removePicture(pictureInfo);
            }

            removingPictureId.value = '';
            submitting.value = false;
        }).catch(error => {
            if (error.error && error.error.errorCode === KnownErrorCode.TransactionPictureNotFound) {
                transaction.value.removePicture(pictureInfo);
            } else if (!error.processed) {
                snackbar.value?.showError(error);
            }

            removingPictureId.value = '';
            submitting.value = false;
        });
    });
}

function onShowDateTimeError(error: string): void {
    snackbar.value?.showError(error);
}

function onBillMatchingNotify(message: string): void {
    snackbar.value?.showMessage(message);
}

function onBillMatchingError(message: string): void {
    snackbar.value?.showError(message);
}

async function onBillMatchingUpdated(): Promise<void> { // matching 面板操作后刷新账单和候选，确保弹窗展示最新配对状态。
    if (props.type !== TransactionEditPageType.Transaction || !editId.value || mode.value !== TransactionEditPageMode.View) {
        return;
    }

    loading.value = true;

    try {
        const latestTransaction = await transactionsStore.getTransaction({ transactionId: editId.value });

        if (latestTransaction instanceof Transaction) {
            setTransaction(latestTransaction, {}, true, true);
            originalTransactionEditable.value = latestTransaction.editable;
        }
    } catch (error) {
        logger.error('failed to refresh transaction after matching update', error);

        if (error instanceof Error) {
            snackbar.value?.showError(error.message);
        } else {
            snackbar.value?.showError('Failed to refresh transaction after matching update');
        }
    } finally {
        loading.value = false;
    }
}

watch(activeTab, (newValue) => {
    if (newValue === 'map') {
        nextTick(() => {
            map.value?.initMapView();
        });
    }
});

// 监听type变化：切换交易类型时重置分类选择
// 注意：只在对话框显示且非加载状态时触发，避免初始化时错误重置
watch(() => transaction.value.type, (newType, oldType) => {
    // 修复v6.21.4问题：仅在对话框已显示且未加载时才重置分类
    // 避免打开交易详情时触发watch导致分类被清空
    if (oldType !== undefined && newType !== oldType && showState.value && !loading.value) {
        // 清空所有分类ID
        transaction.value.expenseCategoryId = '';
        transaction.value.incomeCategoryId = '';
        transaction.value.transferCategoryId = '';
        transaction.value.investmentCategoryId = '';

        logger.info(`用户手动切换交易类型：${oldType} → ${newType}，已重置分类选择`);
    }
});

// Investment金额联动: 当源金额改变时，自动同步到目标金额
watch(() => transaction.value.sourceAmountCents, (newSourceAmount) => {
    if (transaction.value.type === TransactionType.Investment) {
        // Investment的源金额和目标金额应该一致
        transaction.value.destinationAmountCents = newSourceAmount;
    }
});

// Investment金额联动: 当目标金额改变时，自动同步到源金额
watch(() => transaction.value.destinationAmountCents, (newDestinationAmount) => {
    if (transaction.value.type === TransactionType.Investment) {
        // Investment的源金额和目标金额应该一致
        transaction.value.sourceAmountCents = newDestinationAmount;
    }
});

function onKeydown(e: KeyboardEvent): void {
    if (!showState.value) {
        return;
    }

    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
    }

    if (e.key === 'Enter') {
        if (mode.value === TransactionEditPageMode.Add || mode.value === TransactionEditPageMode.Edit) {
            save();
            e.preventDefault();
        }
    } else if (e.key === 'Backspace') {
        cancel();
        e.preventDefault();
    } else if (e.key === 'Delete') {
        if (mode.value === TransactionEditPageMode.View) {
            remove();
            e.preventDefault();
        }
    }
}

onMounted(() => {
    window.addEventListener('keydown', onKeydown);
});

onUnmounted(() => {
    window.removeEventListener('keydown', onKeydown);
});

defineExpose({
    open
});
useExternalTemplateBindings(accountsStore, activeTab, addByTemplateId, addDefaultCategories, addingDefaultCategories, allAccountsMap, allCategories, allCategoriesMap, allTags, allTagsMap, allTimezones, allVisibleAccounts, allVisibleCategorizedAccounts, applyBillRecurringCandidate, applyReceiptDraftAutoFillToTransaction, applyReceiptDraftCandidate, applyReceiptDraftFieldToTransaction, applyReceiptRecognitionResult, BillMatchingPanel, buildReceiptDraftCandidateHints, buildRecurringAuthHeaders, canAddTransactionPicture, cancel, cancelButtonTitle, categorizedArrayToPlainArray, CategoryType, clearBillRecurringMatch, clearBillRecurringMatchFromDialog, clearGeoLocation, clientSessionId, closeBillRecurringCandidateDialog, computed, confirmDialog, ConfirmDialog, coordinateDisplayType, createNewTransactionModel, createRecurringCandidateSubtitleFormatter, defaultAccountId, defaultCurrency, destinationAccountCurrency, destinationAccountName, duplicate, duplicateFromId, edit, editableGeoMenuState, editId, firstVisibleAccountId, formatCoordinate, formatRecurringCandidateSubtitle, generateRandomUUID, geoLocationStatus, GeoLocationStatus, geoLocationStatusInfo, geoMenuState, getAllTransactionDefaultCategories, getCurrentLanguageTag, getCurrentToken, getCurrentUnixTime, getMapProvider, getReceiptDraftCandidateDisplayValue, getRecurringCandidatePrimaryReason, getTimezoneOffsetMinutes, getTransactionPictureUrl, getTransactionPrimaryCategoryName, getTransactionSecondaryCategoryName, hasAvailableExpenseCategories, hasAvailableIncomeCategories, hasAvailableInvestmentCategories, hasAvailableTransferCategories, initAccountId, initAmount, initCategoryId, initTagIds, inputEmptyProblemMessage, inputIsEmpty, isAllFilteredTagHidden, isBestRecurringCandidate, isSupportGeoLocation, isSupportGetGeoLocationByClick, isTransactionModified, isTransactionPicturesEnabled, KnownErrorCode, linkedRecurringId, linkedRecurringName, loading, loadingRecurringCandidates, localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories, logger, map, MapView, mdiCheck, mdiDotsVertical, mdiEyeOffOutline, mdiEyeOutline, mdiMapMarkerOutline, mdiMenuDown, mdiPound, mdiSwapHorizontal, mode, nextTick, noTransactionDraft, onBillMatchingError, onBillMatchingNotify, onBillMatchingUpdated, onCategorySelectorClick, onKeydown, onMounted, onShowDateTimeError, onUnmounted, open, openBillRecurringCandidateDialog, originalTransactionEditable, props, RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD, receiptDraftCandidateHints, recognizeUploadedPicture, recognizingPicture, recurringBindingSubmitting, recurringCandidateCount, recurringCandidates, recurringMatchDisplayText, recurringMatchPrimaryReason, ref, refreshBillRecurringCandidates, rejectFunc, remove, removingPictureId, resetBillRecurringState, resolveFunc, save, saveButtonTitle, saveNewTag, ScheduledTemplateFrequencyType, selectedRecurringCandidateId, setGeoLocationByClickMap, settingsStore, setTransaction, setTransactionModelByTransaction, shouldRecognizeUploadedPicture, showReceiptRecognitionError, showRecurringCandidateDialog, showRecurringOperationError, showState, snackbar, SnackBar, sourceAccountCurrency, sourceAccountName, sourceAccountTitle, sourceAmountColor, sourceAmountName, sourceAmountTitle, submitting, swapTransactionData, tagSearchContent, TemplateType, title, transaction, Transaction, transactionCategoriesStore, transactionDisplayTimezone, TransactionEditPageMode, TransactionEditPageType, TransactionEditScopeType, TransactionPicturesPanel, transactionsStore, TransactionTag, transactionTagsStore, TransactionTemplate, transactionTemplatesStore, transactionTimezoneTimeDifference, TransactionType, transferInAmountTitle, tt, updateGeoLocation, updateSpecifiedGeoLocation, uploadingPicture, uploadPicture, useAccountsStore, useI18n, userStore, useSettingsStore, useTemplateRef, useTransactionCategoriesStore, useTransactionEditPageBase, useTransactionsStore, useTransactionTagsStore, useTransactionTemplatesStore, useUserStore, viewOrRemovePicture, watch);
</script>

<style src="./edit-dialog/EditDialog.css"></style>
