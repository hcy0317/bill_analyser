import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const push = jest.fn();
const replace = jest.fn();
const mockRoute = actualVue.reactive({ query: {} as Record<string, unknown> });
const navigate = jest.fn();
const load = jest.fn<() => Promise<boolean>>();
const detectRecurring = jest.fn<() => Promise<boolean>>();
const acceptRecurring = jest.fn<(...args: any[]) => Promise<boolean>>();
const rejectRecurring = jest.fn<(...args: any[]) => Promise<boolean>>();
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

const anomaly = {
    key: 'large_transaction:41',
    type: 'large_transaction' as const,
    severity: 'warning' as const,
    billIds: [41],
    amountCents: 12_345,
    occurredOn: '2026-08-12',
    category: 'Dining',
    counterparty: '',
    description: 'Coffee',
    message: ''
};

const duplicateAnomaly = {
    ...anomaly,
    key: 'duplicate_charge:42:43',
    type: 'duplicate_charge' as const,
    severity: 'error' as const,
    billIds: [42, 43],
    counterparty: 'Coffee Shop',
    description: ''
};

const spikeAnomaly = {
    ...anomaly,
    key: 'category_spike:2026-08:Transport',
    type: 'category_spike' as const,
    severity: 'info' as const,
    billIds: [],
    amountCents: 88_000,
    occurredOn: '2026-08',
    category: 'Transport',
    description: ''
};

const recurringSuggestion = {
    id: 7,
    status: 'pending',
    name: 'Monthly Coffee',
    counterparty: 'Coffee Shop',
    description: 'Subscription',
    amountCents: 2_500,
    confidenceScore: 0.876,
    suggestedNextDate: '2026-09-01'
};

const controller = {
    months: actualVue.ref(6),
    loading: actualVue.ref(false),
    detecting: actualVue.ref(false),
    mutatingSuggestionId: actualVue.ref(null as number | null),
    error: actualVue.ref(null as string | null),
    detectResult: actualVue.ref(null),
    anomalyData: actualVue.ref({
        items: [anomaly], totalCount: 1, analyzedBills: 10, analyzedMonths: 6,
        startDate: '2026-03-01', endDate: '2026-08-12'
    }),
    recurringSuggestions: actualVue.ref([] as any[]),
    recurringHistory: actualVue.ref([] as any[]),
    summary: actualVue.computed(() => {
        const anomalyCount = controller.anomalyData.value.items.length;
        const recurringCount = controller.recurringSuggestions.value.length;
        return {
            total: anomalyCount + recurringCount,
            anomalyCount,
            recurringCount,
            hasWork: anomalyCount + recurringCount > 0
        };
    }),
    load,
    detectRecurring,
    acceptRecurring,
    rejectRecurring
};

jest.mock('vue-router', () => ({
    useRoute: () => mockRoute,
    useRouter: () => ({ push, replace })
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        formatAmountToLocalizedNumerals: (value: number) => `amount:${value.toFixed(2)}`
    })
}));
jest.mock('@/views/base/action-center/useActionCenter.ts', () => ({
    useActionCenter: () => controller
}));

const DesktopPage = require('@/views/desktop/insights/InsightsPage.vue').default as any;
const MobilePage = require('@/views/mobile/action-center/ActionCenterPage.vue').default as any;

beforeEach(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
    jest.clearAllMocks();
    controller.months.value = 6;
    controller.loading.value = false;
    controller.detecting.value = false;
    controller.mutatingSuggestionId.value = null;
    controller.error.value = null;
    controller.detectResult.value = null;
    controller.anomalyData.value = {
        items: [anomaly], totalCount: 1, analyzedBills: 10, analyzedMonths: 6,
        startDate: '2026-03-01', endDate: '2026-08-12'
    };
    controller.recurringSuggestions.value = [];
    controller.recurringHistory.value = [];
    mockRoute.query = {};
    load.mockResolvedValue(true);
    detectRecurring.mockResolvedValue(true);
    acceptRecurring.mockResolvedValue(true);
    rejectRecurring.mockResolvedValue(true);
});

