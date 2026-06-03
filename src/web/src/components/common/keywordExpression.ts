export type RuleOperator = 'OR' | 'AND' | 'NOT' | 'REGEX';
export type RuleJoiner = 'AND' | 'OR';
export type ExpressionFormat = 'composite';

export interface RuleClause {
    id: string;
    joiner: RuleJoiner;
    negated: boolean;
    operator: RuleOperator;
    terms: string[];
    openParens: number;
    closeParens: number;
    startsExpression: boolean;
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

export interface RuleExpressionDisplayClause {
    label: string;
    connector?: '' | '+' | '/' | '×';
    connectorLabel?: string;
    operator: RuleOperator | 'RAW';
    operatorLabel?: string;
    negated: boolean;
    openParens?: number;
    closeParens?: number;
    openParenLabel?: string;
    closeParenLabel?: string;
    terms: string[];
}

export interface ToExpressionDisplayClauseOptions {
    isFirstClause?: boolean;
    emptyLabel?: string;
}

interface CreateRuleClauseInput {
    id?: string;
    joiner?: RuleJoiner;
    negated?: boolean;
    operator?: RuleOperator;
    terms?: readonly string[];
    openParens?: number;
    closeParens?: number;
    startsExpression?: boolean;
}

const ESCAPABLE_COMPOSITE_CHARS = new Set(['\\', ',', '+', '{', '}', '|', '/', '×', '(', ')']);
const VALID_OPERATORS = new Set(['OR', 'AND', 'NOT', 'REGEX']);
const VALID_JOINERS = new Set(['AND', 'OR']);

let fallbackClauseId = 0;

export const RULE_EXPRESSION_UNBALANCED_KEY = 'Rule expression parentheses are not balanced';
export const RULE_EXPRESSION_UNPARSEABLE_KEY = 'Rule expression uses unsupported syntax and is preserved as raw text';

export function createRuleClause(input: CreateRuleClauseInput = {}, idFactory: () => string = createRuleClauseId): RuleClause {
    return {
        id: input.id ?? idFactory(),
        joiner: normalizeJoiner(input.joiner),
        negated: Boolean(input.negated),
        operator: input.operator ?? 'OR',
        terms: normalizeRuleTerms(input.terms ?? []),
        openParens: normalizeParenCount(input.openParens),
        closeParens: normalizeParenCount(input.closeParens),
        startsExpression: Boolean(input.startsExpression)
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

export function serializeForFormat(clauses: readonly RuleClause[], _format: ExpressionFormat): SerializeRuleExpressionResult {
    return serializeComposite(clauses);
}

export function toExpressionDisplayClause(
    clause: RuleClause,
    options: ToExpressionDisplayClauseOptions = {}
): RuleExpressionDisplayClause {
    const normalizedClause = createRuleClause(clause);
    const connector = options.isFirstClause
        ? ''
        : normalizedClause.negated
            ? '×'
            : normalizedClause.joiner === 'OR'
                ? '/'
                : '+';

    return {
        label: normalizedClause.operator,
        connector,
        connectorLabel: connector,
        operator: normalizedClause.operator,
        operatorLabel: normalizedClause.operator,
        negated: normalizedClause.negated,
        openParens: normalizedClause.openParens,
        closeParens: normalizedClause.closeParens,
        openParenLabel: '('.repeat(normalizedClause.openParens),
        closeParenLabel: ')'.repeat(normalizedClause.closeParens),
        terms: normalizedClause.terms.length > 0
            ? normalizedClause.terms
            : [options.emptyLabel ?? 'Empty'],
    };
}

export function getRuleExpressionDisplayOperatorColor(
    clause: Pick<RuleExpressionDisplayClause, 'negated' | 'operator'>
): 'warning' | 'info' | 'primary' {
    if (clause.negated || clause.operator === 'NOT') {
        return 'warning';
    }
    if (clause.operator === 'REGEX') {
        return 'info';
    }
    return 'primary';
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
                const joiner = getSerializedJoiner(clause, index);
                const open = '('.repeat(clause.openParens);
                const close = ')'.repeat(clause.closeParens);
                const terms = clause.terms.map(escapeCompositeTerm).join(',');
                return `${joiner}${open}${clause.operator}={${terms}}${close}`;
            })
            .join('')
    };
}

function getSerializedJoiner(clause: RuleClause, index: number): string {
    if (index === 0) {
        return '';
    }
    if (clause.startsExpression) {
        return '|';
    }
    if (clause.negated) {
        return '×';
    }
    return normalizeJoiner(clause.joiner) === 'OR' ? '/' : '+';
}

