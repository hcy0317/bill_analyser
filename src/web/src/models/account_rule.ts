export type AccountRuleRoleScope =
    'any'
    | 'source'
    | 'destination'
    | 'investment'
    | 'payment_method_source';

export type AccountRuleTransactionTypeScope =
    'all'
    | 'income'
    | 'expense'
    | 'transfer'
    | 'investment';

export type AccountRuleFieldScope =
    'parser'
    | 'counterparty'
    | 'payment_method'
    | 'description'
    | 'expense_counterparty'
    | 'expense_payment_method'
    | 'expense_description'
    | 'income_counterparty'
    | 'income_payment_method'
    | 'income_description'
    | 'investment_counterparty'
    | 'investment_description';

export interface AccountRuleItem {
    readonly id: number;
    readonly account_id: number;
    readonly accountId: number;
    readonly accountName: string;
    readonly accountType?: number | null;
    readonly accountHidden?: boolean;
    readonly name: string;
    readonly priority: number;
    readonly rule_expression: string;
    readonly ruleExpression: string;
    readonly regex_enabled: boolean;
    readonly regexEnabled: boolean;
    readonly enabled: boolean;
    readonly applied_count: number;
    readonly appliedCount: number;
    readonly match_count: number;
    readonly matchCount: number;
    readonly account_role_scope: AccountRuleRoleScope;
    readonly accountRoleScope: AccountRuleRoleScope;
    readonly transaction_type_scope: AccountRuleTransactionTypeScope;
    readonly transactionTypeScope: AccountRuleTransactionTypeScope;
    readonly field_scope: AccountRuleFieldScope[];
    readonly fieldScope: AccountRuleFieldScope[];
    readonly source?: string | null;
    readonly source_key?: string | null;
    readonly sourceKey?: string | null;
}

export interface AccountRuleForm {
    accountId: string;
    name: string;
    priority: number;
    ruleExpression: string;
    regexEnabled: boolean;
    enabled: boolean;
    accountRoleScope: AccountRuleRoleScope;
    transactionTypeScope: AccountRuleTransactionTypeScope;
    fieldScope: AccountRuleFieldScope[];
}

export interface AccountRulePayload {
    account_id: number;
    name: string;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
    account_role_scope: AccountRuleRoleScope;
    transaction_type_scope: AccountRuleTransactionTypeScope;
    field_scope: AccountRuleFieldScope[];
}

export interface AccountRuleTestContext {
    context: {
        text: string;
        counterparty: string;
        paymentMethod: string;
        description: string;
        parserId: string;
        parserLabel: string;
    };
    account_role_scope: AccountRuleRoleScope;
    transaction_type_scope: AccountRuleTransactionTypeScope;
}

export interface AccountRuleTestResult {
    matched?: boolean | null;
    accountId?: number | null;
    ruleId?: number | null;
    matchedFields?: string[] | null;
    fallbackUsed?: boolean | null;
}

export interface AccountAliasMigrationResult {
    migrated?: number | string | null;
    migrated_count?: number | string | null;
    skipped?: number | string | null;
    skipped_count?: number | string | null;
}

export interface AccountRuleOption<T extends string> {
    readonly titleKey: string;
    readonly value: T;
}

export const ACCOUNT_RULE_ROLE_SCOPE_OPTIONS: AccountRuleOption<AccountRuleRoleScope>[] = [
    { titleKey: 'Any Account Role', value: 'any' },
    { titleKey: 'Source Account', value: 'source' },
    { titleKey: 'Destination Account', value: 'destination' },
    { titleKey: 'Investment Account', value: 'investment' },
    { titleKey: 'Payment Method Source', value: 'payment_method_source' },
];

export const ACCOUNT_RULE_TRANSACTION_SCOPE_OPTIONS: AccountRuleOption<AccountRuleTransactionTypeScope>[] = [
    { titleKey: 'All Transaction Types', value: 'all' },
    { titleKey: 'Income', value: 'income' },
    { titleKey: 'Expense', value: 'expense' },
    { titleKey: 'Transfer', value: 'transfer' },
    { titleKey: 'Investment', value: 'investment' },
];

