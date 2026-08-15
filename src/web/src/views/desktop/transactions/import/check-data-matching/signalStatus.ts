import { trimImportPreviewSignalText } from '@/models/imported_transaction/matching.ts';

export type ImportPreviewStatusFamily = 'transfer' | 'history' | 'learning' | 'llm';

const IMPORT_PREVIEW_SIGNAL_STATUS_FIELDS = [
    'review_status',
    'status',
    'lifecycle_status',
    'signal_state'
] as const;

export const IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES = [
    'pending',
    'none',
    'suppressed',
    'accepted',
    'rejected',
    'skipped',
    'auto_applied',
    'auto-applied'
] as const;

export const IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES = [
    ...IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES,
    'needs_review'
] as const;

const IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUS_SET = new Set<string>(
    IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES
);
const IMPORT_PREVIEW_LEARNING_CANONICAL_STATUS_SET = new Set<string>(
    IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES
);

function recordValue(value: unknown): Record<string, unknown> | null {
    return value !== null && typeof value === 'object' && !Array.isArray(value)
        ? value as Record<string, unknown>
        : null;
}

function normalizeStatusValue(value: unknown): string {
    if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') {
        return '';
    }
    return trimImportPreviewSignalText(String(value)).toLowerCase();
}

export function resolveImportPreviewSignalStatus(section: unknown): string {
    const record = recordValue(section);
    if (!record) {
        return '';
    }
    for (const field of IMPORT_PREVIEW_SIGNAL_STATUS_FIELDS) {
        const value = normalizeStatusValue(record[field]);
        if (value) {
            return value;
        }
    }
    return '';
}

export function importPreviewSignalStatusIsUnknown(
    family: ImportPreviewStatusFamily,
    status: unknown
): boolean {
    const normalized = normalizeStatusValue(status);
    if (!normalized) {
        return false;
    }
    const canonicalStatuses = family === 'learning'
        ? IMPORT_PREVIEW_LEARNING_CANONICAL_STATUS_SET
        : IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUS_SET;
    return !canonicalStatuses.has(normalized);
}

export function importPreviewSignalSectionHasUnknownStatus(
    family: ImportPreviewStatusFamily,
    section: unknown
): boolean {
    return importPreviewSignalStatusIsUnknown(family, resolveImportPreviewSignalStatus(section));
}

export function importPreviewMatchingHasUnknownSignalStatus(matching: unknown): boolean {
    const record = recordValue(matching);
    return !!record && ([
        ['transfer', 'transfer'],
        ['history', 'reconciliation'],
        ['learning', 'learning'],
        ['llm', 'llm']
    ] as const).some(([family, section]) => (
        importPreviewSignalSectionHasUnknownStatus(family, record[section])
    ));
}
