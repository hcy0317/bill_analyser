/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const { proxyRefs, reactive, ref } = actualVue;

const mockShowToast = jest.fn();
const mockRouteBackOnError = jest.fn();
const mockLoadAllCategories = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadAllTags = jest.fn<(...args: any[]) => Promise<void>>();

const mockSelectAllSubCategories = jest.fn();
const mockSelectAllCategories = jest.fn();
const mockSelectNoneCategories = jest.fn();
const mockSelectInvertCategories = jest.fn();
const mockSelectAllVisibleCategories = jest.fn();
const mockIsSubCategoriesAllChecked = jest.fn<(...args: any[]) => boolean>(() => false);
const mockIsSubCategoriesHasButNotAllChecked = jest.fn<(...args: any[]) => boolean>(() => false);

const mockSelectAllTags = jest.fn();
const mockSelectNoneTags = jest.fn();
const mockSelectInvertTags = jest.fn();
const mockSelectAllVisibleTags = jest.fn();

const categoryBaseStates: any[] = [];
const tagBaseStates: any[] = [];
let nextCategoryFilterLoadResult = true;
let nextTagFilterLoadResult = true;

const expenseCategory = {
    id: 'food', name: 'Food', icon: '1', color: '#f00', hidden: false,
};
const hiddenExpenseCategory = {
    id: 'hidden-food', name: 'Hidden Food', icon: '2', color: '#999', hidden: true,
};
const expenseSubCategory = {
    id: 'dining', name: 'Dining', icon: '3', color: '#0f0', hidden: false,
};

const categoryTypeRows = [
    {
        type: 1,
        allCategories: [expenseCategory, hiddenExpenseCategory],
        allSubCategories: { food: [expenseSubCategory], 'hidden-food': [] },
        allVisibleSubCategoryCounts: { food: 1, 'hidden-food': 0 },
    },
    {
        type: 2,
        allCategories: [],
        allSubCategories: {},
        allVisibleSubCategoryCounts: {},
    },
    {
        type: 3,
        allCategories: [],
        allSubCategories: {},
        allVisibleSubCategoryCounts: {},
    },
];

const categoryStore = {
    allTransactionCategoriesMap: {
        food: expenseCategory,
        'hidden-food': hiddenExpenseCategory,
        dining: expenseSubCategory,
    },
    loadAllCategories: (options: unknown) => mockLoadAllCategories(options),
};

const tagRows = [
    { id: 'work', name: 'Work', hidden: false },
    { id: 'secret', name: 'Secret', hidden: true },
];
const tagStore = {
    allTransactionTagsMap: { work: tagRows[0], secret: tagRows[1] },
    loadAllTags: (options: unknown) => mockLoadAllTags(options),
};

function createCategoryBaseState(): any {
    const state = {
        loading: ref(true),
        showHidden: ref(false),
        filterCategoryIds: ref({} as Record<string, boolean>),
        title: ref('Category Filter'),
        applyText: ref('Apply'),
        allTransactionCategories: ref(categoryTypeRows),
        hasAnyAvailableCategory: ref(true),
        hasAnyVisibleCategory: ref(true),
        hasAvailableCategory: reactive({ 1: true, 2: false, 3: false }),
        isCategoryChecked: jest.fn(() => true),
        getCategoryTypeName: jest.fn((type: number) => `category-type:${type}`),
        loadFilterCategoryIds: jest.fn(() => nextCategoryFilterLoadResult),
        saveFilterCategoryIds: jest.fn(),
    };
    categoryBaseStates.push(state);
    return state;
}

