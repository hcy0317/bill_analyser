<template src="./batch-manual-entry-dialog/BatchManualEntryDialog.template.html"></template>

<script setup lang="ts">import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import { computed, nextTick, ref, useTemplateRef } from 'vue';

import AmountInput from '@/components/desktop/AmountInput.vue';
import DateTimeSelect from '@/components/desktop/DateTimeSelect.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import TwoColumnSelect from '@/components/desktop/TwoColumnSelect.vue';

import { useI18n } from '@/locales/helpers.ts';

import { useSettingsStore } from '@/stores/setting.ts';
import { useUserStore } from '@/stores/user.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';

import { CategoryType } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';

import { Transaction } from '@/models/transaction.ts';
import type { CategorizedAccountWithDisplayBalance } from '@/models/account.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';

import { getCurrentUnixTime, getTimezoneOffsetMinutes } from '@/lib/datetime.ts';
import {
    requiresDestination,
    shouldSyncDestinationAmount
} from './batch-manual-entry-dialog/destinationRules.ts';

import {
    mdiDotsVertical,
    mdiPlusCircleOutline
} from '@mdi/js';

interface BatchEntryRow {
    id: string;
    transaction: Transaction;
}

type BatchEntryFieldKey = 'time' | 'type' | 'category' | 'sourceAccount' | 'sourceAmountCents' | 'destinationAccount' | 'destinationAmountCents' | 'comment' | 'tags';

interface BatchManualEntryDialogOpenOptions {
    time?: number;
    type?: string | number;
    categoryId?: string;
    accountId?: string;
    tagIds?: string;
}

interface BatchManualEntryDialogResponse {
    message: string;
}

type SnackBarType = InstanceType<typeof SnackBar>;

const {
    tt,
    getCategorizedAccountsWithDisplayBalance
} = useI18n();
const settingsStore = useSettingsStore();
const userStore = useUserStore();
const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const transactionTagsStore = useTransactionTagsStore();
const transactionsStore = useTransactionsStore();

const snackbar = useTemplateRef<SnackBarType>('snackbar');

const showState = ref<boolean>(false);
const submitting = ref<boolean>(false);
const rows = ref<BatchEntryRow[]>([]);
const currentCategoryIds = ref<Record<string, string>>({});
const destinationAmountSyncState = ref<Record<string, boolean>>({});
const lastOpenOptions = ref<BatchManualEntryDialogOpenOptions | undefined>(undefined);

const BATCH_ENTRY_FIELD_ORDER: BatchEntryFieldKey[] = [
    'time',
    'type',
    'category',
    'sourceAccount',
    'sourceAmountCents',
    'destinationAccount',
    'destinationAmountCents',
    'comment',
    'tags'
];

