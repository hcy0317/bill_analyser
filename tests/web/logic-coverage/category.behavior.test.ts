import { describe, expect, test } from '@jest/globals';

import { CategoryType, type LocalizedPresetCategory } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';
import {
    allTransactionCategoriesWithVisibleCount,
    allVisiblePrimaryTransactionCategoriesByType,
    categoryTypeToTransactionType,
    containsAnyAvailableCategory,
    containsAvailableCategory,
    getAvailableCategoryCount,
    getFinalCategoryIdsByFilteredCategoryIds,
    getFirstAvailableCategoryId,
    getFirstAvailableSubCategoryId,
    getFirstShowingId,
    getLastShowingId,
    getSecondaryTransactionMapByName,
    getTransactionPrimaryCategoryName,
    getTransactionSecondaryCategoryName,
    isCategoryOrSubCategoriesAllChecked,
    isNoAvailableCategory,
    isSubCategoriesAllChecked,
    isSubCategoriesHasButNotAllChecked,
    isSubCategoryIdAvailable,
    localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories,
    localizedPresetCategoryToTransactionCategoryCreateWithSubCategorys,
    selectAll,
    selectAllSubCategories,
    selectAllVisible,
    selectInvert,
    selectNone,
    transactionTypeToCategoryType
} from '@/lib/category.ts';
import {
    TransactionCategory,
    type TransactionCategoriesWithVisibleCount,
    type TransactionCategoryInfoResponse
} from '@/models/transaction_category.ts';

function category(
    id: string,
    options: Partial<TransactionCategoryInfoResponse> = {}
): TransactionCategory {
    return TransactionCategory.of({
        id,
        name: `Category ${id}`,
        parentId: '0',
        type: CategoryType.Expense,
        icon: 'icon',
        color: '#123456',
        comment: '',
        displayOrder: 0,
        hidden: false,
        ...options
    });
}

const categoryTypes = [
    [TransactionType.Income, CategoryType.Income],
    [TransactionType.Expense, CategoryType.Expense],
    [TransactionType.Transfer, CategoryType.Transfer],
    [TransactionType.Investment, CategoryType.Investment]
] as const;

describe('category type and preset conversion', () => {
    test.each(categoryTypes)('maps transaction type %i to category type %i', (transactionType, categoryType) => {
        expect(transactionTypeToCategoryType(transactionType)).toBe(categoryType);
        expect(categoryTypeToTransactionType(categoryType)).toBe(transactionType);
    });

    test('returns null for transaction/category types without a mapping', () => {
        expect(transactionTypeToCategoryType(TransactionType.ModifyBalance)).toBeNull();
        expect(categoryTypeToTransactionType(99 as CategoryType)).toBeNull();
    });

    test('resolves primary and secondary display names through the category facade exports', () => {
        const child = category('child', { name: 'Breakfast', parentId: 'parent' });
        const tree = [category('parent', { name: 'Food', subCategories: [{ ...child, hidden: false }] })];

        expect(getTransactionPrimaryCategoryName('child', tree)).toBe('Food');
        expect(getTransactionSecondaryCategoryName('child', tree)).toBe('Breakfast');
    });

    test('converts localized presets into create requests with initialized fields', () => {
        const preset: LocalizedPresetCategory = {
            name: 'Food',
            type: CategoryType.Expense,
            icon: 'food',
            color: '#111111',
            subCategories: [
                { name: 'Breakfast', type: CategoryType.Expense, icon: 'breakfast', color: '#222222' },
                { name: 'Dinner', type: CategoryType.Expense, icon: 'dinner', color: '#333333' }
            ]
        };

        expect(localizedPresetCategoryToTransactionCategoryCreateWithSubCategorys(preset)).toEqual({
            name: 'Food',
            type: CategoryType.Expense,
            icon: 'food',
            color: '#111111',
            subCategories: [
                {
                    name: 'Breakfast',
                    type: CategoryType.Expense,
                    parentId: '0',
                    icon: 'breakfast',
                    color: '#222222',
                    comment: '',
                    displayOrder: 0,
                    clientSessionId: ''
                },
                {
                    name: 'Dinner',
                    type: CategoryType.Expense,
                    parentId: '0',
                    icon: 'dinner',
                    color: '#333333',
                    comment: '',
                    displayOrder: 0,
                    clientSessionId: ''
                }
            ]
        });
        expect(localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories([preset])).toHaveLength(1);
        expect(localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories([])).toEqual([]);
    });
});

