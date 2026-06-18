export interface ImportPreviewReviewIssueState {
    hasMissingCategoryIssue: boolean;
    hasMissingSourceAccountIssue: boolean;
    hasMissingDestinationAccountIssue: boolean;
    hasTransferAccountReviewIssue: boolean;
}

type ReviewStateRecord = Record<string, unknown>;

function isRecord(value: unknown): value is ReviewStateRecord {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizedText(value: unknown): string {
    return typeof value === 'string' ? value.trim().toLowerCase() : '';
}

function annotationReviewKeys(annotation: unknown): string[] {
    if (typeof annotation === 'string') {
        const value = annotation.trim().toLowerCase();
        return value ? [value] : [];
    }

    if (!isRecord(annotation)) {
        return [];
    }

    return [
        annotation['type'],
        annotation['status'],
        annotation['review_status'],
        annotation['reason']
    ].map(normalizedText).filter(value => value !== '');
}

function preserveManualAnnotationFlag(annotation: unknown): ReviewStateRecord | null {
    if (!isRecord(annotation) || annotation['is_manually_annotated'] !== true) {
        return null;
    }

    return { is_manually_annotated: true };
}

function resolvedIdentityIssueField(field: string, issues: ImportPreviewReviewIssueState): boolean {
    switch (field) {
        case 'category':
        case 'category_id':
        case 'preview_category_id':
            return !issues.hasMissingCategoryIssue;
        case 'account':
        case 'account_id':
        case 'source_account':
        case 'source_account_id':
        case 'preview_source_account_id':
            return !issues.hasMissingSourceAccountIssue;
        case 'destination_account':
        case 'destination_account_id':
        case 'transfer_target_account_id':
        case 'preview_destination_account_id':
            return !issues.hasMissingDestinationAccountIssue;
        case 'transfer_accounts':
        case 'same_transfer_account':
        case 'same_transfer_accounts':
        case 'transfer_account_direction':
            return !issues.hasTransferAccountReviewIssue;
        default:
            return false;
    }
}

function resolvedAnnotationKey(key: string, issues: ImportPreviewReviewIssueState): boolean {
    switch (key) {
        case 'category':
        case 'category_missing':
        case 'missing_category':
        case 'missing-category':
        case 'missing_classification':
            return !issues.hasMissingCategoryIssue;
        case 'account':
        case 'missing_account':
        case 'source_account':
        case 'source_account_missing':
        case 'missing_source_account':
        case 'missing-source-account':
            return !issues.hasMissingSourceAccountIssue;
        case 'destination_account':
        case 'destination_account_missing':
        case 'missing_destination_account':
        case 'missing-destination-account':
            return !issues.hasMissingDestinationAccountIssue;
        case 'transfer_account_direction':
        case 'transfer_accounts':
        case 'review_transfer_accounts':
        case 'same_transfer_account':
        case 'same_transfer_accounts':
            return !issues.hasTransferAccountReviewIssue;
        case 'requires_identity_review':
            return !issues.hasMissingCategoryIssue
                && !issues.hasMissingSourceAccountIssue
                && !issues.hasMissingDestinationAccountIssue
                && !issues.hasTransferAccountReviewIssue;
        default:
            return false;
    }
}

export function clearResolvedImportPreviewReviewState<T extends ReviewStateRecord | undefined>(
    matching: T,
    issues: ImportPreviewReviewIssueState
): T | undefined {
    if (!isRecord(matching)) {
        return matching;
    }

    const next: ReviewStateRecord = { ...matching };
    const identityValidation = next['identity_validation'];
    if (isRecord(identityValidation) && Array.isArray(identityValidation['issues'])) {
        const unresolvedIssues = identityValidation['issues'].filter(issue => {
            if (!isRecord(issue)) {
                return true;
            }
            const field = normalizedText(issue['field']);
            return !resolvedIdentityIssueField(field, issues);
        });

        if (unresolvedIssues.length > 0) {
            next['identity_validation'] = {
                ...identityValidation,
                issues: unresolvedIssues
            };
        } else {
            delete next['identity_validation'];
        }
    }

    const annotation = next['annotation'];
    const reviewKeys = annotationReviewKeys(annotation);
    if (reviewKeys.some(reviewKey => resolvedAnnotationKey(reviewKey, issues))) {
        const manualAnnotation = preserveManualAnnotationFlag(annotation);
        if (manualAnnotation) {
            next['annotation'] = manualAnnotation;
        } else {
            delete next['annotation'];
        }
    }

    return Object.keys(next).length > 0 ? next as T : undefined;
}
