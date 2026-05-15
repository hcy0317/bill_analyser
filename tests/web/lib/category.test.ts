import { describe, expect, test } from '@jest/globals';

import { CategoryType } from '@/core/category.ts';
import {
    getTransactionPrimaryCategoryName,
    getTransactionSecondaryCategoryName
} from '@/lib/category.ts';
import {
    TransactionCategory,
    type TransactionCategoryInfoResponse
} from '@/models/transaction_category.ts';

const categoryTree = TransactionCategory.ofMulti([
    {
        id: '100',
        name: '餐饮',
        parentId: '0',
        type: CategoryType.Expense,
        icon: 'mdi-food',
        color: '#5470c6',
        comment: '',
        displayOrder: 1,
        hidden: false,
        subCategories: [
            {
                id: '101',
                name: '早餐',
                parentId: '100',
                type: CategoryType.Expense,
                icon: 'mdi-coffee',
                color: '#91cc75',
                comment: '',
                displayOrder: 1,
                hidden: false
            }
        ]
    }
]);

function transactionCategory(overrides: Partial<TransactionCategoryInfoResponse>): TransactionCategoryInfoResponse {
    return {
        id: '101',
        name: '早餐',
        parentId: '100',
        type: CategoryType.Expense,
        icon: 'mdi-coffee',
        color: '#91cc75',
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

describe('transaction category display helpers', () => {
    test('prefer the loaded category tree over the transaction category snapshot', () => {
        const staleTransactionCategory = transactionCategory({
            name: '早餐'
        });

        expect(getTransactionPrimaryCategoryName('101', categoryTree, staleTransactionCategory)).toBe('餐饮');
        expect(getTransactionSecondaryCategoryName('101', categoryTree, staleTransactionCategory)).toBe('早餐');
    });

    test('do not duplicate a secondary-only transaction category as the primary label', () => {
        const secondaryOnlyCategory = transactionCategory({
            name: '早餐'
        });

        expect(getTransactionPrimaryCategoryName('101', undefined, secondaryOnlyCategory)).toBe('');
        expect(getTransactionSecondaryCategoryName('101', undefined, secondaryOnlyCategory)).toBe('早餐');
    });

    test('falls back to structured display names when the category tree is unavailable', () => {
        const displayNameCategory = transactionCategory({
            name: '餐饮-早餐'
        });

        expect(getTransactionPrimaryCategoryName('101', undefined, displayNameCategory)).toBe('餐饮');
        expect(getTransactionSecondaryCategoryName('101', undefined, displayNameCategory)).toBe('早餐');
    });
});
