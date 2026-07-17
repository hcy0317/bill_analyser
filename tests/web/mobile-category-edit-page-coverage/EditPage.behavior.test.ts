import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import {
    collectHostCallbacks,
    mountWithHostRenderer,
} from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const actualCategoryModule = jest.requireActual('@/core/category.ts') as any;
const actualTransactionCategoryModule = jest.requireActual('@/models/transaction_category.ts') as any;
const { CategoryType } = actualCategoryModule;
const { TransactionCategory } = actualTransactionCategoryModule;

const mockShowAlert = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockRouteBackOnError = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockGenerateRandomUUID = jest.fn<() => string>();

const mockStore = actualVue.reactive({
    allTransactionCategories: {
        [CategoryType.Income]: [],
        [CategoryType.Expense]: [],
        [CategoryType.Transfer]: [],
        [CategoryType.Investment]: [],
    } as Record<number, any[]>,
    getCategory: jest.fn<(...args: any[]) => Promise<any>>(),
    saveCategory: jest.fn<(...args: any[]) => Promise<any>>(),
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showToast: mockShowToast,
        routeBackOnError: mockRouteBackOnError,
    }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args),
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockStore,
}));
jest.mock('@/lib/misc.ts', () => ({
    generateRandomUUID: () => mockGenerateRandomUUID(),
}));

import EditPageComponent from '@/views/mobile/categories/EditPage.vue';

const EditPage = EditPageComponent as any;

function createCategory(overrides: Record<string, unknown> = {}): any {
    return TransactionCategory.of({
        id: 'category-1',
        name: 'Dining',
        parentId: 'food',
        type: CategoryType.Expense,
        icon: '31',
        color: 'ff3b30',
        comment: 'Meals outside',
        displayOrder: 2,
        hidden: false,
        ...overrides,
    });
}

function resetCategories(): void {
    const food = createCategory({
        id: 'food',
        name: 'Food',
        parentId: '0',
        subCategories: [],
    });
    const salary = createCategory({
        id: 'salary',
        name: 'Salary',
        parentId: '0',
        type: CategoryType.Income,
        subCategories: [],
    });
    mockStore.allTransactionCategories = {
        [CategoryType.Income]: [salary],
        [CategoryType.Expense]: [food],
        [CategoryType.Transfer]: [],
        [CategoryType.Investment]: [],
    };
}

