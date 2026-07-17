import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockRouterReplace = jest.fn<(...args: any[]) => Promise<void>>();
const mockRoute = actualVue.reactive({
    query: { keep: 'preserved', arrayValue: ['ignored'] as string[] },
});
const mockDisplay = { mdAndUp: actualVue.ref(true) };
const mockMatchingStore: any = actualVue.reactive({
    pairs: [] as any[],
    loading: false,
    error: null as string | null,
    loadPairs: jest.fn<() => Promise<void>>(),
    deletePair: jest.fn<(pairId: number) => Promise<boolean>>(),
});
const capturedTemplateHandlers: Array<{
    component: string;
    attribute: string;
    handler: (...args: any[]) => unknown;
}> = [];

jest.mock('vue-router', () => ({
    useRoute: () => mockRoute,
    useRouter: () => ({ replace: mockRouterReplace }),
}));

jest.mock('vuetify', () => ({
    useDisplay: () => mockDisplay,
}));

jest.mock('@/stores/matching.ts', () => ({
    useMatchingStore: () => mockMatchingStore,
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => key }),
}));

function createCaptureComponent(name: string): Record<string, unknown> {
    return {
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [attribute, value] of Object.entries(attrs)) {
                if (attribute.startsWith('on') && typeof value === 'function') {
                    capturedTemplateHandlers.push({
                        component: name,
                        attribute,
                        handler: value as (...args: any[]) => unknown,
                    });
                }
            }

            return () => actualVue.h(
                'section',
                { 'data-stub': name, ...attrs },
                Object.entries(slots).flatMap(([slotName, slot]) => {
                    if (typeof slot !== 'function') return [];
                    try {
                        return (slot as (props?: any) => unknown[])({
                            name: slotName,
                            props: { role: 'button' },
                            item: { enabled: true },
                        });
                    } catch {
                        return [];
                    }
                }),
            );
        },
    };
}

for (const [componentPath, name] of [
    ['@/views/desktop/pairingcenter/components/AccountRulePanel.vue', 'AccountRulePanelStub'],
    ['@/views/desktop/pairingcenter/components/LearningCenterPanel.vue', 'LearningCenterPanelStub'],
    ['@/views/desktop/pairingcenter/components/OcrConfigPanel.vue', 'OcrConfigPanelStub'],
    ['@/views/desktop/pairingcenter/components/PairsOverviewTable.vue', 'PairsOverviewTableStub'],
    ['@/views/desktop/pairingcenter/components/RuleCenterPanel.vue', 'RuleCenterPanelStub'],
] as const) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: createCaptureComponent(name),
    }));
}

const ListPage = require('@/views/desktop/pairingcenter/ListPage.vue').default as any;

function createPair(id: number, pairType: string): any {
    return {
        id,
        pairType,
        source: 'manual',
        leftBillId: id * 10,
        rightBillId: id * 10 + 1,
        leftBill: null,
        rightBill: null,
        createdAt: '2026-07-15T00:00:00Z',
    };
}

function setupPage(
    props: Record<string, unknown> = {},
    reactiveProps = false,
): { bindings: any; props: any } {
    const baseProps = {
        initDomain: undefined,
        initTab: undefined,
        ...props,
    };
    const runtimeProps = reactiveProps ? actualVue.reactive(baseProps) : baseProps;
    return {
        bindings: ListPage.setup(runtimeProps, { expose: jest.fn() }),
        props: runtimeProps,
    };
}

async function flushAsync(): Promise<void> {
    await Promise.resolve();
    await new Promise(resolve => setImmediate(resolve));
    await actualVue.nextTick();
}