describe('category maps and availability summaries', () => {
    const visibleSub = category('visible-sub', { parentId: 'visible-parent' });
    const hiddenSub = category('hidden-sub', { parentId: 'visible-parent', hidden: true });
    const visibleParent = category('visible-parent', { subCategories: [
        { ...visibleSub, hidden: false },
        { ...hiddenSub, hidden: true }
    ] });
    const hiddenParent = category('hidden-parent', { hidden: true, subCategories: [] });

    test('indexes secondary categories by name and handles absent or empty children', () => {
        expect(getSecondaryTransactionMapByName()).toEqual({});
        expect(getSecondaryTransactionMapByName([visibleParent, category('no-children', { parentId: 'root' })])).toEqual({
            'Category visible-sub': visibleParent.subCategories![0],
            'Category hidden-sub': visibleParent.subCategories![1]
        });
    });

    test('summarizes visible primary and secondary categories across supported types', () => {
        const result = allTransactionCategoriesWithVisibleCount({
            [CategoryType.Expense]: [hiddenParent, visibleParent],
            [CategoryType.Income]: [],
            [CategoryType.Investment]: [category('investment')]
        });

        expect(result[CategoryType.Expense]).toMatchObject({
            type: CategoryType.Expense,
            allVisibleCategoryCount: 1,
            firstVisibleCategoryIndex: 1,
            allVisibleSubCategoryCounts: { 'visible-parent': 1 },
            allFirstVisibleSubCategoryIndexes: { 'visible-parent': 0 }
        });
        expect(result[CategoryType.Expense]!.allSubCategories['visible-parent']).toHaveLength(2);
        expect(result[CategoryType.Income]).toMatchObject({ allVisibleCategoryCount: 0, firstVisibleCategoryIndex: -1 });
        expect(result[CategoryType.Transfer]).toBeUndefined();
        expect(result[CategoryType.Investment]!.allVisibleCategoryCount).toBe(1);
    });

    test('allow-category filters accept numeric and string keys and exclude disallowed types', () => {
        const all = {
            [CategoryType.Expense]: [visibleParent],
            [CategoryType.Income]: [category('income')]
        };

        expect(Object.keys(allTransactionCategoriesWithVisibleCount(all, { [CategoryType.Expense]: true }))).toEqual([String(CategoryType.Expense)]);
        expect(Object.keys(allTransactionCategoriesWithVisibleCount(all, { [String(CategoryType.Income)]: true }))).toEqual([String(CategoryType.Income)]);
        expect(Object.keys(allTransactionCategoriesWithVisibleCount(all, {}))).toEqual([String(CategoryType.Income), String(CategoryType.Expense)]);
    });

    test('visible-primary filtering handles absent types and hidden entries', () => {
        const all = { [CategoryType.Expense]: [hiddenParent, visibleParent] };
        expect(allVisiblePrimaryTransactionCategoriesByType(all, CategoryType.Expense)).toEqual([visibleParent]);
        expect(allVisiblePrimaryTransactionCategoriesByType(all, CategoryType.Income)).toEqual([]);
    });

    test('category id filtering includes only categories whose effective selection is checked', () => {
        const leaf = category('leaf');
        const parent = category('parent', { subCategories: [{ ...category('child', { parentId: 'parent' }), hidden: false }] });
        const map = { 1: leaf, 2: parent };

        expect(getFinalCategoryIdsByFilteredCategoryIds(map, { leaf: false, child: false })).toBe('leaf,parent');
        expect(getFinalCategoryIdsByFilteredCategoryIds(map, { leaf: true, child: false })).toBe('parent');
        expect(getFinalCategoryIdsByFilteredCategoryIds(map, { leaf: false, child: true })).toBe('leaf');
        expect(getFinalCategoryIdsByFilteredCategoryIds(null as unknown as typeof map, {})).toBe('');
    });
});

