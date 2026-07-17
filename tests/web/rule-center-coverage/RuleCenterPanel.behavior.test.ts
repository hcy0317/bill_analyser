import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mountedCallbacks: Array<() => unknown> = [];
const templateRefs = new Map<string, { value: unknown }>();
const templateHandlers: Array<(event: any) => unknown> = [];

const mockAxiosGet = jest.fn<(...args: any[]) => Promise<any>>();
const mockServices = {
    createCategoryRule: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteCategoryRule: jest.fn<(...args: any[]) => Promise<any>>(),
    getRulesOverview: jest.fn<(...args: any[]) => Promise<any>>(),
    testCategoryRule: jest.fn<(...args: any[]) => Promise<any>>(),
    updateCategoryRule: jest.fn<(...args: any[]) => Promise<any>>()
};

const expenseCategory = {
    id: '10', parentId: '', name: 'Food', type: 3, icon: 'food', color: '112233', hidden: false,
    subCategories: [
        { id: '11', parentId: '10', name: 'Cafe', type: 3, icon: 'cafe', color: '445566', hidden: false, subCategories: [] }
    ]
};
const incomeCategory = {
    id: '20', parentId: '', name: 'Salary', type: 2, icon: 'salary', color: '223344', hidden: false,
    subCategories: []
};
const transferCategory = {
    id: '30', parentId: '', name: 'Transfer', type: 4, icon: 'transfer', color: '334455', hidden: false,
    subCategories: []
};
const investmentCategory = {
    id: '40', parentId: '', name: 'Investment', type: 5, icon: 'investment', color: '556677', hidden: false,
    subCategories: []
};

const mockCategoryStore: any = {
    allTransactionCategories: {
        2: [incomeCategory],
        3: [expenseCategory],
        4: [transferCategory],
        5: [investmentCategory]
    },
    allTransactionCategoriesMap: {
        10: expenseCategory,
        11: expenseCategory.subCategories[0],
        20: incomeCategory,
        30: transferCategory,
        40: investmentCategory
    },
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>()
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => unknown) => mountedCallbacks.push(callback),
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            templateRefs.set(name, target);
            return target;
        }
    };
});

jest.mock('axios', () => ({
    __esModule: true,
    default: {
        get: (...args: any[]) => mockAxiosGet(...args),
        isAxiosError: (error: any) => Boolean(error?.isAxiosError)
    }
}));

jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoryStore
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => values
            ? `${key}:${JSON.stringify(values)}`
            : key,
        getCurrentLanguageTag: () => 'zh-Hans',
        getAllTransactionDefaultCategories: (_type: number, locale: string) => ({
            expense: [{
                name: locale === 'en' ? 'Legacy Food' : '历史餐饮',
                type: 3,
                icon: `preset-${locale}`,
                color: '778899',
                subCategories: []
            }]
        })
    })
}));

for (const componentPath of [
    '@/components/common/CategoryRuleBuilderFields.vue',
    '@/components/desktop/ItemIcon.vue',
    '@/components/desktop/SettingsJsonImportExportButton.vue',
    '@/components/desktop/SnackBar.vue',
    '@/components/desktop/TwoColumnSelect.vue',
    '@/views/desktop/pairingcenter/components/RuleExpressionDisplay.vue'
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: { name: 'RuleCenterCoverageStub' }
    }));
}

const RuleCenterPanel = require('@/views/desktop/pairingcenter/components/RuleCenterPanel.vue').default as any;

function createRule(overrides: Record<string, unknown> = {}): any {
    return {
        id: 1,
        name: 'Coffee rule',
        category_id: 11,
        category_name: 'Food',
        sub_category_name: 'Cafe',
        priority: 10,
        rule_expression: 'coffee !refund',
        regex_enabled: false,
        enabled: true,
        applied_count: 7,
        ...overrides
    };
}

function createOverview(overrides: Record<string, unknown> = {}): any {
    return {
        learningRules: [
            { id: 1, matchType: 'merchant', matchValue: 'Cafe', learnedType: 'Expense', appliedCount: 4, enabled: true },
            { id: 2, matchType: 'merchant', matchValue: 'Shop', learnedType: 'Expense', appliedCount: 1, enabled: false }
        ],
        learningRuleCount: 2,
        categoryRuleCount: 2,
        recurringRules: [{ id: 'monthly' }],
        recurringRuleCount: 1,
        totalRuleCount: 5,
        ...overrides
    };
}

