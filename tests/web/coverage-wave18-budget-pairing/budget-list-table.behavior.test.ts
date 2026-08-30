import { describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/components/desktop/ItemIcon.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return {
        __esModule: true,
        default: defineComponent({
            name: 'BudgetItemIconStub',
            inheritAttrs: false,
            setup: (_props: unknown, { attrs }: any) => () => h('item-icon-stub', attrs)
        })
    };
});
jest.mock('@mdi/js', () => ({
    mdiAlertCircle: 'alert-circle',
    mdiAlertOctagon: 'alert-octagon',
    mdiChevronDown: 'chevron-down',
    mdiChevronRight: 'chevron-right',
    mdiDeleteOutline: 'delete',
    mdiPencilOutline: 'pencil',
    mdiPlusCircleOutline: 'plus',
    mdiWalletOutline: 'wallet',
    mdiChartTimelineVariant: 'chart-timeline',
    mdiInformationOutline: 'information',
    mdiTrendingDown: 'trending-down',
    mdiTrendingNeutral: 'trending-neutral',
    mdiTrendingUp: 'trending-up'
}));

import BudgetListTable from '@/views/desktop/budgets/components/BudgetListTable.vue';
import BudgetForecastPanel from '@/views/desktop/budgets/components/BudgetForecastPanel.vue';

const uiComponents = [
    'v-table', 'v-skeleton-loader', 'v-icon', 'v-btn', 'v-tooltip',
    'v-progress-circular', 'v-progress-linear', 'v-chip', 'v-card-text', 'v-empty-state'
];

describe('BudgetForecastPanel shared page states', () => {
    const formatAmount = jest.fn((amount: number) => `amount:${amount}`);
    const getForecastConfidenceLabel = jest.fn((confidence?: string | null) => confidence || 'none');
    const getForecastConfidenceClass = jest.fn((confidence?: string | null) => `confidence-${confidence || 'none'}`);
    const baseProps = {
        forecastLoading: false,
        currentForecast: null,
        displayForecasts: [],
        forecastMonthsHistory: 6,
        forecastRiskSummary: { lowConfidenceCount: 0, overBudgetCount: 0 },
        formatAmount,
        getForecastConfidenceLabel,
        getForecastConfidenceClass,
    };

    test('renders loading, empty, populated trend/status and summary variants', () => {
        for (const props of [
            { ...baseProps, forecastLoading: true },
            baseProps,
            {
                ...baseProps,
                currentForecast: {
                    forecasts: [],
                    periodStart: '2026-01-01',
                    periodEnd: '2026-01-31',
                    forecastStrategy: 'moving_average',
                    historyPeriods: 0,
                    avgBacktestMape: 12.5,
                },
                displayForecasts: [
                    {
                        categoryId: 'up', categoryName: 'Up', historicalAverageCents: 1000,
                        currentSpentCents: 2000, projectedTotalCents: 3000, budgetAmountCents: 2500,
                        trend: 'up', projectedOverBudget: true, confidence: 'low', backtestMape: 20,
                        strategyExplanation: 'trend', samplePeriods: 3,
                    },
                    {
                        categoryId: 'down', categoryName: 'Down', historicalAverageCents: 1000,
                        currentSpentCents: 500, projectedTotalCents: 800, budgetAmountCents: 2500,
                        trend: 'down', projectedOverBudget: false, confidence: null, backtestMape: null,
                        strategyExplanation: '', samplePeriods: null,
                    },
                    {
                        categoryId: 'neutral', categoryName: 'Neutral', historicalAverageCents: 1000,
                        currentSpentCents: 1000, projectedTotalCents: 1000, budgetAmountCents: 2500,
                        trend: 'neutral', projectedOverBudget: false, confidence: 'high', backtestMape: 0,
                        strategyExplanation: '', samplePeriods: 0,
                    },
                ],
                forecastRiskSummary: { lowConfidenceCount: 1, overBudgetCount: 1 },
            },
            {
                ...baseProps,
                currentForecast: {
                    forecasts: [], periodStart: '', periodEnd: '', forecastStrategy: 'historical_average',
                    historyPeriods: 2, avgBacktestMape: null,
                },
            },
        ]) {
            const mounted = mountWithHostRenderer(BudgetForecastPanel as any, props, uiComponents);
            mounted.app.unmount();
        }

        expect(formatAmount).toHaveBeenCalled();
        expect(getForecastConfidenceLabel).toHaveBeenCalledWith('low');
        expect(getForecastConfidenceLabel).toHaveBeenCalledWith('high');
        expect(getForecastConfidenceClass).toHaveBeenCalledWith('low');
    });
});

