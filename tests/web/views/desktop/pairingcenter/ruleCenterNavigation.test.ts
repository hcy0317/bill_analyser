import fs from 'node:fs';
import path from 'node:path';

import {
    buildRuleCenterQuery,
    normalizeRuleCenterSelection,
} from '@/views/desktop/pairingcenter/rule_center_navigation.ts';

describe('rule center navigation mapping', () => {
    test('keeps canonical domain and tab query params', () => {
        expect(normalizeRuleCenterSelection({ domain: 'investment', tab: 'overview' })).toEqual({
            domain: 'investment',
            tab: 'overview',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: false,
        });

        expect(normalizeRuleCenterSelection({ domain: 'investment', tab: 'rules' })).toEqual({
            domain: 'transfer',
            tab: 'rules',
            legacyRuleTab: 'rules',
            shouldRewriteQuery: true,
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
            domain: 'transfer',
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
            domain: 'transfer',
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
        expect(source).toContain('rule-center-category-filter-menu');
        expect(source).toContain('max-height: min(500px, calc(100vh - 160px))');
        expect(source).toContain('const primaryCategoryFilterTypeOrder');
        const filterOrder = source.slice(
            source.indexOf('const primaryCategoryFilterTypeOrder'),
            source.indexOf('const primaryCategoryFilterGroups')
        );
        expect(filterOrder.indexOf('CategoryType.Income')).toBeLessThan(filterOrder.indexOf('CategoryType.Expense'));
        expect(filterOrder.indexOf('CategoryType.Expense')).toBeLessThan(filterOrder.indexOf('CategoryType.Transfer'));
        expect(filterOrder.indexOf('CategoryType.Transfer')).toBeLessThan(filterOrder.indexOf('CategoryType.Investment'));
        expect(source).toContain('const fallbackPrimaryCategory = findPrimaryCategoryByName(item.category_name)');
        expect(source).toContain("const localizedCategoryLocales = Array.from(new Set([getCurrentLanguageTag(), 'zh-Hans', 'en']))");
        expect(source).not.toContain('v-if="option.icon && option.color"');
        expect(source).not.toContain('v-if="group.icon && group.color"');
        expect(source).not.toContain('v-if="item.category_icon && item.category_color"');
        expect(source).not.toContain('<v-icon v-else :icon="mdiCloseCircle" color="grey" />');
        expect(source).not.toContain('<v-icon v-else size="22" :icon="mdiCloseCircle" color="grey" />');
        expect(source).not.toContain('<v-icon v-else size="24" :icon="mdiCloseCircle" color="grey" />');
        expect(source).not.toContain("tt('Batch Manage')");
        expect(source).not.toContain('v-if="bulkMode"');
        expect(source).not.toContain('v-model="rulePrimaryCategoryFilterKey"');
        expect(source).not.toContain('v-model="ruleRegexFilter"');
        expect(source).not.toContain('v-model="ruleEnabledFilter"');
    });

    test('investment recognition settings page is removed from rule configuration', () => {
        const source = readSource('src/views/desktop/pairingcenter/ListPage.vue');

        expect(source).not.toContain('InvestmentRecognitionSettingsCard');
        expect(source).not.toContain('investment-recognition-settings-card');
        expect(source).not.toContain("'investment-recognition'");
        expect(source).not.toContain("tt('Investment Recognition Settings')");
    });

    test('legacy rule center routes preserve investment settings deep links for normalization', () => {
        const source = readSource('src/router/desktop.ts');

        expect(source).toContain('function buildLegacyRulesCenterRedirect');
        expect(source).toContain("query['view'] = 'rule-center'");
        expect(source).toContain("path: '/rules/center'");
        expect(source).toContain('redirect: route => buildLegacyRulesCenterRedirect(route)');
    });

    test('transaction category header names investment groups explicitly', () => {
        const source = readSource('src/views/base/transactions/TransactionListPageBase.ts');

        expect(source).toContain('case TransactionType.Investment:');
        expect(source).toContain("return tt('Investment');");
    });

    test('rule expression display splits pairing groups on startsExpression only', () => {
        const source = readSource('src/views/desktop/pairingcenter/components/RuleCenterPanel.vue');

        expect(source).toContain('clause.startsExpression');
        expect(source).not.toContain('parenthesisDepth');
        expect(source).not.toContain("clause.joiner === 'OR' && parenthesisDepth === 0");
    });

    test('pairing overview refresh stays beside the page title before the count chip', () => {
        const source = readSource('src/views/desktop/pairingcenter/ListPage.vue');
        const titleIndex = source.indexOf('<span>{{ currentPageTitle }}</span>');
        const refreshIndex = source.indexOf('v-if="showHeaderRefresh"');
        const spacerIndex = source.indexOf('<v-spacer v-if="!showLearningHeaderActions" />', refreshIndex);
        const chipIndex = source.indexOf('rule-center-pair-count-chip');

        expect(titleIndex).toBeGreaterThanOrEqual(0);
        expect(refreshIndex).toBeGreaterThan(titleIndex);
        expect(spacerIndex).toBeGreaterThan(refreshIndex);
        expect(chipIndex).toBeGreaterThan(spacerIndex);
    });
});
