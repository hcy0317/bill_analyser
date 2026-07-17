import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockSetStatisticsDefaultFilter = jest.fn();
const mockSetOverviewFilter = jest.fn();
const mockUpdateStatisticsFilter = jest.fn<(payload: unknown) => boolean>(() => true);
const mockUpdateTransactionListFilter = jest.fn<(payload: unknown) => boolean>(() => true);
const mockUpdateTransactionListInvalidState = jest.fn();
const mockUpdateOverviewInvalidState = jest.fn();
const mockAllWithVisibleCount = jest.fn((categories: unknown, allowed: unknown) => ({ categories, allowed }));
const mockContainsAnyAvailable = jest.fn((_categories: unknown, showHidden: boolean) => showHidden);
const mockContainsAvailable = jest.fn((_categories: unknown, showHidden: boolean) => ({
    1: showHidden,
    2: true
}));
const mockSelectAllSubCategories = jest.fn((target: Record<string, boolean>, value: boolean, category: any) => {
    target[category.id] = value;
    for (const child of category.subCategories ?? []) target[child.id] = value;
});
const mockAllChecked = jest.fn((category: any, filtered: Record<string, boolean>) => !filtered[category.id]);

const mockSettingsStore = actualVue.reactive({
    appSettings: {
        statistics: { defaultTransactionCategoryFilter: {} as Record<string, boolean> },
        overviewTransactionCategoryFilterInHomePage: {} as Record<string, boolean>
    },
    setStatisticsDefaultTransactionCategoryFilter: mockSetStatisticsDefaultFilter,
    setOverviewTransactionCategoryFilterInHomePage: mockSetOverviewFilter
});
const mockTransactionsStore = actualVue.reactive({
    allFilterCategoryIdsCount: 0,
    allFilterCategoryIds: {} as Record<string, boolean>,
    updateTransactionListFilter: mockUpdateTransactionListFilter,
    updateTransactionListInvalidState: mockUpdateTransactionListInvalidState
});
const mockStatisticsStore = actualVue.reactive({
    transactionStatisticsFilter: { filterCategoryIds: {} as Record<string, boolean> },
    updateTransactionStatisticsFilter: mockUpdateStatisticsFilter
});
const mockOverviewStore = {
    updateTransactionOverviewInvalidState: mockUpdateOverviewInvalidState
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockOverviewStore }));
jest.mock('@/lib/category.ts', () => ({
    allTransactionCategoriesWithVisibleCount: (categories: unknown, allowed: unknown) => (
        mockAllWithVisibleCount(categories, allowed)
    ),
    containsAnyAvailableCategory: (...args: [unknown, boolean]) => mockContainsAnyAvailable(...args),
    containsAvailableCategory: (...args: [unknown, boolean]) => mockContainsAvailable(...args),
    selectAllSubCategories: (
        target: Record<string, boolean>,
        value: boolean,
        category: unknown
    ) => mockSelectAllSubCategories(target, value, category),
    isCategoryOrSubCategoriesAllChecked: (category: unknown, filtered: Record<string, boolean>) => (
        mockAllChecked(category, filtered)
    )
}));

import { CategoryType } from '@/core/category.ts';

const income = { id: 'income', type: CategoryType.Income, subCategories: [] };
const food = { id: 'food', type: CategoryType.Expense, parentId: 'expense', subCategories: [] };
const expense = { id: 'expense', type: CategoryType.Expense, subCategories: [food] };
const transfer = { id: 'transfer', type: CategoryType.Transfer, subCategories: [] };
const mockCategoryStore = actualVue.reactive({
    allTransactionCategories: {
        [CategoryType.Income]: [income],
        [CategoryType.Expense]: [expense],
        [CategoryType.Transfer]: [transfer]
    },
    allTransactionCategoriesMap: {
        income,
        expense,
        food,
        transfer
    } as Record<string, any>
});

jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoryStore
}));

import {
    useCategoryFilterSettingPageBase,
    type CategoryFilterType
} from '@/views/base/settings/CategoryFilterSettingPageBase.ts';

