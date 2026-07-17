import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const actualBudgetModule = jest.requireActual('@/models/budget.ts') as any;
const { Budget, BudgetType, BudgetPeriodType } = actualBudgetModule;

const mockCategoryType = { Expense: 3, Investment: 5 } as const;
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockLoggerInfo = jest.fn<(...args: any[]) => void>();
const mockLoggerError = jest.fn<(...args: any[]) => void>();
const mockBuildMobileBudgetGroups = jest.fn<(...args: any[]) => any[]>();
const mockFormatBudgetAmount = jest.fn<(amount: number) => string>();
const mockGetBudgetGroupExecutionRate = jest.fn<(group: any) => number>();
const mockGetBudgetGroupProgressPercent = jest.fn<(group: any) => number>();
const mockGetBudgetProgressPercent = jest.fn<(budget: any) => number>();
const mockSelectBudgetsByType = jest.fn<(...args: any[]) => any[]>();

function createSlotStub(name: string, exposeMethods = false): any {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return defineComponent({
        name,
        inheritAttrs: false,
        methods: exposeMethods ? { setSaving: jest.fn() } : {},
        setup: (_props: unknown, { attrs, slots }: any) => () => h(
            'stub',
            attrs,
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            }),
        ),
    });
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string, params?: Record<string, string>) => (
        params ? `tt:${key}:${params['name']}` : `tt:${key}`
    ) }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast }),
    showLoading: mockShowLoading,
    hideLoading: mockHideLoading,
}));
jest.mock('@/core/category.ts', () => ({ CategoryType: mockCategoryType }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        info: (...args: any[]) => mockLoggerInfo(...args),
        error: (...args: any[]) => mockLoggerError(...args),
    },
}));
jest.mock('@/views/mobile/budgets/listPageHelpers.ts', () => ({
    buildMobileBudgetGroups: (...args: any[]) => mockBuildMobileBudgetGroups(...args),
    formatBudgetAmount: (amount: number) => mockFormatBudgetAmount(amount),
    getBudgetGroupExecutionRate: (group: any) => mockGetBudgetGroupExecutionRate(group),
    getBudgetGroupProgressPercent: (group: any) => mockGetBudgetGroupProgressPercent(group),
    getBudgetProgressPercent: (budget: any) => mockGetBudgetProgressPercent(budget),
    selectBudgetsByType: (...args: any[]) => mockSelectBudgetsByType(...args),
}));
jest.mock('@/views/mobile/budgets/EditSheet.vue', () => ({
    __esModule: true,
    default: createSlotStub('MobileBudgetEditSheetStub', true),
}));

function createBudget(overrides: Record<string, unknown> = {}): any {
    const budget = new Budget();
    Object.assign(budget, {
        id: 'food-primary',
        name: 'Food budget',
        category: 'Food',
        subCategory: '',
        categoryId: 'food',
        categoryIcon: 'fork_knife',
        categoryColor: '#00aa66',
        type: BudgetType.Expense,
        periodType: BudgetPeriodType.Monthly,
        amountCents: 12_345,
        spentAmountCents: 2_345,
        executionRate: 19,
        alertThreshold: 80,
        enabled: true,
        isOverBudget: false,
        alertTriggered: false,
        ...overrides,
    });
    return budget;
}

const mockExpensePrimary = createBudget();
const mockExpenseSub = createBudget({
    id: 'food-dining',
    name: 'Dining budget',
    subCategory: 'Dining',
    amountCents: 20_000,
    spentAmountCents: 9_000,
    executionRate: 45,
});
const mockTravelPrimaryOne = createBudget({
    id: 'travel-one',
    name: 'Travel one',
    category: 'Travel',
    categoryId: 'travel',
    amountCents: 30_000,
    spentAmountCents: 27_000,
    executionRate: 90,
    alertTriggered: true,
});
const mockTravelPrimaryTwo = createBudget({
    id: 'travel-two',
    name: 'Travel two',
    category: 'Travel',
    categoryId: 'travel',
    amountCents: 10_000,
    spentAmountCents: 12_000,
    executionRate: 120,
    isOverBudget: true,
});
const mockInvestment = createBudget({
    id: 'fund-primary',
    name: 'Fund budget',
    category: 'Fund',
    categoryId: 'fund',
    type: BudgetType.Investment,
    amountCents: 98_765,
    spentAmountCents: 9_876,
    executionRate: 10,
});