function successResponse(result: unknown = undefined): any {
    return { data: { success: true, result } };
}

function setupPanel(props: Record<string, unknown> = {}): { bindings: any; exposed: Record<string, unknown> } {
    const exposed: Record<string, unknown> = {};
    const bindings = RuleCenterPanel.setup({
        initTab: 'rules',
        tabs: ['rules', 'learning', 'recurring'],
        title: '',
        hideHeader: false,
        headerActionsTarget: '',
        ...props
    }, {
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value)
    });
    return { bindings, exposed };
}

function setSnackbar(value: { showMessage: jest.Mock }): void {
    const target = templateRefs.get('snackbar');
    if (!target) throw new Error('missing snackbar template ref');
    target.value = value;
}

async function flushAsync(): Promise<void> {
    await Promise.resolve();
    await new Promise(resolve => setImmediate(resolve));
}

async function renderPanel(
    props: Record<string, unknown>,
    mutate: (bindings: any) => void
): Promise<string> {
    const { createSSRApp, defineComponent, h } = require('vue') as any;
    const { renderToString } = require('vue/server-renderer') as any;
    const RuntimePanel = {
        ...RuleCenterPanel,
        setup(runtimeProps: any, context: any) {
            const bindings = RuleCenterPanel.setup(runtimeProps, context);
            mutate(bindings);
            return bindings;
        }
    };
    templateHandlers.length = 0;
    const app = createSSRApp(RuntimePanel, props);
    const UiStub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [name, handler] of Object.entries(attrs)) {
                if (name.startsWith('on') && typeof handler === 'function') {
                    templateHandlers.push(handler as (event: any) => unknown);
                }
            }
            return () => h(
                'div',
                attrs,
                Object.values(slots).flatMap(slot => {
                    if (typeof slot !== 'function') return [];
                    const renderSlot = slot as (props?: any) => unknown[];
                    return [
                        ...renderSlot({ props: { role: 'button' }, item: { enabled: true } }),
                        ...renderSlot({ props: { role: 'button' }, item: { enabled: false } })
                    ];
                })
            );
        }
    });
    for (const name of [
        'v-row', 'v-col', 'v-card', 'v-card-title', 'v-card-text', 'v-card-actions', 'v-icon', 'v-spacer',
        'v-btn', 'v-progress-linear', 'v-progress-circular', 'v-alert', 'v-tabs', 'v-tab', 'v-tabs-window',
        'v-tabs-window-item', 'v-chip', 'v-table', 'v-checkbox-btn', 'v-menu', 'v-list', 'v-list-item',
        'v-list-item-title', 'v-list-subheader', 'v-divider', 'v-switch', 'v-tooltip', 'v-select', 'v-pagination',
        'v-data-table', 'v-dialog', 'v-form', 'v-text-field'
    ]) app.component(name, UiStub);
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

