import { type TransactionCategoryInfoResponse, TransactionCategory } from '@/models/transaction_category.ts';
import logger from '@/lib/logger.ts';

interface TransactionCategoryPath {
    primaryName: string;
    secondaryName: string;
}

function findTransactionCategoryPath(categoryId: string, allCategories?: TransactionCategory[]): TransactionCategoryPath | null {
    if (!allCategories) {
        return null;
    }

    for (const category of allCategories) {
        if (category.id === categoryId) {
            return {
                primaryName: category.name,
                secondaryName: ''
            };
        }

        const subCategoryList = category.subCategories;

        if (!subCategoryList) {
            continue;
        }

        for (const subCategory of subCategoryList) {
            if (subCategory.id === categoryId) {
                return {
                    primaryName: category.name,
                    secondaryName: subCategory.name
                };
            }
        }
    }

    return null;
}

function splitTransactionCategoryDisplayName(transactionCategory?: TransactionCategoryInfoResponse): TransactionCategoryPath | null {
    if (!transactionCategory?.name) {
        return null;
    }

    const normalizedName = transactionCategory.name.trim();
    const separators = ['-', '＞', '>', '/'];

    for (const separator of separators) {
        const parts = normalizedName.split(separator).map(part => part.trim()).filter(Boolean);

        if (parts.length >= 2) {
            return {
                primaryName: parts[0]!,
                secondaryName: parts.slice(1).join(separator)
            };
        }
    }

    return null;
}

export function getTransactionPrimaryCategoryName(categoryId: string | null | undefined, allCategories?: TransactionCategory[], transactionCategory?: TransactionCategoryInfoResponse): string {
    if (!categoryId || categoryId === '0') {
        return '';
    }

    const categoryPath = findTransactionCategoryPath(categoryId, allCategories);

    if (categoryPath) {
        return categoryPath.primaryName;
    }

    if (transactionCategory && transactionCategory.id === categoryId) {
        const displayNamePath = splitTransactionCategoryDisplayName(transactionCategory);

        if (displayNamePath) {
            return displayNamePath.primaryName;
        }

        if (!transactionCategory.parentId || transactionCategory.parentId === '0') {
            return transactionCategory.name || '';
        }

        return '';
    }

    if (!allCategories && !transactionCategory) {
        return '';
    }

    logger.warn(`[分类显示] 未在allCategories中找到匹配的分类: categoryId=${categoryId}`);
    return '';
}

export function getTransactionSecondaryCategoryName(categoryId: string | null | undefined, allCategories?: TransactionCategory[], transactionCategory?: TransactionCategoryInfoResponse): string {
    if (!categoryId || categoryId === '0') {
        return '';
    }

    const categoryPath = findTransactionCategoryPath(categoryId, allCategories);

    if (categoryPath) {
        return categoryPath.secondaryName;
    }

    if (transactionCategory && transactionCategory.id === categoryId) {
        const displayNamePath = splitTransactionCategoryDisplayName(transactionCategory);

        if (displayNamePath) {
            return displayNamePath.secondaryName;
        }

        if (transactionCategory.parentId && transactionCategory.parentId !== '0') {
            return transactionCategory.name || '';
        }

        return '';
    }

    if (!allCategories && !transactionCategory) {
        return '';
    }

    logger.warn(`[分类显示] 未在allCategories中找到匹配的子分类: categoryId=${categoryId}`);
    return '';
}