const mockBudgetStore = actualVue.reactive({
    expenseBudgets: [mockExpensePrimary, mockExpenseSub, mockTravelPrimaryOne, mockTravelPrimaryTwo] as any[],
    investmentBudgets: [mockInvestment] as any[],
    loadAllBudgets: jest.fn<(...args: any[]) => Promise<void>>(),
    loadBudgetExecution: jest.fn<(...args: any[]) => Promise<void>>(),
    saveBudget: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteBudget: jest.fn<(...args: any[]) => Promise<void>>(),
});

const mockCategoriesStore = actualVue.reactive({
    allTransactionCategories: {
        [mockCategoryType.Expense]: [{ id: 'food', name: 'Food' }, { id: 'travel', name: 'Travel' }],
        [mockCategoryType.Investment]: [{ id: 'fund', name: 'Fund' }],
    } as Record<number, any[]>,
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>(),
});

jest.mock('@/stores/budget.ts', () => ({ useBudgetStore: () => mockBudgetStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoriesStore,
}));

const ListPage = require('@/views/mobile/budgets/ListPage.vue').default as any;

function buildGroups({ budgets, collapsedCategories }: any): any[] {
    if (!budgets.length) return [];
    if (budgets[0]?.type === BudgetType.Investment) {
        return [{
            category: 'Fund',
            categoryIcon: 'chart_bar',
            categoryColor: '#3366ff',
            primaryBudgets: [mockInvestment],
            subBudgets: [],
            totalAmountCents: 98_765,
            totalSpentCents: 9_876,
            isCollapsed: collapsedCategories.has('Fund'),
            rate: 10,
        }];
    }
    return [
        {
            category: 'Food',
            categoryIcon: 'fork_knife',
            categoryColor: '#00aa66',
            primaryBudgets: [mockExpensePrimary],
            subBudgets: [mockExpenseSub],
            totalAmountCents: 12_345,
            totalSpentCents: 2_345,
            isCollapsed: collapsedCategories.has('Food'),
            rate: 19,
        },
        {
            category: 'Travel',
            categoryIcon: 'airplane',
            categoryColor: '#ff8800',
            primaryBudgets: [mockTravelPrimaryOne, mockTravelPrimaryTwo],
            subBudgets: [],
            totalAmountCents: 40_000,
            totalSpentCents: 39_000,
            isCollapsed: collapsedCategories.has('Travel'),
            rate: 120,
        },
        {
            category: 'Misc',
            categoryIcon: '',
            categoryColor: '',
            primaryBudgets: [],
            subBudgets: [createBudget({ id: 'misc-sub', name: '', category: 'Misc', subCategory: '' })],
            totalAmountCents: 5_000,
            totalSpentCents: 4_500,
            isCollapsed: collapsedCategories.has('Misc'),
            rate: 90,
        },
    ];
}

function setup(): { bindings: any } {
    const bindings = ListPage.setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn(),
    });
    return { bindings };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function deferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (reason: unknown) => void;
} {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(): { app: any; root: any; state: any } {
    const { createRenderer, defineComponent, h } = actualVue;
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
        },
    });
    const SlotHost = defineComponent({
        name: 'MobileBudgetSlotHost',
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => h(
            'stub',
            attrs,
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            }),
        ),
    });
    const app = renderer.createApp(ListPage);
    app.config.warnHandler = () => undefined;
    for (const name of [
        'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
        'f7-toolbar', 'f7-segmented', 'f7-button', 'f7-list', 'f7-list-item', 'f7-icon',
        'f7-progressbar', 'f7-swipeout-actions', 'f7-swipeout-button', 'f7-actions',
        'f7-actions-group', 'f7-actions-label', 'f7-actions-button', 'item-icon',
    ]) app.component(name, SlotHost);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState };
}

function walkHostNodes(node: any, visit: (node: any) => void, seen = new Set<any>()): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    visit(node);
    for (const child of node.children ?? []) walkHostNodes(child, visit, seen);
}

function collectHostCallbacks(node: any): Array<{ name: string; callback: (...args: any[]) => unknown }> {
    const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
    walkHostNodes(node, current => {
        for (const [name, value] of Object.entries(current.props ?? {})) {
            if (!name.startsWith('on')) continue;
            for (const candidate of Array.isArray(value) ? value : [value]) {
                if (typeof candidate === 'function') {
                    callbacks.push({ name, callback: candidate as (...args: any[]) => unknown });
                }
            }
        }
    });
    return callbacks;
}

