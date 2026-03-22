<template>
    <v-dialog width="1600" :persistent="submitting" v-model="showState">
        <v-card class="pa-2 pa-sm-4 pa-md-6">
            <template #title>
                <div class="d-flex align-center justify-space-between flex-wrap ga-3">
                    <h4 class="text-h4">{{ tt('Batch Manual Entry') }}</h4>
                    <div class="d-flex align-center ga-2 flex-wrap">
                        <v-chip color="warning" variant="tonal" v-if="invalidNonEmptyRowCount > 0">
                            {{ tt('Rows Need Attention') }}: {{ invalidNonEmptyRowCount }}
                        </v-chip>
                        <v-btn color="default" variant="outlined" :prepend-icon="mdiPlusCircleOutline" :disabled="submitting" @click="appendRow()">
                            {{ tt('Add Row') }}
                        </v-btn>
                    </div>
                </div>
            </template>

            <v-card-text class="mt-md-4">
                <div class="text-medium-emphasis mb-4">
                    {{ tt('Fill multiple transactions and save them at once') }}
                </div>

                <div class="batch-manual-entry-table-wrapper">
                    <v-table density="compact" fixed-header class="batch-manual-entry-table">
                        <thead>
                            <tr>
                                <th style="width: 72px">#</th>
                                <th style="width: 150px">{{ tt('Status') }}</th>
                                <th style="min-width: 220px">{{ tt('Time') }}</th>
                                <th style="min-width: 140px">{{ tt('Type') }}</th>
                                <th style="min-width: 240px">{{ tt('Category') }}</th>
                                <th style="min-width: 220px">{{ tt('Source Account') }}</th>
                                <th style="min-width: 190px">{{ tt('Amount') }}</th>
                                <th style="min-width: 220px">{{ tt('Destination Account') }}</th>
                                <th style="min-width: 210px">{{ tt('Destination Amount') }}</th>
                                <th style="min-width: 220px">{{ tt('Comment') }}</th>
                                <th style="min-width: 260px">{{ tt('Tags') }}</th>
                                <th style="min-width: 100px">{{ tt('Actions') }}</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="(row, index) in rows" :key="row.id">
                                <td class="text-medium-emphasis">{{ index + 1 }}</td>
                                <td>
                                    <div class="d-flex flex-column ga-2">
                                        <v-chip size="small" :color="getRowStatusColor(row)" variant="tonal">
                                            {{ getRowStatusLabel(row) }}
                                        </v-chip>
                                        <div class="text-warning text-caption" v-if="!isRowEmpty(row) && getRowIssues(row).length > 0">
                                            {{ getRowIssues(row).join(' / ') }}
                                        </div>
                                    </div>
                                </td>
                                <td>
                                    <date-time-select class="batch-manual-entry-input"
                                                      :disabled="submitting"
                                                      :label="tt('Time')"
                                                      v-model="row.transaction.time" />
                                </td>
                                <td>
                                    <v-select class="batch-manual-entry-input"
                                              density="comfortable"
                                              variant="outlined"
                                              hide-details
                                              :disabled="submitting"
                                              :items="transactionTypeOptions"
                                              item-title="label"
                                              item-value="value"
                                              v-model="row.transaction.type"
                                              @update:model-value="onTransactionTypeChanged(row.transaction)" />
                                </td>
                                <td>
                                    <v-select class="batch-manual-entry-input"
                                              density="comfortable"
                                              variant="outlined"
                                              hide-details
                                              :disabled="submitting"
                                              :items="getCategoryOptions(row.transaction.type)"
                                              item-title="label"
                                              item-value="id"
                                              v-model="currentCategoryIds[row.id]"
                                              @update:model-value="onCategoryChanged(row.transaction, $event)" />
                                </td>
                                <td>
                                    <v-select class="batch-manual-entry-input"
                                              density="comfortable"
                                              variant="outlined"
                                              hide-details
                                              :disabled="submitting"
                                              :items="accountOptions"
                                              item-title="label"
                                              item-value="id"
                                              v-model="row.transaction.sourceAccountId" />
                                </td>
                                <td>
                                    <amount-input class="batch-manual-entry-input"
                                                  density="comfortable"
                                                  variant="outlined"
                                                  :disabled="submitting"
                                                  :currency="getSourceCurrency(row.transaction)"
                                                  :show-currency="true"
                                                  :enable-formula="true"
                                                  v-model="row.transaction.sourceAmount" />
                                </td>
                                <td>
                                    <v-select v-if="requiresDestination(row.transaction)"
                                              class="batch-manual-entry-input"
                                              density="comfortable"
                                              variant="outlined"
                                              hide-details
                                              :disabled="submitting"
                                              :items="accountOptions"
                                              item-title="label"
                                              item-value="id"
                                              v-model="row.transaction.destinationAccountId" />
                                    <div v-else class="batch-manual-entry-empty-cell">—</div>
                                </td>
                                <td>
                                    <amount-input v-if="requiresDestination(row.transaction)"
                                                  class="batch-manual-entry-input"
                                                  density="comfortable"
                                                  variant="outlined"
                                                  :disabled="submitting"
                                                  :currency="getDestinationCurrency(row.transaction)"
                                                  :show-currency="true"
                                                  :enable-formula="true"
                                                  v-model="row.transaction.destinationAmount" />
                                    <div v-else class="batch-manual-entry-empty-cell">—</div>
                                </td>
                                <td>
                                    <v-text-field class="batch-manual-entry-input"
                                                  density="comfortable"
                                                  variant="outlined"
                                                  hide-details
                                                  :disabled="submitting"
                                                  v-model="row.transaction.comment" />
                                </td>
                                <td>
                                    <v-autocomplete class="batch-manual-entry-input"
                                                    item-title="name"
                                                    item-value="id"
                                                    multiple
                                                    chips
                                                    closable-chips
                                                    density="comfortable"
                                                    variant="outlined"
                                                    hide-details
                                                    :disabled="submitting"
                                                    :placeholder="tt('None')"
                                                    :items="tagOptions"
                                                    v-model="row.transaction.tagIds" />
                                </td>
                                <td>
                                    <div class="d-flex align-center justify-center ga-1">
                                        <v-btn icon variant="text" color="default" :disabled="submitting" size="small">
                                            <v-icon :icon="mdiDotsVertical" />
                                            <v-menu activator="parent">
                                                <v-list density="compact">
                                                    <v-list-item :title="tt('Duplicate')" :disabled="submitting" @click="duplicateRow(row)" />
                                                    <v-list-item :title="tt('Copy Previous Row')" :disabled="submitting || index < 1" @click="copyPreviousRow(index)" />
                                                    <v-list-item :title="tt('Apply Common Fields to All Rows')" :disabled="submitting || rows.length <= 1" @click="applyCommonFieldsToAll(index)" />
                                                    <v-list-item :title="tt('Fill Empty Fields in All Rows')" :disabled="submitting || rows.length <= 1" @click="fillEmptyFieldsToAll(index)" />
                                                    <v-list-item :title="tt('Apply Common Fields Down')" :disabled="submitting || index >= rows.length - 1" @click="applyCommonFieldsDown(index)" />
                                                    <v-list-item :title="tt('Fill Empty Fields Down')" :disabled="submitting || index >= rows.length - 1" @click="fillEmptyFieldsDown(index)" />
                                                    <v-divider />
                                                    <v-list-item :title="tt('Remove Row')" :disabled="submitting || rows.length <= 1" @click="removeRow(row.id)" />
                                                </v-list>
                                            </v-menu>
                                        </v-btn>
                                    </div>
                                </td>
                            </tr>
                        </tbody>
                    </v-table>
                </div>
            </v-card-text>

            <v-card-actions class="justify-center ga-4 pt-4 pb-2">
                <v-btn color="primary" :disabled="submitting" @click="submit">
                    {{ tt('Save All') }}
                    <v-progress-circular indeterminate size="20" class="ms-2" v-if="submitting" />
                </v-btn>
                <v-btn color="secondary" variant="tonal" :disabled="submitting" @click="cancel">
                    {{ tt('Cancel') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <snack-bar ref="snackbar" />
</template>

<script setup lang="ts">
import { computed, ref, useTemplateRef } from 'vue';

import AmountInput from '@/components/desktop/AmountInput.vue';
import DateTimeSelect from '@/components/desktop/DateTimeSelect.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';

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
import type { Account } from '@/models/account.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';

import { getCurrentUnixTime, getTimezoneOffsetMinutes } from '@/lib/datetime.ts';

import {
    mdiDotsVertical,
    mdiPlusCircleOutline
} from '@mdi/js';

interface BatchEntryRow {
    id: string;
    transaction: Transaction;
}

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

interface SelectOption {
    id: string;
    label: string;
}

type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();
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
const lastOpenOptions = ref<BatchManualEntryDialogOpenOptions | undefined>(undefined);

let resolveFunc: ((value: BatchManualEntryDialogResponse) => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const transactionTypeOptions = computed(() => ([
    { label: tt('Expense'), value: TransactionType.Expense },
    { label: tt('Income'), value: TransactionType.Income },
    { label: tt('Transfer'), value: TransactionType.Transfer },
    { label: tt('Investment'), value: TransactionType.Investment }
]));

const tagOptions = computed(() => transactionTagsStore.allVisibleTags || []);

const accountOptions = computed<SelectOption[]>(() => {
    return accountsStore.allVisiblePlainAccounts.map((account: Account) => ({
        id: account.id,
        label: account.name
    }));
});

const categoryOptionsByType = computed<Record<number, SelectOption[]>>(() => {
    const result: Record<number, SelectOption[]> = {};
    const allCategories = transactionCategoriesStore.allTransactionCategories || {};

    const supportedTypes = [
        CategoryType.Expense,
        CategoryType.Income,
        CategoryType.Transfer,
        CategoryType.Investment
    ];

    for (const type of supportedTypes) {
        const categories = allCategories[type] || [];
        const options: SelectOption[] = [];

        for (const primaryCategory of categories as TransactionCategory[]) {
            const subCategories = primaryCategory.subCategories || [];
            if (subCategories.length > 0) {
                for (const subCategory of subCategories) {
                    options.push({
                        id: subCategory.id,
                        label: `${primaryCategory.name} / ${subCategory.name}`
                    });
                }
            } else {
                options.push({
                    id: primaryCategory.id,
                    label: primaryCategory.name
                });
            }
        }

        result[type] = options;
    }

    return result;
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

function createTransactionRow(options?: BatchManualEntryDialogOpenOptions): BatchEntryRow {
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
        transaction.destinationAmount = transaction.sourceAmount;
    }

    const rowId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    currentCategoryIds.value[rowId] = transaction.getCategoryId();

    return { id: rowId, transaction };
}

function createRowFromTransaction(source: Transaction): BatchEntryRow {
    const timezone = settingsStore.appSettings.timeZone;
    const utcOffset = getTimezoneOffsetMinutes(timezone);
    const transaction = Transaction.createNewTransaction(source.type, source.time, timezone, utcOffset);

    transaction.sourceAccountId = source.sourceAccountId;
    transaction.destinationAccountId = source.destinationAccountId;
    transaction.sourceAmount = source.sourceAmount;
    transaction.destinationAmount = source.destinationAmount;
    transaction.hideAmount = source.hideAmount;
    transaction.tagIds = [...source.tagIds];
    transaction.comment = source.comment;
    transaction.setCategoryId(source.getCategoryId());

    const rowId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    currentCategoryIds.value[rowId] = transaction.getCategoryId();

    return { id: rowId, transaction };
}

function getCategoryOptions(transactionType: number): SelectOption[] {
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

    return categoryOptionsByType.value[categoryType] || [];
}

function requiresDestination(transaction: Transaction): boolean {
    return transaction.type === TransactionType.Transfer || transaction.type === TransactionType.Investment;
}

function onTransactionTypeChanged(transaction: Transaction): void {
    transaction.expenseCategoryId = '';
    transaction.incomeCategoryId = '';
    transaction.transferCategoryId = '';
    transaction.investmentCategoryId = '';

    if (!requiresDestination(transaction)) {
        transaction.destinationAccountId = '0';
        transaction.destinationAmount = 0;
    } else if (!transaction.destinationAmount && transaction.sourceAmount) {
        transaction.destinationAmount = transaction.sourceAmount;
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

function applyCommonFields(target: Transaction, source: Transaction, rowId: string): void {
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
        if (!target.destinationAmount || target.destinationAmount <= 0) {
            target.destinationAmount = target.sourceAmount || source.destinationAmount || source.sourceAmount;
        }
    } else {
        target.destinationAccountId = '0';
        target.destinationAmount = 0;
    }

    currentCategoryIds.value[rowId] = target.getCategoryId();
}

function fillEmptyCommonFields(target: Transaction, source: Transaction, rowId: string): void {
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

    if (!target.sourceAmount || target.sourceAmount <= 0) {
        target.sourceAmount = source.sourceAmount;
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
        if (!target.destinationAmount || target.destinationAmount <= 0) {
            target.destinationAmount = source.destinationAmount || source.sourceAmount;
        }
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
}

function isRowEmpty(row: BatchEntryRow): boolean {
    const transaction = row.transaction;
    return !transaction.getCategoryId()
        && !transaction.sourceAccountId
        && !transaction.destinationAccountId
        && !transaction.sourceAmount
        && !transaction.destinationAmount
    && (!transaction.tagIds || transaction.tagIds.length === 0)
        && !transaction.comment;
}

function isRowValid(row: BatchEntryRow): boolean {
    const transaction = row.transaction;
    if (!transaction.getCategoryId()) {
        return false;
    }
    if (!transaction.sourceAccountId || transaction.sourceAccountId === '0') {
        return false;
    }
    if (!transaction.sourceAmount || transaction.sourceAmount <= 0) {
        return false;
    }
    if (requiresDestination(transaction)) {
        if (!transaction.destinationAccountId || transaction.destinationAccountId === '0') {
            return false;
        }
        if (!transaction.destinationAmount || transaction.destinationAmount <= 0) {
            return false;
        }
    }

    return true;
}

function getRowIssues(row: BatchEntryRow): string[] {
    const transaction = row.transaction;
    const issues: string[] = [];

    if (!transaction.getCategoryId()) {
        issues.push(tt('Category'));
    }
    if (!transaction.sourceAccountId || transaction.sourceAccountId === '0') {
        issues.push(tt('Source Account'));
    }
    if (!transaction.sourceAmount || transaction.sourceAmount <= 0) {
        issues.push(tt('Amount'));
    }
    if (requiresDestination(transaction)) {
        if (!transaction.destinationAccountId || transaction.destinationAccountId === '0') {
            issues.push(tt('Destination Account'));
        }
        if (!transaction.destinationAmount || transaction.destinationAmount <= 0) {
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
    rows.value = [createTransactionRow(options)];
}

function open(options?: BatchManualEntryDialogOpenOptions): Promise<BatchManualEntryDialogResponse> {
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

function submit(): void {
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
</script>

<style scoped>
.batch-manual-entry-table-wrapper {
    max-height: 62vh;
    overflow: auto;
    border: 1px solid rgba(var(--v-theme-outline), 0.24);
    border-radius: 12px;
}

.batch-manual-entry-table :deep(th) {
    white-space: nowrap;
    background-color: rgb(var(--v-theme-surface));
    z-index: 1;
}

.batch-manual-entry-table :deep(td) {
    vertical-align: top;
}

.batch-manual-entry-input {
    min-width: 180px;
}

.batch-manual-entry-empty-cell {
    min-height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
    background-color: rgba(var(--v-theme-surface-variant), 0.45);
    border-radius: 8px;
}
</style>
