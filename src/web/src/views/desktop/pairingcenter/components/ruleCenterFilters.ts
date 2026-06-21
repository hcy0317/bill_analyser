import { CategoryType } from '@/core/category.ts';

export type RuleBooleanFilter = 'all' | 'yes' | 'no';

export interface RuleFilterOption<T extends string> {
    title: string;
    value: T;
}

export interface PrimaryCategoryFilterOption extends RuleFilterOption<string> {
    icon: string;
    color: string;
}

export interface PrimaryCategoryFilterGroup {
    type: CategoryType;
    title: string;
    options: PrimaryCategoryFilterOption[];
}

export interface PrimaryCategoryDisplayInfo {
    id?: string;
    name: string;
    icon: string;
    color: string;
}

interface ParsedRuleExpressionFilter {
    includeGroups: string[][];
    excludeGroups: string[][];
}

export const primaryCategoryFilterTypeOrder: CategoryType[] = [
    CategoryType.Income,
    CategoryType.Expense,
    CategoryType.Transfer,
    CategoryType.Investment,
];

function normalizeRuleFilterQuery(value: string): string {
    return value
        .replace(/！/g, '!')
        .replace(/（/g, '(')
        .replace(/）/g, ')')
        .replace(/｜/g, '|')
        .trim();
}

function splitRuleFilterAlternatives(value: string): string[] {
    return value
        .split('|')
        .map(item => item.trim())
        .filter(item => item.length > 0);
}

/**
 * 解析规则表达式筛选框，支持 include、! / NOT exclude 和 `|` 多候选写法。
 */
export function parseRuleExpressionFilterQuery(query: string): ParsedRuleExpressionFilter {
    const normalizedQuery = normalizeRuleFilterQuery(query);
    const includeGroups: string[][] = [];
    const excludeGroups: string[][] = [];
    let index = 0;

    while (index < normalizedQuery.length) {
        while (index < normalizedQuery.length && /\s/.test(normalizedQuery[index] ?? '')) {
            index += 1;
        }

        if (index >= normalizedQuery.length) {
            break;
        }

        let exclude = false;
        if (normalizedQuery[index] === '!') {
            exclude = true;
            index += 1;
        } else {
            const notMatch = normalizedQuery.slice(index).match(/^NOT(?=\s|=|\(|!|$)/i);
            if (notMatch) {
                exclude = true;
                index += notMatch[0].length;
            }
        }

        while (index < normalizedQuery.length && /[\s=]/.test(normalizedQuery[index] ?? '')) {
            index += 1;
        }

        let token = '';
        if (normalizedQuery[index] === '(') {
            index += 1;
            const tokenStart = index;
            while (index < normalizedQuery.length && normalizedQuery[index] !== ')') {
                index += 1;
            }
            token = normalizedQuery.slice(tokenStart, index);
            if (normalizedQuery[index] === ')') {
                index += 1;
            }
        } else {
            const tokenStart = index;
            while (
                index < normalizedQuery.length
                && !/\s/.test(normalizedQuery[index] ?? '')
                && normalizedQuery[index] !== '!'
                && normalizedQuery[index] !== '('
                && normalizedQuery[index] !== ')'
            ) {
                index += 1;
            }
            token = normalizedQuery.slice(tokenStart, index);
        }

        const alternatives = splitRuleFilterAlternatives(token);
        if (alternatives.length < 1) {
            continue;
        }

        if (exclude) {
            excludeGroups.push(alternatives);
        } else {
            includeGroups.push(alternatives);
        }
    }

    return { includeGroups, excludeGroups };
}

function matchesRuleFilterTerm(text: string, term: string, useRegex: boolean): boolean {
    if (useRegex) {
        try {
            return new RegExp(term, 'i').test(text);
        } catch {
            return false;
        }
    }

    return text.toLocaleLowerCase().includes(term.toLocaleLowerCase());
}

function matchesAnyRuleFilterTerm(text: string, terms: string[], useRegex: boolean): boolean {
    return terms.some(term => matchesRuleFilterTerm(text, term, useRegex));
}

/**
 * 对规则表达式应用已解析筛选条件；include 需要全部命中，exclude 任一命中即排除。
 */
export function matchesRuleExpressionFilterText(
    expression: string,
    parsedFilter: ParsedRuleExpressionFilter,
    useRegex: boolean
): boolean {
    if (parsedFilter.includeGroups.length < 1 && parsedFilter.excludeGroups.length < 1) {
        return true;
    }

    const searchableExpression = String(expression || '');
    return parsedFilter.includeGroups.every(terms => (
        matchesAnyRuleFilterTerm(searchableExpression, terms, useRegex)
    )) && !parsedFilter.excludeGroups.some(terms => (
        matchesAnyRuleFilterTerm(searchableExpression, terms, useRegex)
    ));
}
