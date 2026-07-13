import type { ImportPreviewServerQueryFilters } from './importPreviewIndex.ts';
import { normalizePreviewPageFilters } from './import-dialog/previewPageQuery.ts';

export interface CanonicalImportPreviewActionFilters {
    min_datetime: string | null;
    max_datetime: string | null;
    transaction_type: string | null;
    category: string | null;
    account: string | null;
    tag: string | null;
    signal: string | null;
    annotation: string | null;
    description: string | null;
    selected_only: false;
}

export type ImportPreviewActionScope =
    | { kind: 'selected'; selection_hash: string }
    | { kind: 'all_matching'; filters: CanonicalImportPreviewActionFilters; filter_hash: string };

export function hashImportPreviewActionFilters(filters: CanonicalImportPreviewActionFilters): string {
    let hash = 2166136261;
    for (const character of new TextEncoder().encode(JSON.stringify(filters))) {
        hash = Math.imul(hash ^ character, 16777619) >>> 0;
    }
    return `fnv1a32:${hash.toString(16).padStart(8, '0')}`;
}

export function hashImportPreviewSelectionIds(previewIds: number[]): string {
    const canonicalIds = [...new Set(previewIds)]
        .filter(previewId => Number.isSafeInteger(previewId) && previewId > 0)
        .sort((left, right) => left - right);
    let hash = 2166136261;
    for (const character of new TextEncoder().encode(canonicalIds.join(','))) {
        hash = Math.imul(hash ^ character, 16777619) >>> 0;
    }
    return `fnv1a32:${hash.toString(16).padStart(8, '0')}`;
}

export function buildImportPreviewActionScope(options: {
    selectedCount: number;
    selectionHash: string;
    filters: ImportPreviewServerQueryFilters;
}): ImportPreviewActionScope {
    if (options.selectedCount > 0) {
        return { kind: 'selected', selection_hash: options.selectionHash };
    }
    const normalizedFilters = normalizePreviewPageFilters(options.filters);
    const filters: CanonicalImportPreviewActionFilters = {
        min_datetime: normalizedFilters.minDatetime ?? null,
        max_datetime: normalizedFilters.maxDatetime ?? null,
        transaction_type: normalizedFilters.transactionType ?? null,
        category: normalizedFilters.category ?? null,
        account: normalizedFilters.account ?? null,
        tag: normalizedFilters.tag ?? null,
        signal: normalizedFilters.signal ?? null,
        annotation: normalizedFilters.annotation ?? null,
        description: normalizedFilters.description ?? null,
        selected_only: false
    };
    return { kind: 'all_matching', filters, filter_hash: hashImportPreviewActionFilters(filters) };
}
