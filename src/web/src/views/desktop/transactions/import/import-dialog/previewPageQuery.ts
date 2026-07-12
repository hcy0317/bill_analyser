import type { ImportPreviewServerQueryFilters } from '../importPreviewIndex.ts';
import { SERVER_PAGED_PREVIEW_SORTABLE_COLUMNS } from './types.ts';

const PREVIEW_PAGE_FILTER_PARAM_NAMES: Record<keyof ImportPreviewServerQueryFilters, string> = {
    minDatetime: 'min_datetime',
    maxDatetime: 'max_datetime',
    transactionType: 'transaction_type',
    category: 'category',
    account: 'account',
    tag: 'tag',
    signal: 'signal',
    annotation: 'annotation',
    description: 'description',
};

export function normalizePreviewPageSortBy(value: string | null | undefined): string {
    const normalizedValue = String(value || '').trim();
    return SERVER_PAGED_PREVIEW_SORTABLE_COLUMNS.has(normalizedValue) ? normalizedValue : '';
}

export function normalizePreviewPageSortDirection(value: string | null | undefined): 'asc' | 'desc' {
    return String(value || '').toLowerCase() === 'desc' ? 'desc' : 'asc';
}

// 服务端预览分页 query 的字段名在这里集中维护，避免父组件和表格 tab 各自拼接参数。
export function appendPreviewPageFilters(
    searchParams: URLSearchParams,
    filters: ImportPreviewServerQueryFilters | undefined
): void {
    if (!filters) {
        return;
    }

    for (const [key, value] of Object.entries(filters)) {
        if (typeof value !== 'string') {
            continue;
        }
        searchParams.set(PREVIEW_PAGE_FILTER_PARAM_NAMES[key as keyof ImportPreviewServerQueryFilters], value);
    }
}

export function buildCanonicalPreviewPageRequestKey(
    page: number,
    pageSize: number,
    sortBy: string | null | undefined,
    sortDirection: string | null | undefined,
    filters: ImportPreviewServerQueryFilters | undefined
): string {
    const searchParams = new URLSearchParams({
        page: String(Math.max(page || 1, 1)),
        page_size: String(Math.max(pageSize || 10, 1))
    });
    const normalizedSortBy = normalizePreviewPageSortBy(sortBy);
    if (normalizedSortBy) {
        searchParams.set('sort_by', normalizedSortBy);
        searchParams.set('sort_direction', normalizePreviewPageSortDirection(sortDirection));
    }
    appendPreviewPageFilters(searchParams, filters);
    searchParams.sort();
    return searchParams.toString();
}
