import { describe, expect, test } from '@jest/globals';

import {
    getRuleFeatureChips,
    getRuleFeatureSummary,
    getSuggestionFeatureChips,
    type LearningRule,
    type LearningSuggestion,
} from '@/models/learning_center.ts';

const baseSuggestion: LearningSuggestion = {
    id: 1,
    matchType: 'composite',
    matchValue: '',
    matchFeaturesJson: '',
    suggestedType: 'expense',
    suggestedCategoryId: null,
    suggestedSourceAccountId: null,
    suggestedDestinationAccountId: null,
    sampleCount: 1,
    status: 'pending',
    summary: '',
    createdAt: '',
    updatedAt: '',
};

const baseRule: LearningRule = {
    id: 1,
    matchType: 'composite',
    matchValue: '',
    learnedType: 'expense',
    learnedCategoryId: null,
    learnedSourceAccountId: null,
    learnedDestinationAccountId: null,
    enabled: true,
    appliedCount: 0,
    lastAppliedAt: '',
    matchFeaturesJson: '',
    createdAt: '',
    updatedAt: '',
};

describe('learning center feature chips', () => {
    test('normalizes JSON feature keys into labeled chips', () => {
        const chips = getSuggestionFeatureChips({
            ...baseSuggestion,
            matchFeaturesJson: JSON.stringify({
                counterparty: '网银在线（北京）科技有限公司客户备付金',
                description: '快捷支付退货',
                parser_id: 'cmbc',
                payment_method: '网络银行',
            }),
        });

        expect(chips).toEqual([
            { key: 'counterparty', labelKey: 'Counterparty', value: '网银在线（北京）科技有限公司客户备付金' },
            { key: 'description', labelKey: 'Description', value: '快捷支付退货' },
            { key: 'parser_id', labelKey: 'Parser', value: 'cmbc' },
            { key: 'payment_method', labelKey: 'Payment Method', value: '网络银行' },
        ]);
    });

    test('normalizes compact c/d/p/m feature strings without exposing raw source', () => {
        const rule = {
            ...baseRule,
            matchFeaturesJson: 'c=网银在线（北京）科技有限公司客户备付金|d=快捷支付退货 | 网银在线（北京）科技有限公司客户备付金 | 网络银行 | 695438343|p=cmbc|m=网络银行',
        };
        const chips = getRuleFeatureChips(rule);

        expect(chips).toEqual([
            { key: 'counterparty', labelKey: 'Counterparty', value: '网银在线（北京）科技有限公司客户备付金' },
            { key: 'description', labelKey: 'Description', value: '快捷支付退货 | 网银在线（北京）科技有限公司客户备付金 | 网络银行 | 695438343' },
            { key: 'parser_id', labelKey: 'Parser', value: 'cmbc' },
            { key: 'payment_method', labelKey: 'Payment Method', value: '网络银行' },
        ]);
        expect(getRuleFeatureSummary(rule)).not.toContain('c=');
        expect(getRuleFeatureSummary(rule)).not.toContain('|p=');
    });
});
