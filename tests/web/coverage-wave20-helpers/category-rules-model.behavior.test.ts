import { describe, expect, test } from '@jest/globals';

import { CategoryType, type LocalizedPresetCategory } from '@/core/category.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import {
    buildCategoryPickerItems,
    buildCategoryRulePayload,
    buildCategoryRuleTargetGroups,
    buildDisplayCategoryRules,
    buildGroupedCategoryRuleTargets,
    buildLocalizedPresetPrimaryCategoryMap,
    compareDisplayCategoryRuleOrder,
    makePrimaryCategoryGroupKey,
    normalizeCategoryRuleItem,
    resolveRuleCategorySelection,
    type CategoryPickerPrimaryItem,
    type CategoryRuleItem,
    type DisplayCategoryRuleItem
} from '@/views/desktop/pairingcenter/components/categoryRulesModel.ts';

function category(overrides: Partial<TransactionCategory> & { id: string; name: string }): TransactionCategory {
    return {
        parentId: '0',
        type: CategoryType.Expense,
        icon: '',
        color: '',
        hidden: false,
        subCategories: [],
        ...overrides
    } as TransactionCategory;
}

function rule(overrides: Partial<CategoryRuleItem> & { id: number }): CategoryRuleItem {
    return {
        name: `rule-${overrides.id}`,
        category_id: null,
        category_name: null,
        sub_category_name: null,
        priority: 100,
        rule_expression: 'coffee',
        regex_enabled: false,
        enabled: true,
        applied_count: 0,
        ...overrides,
        id: overrides.id
    };
}

function displayRule(overrides: Partial<DisplayCategoryRuleItem> & { id: number }): DisplayCategoryRuleItem {
    return {
        ...rule(overrides),
        category_display_name: 'Coffee',
        category_full_name: 'Food / Coffee',
        category_icon: 'coffee',
        category_color: '#a50',
        category_group_key: 'category:food',
        category_group_name: 'Food',
        category_group_icon: 'food',
        category_group_color: '#f00',
        ...overrides
    };
}

