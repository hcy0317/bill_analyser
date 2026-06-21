import type { TransactionCategory } from '@/models/transaction_category.ts';

export function getCategoryListItemCheckedClass(category: TransactionCategory, queryCategoryIds: Record<string, boolean>): Record<string, boolean> {
    if (queryCategoryIds && queryCategoryIds[category.id]) {
        return {
            'list-item-checked': true
        };
    }

    if (category.subCategories) {
        for (const subCategory of category.subCategories) {
            if (queryCategoryIds && queryCategoryIds[subCategory.id]) {
                return {
                    'list-item-checked': true
                };
            }
        }
    }

    return {};
}
