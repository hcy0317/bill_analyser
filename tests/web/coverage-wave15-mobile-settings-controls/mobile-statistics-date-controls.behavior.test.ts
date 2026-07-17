/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const { proxyRefs, reactive } = actualVue;

const AnalysisType = {
    CategoricalAnalysis: 1,
    TrendAnalysis: 2,
    AssetTrends: 3,
} as const;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/core/statistics.ts', () => ({ StatisticsAnalysisType: AnalysisType }));

const MobileStatisticsDateControls = require(
    '@/views/mobile/statistics/components/MobileStatisticsDateControls.vue'
).default as any;

function props(overrides: Record<string, unknown> = {}): any {
    return {
        showDatePopover: true,
        showDateAggregationPopover: true,
        showCustomDateRangeSheet: true,
        showCustomMonthRangeSheet: true,
        showMoreActionSheet: true,
        analysisType: AnalysisType.TrendAnalysis,
        reloading: false,
        query: {
            categoricalChartStartTime: 100,
            categoricalChartEndTime: 200,
            trendChartStartYearMonth: '2026-01',
            trendChartEndYearMonth: '2026-06',
            keyword: 'coffee',
        },
        queryDateType: 99,
        queryStartTime: '2026-01-01',
        queryEndTime: '2026-01-31',
        allDateRanges: [
            { type: 1, displayName: 'Current', isUserCustomRange: false },
            { type: 99, displayName: 'Custom', isUserCustomRange: true },
        ],
        trendDateAggregationType: 10,
        assetTrendsDateAggregationType: 20,
        allTrendAnalysisDateAggregationTypes: [
            { type: 10, displayName: 'Month' },
            { type: 11, displayName: 'Year' },
        ],
        allAssetTrendsDateAggregationTypes: [
            { type: 20, displayName: 'Day' },
            { type: 21, displayName: 'Week' },
        ],
        canUseCategoryFilter: true,
        canUseTagFilter: true,
        canUseKeywordFilter: true,
        canShowCustomDateRange: jest.fn(() => true),
        ...overrides,
    };
}

function setup(componentProps = props()): { bindings: any; emit: jest.Mock; props: any } {
    const emit = jest.fn();
    const reactiveProps = reactive(componentProps);
    const bindings = MobileStatisticsDateControls.setup(reactiveProps, {
        attrs: {}, slots: {}, emit, expose: jest.fn(),
    });
    return { bindings, emit, props: reactiveProps };
}

function render(bindings: any, componentProps: any): any {
    const exposed = proxyRefs(bindings);
    return MobileStatisticsDateControls.render(
        { ...componentProps, ...exposed }, [], componentProps, exposed, {}, {},
    );
}

function collectCallbacks(value: any, callbacks: Array<{ name: string; callback: (...args: any[]) => any }>, seen = new Set<any>()): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) return;
    seen.add(value);
    if (Array.isArray(value)) {
        for (const item of value) collectCallbacks(item, callbacks, seen);
        return;
    }
    if (value.props && typeof value.props === 'object') {
        for (const [name, callback] of Object.entries(value.props)) {
            if (name.startsWith('on') && typeof callback === 'function') {
                callbacks.push({ name, callback: callback as (...args: any[]) => any });
            }
        }
    }
    if (value.children && typeof value.children === 'object' && !Array.isArray(value.children)) {
        for (const slot of Object.values(value.children)) {
            if (typeof slot === 'function') {
                collectCallbacks((slot as (...args: any[]) => any)({}), callbacks, seen);
            } else {
                collectCallbacks(slot, callbacks, seen);
            }
        }
    } else {
        collectCallbacks(value.children, callbacks, seen);
    }
}

let warnSpy: jest.SpiedFunction<typeof console.warn>;

beforeAll(() => {
    warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    warnSpy.mockRestore();
});

beforeEach(() => {
    jest.clearAllMocks();
});

describe('MobileStatisticsDateControls production behavior', () => {
    test('emits custom ranges and all trend/date/filter interactions', () => {
        const page = setup();
        page.bindings.onCustomDateRangeChange(100, 200);
        expect(page.emit).toHaveBeenCalledWith('customDateFilter', 100, 200);

        const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
        collectCallbacks(render(page.bindings, page.props), callbacks);
        expect(callbacks.length).toBeGreaterThan(14);
        for (const entry of callbacks) {
            if (entry.name.toLowerCase().includes('daterange:change')) {
                entry.callback('2026-02', '2026-03');
            } else if (entry.name.toLowerCase().includes('update')) {
                entry.callback(false);
            } else {
                entry.callback({ $el: { id: 'popover' } });
            }
        }

        expect(page.emit).toHaveBeenCalledWith('dateFilter', 99);
        expect(page.emit).toHaveBeenCalledWith('trendDateAggregationType', 10);
        expect(page.emit).toHaveBeenCalledWith('filterAccounts');
        expect(page.emit).toHaveBeenCalledWith('filterCategories');
        expect(page.emit).toHaveBeenCalledWith('filterTags');
        expect(page.emit).toHaveBeenCalledWith('filterDescription');
        expect(page.emit).toHaveBeenCalledWith('settings');
    });

    test('renders asset aggregation and hides optional filters/custom range details', () => {
        const page = setup(props({
            analysisType: AnalysisType.AssetTrends,
            queryDateType: 1,
            reloading: true,
            canUseCategoryFilter: false,
            canUseTagFilter: false,
            canUseKeywordFilter: false,
            canShowCustomDateRange: jest.fn(() => false),
            query: {
                categoricalChartStartTime: 0,
                categoricalChartEndTime: 0,
                trendChartStartYearMonth: '2025-01',
                trendChartEndYearMonth: '2025-12',
                keyword: '',
            },
        }));
        const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
        collectCallbacks(render(page.bindings, page.props), callbacks);
        for (const entry of callbacks) {
            if (entry.name.toLowerCase().includes('update')) entry.callback(true);
            else entry.callback({});
        }
        expect(page.emit).toHaveBeenCalledWith('assetTrendsDateAggregationType', 20);
        expect(page.emit).not.toHaveBeenCalledWith('filterCategories');
        expect(page.emit).not.toHaveBeenCalledWith('filterTags');
        expect(page.emit).not.toHaveBeenCalledWith('filterDescription');
    });

    test('renders the non-aggregation categorical branch', () => {
        const page = setup(props({ analysisType: AnalysisType.CategoricalAnalysis }));
        expect(render(page.bindings, page.props)).toBeDefined();
    });
});
