/* eslint-disable @typescript-eslint/no-explicit-any */
import { describe, expect, jest, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import { useImportCheckDataBatchActions } from '@/views/desktop/transactions/import/check-data-tab/useImportCheckDataBatchActions.ts';

function createOptions(overrides: Record<string, unknown> = {}): any {
    return {
        allAccountsMapByName: { value: {} },
        allAccountsMap: { value: {} },
        allCategories: { value: {} },
        allCategoriesMap: { value: {} },
        allInvalidAccountNames: { value: [] },
        allInvalidExpenseCategoryNames: { value: [] },
        allInvalidIncomeCategoryNames: { value: [] },
        allInvalidTransactionTagNames: { value: [] },
        allInvalidTransferCategoryNames: { value: [] },
        allTagsMap: { value: {} },
        assignCategoryIdIfKnown: jest.fn(() => false),
        assignDestinationAccountIdIfKnown: jest.fn(() => false),
        assignSourceAccountIdIfKnown: jest.fn(() => false),
        batchCreateDialog: { value: null },
        batchReplaceAllTypesDialog: { value: null },
        batchReplaceDialog: { value: null },
        getDisplayCount: (count: number) => String(count),
        importTransactions: { value: [] },
        isEditing: { value: false },
        snackbar: { value: null },
        syncActionableSuggestionDraftState: jest.fn(),
        updateTransactionData: jest.fn(),
        ...overrides
    };
}

async function flush(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
}

describe('useImportCheckDataBatchActions optional and malformed edge branches', () => {
    test('tolerates every absent dialog ref without scheduling mutations', () => {
        const options = createOptions();
        const actions = useImportCheckDataBatchActions(options);

        expect(() => actions.showBatchReplaceDialog('account')).not.toThrow();
        expect(() => actions.showBatchAddDialog('tag')).not.toThrow();
        expect(() => actions.showReplaceInvalidItemDialog('tag', [])).not.toThrow();
        expect(() => actions.showReplaceAllTypesDialog()).not.toThrow();
        expect(() => actions.showBatchCreateInvalidItemDialog('tag', [])).not.toThrow();
        expect(options.updateTransactionData).not.toHaveBeenCalled();
    });

    test('does not overwrite already-known invalid identities or malformed tag display state', async () => {
        const transaction = {
            selected: true,
            valid: false,
            type: TransactionType.Transfer,
            categoryId: 'known-category',
            originalCategoryName: 'Missing category',
            sourceAccountId: 'known-source',
            originalSourceAccountName: 'Missing account',
            destinationAccountId: 'known-destination',
            originalDestinationAccountName: 'Missing account',
            tagIds: ['known-tag'],
            originalTagNames: [],
            isManuallyAnnotated: false
        };
        const open = jest.fn<(...args: any[]) => Promise<any>>()
            .mockResolvedValue({ sourceItem: 'Missing account', targetItem: 'target' });
        const options = createOptions({
            allAccountsMap: {
                value: {
                    'known-source': { id: 'known-source' },
                    'known-destination': { id: 'known-destination' }
                }
            },
            allTagsMap: { value: { 'known-tag': { id: 'known-tag', name: 'Known' } } },
            importTransactions: { value: [transaction] },
            batchReplaceDialog: { value: { open } }
        });
        const actions = useImportCheckDataBatchActions(options);

        actions.showReplaceInvalidItemDialog('account', []);
        await flush();
        expect(options.assignSourceAccountIdIfKnown).not.toHaveBeenCalled();
        expect(options.assignDestinationAccountIdIfKnown).not.toHaveBeenCalled();

        open.mockResolvedValueOnce({ sourceItem: '', targetItem: '' });
        actions.showReplaceInvalidItemDialog('tag', []);
        await flush();
        expect(options.updateTransactionData).not.toHaveBeenCalled();
    });

    test('keeps transfer-to-income source empty when destination assignment is rejected', () => {
        const transaction = {
            selected: true,
            type: TransactionType.Transfer,
            originalCategoryName: 'Unknown',
            originalDestinationAccountName: '',
            sourceAccountId: 'source',
            destinationAccountId: 'unavailable',
            sourceAmountCents: -100,
            destinationAmountCents: 100,
            isManuallyAnnotated: false
        };
        const options = createOptions({ importTransactions: { value: [transaction] } });
        const actions = useImportCheckDataBatchActions(options);

        actions.convertTransactionType(TransactionType.Transfer, TransactionType.Income);
        expect(transaction).toMatchObject({
            type: TransactionType.Income,
            sourceAccountId: '',
            sourceAmountCents: 100,
            destinationAccountId: '',
            destinationAmountCents: 0,
            isManuallyAnnotated: true
        });
    });
});
