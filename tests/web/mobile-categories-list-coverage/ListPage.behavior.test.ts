import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockCategoryType = {
    Income: 2,
    Expense: 3,
    Transfer: 4,
    Investment: 5
} as const;

const mockTextDirection = { LTR: 1, RTL: 2 } as const;

const mockShowAlert = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockRouteBackOnError = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockOnSwipeoutDeleted = jest.fn<(...args: any[]) => void>();

let mockCurrentTextDirection: number = mockTextDirection.LTR;

function createCategory(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'salary',
        name: 'Salary',
        parentId: '0',
        type: mockCategoryType.Income,
        icon: 'money_dollar_circle_fill',
        color: '#0f766e',
        comment: 'Monthly income',
        displayOrder: 0,
        hidden: false,
        subCategories: [],
        ...overrides
    };
}

const mockVisibleSubCategory = createCategory({
    id: 'base-salary',
    name: 'Base Salary',
    parentId: 'salary',
    comment: 'Main job',
    subCategories: undefined
});
const mockHiddenSubCategory = createCategory({
    id: 'old-bonus',
    name: 'Old Bonus',
    parentId: 'salary',
    comment: '',
    hidden: true,
    subCategories: undefined
});
const mockVisiblePrimaryCategory = createCategory({
    subCategories: [mockVisibleSubCategory, mockHiddenSubCategory]
});
const mockHiddenPrimaryCategory = createCategory({
    id: 'archived-income',
    name: 'Archived Income',
    hidden: true,
    comment: '',
    subCategories: []
});

const mockPrimaryCategories = [mockVisiblePrimaryCategory, mockHiddenPrimaryCategory];
const mockCategoryMap: Record<string, any> = {
    salary: mockVisiblePrimaryCategory,
    'archived-income': mockHiddenPrimaryCategory
};

const mockStore = (jest.requireActual('vue') as any).reactive({
    allTransactionCategories: {
        [mockCategoryType.Income]: mockPrimaryCategories,
        [mockCategoryType.Expense]: [createCategory({ id: 'food', name: 'Food', type: mockCategoryType.Expense })],
        [mockCategoryType.Transfer]: [createCategory({ id: 'transfer', name: 'Transfer', type: mockCategoryType.Transfer })],
        [mockCategoryType.Investment]: [createCategory({ id: 'fund', name: 'Fund', type: mockCategoryType.Investment })]
    } as Record<number, any[]> | undefined,
    allTransactionCategoriesMap: mockCategoryMap as Record<string, any> | undefined,
    transactionCategoryListStateInvalid: false,
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>(),
    hideCategory: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteCategory: jest.fn<(...args: any[]) => Promise<void>>(),
    updateCategoryDisplayOrders: jest.fn<(...args: any[]) => Promise<void>>(),
    changeCategoryDisplayOrder: jest.fn<(...args: any[]) => Promise<void>>()
});

