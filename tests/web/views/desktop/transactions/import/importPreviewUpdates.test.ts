import { describe, expect, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import type { ImportPreviewResolvedCategoryPath } from '@/views/desktop/transactions/import/importPreview.ts';
import {
    buildImportPreviewUpdateFromTransaction,
    getPreviewIdFromImportTransaction,
    getPreviewRowVersionFromImportTransaction,
    getPreviewUpdateId,
    rebaseImportPreviewTextSyncConflict,
    syncPreviewRowVersionToImportTransaction
} from '@/views/desktop/transactions/import/importPreviewUpdates.ts';
import type { ImportPreviewTransactionDraft } from '@/views/desktop/transactions/import/importPreviewTransaction.ts';

const categoryPath: ImportPreviewResolvedCategoryPath = {
    id: '11',
    mainCategory: '餐饮',
    subCategory: '咖啡',
    displayCategory: '咖啡',
    type: TransactionType.Expense
};

const baseline = {
    selected: true,
    type: TransactionType.Transfer,
    categoryId: '',
    sourceAmountCents: 1234,
    destinationAmountCents: 5678,
    sourceAccountId: '101',
    destinationAccountId: '202',
    tagIds: [],
    counterparty: '',
    paymentMethod: '',
    comment: '',
    isManuallyAnnotated: false,
    recurringTemplateId: '44',
    recurringTemplateName: '房租',
    recurringCandidateCount: 2,
    recurringMatchScore: 0.8,
    recurringMatchReasons: '周期相似',
    recurringMatchedDate: '2026-06-01'
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
        sourceAmountCents: 1234,
        destinationAmountCents: 5678,
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
    transaction._rowVersion = 7;
    return transaction;
}

describe('import preview update helper', () => {
    test('builds a category-only patch from a server-paged draft delta', () => {
        const update = buildImportPreviewUpdateFromTransaction(makeTransaction({
            categoryId: '11',
            isManuallyAnnotated: true
        }), {
            categoryPath,
            baseline
        });

        expect(update).toEqual({
            id: 42,
            expected_row_version: 7,
            category_id: 11,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡',
            is_manually_annotated: true
        });
    });

    test('keeps an untouched server-paged action target free of nullable identity fields', () => {
        const transaction = makeTransaction({ categoryId: '' });
        const update = buildImportPreviewUpdateFromTransaction(transaction, {
            categoryPath: null,
            baseline
        });

        expect(update).toEqual({
            id: 42,
            expected_row_version: 7
        });
        expect(update).not.toHaveProperty('category_id');
        expect(update).not.toHaveProperty('preview_source_account_id');
        expect(update).not.toHaveProperty('preview_destination_account_id');
    });

    test('emits every changed server-paged field without replaying unchanged nullable identities', () => {
        const transaction = makeTransaction({
            type: TransactionType.Investment,
            categoryId: '11',
            sourceAmountCents: 4321,
            destinationAmountCents: 8765,
            sourceAccountId: '303',
            destinationAccountId: '404',
            counterparty: 'changed merchant',
            paymentMethod: 'changed payment',
            comment: 'changed comment',
            recurringTemplateId: '55',
            recurringTemplateName: 'changed recurring',
            recurringCandidateCount: 3,
            recurringMatchScore: 0.9,
            recurringMatchReasons: 'changed reason',
            recurringMatchedDate: '2026-07-01',
            selected: false,
            isManuallyAnnotated: true
        });

        const update = buildImportPreviewUpdateFromTransaction(transaction, {
            categoryPath,
            baseline,
            validAccountIds: new Set(['303', '404']),
            clearTransferDecision: true,
            clearLearningDecision: true,
            clearLlmDecision: true,
            includeSuggestionDecisionClears: true
        });

        expect(update).toEqual({
            id: 42,
            expected_row_version: 7,
            preview_type: '投资',
            preview_amount_cents: 4321,
            preview_destination_amount_cents: 8765,
            category_id: 11,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡',
            preview_source_account_id: 303,
            preview_destination_account_id: 404,
            preview_counterparty: 'changed merchant',
            preview_payment_method: 'changed payment',
            preview_description: 'changed comment',
            preview_recurring_id: 55,
            preview_recurring_name: 'changed recurring',
            preview_recurring_candidate_count: 3,
            preview_recurring_match_score: 0.9,
            preview_recurring_match_reasons: 'changed reason',
            preview_recurring_matched_date: '2026-07-01',
            selected: false,
            is_manually_annotated: true,
            clear_transfer_decision: true,
            clear_learning_decision: true,
            clear_llm_decision: true,
            clear_actionable_suggestions: ['transfer', 'learning', 'llm']
        });
    });

    test('rejects a server-paged patch target without a preview id', () => {
        const transaction = makeTransaction();
        transaction._previewId = 0;

        expect(() => buildImportPreviewUpdateFromTransaction(transaction, {
            categoryPath,
            baseline
        })).toThrow('Import preview row id is required.');
    });

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
            expected_row_version: 7,
            preview_type: '转账',
            preview_amount_cents: 1234,
            preview_destination_amount_cents: 5678,
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

    test('marks preview updates as manually annotated only after user edits', () => {
        const update = buildImportPreviewUpdateFromTransaction(makeTransaction({
            isManuallyAnnotated: true
        }), {
            categoryPath
        });

        expect(update.is_manually_annotated).toBe(true);
    });

    test.each([
        ['decimal number', { sourceAmountCents: 12.34 }],
        ['decimal string', { sourceAmountCents: '12.34' as unknown as number }],
        ['boolean', { sourceAmountCents: true as unknown as number }],
        ['object', { sourceAmountCents: {} as unknown as number }]
    ])('rejects non-integer source cents in preview update payloads: %s', (_name, overrides) => {
        expect(() => buildImportPreviewUpdateFromTransaction(makeTransaction(overrides), {
            categoryPath
        })).toThrow('sourceAmountCents must be integer cents.');
    });

    test.each([
        ['decimal number', { destinationAmountCents: 12.34 }],
        ['decimal string', { destinationAmountCents: '12.34' as unknown as number }],
        ['boolean', { destinationAmountCents: true as unknown as number }],
        ['object', { destinationAmountCents: {} as unknown as number }]
    ])('rejects non-integer destination cents in preview update payloads: %s', (_name, overrides) => {
        expect(() => buildImportPreviewUpdateFromTransaction(makeTransaction(overrides), {
            categoryPath
        })).toThrow('destinationAmountCents must be integer cents.');
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

    test('requires positive integer row versions from preview drafts', () => {
        const transaction = makeTransaction();
        transaction._rowVersion = 9;
        expect(getPreviewRowVersionFromImportTransaction(transaction)).toBe(9);

        for (const invalidVersion of [0, -1, 1.5, Number.NaN]) {
            transaction._rowVersion = invalidVersion;
            expect(() => getPreviewRowVersionFromImportTransaction(transaction))
                .toThrow('Import preview row version is required.');
        }

        syncPreviewRowVersionToImportTransaction(transaction, { id: 42, row_version: 12 });
        expect(transaction._rowVersion).toBe(12);
        syncPreviewRowVersionToImportTransaction(transaction, { id: 42, row_version: 0 });
        expect(transaction._rowVersion).toBe(12);
    });

    test('always adds a valid row version and rejects legacy preview drafts', () => {
        const versioned = makeTransaction();
        versioned._rowVersion = 9;
        expect(buildImportPreviewUpdateFromTransaction(versioned, {
            categoryPath
        }))
            .toMatchObject({ id: 42, expected_row_version: 9 });

        const legacy = makeTransaction();
        legacy._rowVersion = undefined;
        expect(() => buildImportPreviewUpdateFromTransaction(legacy, { categoryPath }))
            .toThrow('Import preview row version is required.');
        legacy._rowVersion = 0;
        expect(() => buildImportPreviewUpdateFromTransaction(legacy, { categoryPath }))
            .toThrow('Import preview row version is required.');
    });

    test('rebases learning text sync fields from the authoritative conflict row', () => {
        const transaction = makeTransaction({
            counterparty: 'stale counterparty',
            paymentMethod: 'stale payment',
            comment: 'stale comment',
            selected: true
        });

        rebaseImportPreviewTextSyncConflict(transaction, {
            id: 42,
            row_version: 9,
            preview_counterparty: '',
            preview_payment_method: 'server payment',
            preview_description: 'server comment',
            preview_selected: false
        });

        expect(transaction.counterparty).toBe('');
        expect(transaction.paymentMethod).toBe('server payment');
        expect(transaction.comment).toBe('server comment');
        expect(transaction.selected).toBe(false);
        expect(getPreviewRowVersionFromImportTransaction(transaction)).toBe(9);
    });
});
