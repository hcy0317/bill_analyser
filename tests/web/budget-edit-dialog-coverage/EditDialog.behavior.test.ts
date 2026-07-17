import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockBudgetType = {
    Expense: 3,
    Investment: 5
} as const;

const mockBudgetPeriodType = {
    Daily: 'daily',
    Weekly: 'weekly',
    Monthly: 'monthly',
    Quarterly: 'quarterly',
    Yearly: 'yearly'
} as const;

const mockCategoryType = {
    Expense: 3,
    Investment: 5
} as const;

class MockBudget {
    public id = '';
    public name = '';
    public category = '';
    public subCategory = '';
    public categoryId = '';
    public periodType = mockBudgetPeriodType.Monthly;
    public amountCents = 0;
    public startDate = '';
    public endDate = '';
    public alertThreshold = 80;
    public enabled = true;
    public type = mockBudgetType.Expense;
}

function createCategory(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'food',
        name: 'Food',
        parentId: '0',
        type: mockCategoryType.Expense,
        icon: 'food',
        color: '#ef4444',
        hidden: false,
        subCategories: [],
        ...overrides
    };
}

const mockDiningCategory = createCategory({
    id: 'food-dining',
    name: 'Dining',
    parentId: 'food',
    subCategories: undefined
});
const mockFoodCategory = createCategory({ subCategories: [mockDiningCategory] });
const mockHiddenExpenseCategory = createCategory({
    id: 'archived-expense',
    name: 'Archived Expense',
    hidden: true,
    subCategories: undefined
});
const mockFundCategory = createCategory({
    id: 'fund',
    name: 'Fund',
    type: mockCategoryType.Investment,
    color: '#2563eb',
    subCategories: undefined
});

const mockBudgetStore = {
    saveBudget: jest.fn<(args: { budget: MockBudget }) => Promise<void>>()
};

const mockCategoryStore = (jest.requireActual('vue') as any).reactive({
    allTransactionCategories: {
        [mockCategoryType.Expense]: [mockFoodCategory, mockHiddenExpenseCategory],
        [mockCategoryType.Investment]: [mockFundCategory]
    } as Record<number, any[]>,
    loadAllCategories: jest.fn<(args: { force: boolean }) => Promise<void>>()
});

const mockLogger = {
    debug: jest.fn(),
    error: jest.fn(),
    warn: jest.fn()
};

const mockFindBudgetCategoryIdByNames = jest.fn(
    (categories: any[], categoryName: string, subCategoryName?: string): string => {
        for (const category of categories) {
            if (category.name !== categoryName) continue;
            if (!subCategoryName) return category.id;
            return category.subCategories?.find((item: any) => item.name === subCategoryName)?.id ?? '';
        }
        return '';
    }
);

const mockResolveBudgetCategorySelection = jest.fn(
    (categories: any[], categoryId: string): any => {
        for (const category of categories) {
            if (category.id === categoryId) {
                return { primaryCategoryName: category.name, secondaryCategoryName: '' };
            }
            const subCategory = category.subCategories?.find((item: any) => item.id === categoryId);
            if (subCategory) {
                return {
                    primaryCategoryName: category.name,
                    secondaryCategoryName: subCategory.name
                };
            }
        }
        return undefined;
    }
);

const mockSlotComponent = (() => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return defineComponent({
        name: 'BudgetEditCoverageSlot',
        inheritAttrs: false,
        setup(_props: unknown, { attrs, slots }: any) {
            const slotProps = {
                props: {},
                itemProps: {},
                item: {
                    title: 'Dining',
                    raw: { id: 'food-dining', icon: 'food', color: '#ef4444' }
                },
                modelValue: 80
            };
            return () => h(
                'stub',
                attrs,
                Object.values(slots).flatMap((slot: any) => slot?.(slotProps) ?? [])
            );
        }
    });
})();