function createBudget(id: string, overrides: Record<string, unknown> = {}): any {
    const budget = {
        id,
        name: `Budget ${id}`,
        category: 'Food',
        categoryIcon: 'food-icon',
        categoryColor: '#112233',
        amountCents: 10_000,
        spentAmountCents: 4_000,
        executionRate: 40,
        executionRateText: '40.0%',
        amountInYuan: 100,
        spentAmountInYuan: 40,
        alertTriggered: false,
        isOverBudget: false,
        ...overrides
    };
    return budget;
}

function createGroup(category: string, overrides: Record<string, unknown> = {}): any {
    return {
        category,
        categoryIcon: `${category}-icon`,
        categoryColor: '#445566',
        primaryBudgets: [],
        subBudgets: [],
        totalAmountCents: 20_000,
        totalSpentCents: 8_000,
        subTotalAmountCents: 0,
        subTotalSpentCents: 0,
        primaryAmountCents: 20_000,
        primarySpentCents: 8_000,
        isCollapsed: false,
        ...overrides
    };
}

function createScenario(): {
    readonly budgets: any[];
    readonly groups: any[];
} {
    const warningBudget = createBudget('warning', {
        name: '',
        category: 'Dining',
        alertTriggered: true,
        executionRate: 85,
        executionRateText: '85.0%'
    });
    const overBudget = createBudget('over', {
        category: 'Travel',
        isOverBudget: true,
        alertTriggered: true,
        executionRate: 135,
        executionRateText: '135.0%',
        amountInYuan: 100,
        spentAmountInYuan: 135
    });
    const ordinaryBudget = createBudget('ordinary', {
        category: 'Travel',
        executionRate: 10,
        executionRateText: '10.0%'
    });

    return {
        budgets: [warningBudget, overBudget, ordinaryBudget],
        groups: [
            createGroup('Empty', { isCollapsed: true }),
            createGroup('Dining', {
                primaryBudgets: [warningBudget],
                subBudgets: [],
                isCollapsed: false
            }),
            createGroup('Travel', {
                primaryBudgets: [overBudget],
                subBudgets: [ordinaryBudget],
                isCollapsed: false
            })
        ]
    };
}

function createProps(overrides: Record<string, unknown> = {}): any {
    const scenario = createScenario();
    return {
        loading: false,
        updating: false,
        isDarkMode: false,
        filteredBudgets: scenario.budgets,
        groupedBudgets: scenario.groups,
        budgetRemoving: { warning: false, over: true, ordinary: false },
        groupHasExpandedRows: jest.fn((group: any) => !group.isCollapsed && group.primaryBudgets.length > 0),
        getPrimaryBudgetForHeader: jest.fn((group: any) => group.primaryBudgets[0] ?? null),
        getExpandedPrimaryBudgets: jest.fn((group: any) => {
            if (group.category === 'Dining') return [scenario.budgets[0]];
            if (group.category === 'Travel') return [scenario.budgets[1], scenario.budgets[2]];
            return [];
        }),
        getBudgetCategoryIcon: jest.fn((budget: any, group: any) => budget.categoryIcon || group.categoryIcon),
        getBudgetCategoryColor: jest.fn((budget: any, group: any) => budget.categoryColor || group.categoryColor),
        getGroupExecutionRate: jest.fn((group: any) => group.category === 'Travel' ? 135 : 40),
        getGroupExecutionRateText: jest.fn((group: any) => group.category === 'Travel' ? '135.0%' : '40.0%'),
        getExecutionRateTextClass: jest.fn((rate: number) => rate > 100 ? 'text-error' : 'text-success'),
        getGroupProgressColor: jest.fn((group: any) => group.category === 'Travel' ? 'error' : 'primary'),
        getBudgetProgressColor: jest.fn((budget: any) => budget.isOverBudget ? 'error' : 'primary'),
        formatAmount: jest.fn((amount: number) => `amount:${amount}`),
        ...overrides
    };
}

function createEvent(key = ''): any {
    return {
        key,
        stopPropagation: jest.fn(),
        preventDefault: jest.fn(),
        target: {}
    };
}

async function flush(): Promise<void> {
    await Promise.resolve();
    await actualVue.nextTick();
}

