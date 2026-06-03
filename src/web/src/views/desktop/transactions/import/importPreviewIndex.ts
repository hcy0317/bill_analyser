import { TransactionType } from '@/core/transaction.ts';

import {
    matchesImportTransactionCheckDataFilters,
    type ImportCheckDataFilterLike,
    type ImportCheckVisibleTransactionContext,
    type ImportCheckVisibleTransactionLike,
} from './checkDataFilters.ts';
import {
    buildImportPreviewSignalViewModel,
    type ImportPreviewSignalStatus,
    type ImportPreviewSignalViewModel,
} from './checkDataMatching.ts';

export interface ImportPreviewIndexItem extends ImportCheckVisibleTransactionLike {
    id: number;
    selected: boolean;
    sourceAmount: number;
    counterparty: string;
    paymentMethod: string;
    parserId: string;
    parserTags: string[];
    dedupType: string;
    dedupSourceIds: Array<number | string>;
    transferStatus?: ImportPreviewSignalStatus | null;
    transferTitle?: string;
    learningStatus?: ImportPreviewSignalStatus | null;
    learningTitle?: string;
    learningSummary?: string;
    learningMode?: string;
    recurringTemplateId?: string;
    recurringCandidateCount?: number;
    recurringMatchReasons?: string;
    recurringMatchedDate?: string;
}

export type ImportPreviewIndexFilterContext =
    Pick<ImportCheckVisibleTransactionContext<ImportPreviewIndexItem>, 'tagNameById'>;

export interface ImportPreviewIndexPageResult {
    page: number;
    totalCount: number;
    totalPages: number;
    previewIds: number[];
}

export type PreviewTableSortDirection = 'asc' | 'desc';

export const PREVIEW_FILTER_NONE_VALUE = '__none__';
export const PREVIEW_FILTER_INVALID_VALUE = '__invalid__';

export interface ImportPreviewFacetEntry {
    value: string;
    label?: string | null;
    count: number;
}

export interface ImportPreviewFilterGroup {
    title: string;
    labels: string[];
}

export interface ImportPreviewFilterCategoryLike {
    name: string;
    subCategories?: ImportPreviewFilterCategoryLike[];
}

export interface ImportPreviewFilterAccountLike {
    name: string;
    category?: number | string;
    subAccounts?: ImportPreviewFilterAccountLike[];
}

export interface ImportPreviewFilterAccountCategoryLike {
    type: number;
    name: string;
}

export interface ImportPreviewMetadata {
    facets?: {
        categories?: ImportPreviewFacetEntry[];
        accounts?: ImportPreviewFacetEntry[];
        tags?: ImportPreviewFacetEntry[];
    };
    counts?: {
        annotations?: Record<string, number>;
        signals?: Record<string, number>;
        selected?: number;
        selected_invalid?: number;
        total?: number;
    };
}

export interface ImportPreviewServerQueryFilters {
    minDatetime?: string;
    maxDatetime?: string;
    transactionType?: string;
    category?: string;
    account?: string;
    tag?: string;
    signal?: string;
    annotation?: string;
    description?: string;
}

export interface PreviewPageRequestOptions {
    sortBy?: string | null;
    sortDirection?: PreviewTableSortDirection | null;
    filters?: ImportPreviewServerQueryFilters;
}

export interface PreviewTableSortInputItem {
    key?: string;
    value?: string;
    order?: PreviewTableSortDirection | boolean | string | null;
}

export interface PreviewTableSortItem {
    key: string;
    order?: PreviewTableSortDirection | boolean;
}