export const ACCOUNT_RULE_FIELD_SCOPE_OPTIONS: AccountRuleOption<AccountRuleFieldScope>[] = [
    { titleKey: 'Parser Signal', value: 'parser' },
    { titleKey: 'Counterparty', value: 'counterparty' },
    { titleKey: 'Payment Method', value: 'payment_method' },
    { titleKey: 'Description', value: 'description' },
    { titleKey: 'Expense Counterparty', value: 'expense_counterparty' },
    { titleKey: 'Expense Payment Method', value: 'expense_payment_method' },
    { titleKey: 'Expense Description', value: 'expense_description' },
    { titleKey: 'Income Counterparty', value: 'income_counterparty' },
    { titleKey: 'Income Payment Method', value: 'income_payment_method' },
    { titleKey: 'Income Description', value: 'income_description' },
    { titleKey: 'Investment Counterparty', value: 'investment_counterparty' },
    { titleKey: 'Investment Description', value: 'investment_description' },
];

export const DEFAULT_ACCOUNT_RULE_FIELD_SCOPE: AccountRuleFieldScope[] = [
    'counterparty',
    'payment_method',
    'description',
];

export function createDefaultAccountRuleForm(accountId?: number | string | null): AccountRuleForm {
    return {
        accountId: accountId ? String(accountId) : '',
        name: '',
        priority: 100,
        ruleExpression: '',
        regexEnabled: false,
        enabled: true,
        accountRoleScope: 'any',
        transactionTypeScope: 'all',
        fieldScope: [...DEFAULT_ACCOUNT_RULE_FIELD_SCOPE],
    };
}

export function normalizeAccountRuleItem(raw: Record<string, unknown>): AccountRuleItem {
    const accountId = toFiniteNumber(raw['accountId'] ?? raw['account_id'], 0);
    const ruleExpression = String(raw['ruleExpression'] ?? raw['rule_expression'] ?? '');
    const regexEnabled = toBoolean(raw['regexEnabled'] ?? raw['regex_enabled']);
    const appliedCount = toFiniteNumber(raw['appliedCount'] ?? raw['applied_count'], 0);
    const matchCount = toFiniteNumber(raw['matchCount'] ?? raw['match_count'], 0);
    const accountRoleScope = normalizeRoleScope(raw['accountRoleScope'] ?? raw['account_role_scope']);
    const transactionTypeScope = normalizeTransactionTypeScope(raw['transactionTypeScope'] ?? raw['transaction_type_scope']);
    const fieldScope = normalizeFieldScope(raw['fieldScope'] ?? raw['field_scope']);

    return {
        id: toFiniteNumber(raw['id'], 0),
        account_id: accountId,
        accountId,
        accountName: String(raw['accountName'] ?? raw['account_name'] ?? ''),
        accountType: raw['accountType'] === null || raw['accountType'] === undefined
            ? null
            : toFiniteNumber(raw['accountType'], 0),
        accountHidden: toBoolean(raw['accountHidden'] ?? raw['account_hidden']),
        name: String(raw['name'] ?? ''),
        priority: toFiniteNumber(raw['priority'], 100),
        rule_expression: ruleExpression,
        ruleExpression,
        regex_enabled: regexEnabled,
        regexEnabled,
        enabled: raw['enabled'] === undefined ? true : toBoolean(raw['enabled']),
        applied_count: appliedCount,
        appliedCount,
        match_count: matchCount,
        matchCount,
        account_role_scope: accountRoleScope,
        accountRoleScope,
        transaction_type_scope: transactionTypeScope,
        transactionTypeScope,
        field_scope: fieldScope,
        fieldScope,
        source: raw['source'] === undefined || raw['source'] === null ? null : String(raw['source']),
        source_key: raw['source_key'] === undefined || raw['source_key'] === null ? null : String(raw['source_key']),
        sourceKey: raw['sourceKey'] === undefined || raw['sourceKey'] === null ? null : String(raw['sourceKey']),
    };
}

