import { AccountCategory } from '@/core/account.ts';

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
}

export interface AccountRulePayload {
    account_id: number;
    name: string;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
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
}

export interface AccountRuleTestResult {
    matched?: boolean | null;
    accountId?: number | null;
    ruleId?: number | null;
    matchedFields?: string[] | null;
    fallbackUsed?: boolean | null;
}

export interface AccountRuleGroupingAccount {
    readonly id: string | number;
    readonly name: string;
    readonly parentId?: string | number | null;
    readonly category?: number | null;
    readonly displayOrder?: number | null;
    readonly icon?: string | null;
    readonly color?: string | null;
}

export interface AccountRuleAccountGroup {
    readonly key: string;
    readonly accountId: number;
    readonly accountName: string;
    readonly displayName: string;
    readonly parentAccountName: string;
    readonly categoryType: number;
    readonly icon: string;
    readonly color: string;
    readonly displayOrder: number;
    readonly parentDisplayOrder: number;
    readonly rules: AccountRuleItem[];
    readonly ruleCount: number;
    readonly matchCount: number;
}

export interface AccountRuleCategoryGroup {
    readonly key: string;
    readonly categoryType: number;
    readonly categoryName: string;
    readonly displayOrder: number;
    readonly accounts: AccountRuleAccountGroup[];
    readonly ruleCount: number;
    readonly matchCount: number;
}

export function createDefaultAccountRuleForm(accountId?: number | string | null): AccountRuleForm {
    return {
        accountId: accountId ? String(accountId) : '',
        name: '',
        priority: 100,
        ruleExpression: '',
        regexEnabled: false,
        enabled: true,
    };
}

export function normalizeAccountRuleItem(raw: Record<string, unknown>): AccountRuleItem {
    const accountId = toFiniteNumber(raw['accountId'] ?? raw['account_id'], 0);
    const ruleExpression = String(raw['ruleExpression'] ?? raw['rule_expression'] ?? '');
    const regexEnabled = toBoolean(raw['regexEnabled'] ?? raw['regex_enabled']);
    const appliedCount = toFiniteNumber(raw['appliedCount'] ?? raw['applied_count'], 0);
    const matchCount = toFiniteNumber(raw['matchCount'] ?? raw['match_count'], 0);

    return {
        id: toFiniteNumber(raw['id'], 0),
        account_id: accountId,
        accountId,
        accountName: String(raw['accountName'] ?? raw['account_name'] ?? ''),
        accountType: raw['accountType'] === null
            || raw['accountType'] === undefined
            || raw['account_type'] === null
            || raw['account_type'] === undefined
            ? null
            : toFiniteNumber(raw['accountType'] ?? raw['account_type'], 0),
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

    return {
        account_id: accountId,
        name: String(form.name || fallbackName).trim() || fallbackName,
        priority: Number.isFinite(Number(form.priority)) ? Number(form.priority) : 100,
        rule_expression: ruleExpression,
        regex_enabled: !!form.regexEnabled,
        enabled: !!form.enabled,
    };
}

export function buildAccountRuleTestContext(text: string): AccountRuleTestContext {
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
    };
}

export function buildAccountRuleGroups(
    rules: AccountRuleItem[],
    accounts: AccountRuleGroupingAccount[],
    translate: (key: string) => string
): AccountRuleCategoryGroup[] {
    const accountMap = new Map(accounts.map(account => [String(account.id), account]));
    const categoryGroups = new Map<string, MutableAccountRuleCategoryGroup>();

    for (const rule of [...rules].sort(compareRules)) {
        const account = accountMap.get(String(rule.accountId));
        const parent = resolveParentAccount(account, accountMap);
        const categoryType = normalizedCategoryType(account?.category ?? rule.accountType);
        const category = AccountCategory.valueOf(categoryType);
        const categoryKey = `category:${categoryType || 'unknown'}`;
        const categoryGroup = getOrCreateCategoryGroup(
            categoryGroups,
            categoryKey,
            categoryType,
            category ? translate(category.name) : translate('Account'),
            category?.displayOrder ?? Number.MAX_SAFE_INTEGER
        );
        const accountKey = `account:${rule.accountId}`;
        const accountGroup = getOrCreateAccountGroup(categoryGroup, accountKey, rule, account, parent);
        accountGroup.rules.push(rule);
        accountGroup.rules.sort(compareRules);
        accountGroup.ruleCount = accountGroup.rules.length;
        accountGroup.matchCount = accountGroup.rules.reduce((sum, item) => sum + (item.matchCount || item.appliedCount), 0);
    }

    return Array.from(categoryGroups.values())
        .map(categoryGroup => {
            const accounts = Array.from(categoryGroup.accountMap.values()).sort(compareAccountGroups);
            return {
                key: categoryGroup.key,
                categoryType: categoryGroup.categoryType,
                categoryName: categoryGroup.categoryName,
                displayOrder: categoryGroup.displayOrder,
                accounts,
                ruleCount: accounts.reduce((sum, item) => sum + item.ruleCount, 0),
                matchCount: accounts.reduce((sum, item) => sum + item.matchCount, 0),
            };
        })
        .sort((first, second) => first.displayOrder - second.displayOrder
            || first.categoryName.localeCompare(second.categoryName, 'zh-Hans'));
}

