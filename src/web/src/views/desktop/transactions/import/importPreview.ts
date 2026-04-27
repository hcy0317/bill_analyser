import type { ImportMatchingPayload } from '@/models/import_matching.ts';

export interface ImportPreviewCategoryLike {
    id?: string;
    name: string;
    parentId?: string | null;
}

export type ImportPreviewCategoryMap = Record<string, ImportPreviewCategoryLike | undefined>;

export interface ImportPreviewRecord {
    id: number;
    category_id?: number | string | null;
    categoryId?: number | string | null;
    preview_type?: string;
    suggested_preview_type?: string;
    preview_date?: string;
    preview_amount?: number;
    preview_destination_amount?: number;
    preview_main_category?: string;
    preview_sub_category?: string;
    preview_source_account_id?: number;
    preview_destination_account_id?: number;
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
    matching?: ImportMatchingPayload;
    preview_is_manually_annotated?: boolean;
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

export function resolveImportPreviewCategoryId(
    previewData: ImportPreviewRecord,
    categoriesById: ImportPreviewCategoryMap
): string {
    const persistedCategoryId = normalizePreviewCategoryId(previewData.category_id ?? previewData.categoryId);
    if (persistedCategoryId && categoriesById[persistedCategoryId]) {
        return persistedCategoryId;
    }

    const mainCategory = previewData.preview_main_category || '';
    const subCategory = previewData.preview_sub_category || '';
    if (!mainCategory && !subCategory) {
        return '';
    }

    for (const [categoryId, category] of Object.entries(categoriesById)) {
        if (!category) {
            continue;
        }

        if (subCategory) {
            if (category.name !== subCategory || !category.parentId || category.parentId === '0') {
                continue;
            }

            const parentCategory = categoriesById[category.parentId];
            if (parentCategory?.name === mainCategory) {
                return categoryId;
            }
            continue;
        }

        if (mainCategory && category.name === mainCategory && (!category.parentId || category.parentId === '0')) {
            return categoryId;
        }
    }

    return '';
}