function createTagBaseState(): any {
    const state = {
        loading: ref(true),
        showHidden: ref(false),
        filterTagIds: ref({} as Record<string, boolean>),
        tagFilterType: ref('include'),
        title: ref('Tag Filter'),
        applyText: ref('Apply'),
        allTags: ref(tagRows),
        allTagFilterTypes: ref([
            { type: 'include', displayName: 'Include' },
            { type: 'exclude', displayName: 'Exclude' },
        ]),
        hasAnyAvailableTag: ref(true),
        hasAnyVisibleTag: ref(true),
        loadFilterTagIds: jest.fn(() => nextTagFilterLoadResult),
        saveFilterTagIds: jest.fn(),
    };
    tagBaseStates.push(state);
    return state;
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showToast: (...args: unknown[]) => mockShowToast(...args),
        routeBackOnError: (...args: unknown[]) => mockRouteBackOnError(...args),
    }),
}));
jest.mock('@/views/base/settings/CategoryFilterSettingPageBase.ts', () => ({
    useCategoryFilterSettingPageBase: jest.fn(() => createCategoryBaseState()),
}));
jest.mock('@/views/base/settings/TransactionTagFilterSettingPageBase.ts', () => ({
    useTransactionTagFilterSettingPageBase: jest.fn(() => createTagBaseState()),
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => categoryStore,
}));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => tagStore }));
jest.mock('@/core/category.ts', () => ({
    CategoryType: { Income: 1, Expense: 2, Transfer: 3 },
}));
jest.mock('@/lib/category.ts', () => ({
    selectAllSubCategories: (...args: unknown[]) => mockSelectAllSubCategories(...args),
    selectAllVisible: (...args: unknown[]) => mockSelectAllVisibleCategories(...args),
    selectAll: (...args: unknown[]) => mockSelectAllCategories(...args),
    selectNone: (...args: unknown[]) => mockSelectNoneCategories(...args),
    selectInvert: (...args: unknown[]) => mockSelectInvertCategories(...args),
    isSubCategoriesAllChecked: (...args: unknown[]) => mockIsSubCategoriesAllChecked(...args),
    isSubCategoriesHasButNotAllChecked: (...args: unknown[]) => mockIsSubCategoriesHasButNotAllChecked(...args),
}));
jest.mock('@/lib/common.ts', () => ({
    selectAllVisible: (...args: unknown[]) => mockSelectAllVisibleTags(...args),
    selectAll: (...args: unknown[]) => mockSelectAllTags(...args),
    selectNone: (...args: unknown[]) => mockSelectNoneTags(...args),
    selectInvert: (...args: unknown[]) => mockSelectInvertTags(...args),
}));

const CategoryFilterSettingsPage = require(
    '@/views/mobile/settings/CategoryFilterSettingsPage.vue'
).default as any;
const TransactionTagFilterSettingsPage = require(
    '@/views/mobile/settings/TransactionTagFilterSettingsPage.vue'
).default as any;

function props(type = 'statisticsCurrent'): any {
    return {
        f7route: { query: { type, allowCategoryTypes: '1,2,3' } },
        f7router: { back: jest.fn() },
    };
}

function setup(component: any, componentProps = props()): { bindings: any; props: any } {
    const reactiveProps = reactive(componentProps);
    const bindings = component.setup(reactiveProps, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn(),
    });
    return { bindings, props: reactiveProps };
}

function render(component: any, bindings: any): any {
    return component.render({}, [], {}, proxyRefs(bindings), {}, {});
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

async function flushPromises(times = 8): Promise<void> {
    for (let index = 0; index < times; index += 1) await Promise.resolve();
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
    categoryBaseStates.length = 0;
    tagBaseStates.length = 0;
    nextCategoryFilterLoadResult = true;
    nextTagFilterLoadResult = true;
    mockLoadAllCategories.mockResolvedValue(undefined);
    mockLoadAllTags.mockResolvedValue(undefined);
});

