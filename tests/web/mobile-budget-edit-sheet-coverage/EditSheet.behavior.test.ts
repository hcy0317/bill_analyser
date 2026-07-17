import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import {
    collectHostCallbacks,
    mountWithHostRenderer,
} from '../coverage-auth-mobile-batch1/hostRenderer';
import EditSheetComponent from '@/views/mobile/budgets/EditSheet.vue';

const actualVue = jest.requireActual('vue') as any;
const actualBudgetModule = jest.requireActual('@/models/budget.ts') as any;
const actualCategoryModule = jest.requireActual('@/core/category.ts') as any;
const { Budget, BudgetPeriodType, BudgetType } = actualBudgetModule;
const { CategoryType } = actualCategoryModule;

const mockLoggerWarn = jest.fn<(...args: any[]) => void>();

function createCategory(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'food',
        name: 'Food',
        parentId: '',
        type: CategoryType.Expense,
        icon: 'fork_knife',
        color: '#00aa66',
        hidden: false,
        subCategories: [],
        ...overrides,
    };
}

const diningCategory = createCategory({
    id: 'food-dining',
    name: 'Dining',
    parentId: 'food',
    subCategories: undefined,
});
const foodCategory = createCategory({ subCategories: [diningCategory] });
const travelCategory = createCategory({
    id: 'travel',
    name: 'Travel',
    subCategories: undefined,
});
const fundCategory = createCategory({
    id: 'fund',
    name: 'Fund',
    type: CategoryType.Investment,
    color: '#3366ff',
    subCategories: undefined,
});

const mockCategoryStore = actualVue.reactive({
    allTransactionCategories: {
        [CategoryType.Expense]: [foodCategory, travelCategory],
        [CategoryType.Investment]: [fundCategory],
    } as Record<number, any[]>,
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>(),
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoryStore,
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { warn: (...args: any[]) => mockLoggerWarn(...args) },
}));

const EditSheet = EditSheetComponent as any;

function createBudget(overrides: Record<string, unknown> = {}): any {
    return Object.assign(new Budget(), {
        id: 'budget-1',
        name: 'Dining plan',
        category: 'Food',
        subCategory: 'Dining',
        categoryId: 'food-dining',
        periodType: BudgetPeriodType.Monthly,
        amountCents: 12_345,
        type: BudgetType.Expense,
        ...overrides,
    });
}

function setup(overrides: Record<string, unknown> = {}): {
    bindings: any;
    emit: jest.Mock;
    exposed: Record<string, any>;
    props: Record<string, any>;
} {
    const props = actualVue.reactive({
        show: true,
        budget: null,
        defaultType: undefined,
        ...overrides,
    });
    const emit = jest.fn();
    const exposed: Record<string, any> = {};
    const bindings = EditSheet.setup(props, {
        attrs: {},
        slots: {},
        emit,
        expose: (value: Record<string, any>) => Object.assign(exposed, value),
    });
    return { bindings, emit, exposed, props };
}

async function flush(times = 4): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockCategoryStore.allTransactionCategories = {
        [CategoryType.Expense]: [foodCategory, travelCategory],
        [CategoryType.Investment]: [fundCategory],
    };
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
});

