import { describe, expect, test } from '@jest/globals';

import type { ImportTransaction } from '@/models/imported_transaction.ts';
import {
    applyImportPreviewEditableDraftOverlay,
    mergeImportPreviewEditableDraftBaseline,
    type ImportPreviewEditableDraftState
} from '@/views/desktop/transactions/import/check-data-tab/serverPagedDraftState.ts';
import {
    clearResolvedImportPreviewReviewState
} from '@/views/desktop/transactions/import/importPreviewReviewState.ts';

function editableState(
    overrides: Partial<ImportPreviewEditableDraftState> = {}
): ImportPreviewEditableDraftState {
    return {
        selected: true,
        type: 3,
        categoryId: '',
        sourceAmountCents: 1_000,
        destinationAmountCents: 1_000,
        sourceAccountId: '11',
        destinationAccountId: '',
        tagIds: [],
        counterparty: '',
        paymentMethod: '',
        comment: 'server value',
        isManuallyAnnotated: false,
        recurringTemplateId: '',
        recurringTemplateName: '',
        recurringCandidateCount: 0,
        recurringMatchScore: 0,
        recurringMatchReasons: '',
        recurringMatchedDate: '',
        ...overrides
    };
}

describe('server-paged import preview draft acknowledgement', () => {
    test('converges an acknowledged field while retaining an unrelated unacknowledged overlay', () => {
        const baseline = editableState();
        const draft = editableState({
            categoryId: '8',
            comment: 'local draft'
        });
        const server = editableState({
            categoryId: '8'
        });

        const nextBaseline = mergeImportPreviewEditableDraftBaseline(server, draft, baseline);

        expect(nextBaseline.categoryId).toBe('8');
        expect(nextBaseline.comment).toBe('server value');
    });

    test('reconciles authoritative review state after applying a provisional category overlay', () => {
        const baseline = editableState();
        const draft = editableState({ categoryId: '8' });
        const transaction = {
            ...editableState(),
            matching: {
                annotation: {
                    review_status: 'requires_identity_review',
                    is_manually_annotated: true
                },
                identity_validation: {
                    review_status: 'requires_identity_review',
                    issues: [{ field: 'category_id', reason: 'missing' }]
                }
            }
        } as unknown as ImportTransaction;

        applyImportPreviewEditableDraftOverlay(transaction, draft, baseline, item => {
            item.matching = clearResolvedImportPreviewReviewState(
                item.matching as unknown as Record<string, unknown>,
                {
                    hasMissingCategoryIssue: item.categoryId === '',
                    hasMissingSourceAccountIssue: false,
                    hasMissingDestinationAccountIssue: false,
                    hasTransferAccountReviewIssue: false
                }
            ) as typeof item.matching;
        });

        expect(transaction.categoryId).toBe('8');
        expect(transaction.matching).toEqual({
            annotation: { is_manually_annotated: true }
        });
    });
});
