import {
    buildRuleCenterQuery,
    normalizeRuleCenterSelection,
} from '@/views/desktop/pairingcenter/rule_center_navigation.ts';

describe('rule center navigation mapping', () => {
    test('keeps canonical domain and tab query params', () => {
        expect(normalizeRuleCenterSelection({ domain: 'investment', tab: 'rules' })).toEqual({
            domain: 'investment',
            tab: 'rules',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: false,
        });

        expect(normalizeRuleCenterSelection({ domain: 'llm', tab: 'config' })).toMatchObject({
            domain: 'llm',
            tab: 'config',
            shouldRewriteQuery: false,
        });
    });

    test('maps legacy view and pairType params to canonical Rule Center params', () => {
        expect(normalizeRuleCenterSelection({ pairType: 'transfer' })).toMatchObject({
            domain: 'transfer',
            tab: 'overview',
            shouldRewriteQuery: true,
        });

        expect(normalizeRuleCenterSelection({ view: 'investment-settings' })).toMatchObject({
            domain: 'investment',
            tab: 'rules',
            shouldRewriteQuery: true,
        });

        expect(normalizeRuleCenterSelection({ view: 'learning', tab: 'llm' })).toMatchObject({
            domain: 'llm',
            tab: 'overview',
            shouldRewriteQuery: true,
        });

        expect(normalizeRuleCenterSelection({ view: 'rules', tab: 'recurring' })).toMatchObject({
            domain: 'transfer',
            tab: 'rules',
            legacyRuleTab: 'recurring',
            shouldRewriteQuery: true,
        });

        expect(normalizeRuleCenterSelection({ view: 'rule-center' })).toMatchObject({
            domain: 'transfer',
            tab: 'rules',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: true,
        });

        expect(normalizeRuleCenterSelection({ view: 'rule-center', tab: 'investment' })).toMatchObject({
            domain: 'investment',
            tab: 'rules',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: true,
        });
    });

    test('builds clean canonical queries without spreading legacy params', () => {
        expect(buildRuleCenterQuery(
            { view: 'rules', pairType: 'investment', foo: 'keep', tab: 'recurring' },
            'llm',
            'config'
        )).toEqual({
            foo: 'keep',
            domain: 'llm',
            tab: 'config',
        });
    });
});
