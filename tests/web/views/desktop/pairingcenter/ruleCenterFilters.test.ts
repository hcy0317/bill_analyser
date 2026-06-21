import { describe, expect, test } from '@jest/globals';

import { CategoryType } from '@/core/category.ts';
import {
    matchesRuleExpressionFilterText,
    parseRuleExpressionFilterQuery,
    primaryCategoryFilterTypeOrder,
} from '@/views/desktop/pairingcenter/components/ruleCenterFilters.ts';

describe('rule center filter expression contract', () => {
    test('parses include alternatives and exclusion aliases with full-width punctuation', () => {
        const parsed = parseRuleExpressionFilterQuery('咖啡|茶 NOT=（退款|撤销） ！(测试)');

        expect(matchesRuleExpressionFilterText('星巴克咖啡', parsed, false)).toBe(true);
        expect(matchesRuleExpressionFilterText('热茶 免单', parsed, false)).toBe(true);
        expect(matchesRuleExpressionFilterText('星巴克咖啡 退款', parsed, false)).toBe(false);
        expect(matchesRuleExpressionFilterText('热茶 测试', parsed, false)).toBe(false);
        expect(matchesRuleExpressionFilterText('午餐', parsed, false)).toBe(false);
    });

    test('keeps regex mode bounded and fails closed for invalid patterns', () => {
        const parsed = parseRuleExpressionFilterQuery('^STAR.*KS$ NOT=(refund|void)');

        expect(matchesRuleExpressionFilterText('starbucks', parsed, true)).toBe(true);
        expect(matchesRuleExpressionFilterText('starbucks refund', parsed, true)).toBe(false);
        expect(matchesRuleExpressionFilterText('coffee shop', parsed, true)).toBe(false);

        const invalid = parseRuleExpressionFilterQuery('[');
        expect(matchesRuleExpressionFilterText('anything', invalid, true)).toBe(false);
    });

    test('keeps primary category filter ordering stable for rule center tables', () => {
        expect(primaryCategoryFilterTypeOrder).toStrictEqual([
            CategoryType.Income,
            CategoryType.Expense,
            CategoryType.Transfer,
            CategoryType.Investment,
        ]);
    });
});
