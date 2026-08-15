import { describe, expect, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import {
    ImportTransaction,
    type ImportTransactionResponse
} from '@/models/imported_transaction.ts';
import type { ImportMatchingPayload } from '@/models/import_matching.ts';
import {
    getImportPreviewTransferSignalStatus,
    getImportPreviewTransferSignalTitle
} from '@/views/desktop/transactions/import/importPreviewSignalAdapter.ts';

const baseResponse: ImportTransactionResponse = {
    type: TransactionType.Expense,
    categoryId: '101',
    originalCategoryName: '餐饮',
    time: 1711785600,
    utcOffset: 480,
    sourceAccountId: '201',
    originalSourceAccountName: '微信零钱',
    originalSourceAccountCurrency: 'CNY',
    destinationAccountId: '',
    originalDestinationAccountName: '',
    originalDestinationAccountCurrency: '',
    sourceAmountCents: 1200,
    destinationAmountCents: 0,
    tagIds: [],
    originalTagNames: [],
    comment: '工作日午餐'
};

function matchingPayload(overrides: Partial<ImportMatchingPayload> = {}): ImportMatchingPayload {
    return {
        transfer: {
            candidate_type: '',
            score: 0,
            level: '',
            reason: '',
            review_status: '',
            reviewed_type: '',
            suppressed: false
        },
        investment: {
            score: 0,
            level: '',
            reason: '',
            platform: '',
            product: ''
        },
        learning: {
            rule_id: null,
            score: 0,
            level: '',
            reason: '',
            recommended_type: '',
            summary: ''
        },
        recurring: {
            id: null,
            name: '',
            candidate_count: 0,
            match_score: 0,
            match_reasons: '',
            matched_date: ''
        },
        dedup: {
            type: '',
            source_ids: []
        },
        parser: {
            id: '',
            tags: []
        },
        annotation: {
            is_manually_annotated: false
        },
        ...overrides
    };
}

function importTransaction(response: Partial<ImportTransactionResponse>): ImportTransaction {
    return ImportTransaction.of({
        ...baseResponse,
        ...response
    }, 1);
}

describe('import preview signal adapter', () => {
    test('shows structured transfer matching feedback as a transfer signal', () => {
        const transaction = importTransaction({
            type: TransactionType.Transfer,
            matching: matchingPayload({
                transfer: {
                    candidate_type: 'cash_transfer',
                    score: 0,
                    level: '',
                    reason: '同金额双边匹配',
                    review_status: 'pending',
                    reviewed_type: '',
                    suppressed: false
                }
            })
        });

        expect(getImportPreviewTransferSignalStatus(transaction)).toBe('pending');
        expect(getImportPreviewTransferSignalTitle(transaction)).toBe('同金额双边匹配');
    });

    test('does not treat a plain transfer transaction type as a transfer signal', () => {
        const transaction = importTransaction({
            type: TransactionType.Transfer,
            matching: undefined
        });

        expect(getImportPreviewTransferSignalStatus(transaction)).toBeNull();
        expect(getImportPreviewTransferSignalTitle(transaction)).toBe('');
    });

    test('does not treat transfer-like dedup context as transfer matching feedback', () => {
        const transaction = importTransaction({
            type: TransactionType.Transfer,
            matching: matchingPayload({
                dedup: {
                    type: 'transfer',
                    source_ids: [101, 102]
                }
            })
        });

        expect(getImportPreviewTransferSignalStatus(transaction)).toBeNull();
    });

    test('hides suppressed pending transfer feedback', () => {
        const transaction = importTransaction({
            matching: matchingPayload({
                transfer: {
                    candidate_type: 'cash_transfer',
                    score: 0.9,
                    level: 'high',
                    reason: '已抑制',
                    review_status: 'pending',
                    reviewed_type: '',
                    suppressed: true
                }
            })
        });

        expect(transaction.hasTransferSuggestion()).toBe(false);
        expect(getImportPreviewTransferSignalStatus(transaction)).toBeNull();
    });

    test('fails closed for an unknown non-empty transfer review status', () => {
        const transaction = importTransaction({
            matching: matchingPayload({
                transfer: {
                    candidate_type: 'cash_transfer',
                    score: 0.9,
                    level: 'high',
                    reason: 'future lifecycle evidence',
                    review_status: '__future_unknown__',
                    reviewed_type: '',
                    suppressed: false
                }
            })
        });

        expect(transaction.hasTransferSuggestion()).toBe(true);
        expect(getImportPreviewTransferSignalStatus(transaction)).toBeNull();
        expect(getImportPreviewTransferSignalTitle(transaction)).toBe('future lifecycle evidence');
    });
});
