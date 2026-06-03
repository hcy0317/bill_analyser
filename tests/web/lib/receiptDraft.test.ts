import { describe, expect, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import {
    applyReceiptDraftAutoFillToTransaction,
    applyReceiptDraftFieldToTransaction,
    buildReceiptDraftCandidateHints,
    getReceiptDraftCandidateDisplayValue,
    receiptDraftFieldStringArrayValue,
    receiptDraftFieldStringValue,
    receiptDraftAmountToCents,
    receiptDraftTimeToUnixSeconds,
    transactionTypeFromReceiptDraftValue
} from '@/lib/receiptDraft.ts';
import type { ReceiptDraftField, RecognizedReceiptImageResponse } from '@/models/large_language_model.ts';

class EditableTransactionFixture {
    public type = TransactionType.Expense;
    public sourceAmount = 0;
    public time = 0;
    public comment = '';
    public categoryId = '';
    public sourceAccountId = '';
    public destinationAccountId = '';
    public tagIds: string[] = [];

    public setCategoryId(categoryId: string): void {
        this.categoryId = categoryId;
    }
}

function draftField<T>(value: T, confidence = 0.9, extra: Partial<ReceiptDraftField<T>> = {}): ReceiptDraftField<T> {
    return {
        value,
        confidence,
        reason: 'test',
        evidence: ['fixture'],
        ...extra
    };
}

describe('receiptDraft helper', () => {
    test('converts receipt draft amount units explicitly', () => {
        expect(receiptDraftAmountToCents(draftField(12.34, 0.95, { unit: 'yuan' }))).toBe(1234);
        expect(receiptDraftAmountToCents(draftField(1234, 0.95, { unit: 'cents' }))).toBe(1234);
        expect(receiptDraftAmountToCents(undefined)).toBeUndefined();
    });

    test('coerces display values and rejects unusable primitive fields', () => {
        expect(receiptDraftFieldStringValue(draftField(77))).toBe('77');
        expect(receiptDraftFieldStringValue(draftField(''))).toBeUndefined();
        expect(receiptDraftFieldStringArrayValue(draftField('tag-1'))).toBeUndefined();
        expect(receiptDraftTimeToUnixSeconds(undefined)).toBeUndefined();
        expect(receiptDraftTimeToUnixSeconds(draftField('not a date'))).toBeUndefined();
        expect(getReceiptDraftCandidateDisplayValue(draftField(['tag-1', 'tag-2']))).toBe('tag-1, tag-2');
        expect(getReceiptDraftCandidateDisplayValue(draftField(12.3))).toBe('12.3');
    });

    test('maps receipt draft transaction type values to frontend enum', () => {
        expect(transactionTypeFromReceiptDraftValue('expense')).toBe(TransactionType.Expense);
        expect(transactionTypeFromReceiptDraftValue('income')).toBe(TransactionType.Income);
        expect(transactionTypeFromReceiptDraftValue('transfer')).toBe(TransactionType.Transfer);
        expect(transactionTypeFromReceiptDraftValue(TransactionType.Investment)).toBe(TransactionType.Investment);
        expect(transactionTypeFromReceiptDraftValue('unknown')).toBeUndefined();
    });

    test('applies high-confidence auto-fill while preserving low-confidence candidates', () => {
        const transaction = new EditableTransactionFixture();
        const result: RecognizedReceiptImageResponse = {
            amount: 99,
            tradeTime: '2026-05-01T00:00:00Z',
            description: 'fallback',
            paymentPlatform: null,
            provenance: { provider: 'local_json_ocr', requestId: 'req-1' },
            confidence: 0.87,
            draft: {
                autoFill: {
                    type: draftField('income'),
                    amount: draftField(88.5, 0.95, { unit: 'yuan' }),
                    time: draftField('2026-05-02T03:04:05Z'),
                    description: draftField('payroll'),
                    categoryId: draftField('cat-1'),
                    sourceAccountId: draftField('account-1'),
                    destinationAccountId: draftField('account-2'),
                    tagIds: draftField(['tag-1', 'tag-1', 'tag-2'])
                },
                candidates: {
                    categoryId: [draftField('cat-2', 0.6, { label: 'Candidate Category' })]
                }
            }
        };

        applyReceiptDraftAutoFillToTransaction(transaction, result);

        expect(transaction.type).toBe(TransactionType.Income);
        expect(transaction.sourceAmount).toBe(8850);
        expect(transaction.time).toBe(Math.floor(Date.parse('2026-05-02T03:04:05Z') / 1000));
        expect(transaction.comment).toBe('payroll');
        expect(transaction.categoryId).toBe('cat-1');
        expect(transaction.sourceAccountId).toBe('account-1');
        expect(transaction.destinationAccountId).toBe('account-2');
        expect(transaction.tagIds).toStrictEqual(['tag-1', 'tag-2']);
        expect(buildReceiptDraftCandidateHints(result.draft)).toMatchObject([
            { key: 'categoryId', titleKey: 'Category', field: { value: 'cat-2', label: 'Candidate Category' } }
        ]);
    });

    test('applies a selected candidate without requiring auto-fill', () => {
        const transaction = new EditableTransactionFixture();

        expect(applyReceiptDraftFieldToTransaction(transaction, 'tagIds', draftField(['tag-3']))).toBe(true);
        expect(transaction.tagIds).toStrictEqual(['tag-3']);
    });

    test('rejects invalid candidate fields and falls back to top-level OCR fields', () => {
        const transaction = new EditableTransactionFixture();
        const result: RecognizedReceiptImageResponse = {
            amount: 12.34,
            tradeTime: '2026-06-01T01:02:03Z',
            description: 'receipt text',
            paymentPlatform: null,
            provenance: { provider: 'tesseract', requestId: 'req-2' },
            confidence: 0.51,
            draft: {
                autoFill: {},
                candidates: {}
            }
        };

        expect(applyReceiptDraftFieldToTransaction(transaction, 'type', draftField('unknown'))).toBe(false);
        expect(applyReceiptDraftFieldToTransaction(transaction, 'amount', draftField('bad'))).toBe(false);
        expect(applyReceiptDraftFieldToTransaction(transaction, 'time', draftField('bad'))).toBe(false);
        expect(applyReceiptDraftFieldToTransaction(transaction, 'description', draftField(''))).toBe(false);
        expect(applyReceiptDraftFieldToTransaction(transaction, 'categoryId', draftField(''))).toBe(false);
        expect(applyReceiptDraftFieldToTransaction(transaction, 'sourceAccountId', draftField(''))).toBe(false);
        expect(applyReceiptDraftFieldToTransaction(transaction, 'destinationAccountId', draftField(''))).toBe(false);
        expect(applyReceiptDraftFieldToTransaction(transaction, 'tagIds', draftField([]))).toBe(false);

        applyReceiptDraftAutoFillToTransaction(transaction, result);

        expect(transaction.sourceAmount).toBe(1234);
        expect(transaction.time).toBe(Math.floor(Date.parse('2026-06-01T01:02:03Z') / 1000));
        expect(transaction.comment).toBe('receipt text');
        expect(buildReceiptDraftCandidateHints(undefined)).toStrictEqual([]);
    });
});