describe('CategoryFilterSettingsPage production behavior', () => {
    test('loads, updates selection, applies bulk actions, saves, and routes back on errors', async () => {
        const pageProps = props();
        const page = setup(CategoryFilterSettingsPage, pageProps);
        await flushPromises();
        const base = categoryBaseStates[0];

        expect(mockLoadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(base.loading.value).toBe(false);

        page.bindings.updateCategorySelected({ target: { value: 'missing', checked: true } });
        expect(base.filterCategoryIds.value).toEqual({});
        page.bindings.updateCategorySelected({ target: { value: 'dining', checked: true } });
        expect(base.filterCategoryIds.value).toEqual({ dining: false });

        page.bindings.updateAllSubCategoriesSelected({ target: { value: 'food', checked: false } });
        expect(mockSelectAllSubCategories).toHaveBeenCalledWith(
            base.filterCategoryIds.value, true, expenseCategory,
        );
        page.bindings.selectAllCategories();
        page.bindings.selectNoneCategories();
        page.bindings.selectInvertCategories();
        page.bindings.selectAllVisibleCategories();
        expect(mockSelectAllCategories).toHaveBeenCalledWith(base.filterCategoryIds.value, categoryStore.allTransactionCategoriesMap);
        expect(mockSelectNoneCategories).toHaveBeenCalledWith(base.filterCategoryIds.value, categoryStore.allTransactionCategoriesMap);
        expect(mockSelectInvertCategories).toHaveBeenCalledWith(base.filterCategoryIds.value, categoryStore.allTransactionCategoriesMap);
        expect(mockSelectAllVisibleCategories).toHaveBeenCalledWith(base.filterCategoryIds.value, categoryStore.allTransactionCategoriesMap);

        page.bindings.save();
        expect(base.saveFilterCategoryIds).toHaveBeenCalledTimes(1);
        expect(pageProps.f7router.back).toHaveBeenCalledTimes(1);
        page.bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(pageProps.f7router, page.bindings.loadingError);

        base.showHidden.value = true;
        page.bindings.collapseStates.value[1].opened = false;
        const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
        collectCallbacks(render(CategoryFilterSettingsPage, page.bindings), callbacks);
        expect(callbacks.length).toBeGreaterThan(8);
        for (const entry of callbacks) {
            if (entry.name.toLowerCase().includes('change')) {
                entry.callback({ target: { value: 'food', checked: false } });
            } else {
                entry.callback({});
            }
        }
    });

    test('rejects invalid filter parameters after a successful category load', async () => {
        nextCategoryFilterLoadResult = false;
        const page = setup(CategoryFilterSettingsPage);
        await flushPromises();
        expect(page.bindings.loading.value).toBe(false);
        expect(page.bindings.loadingError.value).toBe('Parameter Invalid');
        expect(mockShowToast).toHaveBeenCalledWith('Parameter Invalid');
    });

    test('distinguishes processed and visible category load failures', async () => {
        mockLoadAllCategories.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processed = setup(CategoryFilterSettingsPage);
        await flushPromises();
        expect(processed.bindings.loading.value).toBe(false);
        expect(processed.bindings.loadingError.value).toBeNull();

        mockLoadAllCategories.mockRejectedValueOnce({ processed: false, message: 'category failed' });
        const visible = setup(CategoryFilterSettingsPage);
        await flushPromises();
        expect(visible.bindings.loadingError.value).toEqual({ processed: false, message: 'category failed' });
        expect(mockShowToast).toHaveBeenCalledWith('category failed');
    });
});

describe('TransactionTagFilterSettingsPage production behavior', () => {
    test('loads, updates tags, applies bulk actions, saves, and executes template events', async () => {
        const pageProps = props();
        const page = setup(TransactionTagFilterSettingsPage, pageProps);
        await flushPromises();
        const base = tagBaseStates[0];

        expect(mockLoadAllTags).toHaveBeenCalledWith({ force: false });
        expect(base.loading.value).toBe(false);
        page.bindings.updateTransactionTagSelected({ target: { value: 'missing', checked: true } });
        expect(base.filterTagIds.value).toEqual({});
        page.bindings.updateTransactionTagSelected({ target: { value: 'work', checked: false } });
        expect(base.filterTagIds.value).toEqual({ work: true });

        page.bindings.selectAllTransactionTags();
        page.bindings.selectNoneTransactionTags();
        page.bindings.selectInvertTransactionTags();
        page.bindings.selectAllVisibleTransactionTags();
        expect(mockSelectAllTags).toHaveBeenCalledWith(base.filterTagIds.value, tagStore.allTransactionTagsMap);
        expect(mockSelectNoneTags).toHaveBeenCalledWith(base.filterTagIds.value, tagStore.allTransactionTagsMap);
        expect(mockSelectInvertTags).toHaveBeenCalledWith(base.filterTagIds.value, tagStore.allTransactionTagsMap);
        expect(mockSelectAllVisibleTags).toHaveBeenCalledWith(base.filterTagIds.value, tagStore.allTransactionTagsMap);

        page.bindings.save();
        expect(base.saveFilterTagIds).toHaveBeenCalledTimes(1);
        expect(pageProps.f7router.back).toHaveBeenCalledTimes(1);
        page.bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(pageProps.f7router, page.bindings.loadingError);

        base.showHidden.value = true;
        page.bindings.collapseStates.value.default.opened = false;
        const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
        collectCallbacks(render(TransactionTagFilterSettingsPage, page.bindings), callbacks);
        expect(callbacks.length).toBeGreaterThan(8);
        for (const entry of callbacks) {
            if (entry.name.toLowerCase().includes('change')) {
                entry.callback({ target: { value: 'work', checked: false } });
            } else {
                entry.callback({});
            }
        }

        base.hasAnyVisibleTag.value = false;
        expect(render(TransactionTagFilterSettingsPage, page.bindings)).toBeDefined();
    });

    test('rejects invalid tag filter parameters after load', async () => {
        nextTagFilterLoadResult = false;
        const page = setup(TransactionTagFilterSettingsPage);
        await flushPromises();
        expect(page.bindings.loading.value).toBe(false);
        expect(page.bindings.loadingError.value).toBe('Parameter Invalid');
        expect(mockShowToast).toHaveBeenCalledWith('Parameter Invalid');
    });

    test('distinguishes processed and visible tag load failures', async () => {
        mockLoadAllTags.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processed = setup(TransactionTagFilterSettingsPage);
        await flushPromises();
        expect(processed.bindings.loading.value).toBe(false);
        expect(processed.bindings.loadingError.value).toBeNull();

        mockLoadAllTags.mockRejectedValueOnce({ processed: false, message: 'tag failed' });
        const visible = setup(TransactionTagFilterSettingsPage);
        await flushPromises();
        expect(visible.bindings.loadingError.value).toEqual({ processed: false, message: 'tag failed' });
        expect(mockShowToast).toHaveBeenCalledWith('tag failed');
    });
});