let resolveFunc: ((value: BatchManualEntryDialogResponse) => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const transactionTypeOptions = computed(() => ([
    { label: tt('Expense'), value: TransactionType.Expense },
    { label: tt('Income'), value: TransactionType.Income },
    { label: tt('Transfer'), value: TransactionType.Transfer },
    { label: tt('Investment'), value: TransactionType.Investment }
]));

const tagOptions = computed(() => transactionTagsStore.allVisibleTags || []);

const categorizedAccountOptions = computed<CategorizedAccountWithDisplayBalance[]>(() => {
    return getCategorizedAccountsWithDisplayBalance(
        accountsStore.allVisiblePlainAccounts || [],
        settingsStore.appSettings.showAccountBalance
    );
});

const invalidNonEmptyRowCount = computed<number>(() => {
    return rows.value.filter(row => !isRowEmpty(row) && !isRowValid(row)).length;
});

function getInitialTransactionType(type?: string | number): number {
    const parsedType = Number(type || TransactionType.Expense);
    if ([TransactionType.Expense, TransactionType.Income, TransactionType.Transfer, TransactionType.Investment].includes(parsedType)) {
        return parsedType;
    }

    return TransactionType.Expense;
}

function createTransactionRow(options?: BatchManualEntryDialogOpenOptions): BatchEntryRow { // 创建批量录入空行，并应用入口传入的筛选上下文默认值。
    const timezone = settingsStore.appSettings.timeZone;
    const utcOffset = getTimezoneOffsetMinutes(timezone);
    const type = getInitialTransactionType(options?.type);
    const time = options?.time || getCurrentUnixTime();
    const transaction = Transaction.createNewTransaction(type, time, timezone, utcOffset);

    if (options?.accountId) {
        transaction.sourceAccountId = options.accountId;
    }
    if (options?.categoryId) {
        transaction.setCategoryId(options.categoryId);
    }
    if (options?.tagIds) {
        transaction.tagIds = options.tagIds.split(',').map(item => item.trim()).filter(item => !!item);
    }
    if (type === TransactionType.Transfer || type === TransactionType.Investment) {
        transaction.destinationAmountCents = transaction.sourceAmountCents;
    }

    const rowId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    currentCategoryIds.value[rowId] = transaction.getCategoryId();
    destinationAmountSyncState.value[rowId] = shouldSyncDestinationAmount(transaction);

    return { id: rowId, transaction };
}

function createRowFromTransaction(source: Transaction): BatchEntryRow { // 从已有行复制新行，保留可批量复用的交易字段并生成新的行 id。
    const timezone = settingsStore.appSettings.timeZone;
    const utcOffset = getTimezoneOffsetMinutes(timezone);
    const transaction = Transaction.createNewTransaction(source.type, source.time, timezone, utcOffset);

    transaction.sourceAccountId = source.sourceAccountId;
    transaction.destinationAccountId = source.destinationAccountId;
    transaction.sourceAmountCents = source.sourceAmountCents;
    transaction.destinationAmountCents = source.destinationAmountCents;
    transaction.hideAmount = source.hideAmount;
    transaction.tagIds = [...source.tagIds];
    transaction.comment = source.comment;
    transaction.setCategoryId(source.getCategoryId());

    const rowId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    currentCategoryIds.value[rowId] = transaction.getCategoryId();
    destinationAmountSyncState.value[rowId] = shouldSyncDestinationAmount(source);

    return { id: rowId, transaction };
}

function getCategoryItems(transactionType: number): TransactionCategory[] {
    const categoryTypeMap: Record<number, number> = {
        [TransactionType.Expense]: CategoryType.Expense,
        [TransactionType.Income]: CategoryType.Income,
        [TransactionType.Transfer]: CategoryType.Transfer,
        [TransactionType.Investment]: CategoryType.Investment
    };
    const categoryType = categoryTypeMap[transactionType];

    if (!categoryType) {
        return [];
    }

    return (transactionCategoriesStore.allTransactionCategories?.[categoryType] || []) as TransactionCategory[];
}

function setDestinationAmountSync(rowId: string, enabled: boolean): void {
    destinationAmountSyncState.value[rowId] = enabled;
}

function isDestinationAmountSync(rowId: string): boolean {
    return !!destinationAmountSyncState.value[rowId];
}

function onTransactionTypeChanged(row: BatchEntryRow): void { // 类型变化时重置不再适用的分类和目标字段，避免转账/投资残留污染普通交易。
    const transaction = row.transaction;
    transaction.expenseCategoryId = '';
    transaction.incomeCategoryId = '';
    transaction.transferCategoryId = '';
    transaction.investmentCategoryId = '';

    if (!requiresDestination(transaction)) {
        transaction.destinationAccountId = '0';
        transaction.destinationAmountCents = 0;
        setDestinationAmountSync(row.id, false);
    } else if (!transaction.destinationAmountCents && transaction.sourceAmountCents) {
        transaction.destinationAmountCents = transaction.sourceAmountCents;
        setDestinationAmountSync(row.id, true);
    } else {
        setDestinationAmountSync(row.id, shouldSyncDestinationAmount(transaction));
    }

    for (const row of rows.value) {
        if (row.transaction === transaction) {
            currentCategoryIds.value[row.id] = '';
            break;
        }
    }
}

function onCategoryChanged(transaction: Transaction, categoryId: string): void {
    transaction.setCategoryId(String(categoryId || ''));
}

function onSourceAmountChanged(row: BatchEntryRow): void {
    if (requiresDestination(row.transaction) && isDestinationAmountSync(row.id)) {
        row.transaction.destinationAmountCents = row.transaction.sourceAmountCents;
    }
}

function onDestinationAmountChanged(row: BatchEntryRow): void {
    if (!requiresDestination(row.transaction)) {
        setDestinationAmountSync(row.id, false);
        return;
    }

    setDestinationAmountSync(row.id, row.transaction.destinationAmountCents === row.transaction.sourceAmountCents);
}

function getDestinationAccountLabel(transaction: Transaction): string {
    return transaction.type === TransactionType.Investment ? 'Investment Account' : 'Destination Account';
}

function appendRow(options?: BatchManualEntryDialogOpenOptions): void {
    rows.value.push(createTransactionRow(options || lastOpenOptions.value));
}

function duplicateRow(row: BatchEntryRow): void {
    rows.value.push(createRowFromTransaction(row.transaction));
}

function copyPreviousRow(index: number): void {
    const previousRow = rows.value[index - 1];
    if (!previousRow) {
        return;
    }

    rows.value.splice(index + 1, 0, createRowFromTransaction(previousRow.transaction));
}

function applyCommonFields(target: Transaction, source: Transaction, rowId: string): void { // 将上一行常用字段强制覆盖到目标行，用于“向下应用”场景。
    target.type = source.type;
    target.expenseCategoryId = '';
    target.incomeCategoryId = '';
    target.transferCategoryId = '';
    target.investmentCategoryId = '';
    target.setCategoryId(source.getCategoryId());
    target.sourceAccountId = source.sourceAccountId;
    target.tagIds = [...source.tagIds];

    if (requiresDestination(source)) {
        target.destinationAccountId = source.destinationAccountId;
        if (!target.destinationAmountCents || target.destinationAmountCents <= 0) {
            target.destinationAmountCents = target.sourceAmountCents || source.destinationAmountCents || source.sourceAmountCents;
        }
        setDestinationAmountSync(rowId, shouldSyncDestinationAmount(target));
    } else {
        target.destinationAccountId = '0';
        target.destinationAmountCents = 0;
        setDestinationAmountSync(rowId, false);
    }

    currentCategoryIds.value[rowId] = target.getCategoryId();
}

function fillEmptyCommonFields(target: Transaction, source: Transaction, rowId: string): void { // 只填充目标行空字段，用于保留用户已输入内容的批量补全场景。
    if (!target.getCategoryId() && source.getCategoryId()) {
        target.type = source.type;
        target.expenseCategoryId = '';
        target.incomeCategoryId = '';
        target.transferCategoryId = '';
        target.investmentCategoryId = '';
        target.setCategoryId(source.getCategoryId());
    }

    if (!target.sourceAccountId || target.sourceAccountId === '0') {
        target.sourceAccountId = source.sourceAccountId;
    }

    if (!target.sourceAmountCents || target.sourceAmountCents <= 0) {
        target.sourceAmountCents = source.sourceAmountCents;
    }

    if ((!target.tagIds || target.tagIds.length === 0) && source.tagIds.length > 0) {
        target.tagIds = [...source.tagIds];
    }

    if (!target.comment && source.comment) {
        target.comment = source.comment;
    }

    if (requiresDestination(source)) {
        if (target.type !== source.type) {
            target.type = source.type;
        }
        if (!target.destinationAccountId || target.destinationAccountId === '0') {
            target.destinationAccountId = source.destinationAccountId;
        }
        if (!target.destinationAmountCents || target.destinationAmountCents <= 0) {
            target.destinationAmountCents = source.destinationAmountCents || source.sourceAmountCents;
        }
        setDestinationAmountSync(rowId, shouldSyncDestinationAmount(target));
    } else {
        setDestinationAmountSync(rowId, false);
    }

    currentCategoryIds.value[rowId] = target.getCategoryId();
}

function applyCommonFieldsDown(index: number): void {
    const sourceRow = rows.value[index];
    if (!sourceRow) {
        return;
    }

    for (let currentIndex = index + 1; currentIndex < rows.value.length; currentIndex++) {
        const currentRow = rows.value[currentIndex];
        if (!currentRow) {
            continue;
        }

        applyCommonFields(currentRow.transaction, sourceRow.transaction, currentRow.id);
    }
}

function applyCommonFieldsToAll(index: number): void {
    const sourceRow = rows.value[index];
    if (!sourceRow) {
        return;
    }

    for (const [currentIndex, currentRow] of rows.value.entries()) {
        if (currentIndex === index || !currentRow) {
            continue;
        }

        applyCommonFields(currentRow.transaction, sourceRow.transaction, currentRow.id);
    }
}

function fillEmptyFieldsDown(index: number): void {
    const sourceRow = rows.value[index];
    if (!sourceRow) {
        return;
    }

    for (let currentIndex = index + 1; currentIndex < rows.value.length; currentIndex++) {
        const currentRow = rows.value[currentIndex];
        if (!currentRow) {
            continue;
        }

        fillEmptyCommonFields(currentRow.transaction, sourceRow.transaction, currentRow.id);
    }
}

function fillEmptyFieldsToAll(index: number): void {
    const sourceRow = rows.value[index];
    if (!sourceRow) {
        return;
    }

    for (const [currentIndex, currentRow] of rows.value.entries()) {
        if (currentIndex === index || !currentRow) {
            continue;
        }

        fillEmptyCommonFields(currentRow.transaction, sourceRow.transaction, currentRow.id);
    }
}

function removeRow(rowId: string): void {
    rows.value = rows.value.filter(row => row.id !== rowId);
    delete currentCategoryIds.value[rowId];
    delete destinationAmountSyncState.value[rowId];
}

function isRowEmpty(row: BatchEntryRow): boolean {
    const transaction = row.transaction;
    return !transaction.getCategoryId()
        && !transaction.sourceAccountId
        && !transaction.destinationAccountId
        && !transaction.sourceAmountCents
        && !transaction.destinationAmountCents
    && (!transaction.tagIds || transaction.tagIds.length === 0)
        && !transaction.comment;
}

function isRowValid(row: BatchEntryRow): boolean { // 校验单行是否满足提交条件，覆盖时间、类型、金额、账户和目标账户要求。
    const transaction = row.transaction;
    if (!transaction.getCategoryId()) {
        return false;
    }
    if (!transaction.sourceAccountId || transaction.sourceAccountId === '0') {
        return false;
    }
    if (!transaction.sourceAmountCents || transaction.sourceAmountCents <= 0) {
        return false;
    }
    if (requiresDestination(transaction)) {
        if (!transaction.destinationAccountId || transaction.destinationAccountId === '0') {
            return false;
        }
        if (!transaction.destinationAmountCents || transaction.destinationAmountCents <= 0) {
            return false;
        }
    }

    return true;
}

function getRowIssues(row: BatchEntryRow): string[] { // 汇总单行校验问题，供状态列和提交前错误提示复用。
    const transaction = row.transaction;
    const issues: string[] = [];

    if (!transaction.getCategoryId()) {
        issues.push(tt('Category'));
    }
    if (!transaction.sourceAccountId || transaction.sourceAccountId === '0') {
        issues.push(tt('Source Account'));
    }
    if (!transaction.sourceAmountCents || transaction.sourceAmountCents <= 0) {
        issues.push(tt('Amount'));
    }
    if (requiresDestination(transaction)) {
        if (!transaction.destinationAccountId || transaction.destinationAccountId === '0') {
            issues.push(tt('Destination Account'));
        }
        if (!transaction.destinationAmountCents || transaction.destinationAmountCents <= 0) {
            issues.push(tt('Destination Amount'));
        }
    }

    return issues;
}

function getSourceCurrency(transaction: Transaction): string {
    return accountsStore.allAccountsMap[transaction.sourceAccountId]?.currency || userStore.currentUserDefaultCurrency || 'CNY';
}

function getDestinationCurrency(transaction: Transaction): string {
    return accountsStore.allAccountsMap[transaction.destinationAccountId]?.currency || userStore.currentUserDefaultCurrency || 'CNY';
}

function getAvailableFieldKeys(transaction: Transaction): BatchEntryFieldKey[] { // 根据交易类型返回键盘导航可进入的字段集合，隐藏不适用的目标字段。
    if (requiresDestination(transaction)) {
        return BATCH_ENTRY_FIELD_ORDER;
    }

    return BATCH_ENTRY_FIELD_ORDER.filter(field => field !== 'destinationAccount' && field !== 'destinationAmountCents');
}

function getFallbackFieldForTransaction(transaction: Transaction, fieldKey: BatchEntryFieldKey): BatchEntryFieldKey {
    if (requiresDestination(transaction)) {
        return fieldKey;
    }

    if (fieldKey === 'destinationAccount') {
        return 'sourceAccount';
    }

    if (fieldKey === 'destinationAmountCents') {
        return 'sourceAmountCents';
    }

    return fieldKey;
}

function shouldHandleGridNavigation(event: KeyboardEvent): boolean {
    if (event.altKey || event.ctrlKey || event.metaKey) {
        return false;
    }

    if (!['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.key)) {
        return false;
    }

    if (event.target instanceof HTMLElement) {
        if (event.target.closest('.v-overlay-container')) {
            return false;
        }

        const expanded = event.target.getAttribute('aria-expanded');
        if ((event.key === 'ArrowUp' || event.key === 'ArrowDown') && expanded === 'true') {
            return false;
        }
    }

    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
        const selectionStart = event.target.selectionStart ?? 0;
        const selectionEnd = event.target.selectionEnd ?? 0;
        const valueLength = event.target.value?.length ?? 0;

        if (event.key === 'ArrowLeft' && selectionStart > 0) {
            return false;
        }

        if (event.key === 'ArrowRight' && selectionEnd < valueLength) {
            return false;
        }
    }

    return true;
}

function focusBatchField(rowIndex: number, fieldKey: BatchEntryFieldKey): void { // 将焦点移动到批量表格指定单元格，供方向键和回车导航复用。
    nextTick(() => {
        window.setTimeout(() => {
            const cell = document.querySelector(`.batch-manual-entry-cell[data-batch-row="${rowIndex}"][data-batch-field="${fieldKey}"]`) as HTMLElement | null;

            if (!cell) {
                return;
            }

            const focusTarget = cell.querySelector('input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])') as HTMLElement | null;
            const clickTarget = cell.querySelector('.v-field, .v-input, .v-autocomplete, .v-select') as HTMLElement | null;

            if (focusTarget instanceof HTMLInputElement || focusTarget instanceof HTMLTextAreaElement) {
                focusTarget.focus();
                focusTarget.select?.();
                return;
            }

            (focusTarget || clickTarget || cell).focus?.();
            (clickTarget || cell).click?.();
        }, 30);
    });
}

function onCellKeydown(rowIndex: number, fieldKey: BatchEntryFieldKey, event: KeyboardEvent): void { // 处理批量录入表格键盘导航，保持 Excel 风格的横纵向移动体验。
    if (!shouldHandleGridNavigation(event)) {
        return;
    }

    const currentRow = rows.value[rowIndex];
    if (!currentRow) {
        return;
    }

    let targetRowIndex = rowIndex;
    let targetFieldKey = fieldKey;

    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
        const fields = getAvailableFieldKeys(currentRow.transaction);
        const currentFieldIndex = fields.indexOf(fieldKey);

        if (currentFieldIndex < 0) {
            return;
        }

        const nextFieldIndex = event.key === 'ArrowLeft' ? currentFieldIndex - 1 : currentFieldIndex + 1;
        if (nextFieldIndex < 0 || nextFieldIndex >= fields.length) {
            return;
        }

        targetFieldKey = fields[nextFieldIndex] as BatchEntryFieldKey;
    } else if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
        targetRowIndex = event.key === 'ArrowUp' ? rowIndex - 1 : rowIndex + 1;
        const targetRow = rows.value[targetRowIndex];

        if (!targetRow) {
            return;
        }

        targetFieldKey = getFallbackFieldForTransaction(targetRow.transaction, fieldKey);
    }

    event.preventDefault();
    event.stopPropagation();
    focusBatchField(targetRowIndex, targetFieldKey);
}