let mockLastBase: ReturnType<typeof createBase>;

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const loading = ref(true);
    const primaryCategoryId = ref('0');
    const currentPrimaryCategory = computed(() => (
        mockStore.allTransactionCategoriesMap?.[primaryCategoryId.value]
    ));
    return { loading, primaryCategoryId, currentPrimaryCategory };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageTextDirection: () => mockCurrentTextDirection
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showToast: mockShowToast,
        routeBackOnError: mockRouteBackOnError
    }),
    showLoading: mockShowLoading,
    hideLoading: mockHideLoading,
    onSwipeoutDeleted: (...args: any[]) => mockOnSwipeoutDeleted(...args)
}));
jest.mock('@/views/base/categories/CategoryListPageBase.ts', () => ({
    useCategoryListPageBase: () => {
        mockLastBase = createBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockStore }));
jest.mock('@/core/text.ts', () => ({ TextDirection: mockTextDirection }));
jest.mock('@/core/category.ts', () => ({ CategoryType: mockCategoryType }));
jest.mock('@/lib/category.ts', () => ({
    isNoAvailableCategory: (categories: any[], showHidden: boolean) => (
        !categories.some(category => showHidden || !category.hidden)
    ),
    getFirstShowingId: (categories: any[], showHidden: boolean) => (
        categories.find(category => showHidden || !category.hidden)?.id ?? null
    ),
    getLastShowingId: (categories: any[], showHidden: boolean) => (
        [...categories].reverse().find(category => showHidden || !category.hidden)?.id ?? null
    )
}));

import ListPage from '@/views/mobile/categories/ListPage.vue';

function setup(type: string = String(mockCategoryType.Income), id?: string): { bindings: any; router: any } {
    const router = { back: jest.fn(), navigate: jest.fn() };
    const query: Record<string, string> = { type };
    if (id !== undefined) query['id'] = id;
    const bindings = (ListPage as any).setup(
        { f7route: { query }, f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() }
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(
    type: string = String(mockCategoryType.Income),
    id?: string
): { app: any; root: any; router: any; state: any } {
    const { createRenderer, defineComponent, h } = jest.requireActual('vue') as any;
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
        createElement(hostType: string) {
            return createHostNode(hostType);
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
    const SlotHost = defineComponent({
        name: 'SlotHost',
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h('stub', attrs, Object.values(slots).flatMap((slot: any) => slot?.() ?? []));
        }
    });
    const router = { back: jest.fn(), navigate: jest.fn() };
    const query: Record<string, string> = { type };
    if (id !== undefined) query['id'] = id;
    const app = renderer.createApp(ListPage as any, { f7route: { query }, f7router: router });
    app.config.warnHandler = () => undefined;
    for (const name of [
        'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link', 'f7-icon',
        'f7-list', 'f7-list-item', 'f7-list-button', 'f7-badge', 'f7-swipeout-actions',
        'f7-swipeout-button', 'f7-actions', 'f7-actions-group', 'f7-actions-button', 'f7-actions-label',
        'ItemIcon'
    ]) app.component(name, SlotHost);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, router, state: vm.$.setupState };
}

function collectHostCallbacks(
    node: any,
    callbacks: Array<{ name: string; callback: (...args: any[]) => any }>,
    seen = new Set<any>()
): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    for (const [name, value] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        if (typeof value === 'function') {
            callbacks.push({ name, callback: value as (...args: any[]) => any });
        } else if (Array.isArray(value)) {
            for (const callback of value) {
                if (typeof callback === 'function') callbacks.push({ name, callback });
            }
        }
    }
    for (const child of node.children ?? []) collectHostCallbacks(child, callbacks, seen);
}

async function invokeHostCallbacks(callbacks: Array<{ name: string; callback: (...args: any[]) => any }>): Promise<void> {
    for (const { name, callback } of callbacks) {
        if (name === 'onSortable:sort') {
            callback({ el: { id: 'category_salary' }, from: 1, to: 2 });
        } else if (name === 'onPtr:refresh') {
            callback(jest.fn());
        } else {
            callback();
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockCurrentTextDirection = mockTextDirection.LTR;
    mockStore.allTransactionCategories = {
        [mockCategoryType.Income]: mockPrimaryCategories,
        [mockCategoryType.Expense]: [createCategory({ id: 'food', name: 'Food', type: mockCategoryType.Expense })],
        [mockCategoryType.Transfer]: [createCategory({ id: 'transfer', name: 'Transfer', type: mockCategoryType.Transfer })],
        [mockCategoryType.Investment]: [createCategory({ id: 'fund', name: 'Fund', type: mockCategoryType.Investment })]
    };
    mockStore.allTransactionCategoriesMap = { ...mockCategoryMap };
    mockStore.transactionCategoryListStateInvalid = false;
    mockStore.loadAllCategories.mockResolvedValue(undefined);
    mockStore.hideCategory.mockResolvedValue(undefined);
    mockStore.deleteCategory.mockResolvedValue(undefined);
    mockStore.updateCategoryDisplayOrders.mockResolvedValue(undefined);
    mockStore.changeCategoryDisplayOrder.mockResolvedValue(undefined);
});

describe('mobile category ListPage state and lifecycle', () => {
    test('initializes primary and secondary category routes with exact titles and visibility helpers', async () => {
        const primary = setup();
        expect(mockLastBase.loading.value).toBe(true);
        await flush();
        expect(mockStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(mockLastBase.loading.value).toBe(false);
        expect(primary.bindings.categoryType.value).toBe(mockCategoryType.Income);
        expect(primary.bindings.hasSubCategories.value).toBe(true);
        expect(mockLastBase.primaryCategoryId.value).toBe('0');
        expect(primary.bindings.title.value).toBe('Income Primary Categories');
        expect(primary.bindings.categories.value).toStrictEqual(mockPrimaryCategories);
        expect(primary.bindings.firstShowingId.value).toBe('salary');
        expect(primary.bindings.lastShowingId.value).toBe('salary');
        expect(primary.bindings.noAvailableCategory.value).toBe(false);
        expect(primary.bindings.noCategory.value).toBe(false);
        expect(primary.bindings.textDirection.value).toBe(mockTextDirection.LTR);
        expect(primary.bindings.getCategoryDomId(mockVisiblePrimaryCategory)).toBe('category_salary');
        expect(primary.bindings.parseCategoryIdFromDomId('category_salary')).toBe('salary');
        expect(primary.bindings.parseCategoryIdFromDomId('')).toBeNull();
        expect(primary.bindings.parseCategoryIdFromDomId('account_salary')).toBeNull();

        primary.bindings.showHidden.value = true;
        expect(primary.bindings.lastShowingId.value).toBe('archived-income');

        const secondary = setup(String(mockCategoryType.Income), 'salary');
        await flush();
        expect(secondary.bindings.hasSubCategories.value).toBe(false);
        expect(mockLastBase.primaryCategoryId.value).toBe('salary');
        expect(secondary.bindings.title.value).toBe('Income Secondary Categories');
        expect(secondary.bindings.categories.value).toStrictEqual([mockVisibleSubCategory, mockHiddenSubCategory]);

        expect(setup(String(mockCategoryType.Expense)).bindings.title.value).toBe('Expense Primary Categories');
        expect(setup(String(mockCategoryType.Transfer)).bindings.title.value).toBe('Transfer Primary Categories');
        expect(setup(String(mockCategoryType.Investment), '0').bindings.title.value).toBe('Investment Primary Categories');
        secondary.bindings.categoryType.value = 0;
        expect(secondary.bindings.title.value).toBe('Transaction Secondary Categories');
    });

    test('returns empty lists for absent primary and secondary store data', () => {
        mockStore.allTransactionCategories = undefined;
        expect(setup().bindings.categories.value).toStrictEqual([]);

        mockStore.allTransactionCategories = {};
        expect(setup().bindings.categories.value).toStrictEqual([]);

        mockStore.allTransactionCategoriesMap = undefined;
        expect(setup(String(mockCategoryType.Income), 'salary').bindings.categories.value).toStrictEqual([]);

        mockStore.allTransactionCategoriesMap = {};
        expect(setup(String(mockCategoryType.Income), 'salary').bindings.categories.value).toStrictEqual([]);

        mockStore.allTransactionCategoriesMap = {
            salary: createCategory({ id: 'salary', subCategories: undefined })
        };
        const noChildren = setup(String(mockCategoryType.Income), 'salary');
        expect(noChildren.bindings.categories.value).toStrictEqual([]);
        expect(noChildren.bindings.noAvailableCategory.value).toBe(true);
        expect(noChildren.bindings.noCategory.value).toBe(true);
    });

    test('rejects invalid route parameters and handles every initialization failure shape', async () => {
        const invalid = setup('99');
        expect(invalid.bindings.loadingError.value).toBe('Parameter Invalid');
        expect(mockShowToast).toHaveBeenCalledWith('Parameter Invalid');
        expect(mockStore.loadAllCategories).not.toHaveBeenCalled();

        mockStore.loadAllCategories.mockRejectedValueOnce({ processed: true, message: 'handled init' });
        setup();
        await flush();
        expect(mockLastBase.loading.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalledWith('handled init');

        mockStore.loadAllCategories.mockRejectedValueOnce({ processed: false, message: 'init failed' });
        const readable = setup();
        await flush();
        expect(readable.bindings.loadingError.value).toMatchObject({ message: 'init failed' });
        expect(mockShowToast).toHaveBeenCalledWith('init failed');

        mockStore.loadAllCategories.mockRejectedValueOnce('raw init failure');
        const raw = setup();
        await flush();
        expect(raw.bindings.loadingError.value).toBe('raw init failure');
        expect(mockShowToast).toHaveBeenCalledWith('raw init failure');
    });

    test('reloads normally and by pull-to-refresh while honoring sortable and error paths', async () => {
        const { bindings } = setup();
        await flush();
        mockStore.loadAllCategories.mockClear();

        bindings.reload();
        await flush();
        expect(mockStore.loadAllCategories).toHaveBeenCalledWith({ force: false });

        const done = jest.fn();
        bindings.reload(done);
        await flush();
        expect(mockStore.loadAllCategories).toHaveBeenLastCalledWith({ force: true });
        expect(done).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('Category list has been updated');

        bindings.sortable.value = true;
        mockStore.loadAllCategories.mockClear();
        const sortableDone = jest.fn();
        bindings.reload(sortableDone);
        expect(sortableDone).toHaveBeenCalled();
        expect(mockStore.loadAllCategories).not.toHaveBeenCalled();
        bindings.sortable.value = false;

        mockStore.loadAllCategories.mockRejectedValueOnce({ processed: true, message: 'handled reload' });
        bindings.reload(jest.fn());
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled reload');

        mockStore.loadAllCategories.mockRejectedValueOnce({ processed: false, message: 'reload failed' });
        bindings.reload(jest.fn());
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('reload failed');

        mockStore.loadAllCategories.mockRejectedValueOnce('raw reload failure');
        bindings.reload();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw reload failure');
    });

    test('refreshes invalid category state after page entry and always invokes route recovery', async () => {
        const { bindings, router } = setup();
        await flush();
        mockStore.loadAllCategories.mockClear();
        mockStore.transactionCategoryListStateInvalid = true;

        mockLastBase.loading.value = true;
        bindings.onPageAfterIn();
        expect(mockStore.loadAllCategories).not.toHaveBeenCalled();

        mockLastBase.loading.value = false;
        bindings.onPageAfterIn();
        await flush();
        expect(mockStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });
});

describe('mobile category ListPage actions and sorting', () => {
    test('navigates to editing and hides or restores categories across all outcomes', async () => {
        const { bindings, router } = setup();
        bindings.edit(mockVisiblePrimaryCategory);
        expect(router.navigate).toHaveBeenCalledWith('/category/edit?id=salary');

        bindings.hide(mockVisiblePrimaryCategory, true);
        expect(mockShowLoading).toHaveBeenCalled();
        expect(mockStore.hideCategory).toHaveBeenCalledWith({
            category: mockVisiblePrimaryCategory,
            hidden: true
        });
        await flush();
        expect(mockHideLoading).toHaveBeenCalled();

        mockStore.hideCategory.mockRejectedValueOnce({ processed: true, message: 'handled hide' });
        bindings.hide(mockVisiblePrimaryCategory, false);
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled hide');

        mockStore.hideCategory.mockRejectedValueOnce({ processed: false, message: 'hide failed' });
        bindings.hide(mockVisiblePrimaryCategory, false);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('hide failed');

        mockStore.hideCategory.mockRejectedValueOnce('raw hide failure');
        bindings.hide(mockVisiblePrimaryCategory, false);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw hide failure');
    });

    test('prompts, deletes with swipeout completion, and reports every delete failure shape', async () => {
        const { bindings } = setup();
        bindings.remove(null, false);
        expect(mockShowAlert).toHaveBeenCalledWith('An error occurred');

        bindings.remove(mockVisiblePrimaryCategory, false);
        expect(bindings.categoryToDelete.value).toStrictEqual(mockVisiblePrimaryCategory);
        expect(bindings.showDeleteActionSheet.value).toBe(true);

        mockStore.deleteCategory.mockImplementationOnce(async ({ beforeResolve }: any) => {
            beforeResolve('done-callback');
        });
        bindings.remove(mockVisiblePrimaryCategory, true);
        await flush();
        expect(mockOnSwipeoutDeleted).toHaveBeenCalledWith('category_salary', 'done-callback');
        expect(bindings.categoryToDelete.value).toBeNull();
        expect(bindings.showDeleteActionSheet.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();

        mockStore.deleteCategory.mockRejectedValueOnce({ processed: true, message: 'handled delete' });
        bindings.remove(mockVisiblePrimaryCategory, true);
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled delete');

        mockStore.deleteCategory.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove(mockVisiblePrimaryCategory, true);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('delete failed');

        mockStore.deleteCategory.mockRejectedValueOnce('raw delete failure');
        bindings.remove(mockVisiblePrimaryCategory, true);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw delete failure');
    });

    test('enters sorting and saves changed or unchanged primary and secondary orders', async () => {
        const { bindings } = setup();
        bindings.setSortable();
        expect(bindings.sortable.value).toBe(true);
        expect(bindings.showHidden.value).toBe(true);
        expect(bindings.displayOrderModified.value).toBe(false);
        bindings.setSortable();
        expect(bindings.sortable.value).toBe(true);

        bindings.saveSortResult();
        expect(bindings.sortable.value).toBe(false);
        expect(bindings.showHidden.value).toBe(false);
        expect(mockStore.updateCategoryDisplayOrders).not.toHaveBeenCalled();

        bindings.setSortable();
        bindings.displayOrderModified.value = true;
        bindings.saveSortResult();
        expect(bindings.displayOrderSaving.value).toBe(true);
        await flush();
        expect(mockStore.updateCategoryDisplayOrders).toHaveBeenCalledWith({
            type: mockCategoryType.Income,
            parentId: '0'
        });
        expect(bindings.displayOrderSaving.value).toBe(false);
        expect(bindings.sortable.value).toBe(false);
        expect(bindings.showHidden.value).toBe(false);
        expect(bindings.displayOrderModified.value).toBe(false);

        const secondary = setup(String(mockCategoryType.Income), 'salary').bindings;
        secondary.setSortable();
        secondary.displayOrderModified.value = true;
        secondary.saveSortResult();
        await flush();
        expect(mockStore.updateCategoryDisplayOrders).toHaveBeenLastCalledWith({
            type: mockCategoryType.Income,
            parentId: 'salary'
        });

        bindings.setSortable();
        bindings.displayOrderModified.value = true;
        mockStore.updateCategoryDisplayOrders.mockRejectedValueOnce({ processed: true, message: 'handled sort' });
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled sort');

        mockStore.updateCategoryDisplayOrders.mockRejectedValueOnce({ processed: false, message: 'sort failed' });
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('sort failed');

        mockStore.updateCategoryDisplayOrders.mockRejectedValueOnce('raw sort failure');
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw sort failure');
    });

    test('validates sortable events and records successful display-order changes', async () => {
        const { bindings } = setup();
        bindings.onSort(null);
        bindings.onSort({});
        bindings.onSort({ el: {} });
        bindings.onSort({ el: { id: 'account_salary' }, from: 1, to: 2 });
        bindings.onSort({ el: { id: 'category_' }, from: 1, to: 2 });
        expect(mockShowToast).toHaveBeenCalledTimes(5);

        bindings.onSort({ el: { id: 'category_salary' }, from: 2, to: 4 });
        await flush();
        expect(mockStore.changeCategoryDisplayOrder).toHaveBeenCalledWith({
            categoryId: 'salary',
            from: 2,
            to: 4
        });
        expect(bindings.displayOrderModified.value).toBe(true);

        mockStore.changeCategoryDisplayOrder.mockRejectedValueOnce(new Error('move failed'));
        bindings.onSort({ el: { id: 'category_salary' }, from: 1, to: 2 });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('move failed');

        mockStore.changeCategoryDisplayOrder.mockRejectedValueOnce('raw move failure');
        bindings.onSort({ el: { id: 'category_salary' }, from: 1, to: 2 });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw move failure');
    });
});

describe('mobile category ListPage production template', () => {
    test('renders named slots and executes generated event wrappers in the no-DOM runtime', async () => {
        const mounted = mountWithHostRenderer();
        try {
            const { nextTick } = jest.requireActual('vue') as any;
            await flush();
            await nextTick();

            mounted.state.categoryToDelete = mockVisiblePrimaryCategory;
            mounted.state.showDeleteActionSheet = true;
            mounted.state.showMoreActionSheet = true;
            await nextTick();
            const normalCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(mounted.root, normalCallbacks);
            await invokeHostCallbacks(normalCallbacks);
            expect(normalCallbacks.length).toBeGreaterThan(10);

            mounted.state.sortable = true;
            mounted.state.showHidden = true;
            mounted.state.displayOrderModified = true;
            await nextTick();
            const sortableCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(mounted.root, sortableCallbacks);
            await invokeHostCallbacks(sortableCallbacks);
            expect(sortableCallbacks.length).toBeGreaterThan(5);

            mounted.state.loading = true;
            await nextTick();
            mounted.state.loading = false;
            mockStore.allTransactionCategories = { [mockCategoryType.Income]: [] };
            mounted.state.sortable = false;
            mounted.state.showHidden = false;
            await nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }

        mockCurrentTextDirection = mockTextDirection.RTL;
        mockStore.allTransactionCategoriesMap = { ...mockCategoryMap };
        const secondary = mountWithHostRenderer(String(mockCategoryType.Income), 'salary');
        try {
            const { nextTick } = jest.requireActual('vue') as any;
            await flush();
            secondary.state.showHidden = true;
            secondary.state.sortable = true;
            await nextTick();
            const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(secondary.root, callbacks);
            await invokeHostCallbacks(callbacks);
            expect(secondary.state.textDirection).toBe(mockTextDirection.RTL);
        } finally {
            secondary.app.unmount();
        }
    });
});