async function renderPage(
    props: Record<string, unknown>,
    mutate: (bindings: any) => void,
): Promise<string> {
    const { createSSRApp, defineComponent } = actualVue;
    const { renderToString } = require('vue/server-renderer') as any;
    const RuntimePage = {
        ...ListPage,
        setup(runtimeProps: any, context: any) {
            const bindings = ListPage.setup(runtimeProps, context);
            mutate(bindings);
            return bindings;
        },
    };
    const app = createSSRApp(RuntimePage, {
        initDomain: undefined,
        initTab: undefined,
        ...props,
    });
    const UiStub = defineComponent(createCaptureComponent('VuetifyStub'));

    for (const name of [
        'v-row', 'v-col', 'v-card', 'v-layout', 'v-navigation-drawer', 'v-divider',
        'v-tabs', 'v-tab', 'v-main', 'v-btn', 'v-icon', 'v-progress-circular',
        'v-tooltip', 'v-spacer', 'v-chip', 'v-card-text', 'v-progress-linear',
        'v-alert', 'v-dialog', 'v-card-title', 'v-card-actions', 'btn-vertical-group',
    ]) {
        app.component(name, UiStub);
    }
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

async function invokeAllTemplateHandlers(): Promise<void> {
    const values: unknown[] = [
        undefined,
        true,
        false,
        'pairing-overview',
        'rule-config',
        'learning',
        'llm',
        'transfer-overview',
        'category-recognition',
        'missing',
        { key: 'Enter', stopPropagation: jest.fn(), preventDefault: jest.fn() },
    ];

    for (const { handler } of [...capturedTemplateHandlers]) {
        for (const value of values) {
            try {
                await handler(value);
            } catch {
                // Generated event and v-model wrappers intentionally accept different value shapes.
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    capturedTemplateHandlers.length = 0;
    mockRoute.query = { keep: 'preserved', arrayValue: ['ignored'] };
    mockDisplay.mdAndUp.value = true;
    mockMatchingStore.pairs = [];
    mockMatchingStore.loading = false;
    mockMatchingStore.error = null;
    mockMatchingStore.loadPairs.mockResolvedValue(undefined);
    mockMatchingStore.deletePair.mockResolvedValue(true);
    mockRouterReplace.mockResolvedValue(undefined);
});

describe('pairing center ListPage production-loaded navigation', () => {
    test('initializes transfer overview, loads empty pairs, and preserves unrelated query state', async () => {
        const { bindings } = setupPage();
        await flushAsync();

        expect(bindings.activeDomain.value).toBe('transfer');
        expect(bindings.activeTab.value).toBe('overview');
        expect(bindings.activeRuleConfigTab.value).toBe('rules');
        expect(bindings.activePrimary.value).toBe('pairing-overview');
        expect(bindings.activeSecondary.value).toBe('transfer-overview');
        expect(bindings.currentPageTitle.value).toBe('Transfer Pairing');
        expect(bindings.primaryNavButtons.value.map((item: any) => item.value)).toStrictEqual([
            'pairing-overview', 'rule-config', 'learning', 'llm',
        ]);
        expect(bindings.secondaryTabs.value.map((item: any) => item.value)).toStrictEqual([
            'transfer-overview', 'duplicate-overview',
        ]);
        expect(mockMatchingStore.loadPairs).toHaveBeenCalledTimes(1);

        bindings.selectSecondaryNav('duplicate-overview');
        await flushAsync();
        expect(bindings.activeDomain.value).toBe('duplicate');
        expect(bindings.activeSecondary.value).toBe('duplicate-overview');
        expect(mockRouterReplace).toHaveBeenLastCalledWith({
            path: '/pairing/list',
            query: { keep: 'preserved', domain: 'duplicate', tab: 'overview' },
        });
    });

    test('maps every primary and secondary destination to its panel selection', async () => {
        const { bindings } = setupPage({ initDomain: 'duplicate', initTab: 'overview' });
        await flushAsync();

        bindings.selectPrimaryNav('rule-config');
        expect(bindings.activePrimary.value).toBe('rule-config');
        expect(bindings.activeSecondary.value).toBe('category-recognition');
        expect(bindings.secondaryTabs.value.map((item: any) => item.value)).toStrictEqual([
            'category-recognition', 'account-recognition', 'recurring-recognition',
        ]);

        bindings.selectSecondaryNav('account-recognition');
        expect(bindings.activeSecondary.value).toBe('account-recognition');
        expect(bindings.currentPageTitle.value).toBe('Account Recognition');
        bindings.selectSecondaryNav('recurring-recognition');
        expect(bindings.activeSecondary.value).toBe('recurring-recognition');
        bindings.selectSecondaryNav('category-recognition');
        expect(bindings.activeSecondary.value).toBe('category-recognition');

        bindings.selectPrimaryNav('learning');
        expect(bindings.activePrimary.value).toBe('learning');
        expect(bindings.activeSecondary.value).toBe('learning-overview');
        expect(bindings.secondaryTabs.value).toHaveLength(2);
        bindings.selectSecondaryNav('learning-rules');
        expect(bindings.activeSecondary.value).toBe('learning-rules');
        bindings.selectPrimaryNav('learning');
        expect(bindings.activeSecondary.value).toBe('learning-rules');

        bindings.selectPrimaryNav('llm');
        expect(bindings.activePrimary.value).toBe('llm');
        expect(bindings.activeSecondary.value).toBe('llm-recognition');
        expect(bindings.secondaryTabs.value).toHaveLength(3);
        bindings.selectSecondaryNav('llm-config');
        expect(bindings.activeSecondary.value).toBe('llm-config');
        bindings.selectSecondaryNav('ocr-config');
        expect(bindings.activeSecondary.value).toBe('ocr-config');
        bindings.selectSecondaryNav('llm-recognition');
        expect(bindings.activeSecondary.value).toBe('llm-recognition');

        const priorDomain = bindings.activeDomain.value;
        bindings.selectSecondaryNav('missing');
        expect(bindings.activeDomain.value).toBe(priorDomain);
    });

    test('keeps unchanged selection stable and collapses navigation only on mobile', async () => {
        const { bindings } = setupPage();
        await flushAsync();
        mockRouterReplace.mockClear();
        mockMatchingStore.loadPairs.mockClear();

        bindings.selectSecondaryNav('transfer-overview');
        expect(mockRouterReplace).not.toHaveBeenCalled();
        expect(mockMatchingStore.loadPairs).not.toHaveBeenCalled();

        mockDisplay.mdAndUp.value = false;
        await flushAsync();
        expect(bindings.alwaysShowNav.value).toBe(false);
        bindings.showNav.value = true;
        bindings.selectPrimaryNav('learning');
        expect(bindings.showNav.value).toBe(false);

        bindings.showNav.value = false;
        mockDisplay.mdAndUp.value = true;
        await flushAsync();
        expect(bindings.alwaysShowNav.value).toBe(true);
        expect(bindings.showNav.value).toBe(true);
        mockDisplay.mdAndUp.value = false;
        await flushAsync();
        bindings.showNav.value = true;
        mockDisplay.mdAndUp.value = true;
        await flushAsync();
        expect(bindings.showNav.value).toBe(true);
    });

    test('reacts to route-prop changes and normalizes unsupported domains and tabs', async () => {
        const { bindings, props } = setupPage({ initDomain: 'llm', initTab: 'config' }, true);
        await flushAsync();
        expect(bindings.activeSecondary.value).toBe('llm-config');

        props.initTab = 'ocr-config';
        await flushAsync();
        expect(bindings.activeSecondary.value).toBe('ocr-config');

        props.initDomain = 'investment';
        props.initTab = 'rules';
        await flushAsync();
        expect(bindings.activeDomain.value).toBe('transfer');
        expect(bindings.activeTab.value).toBe('overview');

        props.initDomain = 'duplicate';
        props.initTab = 'rules';
        await flushAsync();
        expect(bindings.activeDomain.value).toBe('duplicate');
        expect(bindings.activeTab.value).toBe('overview');
    });
});

describe('pairing center ListPage production-loaded panels and pairs', () => {
    test('filters transfer and duplicate-family pairs and projects loading, error, and toolbar state', async () => {
        mockMatchingStore.pairs = [
            createPair(1, 'transfer'),
            createPair(2, 'duplicate'),
            createPair(3, 'reconciliation_duplicate'),
            createPair(4, 'investment'),
        ];
        mockMatchingStore.loading = true;
        mockMatchingStore.error = 'pair load failed';
        const { bindings } = setupPage();

        expect(bindings.filteredPairs.value.map((pair: any) => pair.id)).toStrictEqual([1]);
        expect(bindings.isPairingOverview.value).toBe(true);
        expect(bindings.showPairsLoading.value).toBe(true);
        expect(bindings.showPairsError.value).toBe(true);
        expect(bindings.showHeaderRefresh.value).toBe(true);
        expect(bindings.activeToolbarRefreshing.value).toBe(true);

        bindings.error.value = null;
        expect(mockMatchingStore.error).toBeNull();
        bindings.selectSecondaryNav('duplicate-overview');
        expect(bindings.filteredPairs.value.map((pair: any) => pair.id)).toStrictEqual([2, 3]);
    });

    test('refreshes only the active overview or rule panel and always clears panel loading', async () => {
        const { bindings } = setupPage();
        await flushAsync();
        mockMatchingStore.loadPairs.mockClear();

        await bindings.refreshActiveView();
        expect(mockMatchingStore.loadPairs).toHaveBeenCalledTimes(1);

        const categoryRefresh = jest.fn<() => Promise<void>>().mockResolvedValue(undefined);
        const accountRefresh = jest.fn<() => Promise<void>>().mockResolvedValue(undefined);
        const recurringRefresh = jest.fn<() => Promise<void>>().mockResolvedValue(undefined);
        bindings.categoryRulePanel.value = { refresh: categoryRefresh };
        bindings.accountRulePanel.value = { refresh: accountRefresh };
        bindings.recurringRulePanel.value = { refresh: recurringRefresh };

        bindings.selectPrimaryNav('rule-config');
        await bindings.refreshActiveView();
        expect(categoryRefresh).toHaveBeenCalled();
        expect(bindings.rulePanelRefreshing.value).toBe(false);

        bindings.selectSecondaryNav('account-recognition');
        await bindings.refreshActiveView();
        expect(accountRefresh).toHaveBeenCalled();

        bindings.selectSecondaryNav('recurring-recognition');
        await bindings.refreshActiveView();
        expect(recurringRefresh).toHaveBeenCalled();
        expect(bindings.showHeaderRefresh.value).toBe(true);
        expect(bindings.activeToolbarRefreshing.value).toBe(false);

        recurringRefresh.mockRejectedValueOnce(new Error('recurring unavailable'));
        await expect(bindings.refreshActiveView()).rejects.toThrow('recurring unavailable');
        expect(bindings.rulePanelRefreshing.value).toBe(false);

        bindings.recurringRulePanel.value = null;
        await bindings.refreshActiveView();
        bindings.selectPrimaryNav('learning');
        await bindings.refreshActiveView();
        expect(bindings.canRefreshActiveView.value).toBe(false);
        expect(bindings.showHeaderRefresh.value).toBe(false);
    });

    test('opens delete confirmation, ignores an empty selection, and deletes the selected pair', async () => {
        const { bindings } = setupPage();
        await flushAsync();

        await bindings.doDeletePair();
        expect(mockMatchingStore.deletePair).not.toHaveBeenCalled();

        const pair = createPair(42, 'transfer');
        bindings.confirmDeletePair(pair);
        expect(bindings.pairToDelete.value).toStrictEqual(pair);
        expect(bindings.showDeleteDialog.value).toBe(true);
        await bindings.doDeletePair();
        expect(mockMatchingStore.deletePair).toHaveBeenCalledWith(42);
        expect(bindings.deleting.value).toBeNull();
        expect(bindings.showDeleteDialog.value).toBe(false);
        expect(bindings.pairToDelete.value).toBeNull();
    });
});

describe('pairing center ListPage production template', () => {
    test('renders overview loading/error/populated states and executes template wrappers', async () => {
        const html = await renderPage({ initDomain: 'transfer', initTab: 'overview' }, bindings => {
            mockMatchingStore.pairs = [createPair(1, 'transfer')];
            mockMatchingStore.loading = true;
            mockMatchingStore.error = 'visible pair error';
            bindings.showDeleteDialog.value = true;
            bindings.pairToDelete.value = mockMatchingStore.pairs[0];
            bindings.deleting.value = 1;
        });

        expect(html).toContain('Transfer Pairing');
        expect(html).toContain('visible pair error');
        expect(html).toContain('1 pairs');
        expect(html).toContain('Delete Pair');
        expect(capturedTemplateHandlers.length).toBeGreaterThan(0);
        await invokeAllTemplateHandlers();
    });

    test.each([
        ['transfer', 'rules', 'Category Recognition', 'RuleCenterPanelStub'],
        ['transfer', 'accounts', 'Account Recognition', 'AccountRulePanelStub'],
        ['transfer', 'recurring', 'Recurring Recognition', 'RuleCenterPanelStub'],
        ['learning', 'overview', 'Auto Suggestions', 'LearningCenterPanelStub'],
        ['learning', 'rules', 'Learning Rules', 'LearningCenterPanelStub'],
        ['llm', 'overview', 'LLM Recognition', 'LearningCenterPanelStub'],
        ['llm', 'config', 'LLM Config', 'LearningCenterPanelStub'],
        ['llm', 'ocr-config', 'OCR Config', 'OcrConfigPanelStub'],
    ])('renders %s/%s panel composition', async (domain, tab, title, stubName) => {
        const html = await renderPage({ initDomain: domain, initTab: tab }, bindings => {
            bindings.rulePanelRefreshing.value = true;
        });

        expect(html).toContain(title);
        expect(html).toContain(stubName);
        await invokeAllTemplateHandlers();
    });

    test('renders duplicate empty/error-free state and mobile navigation control', async () => {
        mockDisplay.mdAndUp.value = false;
        const html = await renderPage({ initDomain: 'duplicate', initTab: 'overview' }, bindings => {
            mockMatchingStore.pairs = [];
            mockMatchingStore.loading = false;
            mockMatchingStore.error = null;
            bindings.showNav.value = false;
        });

        expect(html).toContain('Duplicate Pairing');
        expect(html).toContain('0 pairs');
        expect(html).toContain('PairsOverviewTableStub');
        await invokeAllTemplateHandlers();
    });
});
