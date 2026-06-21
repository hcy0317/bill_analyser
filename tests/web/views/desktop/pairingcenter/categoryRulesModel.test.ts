import { describe, expect, test } from '@jest/globals';

import { CategoryType } from '@/core/category.ts';
import type { LocalizedPresetCategory } from '@/core/category.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import {
    buildCategoryPickerItems,
    buildCategoryRulePayload,
    buildCategoryRuleTargetGroups,
    buildDisplayCategoryRules,
    buildGroupedCategoryRuleTargets,
    buildLocalizedPresetPrimaryCategoryMap,
    compareDisplayCategoryRuleOrder,
    getCategoryRuleTargetRuleIds,
    makePrimaryCategoryGroupKey,
    normalizeCategoryRuleItem,
    resolveRuleCategorySelection,
    type CategoryRuleItem,
} from '@/views/desktop/pairingcenter/components/categoryRulesModel.ts';

function makeCategory(input: Partial<TransactionCategory> & {
    id: string;
    name: string;
    type: CategoryType;
}): TransactionCategory {
    return {
        parentId: '0',
        icon: '',
        color: '',
        hidden: false,
        subCategories: [],
        ...input,
    } as TransactionCategory;
}

function makeRule(input: Partial<CategoryRuleItem> & { id: number }): CategoryRuleItem {
    return {
        name: `rule-${input.id}`,
        category_id: null,
        category_name: null,
        sub_category_name: null,
        priority: 100,
        rule_expression: 'coffee',
        regex_enabled: false,
        enabled: true,
        applied_count: 0,
        ...input,
        id: input.id,
    };
}

describe('category rule view-model helpers', () => {
    test('builds picker items and resolves selected category labels', () => {
        const expense = makeCategory({
            id: '10',
            name: 'Food',
            type: CategoryType.Expense,
            icon: 'food',
            color: '#f00',
            subCategories: [
                makeCategory({
                    id: '11',
                    name: 'Coffee',
                    type: CategoryType.Expense,
                    parentId: '10',
                    icon: 'coffee',
                    color: '#a50',
                }),
            ],
        });

        const items = buildCategoryPickerItems(
            { [CategoryType.Expense]: [expense] },
            type => `type-${type}`
        );

        expect(items[0]).toMatchObject({
            id: '10',
            name: 'Food',
            typeLabel: `type-${CategoryType.Expense}`,
        });
        expect(resolveRuleCategorySelection('11', items)).toEqual({
            primaryText: 'Food',
            secondaryText: 'Coffee',
            label: 'Food / Coffee',
        });
        expect(resolveRuleCategorySelection('missing', items).label).toBe('');
    });

    test('builds localized fallback display and grouped target rows', () => {
        const categoryMap: Record<string, TransactionCategory> = {
            '10': makeCategory({
                id: '10',
                name: 'Food',
                type: CategoryType.Expense,
                icon: 'food',
                color: '#f00',
            }),
            '11': makeCategory({
                id: '11',
                name: 'Coffee',
                type: CategoryType.Expense,
                parentId: '10',
                icon: 'coffee',
                color: '#a50',
            }),
        };
        const presetMap = buildLocalizedPresetPrimaryCategoryMap(['zh-Hans'], () => ({
            expense: [{
                name: 'Travel',
                type: CategoryType.Expense,
                icon: 'travel',
                color: '#0af',
                subCategories: [],
            } satisfies LocalizedPresetCategory],
        }));
        const displayRules = buildDisplayCategoryRules([
            makeRule({
                id: 1,
                name: 'b-rule',
                category_id: 11,
                category_name: 'Food',
                sub_category_name: 'Coffee',
                priority: 20,
            }),
            makeRule({
                id: 2,
                name: 'a-rule',
                category_id: null,
                category_name: 'Travel',
                priority: 10,
            }),
        ], {
            categoriesById: categoryMap,
            categoryPickerItems: [],
            localizedPresetPrimaryCategoryMap: presetMap,
            unassignedLabel: 'Unassigned',
        });

        expect(displayRules[0]).toMatchObject({
            category_display_name: 'Coffee',
            category_group_name: 'Food',
        });
        expect(displayRules[1]).toMatchObject({
            category_display_name: 'Travel',
            category_group_icon: 'travel',
        });
        expect([...displayRules].sort(compareDisplayCategoryRuleOrder).map(rule => rule.id)).toEqual([1, 2]);

        const targets = buildCategoryRuleTargetGroups(displayRules);
        expect(targets).toHaveLength(2);
        expect(getCategoryRuleTargetRuleIds(targets[0]!)).toEqual([1]);

        const grouped = buildGroupedCategoryRuleTargets(targets);
        expect(grouped.map(group => group.title)).toEqual(['Food', 'Travel']);
    });

    test('normalizes API rows and validates save payloads', () => {
        expect(makePrimaryCategoryGroupKey(null, ' Food ')).toBe('category-name:food');
        expect(normalizeCategoryRuleItem({
            ...makeRule({ id: 3 }),
            main_category: 'Food',
            sub_category: 'Coffee',
            regex_enabled: 1 as unknown as boolean,
            enabled: 0 as unknown as boolean,
            applied_count: '5' as unknown as number,
        })).toMatchObject({
            category_name: 'Food',
            sub_category_name: 'Coffee',
            regex_enabled: true,
            enabled: false,
            applied_count: 5,
        });

        expect(buildCategoryRulePayload({
            category_id: '11',
            priority: 50,
            rule_expression: ' coffee ',
            regex_enabled: true,
            enabled: false,
        }, 'Coffee · Category Rule', {
            categoryRequired: 'Category is required',
            expressionRequired: 'Expression is required',
        })).toEqual({
            category_id: 11,
            name: 'Coffee · Category Rule',
            priority: 50,
            rule_expression: 'coffee',
            regex_enabled: true,
            enabled: false,
        });

        expect(() => buildCategoryRulePayload({
            category_id: '',
            priority: 50,
            rule_expression: 'coffee',
            regex_enabled: false,
            enabled: true,
        }, 'name', {
            categoryRequired: 'Category is required',
            expressionRequired: 'Expression is required',
        })).toThrow('Category is required');
    });
});