beforeEach(() => {
    jest.clearAllMocks();
    mockSettingsStore.appSettings.statistics.defaultTransactionCategoryFilter = {};
    mockSettingsStore.appSettings.overviewTransactionCategoryFilterInHomePage = {};
    mockStatisticsStore.transactionStatisticsFilter.filterCategoryIds = {};
    mockTransactionsStore.allFilterCategoryIdsCount = 0;
    mockTransactionsStore.allFilterCategoryIds = {};
    mockUpdateStatisticsFilter.mockReturnValue(true);
    mockUpdateTransactionListFilter.mockReturnValue(true);
});

describe('category filter base projections', () => {
    test('derives labels, availability, allowed types, and category selection state', () => {
        const defaults = useCategoryFilterSettingPageBase(
            'statisticsDefault',
            `${CategoryType.Income},${CategoryType.Expense}`
        );
        expect(defaults.title.value).toBe('Default Transaction Category Filter');
        expect(defaults.applyText.value).toBe('Save');
        expect(defaults.loading.value).toBe(true);
        expect(defaults.hasAnyAvailableCategory.value).toBe(true);
        expect(defaults.hasAnyVisibleCategory.value).toBe(false);
        expect(defaults.hasAvailableCategory.value).toStrictEqual({ 1: false, 2: true });
        expect(mockAllWithVisibleCount).toHaveBeenCalledWith(
            mockCategoryStore.allTransactionCategories,
            {
                [CategoryType.Income]: true,
                [CategoryType.Expense]: true
            }
        );

        defaults.showHidden.value = true;
        expect(defaults.hasAnyVisibleCategory.value).toBe(true);
        expect(defaults.hasAvailableCategory.value).toStrictEqual({ 1: true, 2: true });
        expect(defaults.isCategoryChecked(income as never, { income: false })).toBe(true);
        expect(defaults.isCategoryChecked(income as never, { income: true })).toBe(false);
        expect(defaults.getCategoryTypeName(CategoryType.Income)).toBe('tt:Income Categories');
        expect(defaults.getCategoryTypeName(CategoryType.Expense)).toBe('tt:Expense Categories');
        expect(defaults.getCategoryTypeName(CategoryType.Transfer)).toBe('tt:Transfer Categories');
        expect(defaults.getCategoryTypeName(999 as CategoryType)).toBe('tt:Transaction Categories');

        const regular = useCategoryFilterSettingPageBase('homePageOverview');
        expect(regular.title.value).toBe('Filter Transaction Categories');
        expect(regular.applyText.value).toBe('Apply');
    });
});