function getRowStatusLabel(row: BatchEntryRow): string {
    if (isRowEmpty(row)) {
        return tt('Empty Row');
    }

    if (isRowValid(row)) {
        return tt('Ready to Save');
    }

    return tt('Needs Required Fields');
}

function getRowStatusColor(row: BatchEntryRow): string {
    if (isRowEmpty(row)) {
        return 'default';
    }

    if (isRowValid(row)) {
        return 'success';
    }

    return 'warning';
}

function reset(options?: BatchManualEntryDialogOpenOptions): void {
    lastOpenOptions.value = options;
    currentCategoryIds.value = {};
    destinationAmountSyncState.value = {};
    rows.value = [createTransactionRow(options)];
}

function open(options?: BatchManualEntryDialogOpenOptions): Promise<BatchManualEntryDialogResponse> { // 打开批量录入弹窗并返回提交结果 Promise，供交易列表刷新和 snackbar 使用。
    submitting.value = false;
    reset(options);
    showState.value = true;
    accountsStore.loadAllAccounts({ force: false }).catch(() => undefined);
    transactionCategoriesStore.loadAllCategories({ force: false }).catch(() => undefined);
    transactionTagsStore.loadAllTags({ force: false }).catch(() => undefined);

    return new Promise((resolve, reject) => {
        resolveFunc = resolve;
        rejectFunc = reject;
    });
}

