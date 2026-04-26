<template>
    <v-dialog width="1600" :persistent="submitting" v-model="showState">
        <v-card class="batch-manual-entry-dialog-card pa-2 pa-sm-4 pa-md-6 d-flex flex-column">
            <template #title>
                <div class="batch-manual-entry-title ga-3">
                    <div class="batch-manual-entry-title__meta d-flex align-center flex-wrap ga-2">
                        <v-chip color="warning" variant="tonal" v-if="invalidNonEmptyRowCount > 0">
                            {{ tt('Rows Need Attention') }}: {{ invalidNonEmptyRowCount }}
                        </v-chip>
                    </div>
                    <h4 class="batch-manual-entry-title__heading text-h4">{{ tt('Batch Add') }}</h4>
                    <div class="batch-manual-entry-title__actions d-flex justify-end align-center ga-2 flex-wrap">
                        <v-btn color="default" variant="outlined" :prepend-icon="mdiPlusCircleOutline" :disabled="submitting" @click="appendRow()">
                            {{ tt('Add Row') }}
                        </v-btn>
                    </div>
                </div>
            </template>

            <v-card-text class="batch-manual-entry-dialog-body mt-md-4 d-flex flex-column flex-grow-1 overflow-hidden">
                <div class="text-medium-emphasis mb-4">
                    {{ tt('Fill multiple transactions and save them at once') }}
                </div>

                <div class="batch-manual-entry-table-wrapper">
                    <v-table density="compact" fixed-header class="batch-manual-entry-table">
                        <thead>
                            <tr>
                                <th style="width: 56px">#</th>
                                <th style="width: 136px">{{ tt('Status') }}</th>
                                <th style="min-width: 188px">{{ tt('Transaction Time') }}</th>
                                <th style="min-width: 104px">{{ tt('Type') }}</th>
                                <th style="min-width: 220px">{{ tt('Category') }}</th>
                                <th style="min-width: 192px">{{ tt('Source Account') }}</th>
                                <th style="min-width: 208px">{{ tt('Amount') }}</th>
                                <th style="min-width: 192px">{{ tt('Destination Account') }}</th>
                                <th style="min-width: 208px">{{ tt('Destination Amount') }}</th>
                                <th style="min-width: 176px">{{ tt('Comment') }}</th>
                                <th style="min-width: 192px">{{ tt('Tags') }}</th>
                                <th style="width: 84px">{{ tt('Actions') }}</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="(row, index) in rows" :key="row.id">
                                <td class="batch-manual-entry-index text-medium-emphasis">{{ index + 1 }}</td>
                                <td>
                                    <div class="batch-manual-entry-status d-flex flex-column ga-2">
                                        <v-chip class="batch-manual-entry-status-chip" size="small" :color="getRowStatusColor(row)" variant="tonal">
                                            {{ getRowStatusLabel(row) }}
                                        </v-chip>
                                        <div class="text-warning text-caption" v-if="!isRowEmpty(row) && getRowIssues(row).length > 0">
                                            {{ getRowIssues(row).join(' / ') }}
                                        </div>
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--time"
                                         :data-batch-row="index"
                                         data-batch-field="time"
                                         @keydown.capture="onCellKeydown(index, 'time', $event)">
                                        <date-time-select class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--time"
                                                          :disabled="submitting"
                                                          :display-multiline="true"
                                                          :prefer-menu-on-top="true"
                                                          v-model="row.transaction.time" />
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--type"
                                         :data-batch-row="index"
                                         data-batch-field="type"
                                         @keydown.capture="onCellKeydown(index, 'type', $event)">
                                        <v-select class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--type"
                                                  density="compact"
                                                  variant="plain"
                                                  hide-details
                                                  :disabled="submitting"
                                                  :placeholder="tt('Type')"
                                                  :items="transactionTypeOptions"
                                                  item-title="label"
                                                  item-value="value"
                                                  v-model="row.transaction.type"
                                                  @update:model-value="onTransactionTypeChanged(row)" />
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--category"
                                         :data-batch-row="index"
                                         data-batch-field="category"
                                         @keydown.capture="onCellKeydown(index, 'category', $event)">
                                        <two-column-select class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--category"
                                                           primary-key-field="id"
                                                           primary-value-field="id"
                                                           primary-title-field="name"
                                                           primary-icon-field="icon"
                                                           primary-icon-type="category"
                                                           primary-color-field="color"
                                                           primary-hidden-field="hidden"
                                                           primary-sub-items-field="subCategories"
                                                           secondary-key-field="id"
                                                           secondary-value-field="id"
                                                           secondary-title-field="name"
                                                           secondary-icon-field="icon"
                                                           secondary-icon-type="category"
                                                           secondary-color-field="color"
                                                           secondary-hidden-field="hidden"
                                                           density="compact"
                                                           variant="plain"
                                                           :disabled="submitting"
                                                           :enable-filter="true"
                                                           :filter-placeholder="tt('Find category')"
                                                           :filter-no-items-text="tt('No available category')"
                                                           :show-selection-primary-text="true"
                                                           :placeholder="tt('Category')"
                                                           :items="getCategoryItems(row.transaction.type) as unknown as Record<string, unknown>[]"
                                                           v-model="currentCategoryIds[row.id]"
                                                           @update:model-value="onCategoryChanged(row.transaction, String($event || ''))" />
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--source-account"
                                         :data-batch-row="index"
                                         data-batch-field="sourceAccount"
                                         @keydown.capture="onCellKeydown(index, 'sourceAccount', $event)">
                                        <two-column-select class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--account"
                                                           primary-key-field="id"
                                                           primary-value-field="category"
                                                           primary-title-field="name"
                                                           primary-footer-field="displayBalance"
                                                           primary-icon-field="icon"
                                                           primary-icon-type="account"
                                                           primary-sub-items-field="accounts"
                                                           :primary-title-i18n="true"
                                                           secondary-key-field="id"
                                                           secondary-value-field="id"
                                                           secondary-title-field="name"
                                                           secondary-footer-field="displayBalance"
                                                           secondary-icon-field="icon"
                                                           secondary-icon-type="account"
                                                           secondary-color-field="color"
                                                           density="compact"
                                                           variant="plain"
                                                           :disabled="submitting"
                                                           :enable-filter="true"
                                                           :filter-placeholder="tt('Find account')"
                                                           :filter-no-items-text="tt('No available account')"
                                                           :show-selection-primary-text="true"
                                                           :placeholder="tt('Source Account')"
                                                           :items="categorizedAccountOptions as unknown as Record<string, unknown>[]"
                                                           v-model="row.transaction.sourceAccountId" />
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--source-amount"
                                         :data-batch-row="index"
                                         data-batch-field="sourceAmount"
                                         @keydown.capture="onCellKeydown(index, 'sourceAmount', $event)">
                                        <amount-input class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--amount"
                                                      density="compact"
                                                      variant="plain"
                                                      :disabled="submitting"
                                                      :currency="getSourceCurrency(row.transaction)"
                                                      :show-currency="true"
                                                      :compact-currency-display="true"
                                                      :enable-formula="true"
                                                      :placeholder="tt('Amount')"
                                                      v-model="row.transaction.sourceAmount"
                                                      @update:model-value="onSourceAmountChanged(row)" />
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--destination-account"
                                         :data-batch-row="index"
                                         data-batch-field="destinationAccount"
                                         @keydown.capture="onCellKeydown(index, 'destinationAccount', $event)">
                                        <two-column-select v-if="requiresDestination(row.transaction)"
                                                           class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--account"
                                                           primary-key-field="id"
                                                           primary-value-field="category"
                                                           primary-title-field="name"
                                                           primary-footer-field="displayBalance"
                                                           primary-icon-field="icon"
                                                           primary-icon-type="account"
                                                           primary-sub-items-field="accounts"
                                                           :primary-title-i18n="true"
                                                           secondary-key-field="id"
                                                           secondary-value-field="id"
                                                           secondary-title-field="name"
                                                           secondary-footer-field="displayBalance"
                                                           secondary-icon-field="icon"
                                                           secondary-icon-type="account"
                                                           secondary-color-field="color"
                                                           density="compact"
                                                           variant="plain"
                                                           :disabled="submitting"
                                                           :enable-filter="true"
                                                           :filter-placeholder="tt('Find account')"
                                                           :filter-no-items-text="tt('No available account')"
                                                           :show-selection-primary-text="true"
                                                           :placeholder="tt(getDestinationAccountLabel(row.transaction))"
                                                           :items="categorizedAccountOptions as unknown as Record<string, unknown>[]"
                                                           v-model="row.transaction.destinationAccountId" />
                                        <div v-else class="batch-manual-entry-empty-cell">—</div>
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--destination-amount"
                                         :data-batch-row="index"
                                         data-batch-field="destinationAmount"
                                         @keydown.capture="onCellKeydown(index, 'destinationAmount', $event)">
                                        <amount-input v-if="requiresDestination(row.transaction)"
                                                      class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--amount"
                                                      density="compact"
                                                      variant="plain"
                                                      :disabled="submitting"
                                                      :currency="getDestinationCurrency(row.transaction)"
                                                      :show-currency="true"
                                                      :compact-currency-display="true"
                                                      :enable-formula="true"
                                                      :placeholder="tt('Destination Amount')"
                                                      v-model="row.transaction.destinationAmount"
                                                      @update:model-value="onDestinationAmountChanged(row)" />
                                        <div v-else class="batch-manual-entry-empty-cell">—</div>
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--comment"
                                         :data-batch-row="index"
                                         data-batch-field="comment"
                                         @keydown.capture="onCellKeydown(index, 'comment', $event)">
                                        <v-text-field class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--comment"
                                                      density="compact"
                                                      variant="plain"
                                                      hide-details
                                                      :disabled="submitting"
                                                      :placeholder="tt('Comment')"
                                                      v-model="row.transaction.comment" />
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-cell batch-manual-entry-cell--tags"
                                         :data-batch-row="index"
                                         data-batch-field="tags"
                                         @keydown.capture="onCellKeydown(index, 'tags', $event)">
                                        <v-autocomplete class="batch-manual-entry-input batch-manual-entry-input--excel batch-manual-entry-input--tags"
                                                        item-title="name"
                                                        item-value="id"
                                                        multiple
                                                        chips
                                                        closable-chips
                                                        density="compact"
                                                        variant="plain"
                                                        hide-details
                                                        :disabled="submitting"
                                                        :placeholder="tt('None')"
                                                        :items="tagOptions"
                                                        v-model="row.transaction.tagIds" />
                                    </div>
                                </td>
                                <td>
                                    <div class="batch-manual-entry-actions d-flex align-center justify-center ga-0">
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
    mdiDotsVertical,
    mdiPlusCircleOutline
} from '@mdi/js';

