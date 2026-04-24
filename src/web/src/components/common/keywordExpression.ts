export type RuleOperator = 'OR' | 'AND' | 'NOT' | 'REGEX';
export type RuleJoiner = 'AND' | 'OR';
export type ExpressionFormat = 'legacy' | 'composite';

export interface RuleClause {
    id: string;
    joiner: RuleJoiner;
    operator: RuleOperator;
    terms: string[];
    openParens: number;
    closeParens: number;
}

export interface ParseRuleExpressionOptions {
    format?: ExpressionFormat;
    idFactory?: () => string;
}

export interface ParseRuleExpressionResult {
    clauses: RuleClause[];
    sourceFormat: ExpressionFormat | 'empty' | 'raw';
    rawExpression?: string;
    errorKey?: string;
}

export interface SerializeRuleExpressionResult {
    expression: string;
    errorKey?: string;
}

export interface RuleExpressionValidationResult {
    valid: boolean;
    errorKey?: string;
}

interface CreateRuleClauseInput {
    id?: string;
    joiner?: RuleJoiner;
    operator?: RuleOperator;
    terms?: readonly string[];
    openParens?: number;
    closeParens?: number;
}

const ESCAPABLE_COMPOSITE_CHARS = new Set(['\\', ',', '+', '{', '}', '|', '(', ')']);
const VALID_OPERATORS = new Set(['OR', 'AND', 'NOT', 'REGEX']);
const VALID_JOINERS = new Set(['AND', 'OR']);

let fallbackClauseId = 0;

export const RULE_EXPRESSION_UNBALANCED_KEY = 'Rule expression parentheses are not balanced';
export const RULE_EXPRESSION_UNPARSEABLE_KEY = 'Rule expression uses unsupported syntax and is preserved as raw text';

export function createRuleClause(input: CreateRuleClauseInput = {}, idFactory: () => string = createRuleClauseId): RuleClause {
    return {
        id: input.id ?? idFactory(),
        joiner: normalizeJoiner(input.joiner),
        operator: input.operator ?? 'OR',
        terms: normalizeRuleTerms(input.terms ?? []),
        openParens: normalizeParenCount(input.openParens),
        closeParens: normalizeParenCount(input.closeParens)
    };
}

export function createRuleClauseId(): string {
    fallbackClauseId += 1;
    return `rule-clause-${fallbackClauseId}`;
}

export function normalizeRuleTerms(terms: readonly string[]): string[] {
    const seen = new Set<string>();
    const normalizedTerms: string[] = [];
    for (const rawTerm of terms) {
        const term = String(rawTerm).trim();
        if (!term || seen.has(term)) {
            continue;
        }
        seen.add(term);
        normalizedTerms.push(term);
    }
    return normalizedTerms;
}

export function addTermToClause(clause: RuleClause, rawTerm: string): RuleClause {
    const term = rawTerm.trim();
    if (!term) {
        return clause;
    }

    return {
        ...clause,
        terms: [...clause.terms, term]
    };
}

export function removeTermFromClause(clause: RuleClause, termIndex: number): RuleClause {
    return {
        ...clause,
        terms: clause.terms.filter((_, index) => index !== termIndex)
    };
}

export function insertClauseAfter(clauses: readonly RuleClause[], targetId: string, newClause: RuleClause): RuleClause[] {
    const targetIndex = clauses.findIndex(clause => clause.id === targetId);
    if (targetIndex === -1) {
        return [...clauses, newClause];
    }

    return [
        ...clauses.slice(0, targetIndex + 1),
        newClause,
        ...clauses.slice(targetIndex + 1)
    ];
}

export function parseExpression(expression: string, options: ParseRuleExpressionOptions = {}): ParseRuleExpressionResult {
    const trimmed = expression.trim();
    const idFactory = options.idFactory ?? createRuleClauseId;
    if (!trimmed) {
        return { clauses: [], sourceFormat: 'empty' };
    }

    const detectedFormat = detectExpressionFormat(trimmed);
    if (detectedFormat === 'composite') {
        const parsedComposite = parseCompositeExpression(trimmed, idFactory);
        if (parsedComposite) {
            return { clauses: parsedComposite, sourceFormat: 'composite' };
        }
        return {
            clauses: [],
            sourceFormat: 'raw',
            rawExpression: expression,
            errorKey: RULE_EXPRESSION_UNPARSEABLE_KEY
        };
    }

    const parsedLegacy = parseLegacyExpression(trimmed, idFactory);
    if (parsedLegacy.length > 0) {
        return { clauses: parsedLegacy, sourceFormat: 'legacy' };
    }

    if (options.format === 'composite') {
        const parsedComposite = parseCompositeExpression(trimmed, idFactory);
        if (parsedComposite) {
            return { clauses: parsedComposite, sourceFormat: 'composite' };
        }
    }

    return {
        clauses: [],
        sourceFormat: 'raw',
        rawExpression: expression,
        errorKey: RULE_EXPRESSION_UNPARSEABLE_KEY
    };
}

