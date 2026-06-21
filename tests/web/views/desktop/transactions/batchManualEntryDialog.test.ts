import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8').replace(/\r\n/g, '\n');
}

const BATCH_DIALOG_PATH = 'src/views/desktop/transactions/list/dialogs/BatchManualEntryDialog.vue';
const LIST_PAGE_PATH = 'src/views/desktop/transactions/ListPage.vue';

describe('BatchManualEntryDialog source contract', () => {
    test('keeps transfer and investment destination fields in the keyboard grid only when needed', () => {
        const source = readSource(BATCH_DIALOG_PATH);

        expect(source).toContain("type BatchEntryFieldKey = 'time' | 'type' | 'category' | 'sourceAccount' | 'sourceAmountCents' | 'destinationAccount' | 'destinationAmountCents' | 'comment' | 'tags';");
        expect(source).toContain("data-batch-field=\"destinationAccount\"");
        expect(source).toContain("data-batch-field=\"destinationAmountCents\"");
        expect(source).toContain("@keydown.capture=\"onCellKeydown(index, 'destinationAccount', $event)\"");
        expect(source).toContain("@keydown.capture=\"onCellKeydown(index, 'destinationAmountCents', $event)\"");
        expect(source).toContain('return transaction.type === TransactionType.Transfer || transaction.type === TransactionType.Investment;');
        expect(source).toContain("return BATCH_ENTRY_FIELD_ORDER.filter(field => field !== 'destinationAccount' && field !== 'destinationAmountCents');");
        expect(source).toContain("return 'sourceAccount';");
        expect(source).toContain("return 'sourceAmountCents';");
    });

    test('keeps destination amount auto-sync explicit and reversible', () => {
        const source = readSource(BATCH_DIALOG_PATH);

        expect(source).toContain('destinationAmountSyncState.value[rowId] = shouldSyncDestinationAmount(transaction);');
        expect(source).toContain('&& (!transaction.destinationAmountCents || transaction.destinationAmountCents === transaction.sourceAmountCents);');
        expect(source).toContain('transaction.destinationAmountCents = transaction.sourceAmountCents;');
        expect(source).toContain('setDestinationAmountSync(row.id, true);');
        expect(source).toContain('setDestinationAmountSync(row.id, row.transaction.destinationAmountCents === row.transaction.sourceAmountCents);');
        expect(source).toContain("transaction.destinationAccountId = '0';\n        transaction.destinationAmountCents = 0;\n        setDestinationAmountSync(row.id, false);");
    });

    test('validates non-empty rows before submitting one batch transaction request', () => {
        const source = readSource(BATCH_DIALOG_PATH);

        expect(source).toContain('const invalidNonEmptyRowCount = computed<number>(() => {');
        expect(source).toContain('return rows.value.filter(row => !isRowEmpty(row) && !isRowValid(row)).length;');
        expect(source).toContain('{{ getRowIssues(row).join(\' / \') }}');
        expect(source).toContain('const nonEmptyRows = rows.value.filter(row => !isRowEmpty(row));');
        expect(source).toContain("snackbar.value?.showError(tt('No valid transactions to save'));");
        expect(source).toContain('const invalidRows = nonEmptyRows.filter(row => !isRowValid(row));');
        expect(source).toContain("snackbar.value?.showError(tt('Please complete required fields in all non-empty rows'));");
        expect(source).toContain('transactions: nonEmptyRows.map(row => row.transaction)');
        expect(source).toContain("clientSessionId: `batch-manual-${Date.now()}`");
        expect(source).not.toContain('transactions: rows.value.map(row => row.transaction)');
    });

    test('bulk fill actions separate overwrite-all from fill-empty behavior', () => {
        const source = readSource(BATCH_DIALOG_PATH);

        expect(source).toContain('function applyCommonFields(target: Transaction, source: Transaction, rowId: string): void {');
        expect(source).toContain('target.type = source.type;');
        expect(source).toContain('target.sourceAccountId = source.sourceAccountId;');
        expect(source).toContain('target.tagIds = [...source.tagIds];');
        expect(source).toContain('function fillEmptyCommonFields(target: Transaction, source: Transaction, rowId: string): void {');
        expect(source).toContain('if (!target.getCategoryId() && source.getCategoryId())');
        expect(source).toContain("if (!target.sourceAccountId || target.sourceAccountId === '0')");
        expect(source).toContain('if (!target.sourceAmountCents || target.sourceAmountCents <= 0)');
        expect(source).toContain('if ((!target.tagIds || target.tagIds.length === 0) && source.tagIds.length > 0)');
        expect(source).toContain('if (!target.comment && source.comment)');
    });
});

describe('desktop transaction ListPage batch manual entry affordance', () => {
    test('passes the active filter context into the batch entry dialog and reloads after save', () => {
        const source = readSource(LIST_PAGE_PATH);

        expect(source).toContain('function batchAdd(): void {');
        expect(source).toContain('batchManualEntryDialog.value?.open({');
        expect(source).toContain('time: newTransactionTime');
        expect(source).toContain('type: query.value.type');
        expect(source).toContain("categoryId: queryAllFilterCategoryIdsCount.value === 1 ? query.value.categoryIds : ''");
        expect(source).toContain("accountId: queryAllFilterAccountIdsCount.value === 1 ? query.value.accountIds : ''");
        expect(source).toContain("tagIds: query.value.tagIds || ''");
        expect(source).toContain('reload(false, false);');
    });
});
