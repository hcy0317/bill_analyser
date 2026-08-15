import { describe, expect, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import { useImportCheckDataAnnotations } from '@/views/desktop/transactions/import/check-data-tab/useImportCheckDataAnnotations.ts';

function createTransaction(overrides: Record<string, unknown> = {}): ImportTransaction {
    return ImportTransaction.of({
        type: TransactionType.Transfer,
        categoryId: '',
        originalCategoryName: '',
        time: 1_788_480_000,
        utcOffset: 480,
        sourceAccountId: '',
        originalSourceAccountName: '',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '',
        tagIds: [],
        originalTagNames: [],
        sourceAmountCents: 500,
        destinationAmountCents: 500,
        comment: '',
        counterparty: '对方',
        paymentMethod: '',
        selected: true,
        matching: {},
        ...overrides
    } as never, Number(overrides['index'] || 0));
}

function createAnnotations(transactions: ImportTransaction[]) {
    return useImportCheckDataAnnotations({
        translate: key => key,
        isTransactionCategoryAccepted: item => item.categoryId !== '',
        isKnownAccountId: accountId => ['source', 'destination'].includes(String(accountId || '')),
        requiresDestinationAccount: item => item.type === TransactionType.Transfer || item.type === TransactionType.Investment,
        getTrackedTransactionsForSelection: () => transactions,
        getDisplayDateTime: item => `date-${item.index}`
    });
}

describe('useImportCheckDataAnnotations', () => {
    test('collects missing identity reasons and selection summaries', () => {
        const transaction = createTransaction();
        const annotations = createAnnotations([transaction]);

        expect(annotations.getAnnotationSummary(transaction)).toBe(
            'Missing Category · Missing Source Account · Missing Destination Account'
        );
        expect(annotations.needsAnnotation(transaction)).toBe(true);
        expect(annotations.importTransactionSelectionSummary.value).toMatchObject({
            selectedCount: 1,
            selectedTransferCount: 1,
            selectedAnnotationCount: 1,
            annotationCount: 1
        });
        expect(annotations.getAnnotationListTitle(transaction)).toBe('date-0 · 对方');
    });

    test('flags same transfer accounts and clears resolved typed annotations', () => {
        const sameAccounts = createTransaction({
            categoryId: 'transfer',
            sourceAccountId: 'source',
            destinationAccountId: 'source'
        });
        const resolved = createTransaction({
            index: 1,
            categoryId: 'food',
            sourceAccountId: 'source',
            destinationAccountId: 'destination',
            matching: { annotation: { type: 'missing_category' } }
        });
        resolved.matching!.annotation.type = 'missing_category';
        const annotations = createAnnotations([sameAccounts, resolved]);

        expect(annotations.hasTransferAccountReviewIssue(sameAccounts)).toBe(true);
        expect(annotations.hasCurrentAnnotationIssue(sameAccounts)).toBe(true);
        expect(annotations.hasCurrentAnnotationIssue(resolved)).toBe(false);
        expect(annotations.hasBaselineAnnotationIssue(resolved)).toBe(false);
    });

    test.each([
        ['missing_category', { categoryId: '', sourceAccountId: 'source', destinationAccountId: 'destination' }],
        ['missing_source_account', { categoryId: 'transfer', sourceAccountId: '', destinationAccountId: 'destination' }],
        ['missing_destination_account', { categoryId: 'transfer', sourceAccountId: 'source', destinationAccountId: '' }],
        ['review_transfer_accounts', { categoryId: 'transfer', sourceAccountId: 'source', destinationAccountId: 'source' }]
    ])('keeps unresolved persisted %s annotations', (annotationType, overrides) => {
        const transaction = createTransaction(overrides);
        transaction.matching!.annotation.type = annotationType;
        const annotations = createAnnotations([transaction]);
        expect(annotations.hasCurrentPersistedMatchingAnnotationIssue(transaction)).toBe(true);
    });

    test('retains unknown persisted annotations and uses the no-description fallback', () => {
        const transaction = createTransaction({
            type: TransactionType.ModifyBalance,
            categoryId: '',
            sourceAccountId: 'source',
            counterparty: '',
            matching: { annotation: { type: 'manual_review' } }
        });
        transaction.matching!.annotation.type = 'manual_review';
        const annotations = createAnnotations([transaction]);

        expect(annotations.hasMissingCategoryIssue(transaction)).toBe(false);
        expect(annotations.hasMissingSourceAccountIssue(transaction)).toBe(false);
        expect(annotations.hasMissingDestinationAccountIssue(transaction)).toBe(false);
        expect(annotations.hasCurrentAnnotationIssue(transaction)).toBe(true);
        expect(annotations.getAnnotationListTitle(transaction)).toBe('date-0 · No description');

        transaction.matching!.annotation = ' manual_review ' as never;
        expect(annotations.hasCurrentPersistedMatchingAnnotationIssue(transaction)).toBe(true);

        transaction.matching!.annotation = { is_manually_annotated: false };
        expect(annotations.hasCurrentPersistedMatchingAnnotationIssue(transaction)).toBe(false);
    });

    test('marks unknown signal statuses for review without discarding their evidence', () => {
        const transaction = createTransaction({
            categoryId: 'transfer',
            sourceAccountId: 'source',
            destinationAccountId: 'destination',
            matching: {
                transfer: {
                    review_status: '__future_unknown__',
                    candidate_type: 'cash_transfer',
                    reason: 'transfer evidence'
                },
                learning: {
                    review_status: '__future_unknown__',
                    reason: 'learning evidence'
                },
                llm: {
                    review_status: '__future_unknown__',
                    reason: 'LLM evidence'
                },
                reconciliation: {
                    status: '__future_unknown__',
                    planned_operation: 'update_history',
                    reason: 'history evidence'
                }
            }
        });
        const annotations = createAnnotations([transaction]);

        expect(annotations.getAnnotationIssues(transaction)).toStrictEqual(['Unknown Signal State']);
        expect(annotations.needsAnnotation(transaction)).toBe(true);
        expect(annotations.importTransactionSelectionSummary.value.selectedAnnotationCount).toBe(1);
        expect(transaction.matching?.transfer.reason).toBe('transfer evidence');
        expect(transaction.matching?.learning.reason).toBe('learning evidence');
        expect(transaction.matching?.llm?.reason).toBe('LLM evidence');
        expect(transaction.matching?.reconciliation?.reason).toBe('history evidence');
    });

    test('marks an isolated unknown history decision for review', () => {
        const transaction = createTransaction({
            categoryId: 'transfer',
            sourceAccountId: 'source',
            destinationAccountId: 'destination',
            matching: {
                reconciliation: {
                    status: '__future_unknown__',
                    planned_operation: 'update_history'
                }
            }
        });

        expect(createAnnotations([transaction]).getAnnotationIssues(transaction))
            .toStrictEqual(['Unknown Signal State']);
    });
});