export function serializeForFormat(clauses: readonly RuleClause[], format: ExpressionFormat): SerializeRuleExpressionResult {
    if (format === 'legacy') {
        return { expression: serializeLegacy(clauses) };
    }

    return serializeComposite(clauses);
}

export function serializeLegacy(clauses: readonly RuleClause[]): string {
    return clauses
        .map(clause => createRuleClause(clause))
        .filter(clause => clause.terms.length > 0)
        .map(clause => {
            const prefix = clause.operator === 'REGEX' ? 'REGEX' : clause.operator;
            const separator = clause.operator === 'REGEX' ? '' : '|';
            return `${prefix}:${clause.terms.join(separator)}`;
        })
        .join('&');
}

export function serializeComposite(clauses: readonly RuleClause[]): SerializeRuleExpressionResult {
    const serializableClauses = normalizeSerializableClauses(clauses);
    const validation = validateParentheses(serializableClauses);
    if (!validation.valid) {
        return { expression: '', errorKey: validation.errorKey };
    }

    return {
        expression: serializableClauses
            .map((clause, index) => {
                const joiner = index === 0 ? '' : normalizeJoiner(clause.joiner) === 'OR' ? '|' : '+';
                const open = '('.repeat(clause.openParens);
                const close = ')'.repeat(clause.closeParens);
                const terms = clause.terms.map(escapeCompositeTerm).join(',');
                return `${joiner}${open}${clause.operator}={${terms}}${close}`;
            })
            .join('')
    };
}

function normalizeSerializableClauses(clauses: readonly RuleClause[]): RuleClause[] {
    const serializableClauses: RuleClause[] = [];
    let pendingJoiner: RuleJoiner = 'AND';

    for (const clause of clauses.map(item => createRuleClause(item))) {
        if (clause.terms.length === 0) {
            if (clause.joiner === 'OR') {
                pendingJoiner = 'OR';
            }
            continue;
        }

        serializableClauses.push({
            ...clause,
            joiner: serializableClauses.length === 0
                ? 'AND'
                : pendingJoiner === 'OR' || clause.joiner === 'OR'
                    ? 'OR'
                    : 'AND'
        });
        pendingJoiner = 'AND';
    }

    return serializableClauses;
}

export function validateParentheses(clauses: readonly RuleClause[]): RuleExpressionValidationResult {
    let balance = 0;
    for (const clause of clauses) {
        balance += normalizeParenCount(clause.openParens);
        balance -= normalizeParenCount(clause.closeParens);
        if (balance < 0) {
            return { valid: false, errorKey: RULE_EXPRESSION_UNBALANCED_KEY };
        }
    }

    if (balance !== 0) {
        return { valid: false, errorKey: RULE_EXPRESSION_UNBALANCED_KEY };
    }

    return { valid: true };
}

export function escapeCompositeTerm(term: string): string {
    return [...term].map(char => ESCAPABLE_COMPOSITE_CHARS.has(char) ? `\\${char}` : char).join('');
}

export function unescapeCompositeTerm(term: string): string {
    const chars: string[] = [];
    for (let index = 0; index < term.length; index += 1) {
        const char = term[index];
        const nextChar = term[index + 1];
        if (char === '\\' && nextChar && ESCAPABLE_COMPOSITE_CHARS.has(nextChar)) {
            chars.push(nextChar);
            index += 1;
            continue;
        }
        if (char) {
            chars.push(char);
        }
    }
    return chars.join('');
}

function detectExpressionFormat(expression: string): ExpressionFormat {
    if (expression.includes('={')) {
        return 'composite';
    }
    return 'legacy';
}

function parseLegacyExpression(expression: string, idFactory: () => string): RuleClause[] {
    const clauses: RuleClause[] = [];
    for (const part of expression.split('&')) {
        const trimmedPart = part.trim();
        if (!trimmedPart) {
            continue;
        }

        const prefixMatch = /^(OR|AND|NOT|REGEX):/i.exec(trimmedPart);
        const operator = normalizeOperator(prefixMatch?.[1] ?? 'OR');
        const content = prefixMatch ? trimmedPart.slice(prefixMatch[0].length) : trimmedPart;
        const terms = operator === 'REGEX'
            ? normalizeRuleTerms([content])
            : normalizeRuleTerms(content.split('|'));
        if (terms.length > 0) {
            clauses.push(createRuleClause({ operator, terms }, idFactory));
        }
    }
    return clauses;
}