async function exerciseCallbacks(root: any): Promise<string[]> {
    const callbacks = collectHostCallbacks(root);
    for (const { name, callback } of callbacks) {
        if (name === 'onKeydown') {
            callback(createEvent('Delete'));
            callback(createEvent('Enter'));
        } else {
            callback(createEvent());
        }
        await flush();
    }
    return callbacks.map(item => item.name);
}

describe('BudgetListTable visible states', () => {
    test('renders loading skeletons and the empty result independently', () => {
        const loading = mountWithHostRenderer(BudgetListTable as any, createProps({
            loading: true,
            filteredBudgets: [],
            groupedBudgets: []
        }), uiComponents);
        try {
            expect(loading.root.children.length).toBeGreaterThan(0);
            expect(collectHostCallbacks(loading.root)).toHaveLength(0);
        } finally {
            loading.app.unmount();
        }

        const empty = mountWithHostRenderer(BudgetListTable as any, createProps({
            loading: false,
            filteredBudgets: [],
            groupedBudgets: []
        }), uiComponents);
        try {
            expect(empty.root.children.length).toBeGreaterThan(0);
            expect(collectHostCallbacks(empty.root)).toHaveLength(0);
        } finally {
            empty.app.unmount();
        }
    });

    test('projects grouped values, alerts, fallback names, amounts, colors, and light styling', () => {
        const props = createProps();
        const mounted = mountWithHostRenderer(BudgetListTable as any, props, uiComponents);
        try {
            expect(props.groupHasExpandedRows).toHaveBeenCalled();
            expect(props.getPrimaryBudgetForHeader).toHaveBeenCalled();
            expect(props.getExpandedPrimaryBudgets).toHaveBeenCalled();
            expect(props.getBudgetCategoryIcon).toHaveBeenCalled();
            expect(props.getBudgetCategoryColor).toHaveBeenCalled();
            expect(props.getGroupExecutionRate).toHaveBeenCalled();
            expect(props.getGroupExecutionRateText).toHaveBeenCalled();
            expect(props.getExecutionRateTextClass).toHaveBeenCalledWith(135);
            expect(props.getGroupProgressColor).toHaveBeenCalled();
            expect(props.getBudgetProgressColor).toHaveBeenCalled();
            expect(props.formatAmount).toHaveBeenCalledWith(200);
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});

describe('BudgetListTable events and conditional controls', () => {
    test('emits collapse, create, edit, delete, keyboard, and navigation actions', async () => {
        const onToggle = jest.fn();
        const onAdd = jest.fn();
        const onEdit = jest.fn();
        const onRemove = jest.fn();
        const onNavigate = jest.fn();
        const props = createProps({
            onToggleCategoryCollapse: onToggle,
            onAddPrimaryBudget: onAdd,
            onEdit,
            onRemove,
            onNavigateToTransactions: onNavigate
        });
        const mounted = mountWithHostRenderer(BudgetListTable as any, props, uiComponents);
        try {
            const callbackNames = await exerciseCallbacks(mounted.root);
            expect(callbackNames).toEqual(expect.arrayContaining(['onClick', 'onDblclick', 'onKeydown']));
            expect(onToggle).toHaveBeenCalledWith('Empty');
            expect(onAdd).toHaveBeenCalledWith(expect.objectContaining({ category: 'Empty' }));
            expect(onEdit).toHaveBeenCalledWith(expect.objectContaining({ id: 'warning' }));
            expect(onEdit).toHaveBeenCalledWith(expect.objectContaining({ id: 'over' }));
            expect(onRemove).toHaveBeenCalledWith(expect.objectContaining({ id: 'warning' }));
            expect(onRemove).toHaveBeenCalledWith(expect.objectContaining({ id: 'ordinary' }));
            expect(onNavigate).toHaveBeenCalledWith('Empty', null);
            expect(onNavigate).toHaveBeenCalledWith('Travel', expect.objectContaining({ id: 'over' }));
        } finally {
            mounted.app.unmount();
        }
    });

    test('renders dark, loading-with-data, and updating-disabled branches', () => {
        const darkLoading = mountWithHostRenderer(BudgetListTable as any, createProps({
            loading: true,
            updating: false,
            isDarkMode: true
        }), uiComponents);
        try {
            expect(darkLoading.root.children.length).toBeGreaterThan(0);
        } finally {
            darkLoading.app.unmount();
        }

        const updating = mountWithHostRenderer(BudgetListTable as any, createProps({
            loading: false,
            updating: true,
            isDarkMode: false
        }), uiComponents);
        try {
            expect(updating.root.children.length).toBeGreaterThan(0);
        } finally {
            updating.app.unmount();
        }
    });
});