jest.mock('@/models/budget.ts', () => ({
    Budget: MockBudget,
    BudgetType: mockBudgetType,
    BudgetPeriodType: mockBudgetPeriodType
}));
jest.mock('@/core/category.ts', () => ({ CategoryType: mockCategoryType }));
jest.mock('@/stores/budget.ts', () => ({ useBudgetStore: () => mockBudgetStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoryStore
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: mockLogger
}));
jest.mock('@/views/desktop/budgets/categorySelection.ts', () => ({
    findBudgetCategoryIdByNames: (categories: any[], category: string, subCategory?: string) => (
        mockFindBudgetCategoryIdByNames(categories, category, subCategory)
    ),
    resolveBudgetCategorySelection: (categories: any[], categoryId: string) => (
        mockResolveBudgetCategorySelection(categories, categoryId)
    )
}));
jest.mock('@/components/desktop/TwoColumnSelect.vue', () => ({
    __esModule: true,
    default: mockSlotComponent
}));
jest.mock('@/components/desktop/DateOnlySelect.vue', () => ({
    __esModule: true,
    default: mockSlotComponent
}));
jest.mock('@/components/desktop/ItemIcon.vue', () => ({
    __esModule: true,
    default: mockSlotComponent
}));
jest.mock('@mdi/js', () => ({ mdiClose: 'mdi-close' }));

import EditDialog from '@/views/desktop/budgets/list/dialogs/EditDialog.vue';

function createBudget(overrides: Partial<MockBudget> = {}): MockBudget {
    return Object.assign(new MockBudget(), overrides);
}

function setup(): { bindings: any; emit: jest.Mock; exposed: Record<string, any> } {
    const emit = jest.fn();
    let exposed: Record<string, any> = {};
    const bindings = (EditDialog as any).setup({}, {
        emit,
        expose: (value: Record<string, any>) => {
            exposed = value;
        }
    });
    expect(exposed['open']).toBe(bindings.open);
    return { bindings, emit, exposed };
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await (jest.requireActual('vue') as any).nextTick();
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(): { app: any; root: any; state: any } {
    const { createRenderer } = jest.requireActual('vue') as any;
    const renderer = createRenderer({
        patchProp(node: any, key: string, _previous: unknown, value: unknown) {
            node.props[key] = value;
        },
        insert(child: any, parent: any, anchor: any = null) {
            child.parent = parent;
            if (!anchor) {
                parent.children.push(child);
                return;
            }
            const index = parent.children.indexOf(anchor);
            parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
        },
        remove(child: any) {
            const index = child.parent?.children.indexOf(child) ?? -1;
            if (index >= 0) child.parent.children.splice(index, 1);
        },
        createElement(type: string) {
            return createHostNode(type);
        },
        createText(text: string) {
            return createHostNode('#text', text);
        },
        createComment(text: string) {
            return createHostNode('#comment', text);
        },
        setText(node: any, text: string) {
            node.text = text;
        },
        setElementText(node: any, text: string) {
            node.text = text;
            node.children = [];
        },
        parentNode(node: any) {
            return node.parent;
        },
        nextSibling(node: any) {
            const siblings = node.parent?.children ?? [];
            return siblings[siblings.indexOf(node) + 1] ?? null;
        },
        querySelector() {
            return null;
        },
        setScopeId(node: any, scopeId: string) {
            node.props[scopeId] = '';
        },
        cloneNode(node: any) {
            return { ...node, children: [...node.children], props: { ...node.props }, parent: null };
        },
        insertStaticContent(content: string, parent: any, anchor: any) {
            const node = createHostNode('#static', content);
            node.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, node);
            return [node, node];
        }
    });
    const app = renderer.createApp(EditDialog as any);
    app.config.warnHandler = () => undefined;
    for (const name of [
        'v-dialog', 'v-card', 'v-btn', 'v-icon', 'v-card-text', 'v-form', 'v-row', 'v-col',
        'v-select', 'v-list-item', 'amount-input', 'v-switch', 'v-slider', 'v-textarea'
    ]) app.component(name, mockSlotComponent);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState };
}

type HostCallback = {
    node: any;
    name: string;
    callback: (...args: any[]) => any;
};

function collectHostCallbacks(node: any, callbacks: HostCallback[], seen = new Set<any>()): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    for (const [name, value] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        if (typeof value === 'function') {
            callbacks.push({ node, name, callback: value as (...args: any[]) => any });
        } else if (Array.isArray(value)) {
            for (const callback of value) {
                if (typeof callback === 'function') {
                    callbacks.push({ node, name, callback });
                }
            }
        }
    }
    for (const child of node.children ?? []) collectHostCallbacks(child, callbacks, seen);
}