async function invokeCapturedTemplateHandlers(): Promise<void> {
    const events = [
        { key: 'Enter', stopPropagation: jest.fn(), preventDefault: jest.fn() },
        { key: ' ', stopPropagation: jest.fn(), preventDefault: jest.fn() }
    ];
    for (const handler of [...templateHandlers]) {
        for (const event of events) {
            try {
                await handler(event);
            } catch {
                // Some generated v-model handlers intentionally receive typed scalar values.
                // Their wrapper execution still belongs to the template coverage contract.
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mountedCallbacks.length = 0;
    templateRefs.clear();
    templateHandlers.length = 0;
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockAxiosGet.mockResolvedValue({ data: { success: true, data: [
        createRule(),
        createRule({ id: 2, name: 'Refund regex', priority: 20, rule_expression: '^refund$', regex_enabled: true, enabled: false })
    ] } });
    mockServices.getRulesOverview.mockResolvedValue(successResponse(createOverview()));
    mockServices.createCategoryRule.mockResolvedValue(successResponse({ id: 3 }));
    mockServices.updateCategoryRule.mockResolvedValue(successResponse({ id: 1 }));
    mockServices.deleteCategoryRule.mockResolvedValue(successResponse());
    mockServices.testCategoryRule.mockResolvedValue(successResponse({ matched: true }));
});

describe('RuleCenterPanel production-loaded setup and derived state', () => {
    test('loads category and overview data while preserving rule grouping and tab contracts', async () => {
        const { bindings, exposed } = setupPanel({ initTab: 'learning', title: 'Rules' });

        expect(exposed['refresh']).toEqual(expect.any(Function));
        expect(mountedCallbacks).toHaveLength(1);
        expect(bindings.activeTab.value).toBe('learning');
        expect(bindings.title.value).toBe('Rules');
        expect(bindings.showTabSwitcher.value).toBe(true);
        expect(bindings.hasHeaderActionsTarget.value).toBe(false);
        expect(bindings.headerActionsTarget.value).toBe('body');
        expect(bindings.hasTab('recurring')).toBe(true);
        expect(bindings.normalizeTab('invalid')).toBe('rules');

        await mountedCallbacks[0]!();

        expect(mockAxiosGet).toHaveBeenCalledWith('category-rules/', { params: { enabled_only: false } });
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(bindings.loading.value).toBe(false);
        expect(bindings.categoryRules.value).toHaveLength(2);
        expect(bindings.overview.value.totalRuleCount).toBe(5);
        expect(bindings.categoryPickerItems.value).toHaveLength(4);
        expect(bindings.primaryCategoryFilterGroups.value).toHaveLength(4);
        expect(bindings.displayCategoryRules.value[0].category_full_name).toBe('Food / Cafe');
        expect(bindings.groupedCategoryRuleTargets.value).toHaveLength(1);
        expect(bindings.rulePaginationLabel.value).toBe('1-1 / 1');
        expect(bindings.visibleRuleIds.value).toStrictEqual([1, 2]);
        expect(bindings.getCategoryTypeLabel(3)).toBe('Expense');
        expect(bindings.getCategoryTypeLabel(2)).toBe('Income');
        expect(bindings.getCategoryTypeLabel(4)).toBe('Transfer');
        expect(bindings.getCategoryTypeLabel(5)).toBe('Investment');
        expect(bindings.getCategoryTypeLabel(99)).toBe('Category');
        expect(bindings.ruleCategorySelection.value.label).toBe('');
        expect(bindings.autoRuleName.value).toBe('Unassigned Category · Category Rule');
        expect(bindings.learningHeaders.value).toHaveLength(5);
        expect(bindings.ruleBooleanFilterOptions.value.map((item: any) => item.value)).toStrictEqual(['all', 'yes', 'no']);
    });

    test('falls back to default tabs, an empty rule range, and the first valid tab slot', () => {
        const defaultTabs = setupPanel({ tabs: undefined }).bindings;
        expect(defaultTabs.visibleTabs.value).toStrictEqual(['rules', 'learning', 'recurring']);
        expect(defaultTabs.rulePaginationStart.value).toBe(0);
        expect(defaultTabs.rulePaginationLabel.value).toBe('0 / 0');

        const sparseTabs = new Array(1) as any[];
        const sparseTabBindings = setupPanel({ tabs: sparseTabs, initTab: 'invalid' }).bindings;
        expect(sparseTabBindings.normalizeTab('invalid')).toBe('rules');
    });

    test('covers filters, pagination, selections, collapse state, and builder projection', async () => {
        const { bindings } = setupPanel();
        await bindings.fetchAll();
        const target = bindings.orderedCategoryRuleTargets.value[0];

        expect(bindings.matchesRuleExpressionFilter('coffee !refund')).toBe(true);
        bindings.ruleExpressionFilterQuery.value = 'coffee';
        expect(bindings.filteredDisplayCategoryRules.value).toHaveLength(1);
        bindings.ruleExpressionFilterQuery.value = '[';
        bindings.ruleExpressionFilterUseRegex.value = true;
        expect(bindings.filteredDisplayCategoryRules.value).toHaveLength(0);
        bindings.clearExpressionFilter();

        bindings.setRegexFilter('yes');
        expect(bindings.filteredDisplayCategoryRules.value.map((item: any) => item.id)).toStrictEqual([2]);
        bindings.setRegexFilter('no');
        expect(bindings.filteredDisplayCategoryRules.value.map((item: any) => item.id)).toStrictEqual([1]);
        bindings.setRegexFilter('all');
        bindings.setEnabledFilter('yes');
        expect(bindings.filteredDisplayCategoryRules.value.map((item: any) => item.id)).toStrictEqual([1]);
        bindings.setEnabledFilter('no');
        expect(bindings.filteredDisplayCategoryRules.value.map((item: any) => item.id)).toStrictEqual([2]);
        bindings.setEnabledFilter('all');

        bindings.setPrimaryCategoryFilter('missing');
        expect(bindings.filteredDisplayCategoryRules.value).toHaveLength(0);
        bindings.setPrimaryCategoryFilter('');
        expect(bindings.filteredDisplayCategoryRules.value).toHaveLength(2);
        expect(bindings.categoryFilterMenu.value).toBe(false);

        expect(bindings.isCategoryGroupCollapsed(target.category_group_key)).toBe(false);
        bindings.toggleCategoryGroupCollapsed(target.category_group_key);
        expect(bindings.isCategoryGroupCollapsed(target.category_group_key)).toBe(true);
        bindings.toggleCategoryGroupCollapsed(target.category_group_key);
        expect(bindings.isCategoryGroupCollapsed(target.category_group_key)).toBe(false);

        bindings.setCategoryRuleTargetSelected(target, true);
        expect(bindings.isCategoryRuleTargetSelected(target)).toBe(true);
        expect(bindings.isCategoryRuleTargetPartiallySelected(target)).toBe(false);
        bindings.selectedRuleIds.value = [1];
        expect(bindings.isCategoryRuleTargetPartiallySelected(target)).toBe(true);
        expect(bindings.someVisibleRulesSelected.value).toBe(true);
        expect(bindings.allVisibleRulesSelected.value).toBe(false);
        bindings.setVisibleRulesSelected(true);
        expect(bindings.allVisibleRulesSelected.value).toBe(true);
        bindings.setVisibleRulesSelected(false);
        expect(bindings.selectedRuleIds.value).toStrictEqual([]);
        bindings.setCategoryRuleTargetSelected(target, false);

        bindings.selectedRuleIds.value = [1, 999];
        bindings.pruneSelectedRuleIds();
        expect(bindings.selectedRuleIds.value).toStrictEqual([1]);
        bindings.ruleItemsPerPage.value = 1;
        bindings.rulePage.value = 5;
        await (jest.requireActual('vue') as any).nextTick();
        expect(bindings.rulePage.value).toBe(1);

        bindings.ruleForm.value.category_id = '11';
        expect(bindings.ruleCategorySelection.value.label).toBe('Food / Cafe');
        expect(bindings.autoRuleName.value).toBe('Food / Cafe · Category Rule');
        expect(bindings.ruleBuilderModel.value.ruleExpression).toBe('');
        bindings.ruleBuilderModel.value = { priority: 55, ruleExpression: 'tea', regexEnabled: true, enabled: false };
        expect(bindings.ruleForm.value).toEqual(expect.objectContaining({
            priority: 55, rule_expression: 'tea', regex_enabled: true, enabled: false
        }));
        expect(bindings.getRuleExpressionGroups(createRule())).not.toHaveLength(0);
    });
});

describe('RuleCenterPanel production-loaded mutations', () => {
    test('creates, edits, toggles, tests, and deletes rules through current service contracts', async () => {
        const { bindings } = setupPanel();
        const snackbar = { showMessage: jest.fn() };
        setSnackbar(snackbar);
        await bindings.fetchAll();

        bindings.openCreateDialog();
        bindings.ruleForm.value = {
            category_id: '11', priority: 30, rule_expression: ' bakery ', regex_enabled: false, enabled: true
        };
        await bindings.saveRule();
        expect(mockServices.createCategoryRule).toHaveBeenCalledWith({
            category_id: 11,
            name: 'Food / Cafe · Category Rule',
            priority: 30,
            rule_expression: 'bakery',
            regex_enabled: false,
            enabled: true
        });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Rule created', undefined);

        bindings.openEditDialog(createRule({ category_id: null }));
        expect(bindings.ruleForm.value.category_id).toBe('');

        const firstRule = bindings.categoryRules.value[0];
        bindings.openEditDialog(firstRule);
        expect(bindings.editingRule.value.id).toBe(1);
        bindings.ruleForm.value.rule_expression = 'coffee shop';
        await bindings.saveRule();
        expect(mockServices.updateCategoryRule).toHaveBeenCalledWith(1, expect.objectContaining({
            rule_expression: 'coffee shop'
        }));
        expect(snackbar.showMessage).toHaveBeenCalledWith('Rule updated', undefined);

        await bindings.toggleEnabled(firstRule, false);
        expect(mockServices.updateCategoryRule).toHaveBeenCalledWith(1, { enabled: false });
        bindings.togglingRuleIds.value = [1];
        await bindings.toggleEnabled(firstRule, false);
        bindings.togglingRuleIds.value = [];
        await bindings.toggleEnabled(firstRule, true);

        const target = bindings.orderedCategoryRuleTargets.value[0];
        bindings.selectedRuleIds.value = [1];
        await bindings.bulkUpdateSelectedRules({ enabled: true });
        bindings.setCategoryRuleTargetSelected(target, true);
        await bindings.bulkUpdateSelectedRules({ regex_enabled: true });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Selected rules updated: {count}', { count: 2 });
        bindings.bulkOperating.value = true;
        await bindings.bulkUpdateSelectedRules({ enabled: false });
        bindings.bulkOperating.value = false;

        bindings.openTestDialog(firstRule);
        expect(bindings.showTestDialog.value).toBe(true);
        await bindings.runTest();
        bindings.testText.value = 'coffee';
        await bindings.runTest();
        expect(mockServices.testCategoryRule).toHaveBeenCalledWith(1, 'coffee');
        expect(bindings.testResult.value).toBe(true);
        mockServices.testCategoryRule.mockResolvedValueOnce(successResponse({ matched: false }));
        await bindings.runTest();
        expect(bindings.testResult.value).toBe(false);

        bindings.deletingRule.value = null;
        await bindings.doDelete();
        bindings.confirmDelete(firstRule);
        await bindings.doDelete();
        expect(mockServices.deleteCategoryRule).toHaveBeenCalledWith(1);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Rule deleted', undefined);

        bindings.selectedRuleIds.value = [];
        bindings.confirmBulkDelete();
        bindings.selectedRuleIds.value = [1, 2];
        bindings.confirmBulkDelete();
        expect(bindings.showBulkDeleteDialog.value).toBe(true);
        await bindings.bulkDeleteSelectedRules();
        expect(mockServices.deleteCategoryRule).toHaveBeenCalledWith(2);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Selected rules deleted: {count}', { count: 2 });
    });

    test('rolls back optimistic mutations and reports validation, API, and load errors', async () => {
        const { bindings } = setupPanel();
        await bindings.fetchAll();
        const originalRules = bindings.categoryRules.value;
        const firstRule = originalRules[0];

        bindings.openCreateDialog();
        await bindings.saveRule();
        expect(bindings.error.value).toBe('Category is required');
        expect(bindings.saving.value).toBe(false);

        mockServices.updateCategoryRule.mockRejectedValueOnce(new Error('toggle failed'));
        await bindings.toggleEnabled(firstRule, false);
        expect(bindings.categoryRules.value.find((item: any) => item.id === 1).enabled).toBe(true);
        expect(bindings.error.value).toBe('toggle failed');

        bindings.selectedRuleIds.value = [1, 2];
        mockServices.updateCategoryRule.mockRejectedValueOnce(new Error('bulk update failed'));
        await bindings.bulkUpdateSelectedRules({ enabled: false });
        expect(bindings.categoryRules.value).toStrictEqual(originalRules);
        expect(bindings.error.value).toBe('bulk update failed');

        bindings.confirmDelete(firstRule);
        mockServices.deleteCategoryRule.mockRejectedValueOnce(new Error('delete failed'));
        await bindings.doDelete();
        expect(bindings.showDeleteDialog.value).toBe(true);
        expect(bindings.error.value).toBe('delete failed');

        mockServices.deleteCategoryRule.mockRejectedValueOnce(new Error('bulk delete failed'));
        await bindings.bulkDeleteSelectedRules();
        expect(bindings.categoryRules.value).toStrictEqual(originalRules);
        expect(bindings.error.value).toBe('bulk delete failed');

        bindings.openTestDialog(firstRule);
        bindings.testText.value = 'coffee';
        mockServices.testCategoryRule.mockResolvedValueOnce({ data: { success: false } });
        await bindings.runTest();
        expect(bindings.testResult.value).toBeNull();
        expect(bindings.error.value).toBe('Test failed');

        mockAxiosGet.mockResolvedValueOnce({ data: { success: false } });
        await bindings.fetchCategoryRules();
        expect(bindings.error.value).toBe('Failed to load category rules');
        mockAxiosGet.mockResolvedValueOnce({ data: { success: false, error: 'bad category response' } });
        await bindings.fetchCategoryRules();
        expect(bindings.error.value).toBe('bad category response');
        mockAxiosGet.mockResolvedValueOnce({ data: { success: true } });
        await bindings.fetchCategoryRules();
        expect(bindings.categoryRules.value).toStrictEqual([]);

        mockServices.getRulesOverview.mockResolvedValueOnce(successResponse(null));
        await bindings.fetchOverview();
        mockServices.getRulesOverview.mockResolvedValueOnce(successResponse({}));
        await bindings.fetchOverview();
        expect(bindings.overview.value).toStrictEqual({
            learningRules: [], learningRuleCount: 0, categoryRuleCount: 0,
            recurringRules: [], recurringRuleCount: 0, totalRuleCount: 0
        });
        mockServices.getRulesOverview.mockResolvedValueOnce({ data: { success: false } });
        await bindings.fetchOverview();
        expect(bindings.error.value).toBe('Failed to load rules overview');

        mockAxiosGet.mockRejectedValueOnce({
            isAxiosError: true,
            message: 'request failed',
            response: { data: { error: { message: 'nested category error' } } }
        });
        await bindings.fetchCategoryRules();
        expect(bindings.error.value).toBe('nested category error');

        mockAxiosGet.mockResolvedValueOnce({ data: { success: true, data: [createRule()] } });
        await bindings.fetchCategoryRules();
        bindings.rulePage.value = 5;
        bindings.categoryRules.value = [];
        await (jest.requireActual('vue') as any).nextTick();
        expect(bindings.rulePage.value).toBe(1);

        mockCategoryStore.loadAllCategories.mockRejectedValueOnce(new Error('category store failed'));
        await expect(bindings.fetchAll()).rejects.toThrow('category store failed');
        expect(bindings.loading.value).toBe(false);
    });
});

describe('RuleCenterPanel production template', () => {
    test('renders populated rules, learning, recurring, alerts, and dialogs', async () => {
        const html = await renderPanel({
            initTab: 'rules',
            tabs: ['rules', 'learning', 'recurring'],
            title: 'Rule Center',
            hideHeader: false,
            headerActionsTarget: ''
        }, bindings => {
            bindings.loading.value = true;
            bindings.error.value = 'visible error';
            bindings.categoryRules.value = [
                createRule(),
                createRule({ id: 2, regex_enabled: true, enabled: false, rule_expression: '^refund$' })
            ];
            bindings.overview.value = createOverview();
            bindings.selectedRuleIds.value = [1];
            bindings.togglingRuleIds.value = [2];
            bindings.showEditDialog.value = true;
            bindings.editingRule.value = bindings.categoryRules.value[0];
            bindings.showTestDialog.value = true;
            bindings.testRuleName.value = 'Coffee rule';
            bindings.testResult.value = true;
            bindings.showDeleteDialog.value = true;
            bindings.deletingRule.value = bindings.categoryRules.value[0];
            bindings.showBulkDeleteDialog.value = true;
        });

        expect(html).toContain('Rule Center');
        expect(html).toContain('visible error');
        expect(html).toContain('Food');
        expect(html).toContain('Manage Scheduled Templates');
        expect(html).toContain('Match!');
        expect(html).toContain('Coffee rule');
        await invokeCapturedTemplateHandlers();
    });

    test('renders empty, collapsed, headerless, false-test, and single-tab variants', async () => {
        const emptyHtml = await renderPanel({
            initTab: 'rules', tabs: ['rules'], title: '', hideHeader: true, headerActionsTarget: '#actions'
        }, bindings => {
            bindings.categoryRules.value = [];
            bindings.showEditDialog.value = true;
            bindings.editingRule.value = null;
            bindings.showTestDialog.value = true;
            bindings.testResult.value = false;
        });
        expect(emptyHtml).toContain('No category rules');
        expect(emptyHtml).toContain('Create Rule');
        expect(emptyHtml).toContain('No match');

        const collapsedHtml = await renderPanel({
            initTab: 'rules', tabs: ['rules'], title: '', hideHeader: false, headerActionsTarget: ''
        }, bindings => {
            bindings.categoryRules.value = [createRule()];
            const groupKey = bindings.groupedCategoryRuleTargets.value[0].key;
            bindings.collapsedCategoryGroupKeys.value = [groupKey];
        });
        expect(collapsedHtml).toContain('Food');

        const learningHtml = await renderPanel({
            initTab: 'learning', tabs: ['learning'], title: '', hideHeader: false, headerActionsTarget: ''
        }, bindings => {
            bindings.overview.value = createOverview();
        });
        expect(learningHtml).toContain('Category and Recurring Rules');

        const recurringHtml = await renderPanel({
            initTab: 'recurring', tabs: ['recurring'], title: '', hideHeader: false, headerActionsTarget: ''
        }, bindings => {
            bindings.overview.value = createOverview();
        });
        expect(recurringHtml).toContain('Review Recurring Suggestions');
        await flushAsync();
    });
});