afterEach(() => {
    consoleWarnSpy.mockRestore();
});

function renderAndCollectHandlers(component: any, bindings: any, props: Record<string, unknown> = {}): {
    vnode: any;
    handlers: Array<(value?: unknown) => unknown>;
} {
    const handlers: Array<(value?: unknown) => unknown> = [];
    const seen = new Set<any>();
    const visit = (node: any): void => {
        if (node == null) return;
        if (Array.isArray(node)) {
            node.forEach(visit);
            return;
        }
        if (typeof node !== 'object' || seen.has(node)) return;
        seen.add(node);
        for (const [name, value] of Object.entries(node.props ?? {})) {
            if (!name.startsWith('on')) continue;
            for (const candidate of Array.isArray(value) ? value : [value]) {
                if (typeof candidate === 'function') handlers.push(candidate as (value?: unknown) => unknown);
            }
        }
        if (Array.isArray(node.children)) {
            node.children.forEach(visit);
        } else if (node.children && typeof node.children === 'object') {
            for (const slot of Object.values(node.children)) {
                if (typeof slot === 'function') visit((slot as (scope?: unknown) => unknown)({}));
            }
        }
    };
    const exposed = actualVue.proxyRefs(bindings);
    const vnode = component.render({}, [], props, exposed, {}, {});
    visit(vnode);
    return { vnode, handlers };
}

async function renderDesktopWithExpandedSlots(): Promise<Array<(value?: unknown) => unknown>> {
    const { createSSRApp, defineComponent, h } = actualVue;
    const { renderToString } = jest.requireActual('vue/server-renderer') as any;
    const handlers: Array<(value?: unknown) => unknown> = [];
    const Stub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => {
            for (const [name, value] of Object.entries(attrs)) {
                if (!name.startsWith('on')) continue;
                for (const candidate of Array.isArray(value) ? value : [value]) {
                    if (typeof candidate === 'function') handlers.push(candidate as (value?: unknown) => unknown);
                }
            }
            return h('div', attrs, Object.values(slots).flatMap((slot: any) => slot?.({}) ?? []));
        }
    });
    const app = createSSRApp(DesktopPage);
    for (const name of [
        'v-row', 'v-col', 'v-card', 'v-card-title', 'v-card-text', 'v-card-actions', 'v-icon', 'v-chip', 'v-spacer',
        'v-select', 'v-btn', 'v-progress-linear', 'v-alert', 'v-list', 'v-divider',
        'v-list-item', 'v-avatar', 'v-list-item-title', 'v-list-item-subtitle',
        'v-tabs', 'v-tab', 'v-tabs-window', 'v-tabs-window-item', 'v-empty-state'
    ]) app.component(name, Stub);
    app.config.warnHandler = () => undefined;
    await renderToString(app);
    return handlers;
}