describe('category rule model boundary behavior', () => {
    test('builds picker fallbacks and resolves both primary and missing selections', () => {
        const primary = category({
            id: 'food',
            name: 'Food',
            subCategories: undefined
        });
        const items = buildCategoryPickerItems(
            { [CategoryType.Expense]: [primary] },
            type => `type-${type}`
        );

        expect(items).toEqual([
            expect.objectContaining({
                id: 'food',
                name: 'Food',
                typeLabel: `type-${CategoryType.Expense}`,
                subCategories: []
            })
        ]);
        expect(resolveRuleCategorySelection('food', items)).toEqual({
            primaryText: 'Food',
            secondaryText: '',
            label: 'Food'
        });
        expect(resolveRuleCategorySelection('missing', items)).toEqual({
            primaryText: '',
            secondaryText: '',
            label: ''
        });
    });

    test('indexes localized presets while skipping empty, null and duplicate entries', () => {
        const preset = (name: string, icon: string): LocalizedPresetCategory => ({
            name,
            icon,
            color: '#123'
        } as LocalizedPresetCategory);
        const map = buildLocalizedPresetPrimaryCategoryMap(
            ['zh', 'en'],
            (_type, locale) => locale === 'zh'
                ? {
                    empty: null,
                    mixed: [preset('', 'blank'), preset('Food', 'zh-food')]
                } as unknown as Record<string, LocalizedPresetCategory[]>
                : { mixed: [preset(' food ', 'en-food'), preset('Travel', 'travel')] }
        );

        expect([...map.keys()]).toEqual(['food', 'travel']);
        expect(map.get('food')).toEqual({ name: 'Food', icon: 'zh-food', color: '#123' });
    });

    test('rejects empty expressions and normalizes every legacy response fallback', () => {
        expect(() => buildCategoryRulePayload({
            category_id: '7',
            priority: 1,
            rule_expression: undefined as unknown as string,
            regex_enabled: false,
            enabled: true
        }, 'auto', { categoryRequired: 'category', expressionRequired: 'expression' })).toThrow('expression');

        expect(normalizeCategoryRuleItem({
            ...rule({ id: 1 }),
            category_name: null,
            sub_category_name: null,
            applied_count: undefined as unknown as number,
            main_category: 'Legacy main',
            sub_category: 'Legacy sub'
        })).toMatchObject({
            category_name: 'Legacy main',
            sub_category_name: 'Legacy sub',
            applied_count: 0
        });
        expect(normalizeCategoryRuleItem({
            ...rule({ id: 2 }),
            category_name: null,
            sub_category_name: null,
            main_category: null,
            sub_category: null
        })).toMatchObject({ category_name: null, sub_category_name: null });
    });

    test('resolves stored, preset and unassigned category display metadata', () => {
        const primary = category({ id: '10', name: 'Food', icon: 'food', color: '#f00' });
        const secondary = category({
            id: '11',
            name: 'Coffee',
            parentId: '10',
            icon: 'coffee',
            color: '#a50'
        });
        primary.subCategories = [secondary];
        const pickerItems = buildCategoryPickerItems(
            { [CategoryType.Expense]: [primary] },
            String
        );
        const presetMap = new Map([
            ['travel', { id: '20', name: 'Travel', icon: 'plane', color: '#09f' }]
        ]);
        const dependencies = {
            categoriesById: { '10': primary, '11': secondary },
            categoryPickerItems: pickerItems,
            localizedPresetPrimaryCategoryMap: presetMap,
            unassignedLabel: 'Unassigned'
        };

        const rows = buildDisplayCategoryRules([
            rule({ id: 1, category_id: 11, category_name: 'Food', sub_category_name: 'Coffee' }),
            rule({ id: 2, category_name: 'Travel', sub_category_name: 'Taxi' }),
            rule({ id: 3 })
        ], dependencies);

        expect(rows[0]).toMatchObject({
            category_display_name: 'Coffee',
            category_full_name: 'Food / Coffee',
            category_icon: 'coffee',
            category_group_key: 'category:10',
            category_group_name: 'Food'
        });
        expect(rows[1]).toMatchObject({
            category_display_name: 'Taxi',
            category_icon: 'plane',
            category_group_key: 'category:20',
            category_group_name: 'Travel'
        });
        expect(rows[2]).toMatchObject({
            category_display_name: 'Unassigned',
            category_group_key: 'category-name:unassigned',
            category_icon: '',
            category_color: ''
        });

        const emptyLabelRow = buildDisplayCategoryRules([rule({ id: 4 })], {
            ...dependencies,
            categoryPickerItems: [] as CategoryPickerPrimaryItem[],
            localizedPresetPrimaryCategoryMap: new Map(),
            unassignedLabel: ''
        })[0];
        expect(emptyLabelRow).toMatchObject({
            category_display_name: '',
            category_group_key: 'unassigned',
            category_group_name: ''
        });
        const unknownNamedRow = buildDisplayCategoryRules([
            rule({ id: 5, category_name: 'Unknown category' })
        ], dependencies)[0];
        expect(unknownNamedRow).toMatchObject({
            category_display_name: 'Unknown category',
            category_group_key: 'category-name:unknown category',
            category_icon: '',
            category_color: ''
        });
        expect(makePrimaryCategoryGroupKey(null)).toBe('unassigned');
    });

    test('uses every stable sort tie-breaker for targets, groups and rules', () => {
        const rows = [
            displayRule({ id: 3, priority: 2, name: 'B' }),
            displayRule({ id: 2, priority: 1, name: 'B' }),
            displayRule({ id: 1, priority: 1, name: 'A' }),
            displayRule({
                id: 4,
                category_id: null,
                category_full_name: 'Food / Tea',
                category_display_name: 'Tea'
            }),
            displayRule({
                id: 5,
                category_id: null,
                category_full_name: '',
                category_display_name: '',
                category_group_key: 'category-name:food-z',
                category_group_name: 'Food'
            }),
            displayRule({
                id: 6,
                category_id: null,
                category_full_name: 'Books',
                category_display_name: 'Books',
                category_group_key: 'category-name:books',
                category_group_name: 'Books'
            })
        ];

        const targetGroups = buildCategoryRuleTargetGroups(rows);
        const coffee = targetGroups.find(group => group.key === 'category-name:food / coffee');
        expect(coffee?.rules.map(item => item.id)).toEqual([1, 2, 3]);
        expect(targetGroups.map(group => group.category_group_name)).toEqual(['Books', 'Food', 'Food', 'Food']);

        const grouped = buildGroupedCategoryRuleTargets(targetGroups);
        expect(grouped.map(group => group.title)).toEqual(['Books', 'Food', 'Food']);
        expect(grouped.reduce((sum, group) => sum + group.ruleCount, 0)).toBe(rows.length);

        expect(compareDisplayCategoryRuleOrder(
            displayRule({ id: 1, category_full_name: 'A' }),
            displayRule({ id: 2, category_full_name: 'B' })
        )).toBeLessThan(0);
        expect(compareDisplayCategoryRuleOrder(
            displayRule({ id: 1, category_group_key: 'a' }),
            displayRule({ id: 2, category_group_key: 'b' })
        )).toBeLessThan(0);

        const finalKeyTieBreaker = buildCategoryRuleTargetGroups([
            displayRule({ id: 7, category_id: 1 }),
            displayRule({ id: 8, category_id: 2 })
        ]);
        expect(finalKeyTieBreaker.map(group => group.key)).toEqual(['category:1', 'category:2']);
    });
});
