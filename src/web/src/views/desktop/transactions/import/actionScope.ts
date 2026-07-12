import type { ImportPreviewServerQueryFilters } from './importPreviewIndex.ts';

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

export function buildImportPreviewActionScope(options: {
    selectedCount: number;
    selectionHash: string;
    filters: ImportPreviewServerQueryFilters;
}): ImportPreviewActionScope {
    if (options.selectedCount > 0) {
        return { kind: 'selected', selection_hash: options.selectionHash };
    }
    const filters: CanonicalImportPreviewActionFilters = {
        min_datetime: options.filters.minDatetime ?? null,
        max_datetime: options.filters.maxDatetime ?? null,
        transaction_type: options.filters.transactionType ?? null,
        category: options.filters.category ?? null,
        account: options.filters.account ?? null,
        tag: options.filters.tag ?? null,
        signal: options.filters.signal ?? null,
        annotation: options.filters.annotation ?? null,
        description: options.filters.description ?? null,
        selected_only: false
    };
    return { kind: 'all_matching', filters, filter_hash: hashImportPreviewActionFilters(filters) };
}