function normalizeCount(value: number | string | null | undefined): number {
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

function uniqueFilterLabels(labels: string[]): string[] {
    const seen = new Set<string>();
    const uniqueLabels: string[] = [];
    for (const label of labels) {
        const normalizedLabel = String(label || '').trim();
        if (!normalizedLabel || seen.has(normalizedLabel)) {
            continue;
        }
        seen.add(normalizedLabel);
        uniqueLabels.push(normalizedLabel);
    }
    return uniqueLabels;
}

function pushGroupedLabel(
    groupedLabels: Map<string, string[]>,
    matchedLabels: Set<string>,
    groupTitle: string,
    label: string,
    availableLabels: Set<string>
): void {
    if (!availableLabels.has(label) || matchedLabels.has(label)) {
        return;
    }
    const labels = groupedLabels.get(groupTitle) || [];
    labels.push(label);
    groupedLabels.set(groupTitle, labels);
    matchedLabels.add(label);
}

export function groupImportPreviewCategoryFilterLabels(
    labels: string[],
    categoriesByType: Record<string | number, ImportPreviewFilterCategoryLike[]>,
    fallbackTitle: string
): ImportPreviewFilterGroup[] {
    const uniqueLabels = uniqueFilterLabels(labels);
    const availableLabels = new Set(uniqueLabels);
    const matchedLabels = new Set<string>();
    const groupedLabels = new Map<string, string[]>();

    for (const categories of Object.values(categoriesByType)) {
        for (const primaryCategory of categories || []) {
            if (!primaryCategory?.name) {
                continue;
            }
            pushGroupedLabel(groupedLabels, matchedLabels, primaryCategory.name, primaryCategory.name, availableLabels);
            for (const subCategory of primaryCategory.subCategories || []) {
                if (subCategory?.name) {
                    pushGroupedLabel(groupedLabels, matchedLabels, primaryCategory.name, subCategory.name, availableLabels);
                }
            }
        }
    }

    const groups = Array.from(groupedLabels, ([title, groupLabels]) => ({
        title,
        labels: groupLabels
    }));
    const ungroupedLabels = uniqueLabels.filter(label => !matchedLabels.has(label));
    if (ungroupedLabels.length > 0) {
        groups.push({
            title: fallbackTitle,
            labels: ungroupedLabels
        });
    }
    return groups;
}

export function groupImportPreviewAccountFilterLabels(
    labels: string[],
    accounts: ImportPreviewFilterAccountLike[],
    accountCategories: ImportPreviewFilterAccountCategoryLike[],
    fallbackTitle: string
): ImportPreviewFilterGroup[] {
    const uniqueLabels = uniqueFilterLabels(labels);
    const availableLabels = new Set(uniqueLabels);
    const matchedLabels = new Set<string>();
    const groupedLabels = new Map<string, string[]>();
    const categoryTitleByType = new Map<number, string>();
    for (const category of accountCategories) {
        categoryTitleByType.set(Number(category.type), category.name);
    }

    const visitAccount = (account: ImportPreviewFilterAccountLike, inheritedCategory?: number): void => {
        const categoryType = Number(account.category ?? inheritedCategory ?? NaN);
        const groupTitle = categoryTitleByType.get(categoryType) || fallbackTitle;
        if (account?.name) {
            pushGroupedLabel(groupedLabels, matchedLabels, groupTitle, account.name, availableLabels);
        }
        for (const subAccount of account.subAccounts || []) {
            visitAccount(subAccount, categoryType);
        }
    };

    for (const account of accounts || []) {
        visitAccount(account);
    }

    const groups = Array.from(groupedLabels, ([title, groupLabels]) => ({
        title,
        labels: groupLabels
    }));
    const ungroupedLabels = uniqueLabels.filter(label => !matchedLabels.has(label));
    if (ungroupedLabels.length > 0) {
        groups.push({
            title: fallbackTitle,
            labels: ungroupedLabels
        });
    }
    return groups;
}

export interface ImportPreviewIndexResponseItem {
    id: number;
    preview_date?: string;
    type?: number;
    source_amount?: number;
    category_id?: string;
    actual_category_name?: string;
    source_account_id?: string;
    destination_account_id?: string;
    actual_source_account_name?: string;
    actual_destination_account_name?: string;
    comment?: string;
    counterparty?: string;
    payment_method?: string;
    selected?: boolean;
    is_manually_annotated?: boolean;
    parser_source?: string;
    parser_tags?: string[];
    dedup_type?: string;
    dedup_source_ids?: Array<number | string>;
    transfer_status?: ImportPreviewSignalStatus | null;
    transfer_title?: string;
    learning_status?: ImportPreviewSignalStatus | null;
    learning_title?: string;
    learning_summary?: string;
    learning_mode?: string;
    recurring_template_id?: string;
    recurring_candidate_count?: number;
    recurring_match_reasons?: string;
    recurring_matched_date?: string;
}

export const SERVER_PAGED_SORTABLE_COLUMNS = new Set<string>([
    'time',
    'type',
    'sourceAmount',
    'counterparty',
    'paymentMethod',
    'comment'
]);

export function isImportPreviewServerPagedSortableColumn(columnKey: string): boolean {
    return SERVER_PAGED_SORTABLE_COLUMNS.has(columnKey);
}

export function mapImportPreviewIndexResponseItem(item: ImportPreviewIndexResponseItem): ImportPreviewIndexItem {
    const time = new Date(item.preview_date || '').getTime() / 1000;
    return {
        id: Number(item.id || 0),
        time: Number.isFinite(time) ? time : Date.now() / 1000,
        type: Number(item.type || TransactionType.ModifyBalance),
        actualCategoryName: item.actual_category_name || '',
        categoryId: item.category_id || '',
        actualSourceAccountName: item.actual_source_account_name || '',
        actualDestinationAccountName: item.actual_destination_account_name || '',
        sourceAccountId: item.source_account_id || '',
        destinationAccountId: item.destination_account_id || '',
        tagIds: [],
        originalTagNames: [],
        comment: item.comment || '',
        isManuallyAnnotated: !!item.is_manually_annotated,
        selected: !!item.selected,
        sourceAmount: Number(item.source_amount || 0),
        counterparty: item.counterparty || '',
        paymentMethod: item.payment_method || '',
        parserId: item.parser_source || '',
        parserTags: item.parser_tags || [],
        dedupType: item.dedup_type || '',
        dedupSourceIds: item.dedup_source_ids || [],
        transferStatus: item.transfer_status ?? null,
        transferTitle: item.transfer_title || '',
        learningStatus: item.learning_status ?? null,
        learningTitle: item.learning_title || '',
        learningSummary: item.learning_summary || '',
        learningMode: item.learning_mode || '',
        recurringTemplateId: item.recurring_template_id || '',
        recurringCandidateCount: Number(item.recurring_candidate_count || 0),
        recurringMatchReasons: item.recurring_match_reasons || '',
        recurringMatchedDate: item.recurring_matched_date || '',
    };
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

function previewTypeFilterValue(value: number | null | undefined): string | undefined {
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

function namedFilterValue(
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

export function buildImportPreviewServerQueryFilters(
    filters: ImportCheckDataFilterLike,
    context: {
        accountIdByName?: Record<string, string | number | undefined>;
    } = {}
): ImportPreviewServerQueryFilters {
    const query: ImportPreviewServerQueryFilters = {};
    const minDatetime = formatPreviewServerFilterDatetime(filters.minDatetime);
    const maxDatetime = formatPreviewServerFilterDatetime(filters.maxDatetime);
    const transactionType = previewTypeFilterValue(filters.transactionType);
    const category = namedFilterValue(filters.category);
    const account = namedFilterValue(filters.account, context.accountIdByName);
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

function getPrimaryRecurringReason(item: ImportPreviewIndexItem): string {
    if (!item.recurringMatchReasons) {
        return '';
    }

    return item.recurringMatchReasons
        .split('|')
        .map(text => text.trim())
        .filter(text => !!text)[0] || '';
}

export function collectImportPreviewIndexAnnotationIssues(item: ImportPreviewIndexItem): string[] {
    const reasons: string[] = [];

    if (item.type !== TransactionType.ModifyBalance && (!item.categoryId || item.categoryId === '0')) {
        reasons.push('Missing Category');
    }

    if (!item.sourceAccountId || item.sourceAccountId === '0') {
        reasons.push('Missing Source Account');
    }

    const requiresDestinationAccount = item.type === 4 || item.type === 5;
    if (requiresDestinationAccount && (!item.destinationAccountId || item.destinationAccountId === '0')) {
        reasons.push('Missing Destination Account');
    }

    if (
        requiresDestinationAccount
        && item.sourceAccountId
        && item.destinationAccountId
        && item.sourceAccountId !== '0'
        && item.destinationAccountId !== '0'
        && item.sourceAccountId === item.destinationAccountId
    ) {
        reasons.push('Review Transfer Accounts');
    }

    return reasons;
}

export function buildImportPreviewIndexSignalViewModel(item: ImportPreviewIndexItem): ImportPreviewSignalViewModel {
    return buildImportPreviewSignalViewModel({
        parserId: item.parserId,
        parserTags: item.parserTags,
        dedupType: item.dedupType,
        dedupSourceIds: item.dedupSourceIds,
        isManuallyAnnotated: item.isManuallyAnnotated,
        transferStatus: item.transferStatus ?? null,
        transferTitle: item.transferTitle || '',
        learningStatus: item.learningStatus ?? null,
        learningTitle: item.learningTitle || '',
        learningSummary: item.learningSummary || '',
        learningMode: item.learningMode || '',
        hasRecurringMatch: !!item.recurringTemplateId,
        recurringTitle: item.recurringMatchReasons || '',
        recurringCandidateCount: Number(item.recurringCandidateCount || 0),
        recurringPrimaryReason: getPrimaryRecurringReason(item),
    });
}

export function matchesImportPreviewIndexItemFilters(
    item: ImportPreviewIndexItem,
    filters: ImportCheckDataFilterLike,
    context: ImportPreviewIndexFilterContext = {},
): boolean {
    return matchesImportTransactionCheckDataFilters(item, filters, {
        tagNameById: context.tagNameById,
        hasAnnotationIssues: candidate => collectImportPreviewIndexAnnotationIssues(candidate).length > 0,
        isEditing: () => false,
        signalViewModelFor: candidate => buildImportPreviewIndexSignalViewModel(candidate),
    });
}

export function sortImportPreviewIndexItems(
    items: ImportPreviewIndexItem[],
    sortBy: string | null | undefined,
    sortDirection: 'asc' | 'desc' | null | undefined,
): ImportPreviewIndexItem[] {
    const normalizedSortBy = String(sortBy || '').trim();
    const normalizedSortDirection = sortDirection === 'desc' ? 'desc' : 'asc';
    const directionMultiplier = normalizedSortDirection === 'desc' ? -1 : 1;
    const stabilizedItems = items.map((item, index) => ({ item, index }));

    const compareNumber = (left: number, right: number): number => {
        if (left === right) {
            return 0;
        }
        return left < right ? -1 : 1;
    };

    const compareString = (left: string, right: string): number => left.localeCompare(right, undefined, {
        sensitivity: 'base',
    });

    stabilizedItems.sort((left, right) => {
        let compareResult = 0;
        switch (normalizedSortBy) {
            case 'time':
                compareResult = compareNumber(Number(left.item.time || 0), Number(right.item.time || 0));
                break;
            case 'type':
                compareResult = compareNumber(Number(left.item.type || 0), Number(right.item.type || 0));
                break;
            case 'sourceAmount':
                compareResult = compareNumber(Number(left.item.sourceAmount || 0), Number(right.item.sourceAmount || 0));
                break;
            case 'counterparty':
                compareResult = compareString(left.item.counterparty || '', right.item.counterparty || '');
                break;
            case 'paymentMethod':
                compareResult = compareString(left.item.paymentMethod || '', right.item.paymentMethod || '');
                break;
            case 'comment':
                compareResult = compareString(left.item.comment || '', right.item.comment || '');
                break;
            default:
                compareResult = compareNumber(Number(left.item.time || 0), Number(right.item.time || 0));
                break;
        }

        if (compareResult !== 0) {
            return compareResult * directionMultiplier;
        }

        return left.index - right.index;
    });

    return stabilizedItems.map(entry => entry.item);
}

export function resolveImportPreviewIndexPage(
    items: ImportPreviewIndexItem[],
    page: number,
    pageSize: number,
): ImportPreviewIndexPageResult {
    const totalCount = items.length;
    const normalizedPageSize = Math.max(Number(pageSize || 10), 1);
    const totalPages = Math.max(Math.ceil(totalCount / normalizedPageSize), 1);
    const normalizedPage = Math.min(Math.max(Number(page || 1), 1), totalPages);
    const start = (normalizedPage - 1) * normalizedPageSize;
    const previewIds = items.slice(start, start + normalizedPageSize).map(item => item.id);

    return {
        page: normalizedPage,
        totalCount,
        totalPages,
        previewIds,
    };
}
