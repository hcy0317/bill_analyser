import { describe, expect, test } from '@jest/globals';

const { computed } = require('vue');

import { TransactionType } from '@/core/transaction.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import {
    resolveSignalStatusAuthority,
    serializeSignalCacheValue,
    useImportCheckDataSignals
} from '@/views/desktop/transactions/import/check-data-tab/useImportCheckDataSignals.ts';

function createTransaction(matching: Record<string, unknown> = {}): ImportTransaction {
    return ImportTransaction.of({
        type: TransactionType.Expense,
        categoryId: 'food',
        originalCategoryName: '餐饮',
        time: 1_788_480_000,
        utcOffset: 480,
        sourceAccountId: 'wallet',
        originalSourceAccountName: '钱包',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '',
        tagIds: [],
        originalTagNames: [],
        sourceAmountCents: 1_234,
        comment: '午餐',
        counterparty: '测试商户',
        paymentMethod: '微信支付',
        matching: {
            parser: { id: 'wechat', tags: ['parser:wechat'] },
            learning: {
                review_status: 'pending',
                score: 0.91,
                reason: 'learned merchant'
            },
            ...matching
        }
    } as never, 4);
}

function createSignals(transaction: ImportTransaction) {
    return useImportCheckDataSignals({
        importTransactions: computed(() => [transaction]),
        translate: key => key,
        getPreviewId: item => (item as ImportTransaction & { _previewId?: number })._previewId ?? null,
        getLLMMatchingPayload: item => (item.matching as typeof item.matching & { llm?: Record<string, unknown> })?.llm || {},
        getLLMSignalStatus: item => {
            const llm = (item.matching as typeof item.matching & { llm?: Record<string, unknown> })?.llm;
            return llm?.['review_status'] === 'accepted' ? 'accepted' : (llm ? 'pending' : null);
        },
        getLLMSignalCategoryPath: payload => [payload.suggested_main_category, payload.suggested_sub_category].filter(Boolean).join('/'),
        buildLLMSignalSummary: payload => String(payload.reason || ''),
        getRecurringMatchSummary: item => item.recurringTemplateName,
        getPrimaryRecurringReason: item => item.recurringMatchReasons.split('|')[0] || '',
        formatAmountWithCurrency: (amount, currency) => `${currency}:${amount}`
    });
}

describe('useImportCheckDataSignals', () => {
    test('projects learning and LLM authority, reuses cache, and invalidates it on signal changes', () => {
        const transaction = createTransaction({
            llm: {
                review_status: 'accepted',
                suggested_main_category: '餐饮',
                suggested_sub_category: '午餐',
                reason: 'LLM accepted',
                confidence: 0.88
            }
        });
        const signals = createSignals(transaction);

        const first = signals.getImportPreviewSignalViewModel(transaction);
        const cached = signals.getImportPreviewSignalViewModel(transaction);
        expect(cached).toBe(first);
        expect(first.learning).toMatchObject({ status: 'pending' });
        expect(first.llm).toMatchObject({ status: 'accepted' });

        transaction.matching!.learning.score = 0.75;
        expect(signals.getImportPreviewSignalViewModel(transaction)).not.toBe(first);
    });

    test('builds row identities and complete history rewrite acknowledgements', () => {
        const transaction = createTransaction({
            reconciliation: {
                planned_operation: 'update_history',
                history_bill_id: 9,
                history_bill_version: 3,
                operation_id: 'operation-1',
                acknowledgement_token: 'ack-1',
                destructive_ack_required: true
            }
        }) as ImportTransaction & { _previewId?: number };
        transaction._previewId = 7;
        const signals = createSignals(transaction);

        expect(signals.getImportTransactionRowKey(transaction)).toBe('preview:7');
        expect(signals.getImportPreviewSignalViewModel(transaction)).toBeDefined();
        expect(signals.getImportPreviewHistoryRewriteOperation(transaction)).toEqual({
            preview_id: 7,
            operation_id: 'operation-1',
            planned_operation: 'update_history',
            history_bill_id: 9,
            history_bill_version: 3,
            acknowledgement_token: 'ack-1'
        });

        delete transaction._previewId;
        expect(signals.getImportTransactionRowKey(transaction)).toBe('row:4');
        expect(signals.getImportPreviewHistoryRewriteOperation(transaction)).toBeNull();
    });

    test.each([
        [false, false, false, false, false, null],
        [true, true, false, false, false, 'pending'],
        [true, false, true, false, false, 'accepted'],
        [true, false, false, true, false, 'rejected'],
        [true, false, false, false, true, 'skipped'],
        [true, false, false, false, false, null]
    ] as const)(
        'derives legacy learning status from transaction helpers',
        (hasLearning, pending, accepted, rejected, skipped, expected) => {
            const transaction = createTransaction({ learning: { reason: 'legacy signal' } });
            transaction.parserId = '';
            transaction.parserTags = [];
            transaction.hasLearningRecommendation = () => hasLearning;
            transaction.hasPendingLearningRecommendation = () => pending;
            transaction.isLearningRecommendationAccepted = () => accepted;
            transaction.isLearningRecommendationRejected = () => rejected;
            transaction.isLearningRecommendationSkipped = () => skipped;
            transaction.isTransferProtectedLearningSkip = () => false;

            const viewModel = createSignals(transaction).getImportPreviewSignalViewModel(transaction);
            if (expected !== null) {
                expect(viewModel.learning?.status).toBe(expected);
            } else {
                expect(viewModel).toBeDefined();
            }
        }
    );

    test('preserves explicit-null status authority and cache sentinel values', () => {
        expect(resolveSignalStatusAuthority(undefined, null)).toEqual({ status: null, authoritative: true });
        expect(resolveSignalStatusAuthority(undefined)).toEqual({ status: undefined, authoritative: false });
        expect(resolveSignalStatusAuthority('rejected')).toEqual({ status: 'rejected', authoritative: true });
        expect(serializeSignalCacheValue(null)).toBe('<null>');
        expect(serializeSignalCacheValue(undefined)).toBe('<undefined>');
        expect(serializeSignalCacheValue(42)).toBe('42');
    });

    test('does not expose actions for unknown authoritative learning and LLM statuses', () => {
        const transaction = createTransaction({
            learning: {
                review_status: '__future_unknown__',
                score: 0.91,
                reason: 'learning evidence'
            },
            llm: {
                review_status: '__future_unknown__',
                confidence: 0.88,
                reason: 'LLM evidence'
            }
        });

        const viewModel = createSignals(transaction).getImportPreviewSignalViewModel(transaction);

        expect(viewModel.learning).toBeNull();
        expect(viewModel.llm).toBeNull();
    });

    test('does not expose a history rewrite for an unknown reconciliation status', () => {
        const transaction = createTransaction({
            reconciliation: {
                status: '__future_unknown__',
                planned_operation: 'update_history',
                destructive_ack_required: true,
                history_bill_id: 9
            }
        });

        const viewModel = createSignals(transaction).getImportPreviewSignalViewModel(transaction);

        expect(viewModel.historyRewrite).toBeNull();
    });
});