describe('mobile budget EditSheet initialization and derived state', () => {
    test('initializes a new budget with the requested type and exposes all period options', () => {
        const { bindings } = setup({ defaultType: BudgetType.Investment });

        expect(bindings.form.value).toBeInstanceOf(Budget);
        expect(bindings.form.value.type).toBe(BudgetType.Investment);
        expect(bindings.isNew.value).toBe(true);
        expect(bindings.amountInYuanInput.value).toBe('');
        expect(bindings.availableCategories.value).toStrictEqual([fundCategory]);
        expect(bindings.periodTypeOptions.value).toStrictEqual([
            { value: BudgetPeriodType.Monthly, text: 'tt:Monthly' },
            { value: BudgetPeriodType.Quarterly, text: 'tt:Quarterly' },
            { value: BudgetPeriodType.Yearly, text: 'tt:Yearly' },
        ]);
        expect(bindings.periodTypeLabel.value).toBe('tt:Monthly');
        expect(bindings.selectedCategoryDisplay.value).toBe('tt:Select Category');
        expect(bindings.canSave.value).toBe(false);
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledWith({ force: false });

        bindings.form.value.periodType = BudgetPeriodType.Daily;
        expect(bindings.periodTypeLabel.value).toBe('');
        mockCategoryStore.allTransactionCategories = {};
        expect(bindings.availableCategories.value).toStrictEqual([]);
    });

    test('clones an existing budget, formats cents as yuan, and resolves category labels', () => {
        const source = createBudget();
        const { bindings } = setup({ budget: source });

        expect(bindings.form.value).not.toBe(source);
        expect(bindings.form.value).toMatchObject({
            id: 'budget-1',
            amountCents: 12_345,
            categoryId: 'food-dining',
        });
        expect(bindings.amountInYuanInput.value).toBe('123.45');
        expect(bindings.isNew.value).toBe(false);
        expect(bindings.selectedCategoryDisplay.value).toBe('Food / Dining');
        expect(bindings.canSave.value).toBe(true);

        bindings.form.value.categoryId = 'food';
        expect(bindings.selectedCategoryDisplay.value).toBe('Food');
        bindings.form.value.categoryId = 'missing';
        expect(bindings.selectedCategoryDisplay.value).toBe('tt:Select Category');
        bindings.form.value.categoryId = '';
        bindings.form.value.category = 'Legacy category';
        expect(bindings.canSave.value).toBe(true);
        bindings.form.value.amountCents = 0;
        expect(bindings.canSave.value).toBe(false);
    });

    test('reinitializes only when reopened and falls back to expense when no default is supplied', async () => {
        const first = createBudget({ id: 'first', amountCents: 1_001 });
        const second = createBudget({
            id: 'second',
            amountCents: 9_876,
            type: BudgetType.Investment,
            category: 'Fund',
            subCategory: '',
            categoryId: 'fund',
        });
        const { bindings, props } = setup({ show: false, budget: first });

        expect(bindings.form.value.id).toBe('');
        props['budget'] = second;
        await flush();
        expect(bindings.form.value.id).toBe('');

        props['show'] = true;
        await flush();
        expect(bindings.form.value).toMatchObject({ id: 'second', amountCents: 9_876 });
        expect(bindings.amountInYuanInput.value).toBe('98.76');

        props['show'] = false;
        props['budget'] = null;
        props['defaultType'] = undefined;
        await flush();
        props['show'] = true;
        await flush();
        expect(bindings.form.value.type).toBe(BudgetType.Expense);
        expect(bindings.amountInYuanInput.value).toBe('');
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledTimes(2);
    });
});

