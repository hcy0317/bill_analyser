import type { ImportMatchingPayload } from '@/models/import_matching.ts';
import { CategoryType } from '@/core/category.ts';

export interface ImportPreviewLLMMatchingPayload {
    suggested_main_category?: string;
    suggested_sub_category?: string;
    suggested_source_account?: string;
    suggested_destination_account?: string;
    confidence?: number;
    reason?: string;
    review_status?: string;
    suppressed?: boolean;
}

export type ImportPreviewMatchingPayload = ImportMatchingPayload & {
    llm?: ImportPreviewLLMMatchingPayload;
};

export interface ImportPreviewCategoryLike {
    id?: string;
    name: string;
    parentId?: string | null;
    type?: number | null;
    hidden?: boolean;
    subCategories?: ImportPreviewCategoryLike[];
}

export type ImportPreviewCategoryMap = Record<string, ImportPreviewCategoryLike | undefined>;

export interface ImportPreviewResolvedCategoryPath {
    id: string;
    mainCategory: string;
    subCategory: string;
    displayCategory: string;
    type: number | null;
}

export interface ImportPreviewRecord {
    id: number;
    category_id?: number | string | null;
    categoryId?: number | string | null;
    preview_type?: string;
    suggested_preview_type?: string;
    preview_date?: string;
    preview_amount_cents?: number;
    preview_destination_amount_cents?: number;
    preview_main_category?: string;
    preview_sub_category?: string;
    preview_source_account_id?: number | string | null;
    preview_destination_account_id?: number | string | null;
    preview_description?: string;
    preview_counterparty?: string;
    preview_payment_method?: string;
    transfer_suggestion_score?: number;
    transfer_suggestion_level?: string;
    transfer_suggestion_reason?: string;
    investment_signal_score?: number;
    investment_signal_level?: string;
    investment_signal_reason?: string;
    learning_recommendation_score?: number;
    learning_recommendation_level?: string;
    learning_recommendation_reason?: string;
    learning_recommendation_type?: string;
    learning_recommendation_summary?: string;
    investment_platform?: string;
    investment_product?: string;
    preview_recurring_id?: number;
    preview_recurring_name?: string;
    preview_recurring_candidate_count?: number;
    preview_recurring_match_score?: number;
    preview_recurring_match_reasons?: string;
    preview_recurring_matched_date?: string;
    dedup_type?: string;
    dedup_source_ids?: Array<number | string> | string;
    preview_parser_id?: string;
    preview_parser_tags?: string[];
    matching?: ImportPreviewMatchingPayload;
    preview_is_manually_annotated?: boolean;
    preview_selected?: boolean;
    selected?: boolean;
}

function normalizePreviewCategoryId(rawCategoryId: number | string | null | undefined): string {
    if (typeof rawCategoryId === 'number' && Number.isFinite(rawCategoryId) && rawCategoryId > 0) {
        return String(rawCategoryId);
    }

    if (typeof rawCategoryId === 'string') {
        const normalizedCategoryId = rawCategoryId.trim();
        return normalizedCategoryId && normalizedCategoryId !== '0' ? normalizedCategoryId : '';
    }

    return '';
}

export function resolveImportPreviewCategoryPath(
    categoryId: number | string | null | undefined,
    categoriesById: ImportPreviewCategoryMap
): ImportPreviewResolvedCategoryPath | null {
    const normalizedCategoryId = normalizePreviewCategoryId(categoryId);
    if (!normalizedCategoryId) {
        return null;
    }

    const category = categoriesById[normalizedCategoryId];
    if (!category || category.hidden) {
        return null;
    }

    const parentId = category.parentId || '';
    if (parentId && parentId !== '0') {
        const parentCategory = categoriesById[parentId];
        if (!parentCategory || parentCategory.hidden) {
            return null;
        }

        return {
            id: normalizedCategoryId,
            mainCategory: parentCategory.name,
            subCategory: category.name,
            displayCategory: category.name,
            type: typeof category.type === 'number'
                ? category.type
                : (typeof parentCategory.type === 'number' ? parentCategory.type : null)
        };
    }

    return {
        id: normalizedCategoryId,
        mainCategory: category.name,
        subCategory: '',
        displayCategory: category.name,
        type: typeof category.type === 'number' ? category.type : null
    };
}

export function resolveImportPreviewCategoryId(
    previewData: ImportPreviewRecord,
    categoriesById: ImportPreviewCategoryMap
): string {
    const persistedCategoryId = normalizePreviewCategoryId(previewData.category_id ?? previewData.categoryId);
    const persistedCategory = resolveImportPreviewCategoryPath(persistedCategoryId, categoriesById);
    if (persistedCategory) {
        return persistedCategoryId;
    }

    return '';
}

export function resolveImportPreviewDefaultTransferCategoryId(
    categoriesById: ImportPreviewCategoryMap,
    transferCategories: ImportPreviewCategoryLike[] | undefined,
    cashTransferCategoryId: number | string | null | undefined
): string {
    const cashTransferCategory = resolveImportPreviewCategoryPath(cashTransferCategoryId, categoriesById);
    if (cashTransferCategory?.type === CategoryType.Transfer) {
        return cashTransferCategory.id;
    }

    for (const primaryCategory of transferCategories || []) {
        if (primaryCategory.hidden) {
            continue;
        }

        for (const secondaryCategory of primaryCategory.subCategories || []) {
            if (!secondaryCategory.hidden && secondaryCategory.id) {
                return secondaryCategory.id;
            }
        }
    }

    return '';
}
