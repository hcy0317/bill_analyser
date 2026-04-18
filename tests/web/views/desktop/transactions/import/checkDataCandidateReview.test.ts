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
            reviewStatus: 'accepted',
            type: TransactionType.Investment,
            categoryId: '42',
            recurringTemplateId: ''
        })).toStrictEqual({
            sessionId: 'session-investment-review',
            reviewStatus: 'accepted',
            previewType: '投资',
            categoryId: 42,
            recurringId: null
        });
    });

    test('builds learning expectedState with source and destination accounts', () => {
        expect(buildImportCheckLearningDecisionExpectedState({
            sessionId: 'session-learning-review',
            reviewStatus: 'pending',
            type: TransactionType.Transfer,
            categoryId: '',
            recurringTemplateId: '18',
            sourceAccountId: '11',
            destinationAccountId: '19'
        })).toStrictEqual({
            sessionId: 'session-learning-review',
            reviewStatus: 'pending',
            previewType: '转账',
            categoryId: null,
            recurringId: 18,
            sourceAccountId: 11,
            destinationAccountId: 19
        });
    });
});
