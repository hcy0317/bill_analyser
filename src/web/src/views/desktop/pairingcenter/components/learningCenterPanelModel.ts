import type { LearningRule } from '@/models/learning_center.ts';

import type { SelectOption } from './llmConfigHelpers.ts';

type Translate = (key: string) => string;

export interface LearningRuleFilters {
    matchType: string;
    featureText: string;
    learnedAction: string;
    enabledState: string;
    appliedState: string;
}

/**
 * 归一化学习中心 tab，避免路由或外部调用传入未知 tab 后落入空白页面。
 */
export function normalizeLearningPanelTab(tab?: string): string {
    if (tab === 'rules' || tab === 'llm' || tab === 'llm-config') {
        return tab;
    }

    return 'suggestions';
}

/**
 * 根据当前学习规则生成匹配类型筛选项，标题保持与表格显示文案一致。
 */
export function createLearningRuleMatchTypeOptions(rules: LearningRule[], tt: Translate): SelectOption[] {
    const options = new Map<string, string>();
    for (const rule of rules) {
        if (rule.matchType) {
            options.set(rule.matchType, translateLearningMatchType(rule.matchType, tt));
        }
    }

    return [
        { title: tt('All'), value: '' },
        ...Array.from(options, ([value, title]) => ({ title, value }))
    ];
}

/**
 * 生成学习规则启用状态筛选项。
 */
export function createRuleEnabledFilterOptions(tt: Translate): SelectOption[] {
    return [
        { title: tt('All'), value: 'all' },
        { title: tt('Enabled'), value: 'enabled' },
        { title: tt('Disabled'), value: 'disabled' },
    ];
}

/**
 * 生成学习规则应用次数筛选项。
 */
export function createRuleAppliedFilterOptions(tt: Translate): SelectOption[] {
    return [
        { title: tt('All'), value: 'all' },
        { title: tt('Applied'), value: 'applied' },
        { title: tt('Not Applied'), value: 'not-applied' },
    ];
}

/**
 * 按当前筛选器过滤学习规则列表，不改变排序和原始规则对象。
 */
export function filterLearningRules(
    rules: LearningRule[],
    filters: LearningRuleFilters,
    getRuleFeatureSummary: (rule: LearningRule) => string
): LearningRule[] {
    return rules.filter((rule) => {
        if (filters.matchType && rule.matchType !== filters.matchType) {
            return false;
        }

        const featureText = `${rule.matchValue} ${getRuleFeatureSummary(rule)}`;
        if (!containsText(featureText, filters.featureText)) {
            return false;
        }

        if (!containsText(rule.learnedType, filters.learnedAction)) {
            return false;
        }

        if (filters.enabledState === 'enabled' && !rule.enabled) {
            return false;
        }

        if (filters.enabledState === 'disabled' && rule.enabled) {
            return false;
        }

        if (filters.appliedState === 'applied' && rule.appliedCount <= 0) {
            return false;
        }

        if (filters.appliedState === 'not-applied' && rule.appliedCount > 0) {
            return false;
        }

        return true;
    });
}

/**
 * 将学习建议/候选状态映射为 Vuetify 色彩 token。
 */
export function getLearningStatusColor(status: string): string {
    switch (status) {
        case 'pending': return 'warning';
        case 'accepted': return 'success';
        case 'rejected': return 'error';
        default: return 'grey';
    }
}

/**
 * 将学习建议状态映射为可翻译文案 key。
 */
export function getLearningStatusLabel(status: string): string {
    switch (status) {
        case 'pending': return 'Pending';
        case 'accepted': return 'Accepted';
        case 'rejected': return 'Rejected';
        default: return status;
    }
}

/**
 * 将 LLM 候选状态映射为已翻译文案。
 */
export function getTranslatedLearningStatusLabel(status: string, tt: Translate): string {
    const label = getLearningStatusLabel(status);
    return label === status ? status : tt(label);
}

/**
 * 将学习规则匹配类型映射为当前语言的展示文案。
 */
export function translateLearningMatchType(type: string, tt: Translate): string {
    const map: Record<string, string> = {
        'composite': tt('Composite Rule'),
        'counterparty': tt('Counterparty'),
        'description': tt('Description'),
        'keyword': tt('Keyword'),
        'amount': tt('Amount'),
        'payment_method': tt('Payment Method'),
    };
    return map[type] || type;
}

/**
 * 根据置信度选择候选规则的状态色。
 */
export function confidenceColor(confidence: number): string {
    if (confidence >= 0.8) return 'success';
    if (confidence >= 0.5) return 'warning';
    return 'error';
}

function containsText(source: string, query: string): boolean {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) {
        return true;
    }

    return source.toLowerCase().includes(normalizedQuery);
}
