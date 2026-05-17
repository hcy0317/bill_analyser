import type { ImportMatchingPayload } from '@/models/import_matching.ts';
import { TransactionType } from '@/core/transaction.ts';

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
    matching?: ImportPreviewMatchingPayload;
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

function getImportPreviewTransactionType(previewType?: string): TransactionType | null {
    const normalizedPreviewType = (previewType || '').trim().toLowerCase();
    switch (normalizedPreviewType) {
        case '收入':
        case 'income':
        case '2':
            return TransactionType.Income;
        case '支出':
        case 'expense':
        case '3':
            return TransactionType.Expense;
        case '转账':
        case 'transfer':
        case '4':
            return TransactionType.Transfer;
        case '投资':
        case 'investment':
        case '5':
            return TransactionType.Investment;
        default:
            return null;
    }
}

function categoryMatchesPreviewType(
    category: ImportPreviewCategoryLike | undefined,
    previewTransactionType: TransactionType | null
): boolean {
    if (!category || previewTransactionType === null || typeof category.type !== 'number') {
        return true;
    }

    return category.type === previewTransactionType;
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
    if (!category) {
        return null;
    }

    const parentId = category.parentId || '';
    if (parentId && parentId !== '0') {
        const parentCategory = categoriesById[parentId];
        if (!parentCategory) {
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
    const previewTransactionType = getImportPreviewTransactionType(previewData.preview_type);
    const persistedCategoryId = normalizePreviewCategoryId(previewData.category_id ?? previewData.categoryId);
    const persistedCategory = resolveImportPreviewCategoryPath(persistedCategoryId, categoriesById);
    if (
        persistedCategory
        && (
            previewTransactionType === null
            || persistedCategory.type === null
            || persistedCategory.type === previewTransactionType
        )
    ) {
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

        if (!categoryMatchesPreviewType(category, previewTransactionType)) {
            continue;
        }

        if (subCategory) {
            if (category.name !== subCategory || !category.parentId || category.parentId === '0') {
                continue;
            }

            const parentCategory = categoriesById[category.parentId];
            if (
                parentCategory?.name === mainCategory
                && categoryMatchesPreviewType(parentCategory, previewTransactionType)
            ) {
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