describe('category filter base loading', () => {
    test('loads default, current statistics, and home filters over the full category map', () => {
        mockSettingsStore.appSettings.statistics.defaultTransactionCategoryFilter = { expense: true };
        const defaults = useCategoryFilterSettingPageBase('statisticsDefault');
        expect(defaults.loadFilterCategoryIds()).toBe(true);
        expect(defaults.filterCategoryIds.value).toStrictEqual({
            income: false,
            expense: true,
            food: false,
            transfer: false
        });

        mockStatisticsStore.transactionStatisticsFilter.filterCategoryIds = { income: true };
        const current = useCategoryFilterSettingPageBase('statisticsCurrent');
        expect(current.loadFilterCategoryIds()).toBe(true);
        expect(current.filterCategoryIds.value['income']).toBe(true);

        mockSettingsStore.appSettings.overviewTransactionCategoryFilterInHomePage = { transfer: true };
        const home = useCategoryFilterSettingPageBase('homePageOverview');
        expect(home.loadFilterCategoryIds()).toBe(true);
        expect(home.filterCategoryIds.value['transfer']).toBe(true);
    });

    test('loads current transaction filters for leaf, parent, missing, and empty selections', () => {
        mockTransactionsStore.allFilterCategoryIdsCount = 3;
        mockTransactionsStore.allFilterCategoryIds = {
            income: true,
            expense: true,
            missing: true
        };
        const current = useCategoryFilterSettingPageBase('transactionListCurrent');
        expect(current.loadFilterCategoryIds()).toBe(true);
        expect(current.filterCategoryIds.value).toStrictEqual({
            income: false,
            expense: false,
            food: false,
            transfer: true
        });
        expect(mockSelectAllSubCategories).toHaveBeenCalledWith(
            expect.any(Object),
            false,
            expense
        );

        mockTransactionsStore.allFilterCategoryIdsCount = 0;
        mockTransactionsStore.allFilterCategoryIds = {};
        const empty = useCategoryFilterSettingPageBase('transactionListCurrent');
        expect(empty.loadFilterCategoryIds()).toBe(true);
        expect(empty.filterCategoryIds.value).toStrictEqual({
            income: false,
            expense: false,
            food: false,
            transfer: false
        });
    });

    test('skips disallowed category types and rejects an unknown load target', () => {
        const allowed = useCategoryFilterSettingPageBase(
            'statisticsDefault',
            String(CategoryType.Income)
        );
        expect(allowed.loadFilterCategoryIds()).toBe(true);
        expect(allowed.filterCategoryIds.value).toStrictEqual({ income: false });

        const unknown = useCategoryFilterSettingPageBase(undefined);
        expect(unknown.loadFilterCategoryIds()).toBe(false);
    });
});

describe('category filter base saving', () => {
    function createBase(type?: CategoryFilterType) {
        const base = useCategoryFilterSettingPageBase(type);
        base.filterCategoryIds.value = {
            income: false,
            expense: true,
            food: false,
            transfer: false,
            ghost: true
        };
        return base;
    }

    test('persists default and current statistics filters and returns the store change result', () => {
        const defaults = createBase('statisticsDefault');
        expect(defaults.saveFilterCategoryIds()).toBe(true);
        expect(mockSetStatisticsDefaultFilter).toHaveBeenCalledWith({ expense: true });

        mockUpdateStatisticsFilter.mockReturnValue(false);
        const current = createBase('statisticsCurrent');
        expect(current.saveFilterCategoryIds()).toBe(false);
        expect(mockUpdateStatisticsFilter).toHaveBeenCalledWith({
            filterCategoryIds: { expense: true }
        });
    });

    test('persists home filters and invalidates the overview cache', () => {
        const home = createBase('homePageOverview');
        expect(home.saveFilterCategoryIds()).toBe(true);
        expect(mockSetOverviewFilter).toHaveBeenCalledWith({ expense: true });
        expect(mockUpdateOverviewInvalidState).toHaveBeenCalledWith(true);
    });

    test('writes transaction category ids, including all-selected and unchanged branches', () => {
        const current = createBase('transactionListCurrent');
        expect(current.saveFilterCategoryIds()).toBe(true);
        expect(mockUpdateTransactionListFilter).toHaveBeenCalledWith({
            categoryIds: 'income,food,transfer'
        });
        expect(mockUpdateTransactionListInvalidState).toHaveBeenCalledWith(true);

        mockUpdateTransactionListFilter.mockClear();
        mockUpdateTransactionListInvalidState.mockClear();
        const all = useCategoryFilterSettingPageBase('transactionListCurrent');
        all.filterCategoryIds.value = {
            income: false,
            expense: false,
            food: false,
            transfer: false
        };
        expect(all.saveFilterCategoryIds()).toBe(true);
        expect(mockUpdateTransactionListFilter).toHaveBeenCalledWith({ categoryIds: '' });

        mockUpdateTransactionListFilter.mockReturnValue(false);
        expect(all.saveFilterCategoryIds()).toBe(false);
        expect(mockUpdateTransactionListInvalidState).toHaveBeenCalledTimes(1);
    });

    test('keeps the default changed result for an unspecified save target', () => {
        const unknown = createBase(undefined);
        expect(unknown.saveFilterCategoryIds()).toBe(true);
    });
});
