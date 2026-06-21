import { TransactionType } from '@/core/transaction.ts';

import type { ImportCheckDataFilterLike } from '../checkDataFilters.ts';

import { PREVIEW_FILTER_INVALID_VALUE, PREVIEW_FILTER_NONE_VALUE, SERVER_PAGED_SORTABLE_COLUMNS, type ImportPreviewServerQueryFilters, type PreviewTableSortDirection, type PreviewTableSortInputItem, type PreviewTableSortItem } from './types.ts';



// 服务端分页查询 helper 负责把本地筛选草稿转换为 REST query，保持 sentinel 与日期格式集中。

export function normalizeCount(value: number | string | null | undefined): number {
    const normalizedValue = Number(value);
    if (!Number.isFinite(normalizedValue)) {
        return 0;
    }
    return Math.max(Math.floor(normalizedValue), 0);
}

export function resolveServerPagedSelectionCount(options: {
    total?: number | string | null;
    selected?: number | string | null;
    delta?: number | string | null;
}): number {
    const total = normalizeCount(options.total);
    const selected = normalizeCount(options.selected);
    const delta = Math.trunc(Number(options.delta || 0));
    const nextSelected = Number.isFinite(delta) ? selected + delta : selected;
    return Math.min(Math.max(nextSelected, 0), total);
}

export function isImportPreviewServerPagedSortableColumn(columnKey: string): boolean {
    return SERVER_PAGED_SORTABLE_COLUMNS.has(columnKey);
}

export function normalizePreviewPage(value: number | string | null | undefined): number {
    const normalizedValue = Number(value);
    if (!Number.isFinite(normalizedValue) || normalizedValue < 1) {
        return 1;
    }

    return Math.floor(normalizedValue);
}

export function normalizePreviewPageSize(
    value: number | string | null | undefined,
    options: { serverPaged?: boolean } = {}
): number {
    const normalizedValue = Number(value);
    if (!Number.isFinite(normalizedValue)) {
        return 10;
    }

    if (!options.serverPaged && normalizedValue === -1) {
        return -1;
    }

    return Math.max(Math.floor(normalizedValue), 1);
}

export function normalizePreviewTableSortDirection(
    value: string | boolean | null | undefined
): PreviewTableSortDirection {
    return String(value || '').toLowerCase() === 'desc' ? 'desc' : 'asc';
}

export function normalizeServerPagedSortKey(value: string | null | undefined): string {
    const normalizedValue = String(value || '').trim();
    return SERVER_PAGED_SORTABLE_COLUMNS.has(normalizedValue) ? normalizedValue : '';
}

export function formatPreviewServerFilterDatetime(value: number | null | undefined): string | undefined {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
        return undefined;
    }

    return new Date(value * 1000).toISOString().slice(0, 19).replace('T', ' ');
}

export function previewTypeFilterValue(value: number | null | undefined): string | undefined {
    switch (value) {
        case TransactionType.Income:
            return '收入';
        case TransactionType.Expense:
            return '支出';
        case TransactionType.Transfer:
            return '转账';
        case TransactionType.Investment:
            return '投资';
        default:
            return undefined;
    }
}

export function namedFilterValue(
    value: string | null | undefined,
    valueLabels: Record<string, string | number | undefined> = {}
): string | undefined {
    if (value === null) {
        return undefined;
    }

    if (value === undefined) {
        return PREVIEW_FILTER_INVALID_VALUE;
    }

    if (value === '') {
        return PREVIEW_FILTER_NONE_VALUE;
    }

    return String(valueLabels[value] ?? value);
}

export function identityFilterValue(
    value: string | null | undefined,
    valueLabels: Record<string, string | number | undefined> = {}
): string | undefined {
    if (value === null) {
        return undefined;
    }

    if (value === undefined) {
        return PREVIEW_FILTER_INVALID_VALUE;
    }

    if (value === '') {
        return PREVIEW_FILTER_NONE_VALUE;
    }

    if (value === PREVIEW_FILTER_INVALID_VALUE || value === PREVIEW_FILTER_NONE_VALUE) {
        return value;
    }

    const mappedValue = valueLabels[value];
    if (mappedValue === undefined || mappedValue === null || String(mappedValue).trim() === '') {
        return undefined;
    }

    return String(mappedValue);
}

// 将 check-data 本地筛选草稿转换为服务端 preview page query，集中处理 invalid/none sentinel 和身份标签映射。
export function buildImportPreviewServerQueryFilters(
    filters: ImportCheckDataFilterLike,
    context: {
        categoryValueByLabel?: Record<string, string | number | undefined>;
        accountValueByLabel?: Record<string, string | number | undefined>;
    } = {}
): ImportPreviewServerQueryFilters {
    const query: ImportPreviewServerQueryFilters = {};
    const minDatetime = formatPreviewServerFilterDatetime(filters.minDatetime);
    const maxDatetime = formatPreviewServerFilterDatetime(filters.maxDatetime);
    const transactionType = previewTypeFilterValue(filters.transactionType);
    const category = identityFilterValue(filters.category, context.categoryValueByLabel);
    const account = identityFilterValue(filters.account, context.accountValueByLabel);
    const tag = namedFilterValue(filters.tag);
    const description = namedFilterValue(filters.description);

    if (minDatetime) {
        query.minDatetime = minDatetime;
    }
    if (maxDatetime) {
        query.maxDatetime = maxDatetime;
    }
    if (transactionType) {
        query.transactionType = transactionType;
    }
    if (category) {
        query.category = category;
    }
    if (account) {
        query.account = account;
    }
    if (tag) {
        query.tag = tag;
    }
    if (filters.signal) {
        query.signal = filters.signal;
    }
    if (filters.annotation) {
        query.annotation = filters.annotation;
    }
    if (description) {
        query.description = description;
    }

    return query;
}

// 表格排序输入同时兼容 Vuetify 本地排序和服务端分页，服务端模式只保留后端支持的主排序字段。
export function normalizePreviewTableSortItems(
    sortBy: PreviewTableSortInputItem[] | null | undefined,
    options: { serverPaged?: boolean } = {}
): PreviewTableSortItem[] {
    if (!Array.isArray(sortBy) || sortBy.length < 1) {
        return [];
    }

    const normalizedItems: PreviewTableSortItem[] = [];
    for (const item of sortBy) {
        const rawKey = item?.key ?? item?.value;
        const key = String(rawKey || '').trim();

        if (!key) {
            continue;
        }

        normalizedItems.push({
            key,
            order: normalizePreviewTableSortDirection(item?.order)
        });
    }

    if (!options.serverPaged) {
        return normalizedItems;
    }

    const primarySortableItem = normalizedItems.find(item => normalizeServerPagedSortKey(item.key));
    return primarySortableItem ? [primarySortableItem] : [];
}