interface MutableAccountRuleAccountGroup extends AccountRuleAccountGroup {
    rules: AccountRuleItem[];
    ruleCount: number;
    matchCount: number;
}

interface MutableAccountRuleCategoryGroup extends Omit<AccountRuleCategoryGroup, 'accounts'> {
    readonly accountMap: Map<string, MutableAccountRuleAccountGroup>;
}

function getOrCreateCategoryGroup(
    groups: Map<string, MutableAccountRuleCategoryGroup>,
    key: string,
    categoryType: number,
    categoryName: string,
    displayOrder: number
): MutableAccountRuleCategoryGroup {
    const existing = groups.get(key);
    if (existing) {
        return existing;
    }

    const created: MutableAccountRuleCategoryGroup = {
        key,
        categoryType,
        categoryName,
        displayOrder,
        accountMap: new Map(),
        ruleCount: 0,
        matchCount: 0,
    };
    groups.set(key, created);
    return created;
}

function getOrCreateAccountGroup(
    categoryGroup: MutableAccountRuleCategoryGroup,
    key: string,
    rule: AccountRuleItem,
    account: AccountRuleGroupingAccount | undefined,
    parent: AccountRuleGroupingAccount | undefined
): MutableAccountRuleAccountGroup {
    const existing = categoryGroup.accountMap.get(key);
    if (existing) {
        return existing;
    }

    const accountName = account?.name || rule.accountName || 'Account';
    const parentAccountName = parent?.name || '';
    const created: MutableAccountRuleAccountGroup = {
        key,
        accountId: rule.accountId,
        accountName,
        displayName: parentAccountName ? `${parentAccountName} / ${accountName}` : accountName,
        parentAccountName,
        categoryType: categoryGroup.categoryType,
        icon: String(account?.icon ?? ''),
        color: String(account?.color ?? ''),
        displayOrder: normalizedDisplayOrder(account?.displayOrder),
        parentDisplayOrder: normalizedDisplayOrder(parent?.displayOrder ?? account?.displayOrder),
        rules: [],
        ruleCount: 0,
        matchCount: 0,
    };
    categoryGroup.accountMap.set(key, created);
    return created;
}

function resolveParentAccount(
    account: AccountRuleGroupingAccount | undefined,
    accountMap: Map<string, AccountRuleGroupingAccount>
): AccountRuleGroupingAccount | undefined {
    const parentId = String(account?.parentId ?? '');
    if (!parentId || parentId === '0') {
        return undefined;
    }

    return accountMap.get(parentId);
}

function compareRules(first: AccountRuleItem, second: AccountRuleItem): number {
    return first.priority - second.priority
        || first.accountId - second.accountId
        || first.id - second.id;
}

function compareAccountGroups(first: AccountRuleAccountGroup, second: AccountRuleAccountGroup): number {
    return first.parentDisplayOrder - second.parentDisplayOrder
        || first.displayOrder - second.displayOrder
        || first.displayName.localeCompare(second.displayName, 'zh-Hans')
        || first.accountId - second.accountId;
}

function normalizedCategoryType(value: unknown): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}

function normalizedDisplayOrder(value: unknown): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : Number.MAX_SAFE_INTEGER;
}

function toFiniteNumber(value: unknown, fallback: number): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : fallback;
}

function toBoolean(value: unknown): boolean {
    return value === true || value === 1 || value === '1' || value === 'true';
}