describe('category id availability and display boundaries', () => {
    const hiddenSub = category('hidden-sub', { parentId: 'visible', hidden: true });
    const visibleSub = category('visible-sub', { parentId: 'visible' });
    const tree = [
        category('hidden-parent', { hidden: true, subCategories: [{ ...visibleSub, hidden: false }] }),
        category('without-children', { parentId: 'detached-parent' }),
        category('visible', { subCategories: [{ ...hiddenSub, hidden: true }, { ...visibleSub, hidden: false }] })
    ];

    test('sub-category availability skips hidden parents, missing lists, and hidden children', () => {
        expect(isSubCategoryIdAvailable([], 'visible-sub')).toBe(false);
        expect(isSubCategoryIdAvailable(tree, 'visible-sub')).toBe(true);
        expect(isSubCategoryIdAvailable(tree, 'hidden-sub')).toBe(false);
        expect(isSubCategoryIdAvailable(tree, 'missing')).toBe(false);
    });

    test('first available ids skip hidden and structurally unavailable categories', () => {
        expect(getFirstAvailableCategoryId()).toBe('');
        expect(getFirstAvailableCategoryId(tree)).toBe('visible-sub');
        expect(getFirstAvailableCategoryId([category('empty')])).toBe('');

        expect(getFirstAvailableSubCategoryId([], 'visible')).toBe('');
        expect(getFirstAvailableSubCategoryId(tree, 'hidden-parent')).toBe('');
        expect(getFirstAvailableSubCategoryId(tree, 'without-children')).toBe('');
        expect(getFirstAvailableSubCategoryId(tree, 'visible')).toBe('visible-sub');
        expect(getFirstAvailableSubCategoryId(tree, 'missing')).toBe('');
        expect(getFirstAvailableSubCategoryId([category('only-hidden', { subCategories: [{ ...hiddenSub, hidden: true }] })], 'only-hidden')).toBe('');
    });

    test('available count and first/last showing ids respect showHidden', () => {
        const categories = [category('hidden-first', { hidden: true }), category('visible'), category('hidden-last', { hidden: true })];

        expect(isNoAvailableCategory(categories, false)).toBe(false);
        expect(isNoAvailableCategory([category('hidden', { hidden: true })], false)).toBe(true);
        expect(isNoAvailableCategory([category('hidden', { hidden: true })], true)).toBe(false);
        expect(getAvailableCategoryCount(categories, false)).toBe(1);
        expect(getAvailableCategoryCount(categories, true)).toBe(3);
        expect(getFirstShowingId(categories, false)).toBe('visible');
        expect(getFirstShowingId(categories, true)).toBe('hidden-first');
        expect(getFirstShowingId([category('hidden', { hidden: true })], false)).toBeNull();
        expect(getLastShowingId(categories, false)).toBe('visible');
        expect(getLastShowingId(categories, true)).toBe('hidden-last');
        expect(getLastShowingId([category('hidden', { hidden: true })], false)).toBeNull();
    });
});

