import { describe, expect, test } from '@jest/globals';

import {
    RULE_EXPRESSION_UNBALANCED_KEY,
    RULE_EXPRESSION_UNPARSEABLE_KEY,
    addTermToClause,
    createRuleClause,
    escapeCompositeTerm,
    getRuleExpressionDisplayOperatorColor,
    insertClauseAfter,
    parseExpression,
    serializeComposite,
    toExpressionDisplayClause,
    unescapeCompositeTerm,
    validateParentheses
} from '@/components/common/keywordExpression.ts';

describe('keyword expression boundary behavior', () => {
    test('applies public defaults and keeps blank term updates referentially stable', () => {
        const clause = createRuleClause();

        expect(clause).toMatchObject({
            id: expect.stringMatching(/^rule-clause-/),
            joiner: 'AND',
            operator: 'OR',
            terms: [],
            openParens: 0,
            closeParens: 0
        });
        expect(addTermToClause(clause, '   ')).toBe(clause);
        expect(insertClauseAfter([clause], 'missing', createRuleClause({ id: 'tail' })))
            .toHaveLength(2);
    });

    test('normalizes invalid joiners, paren counts and empty display labels', () => {
        const clause = createRuleClause({
            id: 'invalid-defaults',
            joiner: 'invalid' as never,
            openParens: Number.NaN,
            closeParens: -3,
            terms: [' ', 'coffee', 'coffee']
        });

        expect(clause).toMatchObject({
            joiner: 'AND',
            openParens: 0,
            closeParens: 0,
            terms: ['coffee']
        });
        expect(toExpressionDisplayClause(createRuleClause({ id: 'empty' }))).toMatchObject({
            connector: '+',
            terms: ['Empty']
        });
        expect(toExpressionDisplayClause(createRuleClause({ id: 'empty-custom' }), {
            isFirstClause: true,
            emptyLabel: 'No terms'
        })).toMatchObject({ connector: '', terms: ['No terms'] });
    });

    test('covers default parsing, empty clauses and unsupported syntax diagnostics', () => {
        expect(parseExpression('')).toEqual({ clauses: [], sourceFormat: 'empty' });
        expect(parseExpression('OR={}')).toMatchObject({
            sourceFormat: 'composite',
            clauses: [expect.objectContaining({ terms: [] })]
        });
        expect(parseExpression('OR={coffee} trailing')).toEqual({
            clauses: [],
            sourceFormat: 'raw',
            rawExpression: 'OR={coffee} trailing',
            errorKey: RULE_EXPRESSION_UNPARSEABLE_KEY
        });
        expect(parseExpression('(OR={coffee}')).toMatchObject({
            sourceFormat: 'raw',
            errorKey: RULE_EXPRESSION_UNPARSEABLE_KEY
        });
        expect(parseExpression('OR={coffee})')).toMatchObject({
            sourceFormat: 'raw',
            errorKey: RULE_EXPRESSION_UNPARSEABLE_KEY
        });
    });

    test('repairs nested clauses but preserves nested terms that are not composite expressions', () => {
        const repaired = parseExpression('OR={\\(AND=\\{coffee\\}+NOT=\\{refund\\}\\)}');
        expect(repaired.sourceFormat).toBe('composite');
        expect(repaired.clauses.map(clause => clause.operator)).toEqual(['AND', 'NOT']);

        const notRepairable = parseExpression('OR={prefix=\\{coffee\\}}');
        expect(notRepairable).toMatchObject({
            sourceFormat: 'composite',
            clauses: [expect.objectContaining({ terms: ['prefix={coffee}'] })]
        });

        const failedNestedRepair = parseExpression('OR={bad\\+connector}');
        expect(failedNestedRepair).toMatchObject({
            sourceFormat: 'composite',
            clauses: [expect.objectContaining({ terms: ['bad+connector'] })]
        });
    });

    test('stops recursive repair at depth two and normalizes unknown operators', () => {
        const deepest = 'OR={coffee}+OR={tea}';
        const middle = `OR={${escapeCompositeTerm(`(${deepest})`)}}`;
        const outer = `OR={${escapeCompositeTerm(`(${middle})`)}}`;

        const parsed = parseExpression(outer);
        expect(parsed.sourceFormat).toBe('composite');
        expect(parsed.clauses.length).toBeGreaterThan(0);
        expect(parseExpression('BOGUS={coffee}')).toMatchObject({
            sourceFormat: 'composite',
            clauses: [expect.objectContaining({ operator: 'OR', terms: ['coffee'] })]
        });
        expect(parseExpression('OR={,coffee}')).toMatchObject({
            sourceFormat: 'composite',
            clauses: [expect.objectContaining({ terms: ['coffee'] })]
        });
    });

    test('serializes empty intermediates and reports both directions of unbalanced parentheses', () => {
        const emptyExpressionStart = createRuleClause({
            id: 'empty-or',
            joiner: 'OR',
            negated: true,
            terms: []
        });
        const value = createRuleClause({
            id: 'value',
            terms: ['coffee']
        });

        expect(serializeComposite([emptyExpressionStart, value])).toEqual({ expression: 'OR={coffee}' });
        expect(validateParentheses([
            createRuleClause({ id: 'close-first', terms: ['x'], closeParens: 1 })
        ])).toEqual({ valid: false, errorKey: RULE_EXPRESSION_UNBALANCED_KEY });
        expect(validateParentheses([
            createRuleClause({ id: 'open-only', terms: ['x'], openParens: 1 })
        ])).toEqual({ valid: false, errorKey: RULE_EXPRESSION_UNBALANCED_KEY });
    });

    test('escapes every composite delimiter and preserves ordinary characters', () => {
        const raw = String.raw`a\b,c+d{e}|f/g×h(i)`;
        const escaped = escapeCompositeTerm(raw);

        expect(escaped).toContain('\\,');
        expect(escaped).toContain('\\+');
        expect(escaped).toContain('\\(');
        expect(unescapeCompositeTerm(escaped)).toBe(raw);
        expect(unescapeCompositeTerm('plain')).toBe('plain');
    });

    test('classifies REGEX and non-special display operators', () => {
        expect(getRuleExpressionDisplayOperatorColor({ negated: false, operator: 'REGEX' })).toBe('info');
        expect(getRuleExpressionDisplayOperatorColor({ negated: false, operator: 'AND' })).toBe('primary');
    });
});