function close(): void {
    showState.value = false;
}

function cancel(): void {
    close();
    if (rejectFunc) {
        rejectFunc();
    }
}

function submit(): void { // 提交所有非空且校验通过的批量交易，并保留逐行错误提示。
    const nonEmptyRows = rows.value.filter(row => !isRowEmpty(row));

    if (nonEmptyRows.length === 0) {
        snackbar.value?.showError(tt('No valid transactions to save'));
        return;
    }

    const invalidRows = nonEmptyRows.filter(row => !isRowValid(row));
    if (invalidRows.length > 0) {
        snackbar.value?.showError(tt('Please complete required fields in all non-empty rows'));
        return;
    }

    submitting.value = true;

    transactionsStore.saveTransactions({
        transactions: nonEmptyRows.map(row => row.transaction),
        clientSessionId: `batch-manual-${Date.now()}`
    }).then(createdTransactions => {
        submitting.value = false;
        close();
        if (resolveFunc) {
            resolveFunc({
                message: tt('Batch created transactions successfully', { count: createdTransactions.length })
            });
        }
    }).catch(error => {
        submitting.value = false;
        if (error && !error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

defineExpose({
    open
});
useExternalTemplateBindings(accountsStore, AmountInput, appendRow, applyCommonFields, applyCommonFieldsDown, applyCommonFieldsToAll, BATCH_ENTRY_FIELD_ORDER, cancel, categorizedAccountOptions, CategoryType, close, computed, copyPreviousRow, createRowFromTransaction, createTransactionRow, currentCategoryIds, DateTimeSelect, destinationAmountSyncState, duplicateRow, fillEmptyCommonFields, fillEmptyFieldsDown, fillEmptyFieldsToAll, focusBatchField, getAvailableFieldKeys, getCategorizedAccountsWithDisplayBalance, getCategoryItems, getCurrentUnixTime, getDestinationAccountLabel, getDestinationCurrency, getFallbackFieldForTransaction, getInitialTransactionType, getRowIssues, getRowStatusColor, getRowStatusLabel, getSourceCurrency, getTimezoneOffsetMinutes, invalidNonEmptyRowCount, isDestinationAmountSync, isRowEmpty, isRowValid, lastOpenOptions, mdiDotsVertical, mdiPlusCircleOutline, nextTick, onCategoryChanged, onCellKeydown, onDestinationAmountChanged, onSourceAmountChanged, onTransactionTypeChanged, open, ref, rejectFunc, removeRow, requiresDestination, reset, resolveFunc, rows, setDestinationAmountSync, settingsStore, shouldHandleGridNavigation, shouldSyncDestinationAmount, showState, snackbar, SnackBar, submit, submitting, tagOptions, Transaction, transactionCategoriesStore, transactionsStore, transactionTagsStore, TransactionType, transactionTypeOptions, tt, TwoColumnSelect, useAccountsStore, useI18n, userStore, useSettingsStore, useTemplateRef, useTransactionCategoriesStore, useTransactionsStore, useTransactionTagsStore, useUserStore);
</script>

<style scoped src="./batch-manual-entry-dialog/BatchManualEntryDialog.css"></style>