describe('action center desktop and mobile pages', () => {
    test('desktop formats cents and navigates to the supported transaction-list query', () => {
        const bindings = DesktopPage.setup({}, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });

        expect(bindings.formatAmount(12_345)).toBe('amount:123.45');
        expect(bindings.anomalyContext(anomaly)).toBe('Coffee');
        bindings.reviewAnomaly(anomaly);
        expect(push).toHaveBeenCalledWith({
            path: '/transaction/list',
            query: { keyword: 'Coffee' }
        });
    });

    test('desktop opens legacy recurring discovery links on the history view', () => {
        mockRoute.query = { recurring: 'history' };
        controller.recurringHistory.value = [{ ...recurringSuggestion, status: 'accepted' }];

        const bindings = DesktopPage.setup({}, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });

        expect(bindings.recurringView.value).toBe('history');
        expect(bindings.recurringHistory.value).toEqual([
            expect.objectContaining({ id: recurringSuggestion.id, status: 'accepted' })
        ]);
    });

    test('desktop recurring tabs keep the canonical query in sync without losing other filters', async () => {
        mockRoute.query = { source: 'direct' };
        const bindings = DesktopPage.setup({}, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });

        bindings.recurringView.value = 'history';
        await actualVue.nextTick();
        expect(replace).toHaveBeenLastCalledWith({
            query: { source: 'direct', recurring: 'history' }
        });

        mockRoute.query = { source: 'legacy', recurring: 'history' };
        await actualVue.nextTick();
        bindings.recurringView.value = 'pending';
        await actualVue.nextTick();
        expect(replace).toHaveBeenLastCalledWith({ query: { source: 'legacy' } });
    });

    test('mobile exposes pending and terminal recurring views', () => {
        controller.recurringSuggestions.value = [recurringSuggestion];
        controller.recurringHistory.value = [
            { ...recurringSuggestion, id: 8, status: 'accepted' },
            { ...recurringSuggestion, id: 9, status: 'rejected' }
        ];
        const bindings = MobilePage.setup({ f7router: { navigate } }, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });

        expect(bindings.recurringView.value).toBe('pending');
        bindings.recurringView.value = 'history';
        expect(bindings.recurringStatusLabel(controller.recurringHistory.value[0])).toBe('tt:Accepted');
        expect(bindings.recurringStatusLabel(controller.recurringHistory.value[1])).toBe('tt:Rejected');
        expect(bindings.recurringStatusLabel({ ...recurringSuggestion, status: 'archived' })).toBe('');
        expect(renderAndCollectHandlers(
            MobilePage,
            bindings,
            { f7router: { navigate } }
        ).vnode).toBeTruthy();
    });

    test('mobile refresh completes the pull-to-refresh callback and uses its own route syntax', async () => {
        const bindings = MobilePage.setup({ f7router: { navigate } }, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });
        const done = jest.fn();

        await bindings.refresh(done);
        expect(load).toHaveBeenCalledTimes(1);
        expect(done).toHaveBeenCalledTimes(1);
        expect(bindings.anomalyDetails(anomaly)).toBe('2026-08-12 · amount:123.45');

        bindings.reviewAnomaly(anomaly);
        expect(navigate).toHaveBeenCalledWith('/transaction/list?keyword=Coffee');
    });

    test('formats every anomaly, severity, and recurring-detail branch', () => {
        const desktop = DesktopPage.setup({}, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });
        const mobile = MobilePage.setup({ f7router: { navigate } }, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });

        expect(['error', 'warning', 'info'].map(desktop.severityColor)).toEqual(['error', 'warning', 'info']);
        expect([
            desktop.anomalyIcon(anomaly.type),
            desktop.anomalyIcon(duplicateAnomaly.type),
            desktop.anomalyIcon(spikeAnomaly.type)
        ]).toEqual(expect.arrayContaining([expect.any(String), expect.any(String), expect.any(String)]));
        expect([
            desktop.anomalyTitle(anomaly.type),
            desktop.anomalyTitle(duplicateAnomaly.type),
            desktop.anomalyTitle(spikeAnomaly.type)
        ]).toEqual(['Large Transaction', 'Possible Duplicate', 'Category Spike']);
        expect(desktop.anomalyContext(duplicateAnomaly)).toBe('Coffee Shop');
        expect(desktop.anomalyContext({ ...anomaly, description: '', category: 'Dining' })).toBe('Dining');
        expect(desktop.anomalyContext({ ...anomaly, description: '', category: '' })).toBe('tt:Transaction');
        expect(desktop.formatPercent(0.876)).toBe('88%');
        expect(desktop.monthOptions.value.map((item: any) => item.value)).toEqual([3, 6, 12]);

        expect([
            mobile.anomalyIcon(anomaly.type),
            mobile.anomalyIcon(duplicateAnomaly.type),
            mobile.anomalyIcon(spikeAnomaly.type)
        ]).toEqual(['exclamationmark_circle', 'doc_on_doc', 'chart_bar_alt_fill']);
        expect([
            mobile.anomalyTitle(anomaly.type),
            mobile.anomalyTitle(duplicateAnomaly.type),
            mobile.anomalyTitle(spikeAnomaly.type)
        ]).toEqual(['Large Transaction', 'Possible Duplicate', 'Category Spike']);
        expect(mobile.anomalyContext({ ...anomaly, description: '', category: '' })).toBe('tt:Transaction');
        expect(mobile.anomalyDetails({ ...anomaly, occurredOn: '', amountCents: 0 })).toBe('');
        expect(mobile.recurringDetails(recurringSuggestion)).toBe(
            'amount:25.00 · tt:Confidence 88% · tt:Next Date: 2026-09-01'
        );
        expect(mobile.recurringDetails({ ...recurringSuggestion, suggestedNextDate: '' })).toBe(
            'amount:25.00 · tt:Confidence 88%'
        );
    });

    test('renders populated, empty, busy, error, and detection states and wires page actions', async () => {
        controller.anomalyData.value = {
            items: [anomaly, duplicateAnomaly, spikeAnomaly],
            totalCount: 3,
            analyzedBills: 10,
            analyzedMonths: 6,
            startDate: '2026-03-01',
            endDate: '2026-08-12'
        };
        controller.recurringSuggestions.value = [recurringSuggestion];
        controller.error.value = 'Failed to load action center';
        controller.detectResult.value = { detected: 2, created: 1, updated: 1, skipped: 0 };
        controller.mutatingSuggestionId.value = recurringSuggestion.id;

        const desktopBindings = DesktopPage.setup({}, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });
        const mobileBindings = MobilePage.setup({ f7router: { navigate } }, {
            attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
        });
        const populatedDesktop = renderAndCollectHandlers(DesktopPage, desktopBindings);
        const populatedMobile = renderAndCollectHandlers(
            MobilePage,
            mobileBindings,
            { f7router: { navigate } }
        );
        expect(populatedDesktop.vnode).toBeTruthy();
        expect(populatedMobile.vnode).toBeTruthy();
        expect(populatedDesktop.handlers.length).toBeGreaterThanOrEqual(3);
        expect(populatedMobile.handlers.length).toBeGreaterThan(4);

        const expandedDesktopHandlers = await renderDesktopWithExpandedSlots();
        expect(expandedDesktopHandlers.length).toBeGreaterThan(4);

        await Promise.allSettled([
            ...populatedDesktop.handlers.map(handler => Promise.resolve(handler())),
            ...populatedMobile.handlers.map(handler => Promise.resolve(handler())),
            ...expandedDesktopHandlers.map(handler => Promise.resolve(handler()))
        ]);
        expect(load).toHaveBeenCalled();
        expect(detectRecurring).toHaveBeenCalled();
        expect(acceptRecurring).toHaveBeenCalledWith(recurringSuggestion.id);
        expect(rejectRecurring).toHaveBeenCalledWith(recurringSuggestion.id);

        controller.anomalyData.value = {
            items: [], totalCount: 0, analyzedBills: 0, analyzedMonths: 6,
            startDate: '', endDate: ''
        };
        controller.recurringSuggestions.value = [];
        controller.error.value = null;
        controller.detectResult.value = null;
        controller.mutatingSuggestionId.value = null;
        expect(renderAndCollectHandlers(DesktopPage, desktopBindings).vnode).toBeTruthy();
        expect(renderAndCollectHandlers(MobilePage, mobileBindings, { f7router: { navigate } }).vnode).toBeTruthy();

        controller.loading.value = true;
        controller.detecting.value = true;
        expect(renderAndCollectHandlers(DesktopPage, desktopBindings).vnode).toBeTruthy();
        expect(renderAndCollectHandlers(MobilePage, mobileBindings, { f7router: { navigate } }).vnode).toBeTruthy();

        controller.months.value = 12;
        await actualVue.nextTick();
        expect(load).toHaveBeenCalled();
    });
});
