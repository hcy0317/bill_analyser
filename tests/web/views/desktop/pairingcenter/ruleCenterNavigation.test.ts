import fs from 'node:fs';
import path from 'node:path';

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

describe('rule center UX source guards', () => {
    const readSource = (relativePath: string) => fs.readFileSync(
        path.resolve(process.cwd(), relativePath),
        'utf-8'
    );

    test('primary navigation buttons render label-only options', () => {
        const source = readSource('src/components/desktop/BtnVerticalGroup.vue');

        expect(source).toContain('button.name ?? button.label');
        expect(source).toContain('label?: string');
    });

    test('category recognition table keeps selection/filter/pagination controls stable', () => {
        const source = readSource('src/views/desktop/pairingcenter/components/RuleCenterPanel.vue');

        expect(source).toContain('v-if="showTabSwitcher"');
        expect(source).toContain(':open-delay="1500"');
        expect(source).toContain("tt('Rows per page')");
        expect(source).toContain("tt('Rule range'");
        expect(source).toContain('const ruleTableColumnCount = 7');
        expect(source).not.toContain("tt('Batch Manage')");
        expect(source).not.toContain('v-if="bulkMode"');
        expect(source).not.toContain('v-model="rulePrimaryCategoryFilterKey"');
        expect(source).not.toContain('v-model="ruleRegexFilter"');
        expect(source).not.toContain('v-model="ruleEnabledFilter"');
    });

    test('rule expression display keeps slash OR in the same expression group', () => {
        const source = readSource('src/views/desktop/pairingcenter/components/RuleCenterPanel.vue');

        expect(source).toContain('clause.startsExpression');
        expect(source).toContain('parenthesisDepth === 0');
        expect(source).not.toContain("clause.joiner === 'OR' && parenthesisDepth === 0");
    });

    test('pairing overview refresh stays beside the page title before the count chip', () => {
        const source = readSource('src/views/desktop/pairingcenter/ListPage.vue');
        const titleIndex = source.indexOf('<span>{{ currentPageTitle }}</span>');
        const refreshIndex = source.indexOf('v-if="showHeaderRefresh"');
        const spacerIndex = source.indexOf('<v-spacer />', refreshIndex);
        const chipIndex = source.indexOf('rule-center-pair-count-chip');

        expect(titleIndex).toBeGreaterThanOrEqual(0);
        expect(refreshIndex).toBeGreaterThan(titleIndex);
        expect(spacerIndex).toBeGreaterThan(refreshIndex);
        expect(chipIndex).toBeGreaterThan(spacerIndex);
    });
});
