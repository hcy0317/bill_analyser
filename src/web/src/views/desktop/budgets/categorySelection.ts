import type { TransactionCategory } from '@/models/transaction_category.ts';

export interface ResolvedBudgetCategorySelection {
    categoryId: string;
    primaryCategoryId: string;
    primaryCategoryName: string;
    secondaryCategoryId: string;
    secondaryCategoryName: string;
    isPrimaryCategory: boolean;
}

export function resolveBudgetCategorySelection(
    categories: TransactionCategory[],
    categoryId: string
): ResolvedBudgetCategorySelection | null {
    const normalizedCategoryId = String(categoryId || '');
    if (!normalizedCategoryId) {
        return null;
    }

    for (const primaryCategory of categories) {
        if (primaryCategory.id === normalizedCategoryId) {
            return {
                categoryId: primaryCategory.id,
                primaryCategoryId: primaryCategory.id,
                primaryCategoryName: primaryCategory.name,
                secondaryCategoryId: '',
                secondaryCategoryName: '',
                isPrimaryCategory: true
            };
        }

        for (const secondaryCategory of primaryCategory.subCategories || []) {
            if (secondaryCategory.id === normalizedCategoryId) {
                return {
                    categoryId: secondaryCategory.id,
                    primaryCategoryId: primaryCategory.id,
                    primaryCategoryName: primaryCategory.name,
                    secondaryCategoryId: secondaryCategory.id,
                    secondaryCategoryName: secondaryCategory.name,
                    isPrimaryCategory: false
                };
            }
        }
    }

    return null;
}

export function findBudgetCategoryIdByNames(
    categories: TransactionCategory[],
    primaryCategoryName: string,
    secondaryCategoryName?: string
): string {
    for (const primaryCategory of categories) {
        if (primaryCategory.name !== primaryCategoryName) {
            continue;
        }

        if (!secondaryCategoryName) {
            return primaryCategory.id;
        }

        for (const secondaryCategory of primaryCategory.subCategories || []) {
            if (secondaryCategory.name === secondaryCategoryName) {
                return secondaryCategory.id;
            }
        }
    }

    return '';
}