function normalizeSerializableClauses(clauses: readonly RuleClause[]): RuleClause[] {
    const serializableClauses: RuleClause[] = [];
    let pendingJoiner: RuleJoiner = 'AND';
    let pendingExpressionStart = false;
    let pendingNegated = false;

    for (const clause of clauses.map(item => createRuleClause(item))) {
        if (clause.terms.length === 0) {
            if (clause.joiner === 'OR') {
                pendingJoiner = 'OR';
                pendingExpressionStart = true;
            }
            if (clause.negated) {
                pendingNegated = true;
            }
            continue;
        }

        const startsExpression = serializableClauses.length > 0
            && (pendingExpressionStart || clause.startsExpression);
        const joiner = serializableClauses.length === 0
            ? 'AND'
            : startsExpression || pendingJoiner === 'OR' || clause.joiner === 'OR'
                ? 'OR'
                : 'AND';

        serializableClauses.push({
            ...clause,
            joiner,
            negated: serializableClauses.length === 0 || startsExpression
                ? false
                : pendingNegated || clause.negated,
            startsExpression
        });
        pendingJoiner = 'AND';
        pendingExpressionStart = false;
        pendingNegated = false;
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

function parseCompositeExpression(expression: string, idFactory: () => string, repairDepth = 0): RuleClause[] | null {
    const clauses: RuleClause[] = [];
    let index = 0;
    let pendingOpenParens = 0;
    let expectingClause = true;
    let isFirstClause = true;
    let pendingJoiner: RuleJoiner = 'AND';
    let pendingNegated = false;
    let pendingExpressionStart = false;
    let depth = 0;

    while (index < expression.length) {
        index = skipSpaces(expression, index);
        if (index >= expression.length) {
            break;
        }

        const char = expression[index];
        if (char === '(') {
            pendingOpenParens += 1;
            depth += 1;
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
            depth = Math.max(0, depth - 1);
            index += 1;
            index = skipSpaces(expression, index);
        }

        const connectorNegated = !isFirstClause && pendingNegated;
        const operator = connectorNegated && parsedClause.operator === 'NOT'
            ? 'OR'
            : parsedClause.operator;

        clauses.push(createRuleClause({
            joiner: isFirstClause ? 'AND' : pendingJoiner,
            negated: connectorNegated,
            operator,
            terms: parsedClause.terms,
            openParens: pendingOpenParens,
            closeParens,
            startsExpression: !isFirstClause && pendingExpressionStart
        }, idFactory));
        isFirstClause = false;
        pendingOpenParens = 0;
        pendingNegated = false;
        pendingExpressionStart = false;

        if (index >= expression.length) {
            break;
        }

        const connector = readCompositeConnector(expression, index, depth);
        if (!connector) {
            return null;
        }
        pendingJoiner = connector.joiner;
        pendingNegated = connector.negated;
        pendingExpressionStart = connector.startsExpression;
        index = connector.nextIndex;
        expectingClause = true;
    }

    if (pendingOpenParens > 0) {
        return null;
    }

    const validation = validateParentheses(clauses);
    if (!validation.valid) {
        return null;
    }

    if (repairDepth >= 2) {
        return clauses.length > 0 ? clauses : null;
    }

    const repairedClauses = repairNestedCompositeTerms(clauses, idFactory, repairDepth);
    if (repairedClauses !== clauses) {
        const repairedValidation = validateParentheses(repairedClauses);
        if (!repairedValidation.valid) {
            return null;
        }
        return repairedClauses.length > 0 ? repairedClauses : null;
    }

    return clauses.length > 0 ? clauses : null;
}

function readCompositeConnector(
    expression: string,
    startIndex: number,
    depth: number
): { joiner: RuleJoiner; negated: boolean; startsExpression: boolean; nextIndex: number } | null {
    let index = skipSpaces(expression, startIndex);
    const char = expression[index];

    if (char === '+') {
        return { joiner: 'AND', negated: false, startsExpression: false, nextIndex: skipSpaces(expression, index + 1) };
    }
    if (char === '|') {
        return { joiner: 'OR', negated: false, startsExpression: depth === 0, nextIndex: skipSpaces(expression, index + 1) };
    }
    if (char === '/') {
        return { joiner: 'OR', negated: false, startsExpression: false, nextIndex: skipSpaces(expression, index + 1) };
    }
    if (char === '×') {
        return { joiner: 'AND', negated: true, startsExpression: false, nextIndex: skipSpaces(expression, index + 1) };
    }

    const remainingExpression = expression.slice(index);
    const notMatch = /^NOT\b/i.exec(remainingExpression);
    if (notMatch) {
        index += notMatch[0].length;
        return { joiner: 'AND', negated: true, startsExpression: false, nextIndex: skipSpaces(expression, index) };
    }

    return null;
}

function repairNestedCompositeTerms(
    clauses: readonly RuleClause[],
    idFactory: () => string,
    repairDepth: number
): RuleClause[] {
    let changed = false;
    const repairedClauses: RuleClause[] = [];

    for (const clause of clauses) {
        const nestedExpression = getRepairableNestedCompositeExpression(clause);
        if (!nestedExpression) {
            repairedClauses.push(clause);
            continue;
        }

        const nestedClauses = parseCompositeExpression(nestedExpression, idFactory, repairDepth + 1);
        if (!nestedClauses || nestedClauses.length === 0) {
            repairedClauses.push(clause);
            continue;
        }

        changed = true;
        const lastNestedClause = nestedClauses[nestedClauses.length - 1]!;
        repairedClauses.push(...nestedClauses.map((nestedClause, index) => ({
            ...nestedClause,
            joiner: index === 0 ? clause.joiner : nestedClause.joiner,
            startsExpression: index === 0 ? clause.startsExpression : nestedClause.startsExpression,
            openParens: index === 0
                ? nestedClause.openParens + clause.openParens
                : nestedClause.openParens,
            closeParens: nestedClause.id === lastNestedClause.id
                ? nestedClause.closeParens + clause.closeParens
                : nestedClause.closeParens,
        })));
    }

    return changed ? repairedClauses : clauses as RuleClause[];
}

function getRepairableNestedCompositeExpression(clause: RuleClause): string | null {
    if (clause.operator !== 'OR') {
        return null;
    }

    if (clause.terms.length !== 1) {
        return null;
    }

    const term = clause.terms[0]?.trim() ?? '';
    if (!term.includes('={')) {
        return null;
    }

    const isWrappedExpression = term.startsWith('(') && term.endsWith(')');
    const containsCompositeConnector = /[+|/×]/.test(term) || /\bNOT\b/i.test(term);
    if (!isWrappedExpression && !containsCompositeConnector) {
        return null;
    }

    return term;
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
