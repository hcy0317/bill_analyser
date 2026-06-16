import { describe, expect, test } from '@jest/globals';

import {
    clearResolvedImportPreviewReviewState
} from '@/views/desktop/transactions/import/importPreviewReviewState.ts';

describe('import preview review state helpers', () => {
    test('clears resolved identity validation and annotation state before a refetch', () => {
        const matching = clearResolvedImportPreviewReviewState({
            annotation: {
                status: 'missing_category',
                is_manually_annotated: true,
            },
            identity_validation: {
                review_status: 'requires_identity_review',
                issues: [
                    { field: 'category_id', reason: 'missing' },
                    { field: 'source_account_id', reason: 'missing' },
                    { field: 'destination_account_id', reason: 'missing' },
                    { field: 'same_transfer_account', reason: 'same_account' },
                ],
            },
            parser: { id: 'alipay' },
        }, {
            hasMissingCategoryIssue: false,
            hasMissingSourceAccountIssue: false,
            hasMissingDestinationAccountIssue: false,
            hasTransferAccountReviewIssue: false,
        });

        expect(matching).toEqual({
            annotation: {
                is_manually_annotated: true,
            },
            parser: { id: 'alipay' },
        });
    });

    test('keeps unresolved review issues while dropping only fields fixed by local edits', () => {
        const matching = clearResolvedImportPreviewReviewState({
            annotation: {
                type: 'missing_source_account',
                status: 'needs_review',
            },
            identity_validation: {
                issues: [
                    { field: 'source_account_id', reason: 'missing' },
                    { field: 'destination_account_id', reason: 'missing' },
                ],
            },
        }, {
            hasMissingCategoryIssue: false,
            hasMissingSourceAccountIssue: false,
            hasMissingDestinationAccountIssue: true,
            hasTransferAccountReviewIssue: false,
        });

        expect(matching).toEqual({
            identity_validation: {
                issues: [
                    { field: 'destination_account_id', reason: 'missing' },
                ],
            },
        });
    });
});
