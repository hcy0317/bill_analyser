import {
    parseExpression,
    toExpressionDisplayClause,
} from '@/components/common/keywordExpression.ts';
import type { RuleExpressionDisplayClause } from '@/components/common/keywordExpression.ts';

export interface RuleExpressionDisplayGroup {
    clauses: RuleExpressionDisplayClause[];
}

interface RuleExpressionDisplayLabels {
    empty: string;
    raw: string;
}

/**
 * 将规则表达式解析结果转换为 UI 可展示的 clause 分组，无法解析时保留 RAW fallback。
 */
export function buildRuleExpressionDisplayGroups(
    expressionValue: string | null | undefined,
    labels: RuleExpressionDisplayLabels
): RuleExpressionDisplayGroup[] {
    const expression = String(expressionValue || '').trim();
    const parsedExpression = parseExpression(expression, { format: 'composite' });

    if (parsedExpression.clauses.length < 1) {
        return [{
            clauses: [{
                label: parsedExpression.sourceFormat === 'empty' ? labels.empty : labels.raw,
                operator: 'RAW',
                negated: false,
                terms: [parsedExpression.rawExpression || expression || labels.empty],
            }],
        }];
    }

    const groups: RuleExpressionDisplayGroup[] = [];
    let currentClauses: RuleExpressionDisplayClause[] = [];

    parsedExpression.clauses.forEach((clause, index) => {
        if (
            index > 0
            && clause.startsExpression
            && currentClauses.length > 0
        ) {
            groups.push({ clauses: currentClauses });
            currentClauses = [];
        }

        currentClauses.push(toExpressionDisplayClause(clause, {
            isFirstClause: currentClauses.length === 0,
            emptyLabel: labels.empty,
        }));
    });

    if (currentClauses.length > 0) {
        groups.push({ clauses: currentClauses });
    }

    return groups;
}
