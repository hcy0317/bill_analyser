export type RuleCenterDomain = 'transfer' | 'duplicate' | 'investment' | 'learning' | 'llm';
export type RuleCenterTab = 'overview' | 'rules' | 'config' | 'ocr-config';
export type RuleConfigTab = 'rules' | 'accounts' | 'learning' | 'recurring';

export interface RuleCenterSelection {
    domain: RuleCenterDomain;
    tab: RuleCenterTab;
    ruleConfigTab: RuleConfigTab;
    shouldRewriteQuery: boolean;
}

export const RULE_CENTER_DOMAINS: RuleCenterDomain[] = ['transfer', 'duplicate', 'learning', 'llm'];

/**
 * 判断路由 query 中的 domain 是否属于规则中心当前支持的一级域。
 */
export function isRuleCenterDomain(value: unknown): value is RuleCenterDomain {
    return typeof value === 'string'
        && (RULE_CENTER_DOMAINS as readonly string[]).includes(value);
}

function firstString(value: unknown): string | undefined {
    return typeof value === 'string' ? value : undefined;
}

/**
 * 根据一级域归一二级 tab，屏蔽投资域和重复域中不存在的规则配置入口。
 */
export function normalizeRuleCenterTab(domain: RuleCenterDomain, tab?: string): RuleCenterTab {
    if (domain === 'llm') {
        if (tab === 'ocr-config') {
            return 'ocr-config';
        }
        return tab === 'config' ? 'config' : 'overview';
    }

    if (domain === 'transfer' && (tab === 'accounts' || tab === 'recurring')) {
        return 'rules';
    }

    if (domain === 'duplicate') {
        return 'overview';
    }

    return tab === 'rules' ? 'rules' : 'overview';
}

function normalizeRuleConfigTab(tab?: string): RuleConfigTab {
    if (tab === 'accounts' || tab === 'learning' || tab === 'recurring') {
        return tab;
    }

    return 'rules';
}

/**
 * 将路由 query 归一为规则中心选择状态，非法或废弃 investment 域回到默认入口。
 */
export function normalizeRuleCenterSelection(input: {
    domain?: unknown;
    tab?: unknown;
}): RuleCenterSelection {
    const domain = firstString(input.domain);
    const tab = firstString(input.tab);

    if (domain === 'investment') {
        return defaultRuleCenterSelection();
    }

    if (isRuleCenterDomain(domain)) {
        return {
            domain,
            tab: normalizeRuleCenterTab(domain, tab),
            ruleConfigTab: normalizeRuleConfigTab(tab),
            shouldRewriteQuery: false,
        };
    }
    return defaultRuleCenterSelection();
}

function defaultRuleCenterSelection(): RuleCenterSelection {
    return {
        domain: 'transfer',
        tab: 'overview',
        ruleConfigTab: 'rules',
        shouldRewriteQuery: false,
    };
}

/**
 * 构造规则中心路由 query，保留无关 query 并把规则配置子 tab 映射回历史 tab 参数。
 */
export function buildRuleCenterQuery(
    currentQuery: Record<string, unknown>,
    domain: RuleCenterDomain,
    tab: RuleCenterTab,
    ruleConfigTab: RuleConfigTab = 'rules'
): Record<string, string> {
    const nextQuery: Record<string, string> = {};

    for (const [key, value] of Object.entries(currentQuery)) {
        if (key === 'domain' || key === 'tab') {
            continue;
        }

        if (typeof value === 'string') {
            nextQuery[key] = value;
        }
    }

    nextQuery['domain'] = domain;
    nextQuery['tab'] = domain === 'transfer' && tab === 'rules' && (ruleConfigTab === 'recurring' || ruleConfigTab === 'accounts')
        ? ruleConfigTab
        : normalizeRuleCenterTab(domain, tab);
    return nextQuery;
}
