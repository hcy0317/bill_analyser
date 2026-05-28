export type RuleCenterDomain = 'transfer' | 'duplicate' | 'investment' | 'learning' | 'llm';
export type RuleCenterTab = 'overview' | 'rules' | 'config' | 'ocr-config';
export type LegacyRuleTab = 'rules' | 'accounts' | 'learning' | 'recurring';

export interface RuleCenterSelection {
    domain: RuleCenterDomain;
    tab: RuleCenterTab;
    legacyRuleTab: LegacyRuleTab;
    shouldRewriteQuery: boolean;
}

export const RULE_CENTER_DOMAINS: RuleCenterDomain[] = ['transfer', 'duplicate', 'learning', 'llm'];

export function isRuleCenterDomain(value: unknown): value is RuleCenterDomain {
    return typeof value === 'string'
        && (RULE_CENTER_DOMAINS as readonly string[]).includes(value);
}

function firstString(value: unknown): string | undefined {
    return typeof value === 'string' ? value : undefined;
}

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

function normalizeLegacyRuleTab(tab?: string): LegacyRuleTab {
    if (tab === 'accounts' || tab === 'learning' || tab === 'recurring') {
        return tab;
    }

    return 'rules';
}

export function normalizeRuleCenterSelection(input: {
    domain?: unknown;
    tab?: unknown;
    view?: unknown;
    pairType?: unknown;
}): RuleCenterSelection {
    const domain = firstString(input.domain);
    const tab = firstString(input.tab);
    const view = firstString(input.view);
    const pairType = firstString(input.pairType);

    if (domain === 'investment') {
        return {
            domain: 'transfer',
            tab: 'rules',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: true,
        };
    }

    if (isRuleCenterDomain(domain)) {
        return {
            domain,
            tab: normalizeRuleCenterTab(domain, tab),
            legacyRuleTab: normalizeLegacyRuleTab(tab),
            shouldRewriteQuery: false,
        };
    }

    if (pairType === 'investment') {
        return {
            domain: 'transfer',
            tab: 'rules',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: true,
        };
    }

    if (pairType === 'duplicate') {
        return {
            domain: 'duplicate',
            tab: 'overview',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: true,
        };
    }

    if (pairType === 'transfer') {
        return {
            domain: 'transfer',
            tab: 'overview',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: true,
        };
    }

    if (view === 'investment-settings') {
        return {
            domain: 'transfer',
            tab: 'rules',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: true,
        };
    }

    if (view === 'learning' || view === 'learning-center') {
        if (tab === 'llm') {
            return {
                domain: 'llm',
                tab: 'overview',
                legacyRuleTab: 'learning',
                shouldRewriteQuery: true,
            };
        }

        if (tab === 'ocr-config') {
            return {
                domain: 'llm',
                tab: 'ocr-config',
                legacyRuleTab: 'learning',
                shouldRewriteQuery: true,
            };
        }

        return {
            domain: 'learning',
            tab: tab === 'rules' ? 'rules' : 'overview',
            legacyRuleTab: 'learning',
            shouldRewriteQuery: true,
        };
    }

    if (view === 'rules' || view === 'rule-center') {
        if (tab === 'investment') {
            return {
                domain: 'transfer',
                tab: 'rules',
                legacyRuleTab: 'rules',
                shouldRewriteQuery: true,
            };
        }

        if (tab === 'learning') {
            return {
                domain: 'learning',
                tab: 'rules',
                legacyRuleTab: 'learning',
                shouldRewriteQuery: true,
            };
        }

        return {
            domain: 'transfer',
            tab: 'rules',
            legacyRuleTab: normalizeLegacyRuleTab(tab),
            shouldRewriteQuery: true,
        };
    }

    return {
        domain: 'transfer',
        tab: 'overview',
        legacyRuleTab: 'rules',
        shouldRewriteQuery: false,
    };
}

export function buildRuleCenterQuery(
    currentQuery: Record<string, unknown>,
    domain: RuleCenterDomain,
    tab: RuleCenterTab,
    legacyRuleTab: LegacyRuleTab = 'rules'
): Record<string, string> {
    const nextQuery: Record<string, string> = {};

    for (const [key, value] of Object.entries(currentQuery)) {
        if (key === 'domain' || key === 'tab' || key === 'view' || key === 'pairType') {
            continue;
        }

        if (typeof value === 'string') {
            nextQuery[key] = value;
        }
    }

    nextQuery['domain'] = domain;
    nextQuery['tab'] = domain === 'transfer' && tab === 'rules' && (legacyRuleTab === 'recurring' || legacyRuleTab === 'accounts')
        ? legacyRuleTab
        : normalizeRuleCenterTab(domain, tab);
    return nextQuery;
}