export function accountRuleToForm(rule: AccountRuleItem): AccountRuleForm {
    return {
        accountId: String(rule.accountId),
        name: rule.name,
        priority: rule.priority,
        ruleExpression: rule.ruleExpression,
        regexEnabled: rule.regexEnabled,
        enabled: rule.enabled,
        accountRoleScope: rule.accountRoleScope,
        transactionTypeScope: rule.transactionTypeScope,
        fieldScope: [...rule.fieldScope],
    };
}

export function buildAccountRulePayload(form: AccountRuleForm, fallbackName: string): AccountRulePayload {
    const accountId = Number.parseInt(String(form.accountId || ''), 10);
    if (!Number.isFinite(accountId) || accountId <= 0) {
        throw new Error('Account is required');
    }

    const ruleExpression = String(form.ruleExpression || '').trim();
    if (!ruleExpression) {
        throw new Error('Expression is required');
    }

    const fieldScope = form.fieldScope.length > 0
        ? [...new Set(form.fieldScope)]
        : [...DEFAULT_ACCOUNT_RULE_FIELD_SCOPE];

    return {
        account_id: accountId,
        name: String(form.name || fallbackName).trim() || fallbackName,
        priority: Number.isFinite(Number(form.priority)) ? Number(form.priority) : 100,
        rule_expression: ruleExpression,
        regex_enabled: !!form.regexEnabled,
        enabled: !!form.enabled,
        account_role_scope: form.accountRoleScope,
        transaction_type_scope: form.transactionTypeScope,
        field_scope: fieldScope,
    };
}

export function buildAccountRuleTestContext(
    text: string,
    accountRoleScope: AccountRuleRoleScope,
    transactionTypeScope: AccountRuleTransactionTypeScope
): AccountRuleTestContext {
    const normalizedText = String(text || '').trim();
    return {
        context: {
            text: normalizedText,
            counterparty: normalizedText,
            paymentMethod: normalizedText,
            description: normalizedText,
            parserId: normalizedText,
            parserLabel: normalizedText,
        },
        account_role_scope: accountRoleScope,
        transaction_type_scope: transactionTypeScope,
    };
}

export function getAccountRuleLabel(
    options: AccountRuleOption<string>[],
    value: string,
    translate: (key: string) => string
): string {
    return translate(options.find(option => option.value === value)?.titleKey || value);
}

function normalizeRoleScope(value: unknown): AccountRuleRoleScope {
    const normalized = normalizeScopeText(value);
    return ACCOUNT_RULE_ROLE_SCOPE_OPTIONS.some(option => option.value === normalized)
        ? normalized as AccountRuleRoleScope
        : 'any';
}

function normalizeTransactionTypeScope(value: unknown): AccountRuleTransactionTypeScope {
    const normalized = normalizeScopeText(value);
    if (normalized === 'any') {
        return 'all';
    }

    return ACCOUNT_RULE_TRANSACTION_SCOPE_OPTIONS.some(option => option.value === normalized)
        ? normalized as AccountRuleTransactionTypeScope
        : 'all';
}

function normalizeFieldScope(value: unknown): AccountRuleFieldScope[] {
    const rawValues = Array.isArray(value)
        ? value
        : typeof value === 'string'
            ? value.split(/[;,|，；]/)
            : DEFAULT_ACCOUNT_RULE_FIELD_SCOPE;
    const allowedValues = new Set(ACCOUNT_RULE_FIELD_SCOPE_OPTIONS.map(option => option.value));
    const result = rawValues
        .map(item => normalizeScopeText(item))
        .filter((item): item is AccountRuleFieldScope => allowedValues.has(item as AccountRuleFieldScope));

    return result.length > 0 ? [...new Set(result)] : [...DEFAULT_ACCOUNT_RULE_FIELD_SCOPE];
}

function normalizeScopeText(value: unknown): string {
    return String(value ?? '').trim().toLowerCase().replaceAll('-', '_');
}

function toFiniteNumber(value: unknown, fallback: number): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : fallback;
}

function toBoolean(value: unknown): boolean {
    return value === true || value === 1 || value === '1' || value === 'true';
}
