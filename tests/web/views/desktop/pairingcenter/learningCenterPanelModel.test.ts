import { describe, expect, test } from '@jest/globals';

import type { LearningRule } from '@/models/learning_center.ts';
import {
    confidenceColor,
    createLearningRuleMatchTypeOptions,
    createRuleAppliedFilterOptions,
    createRuleEnabledFilterOptions,
    filterLearningRules,
    getLearningStatusColor,
    getLearningStatusLabel,
    getTranslatedLearningStatusLabel,
    normalizeLearningPanelTab,
    translateLearningMatchType,
} from '@/views/desktop/pairingcenter/components/learningCenterPanelModel.ts';

function makeRule(input: Partial<LearningRule> & { id: number }): LearningRule {
    return {
        matchType: 'counterparty',
        matchValue: 'Starbucks',
        learnedType: 'Food/Coffee',
        learnedCategoryId: 1,
        learnedSourceAccountId: null,
        learnedDestinationAccountId: null,
        enabled: true,
        appliedCount: 0,
        lastAppliedAt: '',
        matchFeaturesJson: '',
        createdAt: '',
        updatedAt: '',
        ...input,
        id: input.id,
    };
}

describe('learning center panel view-model helpers', () => {
    const tt = (key: string) => `t:${key}`;

    test('normalizes tabs and builds translated filter options', () => {
        expect(normalizeLearningPanelTab('rules')).toBe('rules');
        expect(normalizeLearningPanelTab('unknown')).toBe('suggestions');

        expect(createLearningRuleMatchTypeOptions([
            makeRule({ id: 1, matchType: 'counterparty' }),
            makeRule({ id: 2, matchType: 'payment_method' }),
        ], tt)).toEqual([
            { title: 't:All', value: '' },
            { title: 't:Counterparty', value: 'counterparty' },
            { title: 't:Payment Method', value: 'payment_method' },
        ]);
        expect(createRuleEnabledFilterOptions(tt).map(option => option.value)).toEqual(['all', 'enabled', 'disabled']);
        expect(createRuleAppliedFilterOptions(tt).map(option => option.value)).toEqual(['all', 'applied', 'not-applied']);
    });

    test('filters learning rules by type, feature text, action, enabled and applied state', () => {
        const rules = [
            makeRule({ id: 1, matchType: 'counterparty', matchValue: 'Starbucks', enabled: true, appliedCount: 2 }),
            makeRule({ id: 2, matchType: 'description', matchValue: 'Taxi', learnedType: 'Transport', enabled: false, appliedCount: 0 }),
        ];

        expect(filterLearningRules(rules, {
            matchType: 'counterparty',
            featureText: 'star',
            learnedAction: 'coffee',
            enabledState: 'enabled',
            appliedState: 'applied',
        }, rule => rule.learnedType).map(rule => rule.id)).toEqual([1]);

        expect(filterLearningRules(rules, {
            matchType: '',
            featureText: '',
            learnedAction: '',
            enabledState: 'disabled',
            appliedState: 'not-applied',
        }, rule => rule.learnedType).map(rule => rule.id)).toEqual([2]);

        expect(filterLearningRules(rules, {
            matchType: '',
            featureText: 'missing',
            learnedAction: '',
            enabledState: 'all',
            appliedState: 'all',
        }, rule => rule.learnedType)).toEqual([]);
        expect(filterLearningRules(rules, {
            matchType: '',
            featureText: '',
            learnedAction: 'missing',
            enabledState: 'all',
            appliedState: 'all',
        }, rule => rule.learnedType)).toEqual([]);
        expect(filterLearningRules(rules, {
            matchType: '',
            featureText: '',
            learnedAction: '',
            enabledState: 'enabled',
            appliedState: 'not-applied',
        }, rule => rule.learnedType)).toEqual([]);
        expect(filterLearningRules(rules, {
            matchType: '',
            featureText: '',
            learnedAction: '',
            enabledState: 'disabled',
            appliedState: 'applied',
        }, rule => rule.learnedType)).toEqual([]);
    });

    test('maps status, match type and confidence display values', () => {
        expect(getLearningStatusColor('pending')).toBe('warning');
        expect(getLearningStatusColor('accepted')).toBe('success');
        expect(getLearningStatusColor('rejected')).toBe('error');
        expect(getLearningStatusColor('other')).toBe('grey');
        expect(getLearningStatusLabel('pending')).toBe('Pending');
        expect(getLearningStatusLabel('accepted')).toBe('Accepted');
        expect(getLearningStatusLabel('rejected')).toBe('Rejected');
        expect(getTranslatedLearningStatusLabel('rejected', tt)).toBe('t:Rejected');
        expect(getTranslatedLearningStatusLabel('custom', tt)).toBe('custom');
        expect(translateLearningMatchType('composite', tt)).toBe('t:Composite Rule');
        expect(translateLearningMatchType('custom', tt)).toBe('custom');
        expect(confidenceColor(0.9)).toBe('success');
        expect(confidenceColor(0.5)).toBe('warning');
        expect(confidenceColor(0.1)).toBe('error');
    });
});