function parseCompositeExpression(expression: string, idFactory: () => string): RuleClause[] | null {
    const clauses: RuleClause[] = [];
    let index = 0;
    let pendingOpenParens = 0;
    let expectingClause = true;
    let isFirstClause = true;
    let pendingJoiner: RuleJoiner = 'AND';

    while (index < expression.length) {
        index = skipSpaces(expression, index);
        if (index >= expression.length) {
            break;
        }

        const char = expression[index];
        if (char === '(') {
            pendingOpenParens += 1;
            index += 1;
            expectingClause = true;
            continue;
        }

        if (!expectingClause) {
            return null;
        }

        const parsedClause = readCompositeClause(expression, index);
        if (!parsedClause) {
            return null;
        }

        index = skipSpaces(expression, parsedClause.nextIndex);
        let closeParens = 0;
        while (expression[index] === ')') {
            closeParens += 1;
            index += 1;
            index = skipSpaces(expression, index);
        }

        clauses.push(createRuleClause({
            joiner: isFirstClause ? 'AND' : pendingJoiner,
            operator: parsedClause.operator,
            terms: parsedClause.terms,
            openParens: pendingOpenParens,
            closeParens
        }, idFactory));
        isFirstClause = false;
        pendingOpenParens = 0;

        if (index >= expression.length) {
            break;
        }

        if (expression[index] !== '+' && expression[index] !== '|') {
            return null;
        }
        pendingJoiner = expression[index] === '|' ? 'OR' : 'AND';
        index += 1;
        expectingClause = true;
    }

    if (pendingOpenParens > 0) {
        return null;
    }

    const validation = validateParentheses(clauses);
    if (!validation.valid) {
        return null;
    }

    return clauses.length > 0 ? clauses : null;
}

function readCompositeClause(expression: string, startIndex: number): { operator: RuleOperator; terms: string[]; nextIndex: number } | null {
    const eqIndex = expression.indexOf('={', startIndex);
    if (eqIndex === -1) {
        return null;
    }

    const rawPrefix = expression.slice(startIndex, eqIndex).trim().toUpperCase();
    const operator = normalizeOperator(rawPrefix);
    let index = eqIndex + 2;
    const content: string[] = [];

    while (index < expression.length) {
        const char = expression[index];
        const nextChar = expression[index + 1];
        if (char === '\\' && nextChar && ESCAPABLE_COMPOSITE_CHARS.has(nextChar)) {
            content.push(char, nextChar);
            index += 2;
            continue;
        }
        if (char === '}') {
            const terms = splitCompositeTerms(content.join(''));
            return { operator, terms, nextIndex: index + 1 };
        }
        if (char) {
            content.push(char);
        }
        index += 1;
    }

    return null;
}

function splitCompositeTerms(content: string): string[] {
    const terms: string[] = [];
    const current: string[] = [];
    for (let index = 0; index < content.length; index += 1) {
        const char = content[index];
        const nextChar = content[index + 1];
        if (char === '\\' && nextChar && ESCAPABLE_COMPOSITE_CHARS.has(nextChar)) {
            current.push(char, nextChar);
            index += 1;
            continue;
        }
        if (char === ',') {
            const term = unescapeCompositeTerm(current.join('').trim()).trim();
            if (term) {
                terms.push(term);
            }
            current.length = 0;
            continue;
        }
        if (char) {
            current.push(char);
        }
    }

    const finalTerm = unescapeCompositeTerm(current.join('').trim()).trim();
    if (finalTerm) {
        terms.push(finalTerm);
    }
    return terms;
}

function normalizeOperator(rawOperator: string): RuleOperator {
    const operator = rawOperator.trim().toUpperCase();
    if (VALID_OPERATORS.has(operator)) {
        return operator as RuleOperator;
    }
    return 'OR';
}

function normalizeJoiner(rawJoiner: string | undefined): RuleJoiner {
    const joiner = String(rawJoiner ?? 'AND').trim().toUpperCase();
    if (VALID_JOINERS.has(joiner)) {
        return joiner as RuleJoiner;
    }
    return 'AND';
}

function normalizeParenCount(count: number | undefined): number {
    if (!Number.isFinite(count)) {
        return 0;
    }
    return Math.max(0, Math.trunc(count ?? 0));
}

function skipSpaces(expression: string, index: number): number {
    let currentIndex = index;
    while (currentIndex < expression.length && /\s/.test(expression[currentIndex] ?? '')) {
        currentIndex += 1;
    }
    return currentIndex;
}