interface BatchEntryRow {
    id: string;
    transaction: Transaction;
}

type BatchEntryFieldKey = 'time' | 'type' | 'category' | 'sourceAccount' | 'sourceAmount' | 'destinationAccount' | 'destinationAmount' | 'comment' | 'tags';

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
    'sourceAmount',
    'destinationAccount',
    'destinationAmount',
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
    destinationAmountSyncState.value[rowId] = shouldSyncDestinationAmount(transaction);

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

function requiresDestination(transaction: Transaction): boolean {
    return transaction.type === TransactionType.Transfer || transaction.type === TransactionType.Investment;
}

function shouldSyncDestinationAmount(transaction: Transaction): boolean {
    return requiresDestination(transaction)
        && (!transaction.destinationAmount || transaction.destinationAmount === transaction.sourceAmount);
}

function setDestinationAmountSync(rowId: string, enabled: boolean): void {
    destinationAmountSyncState.value[rowId] = enabled;
}

function isDestinationAmountSync(rowId: string): boolean {
    return !!destinationAmountSyncState.value[rowId];
}

function onTransactionTypeChanged(row: BatchEntryRow): void {
    const transaction = row.transaction;
    transaction.expenseCategoryId = '';
    transaction.incomeCategoryId = '';
    transaction.transferCategoryId = '';
    transaction.investmentCategoryId = '';

    if (!requiresDestination(transaction)) {
        transaction.destinationAccountId = '0';
        transaction.destinationAmount = 0;
        setDestinationAmountSync(row.id, false);
    } else if (!transaction.destinationAmount && transaction.sourceAmount) {
        transaction.destinationAmount = transaction.sourceAmount;
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
        row.transaction.destinationAmount = row.transaction.sourceAmount;
    }
}

