import { describe, expect, jest, test } from '@jest/globals';

import { CategoryType } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { useImportCheckDataBatchActions } from '@/views/desktop/transactions/import/check-data-tab/useImportCheckDataBatchActions.ts';

type DialogResult = Record<string, unknown> | null | undefined;

function transaction(
    id: number,
    type: TransactionType,
    overrides: Record<string, unknown> = {}
): ImportTransaction {
    const value = ImportTransaction.of({
        type,
        categoryId: '',
        originalCategoryName: type === TransactionType.Income ? 'Salary' : 'Cafe',
        time: 1_789_000_000 + id,
        utcOffset: 480,
        sourceAccountId: '',
        originalSourceAccountName: 'Wallet',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '',
        originalDestinationAccountName: 'Savings',
        originalDestinationAccountCurrency: 'CNY',
        sourceAmountCents: type === TransactionType.Income ? 1_000 : -1_000,
        destinationAmountCents: 1_000,
        tagIds: [],
        originalTagNames: [],
        comment: `row-${id}`,
        counterparty: `merchant-${id}`,
        paymentMethod: 'card',
        selected: true,
        valid: false,
        ...overrides
    } as never, id);

    if ('valid' in overrides) {
        value.valid = Boolean(overrides['valid']);
    }

    return value;
}

function category(id: string, name: string, type: CategoryType): TransactionCategory {
    return TransactionCategory.of({
        id,
        name,
        parentId: 'parent',
        type,
        icon: '1',
        color: '#000000',
        comment: '',
        displayOrder: 0,
        hidden: false
    });
}

function dialog(result: DialogResult) {
    return {
        value: {
            open: jest.fn<(...args: unknown[]) => Promise<DialogResult>>().mockResolvedValue(result)
        }
    };
}

function createHarness(transactions: ImportTransaction[], results: {
    replace?: DialogResult;
    replaceAll?: DialogResult;
    create?: DialogResult;
    editing?: boolean;
} = {}) {
    const expense = category('expense-cafe', 'Cafe', CategoryType.Expense);
    const income = category('income-salary', 'Salary', CategoryType.Income);
    const transfer = category('transfer-move', 'Move', CategoryType.Transfer);
    const categories = {
        [CategoryType.Expense]: [{ subCategories: [expense] }],
        [CategoryType.Income]: [{ subCategories: [income] }],
        [CategoryType.Transfer]: [{ subCategories: [transfer] }]
    } as unknown as Record<number, TransactionCategory[]>;
    const categoriesMap = { [expense.id]: expense, [income.id]: income, [transfer.id]: transfer };
    const accounts = {
        wallet: { id: 'wallet', name: 'Wallet' },
        savings: { id: 'savings', name: 'Savings' },
        income: { id: 'income', name: 'Income' }
    };
    const accountsByName = { Wallet: accounts.wallet, Savings: accounts.savings, Income: accounts.income };
    const tags = {
        'tag-new': { id: 'tag-new', name: 'New' },
        'tag-other': { id: 'tag-other', name: 'Other' }
    };
    const replaceDialog = dialog(results.replace);
    const replaceAllDialog = dialog(results.replaceAll);
    const createDialog = dialog(results.create);
    const updateTransactionData = jest.fn<(value: ImportTransaction) => void>();
    const syncActionableSuggestionDraftState = jest.fn<(value: ImportTransaction) => void>();
    const showMessage = jest.fn<(message: string, payload?: Record<string, unknown>) => void>();
    const assignCategoryIdIfKnown = jest.fn((value: ImportTransaction, id: unknown) => {
        const key = String(id ?? '');
        if (!categoriesMap[key]) {
            return false;
        }
        value.categoryId = key;
        value.originalCategoryName = categoriesMap[key].name;
        return true;
    });
    const assignSourceAccountIdIfKnown = jest.fn((value: ImportTransaction, id: unknown) => {
        const key = String(id ?? '');
        if (!accounts[key as keyof typeof accounts]) {
            return false;
        }
        value.sourceAccountId = key;
        return true;
    });
    const assignDestinationAccountIdIfKnown = jest.fn((value: ImportTransaction, id: unknown) => {
        const key = String(id ?? '');
        if (!accounts[key as keyof typeof accounts]) {
            return false;
        }
        value.destinationAccountId = key;
        return true;
    });
    const options = {
        allAccountsMapByName: { value: accountsByName },
        allAccountsMap: { value: accounts },
        allCategories: { value: categories },
        allCategoriesMap: { value: categoriesMap },
        allInvalidAccountNames: { value: [{ name: 'Missing account', value: 'Missing account' }] },
        allInvalidExpenseCategoryNames: { value: [{ name: 'Missing expense', value: 'Missing expense' }] },
        allInvalidIncomeCategoryNames: { value: [{ name: 'Missing income', value: 'Missing income' }] },
        allInvalidTransactionTagNames: { value: [{ name: 'Missing tag', value: 'Missing tag' }] },
        allInvalidTransferCategoryNames: { value: [{ name: 'Missing transfer', value: 'Missing transfer' }] },
        allTagsMap: { value: tags },
        assignCategoryIdIfKnown,
        assignDestinationAccountIdIfKnown,
        assignSourceAccountIdIfKnown,
        batchCreateDialog: createDialog,
        batchReplaceAllTypesDialog: replaceAllDialog,
        batchReplaceDialog: replaceDialog,
        getDisplayCount: (count: number) => `count:${count}`,
        importTransactions: { value: transactions },
        isEditing: { value: results.editing ?? false },
        snackbar: { value: { showMessage } },
        syncActionableSuggestionDraftState,
        updateTransactionData
    };

    return {
        actions: useImportCheckDataBatchActions(options as never),
        assignCategoryIdIfKnown,
        assignDestinationAccountIdIfKnown,
        assignSourceAccountIdIfKnown,
        createDialog,
        replaceAllDialog,
        replaceDialog,
        showMessage,
        syncActionableSuggestionDraftState,
        updateTransactionData
    };
}