describe('category availability aggregates and selection state', () => {
    const emptySummary: TransactionCategoriesWithVisibleCount = {
        type: CategoryType.Income,
        allCategories: [],
        allVisibleCategoryCount: 0,
        firstVisibleCategoryIndex: -1,
        allSubCategories: {},
        allVisibleSubCategoryCounts: {},
        allFirstVisibleSubCategoryIndexes: {}
    };
    const populatedSummary: TransactionCategoriesWithVisibleCount = {
        ...emptySummary,
        type: CategoryType.Expense,
        allCategories: [category('hidden', { hidden: true })],
        allVisibleCategoryCount: 1
    };
    const summaries = { [CategoryType.Income]: emptySummary, [CategoryType.Expense]: populatedSummary };

    test('aggregate availability supports visible-only and show-hidden views', () => {
        expect(containsAnyAvailableCategory({}, false)).toBe(false);
        expect(containsAnyAvailableCategory(summaries, false)).toBe(true);
        expect(containsAnyAvailableCategory({ [CategoryType.Income]: emptySummary }, true)).toBe(false);
        expect(containsAnyAvailableCategory(summaries, true)).toBe(true);
        expect(containsAvailableCategory(summaries, false)).toEqual({
            [CategoryType.Income]: false,
            [CategoryType.Expense]: true
        });
        expect(containsAvailableCategory(summaries, true)).toEqual({
            [CategoryType.Income]: false,
            [CategoryType.Expense]: true
        });
    });

    test('sub-category bulk selection is a no-op without children and updates every child otherwise', () => {
        const filters = { child1: false, child2: false };
        selectAllSubCategories(filters, true);
        selectAllSubCategories(filters, true, category('empty'));
        expect(filters).toEqual({ child1: false, child2: false });

        selectAllSubCategories(filters, true, category('parent', { subCategories: [
            { ...category('child1', { parentId: 'parent' }), hidden: false },
            { ...category('child2', { parentId: 'parent' }), hidden: false }
        ] }));
        expect(filters).toEqual({ child1: true, child2: true });
    });

    test('category selection helpers update existing categories and honor visibility ancestry', () => {
        const visibleParent = category('parent');
        const hiddenParent = category('hidden-parent', { hidden: true });
        const visibleChild = category('child', { parentId: 'parent' });
        const hiddenChild = category('hidden-child', { parentId: 'parent', hidden: true });
        const childOfHidden = category('child-of-hidden', { parentId: 'hidden-parent' });
        const map = { parent: visibleParent, 'hidden-parent': hiddenParent, child: visibleChild, 'hidden-child': hiddenChild, 'child-of-hidden': childOfHidden };

        const visibleFilters = { parent: true, 'hidden-parent': true, child: true, 'hidden-child': true, 'child-of-hidden': true, missing: true };
        selectAllVisible(visibleFilters, map);
        expect(visibleFilters).toEqual({ parent: false, 'hidden-parent': true, child: false, 'hidden-child': true, 'child-of-hidden': true, missing: true });

        const allFilters = { parent: true, missing: true };
        selectAll(allFilters, map);
        expect(allFilters).toEqual({ parent: false, missing: true });

        const noneFilters = { parent: false, missing: false };
        selectNone(noneFilters, map);
        expect(noneFilters).toEqual({ parent: true, missing: false });

        const invertFilters = { parent: true, missing: false };
        selectInvert(invertFilters, map);
        expect(invertFilters).toEqual({ parent: false, missing: false });
    });

    test('checked-state helpers distinguish leaf, all, none, and partial child selections', () => {
        const leaf = category('leaf');
        const parent = category('parent', { subCategories: [
            { ...category('one', { parentId: 'parent' }), hidden: false },
            { ...category('two', { parentId: 'parent' }), hidden: false }
        ] });

        expect(isCategoryOrSubCategoriesAllChecked(leaf, { leaf: false })).toBe(true);
        expect(isCategoryOrSubCategoriesAllChecked(leaf, { leaf: true })).toBe(false);
        expect(isCategoryOrSubCategoriesAllChecked(parent, { one: false, two: false })).toBe(true);
        expect(isCategoryOrSubCategoriesAllChecked(parent, { one: false, two: true })).toBe(false);
        expect(isSubCategoriesAllChecked(leaf, {})).toBe(false);
        expect(isSubCategoriesAllChecked(parent, { one: false, two: false })).toBe(true);
        expect(isSubCategoriesAllChecked(parent, { one: false, two: true })).toBe(false);
        expect(isSubCategoriesHasButNotAllChecked(leaf, {})).toBe(false);
        expect(isSubCategoriesHasButNotAllChecked(parent, { one: true, two: true })).toBe(false);
        expect(isSubCategoriesHasButNotAllChecked(parent, { one: false, two: true })).toBe(true);
        expect(isSubCategoriesHasButNotAllChecked(parent, { one: false, two: false })).toBe(false);
    });
});
