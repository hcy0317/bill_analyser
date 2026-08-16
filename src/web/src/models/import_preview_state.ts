export const IMPORT_PREVIEW_STATE_PROJECTION_VERSION = 1 as const;

export const IMPORT_PREVIEW_STATE_SIGNAL_FAMILIES = [
    'parser',
    'platform_duplicate',
    'transfer',
    'history',
    'learning',
    'llm',
] as const;

export type ImportPreviewStateSignalFamily = typeof IMPORT_PREVIEW_STATE_SIGNAL_FAMILIES[number];

export const IMPORT_PREVIEW_STATE_SIGNAL_STATUSES = [
    'absent',
    'pending',
    'accepted',
    'rejected',
    'skipped',
    'auto_applied',
    'needs_review',
    'suppressed',
    'unknown',
] as const;

export type ImportPreviewStateSignalStatus = typeof IMPORT_PREVIEW_STATE_SIGNAL_STATUSES[number];

export interface ImportPreviewFamilyEvidence {
    status: ImportPreviewStateSignalStatus;
    has_evidence: boolean;
}

export const IMPORT_PREVIEW_REVIEW_ISSUE_CODES = [
    'missing_category',
    'invalid_category',
    'missing_source_account',
    'invalid_source_account',
    'missing_destination_account',
    'invalid_destination_account',
    'same_transfer_accounts',
    'unknown_transfer_state',
    'unknown_history_state',
    'unknown_learning_state',
    'unknown_llm_state',
] as const;

export type ImportPreviewReviewIssueCode = typeof IMPORT_PREVIEW_REVIEW_ISSUE_CODES[number];

export interface ImportPreviewStateSnapshot {
    projection_version: typeof IMPORT_PREVIEW_STATE_PROJECTION_VERSION;
    signals: ImportPreviewStateSignalFamily[];
    issues: ImportPreviewReviewIssueCode[];
    decisions: {
        transfer: ImportPreviewFamilyEvidence;
        history: ImportPreviewFamilyEvidence;
        learning: ImportPreviewFamilyEvidence;
        llm: ImportPreviewFamilyEvidence;
    };
    effective: {
        category_id: number | null;
        source_account_id: number | null;
        destination_account_id: number | null;
    };
}

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isFamilyEvidence(value: unknown): value is ImportPreviewFamilyEvidence {
    if (!isRecord(value)) {
        return false;
    }
    return IMPORT_PREVIEW_STATE_SIGNAL_STATUSES.includes(value['status'] as ImportPreviewStateSignalStatus)
        && typeof value['has_evidence'] === 'boolean';
}

function isNullableNumber(value: unknown): value is number | null {
    return value === null || (typeof value === 'number' && Number.isFinite(value));
}

export function isImportPreviewStateSnapshot(value: unknown): value is ImportPreviewStateSnapshot {
    if (!isRecord(value) || value['projection_version'] !== IMPORT_PREVIEW_STATE_PROJECTION_VERSION) {
        return false;
    }
    if (!Array.isArray(value['signals']) || !value['signals'].every(family => (
        IMPORT_PREVIEW_STATE_SIGNAL_FAMILIES.includes(family as ImportPreviewStateSignalFamily)
    ))) {
        return false;
    }
    if (!Array.isArray(value['issues']) || !value['issues'].every(issue => (
        IMPORT_PREVIEW_REVIEW_ISSUE_CODES.includes(issue as ImportPreviewReviewIssueCode)
    ))) {
        return false;
    }
    const decisions = value['decisions'];
    const effective = value['effective'];
    return isRecord(decisions)
        && ['transfer', 'history', 'learning', 'llm'].every(family => (
            isFamilyEvidence(decisions[family])
        ))
        && isRecord(effective)
        && isNullableNumber(effective['category_id'])
        && isNullableNumber(effective['source_account_id'])
        && isNullableNumber(effective['destination_account_id']);
}
