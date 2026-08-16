import type { ImportPreviewRecord } from '@/models/import_preview.ts';
import { CategoryType } from '@/core/category.ts';

export type {
    ImportPreviewLLMMatchingPayload,
    ImportPreviewMatchingPayload,
    ImportPreviewRecord
} from '@/models/import_preview.ts';

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
