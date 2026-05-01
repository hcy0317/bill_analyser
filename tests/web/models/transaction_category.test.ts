import { describe, expect, test } from '@jest/globals';

import { DEFAULT_CATEGORY_COLOR } from '@/consts/color.ts';
import { DEFAULT_CATEGORY_ICON_ID } from '@/consts/icon.ts';
import { CategoryType } from '@/core/category.ts';
import { resolveCategoryIcon } from '@/lib/icon.ts';
import {
    TransactionCategory,
    type TransactionCategoryInfoResponse
} from '@/models/transaction_category.ts';

const SAMPLE_CATEGORY_RESPONSE: TransactionCategoryInfoResponse = {
    id: '100',
    name: '餐饮',
    parentId: '0',
    type: CategoryType.Expense,
    icon: 'mdi-food',
    color: '#5470c6',
    comment: '主分类',
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
            comment: '子分类',
            displayOrder: 1,
            hidden: false
        }
    ]
};

describe('TransactionCategory model', () => {
    test('TransactionCategory.of builds nested categories and hidden getter', () => {
        const category = TransactionCategory.of(SAMPLE_CATEGORY_RESPONSE);

        expect(category.id).toBe('100');
        expect(category.name).toBe('餐饮');
        expect(category.hidden).toBe(false);
        expect(category.subCategories).toHaveLength(1);
        expect(category.subCategories?.[0]?.name).toBe('早餐');
        expect(category.subCategories?.[0]?.hidden).toBe(false);
    });

    test('TransactionCategory.of validates required fields', () => {
        expect(() => TransactionCategory.of(null)).toThrow('JSON cannot be null');
        expect(() => TransactionCategory.of({ name: '缺少ID' })).toThrow('Field "id" is invalid');
        expect(() => TransactionCategory.of({ id: '1' })).toThrow('Field "name" is invalid');
    });

    test('TransactionCategory.equals compares nested subcategories', () => {
        const first = TransactionCategory.of(SAMPLE_CATEGORY_RESPONSE);
        const second = TransactionCategory.of(SAMPLE_CATEGORY_RESPONSE);
        const changedNested = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE,
            subCategories: [
                {
                    ...SAMPLE_CATEGORY_RESPONSE.subCategories?.[0]!,
                    name: '午餐'
                }
            ]
        });
        const changedFlat = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE,
            color: '#ee6666'
        });

        expect(first.equals(second)).toBe(true);
        expect(first.equals(changedNested)).toBe(false);
        expect(first.equals(changedFlat)).toBe(false);
    });

    test('TransactionCategory.equals rejects mismatched child counts and nested-vs-leaf shapes', () => {
        const parentWithOneChild = TransactionCategory.of(SAMPLE_CATEGORY_RESPONSE);
        const parentWithTwoChildren = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE,
            subCategories: [
                ...SAMPLE_CATEGORY_RESPONSE.subCategories!,
                {
                    id: '102',
                    name: '午餐',
                    parentId: '100',
                    type: CategoryType.Expense,
                    icon: 'mdi-silverware-fork-knife',
                    color: '#fac858',
                    comment: '额外子分类',
                    displayOrder: 2,
                    hidden: false
                }
            ]
        });
        const leafCategory = TransactionCategory.of(SAMPLE_CATEGORY_RESPONSE.subCategories?.[0]!);
        const nestedCategory = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE.subCategories?.[0]!,
            subCategories: [
                {
                    id: '103',
                    name: '咖啡',
                    parentId: '101',
                    type: CategoryType.Expense,
                    icon: 'mdi-coffee',
                    color: '#73c0de',
                    comment: '孙分类',
                    displayOrder: 1,
                    hidden: false
                }
            ]
        });

        expect(parentWithOneChild.equals(parentWithTwoChildren)).toBe(false);
        expect(leafCategory.equals(nestedCategory)).toBe(false);
        expect(nestedCategory.equals(leafCategory)).toBe(false);
    });

    test('TransactionCategory.fillFrom copies mutable fields', () => {
        const target = TransactionCategory.of(SAMPLE_CATEGORY_RESPONSE);
        const source = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE,
            id: '200',
            name: '交通',
            color: '#fac858',
            comment: '已更新',
            hidden: true,
            displayOrder: 9,
            ruleExpression: 'OR={地铁,打车}'
        });

        target.fillFrom(source);

        expect(target.id).toBe('200');
        expect(target.name).toBe('交通');
        expect(target.color).toBe('#fac858');
        expect(target.comment).toBe('已更新');
        expect(target.displayOrder).toBe(9);
        expect(target.visible).toBe(false);
        expect(target.ruleExpression).toBe('OR={地铁,打车}');
        expect(target.keywords).toBe('OR={地铁,打车}');
    });

    test('TransactionCategory request converters preserve canonical rule expression and legacy alias', () => {
        const category = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE,
            ruleExpression: 'OR={早餐,咖啡}'
        });

        expect(category.toCreateRequest('session-1')).toStrictEqual({
            name: '餐饮',
            type: CategoryType.Expense,
            parentId: '0',
            icon: 'mdi-food',
            color: '#5470c6',
            comment: '主分类',
            displayOrder: 1,
            ruleExpression: 'OR={早餐,咖啡}',
            keywords: 'OR={早餐,咖啡}',
            clientSessionId: 'session-1'
        });

        expect(category.toModifyRequest()).toStrictEqual({
            id: '100',
            name: '餐饮',
            parentId: '0',
            icon: 'mdi-food',
            color: '#5470c6',
            comment: '主分类',
            displayOrder: 1,
            ruleExpression: 'OR={早餐,咖啡}',
            keywords: 'OR={早餐,咖啡}',
            hidden: false
        });
    });

    test('TransactionCategory.of accepts legacy keywords payloads as ruleExpression compatibility input', () => {
        const category = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE,
            keywords: '早餐|咖啡'
        });

        expect(category.ruleExpression).toBe('早餐|咖啡');
        expect(category.keywords).toBe('早餐|咖啡');
    });

    test('TransactionCategory icons are display-resolved through the shared category resolver', () => {
        const legacyCategory = TransactionCategory.of(SAMPLE_CATEGORY_RESPONSE);
        const presetCategory = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE,
            icon: 830 as unknown as string
        });
        const lineAwesomeCategory = TransactionCategory.of({
            ...SAMPLE_CATEGORY_RESPONSE,
            icon: 'las la-wallet'
        });

        expect(resolveCategoryIcon(legacyCategory.icon)).toBe('las la-utensils');
        expect(resolveCategoryIcon(presetCategory.icon)).toBe('las la-chart-pie');
        expect(resolveCategoryIcon(lineAwesomeCategory.icon)).toBe('las la-wallet');
    });

    test('TransactionCategory collection helpers build arrays, maps and lookups', () => {
        const multi = TransactionCategory.ofMulti([SAMPLE_CATEGORY_RESPONSE]);
        const mapped = TransactionCategory.ofMap({
            [CategoryType.Expense]: [SAMPLE_CATEGORY_RESPONSE]
        });

        expect(multi).toHaveLength(1);
        expect(mapped[CategoryType.Expense]).toHaveLength(1);
        expect(TransactionCategory.findNameById(multi, '100')).toBe('餐饮');
        expect(TransactionCategory.findNameById(multi, '999')).toBeNull();
    });

    test('TransactionCategory.createNewCategory uses defaults for type, parent and presentation', () => {
        const defaultCategory = TransactionCategory.createNewCategory();
        const childCategory = TransactionCategory.createNewCategory(CategoryType.Income, '500');

        expect(defaultCategory.parentId).toBe('0');
        expect(defaultCategory.type).toBe(CategoryType.Income);
        expect(defaultCategory.icon).toBe(DEFAULT_CATEGORY_ICON_ID);
        expect(defaultCategory.color).toBe(DEFAULT_CATEGORY_COLOR);
        expect(defaultCategory.subCategories).toStrictEqual([]);

        expect(childCategory.parentId).toBe('500');
        expect(childCategory.type).toBe(CategoryType.Income);
        expect(childCategory.subCategories).toBeUndefined();
    });
});
