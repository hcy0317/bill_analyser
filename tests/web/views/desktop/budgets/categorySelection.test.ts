import { describe, expect, test } from '@jest/globals';

import type { TransactionCategory } from '@/models/transaction_category.ts';
import {
    findBudgetCategoryIdByNames,
    resolveBudgetCategorySelection
} from '@/views/desktop/budgets/categorySelection.ts';

function createCategory(id: string, name: string, subCategories?: TransactionCategory[]): TransactionCategory {
    return {
        id,
        name,
        parentId: '0',
        type: 1,
        icon: 'mdi-shape',
        color: '#5470c6',
        comment: '',
        displayOrder: 0,
        visible: true,
        subCategories
    } as unknown as TransactionCategory;
}

const CATEGORIES: TransactionCategory[] = [
    createCategory('100', '餐饮', [
        createCategory('101', '早餐'),
        createCategory('102', '午餐')
    ]),
    createCategory('200', '交通', [
        createCategory('201', '地铁')
    ]),
    createCategory('300', '杂项')
];

describe('categorySelection helpers', () => {
    test('resolveBudgetCategorySelection returns null for blank and unknown category ids', () => {
        expect(resolveBudgetCategorySelection(CATEGORIES, '')).toBeNull();
        expect(resolveBudgetCategorySelection(CATEGORIES, '999')).toBeNull();
    });

    test('resolveBudgetCategorySelection resolves a primary category id', () => {
        expect(resolveBudgetCategorySelection(CATEGORIES, '100')).toStrictEqual({
            categoryId: '100',
            primaryCategoryId: '100',
            primaryCategoryName: '餐饮',
            secondaryCategoryId: '',
            secondaryCategoryName: '',
            isPrimaryCategory: true
        });
    });

    test('resolveBudgetCategorySelection resolves a secondary category id', () => {
        expect(resolveBudgetCategorySelection(CATEGORIES, '201')).toStrictEqual({
            categoryId: '201',
            primaryCategoryId: '200',
            primaryCategoryName: '交通',
            secondaryCategoryId: '201',
            secondaryCategoryName: '地铁',
            isPrimaryCategory: false
        });
    });

    test('findBudgetCategoryIdByNames returns the primary id when no secondary name is requested', () => {
        expect(findBudgetCategoryIdByNames(CATEGORIES, '餐饮')).toBe('100');
        expect(findBudgetCategoryIdByNames(CATEGORIES, '杂项')).toBe('300');
    });

    test('findBudgetCategoryIdByNames returns the matching secondary id when both names are provided', () => {
        expect(findBudgetCategoryIdByNames(CATEGORIES, '餐饮', '午餐')).toBe('102');
        expect(findBudgetCategoryIdByNames(CATEGORIES, '交通', '地铁')).toBe('201');
    });

    test('findBudgetCategoryIdByNames returns an empty string when names do not match', () => {
        expect(findBudgetCategoryIdByNames(CATEGORIES, '餐饮', '晚餐')).toBe('');
        expect(findBudgetCategoryIdByNames(CATEGORIES, '杂项', '不存在')).toBe('');
        expect(findBudgetCategoryIdByNames(CATEGORIES, '不存在')).toBe('');
    });
});