async function flushDialog(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
}

describe('useImportCheckDataBatchActions', () => {
    test('editing mode blocks every managed dialog before opening it', () => {
        const harness = createHarness([], { editing: true });

        harness.actions.showBatchReplaceDialog('account');
        harness.actions.showBatchAddDialog('tag');
        harness.actions.showReplaceInvalidItemDialog('tag', []);
        harness.actions.showReplaceAllTypesDialog();
        harness.actions.showBatchCreateInvalidItemDialog('tag', []);

        expect(harness.replaceDialog.value.open).not.toHaveBeenCalled();
        expect(harness.replaceAllDialog.value.open).not.toHaveBeenCalled();
        expect(harness.createDialog.value.open).not.toHaveBeenCalled();
    });

    test.each([
        ['expenseCategory', TransactionType.Expense, 'expense-cafe'],
        ['incomeCategory', TransactionType.Income, 'income-salary'],
        ['transferCategory', TransactionType.Transfer, 'transfer-move'],
        ['account', TransactionType.Expense, 'wallet'],
        ['destinationAccount', TransactionType.Transfer, 'savings']
    ] as const)('batch replace applies %s only to selected compatible rows', async (type, transactionType, targetItem) => {
        const selected = transaction(1, transactionType);
        const unselected = transaction(2, transactionType, { selected: false });
        const incompatible = transaction(3, TransactionType.ModifyBalance, {
            originalSourceAccountName: type === 'account' ? 'Other' : 'Wallet'
        });
        const harness = createHarness([selected, unselected, incompatible], {
            replace: { targetItem }
        });

        harness.actions.showBatchReplaceDialog(type);
        await flushDialog();

        expect(selected.isManuallyAnnotated).toBe(true);
        expect(unselected.isManuallyAnnotated).toBe(false);
        expect(incompatible.isManuallyAnnotated).toBe(type === 'account');
        expect(harness.updateTransactionData).toHaveBeenCalledTimes(type === 'account' ? 2 : 1);
        expect(harness.showMessage).toHaveBeenCalledWith(
            'format.misc.youHaveUpdatedTransactions',
            { count: type === 'account' ? 'count:2' : 'count:1' }
        );
        expect(harness.syncActionableSuggestionDraftState).toHaveBeenCalledTimes(
            type.endsWith('Category') ? 1 : 0
        );
    });

    test('batch tag replace supports replacement and removal while preserving unrelated rows', async () => {
        const replaced = transaction(4, TransactionType.Expense, {
            tagIds: ['tag-old', 'tag-other'],
            originalTagNames: ['Old', 'Other']
        });
        const harness = createHarness([replaced], {
            replace: { sourceItem: 'Old', targetItem: 'tag-new' }
        });
        harness.actions.showBatchReplaceDialog('tag', [{ name: 'Old', value: 'Old' }]);
        await flushDialog();
        expect(replaced.tagIds).toStrictEqual(['tag-new', 'tag-other']);
        expect(replaced.originalTagNames).toStrictEqual(['New', 'Other']);

        harness.replaceDialog.value.open.mockResolvedValueOnce({ sourceItem: 'Other', targetItem: '' });
        harness.actions.showBatchReplaceDialog('tag');
        await flushDialog();
        expect(replaced.tagIds).toStrictEqual(['tag-new']);
        expect(replaced.originalTagNames).toStrictEqual(['New']);
    });

    test('unknown tag ids retain an explicit empty display name in replace and add flows', async () => {
        const replaced = transaction(32, TransactionType.Expense, {
            tagIds: ['tag-old'],
            originalTagNames: ['Old']
        });
        const replaceHarness = createHarness([replaced], {
            replace: { sourceItem: 'Old', targetItem: 'tag-missing' }
        });
        replaceHarness.actions.showBatchReplaceDialog('tag');
        await flushDialog();
        expect(replaced).toMatchObject({
            tagIds: ['tag-missing'],
            originalTagNames: ['']
        });

        const added = transaction(33, TransactionType.Expense);
        const addHarness = createHarness([added], {
            replace: { targetItem: 'tag-missing' }
        });
        addHarness.actions.showBatchAddDialog('tag');
        await flushDialog();
        expect(added).toMatchObject({
            tagIds: ['tag-missing'],
            originalTagNames: ['']
        });
    });

    test.each([null, {}, { targetItem: '' }])('batch replace ignores cancelled or incomplete result %#', async result => {
        const value = transaction(5, TransactionType.Expense);
        const harness = createHarness([value], { replace: result });
        harness.actions.showBatchReplaceDialog('account');
        await flushDialog();
        expect(harness.updateTransactionData).not.toHaveBeenCalled();
        expect(harness.showMessage).not.toHaveBeenCalled();
    });

    test('batch add adds a missing tag once and ignores duplicates, unselected rows, and invalid results', async () => {
        const missing = transaction(6, TransactionType.Expense, {
            tagIds: null,
            originalTagNames: null
        });
        missing.tagIds = null as never;
        missing.originalTagNames = '' as never;
        const duplicate = transaction(7, TransactionType.Expense, {
            tagIds: ['tag-new'],
            originalTagNames: ['tag-new']
        });
        const unselected = transaction(8, TransactionType.Expense, { selected: false });
        const harness = createHarness([missing, duplicate, unselected], {
            replace: { targetItem: 'tag-new' }
        });

        harness.actions.showBatchAddDialog('tag');
        await flushDialog();
        expect(missing.tagIds).toStrictEqual(['tag-new']);
        expect(missing.originalTagNames).toStrictEqual(['New']);
        expect(harness.updateTransactionData).toHaveBeenCalledTimes(1);

        for (const result of [null, {}, { targetItem: '' }]) {
            harness.replaceDialog.value.open.mockResolvedValueOnce(result);
            harness.actions.showBatchAddDialog('tag');
            await flushDialog();
        }
        expect(harness.updateTransactionData).toHaveBeenCalledTimes(1);
    });

    test('replace-invalid handles category, both transfer accounts, tag replacement and tag removal', async () => {
        const expense = transaction(9, TransactionType.Expense, {
            originalCategoryName: 'Missing expense',
            categoryId: '0'
        });
        const transfer = transaction(10, TransactionType.Transfer, {
            sourceAccountId: '0',
            destinationAccountId: 'missing',
            originalSourceAccountName: 'Missing account',
            originalDestinationAccountName: 'Missing account'
        });
        const tagged = transaction(11, TransactionType.Expense, {
            tagIds: ['0', 'missing'],
            originalTagNames: ['Missing tag', 'Remove tag']
        });
        const valid = transaction(12, TransactionType.Expense, {
            valid: true,
            originalCategoryName: 'Missing expense'
        });
        const harness = createHarness([expense, transfer, tagged, valid], {
            replace: { sourceItem: 'Missing expense', targetItem: 'expense-cafe' }
        });

        harness.actions.showReplaceInvalidItemDialog('expenseCategory', []);
        await flushDialog();
        expect(expense.categoryId).toBe('expense-cafe');
        expect(valid.categoryId).toBe('');

        harness.replaceDialog.value.open.mockResolvedValueOnce({
            sourceItem: 'Missing account',
            targetItem: 'wallet'
        });
        harness.actions.showReplaceInvalidItemDialog('account', []);
        await flushDialog();
        expect(transfer.sourceAccountId).toBe('wallet');
        expect(transfer.destinationAccountId).toBe('wallet');

        harness.replaceDialog.value.open.mockResolvedValueOnce({
            sourceItem: 'Missing tag',
            targetItem: 'tag-new'
        });
        harness.actions.showReplaceInvalidItemDialog('tag', []);
        await flushDialog();
        expect(tagged.tagIds[0]).toBe('tag-new');
        expect(tagged.originalTagNames[0]).toBe('New');

        harness.replaceDialog.value.open.mockResolvedValueOnce({ sourceItem: 'Remove tag', targetItem: '' });
        harness.actions.showReplaceInvalidItemDialog('tag', []);
        await flushDialog();
        expect(tagged.tagIds).toStrictEqual(['tag-new']);
        expect(tagged.originalTagNames).toStrictEqual(['New']);
    });

    test.each([null, {}, { sourceItem: null }, { sourceItem: 'x', targetItem: '' }])(
        'replace-invalid ignores incomplete non-tag result %#',
        async result => {
            const value = transaction(13, TransactionType.Expense);
            const harness = createHarness([value], { replace: result });
            harness.actions.showReplaceInvalidItemDialog('expenseCategory', []);
            await flushDialog();
            expect(harness.updateTransactionData).not.toHaveBeenCalled();
        }
    );

    test('replace-all applies valid rules for every family and skips malformed rules', async () => {
        const expense = transaction(14, TransactionType.Expense, {
            originalCategoryName: 'Missing expense',
            originalSourceAccountName: 'Missing account',
            tagIds: ['old'],
            originalTagNames: ['Missing tag']
        });
        const income = transaction(15, TransactionType.Income, { originalCategoryName: 'Missing income' });
        const transfer = transaction(16, TransactionType.Transfer, {
            originalCategoryName: 'Missing transfer',
            originalDestinationAccountName: 'Missing destination'
        });
        const balance = transaction(17, TransactionType.ModifyBalance, {
            originalCategoryName: 'Missing expense'
        });
        const harness = createHarness([expense, income, transfer, balance], {
            replaceAll: {
                rules: [
                    null,
                    {},
                    { dataType: 'expenseCategory', sourceValue: 'Missing expense', targetId: 'expense-cafe' },
                    { dataType: 'incomeCategory', sourceValue: 'Missing income', targetId: 'income-salary' },
                    { dataType: 'transferCategory', sourceValue: 'Missing transfer', targetId: 'transfer-move' },
                    { dataType: 'account', sourceValue: 'Missing account', targetId: 'wallet' },
                    { dataType: 'account', sourceValue: 'Missing destination', targetId: 'savings' },
                    { dataType: 'tag', sourceValue: 'Missing tag', targetId: 'tag-new' }
                ]
            }
        });

        harness.actions.showReplaceAllTypesDialog();
        await flushDialog();
        expect(expense).toMatchObject({ categoryId: 'expense-cafe', sourceAccountId: 'wallet' });
        expect(expense.tagIds).toStrictEqual(['tag-new']);
        expect(income.categoryId).toBe('income-salary');
        expect(transfer).toMatchObject({ categoryId: 'transfer-move', destinationAccountId: 'savings' });
        expect(balance.categoryId).toBe('');
        expect(harness.updateTransactionData).toHaveBeenCalledTimes(3);
        expect(harness.syncActionableSuggestionDraftState).toHaveBeenCalledTimes(3);
    });

    test.each([null, {}, { rules: null }])('replace-all ignores cancelled result %#', async result => {
        const harness = createHarness([transaction(18, TransactionType.Expense)], { replaceAll: result });
        harness.actions.showReplaceAllTypesDialog();
        await flushDialog();
        expect(harness.updateTransactionData).not.toHaveBeenCalled();
    });

    test('batch create maps invalid categories and tags while leaving valid rows unchanged', async () => {
        const expense = transaction(19, TransactionType.Expense, {
            categoryId: '0',
            originalCategoryName: 'Missing expense'
        });
        const tag = transaction(20, TransactionType.Expense, {
            tagIds: ['0'],
            originalTagNames: ['Missing tag']
        });
        const valid = transaction(21, TransactionType.Expense, {
            valid: true,
            categoryId: '0',
            originalCategoryName: 'Missing expense'
        });
        const harness = createHarness([expense, tag, valid], {
            create: { sourceTargetMap: { 'Missing expense': 'expense-cafe' } }
        });
        harness.actions.showBatchCreateInvalidItemDialog('expenseCategory', []);
        await flushDialog();
        expect(expense.categoryId).toBe('expense-cafe');
        expect(valid.categoryId).toBe('0');

        harness.createDialog.value.open.mockResolvedValueOnce({
            sourceTargetMap: { 'Missing tag': 'tag-new' }
        });
        harness.actions.showBatchCreateInvalidItemDialog('tag', []);
        await flushDialog();
        expect(tag.tagIds).toStrictEqual(['tag-new']);
    });

    test.each([
        ['incomeCategory', TransactionType.Income, 'Missing income', 'income-salary'],
        ['transferCategory', TransactionType.Transfer, 'Missing transfer', 'transfer-move']
    ] as const)('replace-invalid and batch-create cover %s identity mappings', async (
        type,
        transactionType,
        originalCategoryName,
        targetId
    ) => {
        const replaceValue = transaction(29, transactionType, {
            categoryId: '0',
            originalCategoryName,
            valid: false
        });
        const replaceHarness = createHarness([replaceValue], {
            replace: { sourceItem: originalCategoryName, targetItem: targetId }
        });

        replaceHarness.actions.showReplaceInvalidItemDialog(type, []);
        await flushDialog();
        expect(replaceValue.categoryId).toBe(targetId);

        const createValue = transaction(30, transactionType, {
            categoryId: '0',
            originalCategoryName,
            valid: false
        });
        const createMappingHarness = createHarness([createValue], {
            create: { sourceTargetMap: { [originalCategoryName]: targetId } }
        });

        createMappingHarness.actions.showBatchCreateInvalidItemDialog(type, []);
        await flushDialog();
        expect(createValue.categoryId).toBe(targetId);
    });

    test.each([null, {}, { sourceTargetMap: null }])('batch create ignores incomplete result %#', async result => {
        const harness = createHarness([transaction(22, TransactionType.Expense)], { create: result });
        harness.actions.showBatchCreateInvalidItemDialog('tag', []);
        await flushDialog();
        expect(harness.updateTransactionData).not.toHaveBeenCalled();
    });

    test('valid dialog results are harmless when the import list is empty', async () => {
        const harness = createHarness([], {
            replace: { sourceItem: 'Missing account', targetItem: 'wallet' },
            replaceAll: {
                rules: [{ dataType: 'account', sourceValue: 'Missing account', targetId: 'wallet' }]
            },
            create: { sourceTargetMap: { 'Missing tag': 'tag-new' } }
        });

        harness.actions.showBatchReplaceDialog('account');
        await flushDialog();
        harness.actions.showBatchAddDialog('tag');
        await flushDialog();
        harness.actions.showReplaceInvalidItemDialog('account', []);
        await flushDialog();
        harness.actions.showReplaceAllTypesDialog();
        await flushDialog();
        harness.actions.showBatchCreateInvalidItemDialog('tag', []);
        await flushDialog();

        expect(harness.updateTransactionData).not.toHaveBeenCalled();
        expect(harness.showMessage).not.toHaveBeenCalled();
    });

    test('valid no-op inputs do not mutate rows or report phantom updates', async () => {
        const value = transaction(31, TransactionType.Expense, {
            recurringTemplateId: '',
            valid: false
        });
        const harness = createHarness([value], {
            replace: { sourceItem: 'No match', targetItem: 'wallet' },
            replaceAll: {
                rules: [{ dataType: 'account', sourceValue: 'No match', targetId: 'wallet' }]
            },
            create: { sourceTargetMap: { 'No match': 'tag-new' } }
        });

        harness.actions.showBatchReplaceDialog('unsupported' as never);
        await flushDialog();
        harness.actions.showBatchAddDialog('unsupported' as never);
        await flushDialog();
        harness.actions.showReplaceInvalidItemDialog('expenseCategory', []);
        await flushDialog();
        harness.actions.showReplaceAllTypesDialog();
        await flushDialog();
        harness.actions.showBatchCreateInvalidItemDialog('tag', []);
        await flushDialog();
        harness.actions.clearSelectedRecurringMatches();

        expect(harness.updateTransactionData).not.toHaveBeenCalled();
        expect(harness.showMessage).not.toHaveBeenCalled();
    });

    test('type conversion resolves category and transfer identities and preserves amount direction', () => {
        const expense = transaction(23, TransactionType.Expense, {
            originalCategoryName: 'Move',
            originalDestinationAccountName: 'Savings',
            sourceAmountCents: -2500,
            destinationAmountCents: 0
        });
        const harness = createHarness([expense]);
        harness.actions.convertTransactionType(TransactionType.Expense, TransactionType.Transfer);
        expect(expense).toMatchObject({
            type: TransactionType.Transfer,
            categoryId: 'transfer-move',
            destinationAccountId: 'savings',
            destinationAmountCents: -2500
        });

        harness.actions.convertTransactionType(TransactionType.Transfer, TransactionType.Income);
        expect(expense).toMatchObject({
            type: TransactionType.Income,
            sourceAccountId: 'savings',
            sourceAmountCents: -2500,
            destinationAccountId: '',
            destinationAmountCents: 0
        });
        expect(harness.updateTransactionData).toHaveBeenCalledTimes(2);
    });

    test('type conversion clears unresolved identities and ignores empty, unselected and unsupported targets', () => {
        const selected = transaction(24, TransactionType.Expense, {
            originalCategoryName: 'Unknown',
            originalDestinationAccountName: 'Unknown'
        });
        const unselected = transaction(25, TransactionType.Expense, { selected: false });
        const harness = createHarness([selected, unselected]);
        harness.actions.convertTransactionType(TransactionType.Expense, TransactionType.ModifyBalance);
        expect(harness.updateTransactionData).not.toHaveBeenCalled();

        harness.actions.convertTransactionType(TransactionType.Expense, TransactionType.Transfer);
        expect(selected).toMatchObject({ categoryId: '', destinationAccountId: '' });
        expect(unselected.type).toBe(TransactionType.Expense);

        harness.actions.convertTransactionType(TransactionType.Transfer, TransactionType.Income);
        expect(selected).toMatchObject({ sourceAccountId: '', destinationAccountId: '' });

        createHarness([]).actions.convertTransactionType(TransactionType.Expense, TransactionType.Income);
    });

    test('clear recurring matches mutates only selected matched rows and retains candidate count', () => {
        const matched = transaction(26, TransactionType.Expense, {
            recurringTemplateId: 'recurring-1',
            recurringTemplateName: 'Rent',
            recurringCandidateCount: 3
        });
        const unselected = transaction(27, TransactionType.Expense, {
            selected: false,
            recurringTemplateId: 'recurring-2'
        });
        const absent = transaction(28, TransactionType.Expense);
        const harness = createHarness([matched, unselected, absent]);

        harness.actions.clearSelectedRecurringMatches();
        expect(matched).toMatchObject({
            recurringTemplateId: '',
            recurringTemplateName: '',
            recurringCandidateCount: 3
        });
        expect(unselected.recurringTemplateId).toBe('recurring-2');
        expect(harness.updateTransactionData).toHaveBeenCalledTimes(1);
        expect(harness.syncActionableSuggestionDraftState).toHaveBeenCalledTimes(1);
        expect(harness.showMessage).toHaveBeenCalledWith(
            'format.misc.youHaveUpdatedTransactions',
            { count: 'count:1' }
        );

        createHarness([]).actions.clearSelectedRecurringMatches();
    });
});