async function invokeHostCallbacks(
    callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }>,
): Promise<void> {
    for (const { name, callback } of callbacks) {
        try {
            if (name === 'onPtr:refresh') callback(jest.fn());
            else if (name === 'onUpdate:show') callback(true);
            else if (name === 'onSave') callback(createBudget({ id: 'saved', amountCents: 54_321 }));
            else if (name === 'onDelete:request') callback(mockExpensePrimary);
            else callback();
        } catch {
            // Generated Framework7 wrappers close over different row states.
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockBudgetStore.expenseBudgets = [
        mockExpensePrimary, mockExpenseSub, mockTravelPrimaryOne, mockTravelPrimaryTwo,
    ];
    mockBudgetStore.investmentBudgets = [mockInvestment];
    mockCategoriesStore.allTransactionCategories = {
        [mockCategoryType.Expense]: [{ id: 'food', name: 'Food' }, { id: 'travel', name: 'Travel' }],
        [mockCategoryType.Investment]: [{ id: 'fund', name: 'Fund' }],
    };
    mockBudgetStore.loadAllBudgets.mockResolvedValue(undefined);
    mockBudgetStore.loadBudgetExecution.mockResolvedValue(undefined);
    mockBudgetStore.saveBudget.mockResolvedValue(undefined);
    mockBudgetStore.deleteBudget.mockResolvedValue(undefined);
    mockCategoriesStore.loadAllCategories.mockResolvedValue(undefined);
    mockSelectBudgetsByType.mockImplementation((type, expense, investment) => (
        type === BudgetType.Expense ? expense : investment
    ));
    mockBuildMobileBudgetGroups.mockImplementation(buildGroups);
    mockFormatBudgetAmount.mockImplementation(amount => `formatted:${amount.toFixed(2)}`);
    mockGetBudgetGroupExecutionRate.mockImplementation(group => group.rate);
    mockGetBudgetGroupProgressPercent.mockImplementation(group => Math.min(group.rate, 100));
    mockGetBudgetProgressPercent.mockImplementation(budget => Math.min(budget.executionRate, 100));
});

describe('mobile budget ListPage computed state and list actions', () => {
    test('selects budget/category types, builds groups, and preserves cents', () => {
        const { bindings } = setup();
        expect(bindings.loading.value).toBe(true);
        expect(bindings.activeBudgetType.value).toBe(BudgetType.Expense);
        expect(bindings.filteredBudgets.value).toStrictEqual(mockBudgetStore.expenseBudgets);
        expect(bindings.currentCategoryType.value).toBe(mockCategoryType.Expense);
        expect(bindings.budgetPrimaryCategories.value).toStrictEqual(
            mockCategoriesStore.allTransactionCategories[mockCategoryType.Expense],
        );
        expect(bindings.groupedBudgets.value).toHaveLength(3);
        expect(mockBuildMobileBudgetGroups).toHaveBeenCalledWith({
            budgets: mockBudgetStore.expenseBudgets,
            primaryCategories: mockCategoriesStore.allTransactionCategories[mockCategoryType.Expense],
            collapsedCategories: bindings.collapsedBudgetGroups.value,
        });
        expect(bindings.filteredBudgets.value[0].amountCents).toBe(12_345);

        bindings.activeBudgetType.value = BudgetType.Investment;
        expect(bindings.filteredBudgets.value).toStrictEqual([mockInvestment]);
        expect(bindings.currentCategoryType.value).toBe(mockCategoryType.Investment);
        expect(bindings.budgetPrimaryCategories.value).toStrictEqual(
            mockCategoriesStore.allTransactionCategories[mockCategoryType.Investment],
        );
        expect(bindings.groupedBudgets.value[0].totalAmountCents).toBe(98_765);

        bindings.activeBudgetType.value = BudgetType.Expense;
        mockCategoriesStore.allTransactionCategories[mockCategoryType.Expense] = undefined as any;
        expect(bindings.budgetPrimaryCategories.value).toStrictEqual([]);
        expect(bindings.formatAmount(123.45)).toBe('formatted:123.45');
    });

    test('switches tabs once, clears collapse state, and reloads current type', async () => {
        const { bindings } = setup();
        bindings.collapsedBudgetGroups.value = new Set(['Food']);
        mockBudgetStore.loadAllBudgets.mockClear();

        bindings.switchBudgetType(BudgetType.Expense);
        expect(mockBudgetStore.loadAllBudgets).not.toHaveBeenCalled();
        expect(bindings.collapsedBudgetGroups.value.has('Food')).toBe(true);

        bindings.switchBudgetType(BudgetType.Investment);
        await flush();
        expect(bindings.activeBudgetType.value).toBe(BudgetType.Investment);
        expect(bindings.collapsedBudgetGroups.value.size).toBe(0);
        expect(mockBudgetStore.loadAllBudgets).toHaveBeenCalledWith({
            force: true,
            type: BudgetType.Investment,
            periodType: BudgetPeriodType.Monthly,
        });
    });

    test('toggles groups and projects primary, expanded, rate, and progress branches', () => {
        const { bindings } = setup();
        bindings.toggleBudgetGroup('Food');
        expect(bindings.collapsedBudgetGroups.value.has('Food')).toBe(true);
        bindings.toggleBudgetGroup('Food');
        expect(bindings.collapsedBudgetGroups.value.has('Food')).toBe(false);

        const [food, travel, misc] = bindings.groupedBudgets.value;
        expect(bindings.getPrimaryBudgetForHeader(food)).toBe(mockExpensePrimary);
        expect(bindings.getPrimaryBudgetForHeader(misc)).toBeNull();
        expect(bindings.getExpandedBudgetRows(food)).toStrictEqual([mockExpenseSub]);
        expect(bindings.getExpandedBudgetRows(travel)).toStrictEqual([
            mockTravelPrimaryOne, mockTravelPrimaryTwo,
        ]);

        expect(bindings.getBudgetGroupRateClass(food)).toEqual({
            'text-color-red': false,
            'text-color-orange': false,
        });
        expect(bindings.getBudgetGroupRateClass(misc)).toEqual({
            'text-color-red': false,
            'text-color-orange': true,
        });
        expect(bindings.getBudgetGroupRateClass(travel)).toEqual({
            'text-color-red': true,
            'text-color-orange': false,
        });
        expect(bindings.getBudgetGroupProgressClass(food)).toEqual({
            'color-red': false,
            'color-orange': false,
        });
        expect(bindings.getBudgetGroupProgressClass(misc)).toEqual({
            'color-red': false,
            'color-orange': true,
        });
        expect(bindings.getBudgetGroupProgressClass(travel)).toEqual({
            'color-red': true,
            'color-orange': false,
        });
    });

    test('opens create/edit sheets, logs deferred taps, and keeps minor units unchanged', () => {
        const { bindings } = setup();
        bindings.openCreateSheet();
        expect(bindings.editingBudget.value).toBeNull();
        expect(bindings.showEditSheet.value).toBe(true);

        bindings.openEditSheet(mockInvestment);
        expect(bindings.editingBudget.value).toBeInstanceOf(Budget);
        expect(bindings.editingBudget.value).not.toBe(mockInvestment);
        expect(bindings.editingBudget.value.id).toBe('fund-primary');
        expect(bindings.editingBudget.value.amountCents).toBe(98_765);
        expect(mockInvestment.amountCents).toBe(98_765);

        bindings.onBudgetTap(mockInvestment);
        expect(mockLoggerInfo).toHaveBeenCalledWith(expect.stringContaining('fund-primary'));
        expect(mockInvestment.amountCents).toBe(98_765);
    });

    test('builds generic, named, and category-fallback delete labels', () => {
        const { bindings } = setup();
        expect(bindings.deleteConfirmLabel.value).toBe(
            'tt:Are you sure you want to delete this budget?',
        );

        bindings.budgetToDelete.value = mockExpensePrimary;
        expect(bindings.deleteConfirmLabel.value).toContain('Food budget');

        const categoryOnly = createBudget({ name: '', category: 'Housing', subCategory: 'Rent' });
        bindings.budgetToDelete.value = categoryOnly;
        expect(bindings.deleteConfirmLabel.value).toContain('Housing-Rent');

        const nameless = createBudget({ name: '', category: '', subCategory: '' });
        bindings.budgetToDelete.value = nameless;
        expect(bindings.deleteConfirmLabel.value).toBe(
            'tt:Are you sure you want to delete this budget "{name}"?:',
        );
    });
});

describe('mobile budget ListPage save, delete, and loading lifecycle', () => {
    test('saves integer cents, resets sheet state, and reloads on success', async () => {
        const { bindings } = setup();
        const editSheet = { setSaving: jest.fn() };
        bindings.editSheetRef.value = editSheet;
        bindings.showEditSheet.value = true;
        bindings.editingBudget.value = mockExpensePrimary;
        const updated = createBudget({ id: 'updated', amountCents: 54_321 });

        bindings.onSheetSave(updated);
        expect(mockShowLoading).toHaveBeenCalled();
        expect(mockBudgetStore.saveBudget).toHaveBeenCalledWith({ budget: updated });
        expect((mockBudgetStore.saveBudget.mock.calls[0]?.[0] as any).budget.amountCents).toBe(54_321);
        await flush();
        expect(mockHideLoading).toHaveBeenCalled();
        expect(editSheet.setSaving).toHaveBeenCalledWith(false);
        expect(bindings.showEditSheet.value).toBe(false);
        expect(bindings.editingBudget.value).toBeNull();
        expect(mockShowToast).toHaveBeenCalledWith('Budget has been saved');
        expect(mockBudgetStore.loadAllBudgets).toHaveBeenCalled();

        bindings.editSheetRef.value = null;
        bindings.onSheetSave(createBudget({ id: 'second-save', amountCents: 98_765 }));
        await flush();
        expect(mockBudgetStore.saveBudget).toHaveBeenLastCalledWith({
            budget: expect.objectContaining({ id: 'second-save', amountCents: 98_765 }),
        });
    });

    test('reports readable and fallback save failures while releasing the sheet', async () => {
        const { bindings } = setup();
        const editSheet = { setSaving: jest.fn() };
        bindings.editSheetRef.value = editSheet;

        mockBudgetStore.saveBudget.mockRejectedValueOnce({ message: 'save failed' });
        bindings.onSheetSave(mockExpensePrimary);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('save failed');
        expect(editSheet.setSaving).toHaveBeenCalledWith(false);
        expect(mockLoggerError).toHaveBeenCalledWith(
            '[MobileBudgets] Failed to save budget',
            expect.objectContaining({ message: 'save failed' }),
        );

        mockBudgetStore.saveBudget.mockRejectedValueOnce(undefined);
        bindings.onSheetSave(mockExpensePrimary);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('Failed to save budget');
    });

    test('validates deletion, confirms it, and closes only the matching edit sheet', async () => {
        const { bindings } = setup();
        bindings.requestDelete(null);
        bindings.requestDelete(createBudget({ id: '' }));
        expect(bindings.showDeleteActionSheet.value).toBe(false);

        bindings.requestDelete(mockExpensePrimary);
        expect(bindings.budgetToDelete.value.id).toBe('food-primary');
        expect(bindings.budgetToDelete.value.amountCents).toBe(12_345);
        expect(bindings.showDeleteActionSheet.value).toBe(true);

        bindings.editingBudget.value = Object.assign(new Budget(), mockExpensePrimary);
        bindings.showEditSheet.value = true;
        bindings.confirmDelete();
        expect(mockShowLoading).toHaveBeenCalled();
        expect(mockBudgetStore.deleteBudget).toHaveBeenCalledWith({ budgetId: 'food-primary' });
        await flush();
        expect(bindings.budgetToDelete.value).toBeNull();
        expect(bindings.showEditSheet.value).toBe(false);
        expect(bindings.editingBudget.value).toBeNull();
        expect(mockShowToast).toHaveBeenCalledWith('Budget has been deleted');

        bindings.confirmDelete();
        expect(mockBudgetStore.deleteBudget).toHaveBeenCalledTimes(1);

        bindings.requestDelete(mockExpenseSub);
        bindings.editingBudget.value = Object.assign(new Budget(), mockInvestment);
        bindings.showEditSheet.value = true;
        bindings.confirmDelete();
        await flush();
        expect(bindings.showEditSheet.value).toBe(true);
        expect(bindings.editingBudget.value.id).toBe('fund-primary');
    });

    test('reports readable and fallback delete failures without converting amounts', async () => {
        const { bindings } = setup();
        bindings.requestDelete(mockInvestment);
        mockBudgetStore.deleteBudget.mockRejectedValueOnce({ message: 'delete failed' });
        bindings.confirmDelete();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('delete failed');
        expect(mockLoggerError).toHaveBeenCalledWith(
            '[MobileBudgets] Failed to delete budget',
            expect.objectContaining({ message: 'delete failed' }),
        );
        expect(mockInvestment.amountCents).toBe(98_765);

        bindings.requestDelete(mockInvestment);
        mockBudgetStore.deleteBudget.mockRejectedValueOnce(undefined);
        bindings.confirmDelete();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('Failed to delete budget');
    });

    test('loads current monthly data sequentially and invokes only function PTR callbacks', async () => {
        const { bindings } = setup();
        const done = jest.fn();
        const pending = deferred<void>();
        mockBudgetStore.loadBudgetExecution.mockReturnValueOnce(pending.promise);

        const promise = bindings.reload(done);
        expect(bindings.loading.value).toBe(true);
        await flush(2);
        expect(mockCategoriesStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(mockBudgetStore.loadAllBudgets).toHaveBeenCalledWith({
            force: true,
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly,
        });
        expect(mockBudgetStore.loadBudgetExecution).toHaveBeenCalledWith(expect.objectContaining({
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly,
            year: new Date().getFullYear(),
            month: new Date().getMonth() + 1,
        }));
        expect(bindings.loading.value).toBe(true);
        pending.resolve(undefined);
        await promise;
        expect(bindings.loading.value).toBe(false);
        expect(done).toHaveBeenCalled();

        await bindings.reload(false);
        expect(bindings.loading.value).toBe(false);
    });

    test('handles list and execution failures and page-entry reloads', async () => {
        const { bindings } = setup();
        const done = jest.fn();
        mockBudgetStore.loadAllBudgets.mockRejectedValueOnce(new Error('list failed'));
        await bindings.reload(done);
        expect(bindings.loading.value).toBe(false);
        expect(done).toHaveBeenCalled();
        expect(mockLoggerError).toHaveBeenCalledWith(
            '[MobileBudgets] Failed to load budgets',
            expect.objectContaining({ message: 'list failed' }),
        );

        mockBudgetStore.loadBudgetExecution.mockRejectedValueOnce('execution failed');
        await bindings.reload(false);
        expect(mockLoggerError).toHaveBeenCalledWith(
            '[MobileBudgets] Failed to load budgets',
            'execution failed',
        );

        mockBudgetStore.loadAllBudgets.mockClear();
        bindings.onPageAfterIn();
        await flush();
        expect(mockBudgetStore.loadAllBudgets).toHaveBeenCalled();
    });
});

describe('mobile budget ListPage production template', () => {
    test('renders loading, empty, grouped, collapse, sheet, and event-wrapper branches', async () => {
        const mounted = mountWithHostRenderer();
        try {
            await actualVue.nextTick();
            mounted.state.loading = false;
            mounted.state.budgetToDelete = mockExpensePrimary;
            mounted.state.showDeleteActionSheet = true;
            mounted.state.showEditSheet = true;
            await actualVue.nextTick();
            const callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.some(item => item.name === 'onPtr:refresh')).toBe(true);
            expect(callbacks.some(item => item.name === 'onPage:afterin')).toBe(true);
            expect(callbacks.some(item => item.name === 'onSave')).toBe(true);
            expect(callbacks.some(item => item.name === 'onDelete:request')).toBe(true);
            expect(callbacks.some(item => item.name === 'onClick')).toBe(true);
            await invokeHostCallbacks(callbacks);

            expect(mockFormatBudgetAmount).toHaveBeenCalledWith(123.45);
            expect(mockFormatBudgetAmount).toHaveBeenCalledWith(23.45);
            expect(mockFormatBudgetAmount).not.toHaveBeenCalledWith(1.2345);

            mounted.state.collapsedBudgetGroups = new Set(['Food', 'Travel', 'Misc']);
            await actualVue.nextTick();
            mounted.state.activeBudgetType = BudgetType.Investment;
            await actualVue.nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);

            mounted.state.loading = true;
            await actualVue.nextTick();
            mounted.state.loading = false;
            mockBudgetStore.investmentBudgets = [];
            await actualVue.nextTick();
        } finally {
            mounted.app.unmount();
        }
    });
});
