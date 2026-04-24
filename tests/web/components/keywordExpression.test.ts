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
    RULE_EXPRESSION_UNBALANCED_KEY
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

    test('keeps legacy REGEX clauses parseable and serializable', () => {
        const parsed = parseExpression('REGEX:^coffee.*&OR:breakfast|brunch', {
            format: 'legacy',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('legacy');
        expect(parsed.clauses.map(clause => clause.operator)).toStrictEqual(['REGEX', 'OR']);
        expect(parsed.clauses[0]?.terms).toStrictEqual(['^coffee.*']);
        expect(serializeForFormat(parsed.clauses, 'legacy')).toStrictEqual({
            expression: 'REGEX:^coffee.*&OR:breakfast|brunch'
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

    test('keeps composite REGEX clauses parseable and serializable', () => {
        const expression = 'REGEX={^coffee.*,tea\\,milk}+NOT={refund}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses.map(clause => clause.operator)).toStrictEqual(['REGEX', 'NOT']);
        expect(parsed.clauses[0]?.terms).toStrictEqual(['^coffee.*', 'tea,milk']);
        expect(serializeForFormat(parsed.clauses, 'composite')).toStrictEqual({
            expression
        });
    });

    test('parses and serializes expression-level OR groups for multiple expressions', () => {
        const expression = 'OR={早餐}|AND={咖啡}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { joiner: 'AND', operator: 'OR', terms: ['早餐'] },
            { joiner: 'OR', operator: 'AND', terms: ['咖啡'] }
        ]);
        expect(serializeForFormat(parsed.clauses, 'composite')).toStrictEqual({
            expression
        });
    });

    test('accepts visible connector aliases but serializes back to canonical composite syntax', () => {
        const expression = 'OR={早餐}/OR={咖啡}×NOT={退款} NOT OR={测试}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { joiner: 'AND', operator: 'OR', terms: ['早餐'] },
            { joiner: 'OR', operator: 'OR', terms: ['咖啡'] },
            { joiner: 'AND', operator: 'NOT', terms: ['退款'] },
            { joiner: 'AND', operator: 'NOT', terms: ['测试'] }
        ]);
        expect(serializeForFormat(parsed.clauses, 'composite')).toStrictEqual({
            expression: 'OR={早餐}|OR={咖啡}+NOT={退款}+NOT={测试}'
        });
    });

    test('treats bare visible multiplication connector as canonical AND instead of NOT', () => {
        const expression = 'OR={早餐}×OR={咖啡}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { joiner: 'AND', operator: 'OR', terms: ['早餐'] },
            { joiner: 'AND', operator: 'OR', terms: ['咖啡'] }
        ]);
        expect(serializeForFormat(parsed.clauses, 'composite')).toStrictEqual({
            expression: 'OR={早餐}+OR={咖啡}'
        });
    });

    test('keeps escaped pipe terms inside one expression while splitting top-level expressions', () => {
        const expression = 'OR={A\\|B}|OR={C}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { joiner: 'AND', operator: 'OR', terms: ['A|B'] },
            { joiner: 'OR', operator: 'OR', terms: ['C'] }
        ]);
        expect(serializeComposite(parsed.clauses)).toStrictEqual({ expression });
    });

    test('roundtrips escaped slash and multiplication literals without double escaping', () => {
        const expression = 'OR={拿铁\\/燕麦\\×热}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { joiner: 'AND', operator: 'OR', terms: ['拿铁/燕麦×热'] }
        ]);
        expect(serializeComposite(parsed.clauses)).toStrictEqual({ expression });
    });

    test('repairs escaped nested composite expressions instead of preserving malformed text terms', () => {
        const expression = 'OR={\\(OR=\\{共享单车\\,摩拜\\}+OR=\\{549\\}\\)}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { joiner: 'AND', operator: 'OR', terms: ['共享单车', '摩拜'], openParens: 1, closeParens: 0 },
            { joiner: 'AND', operator: 'OR', terms: ['549'], openParens: 0, closeParens: 1 }
        ]);
        expect(serializeComposite(parsed.clauses)).toStrictEqual({
            expression: '(OR={共享单车,摩拜}+OR={549})'
        });
    });

    test('does not repair nested composite literals under NOT wrappers', () => {
        const expression = 'NOT={\\(OR=\\{退款\\}\\)}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { joiner: 'AND', operator: 'NOT', terms: ['(OR={退款})'], openParens: 0, closeParens: 0 }
        ]);
        expect(serializeComposite(parsed.clauses)).toStrictEqual({
            expression
        });
        expect(serializeComposite(parsed.clauses)).not.toStrictEqual({
            expression: 'OR={退款}'
        });
    });

    test('does not repair nested composite literals under AND wrappers', () => {
        const expression = 'AND={\\(OR=\\{门店\\}\\)}';
        const parsed = parseExpression(expression, {
            format: 'composite',
            idFactory: createIdFactory()
        });

        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses).toMatchObject([
            { joiner: 'AND', operator: 'AND', terms: ['(OR={门店})'], openParens: 0, closeParens: 0 }
        ]);
        expect(serializeComposite(parsed.clauses)).toStrictEqual({
            expression
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

    test('normalizes duplicate terms so a single Enter does not serialize duplicated chips', () => {
        const clause = createRuleClause({ id: 'clause-1', operator: 'OR', terms: ['咖啡', '咖啡', ' 早餐 '] });

        expect(clause.terms).toStrictEqual(['咖啡', '早餐']);
        expect(serializeComposite([clause])).toStrictEqual({
            expression: 'OR={咖啡,早餐}'
        });
    });

    test('preserves expression OR boundary when an expression starts with an empty clause', () => {
        const clauses = [
            createRuleClause({ id: 'first', operator: 'OR', terms: ['早餐'] }),
            createRuleClause({ id: 'empty-expression-head', joiner: 'OR', operator: 'OR', terms: [] }),
            createRuleClause({ id: 'second-expression-term', joiner: 'AND', operator: 'NOT', terms: ['退款'] }),
        ];

        expect(serializeComposite(clauses)).toStrictEqual({
            expression: 'OR={早餐}|NOT={退款}'
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

describe('keyword expression component copy', () => {
    test('builder components no longer render stale helper/example copy or priority control', () => {
        const componentFiles = [
            path.resolve(process.cwd(), 'src/components/common/KeywordInput.vue'),
            path.resolve(process.cwd(), 'src/components/common/CategoryRuleBuilderFields.vue'),
        ];
        const combinedSource = componentFiles
            .map(componentPath => fs.readFileSync(componentPath, 'utf-8'))
            .join('\n');

        expect(combinedSource).not.toContain(
            'Add one or more expressions. Each expression contains rule blocks joined by AND; expressions are joined by OR.'
        );
        expect(combinedSource).not.toContain('Example: OR={早餐,咖啡}+NOT={退款}|OR={午餐}');
        expect(combinedSource).not.toContain('Rule Priority');
    });
});