function updateValueFor(node: any): unknown {
    const label = String(node.props?.label ?? '');
    if (label.includes('Budget Type')) return mockBudgetType.Investment;
    if (label.includes('Period Type')) return mockBudgetPeriodType.Quarterly;
    if (label.includes('Category')) return 'food-dining';
    if (label.includes('Budget Amount')) return 12_345;
    if (label.includes('Primary Category Only')) return true;
    if (label.includes('Start Date')) return Date.UTC(2026, 0, 2);
    if (label.includes('End Date')) return Date.UTC(2026, 0, 31);
    if (label.includes('Remarks')) return 'Template update';
    if (node.props?.max === 100) return 73;
    return true;
}

async function invokeHostCallbacks(callbacks: HostCallback[]): Promise<void> {
    for (const { node, name, callback } of callbacks) {
        if (name === 'onUpdate:modelValue') {
            callback(updateValueFor(node));
        } else {
            callback({ preventDefault: jest.fn(), stopPropagation: jest.fn() });
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockCategoryStore.allTransactionCategories = {
        [mockCategoryType.Expense]: [mockFoodCategory, mockHiddenExpenseCategory],
        [mockCategoryType.Investment]: [mockFundCategory]
    };
    mockBudgetStore.saveBudget.mockResolvedValue(undefined);
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockFindBudgetCategoryIdByNames.mockImplementation(
        (categories: any[], categoryName: string, subCategoryName?: string): string => {
            for (const category of categories) {
                if (category.name !== categoryName) continue;
                if (!subCategoryName) return category.id;
                return category.subCategories?.find((item: any) => item.name === subCategoryName)?.id ?? '';
            }
            return '';
        }
    );
    mockResolveBudgetCategorySelection.mockImplementation(
        (categories: any[], categoryId: string): any => {
            for (const category of categories) {
                if (category.id === categoryId) {
                    return { primaryCategoryName: category.name, secondaryCategoryName: '' };
                }
                const subCategory = category.subCategories?.find((item: any) => item.id === categoryId);
                if (subCategory) {
                    return {
                        primaryCategoryName: category.name,
                        secondaryCategoryName: subCategory.name
                    };
                }
            }
            return undefined;
        }
    );
});

describe('desktop budget EditDialog behavior', () => {
    test('opens an existing budget, resolves its category id, and preserves integer minor units', async () => {
        const { bindings } = setup();
        const source = createBudget({
            id: 'budget-1',
            name: 'Dining plan',
            category: 'Food',
            subCategory: 'Dining',
            amountCents: 12_345,
            startDate: '2026-01-02',
            endDate: '2026-01-31'
        });

        bindings.open({ budget: source });
        expect(bindings.showState.value).toBe(true);
        expect(bindings.originalBudget.value).toStrictEqual(source);
        expect(bindings.budget.value).not.toBe(source);
        expect(bindings.budgetAmountInCents.value).toBe(12_345);
        expect(bindings.budgetAmountInCents.value).not.toBe(123.45);
        expect(bindings.usePrimaryCategoryOnly.value).toBe(false);
        expect(bindings.startDateTime.value).toBe(new Date('2026-01-02').getTime());
        expect(bindings.endDateTime.value).toBe(new Date('2026-01-31').getTime());
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledWith({ force: false });

        await flush();
        expect(bindings.budget.value.categoryId).toBe('food-dining');
        expect(bindings.isInitializing.value).toBe(false);
        expect(mockFindBudgetCategoryIdByNames).toHaveBeenCalledWith(
            [mockFoodCategory, mockHiddenExpenseCategory],
            'Food',
            'Dining'
        );
    });

    test('uses new-budget overrides, default month dates, primary filtering, and safe fallbacks', async () => {
        jest.useFakeTimers().setSystemTime(new Date('2026-07-15T08:00:00Z'));
        try {
            const { bindings } = setup();
            const source = createBudget({ category: 'Missing', amountCents: 7_777 });

            bindings.open({
                budget: source,
                type: mockBudgetType.Investment,
                usePrimaryCategoryOnly: true
            });
            await flush();

            expect(bindings.budget.value.type).toBe(mockBudgetType.Investment);
            expect(bindings.usePrimaryCategoryOnly.value).toBe(true);
            expect(bindings.budgetAmountInCents.value).toBe(7_777);
            expect(new Date(bindings.startDateTime.value).getDate()).toBe(1);
            expect(new Date(bindings.endDateTime.value).getDate()).toBe(31);
            expect(bindings.availableCategories.value).toStrictEqual([mockFundCategory]);
            expect(bindings.availablePrimaryCategories.value).toStrictEqual([mockFundCategory]);
            expect(bindings.budget.value.categoryId).toBe('');

            mockCategoryStore.allTransactionCategories = {};
            expect(bindings.availableCategories.value).toStrictEqual([]);
            expect(bindings.availablePrimaryCategories.value).toStrictEqual([]);
        } finally {
            jest.useRealTimers();
        }
    });

    test('computes options, category maps, names, and selection changes for primary and secondary ids', () => {
        const { bindings } = setup();
        expect(bindings.isNew.value).toBe(true);
        expect(bindings.budgetTypeOptions.value).toStrictEqual([
            { text: 'tt:Expense', value: mockBudgetType.Expense },
            { text: 'tt:Investment', value: mockBudgetType.Investment }
        ]);
        expect(bindings.periodTypeOptions.value).toStrictEqual([
            { text: 'tt:Monthly', value: mockBudgetPeriodType.Monthly },
            { text: 'tt:Quarterly', value: mockBudgetPeriodType.Quarterly },
            { text: 'tt:Yearly', value: mockBudgetPeriodType.Yearly }
        ]);
        expect(bindings.availablePrimaryCategories.value).toStrictEqual([mockFoodCategory]);
        expect(bindings.allCategoriesMap.value).toMatchObject({
            food: mockFoodCategory,
            'food-dining': mockDiningCategory,
            'archived-expense': mockHiddenExpenseCategory
        });
        expect(bindings.selectedPrimaryCategoryName.value).toBe('');
        expect(bindings.selectedSecondaryCategoryName.value).toBe('');

        bindings.onCategoryChange('food-dining');
        expect(bindings.budget.value).toMatchObject({
            categoryId: 'food-dining',
            category: 'Food',
            subCategory: 'Dining'
        });
        expect(bindings.selectedPrimaryCategoryName.value).toBe('Food');
        expect(bindings.selectedSecondaryCategoryName.value).toBe('Dining');

        bindings.onCategoryChange('food');
        expect(bindings.budget.value).toMatchObject({
            categoryId: 'food',
            category: 'Food',
            subCategory: ''
        });

        bindings.onCategoryChange(undefined);
        mockResolveBudgetCategorySelection.mockReturnValueOnce(undefined);
        bindings.onCategoryChange('missing-id');
        expect(bindings.budget.value.categoryId).toBe('missing-id');
        expect(mockLogger.warn).toHaveBeenCalledWith(
            '[Budget] Category not found anywhere for id: "missing-id"'
        );
    });

    test('keeps category selection during initialization, then clears it on type and mode changes', async () => {
        let resolveLoad: (() => void) | undefined;
        mockCategoryStore.loadAllCategories.mockReturnValueOnce(new Promise<void>(resolve => {
            resolveLoad = resolve;
        }));
        const { bindings } = setup();
        bindings.open({
            budget: createBudget({
                category: 'Food',
                subCategory: 'Dining',
                categoryId: 'food-dining'
            })
        });

        bindings.budget.value.type = mockBudgetType.Investment;
        bindings.usePrimaryCategoryOnly.value = true;
        await (jest.requireActual('vue') as any).nextTick();
        expect(bindings.budget.value.categoryId).toBe('food-dining');

        resolveLoad?.();
        await flush();
        bindings.budget.value.type = mockBudgetType.Expense;
        await (jest.requireActual('vue') as any).nextTick();
        expect(bindings.budget.value).toMatchObject({ category: '', subCategory: '', categoryId: '' });

        bindings.budget.value.category = 'Food';
        bindings.budget.value.subCategory = 'Dining';
        bindings.budget.value.categoryId = 'food-dining';
        bindings.usePrimaryCategoryOnly.value = false;
        await (jest.requireActual('vue') as any).nextTick();
        expect(bindings.budget.value).toMatchObject({ category: '', subCategory: '', categoryId: '' });
    });

    test('rejects a missing category after synchronizing amount and optional dates', async () => {
        const { bindings, emit } = setup();
        bindings.budgetAmountInCents.value = 12_345;
        bindings.startDateTime.value = 0;
        bindings.endDateTime.value = 0;

        await bindings.save();

        expect(bindings.budget.value.amountCents).toBe(12_345);
        expect(bindings.budget.value.amountCents).not.toBe(1_234_500);
        expect(mockBudgetStore.saveBudget).not.toHaveBeenCalled();
        expect(mockLogger.warn).toHaveBeenCalledWith('[Budget Save] Category is required');
        expect(emit).not.toHaveBeenCalled();
    });

    test('saves exact cents and ISO dates, emits success, and resets dialog state', async () => {
        const { bindings, emit } = setup();
        bindings.open({
            budget: createBudget({ categoryId: 'food-dining', amountCents: 12_345 })
        });
        await flush();
        bindings.budgetAmountInCents.value = 12_345;
        bindings.startDateTime.value = Date.UTC(2026, 0, 2);
        bindings.endDateTime.value = Date.UTC(2026, 0, 31);

        await bindings.save();

        expect(mockBudgetStore.saveBudget).toHaveBeenCalledTimes(1);
        const saved = mockBudgetStore.saveBudget.mock.calls[0]?.[0].budget;
        expect(saved).toMatchObject({
            categoryId: 'food-dining',
            category: 'Food',
            subCategory: 'Dining',
            amountCents: 12_345,
            startDate: '2026-01-02',
            endDate: '2026-01-31'
        });
        expect(saved?.amountCents).not.toBe(123.45);
        expect(saved?.amountCents).not.toBe(1_234_500);
        expect(emit).toHaveBeenCalledWith('budget:saved');
        expect(bindings.showState.value).toBe(false);
        expect(bindings.originalBudget.value).toBeNull();
        expect(bindings.budgetAmountInCents.value).toBe(0);
        expect(bindings.startDateTime.value).toBe(0);
        expect(bindings.endDateTime.value).toBe(0);
        expect(bindings.saving.value).toBe(false);
    });

    test('logs store failures, clears saving, and keeps the editable budget open', async () => {
        const failure = new Error('synthetic budget save failure');
        mockBudgetStore.saveBudget.mockRejectedValueOnce(failure);
        const { bindings, emit } = setup();
        bindings.open({ budget: createBudget({ categoryId: 'food', amountCents: 9_999 }) });
        await flush();

        await bindings.save();

        expect(mockLogger.error).toHaveBeenCalledWith('Failed to save budget:', failure);
        expect(bindings.saving.value).toBe(false);
        expect(bindings.showState.value).toBe(true);
        expect(bindings.budget.value.amountCents).toBe(9_999);
        expect(emit).not.toHaveBeenCalled();

        bindings.close();
        expect(bindings.showState.value).toBe(false);
        expect(bindings.budget.value).toStrictEqual(new MockBudget());
    });
});

describe('desktop budget EditDialog production template', () => {
    test('renders both category modes, slot bodies, state branches, and event wrappers', async () => {
        const mounted = mountWithHostRenderer();
        try {
            await (jest.requireActual('vue') as any).nextTick();
            const initialCallbacks: HostCallback[] = [];
            collectHostCallbacks(mounted.root, initialCallbacks);
            expect(initialCallbacks.length).toBeGreaterThan(10);
            await invokeHostCallbacks(initialCallbacks.filter(item => item.name === 'onUpdate:modelValue'));

            mounted.state.budget = createBudget({
                id: 'existing-budget',
                category: 'Food',
                categoryId: 'food',
                amountCents: 12_345
            });
            mounted.state.budgetAmountInCents = 12_345;
            mounted.state.usePrimaryCategoryOnly = true;
            mounted.state.saving = true;
            await (jest.requireActual('vue') as any).nextTick();

            const alternateCallbacks: HostCallback[] = [];
            collectHostCallbacks(mounted.root, alternateCallbacks);
            await invokeHostCallbacks(alternateCallbacks);
            expect(alternateCallbacks.length).toBeGreaterThan(10);
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
