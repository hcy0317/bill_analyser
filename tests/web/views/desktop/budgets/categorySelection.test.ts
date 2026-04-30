import { describe, expect, test } from '@jest/globals';

import { DateRange } from '@/core/datetime.ts';
import { getUnixTimeFromLocalDatetime } from '@/lib/datetime.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import {
    buildBudgetDrilldownRouteQuery,
    findBudgetCategoryIdByNames,
    getBudgetDrilldownCategoryIds,
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

    test('getBudgetDrilldownCategoryIds expands a primary category to include its children', () => {
        expect(getBudgetDrilldownCategoryIds(CATEGORIES, '餐饮')).toBe('100,101,102');
        expect(getBudgetDrilldownCategoryIds(CATEGORIES, '杂项')).toBe('300');
    });

    test('getBudgetDrilldownCategoryIds resolves secondary categories and falls back safely', () => {
        expect(getBudgetDrilldownCategoryIds(CATEGORIES, '交通', '地铁')).toBe('201');
        expect(getBudgetDrilldownCategoryIds(CATEGORIES, '交通', '不存在', '200')).toBe('200');
        expect(getBudgetDrilldownCategoryIds(CATEGORIES, '不存在', undefined, '999')).toBe('999');
    });

    test('buildBudgetDrilldownRouteQuery builds transaction-list-compatible query params', () => {
        const minDate = new Date(2026, 3, 1, 0, 0, 0);
        const maxDate = new Date(2026, 5, 30, 23, 59, 59);

        expect(buildBudgetDrilldownRouteQuery({
            categories: CATEGORIES,
            primaryCategoryName: '餐饮',
            startDate: '2026-04-01',
            endDate: '2026-06-30',
            transactionType: 3,
            accountIds: ['acc-1', '', 'acc-2'],
            tagIds: ['tag-1', '', 'tag-2']
        })).toStrictEqual({
            type: '3',
            categoryIds: '100,101,102',
            dateType: String(DateRange.Custom.type),
            minTime: String(getUnixTimeFromLocalDatetime(minDate)),
            maxTime: String(getUnixTimeFromLocalDatetime(maxDate)),
            accountIds: 'acc-1,acc-2',
            tagIds: 'tag-1,tag-2'
        });
    });

    test('buildBudgetDrilldownRouteQuery omits invalid optional params while preserving fallback category', () => {
        expect(buildBudgetDrilldownRouteQuery({
            categories: CATEGORIES,
            primaryCategoryName: '不存在',
            secondaryCategoryName: '不存在',
            fallbackCategoryId: 'fallback-1',
            startDate: 'bad-date',
            endDate: '2026-06-30',
            transactionType: 5
        })).toStrictEqual({
            type: '5',
            categoryIds: 'fallback-1'
        });
    });
});