function onDestinationAmountChanged(row: BatchEntryRow): void {
    if (!requiresDestination(row.transaction)) {
        setDestinationAmountSync(row.id, false);
        return;
    }

    setDestinationAmountSync(row.id, row.transaction.destinationAmount === row.transaction.sourceAmount);
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
        setDestinationAmountSync(rowId, shouldSyncDestinationAmount(target));
    } else {
        target.destinationAccountId = '0';
        target.destinationAmount = 0;
        setDestinationAmountSync(rowId, false);
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

function getAvailableFieldKeys(transaction: Transaction): BatchEntryFieldKey[] {
    if (requiresDestination(transaction)) {
        return BATCH_ENTRY_FIELD_ORDER;
    }

    return BATCH_ENTRY_FIELD_ORDER.filter(field => field !== 'destinationAccount' && field !== 'destinationAmount');
}

function getFallbackFieldForTransaction(transaction: Transaction, fieldKey: BatchEntryFieldKey): BatchEntryFieldKey {
    if (requiresDestination(transaction)) {
        return fieldKey;
    }

    if (fieldKey === 'destinationAccount') {
        return 'sourceAccount';
    }

    if (fieldKey === 'destinationAmount') {
        return 'sourceAmount';
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

function focusBatchField(rowIndex: number, fieldKey: BatchEntryFieldKey): void {
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

function onCellKeydown(rowIndex: number, fieldKey: BatchEntryFieldKey, event: KeyboardEvent): void {
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
.batch-manual-entry-dialog-card {
    height: min(88vh, 980px);
    min-height: min(88vh, 980px);
    max-height: min(88vh, 980px);
}

.batch-manual-entry-dialog-body {
    min-height: 0;
}

.batch-manual-entry-title {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
}

.batch-manual-entry-title__meta {
    min-width: 0;
}

.batch-manual-entry-title__heading {
    margin: 0;
    text-align: center;
}

.batch-manual-entry-title__actions {
    min-width: 0;
}

.batch-manual-entry-table-wrapper {
    flex: 1 1 auto;
    min-height: 0;
    max-height: 62vh;
    overflow: auto;
    border: 1px solid rgba(var(--v-theme-outline), 0.24);
    border-radius: 8px;
}

.batch-manual-entry-table :deep(table) {
    min-width: 100%;
    border-collapse: collapse;
}

.batch-manual-entry-table :deep(th) {
    white-space: nowrap;
    background-color: rgb(var(--v-theme-surface));
    z-index: 1;
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    padding: 8px 10px;
    border-right: 1px solid rgba(var(--v-theme-outline), 0.3);
    border-bottom: 1px solid rgba(var(--v-theme-outline), 0.3);
}

.batch-manual-entry-table :deep(td) {
    vertical-align: top;
    padding: 0;
    border-right: 1px solid rgba(var(--v-theme-outline), 0.22);
    border-bottom: 1px solid rgba(var(--v-theme-outline), 0.12);
}

.batch-manual-entry-input {
    min-width: 128px;
}

.batch-manual-entry-input--category {
    min-width: 260px;
}

.batch-manual-entry-input--time {
    min-width: 168px;
}

.batch-manual-entry-input--account {
    min-width: 260px;
}

.batch-manual-entry-input--amount {
    min-width: 192px;
}

.batch-manual-entry-input--comment,
.batch-manual-entry-input--tags {
    min-width: 160px;
}

.batch-manual-entry-cell {
    min-height: 38px;
    padding: 2px 8px;
    background-color: rgb(var(--v-theme-surface));
}

.batch-manual-entry-status {
    align-items: flex-start;
    padding: 8px;
}

.batch-manual-entry-index {
    vertical-align: middle !important;
    text-align: center;
    padding: 8px 6px !important;
    font-weight: 500;
}

.batch-manual-entry-status-chip {
    min-width: 108px;
    justify-content: center;
    text-align: center;
    white-space: nowrap;
}

.batch-manual-entry-input--excel {
    width: 100%;
}

.batch-manual-entry-cell :deep(.v-input) {
    font-size: 0.875rem;
}

.batch-manual-entry-cell :deep(.v-input__details) {
    display: none;
}

.batch-manual-entry-cell :deep(.v-field) {
    border-radius: 0;
    box-shadow: none;
    background: transparent;
}

.batch-manual-entry-cell :deep(.v-field__overlay) {
    background: transparent;
}

.batch-manual-entry-cell :deep(.v-field__outline) {
    display: none;
}

.batch-manual-entry-cell :deep(.v-field__input) {
    min-height: 32px;
    padding-top: 4px;
    padding-bottom: 4px;
}

.batch-manual-entry-cell--time :deep(.date-time-select-display--multiline) {
    display: inline-flex;
    flex-direction: column;
    align-items: flex-start;
    line-height: 1.15;
    white-space: normal;
    width: 100%;
}

.batch-manual-entry-cell--time :deep(.date-time-select-display__line) {
    display: block;
    white-space: nowrap;
}

.batch-manual-entry-cell--category,
.batch-manual-entry-cell--source-account,
.batch-manual-entry-cell--destination-account {
    width: max-content;
    min-width: 0;
}

.batch-manual-entry-cell--category :deep(.text-truncate),
.batch-manual-entry-cell--category :deep(.v-select__selection-text),
.batch-manual-entry-cell--source-account :deep(.text-truncate),
.batch-manual-entry-cell--source-account :deep(.v-select__selection-text),
.batch-manual-entry-cell--destination-account :deep(.text-truncate),
.batch-manual-entry-cell--destination-account :deep(.v-select__selection-text) {
    overflow: visible;
    text-overflow: clip;
    white-space: nowrap;
    max-width: none;
}

.batch-manual-entry-cell--category :deep(.v-field__input),
.batch-manual-entry-cell--source-account :deep(.v-field__input),
.batch-manual-entry-cell--destination-account :deep(.v-field__input) {
    width: max-content;
    min-width: 100%;
}

.batch-manual-entry-cell :deep(.v-field__prepend-inner),
.batch-manual-entry-cell :deep(.v-field__append-inner) {
    padding-top: 4px;
}

.batch-manual-entry-cell--source-amount,
.batch-manual-entry-cell--destination-amount {
    display: flex;
    align-items: center;
}

.batch-manual-entry-cell--source-amount :deep(.v-input),
.batch-manual-entry-cell--destination-amount :deep(.v-input) {
    width: 100%;
}

.batch-manual-entry-cell--source-amount :deep(.v-field),
.batch-manual-entry-cell--destination-amount :deep(.v-field) {
    align-items: center;
}

.batch-manual-entry-cell--source-amount :deep(.text-field-with-colored-label.has-pretend-text .v-field),
.batch-manual-entry-cell--destination-amount :deep(.text-field-with-colored-label.has-pretend-text .v-field) {
    grid-template-columns: max-content minmax(0, 1fr) max-content max-content;
}

.batch-manual-entry-cell--source-amount :deep(.v-field__input),
.batch-manual-entry-cell--destination-amount :deep(.v-field__input) {
    align-items: center;
    white-space: nowrap;
    padding-inline-start: 2px;
    padding-inline-end: 2px;
}

.batch-manual-entry-cell--source-amount :deep(.v-field__prepend-inner),
.batch-manual-entry-cell--source-amount :deep(.v-field__append-inner),
.batch-manual-entry-cell--destination-amount :deep(.v-field__prepend-inner),
.batch-manual-entry-cell--destination-amount :deep(.v-field__append-inner) {
    display: flex;
    align-items: center;
    align-self: center;
    height: 100%;
    padding-top: 0;
    padding-inline-start: 2px;
    padding-inline-end: 2px;
}

.batch-manual-entry-cell--source-amount :deep(.v-field__append-inner .v-icon),
.batch-manual-entry-cell--destination-amount :deep(.v-field__append-inner .v-icon) {
    margin-top: 0;
    margin-bottom: 0;
}

.batch-manual-entry-cell--source-amount :deep(.text-field-with-colored-label.has-pretend-text .v-field__input),
.batch-manual-entry-cell--destination-amount :deep(.text-field-with-colored-label.has-pretend-text .v-field__input) {
    padding-inline-start: 0.15rem;
}

.batch-manual-entry-cell--source-amount :deep(.v-field__append-inner),
.batch-manual-entry-cell--destination-amount :deep(.v-field__append-inner) {
    column-gap: 2px;
}

.batch-manual-entry-cell--source-amount :deep(input),
.batch-manual-entry-cell--destination-amount :deep(input) {
    text-align: right;
}

.batch-manual-entry-cell--destination-amount .batch-manual-entry-empty-cell {
    width: 100%;
    min-height: 32px;
    margin: 0;
}

.batch-manual-entry-cell--destination-account .batch-manual-entry-empty-cell {
    width: 100%;
    min-height: 32px;
    margin: 0;
}

.batch-manual-entry-cell :deep(.v-select__selection-text),
.batch-manual-entry-cell :deep(.v-autocomplete__selection),
.batch-manual-entry-cell :deep(.text-truncate) {
    font-size: 0.875rem;
}

.batch-manual-entry-cell :deep(.v-chip) {
    margin-top: 2px;
    margin-bottom: 2px;
}

.batch-manual-entry-empty-cell {
    min-height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
    background-color: rgba(var(--v-theme-surface-variant), 0.28);
    border-radius: 4px;
    margin: 2px 0;
}

.batch-manual-entry-actions :deep(.v-btn) {
    min-width: 32px;
}

@media (max-width: 960px) {
    .batch-manual-entry-title {
        grid-template-columns: 1fr;
    }

    .batch-manual-entry-title__meta,
    .batch-manual-entry-title__heading,
    .batch-manual-entry-title__actions {
        justify-content: center;
        text-align: center;
    }
}

.batch-manual-entry-table :deep(th:last-child),
.batch-manual-entry-table :deep(td:last-child) {
    border-right: none;
}
</style>
