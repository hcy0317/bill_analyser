import { describe, expect, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import {
    buildImportCheckDecisionExpectedState,
    buildImportCheckLearningDecisionExpectedState
} from '@/views/desktop/transactions/import/checkDataCandidateReview.ts';

describe('checkDataCandidateReview helpers', () => {
    test('builds generic matching expectedState with the supplied review status family', () => {
        expect(buildImportCheckDecisionExpectedState({
            sessionId: 'session-investment-review',
            rowVersion: 7,
            reviewStatus: 'accepted',
            type: TransactionType.Investment,
            categoryId: '42',
            recurringTemplateId: ''
        })).toStrictEqual({
            sessionId: 'session-investment-review',
            rowVersion: 7,
            reviewStatus: 'accepted',
            previewType: '投资',
            categoryId: 42,
            recurringId: null,
            sourceAccountId: null,
            destinationAccountId: null
        });
    });

    test('builds learning expectedState with source and destination accounts', () => {
        expect(buildImportCheckLearningDecisionExpectedState({
            sessionId: 'session-learning-review',
            rowVersion: 12,
            reviewStatus: 'pending',
            type: TransactionType.Transfer,
            categoryId: '',
            recurringTemplateId: '18',
            sourceAccountId: '0',
            destinationAccountId: '19'
        })).toStrictEqual({
            sessionId: 'session-learning-review',
            rowVersion: 12,
            reviewStatus: 'pending',
            previewType: '转账',
            categoryId: null,
            recurringId: 18,
            sourceAccountId: null,
            destinationAccountId: 19
        });
    });

    test('rejects an invalid row version before building expectedState', () => {
        expect(() => buildImportCheckLearningDecisionExpectedState({
            sessionId: 'session-learning-legacy',
            rowVersion: 0,
            reviewStatus: 'pending',
            type: TransactionType.Expense
        })).toThrow('Import preview row version is required.');
    });
});
