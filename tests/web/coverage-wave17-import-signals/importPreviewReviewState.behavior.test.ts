import { describe, expect, test } from '@jest/globals';

import {
    clearResolvedImportPreviewReviewState,
    type ImportPreviewReviewIssueState
} from '@/views/desktop/transactions/import/importPreviewReviewState.ts';

const resolvedIssues: ImportPreviewReviewIssueState = {
    hasMissingCategoryIssue: false,
    hasMissingSourceAccountIssue: false,
    hasMissingDestinationAccountIssue: false,
    hasTransferAccountReviewIssue: false
};

describe('importPreviewReviewState exhaustive review cleanup', () => {
    test('preserves non-record matching values and ignores malformed review payloads', () => {
        expect(clearResolvedImportPreviewReviewState(undefined, resolvedIssues)).toBeUndefined();
        expect(clearResolvedImportPreviewReviewState(null as never, resolvedIssues)).toBeNull();
        expect(clearResolvedImportPreviewReviewState([] as never, resolvedIssues)).toEqual([]);

        expect(clearResolvedImportPreviewReviewState({
            annotation: 9,
            identity_validation: { issues: 'not-an-array' },
            parser: { id: 'wechat' }
        }, resolvedIssues)).toEqual({
            annotation: 9,
            identity_validation: { issues: 'not-an-array' },
            parser: { id: 'wechat' }
        });
    });

    test('removes every resolved identity alias while retaining malformed and unknown issues', () => {
        const resolvedAliases = [
            'category', 'category_id', 'preview_category_id',
            'account', 'account_id', 'source_account', 'source_account_id', 'preview_source_account_id',
            'destination_account', 'destination_account_id', 'transfer_target_account_id',
            'preview_destination_account_id', 'transfer_accounts', 'same_transfer_account',
            'same_transfer_accounts', 'transfer_account_direction'
        ];
        const matching = clearResolvedImportPreviewReviewState({
            identity_validation: {
                marker: 'keep',
                issues: [
                    ...resolvedAliases.map(field => ({ field: ` ${field.toUpperCase()} ` })),
                    'malformed',
                    { field: 'unknown_field', reason: 'keep' },
                    { field: 123, reason: 'keep' }
                ]
            }
        }, resolvedIssues);

        expect(matching).toEqual({
            identity_validation: {
                marker: 'keep',
                issues: [
                    'malformed',
                    { field: 'unknown_field', reason: 'keep' },
                    { field: 123, reason: 'keep' }
                ]
            }
        });
    });

    test('retains each identity family while its corresponding issue remains unresolved', () => {
        const issueCases = [
            ['category_id', 'hasMissingCategoryIssue'],
            ['source_account_id', 'hasMissingSourceAccountIssue'],
            ['destination_account_id', 'hasMissingDestinationAccountIssue'],
            ['same_transfer_accounts', 'hasTransferAccountReviewIssue']
        ] as const;

        for (const [field, issueKey] of issueCases) {
            const issues = { ...resolvedIssues, [issueKey]: true };
            expect(clearResolvedImportPreviewReviewState({
                identity_validation: { issues: [{ field }] }
            }, issues)).toEqual({
                identity_validation: { issues: [{ field }] }
            });
        }
    });

    test.each([
        'category', 'category_missing', 'missing_category', 'missing-category', 'missing_classification',
        'account', 'missing_account', 'source_account', 'source_account_missing',
        'missing_source_account', 'missing-source-account',
        'destination_account', 'destination_account_missing', 'missing_destination_account',
        'missing-destination-account', 'transfer_account_direction', 'transfer_accounts',
        'review_transfer_accounts', 'same_transfer_account', 'same_transfer_accounts'
    ])('removes resolved string annotation alias %s', alias => {
        expect(clearResolvedImportPreviewReviewState({ annotation: ` ${alias.toUpperCase()} ` }, resolvedIssues))
            .toBeUndefined();
    });

    test('covers blank, unknown, object-key, and manual annotation preservation branches', () => {
        expect(clearResolvedImportPreviewReviewState({ annotation: '   ' }, resolvedIssues))
            .toEqual({ annotation: '   ' });
        expect(clearResolvedImportPreviewReviewState({ annotation: 'unknown' }, resolvedIssues))
            .toEqual({ annotation: 'unknown' });

        expect(clearResolvedImportPreviewReviewState({
            annotation: {
                type: 7,
                status: 'missing_category',
                review_status: '',
                reason: null,
                is_manually_annotated: true,
                stale: 'remove'
            }
        }, resolvedIssues)).toEqual({
            annotation: { is_manually_annotated: true }
        });

        expect(clearResolvedImportPreviewReviewState({
            annotation: { reason: 'missing_account', is_manually_annotated: false }
        }, resolvedIssues)).toBeUndefined();
    });

    test('requires every identity issue to resolve before clearing the aggregate annotation', () => {
        expect(clearResolvedImportPreviewReviewState({
            annotation: { review_status: 'requires_identity_review' }
        }, resolvedIssues)).toBeUndefined();

        for (const issueKey of Object.keys(resolvedIssues) as Array<keyof ImportPreviewReviewIssueState>) {
            expect(clearResolvedImportPreviewReviewState({
                annotation: { review_status: 'requires_identity_review' }
            }, { ...resolvedIssues, [issueKey]: true })).toEqual({
                annotation: { review_status: 'requires_identity_review' }
            });
        }
    });
});
