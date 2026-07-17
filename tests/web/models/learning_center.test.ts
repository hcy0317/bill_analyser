import { describe, expect, test } from '@jest/globals';

import {
    getSuggestionFeatureSummary,
    getRuleFeatureChips,
    getRuleFeatureSummary,
    getSuggestionFeatureChips,
    normalizeBatchAcceptResponse,
    normalizeGenerateResponse,
    normalizeRulesResponse,
    normalizeSuggestionsResponse,
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

    test('ignores empty and unsupported JSON features while keeping aliases ordered', () => {
        const suggestion = {
            ...baseSuggestion,
            matchFeaturesJson: JSON.stringify({
                m: ' card ',
                c: ' merchant ',
                d: 42,
                unknown: 'hidden',
                parser: '',
            }),
        };

        expect(getSuggestionFeatureChips(suggestion)).toEqual([
            { key: 'counterparty', labelKey: 'Counterparty', value: 'merchant' },
            { key: 'payment_method', labelKey: 'Payment Method', value: 'card' },
        ]);
        expect(getSuggestionFeatureSummary(suggestion)).toBe('Counterparty: merchant · Payment Method: card');
        expect(getSuggestionFeatureChips({ ...suggestion, matchFeaturesJson: 'not-json' })).toEqual([]);
        expect(getSuggestionFeatureChips({ ...suggestion, matchFeaturesJson: '  ' })).toEqual([]);
        expect(getSuggestionFeatureChips({ ...suggestion, matchFeaturesJson: 'c= |d=valid' })).toEqual([
            { key: 'description', labelKey: 'Description', value: 'valid' },
        ]);
    });
});

describe('learning center response normalization', () => {
    test('returns stable empty pagination shapes for non-record payloads', () => {
        for (const payload of [null, undefined, 'invalid', 42, []]) {
            expect(normalizeSuggestionsResponse(payload)).toEqual({ items: [], total: 0, limit: 0, offset: 0 });
            expect(normalizeRulesResponse(payload)).toEqual({ items: [], total: 0, limit: 0, offset: 0 });
        }
    });

    test('normalizes suggestion numbers, nullable identities, strings and default status', () => {
        expect(normalizeSuggestionsResponse({
            items: [{
                id: '7',
                match_type: 'composite',
                match_value: 99,
                match_features_json: '{}',
                suggested_type: 'expense',
                suggested_category_id: '12',
                suggested_source_account_id: 0,
                suggested_destination_account_id: '',
                sample_count: Number.POSITIVE_INFINITY,
                status: '',
                summary: 'summary',
                created_at: null,
                updated_at: 'later',
            }, null],
            total: '2',
            limit: 20,
            offset: 'not-a-number',
        })).toEqual({
            items: [
                {
                    id: 7,
                    matchType: 'composite',
                    matchValue: '',
                    matchFeaturesJson: '{}',
                    suggestedType: 'expense',
                    suggestedCategoryId: 12,
                    suggestedSourceAccountId: null,
                    suggestedDestinationAccountId: null,
                    sampleCount: 0,
                    status: 'pending',
                    summary: 'summary',
                    createdAt: '',
                    updatedAt: 'later',
                },
                {
                    id: 0,
                    matchType: '',
                    matchValue: '',
                    matchFeaturesJson: '',
                    suggestedType: '',
                    suggestedCategoryId: null,
                    suggestedSourceAccountId: null,
                    suggestedDestinationAccountId: null,
                    sampleCount: 0,
                    status: 'pending',
                    summary: '',
                    createdAt: '',
                    updatedAt: '',
                },
            ],
            total: 2,
            limit: 20,
            offset: 0,
        });
    });

    test('normalizes rule booleans and identity fallbacks', () => {
        const response = normalizeRulesResponse({
            items: [true, 1, '1', false].map((enabled, index) => ({
                id: index + 1,
                match_type: 'merchant',
                match_value: `value-${index}`,
                learned_type: 'income',
                learned_category_id: index === 0 ? undefined : index,
                learned_source_account_id: -1,
                learned_destination_account_id: null,
                enabled,
                applied_count: `${index}`,
                last_applied_at: index === 0 ? 1 : 'now',
                match_features_json: '{}',
                created_at: 'created',
                updated_at: 'updated',
            })),
            total: 4,
            limit: '4',
            offset: 0,
        });

        expect(response.items.map(item => item.enabled)).toEqual([true, true, true, false]);
        expect(response.items.map(item => item.learnedCategoryId)).toEqual([null, 1, 2, 3]);
        expect(response.items.every(item => item.learnedSourceAccountId === null)).toBe(true);
        expect(response.items[0]?.lastAppliedAt).toBe('');
        expect(response).toMatchObject({ total: 4, limit: 4, offset: 0 });
    });

    test('normalizes batch and generation responses including optional fields', () => {
        expect(normalizeBatchAcceptResponse({
            accepted: [{ id: '1', ruleId: '9' }, { id: 2, ruleId: 0 }],
            failed: [{ id: 3, error: 'rejected' }, { id: 'bad', error: 7 }],
            acceptedCount: '2',
            failedCount: 2,
        })).toEqual({
            accepted: [{ id: 1, ruleId: 9 }, { id: 2, ruleId: undefined }],
            failed: [{ id: 3, error: 'rejected' }, { id: 0, error: undefined }],
            acceptedCount: 2,
            failedCount: 2,
        });
        expect(normalizeBatchAcceptResponse({ accepted: 'bad', failed: null })).toEqual({
            accepted: [], failed: [], acceptedCount: 0, failedCount: 0,
        });
        expect(normalizeGenerateResponse({
            mined: '8', created: 3, updated: Number.NaN, skipped_existing: '2',
        })).toEqual({ mined: 8, created: 3, updated: 0, skippedExisting: 2 });
        expect(normalizeGenerateResponse(undefined)).toEqual({ mined: 0, created: 0, updated: 0, skippedExisting: 0 });
    });
});