describe('mobile budget EditSheet input, save, and delete behavior', () => {
    test('switches new-budget type, clears stale category data, and locks persisted or saving forms', () => {
        const { bindings } = setup();
        bindings.form.value.categoryId = 'food';
        bindings.form.value.category = 'Food';
        bindings.form.value.subCategory = 'Dining';

        bindings.setType(BudgetType.Investment);
        expect(bindings.form.value).toMatchObject({
            type: BudgetType.Investment,
            categoryId: '',
            category: '',
            subCategory: '',
        });
        expect(bindings.availableCategories.value).toStrictEqual([fundCategory]);

        bindings.form.value.categoryId = 'fund';
        bindings.setType(BudgetType.Investment);
        expect(bindings.form.value.categoryId).toBe('fund');
        bindings.form.value.id = 'persisted';
        bindings.setType(BudgetType.Expense);
        expect(bindings.form.value.type).toBe(BudgetType.Investment);
        bindings.form.value.id = '';
        bindings.saving.value = true;
        bindings.setType(BudgetType.Expense);
        expect(bindings.form.value.type).toBe(BudgetType.Investment);
    });

    test.each([
        ['123.45', 12_345],
        ['0.005', 1],
        ['0', 0],
        ['', 0],
        ['not-a-number', 0],
        ['-1.25', 0],
    ])('converts yuan input %s to integer cents %s', (raw, expectedCents) => {
        const { bindings } = setup();
        bindings.onAmountInput({ target: { value: raw } });
        expect(bindings.amountInYuanInput.value).toBe(raw);
        expect(bindings.form.value.amountCents).toBe(expectedCents);
        expect(Number.isInteger(bindings.form.value.amountCents)).toBe(true);
    });

    test('saves primary and secondary categories with exact cents and blocks repeated saves', () => {
        const primary = setup();
        primary.bindings.form.value.amountCents = 12_345;
        primary.bindings.form.value.categoryId = 'food';
        primary.bindings.onSave();
        expect(primary.emit).toHaveBeenCalledWith('save', expect.objectContaining({
            categoryId: 'food',
            category: 'Food',
            subCategory: '',
            amountCents: 12_345,
        }));
        expect(primary.bindings.saving.value).toBe(true);
        primary.bindings.onSave();
        expect(primary.emit).toHaveBeenCalledTimes(1);

        const secondary = setup();
        secondary.bindings.form.value.amountCents = 98_765;
        secondary.bindings.form.value.categoryId = 'food-dining';
        secondary.bindings.onSave();
        expect(secondary.emit).toHaveBeenCalledWith('save', expect.objectContaining({
            categoryId: 'food-dining',
            category: 'Food',
            subCategory: 'Dining',
            amountCents: 98_765,
        }));
        expect(secondary.bindings.form.value.amountCents).not.toBe(987.65);
    });

    test('rejects invalid forms and unresolved category ids without emitting', () => {
        const invalid = setup();
        invalid.bindings.onSave();
        invalid.bindings.saving.value = true;
        invalid.bindings.onSave();
        expect(invalid.emit).not.toHaveBeenCalled();

        const unresolved = setup();
        unresolved.bindings.form.value.amountCents = 500;
        unresolved.bindings.form.value.categoryId = 'missing';
        unresolved.bindings.onSave();
        expect(mockLoggerWarn).toHaveBeenCalledWith(
            '[MobileBudgetEditSheet] Category is required',
        );
        expect(unresolved.emit).not.toHaveBeenCalled();

        unresolved.bindings.form.value.categoryId = '';
        unresolved.bindings.syncCategoryNamesFromId();
        expect(unresolved.bindings.form.value.category).toBe('');
    });

    test('closes through both sheet paths, exposes saving control, and guards deletion', () => {
        const visible = setup({ budget: createBudget() });
        visible.bindings.closeSheet();
        visible.bindings.onSheetClosed();
        expect(visible.emit).toHaveBeenNthCalledWith(1, 'update:show', false);
        expect(visible.emit).toHaveBeenNthCalledWith(2, 'update:show', false);

        visible.exposed['setSaving'](true);
        visible.bindings.onDeleteRequested();
        expect(visible.emit).toHaveBeenCalledTimes(2);
        visible.exposed['setSaving'](false);
        visible.bindings.onDeleteRequested();
        expect(visible.emit).toHaveBeenLastCalledWith(
            'delete:request',
            expect.objectContaining({ id: 'budget-1', amountCents: 12_345 }),
        );

        const hiddenNew = setup({ show: false });
        hiddenNew.bindings.onSheetClosed();
        hiddenNew.bindings.onDeleteRequested();
        expect(hiddenNew.emit).not.toHaveBeenCalled();
    });
});

describe('mobile budget EditSheet production template', () => {
    test('renders create/edit branches and executes the real template event wrappers', async () => {
        const mounted = mountWithHostRenderer(
            EditSheet,
            { show: true, budget: null, defaultType: BudgetType.Expense },
            [
                'f7-sheet',
                'f7-page-content',
                'f7-list',
                'f7-list-item',
                'f7-segmented',
                'f7-button',
                'f7-list-input',
                'f7-link',
                'list-item-selection-sheet',
                'tree-view-selection-sheet',
            ],
        );
        try {
            await actualVue.nextTick();
            const createCallbacks = collectHostCallbacks(mounted.root);
            expect(createCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onSheet:closed',
                'onClick',
                'onInput',
                'onUpdate:show',
                'onUpdate:modelValue',
            ]));

            for (const { name, callback } of createCallbacks) {
                if (name === 'onInput') {
                    callback({ target: { value: '123.45' } });
                    callback({ target: { value: '' } });
                    callback({ target: { value: '123.45' } });
                } else if (name === 'onClick') {
                    callback({ type: 'synthetic-click' });
                } else if (name.startsWith('onUpdate:')) {
                    callback(false);
                }
                await actualVue.nextTick();
            }
            expect(mounted.state.amountInYuanInput).toBe('123.45');
            expect(mounted.state.form.amountCents).toBe(12_345);

            mounted.state.form = createBudget();
            mounted.state.saving = true;
            mounted.state.showPeriodSheet = true;
            mounted.state.showCategorySheet = true;
            await actualVue.nextTick();
            const editCallbacks = collectHostCallbacks(mounted.root);
            expect(editCallbacks.filter(item => item.name === 'onClick').length).toBeGreaterThan(4);
            expect(mounted.root.children.length).toBeGreaterThan(0);

            mounted.state.saving = false;
            await actualVue.nextTick();
            for (const { name, callback } of collectHostCallbacks(mounted.root)) {
                if (name === 'onSheet:closed' || name === 'onClick') {
                    callback({ type: 'synthetic-template-event' });
                }
            }
        } finally {
            mounted.app.unmount();
        }
    });
});
