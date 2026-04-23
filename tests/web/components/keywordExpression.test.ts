import { describe, expect, test } from '@jest/globals';
import fs from 'node:fs';
import path from 'node:path';

import {
    addTermToClause,
    createRuleClause,
    insertClauseAfter,
    parseExpression,
    removeTermFromClause,
    serializeComposite,
    serializeForFormat,
    validateParentheses,
    RULE_EXPRESSION_UNBALANCED_KEY,
    RULE_EXPRESSION_UNPARSEABLE_KEY
} from '@/components/common/keywordExpression.ts';

function createIdFactory(): () => string {
    let id = 0;
    return () => {
        id += 1;
        return `clause-${id}`;
    };
}

describe('keywordExpression helpers', () => {
    test('parses and serializes legacy expressions without changing legacy callers', () => {
        const parsed = parseExpression('OR:早餐|咖啡&AND:星巴克&NOT:退款', {
            format: 'legacy',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('legacy');
        expect(parsed.clauses).toMatchObject([
            { operator: 'OR', terms: ['早餐', '咖啡'], openParens: 0, closeParens: 0 },
            { operator: 'AND', terms: ['星巴克'], openParens: 0, closeParens: 0 },
            { operator: 'NOT', terms: ['退款'], openParens: 0, closeParens: 0 }
        ]);
        expect(serializeForFormat(parsed.clauses, 'legacy')).toStrictEqual({
            expression: 'OR:早餐|咖啡&AND:星巴克&NOT:退款'
        });
    });

    test('parses parenthesized composite expressions and roundtrips escaped terms', () => {
        const expression = '(OR={早餐\\,咖啡,星巴克}+AND={门店\\+优惠})+NOT={退款\\(退货\\)}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { operator: 'OR', terms: ['早餐,咖啡', '星巴克'], openParens: 1, closeParens: 0 },
            { operator: 'AND', terms: ['门店+优惠'], openParens: 0, closeParens: 1 },
            { operator: 'NOT', terms: ['退款(退货)'], openParens: 0, closeParens: 0 }
        ]);
        expect(serializeComposite(parsed.clauses)).toStrictEqual({ expression });
    });

    test('returns raw mode for unsupported expression-level OR instead of losing the original rule', () => {
        const expression = 'OR={早餐}|AND={咖啡}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed).toStrictEqual({
            clauses: [],
            sourceFormat: 'raw',
            rawExpression: expression,
            errorKey: RULE_EXPRESSION_UNPARSEABLE_KEY
        });
    });

    test('supports Enter-style chip creation and removable chips as clause updates', () => {
        const initialClause = createRuleClause({ id: 'clause-1', operator: 'OR', terms: ['早餐'] });
        const withEnteredChip = addTermToClause(initialClause, ' 咖啡 ');
        const afterRemovingFirstChip = removeTermFromClause(withEnteredChip, 0);

        expect(withEnteredChip.terms).toStrictEqual(['早餐', '咖啡']);
        expect(afterRemovingFirstChip.terms).toStrictEqual(['咖啡']);
        expect(serializeComposite([afterRemovingFirstChip])).toStrictEqual({
            expression: 'OR={咖啡}'
        });
    });

    test('inserts a new default OR clause after the selected row', () => {
        const firstClause = createRuleClause({ id: 'first', operator: 'AND', terms: ['门店'] });
        const secondClause = createRuleClause({ id: 'second', operator: 'NOT', terms: ['退款'] });
        const insertedClause = createRuleClause({ id: 'inserted', operator: 'OR', terms: ['咖啡'] });

        const clauses = insertClauseAfter([firstClause, secondClause], 'first', insertedClause);

        expect(clauses.map(clause => clause.id)).toStrictEqual(['first', 'inserted', 'second']);
        expect(serializeComposite(clauses)).toStrictEqual({
            expression: 'AND={门店}+OR={咖啡}+NOT={退款}'
        });
    });

    test('reports unbalanced parentheses and blocks composite serialization', () => {
        const invalidClauses = [
            createRuleClause({ id: 'first', operator: 'OR', terms: ['早餐'], closeParens: 1 })
        ];

        expect(validateParentheses(invalidClauses)).toStrictEqual({
            valid: false,
            errorKey: RULE_EXPRESSION_UNBALANCED_KEY
        });
        expect(serializeComposite(invalidClauses)).toStrictEqual({
            expression: '',
            errorKey: RULE_EXPRESSION_UNBALANCED_KEY
        });
    });
});

describe('keyword expression i18n keys', () => {
    test('new row editor messages exist in all frontend locale packs', () => {
        const requiredKeys = [
            'Build a boolean rule expression with OR / AND / NOT rows. Use parentheses on any row to control precedence; each row serializes to backend-supported OR={...}+AND={...}+NOT={...} syntax.',
            'Rule expression parentheses are not balanced',
            'Rule expression uses unsupported syntax and is preserved as raw text',
            'Original rule expression',
            'Regex Patterns',
            'Left parentheses',
            'Right parentheses',
            'Add left parenthesis',
            'Remove left parenthesis',
            'Add right parenthesis',
            'Remove right parenthesis',
            'Insert clause after this row'
        ];
        const localeFiles = ['en.json', 'zh_Hans.json', 'zh_Hant.json'];

        for (const localeFile of localeFiles) {
            const localePath = path.resolve(process.cwd(), 'src/locales', localeFile);
            const locale = JSON.parse(fs.readFileSync(localePath, 'utf-8')) as Record<string, string>;
            for (const key of requiredKeys) {
                expect(locale[key]).toBeTruthy();
            }
        }
    });
});
