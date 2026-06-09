import { describe, expect, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import type { ImportPreviewResolvedCategoryPath } from '@/views/desktop/transactions/import/importPreview.ts';
import {
    buildImportPreviewUpdateFromTransaction,
    getPreviewIdFromImportTransaction,
    getPreviewUpdateId
} from '@/views/desktop/transactions/import/importPreviewUpdates.ts';
import type { ImportPreviewTransactionDraft } from '@/views/desktop/transactions/import/importPreviewTransaction.ts';

const categoryPath: ImportPreviewResolvedCategoryPath = {
    id: '11',
    mainCategory: '餐饮',
    subCategory: '咖啡',
    displayCategory: '咖啡',
    type: TransactionType.Expense
};

function makeTransaction(overrides: Partial<ImportTransaction> = {}): ImportPreviewTransactionDraft {
    const transaction = ImportTransaction.of({
        type: TransactionType.Transfer,
        categoryId: '11',
        originalCategoryName: '咖啡',
        time: 0,
        utcOffset: 480,
        sourceAccountId: '101',
        originalSourceAccountName: '招商银行',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '202',
        sourceAmount: 1234,
        destinationAmount: 5678,
        tagIds: [],
        originalTagNames: [],
        comment: '',
        selected: true,
        recurringTemplateId: '44',
        recurringTemplateName: '房租',
        recurringCandidateCount: 2,
        recurringMatchScore: 0.8,
        recurringMatchReasons: '周期相似',
        recurringMatchedDate: '2026-06-01'
    }, 1) as ImportPreviewTransactionDraft;

    Object.assign(transaction, overrides);
    transaction._previewId = 42;
    return transaction;
}

describe('import preview update helper', () => {
    test('builds stage3 preview update payload with selected transaction edits and suggestion clears', () => {
        const update = buildImportPreviewUpdateFromTransaction(makeTransaction(), {
            categoryPath,
            clearTransferDecision: true,
            clearLearningDecision: true,
            clearLlmDecision: true,
            includeSuggestionDecisionClears: true
        });

        expect(update).toEqual({
            id: 42,
            preview_type: '转账',
            preview_amount: 12.34,
            preview_destination_amount: 56.78,
            preview_source_account_id: 101,
            preview_destination_account_id: 202,
            preview_recurring_id: 44,
            preview_recurring_name: '房租',
            preview_recurring_candidate_count: 2,
            preview_recurring_match_score: 0.8,
            preview_recurring_match_reasons: '周期相似',
            preview_recurring_matched_date: '2026-06-01',
            category_id: 11,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡',
            clear_transfer_decision: true,
            clear_learning_decision: true,
            clear_llm_decision: true,
            clear_actionable_suggestions: ['transfer', 'learning', 'llm'],
            selected: true
        });
    });

    test('keeps ImportDialog-style confirm updates free of learning and LLM clear fields unless requested', () => {
        const transaction = makeTransaction({
            type: TransactionType.Investment,
            sourceAccountId: '',
            destinationAccountId: 'invalid-id',
            recurringTemplateId: '',
            selected: false
        });
        const update = buildImportPreviewUpdateFromTransaction(transaction, {
            categoryPath: null,
            clearTransferDecision: false,
            clearLearningDecision: true,
            clearLlmDecision: true
        });

        expect(update.preview_type).toBe('投资');
        expect(update.preview_source_account_id).toBeNull();
        expect(update.preview_destination_account_id).toBeNull();
        expect(update.preview_recurring_id).toBeNull();
        expect(update.category_id).toBeNull();
        expect(update.preview_main_category).toBe('');
        expect(update.preview_sub_category).toBe('');
        expect(update.clear_transfer_decision).toBe(false);
        expect(update).not.toHaveProperty('clear_learning_decision');
        expect(update).not.toHaveProperty('clear_llm_decision');
        expect(update).not.toHaveProperty('clear_actionable_suggestions');
        expect(update.selected).toBe(false);
    });

    test('extracts positive preview ids from transactions and preview update records only', () => {
        expect(getPreviewIdFromImportTransaction(makeTransaction())).toBe(42);

        const missingPreviewId = makeTransaction();
        missingPreviewId._previewId = 0;
        expect(getPreviewIdFromImportTransaction(missingPreviewId)).toBeNull();

        expect(getPreviewUpdateId({ id: 99 })).toBe(99);
        expect(getPreviewUpdateId({ id: ' 100 ' })).toBe(100);
        expect(getPreviewUpdateId({ id: 0 })).toBeNull();
        expect(getPreviewUpdateId({ id: -1 })).toBeNull();
        expect(getPreviewUpdateId({ id: 'abc' })).toBeNull();
        expect(getPreviewUpdateId({})).toBeNull();
    });
});