function setup(query: Record<string, string>): { bindings: any; router: any } {
    const router = { back: jest.fn(), navigate: jest.fn() };
    const bindings = EditPage.setup(
        { f7route: { query }, f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() },
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

beforeEach(() => {
    jest.clearAllMocks();
    resetCategories();
    mockGenerateRandomUUID.mockReturnValue('category-client-session');
    mockStore.getCategory.mockResolvedValue(createCategory());
    mockStore.saveCategory.mockResolvedValue(createCategory());
});

describe('mobile category EditPage initialization and identity boundaries', () => {
    test('rejects routes without an edit id or parent id and recovers after page entry', () => {
        const { bindings, router } = setup({});

        expect(mockShowToast).toHaveBeenCalledWith('Parameter Invalid');
        expect(bindings.loadingError.value).toBe('Parameter Invalid');
        expect(mockStore.getCategory).not.toHaveBeenCalled();

        bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });

    test.each([
        CategoryType.Income,
        CategoryType.Expense,
        CategoryType.Transfer,
        CategoryType.Investment,
    ])('creates category type %s with exact parent, optional appearance, and session identity', (type: number) => {
        const { bindings } = setup({
            parentId: 'parent-category',
            type: String(type),
            color: '2196f3',
            icon: '31',
        });

        expect(bindings.editCategoryId.value).toBeNull();
        expect(bindings.category.value).toMatchObject({
            parentId: 'parent-category',
            type,
            color: '2196f3',
            icon: '31',
        });
        expect(bindings.clientSessionId.value).toBe('category-client-session');
        expect(bindings.loading.value).toBe(false);
    });

    test('uses default appearance when omitted and rejects missing or unknown category types', () => {
        const valid = setup({ parentId: '0', type: String(CategoryType.Expense) }).bindings;
        expect(valid.category.value).toMatchObject({
            parentId: '0',
            type: CategoryType.Expense,
        });

        const missingType = setup({ parentId: '0' }).bindings;
        expect(missingType.loadingError.value).toBe('Parameter Invalid');

        const invalidType = setup({ parentId: '0', type: '999' }).bindings;
        expect(invalidType.loadingError.value).toBe('Parameter Invalid');
        expect(mockShowToast).toHaveBeenCalledWith('Parameter Invalid');
    });

    test('loads an edited category and resolves its primary category display name', async () => {
        const loaded = createCategory({ id: 'category-9', parentId: 'food', name: 'Cafe' });
        mockStore.getCategory.mockResolvedValueOnce(loaded);

        const { bindings } = setup({ id: 'category-9' });
        expect(bindings.loading.value).toBe(true);
        expect(bindings.editCategoryId.value).toBe('category-9');
        expect(mockStore.getCategory).toHaveBeenCalledWith({ categoryId: 'category-9' });

        await flush();
        expect(bindings.category.value).toMatchObject({
            id: 'category-9',
            name: 'Cafe',
            parentId: 'food',
        });
        expect(bindings.loading.value).toBe(false);
        expect(bindings.getPrimaryCategoryName('food')).toBe('Food');
        expect(bindings.getPrimaryCategoryName('missing')).toBeNull();
    });

    test('handles processed, readable, and primitive category-load failures', async () => {
        mockStore.getCategory.mockRejectedValueOnce({ processed: true, message: 'already handled' });
        const processed = setup({ id: 'processed' }).bindings;
        await flush();
        expect(processed.loading.value).toBe(false);
        expect(processed.loadingError.value).toBeNull();
        expect(mockShowToast).not.toHaveBeenCalledWith('already handled');

        const readableError = { processed: false, message: 'load failed' };
        mockStore.getCategory.mockRejectedValueOnce(readableError);
        const readable = setup({ id: 'readable' }).bindings;
        await flush();
        expect(readable.loadingError.value).toStrictEqual(readableError);
        expect(mockShowToast).toHaveBeenCalledWith('load failed');

        mockStore.getCategory.mockRejectedValueOnce('raw load failure');
        const primitive = setup({ id: 'primitive' }).bindings;
        await flush();
        expect(primitive.loadingError.value).toBe('raw load failure');
        expect(mockShowToast).toHaveBeenCalledWith('raw load failure');
    });
});

describe('mobile category EditPage validation and persistence', () => {
    test('blocks an empty category name without entering the submitting state', () => {
        const { bindings } = setup({ parentId: '0', type: String(CategoryType.Expense) });

        bindings.save();

        expect(mockShowAlert).toHaveBeenCalledWith('Category name cannot be blank');
        expect(bindings.submitting.value).toBe(false);
        expect(mockStore.saveCategory).not.toHaveBeenCalled();
    });

    test('creates a category, exposes loading state, and navigates back after success', async () => {
        const saved = createCategory({ id: 'created-1', name: 'Groceries', parentId: '0' });
        mockStore.saveCategory.mockResolvedValueOnce(saved);
        const { bindings, router } = setup({ parentId: '0', type: String(CategoryType.Expense) });
        bindings.category.value.name = 'Groceries';

        bindings.save();
        expect(bindings.submitting.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(expect.any(Function));
        const loadingPredicate = mockShowLoading.mock.calls[0]?.[0] as () => boolean;
        expect(loadingPredicate()).toBe(true);
        expect(mockStore.saveCategory).toHaveBeenCalledWith({
            category: bindings.category.value,
            isEdit: false,
            clientSessionId: 'category-client-session',
        });

        await flush();
        expect(bindings.submitting.value).toBe(false);
        expect(loadingPredicate()).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('You have added a new category');
        expect(router.back).toHaveBeenCalled();
    });

    test('updates an edited category and preserves the edit identity in the save request', async () => {
        const { bindings, router } = setup({ id: 'category-9' });
        await flush();
        bindings.category.value.name = 'Updated dining';
        mockStore.saveCategory.mockResolvedValueOnce(bindings.category.value);

        bindings.save();
        await flush();

        expect(mockStore.saveCategory).toHaveBeenLastCalledWith({
            category: bindings.category.value,
            isEdit: true,
            clientSessionId: '',
        });
        expect(mockShowToast).toHaveBeenCalledWith('You have saved this category');
        expect(router.back).toHaveBeenCalled();
    });

    test('handles processed and unprocessed save failures without navigating away', async () => {
        mockStore.saveCategory.mockRejectedValueOnce({ processed: true, message: 'already handled' });
        const processed = setup({ parentId: '0', type: String(CategoryType.Income) });
        processed.bindings.category.value.name = 'Bonus';
        processed.bindings.save();
        await flush();
        expect(processed.bindings.submitting.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).not.toHaveBeenCalledWith('already handled');
        expect(processed.router.back).not.toHaveBeenCalled();

        mockStore.saveCategory.mockRejectedValueOnce({ processed: false, message: 'save failed' });
        const readable = setup({ parentId: '0', type: String(CategoryType.Income) });
        readable.bindings.category.value.name = 'Bonus';
        readable.bindings.save();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('save failed');
        expect(readable.router.back).not.toHaveBeenCalled();

        mockStore.saveCategory.mockRejectedValueOnce('raw save failure');
        const primitive = setup({ parentId: '0', type: String(CategoryType.Income) });
        primitive.bindings.category.value.name = 'Bonus';
        primitive.bindings.save();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw save failure');
        expect(primitive.router.back).not.toHaveBeenCalled();
    });
});

describe('mobile category EditPage production template', () => {
    const componentNames = [
        'f7-page',
        'f7-navbar',
        'f7-nav-left',
        'f7-nav-title',
        'f7-nav-right',
        'f7-link',
        'f7-list',
        'f7-list-input',
        'f7-list-item',
        'f7-icon',
        'f7-toggle',
        'list-item-selection-sheet',
        'icon-selection-sheet',
        'color-selection-sheet',
        'ItemIcon',
    ];

    test('renders loading, create, and secondary-edit branches with real template event wrappers', async () => {
        const router = { back: jest.fn(), navigate: jest.fn() };
        let resolveCategory: ((category: any) => void) | undefined;
        mockStore.getCategory.mockImplementationOnce(() => new Promise(resolve => {
            resolveCategory = resolve;
        }));
        const loading = mountWithHostRenderer(
            EditPage,
            { f7route: { query: { id: 'loading' } }, f7router: router },
            componentNames,
        );
        try {
            expect(loading.state.loading).toBe(true);
            expect(loading.root.children.length).toBeGreaterThan(0);
            resolveCategory?.(createCategory({ id: 'loading' }));
            await flush();
        } finally {
            loading.app.unmount();
        }

        const create = mountWithHostRenderer(
            EditPage,
            {
                f7route: { query: { parentId: '0', type: String(CategoryType.Expense) } },
                f7router: router,
            },
            componentNames,
        );
        try {
            await actualVue.nextTick();
            const createCallbacks = collectHostCallbacks(create.root);
            expect(createCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onPage:afterin',
                'onClick',
                'onUpdate:value',
                'onUpdate:show',
                'onUpdate:modelValue',
            ]));

            for (const { name, callback } of createCallbacks) {
                if (name === 'onPage:afterin') {
                    callback();
                } else if (name === 'onUpdate:value') {
                    callback('Template category');
                } else if (name === 'onUpdate:show') {
                    callback(true);
                } else if (name === 'onUpdate:modelValue') {
                    callback('31');
                }
                await actualVue.nextTick();
            }
            expect(mockRouteBackOnError).toHaveBeenCalled();
            expect(create.state.category.name).toBe('Template category');
        } finally {
            create.app.unmount();
        }

        mockStore.getCategory.mockResolvedValueOnce(createCategory({
            id: 'secondary-1',
            parentId: 'food',
            visible: false,
        }));
        const edit = mountWithHostRenderer(
            EditPage,
            { f7route: { query: { id: 'secondary-1' } }, f7router: router },
            componentNames,
        );
        try {
            await flush();
            edit.state.showPrimaryCategorySheet = true;
            edit.state.showIconSelectionSheet = true;
            edit.state.showColorSelectionSheet = true;
            edit.state.submitting = true;
            await actualVue.nextTick();

            const editCallbacks = collectHostCallbacks(edit.root);
            expect(editCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onClick',
                'onToggle:change',
                'onUpdate:value',
                'onUpdate:show',
                'onUpdate:modelValue',
            ]));
            for (const { name, callback } of editCallbacks) {
                if (name === 'onToggle:change') {
                    callback(true);
                } else if (name === 'onUpdate:value') {
                    callback('Template edit');
                } else if (name === 'onUpdate:show') {
                    callback(false);
                } else if (name === 'onUpdate:modelValue') {
                    callback('food');
                } else if (name === 'onClick') {
                    callback({ type: 'synthetic-click' });
                }
                await actualVue.nextTick();
            }
            expect(edit.state.category.visible).toBe(true);
            expect(edit.root.children.length).toBeGreaterThan(0);
        } finally {
            edit.app.unmount();
        }
    });
});
