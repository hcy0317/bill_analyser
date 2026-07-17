import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { CategoryType } from '@/core/category.ts';
import { TemplateType } from '@/core/template.ts';
import { TransactionType } from '@/core/transaction.ts';
import { TransactionCategory, type TransactionCategoryInfoResponse } from '@/models/transaction_category.ts';
import { TransactionTag, type TransactionTagInfoResponse } from '@/models/transaction_tag.ts';
import { TransactionTemplate, type TransactionTemplateInfoResponse } from '@/models/transaction_template.ts';

type ApiResponse<T> = Promise<{ data: { success: boolean; result?: T } }>;
type ServiceMock = jest.Mock<(...args: any[]) => ApiResponse<any>>;

const mockServices = {
    getAllTransactionCategories: jest.fn(),
    getTransactionCategory: jest.fn(),
    addTransactionCategory: jest.fn(),
    modifyTransactionCategory: jest.fn(),
    addTransactionCategoryBatch: jest.fn(),
    moveTransactionCategory: jest.fn(),
    hideTransactionCategory: jest.fn(),
    deleteTransactionCategory: jest.fn(),
    getAllTransactionTemplates: jest.fn(),
    getTransactionTemplate: jest.fn(),
    addTransactionTemplate: jest.fn(),
    modifyTransactionTemplate: jest.fn(),
    moveTransactionTemplate: jest.fn(),
    hideTransactionTemplate: jest.fn(),
    deleteTransactionTemplate: jest.fn(),
    getAllTransactionTags: jest.fn(),
    addTransactionTag: jest.fn(),
    modifyTransactionTag: jest.fn(),
    addTransactionTagBatch: jest.fn(),
    moveTransactionTag: jest.fn(),
    hideTransactionTag: jest.fn(),
    deleteTransactionTag: jest.fn()
} satisfies Record<string, ServiceMock>;

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: mockServices
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: jest.fn(),
        error: jest.fn()
    }
}));

import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useTransactionTemplatesStore } from '@/stores/transactionTemplate.ts';

function ok<T>(result: T): ApiResponse<T> {
    return Promise.resolve({ data: { success: true, result } });
}

function invalid(): ApiResponse<never> {
    return Promise.resolve({ data: { success: false } });
}

function categoryInfo(
    id: string,
    type: CategoryType,
    parentId = '0',
    hidden = false,
    subCategories?: TransactionCategoryInfoResponse[]
): TransactionCategoryInfoResponse {
    return {
        id,
        name: `category-${id}`,
        parentId,
        type,
        icon: 'las la-wallet',
        color: '#123456',
        comment: '',
        displayOrder: 1,
        hidden,
        subCategories
    };
}

function categoryMap(): Record<number, TransactionCategoryInfoResponse[]> {
    return {
        [CategoryType.Expense]: [categoryInfo('expense', CategoryType.Expense, '0', false, [
            categoryInfo('expense-sub', CategoryType.Expense, 'expense')
        ])],
        [CategoryType.Income]: [categoryInfo('income', CategoryType.Income, '0', false, [
            categoryInfo('income-sub', CategoryType.Income, 'income')
        ])],
        [CategoryType.Transfer]: [categoryInfo('transfer', CategoryType.Transfer, '0', false, [
            categoryInfo('transfer-sub', CategoryType.Transfer, 'transfer')
        ])],
        [CategoryType.Investment]: [categoryInfo('investment', CategoryType.Investment, '0', false, [
            categoryInfo('investment-sub', CategoryType.Investment, 'investment')
        ])]
    };
}

function tagInfo(id: string, hidden = false, displayOrder = 1): TransactionTagInfoResponse {
    return { id, name: `tag-${id}`, displayOrder, hidden };
}

function templateInfo(
    id: string,
    templateType = TemplateType.Normal.type,
    type: TransactionType = TransactionType.Expense,
    hidden = false,
    displayOrder = 1
): TransactionTemplateInfoResponse {
    return {
        id,
        templateType,
        name: `template-${id}`,
        timeSequenceId: '',
        type,
        categoryId: 'category',
        time: 0,
        utcOffset: 480,
        sourceAccountId: 'source',
        destinationAccountId: type === TransactionType.Transfer ? 'destination' : '0',
        sourceAmountCents: 1_000,
        destinationAmountCents: type === TransactionType.Transfer ? 1_000 : 0,
        hideAmount: false,
        tagIds: [],
        comment: '',
        editable: true,
        displayOrder,
        hidden
    };
}

function responseError(): { response: { data: { message: string } } } {
    return { response: { data: { message: 'server detail' } } };
}

beforeEach(() => {
    setActivePinia(createPinia());
    for (const mock of Object.values(mockServices)) {
        mock.mockReset();
    }
});

describe('transaction category store behavior coverage', () => {
    test('loads all families, builds nested map, exposes availability and caches', async () => {
        mockServices.getAllTransactionCategories.mockReturnValue(ok(categoryMap()));
        const store = useTransactionCategoriesStore();

        await expect(store.loadAllCategories({ force: false })).resolves.toMatchObject({
            [CategoryType.Expense]: [expect.objectContaining({ id: 'expense' })]
        });
        expect(store.allTransactionCategoriesMap['expense-sub']?.parentId).toBe('expense');
        expect(store.hasAvailableExpenseCategories).toBe(true);
        expect(store.hasAvailableIncomeCategories).toBe(true);
        expect(store.hasAvailableTransferCategories).toBe(true);
        expect(store.hasAvailableInvestmentCategories).toBe(true);
        await store.loadAllCategories({ force: false });
        expect(mockServices.getAllTransactionCategories).toHaveBeenCalledTimes(1);

        mockServices.getAllTransactionCategories.mockReturnValueOnce(ok(categoryMap()));
        await expect(store.loadAllCategories({ force: true })).rejects.toMatchObject({ isUpToDate: true });
    });

    test('normalizes absent canonical families and reports unavailable categories', async () => {
        mockServices.getAllTransactionCategories.mockReturnValue(ok({
            [CategoryType.Investment]: [categoryInfo('hidden', CategoryType.Investment, '0', true)]
        }));
        const store = useTransactionCategoriesStore();
        await store.loadAllCategories({ force: false });
        expect(store.allTransactionCategories[CategoryType.Income]).toEqual([]);
        expect(store.allTransactionCategories[CategoryType.Expense]).toEqual([]);
        expect(store.allTransactionCategories[CategoryType.Transfer]).toEqual([]);
        expect(store.hasAvailableIncomeCategories).toBe(false);
        expect(store.hasAvailableExpenseCategories).toBe(false);
        expect(store.hasAvailableTransferCategories).toBe(false);
        expect(store.hasAvailableInvestmentCategories).toBe(false);
    });

    test.each([
        ['invalid response', invalid(), { message: 'Unable to retrieve category list' }, false],
        ['server response', Promise.reject(responseError()), { error: { message: 'server detail' } }, false],
        ['unprocessed error', Promise.reject({ processed: false }), { message: 'Unable to retrieve category list' }, false],
        ['processed error', Promise.reject({ processed: true, code: 'kept' }), { processed: true, code: 'kept' }, false],
        ['force server response', Promise.reject(responseError()), { error: { message: 'server detail' } }, true]
    ])('maps %s while loading categories', async (_name, response, expected, force) => {
        mockServices.getAllTransactionCategories.mockReturnValueOnce(response as ApiResponse<any>);
        const store = useTransactionCategoriesStore();
        await expect(store.loadAllCategories({ force })).rejects.toEqual(expected);
    });

    test('gets category details and maps all retrieval failures', async () => {
        const store = useTransactionCategoriesStore();
        mockServices.getTransactionCategory.mockReturnValueOnce(ok(categoryInfo('detail', CategoryType.Expense)));
        await expect(store.getCategory({ categoryId: 'detail' })).resolves.toMatchObject({ id: 'detail' });
        expect(mockServices.getTransactionCategory).toHaveBeenCalledWith({ id: 'detail' });

        mockServices.getTransactionCategory.mockReturnValueOnce(invalid());
        await expect(store.getCategory({ categoryId: 'bad' })).rejects.toEqual({ message: 'Unable to retrieve category' });
        mockServices.getTransactionCategory.mockRejectedValueOnce(responseError());
        await expect(store.getCategory({ categoryId: 'bad' })).rejects.toEqual({ error: { message: 'server detail' } });
        mockServices.getTransactionCategory.mockRejectedValueOnce({ processed: false });
        await expect(store.getCategory({ categoryId: 'bad' })).rejects.toEqual({ message: 'Unable to retrieve category' });
        const processed = { processed: true, code: 'kept' };
        mockServices.getTransactionCategory.mockRejectedValueOnce(processed);
        await expect(store.getCategory({ categoryId: 'bad' })).rejects.toBe(processed);
    });

    test('creates root/child categories and updates same-parent and moved categories', async () => {
        mockServices.getAllTransactionCategories.mockReturnValue(ok(categoryMap()));
        const store = useTransactionCategoriesStore();
        await store.loadAllCategories({ force: false });

        const newRoot = TransactionCategory.createNewCategory(CategoryType.Expense);
        newRoot.name = 'created root';
        mockServices.addTransactionCategory.mockReturnValueOnce(ok(categoryInfo('created-root', CategoryType.Expense)));
        await store.saveCategory({ category: newRoot, isEdit: false, clientSessionId: 'session' });
        expect(store.allTransactionCategories[CategoryType.Expense]?.at(-1)?.id).toBe('created-root');

        const newChild = TransactionCategory.createNewCategory(CategoryType.Expense, 'expense');
        newChild.name = 'created child';
        mockServices.addTransactionCategory.mockReturnValueOnce(ok(categoryInfo('created-child', CategoryType.Expense, 'expense')));
        await store.saveCategory({ category: newChild, isEdit: false, clientSessionId: 'session' });
        expect(store.allTransactionCategoriesMap['expense']?.subCategories?.at(-1)?.id).toBe('created-child');

        const root = store.allTransactionCategoriesMap['expense']!;
        mockServices.modifyTransactionCategory.mockReturnValueOnce(ok({ ...categoryInfo('expense', CategoryType.Expense), name: 'updated' }));
        await store.saveCategory({ category: root, isEdit: true, clientSessionId: '' });
        expect(store.allTransactionCategoriesMap['expense']?.name).toBe('updated');
        expect(store.allTransactionCategoriesMap['expense']?.subCategories).toHaveLength(2);

        const child = store.allTransactionCategoriesMap['expense-sub']!;
        mockServices.modifyTransactionCategory.mockReturnValueOnce(ok({
            ...categoryInfo('expense-sub', CategoryType.Expense, 'expense'),
            name: 'child-updated'
        }));
        await store.saveCategory({ category: child, isEdit: true, clientSessionId: '' });
        expect(store.allTransactionCategoriesMap['expense-sub']?.name).toBe('child-updated');

        mockServices.modifyTransactionCategory.mockReturnValueOnce(ok({ ...categoryInfo('expense-sub', CategoryType.Expense, 'income'), name: 'moved' }));
        await store.saveCategory({ category: store.allTransactionCategoriesMap['expense-sub']!, isEdit: true, clientSessionId: '' });
        expect(store.transactionCategoryListStateInvalid).toBe(true);

        mockServices.addTransactionCategory.mockReturnValueOnce(ok(categoryInfo('orphan', CategoryType.Expense, 'unknown')));
        await store.saveCategory({
            category: TransactionCategory.createNewCategory(CategoryType.Expense, 'unknown'),
            isEdit: false,
            clientSessionId: 'session'
        });
        expect(store.allTransactionCategoriesMap['orphan']).toBeDefined();
    });

    test.each([
        ['create invalid', false, invalid(), { message: 'Unable to add category' }],
        ['edit invalid', true, invalid(), { message: 'Unable to save category' }],
        ['server error', false, Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['create unprocessed', false, Promise.reject({ processed: false }), { message: 'Unable to add category' }],
        ['edit unprocessed', true, Promise.reject({ processed: false }), { message: 'Unable to save category' }],
        ['processed', true, Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s when saving categories', async (_name, isEdit, response, expected) => {
        const store = useTransactionCategoriesStore();
        const category = TransactionCategory.of(categoryInfo(isEdit ? 'edit' : 'new', CategoryType.Expense));
        const method = isEdit ? mockServices.modifyTransactionCategory : mockServices.addTransactionCategory;
        method.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(store.saveCategory({ category, isEdit, clientSessionId: 'session' })).rejects.toEqual(expected);
    });

    test('adds normal and preset batches and invalidates a previously loaded list', async () => {
        mockServices.getAllTransactionCategories.mockReturnValue(ok(categoryMap()));
        const store = useTransactionCategoriesStore();
        await store.loadAllCategories({ force: false });
        const req = { categories: [] };
        mockServices.addTransactionCategoryBatch.mockReturnValue(ok(categoryMap()));
        await expect(store.addCategories(req)).resolves.toMatchObject({ [CategoryType.Expense]: expect.any(Array) });
        expect(store.transactionCategoryListStateInvalid).toBe(true);
        store.updateTransactionCategoryListInvalidState(false);
        await expect(store.addPresetCategories(req)).resolves.toMatchObject({ [CategoryType.Income]: expect.any(Array) });
        expect(store.transactionCategoryListStateInvalid).toBe(true);
    });

    test.each([
        ['normal invalid', 'addCategories', invalid(), { message: 'Unable to add category' }],
        ['preset invalid', 'addPresetCategories', invalid(), { message: 'Unable to add preset categories' }],
        ['server error', 'addCategories', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['normal unprocessed', 'addCategories', Promise.reject({ processed: false }), { message: 'Unable to add category' }],
        ['preset unprocessed', 'addPresetCategories', Promise.reject({ processed: false }), { message: 'Unable to add preset categories' }],
        ['processed', 'addPresetCategories', Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s batch category response', async (_name, action, response, expected) => {
        mockServices.addTransactionCategoryBatch.mockReturnValueOnce(response as ApiResponse<any>);
        const store = useTransactionCategoriesStore();
        await expect(store[action as 'addCategories' | 'addPresetCategories']({ categories: [] })).rejects.toEqual(expected);
    });

    test('reorders roots/subcategories and persists complete display order requests', async () => {
        const map = categoryMap();
        map[CategoryType.Expense]!.push(categoryInfo('expense-2', CategoryType.Expense, '0', false, [
            categoryInfo('expense-2-sub', CategoryType.Expense, 'expense-2')
        ]));
        map[CategoryType.Expense]![0]!.subCategories!.push(categoryInfo('expense-sub-2', CategoryType.Expense, 'expense'));
        mockServices.getAllTransactionCategories.mockReturnValue(ok(map));
        const store = useTransactionCategoriesStore();
        await store.loadAllCategories({ force: false });

        await store.changeCategoryDisplayOrder({ categoryId: 'expense', from: 0, to: 1 });
        expect(store.allTransactionCategories[CategoryType.Expense]?.map(item => item.id)).toEqual(['expense-2', 'expense']);
        store.updateTransactionCategoryListInvalidState(false);
        await store.changeCategoryDisplayOrder({ categoryId: 'expense-sub', from: 0, to: 1 });
        expect(store.allTransactionCategoriesMap['expense']?.subCategories?.map(item => item.id)).toEqual(['expense-sub-2', 'expense-sub']);

        mockServices.moveTransactionCategory.mockReturnValue(ok(true));
        await expect(store.updateCategoryDisplayOrders({ type: CategoryType.Expense, parentId: '0' })).resolves.toBe(true);
        expect(mockServices.moveTransactionCategory).toHaveBeenLastCalledWith({
            newDisplayOrders: [{ id: 'expense-2', displayOrder: 1 }, { id: 'expense', displayOrder: 2 }]
        });
        store.updateTransactionCategoryListInvalidState(true);
        await store.updateCategoryDisplayOrders({ type: CategoryType.Expense, parentId: 'expense' });
        await store.updateCategoryDisplayOrders({ type: CategoryType.Expense, parentId: 'unknown' });
        expect(mockServices.moveTransactionCategory).toHaveBeenLastCalledWith({ newDisplayOrders: [] });
    });

    test('rejects invalid moves and maps persistence errors', async () => {
        const map = categoryMap();
        map[CategoryType.Expense]![0]!.subCategories!.push(categoryInfo('expense-sub-2', CategoryType.Expense, 'expense'));
        mockServices.getAllTransactionCategories.mockReturnValue(ok(map));
        const store = useTransactionCategoriesStore();
        await store.loadAllCategories({ force: false });
        await expect(store.changeCategoryDisplayOrder({ categoryId: 'missing', from: 0, to: 0 })).rejects.toBeDefined();
        await expect(store.changeCategoryDisplayOrder({ categoryId: 'expense', from: 0, to: 9 })).rejects.toBeDefined();
        await expect(store.changeCategoryDisplayOrder({ categoryId: 'expense-sub', from: 0, to: 9 })).rejects.toBeDefined();

        mockServices.moveTransactionCategory.mockReturnValueOnce(invalid());
        await expect(store.updateCategoryDisplayOrders({ type: CategoryType.Expense, parentId: '0' })).rejects.toEqual({ message: 'Unable to move category' });
        mockServices.moveTransactionCategory.mockRejectedValueOnce(responseError());
        await expect(store.updateCategoryDisplayOrders({ type: CategoryType.Expense, parentId: '0' })).rejects.toEqual({ error: { message: 'server detail' } });
        mockServices.moveTransactionCategory.mockRejectedValueOnce({ processed: false });
        await expect(store.updateCategoryDisplayOrders({ type: CategoryType.Expense, parentId: '0' })).rejects.toEqual({ message: 'Unable to move category' });
        const processed = { processed: true };
        mockServices.moveTransactionCategory.mockRejectedValueOnce(processed);
        await expect(store.updateCategoryDisplayOrders({ type: CategoryType.Expense, parentId: '0' })).rejects.toBe(processed);
    });

    test('hides/unhides mapped categories and maps visibility failures', async () => {
        mockServices.getAllTransactionCategories.mockReturnValue(ok(categoryMap()));
        const store = useTransactionCategoriesStore();
        await store.loadAllCategories({ force: false });
        const category = store.allTransactionCategoriesMap['expense']!;
        mockServices.hideTransactionCategory.mockReturnValue(ok(true));
        await store.hideCategory({ category, hidden: true });
        expect(category.visible).toBe(false);
        await store.hideCategory({ category: TransactionCategory.of(categoryInfo('not-loaded', CategoryType.Expense)), hidden: false });

        mockServices.hideTransactionCategory.mockReturnValueOnce(invalid());
        await expect(store.hideCategory({ category, hidden: true })).rejects.toEqual({ message: 'Unable to hide this category' });
        mockServices.hideTransactionCategory.mockReturnValueOnce(invalid());
        await expect(store.hideCategory({ category, hidden: false })).rejects.toEqual({ message: 'Unable to unhide this category' });
        mockServices.hideTransactionCategory.mockRejectedValueOnce(responseError());
        await expect(store.hideCategory({ category, hidden: true })).rejects.toEqual({ error: { message: 'server detail' } });
        mockServices.hideTransactionCategory.mockRejectedValueOnce({ processed: false });
        await expect(store.hideCategory({ category, hidden: true })).rejects.toEqual({ message: 'Unable to hide this category' });
        mockServices.hideTransactionCategory.mockRejectedValueOnce({ processed: false });
        await expect(store.hideCategory({ category, hidden: false })).rejects.toEqual({ message: 'Unable to unhide this category' });
        const processed = { processed: true };
        mockServices.hideTransactionCategory.mockRejectedValueOnce(processed);
        await expect(store.hideCategory({ category, hidden: false })).rejects.toBe(processed);
    });

    test('deletes child/root categories immediately or through deferred callback', async () => {
        mockServices.getAllTransactionCategories.mockReturnValue(ok(categoryMap()));
        mockServices.deleteTransactionCategory.mockReturnValue(ok(true));
        const store = useTransactionCategoriesStore();
        await store.loadAllCategories({ force: false });
        const child = store.allTransactionCategoriesMap['expense-sub']!;
        await store.deleteCategory({ category: child });
        expect(store.allTransactionCategoriesMap['expense-sub']).toBeUndefined();

        const root = store.allTransactionCategoriesMap['income']!;
        let deferredRemoval: (() => void) | undefined;
        await store.deleteCategory({
            category: root,
            beforeResolve: callback => { deferredRemoval = callback; }
        });
        expect(store.allTransactionCategoriesMap['income']).toBeDefined();
        deferredRemoval!();
        expect(store.allTransactionCategoriesMap['income']).toBeUndefined();
        expect(store.allTransactionCategoriesMap['income-sub']).toBeUndefined();

        await store.deleteCategory({ category: TransactionCategory.of(categoryInfo('missing', CategoryType.Expense, 'missing-parent')) });
        await store.deleteCategory({ category: TransactionCategory.of(categoryInfo('missing-root', CategoryType.Expense)) });
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to delete this category' }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['unprocessed', Promise.reject({ processed: false }), { message: 'Unable to delete this category' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s category delete failure', async (_name, response, expected) => {
        mockServices.deleteTransactionCategory.mockReturnValueOnce(response as ApiResponse<any>);
        const store = useTransactionCategoriesStore();
        await expect(store.deleteCategory({ category: TransactionCategory.of(categoryInfo('x', CategoryType.Expense)) })).rejects.toEqual(expected);
    });

    test('resets category state and explicitly toggles invalidity', () => {
        const store = useTransactionCategoriesStore();
        expect(store.hasAvailableInvestmentCategories).toBe(false);
        store.updateTransactionCategoryListInvalidState(false);
        expect(store.transactionCategoryListStateInvalid).toBe(false);
        store.resetTransactionCategories();
        expect(store.allTransactionCategories).toEqual({});
        expect(store.allTransactionCategoriesMap).toEqual({});
        expect(store.transactionCategoryListStateInvalid).toBe(true);
    });
});

describe('transaction tag store behavior coverage', () => {
    test('loads/caches tags and derives visible counts', async () => {
        mockServices.getAllTransactionTags.mockReturnValue(ok([tagInfo('a'), tagInfo('b', true, 2)]));
        const store = useTransactionTagsStore();
        await store.loadAllTags({ force: false });
        await store.loadAllTags({ force: false });
        expect(mockServices.getAllTransactionTags).toHaveBeenCalledTimes(1);
        expect(store.allAvailableTagsCount).toBe(2);
        expect(store.allVisibleTagsCount).toBe(1);
        expect(store.allVisibleTags.map(tag => tag.id)).toEqual(['a']);
        expect(store.allTransactionTagsMap['b']?.hidden).toBe(true);
        mockServices.getAllTransactionTags.mockReturnValueOnce(ok([tagInfo('a')]));
        await store.loadAllTags({ force: true });
        expect(store.allAvailableTagsCount).toBe(1);
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to retrieve tag list' }, false],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }, false],
        ['unprocessed', Promise.reject({ processed: false }), { message: 'Unable to retrieve tag list' }, false],
        ['processed', Promise.reject({ processed: true }), { processed: true }, false],
        ['force server', Promise.reject(responseError()), { error: { message: 'server detail' } }, true]
    ])('maps %s tag load failure', async (_name, response, expected, force) => {
        mockServices.getAllTransactionTags.mockReturnValueOnce(response as ApiResponse<any>);
        const store = useTransactionTagsStore();
        await expect(store.loadAllTags({ force })).rejects.toEqual(expected);
    });

    test('creates/updates/batch-adds tags and invalidates the loaded list', async () => {
        mockServices.getAllTransactionTags.mockReturnValue(ok([tagInfo('existing')]));
        const store = useTransactionTagsStore();
        await store.loadAllTags({ force: false });
        mockServices.addTransactionTag.mockReturnValueOnce(ok(tagInfo('created')));
        await store.saveTag({ tag: TransactionTag.createNewTag('created') });
        expect(store.allTransactionTagsMap['created']).toBeDefined();
        mockServices.modifyTransactionTag.mockReturnValueOnce(ok({ ...tagInfo('existing'), name: 'updated' }));
        await store.saveTag({ tag: store.allTransactionTagsMap['existing']! });
        expect(store.allTransactionTagsMap['existing']?.name).toBe('updated');
        mockServices.modifyTransactionTag.mockReturnValueOnce(ok(tagInfo('map-only')));
        await store.saveTag({ tag: TransactionTag.of(tagInfo('map-only')) });
        expect(store.allTransactionTags.some(tag => tag.id === 'map-only')).toBe(false);
        expect(store.allTransactionTagsMap['map-only']).toBeDefined();

        mockServices.addTransactionTagBatch.mockReturnValue(ok([tagInfo('batch')]));
        store.updateTransactionTagListInvalidState(false);
        await expect(store.addTags({ tags: [{ name: 'batch' }], skipExists: true })).resolves.toEqual([
            expect.objectContaining({ id: 'batch' })
        ]);
        expect(store.transactionTagListStateInvalid).toBe(true);
    });

    test.each([
        ['create invalid', false, invalid(), { message: 'Unable to add tag' }],
        ['edit invalid', true, invalid(), { message: 'Unable to save tag' }],
        ['server', false, Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['create unprocessed', false, Promise.reject({ processed: false }), { message: 'Unable to add tag' }],
        ['edit unprocessed', true, Promise.reject({ processed: false }), { message: 'Unable to save tag' }],
        ['processed', true, Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s tag save failure', async (_name, edit, response, expected) => {
        const tag = edit ? TransactionTag.of(tagInfo('edit')) : TransactionTag.createNewTag('new');
        const method = edit ? mockServices.modifyTransactionTag : mockServices.addTransactionTag;
        method.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useTransactionTagsStore().saveTag({ tag })).rejects.toEqual(expected);
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to add tag' }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['unprocessed', Promise.reject({ processed: false }), { message: 'Unable to add tag' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s tag batch failure', async (_name, response, expected) => {
        mockServices.addTransactionTagBatch.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useTransactionTagsStore().addTags({ tags: [], skipExists: false })).rejects.toEqual(expected);
    });

    test('reorders and persists tag display order', async () => {
        mockServices.getAllTransactionTags.mockReturnValue(ok([tagInfo('a'), tagInfo('b', false, 2)]));
        const store = useTransactionTagsStore();
        await store.loadAllTags({ force: false });
        await store.changeTagDisplayOrder({ tagId: 'a', from: 0, to: 1 });
        expect(store.allTransactionTags.map(tag => tag.id)).toEqual(['b', 'a']);
        await expect(store.changeTagDisplayOrder({ tagId: 'missing', from: 0, to: 0 })).rejects.toBeDefined();
        await expect(store.changeTagDisplayOrder({ tagId: 'a', from: 1, to: 9 })).rejects.toBeDefined();
        mockServices.moveTransactionTag.mockReturnValue(ok(true));
        await expect(store.updateTagDisplayOrders()).resolves.toBe(true);
        expect(mockServices.moveTransactionTag).toHaveBeenCalledWith({
            newDisplayOrders: [{ id: 'b', displayOrder: 1 }, { id: 'a', displayOrder: 2 }]
        });
        expect(store.transactionTagListStateInvalid).toBe(false);
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to move tag' }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['unprocessed', Promise.reject({ processed: false }), { message: 'Unable to move tag' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s tag move failure', async (_name, response, expected) => {
        mockServices.moveTransactionTag.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useTransactionTagsStore().updateTagDisplayOrders()).rejects.toEqual(expected);
    });

    test('hides and deletes tags with direct/deferred list mutation', async () => {
        mockServices.getAllTransactionTags.mockReturnValue(ok([tagInfo('a'), tagInfo('b')]));
        mockServices.hideTransactionTag.mockReturnValue(ok(true));
        mockServices.deleteTransactionTag.mockReturnValue(ok(true));
        const store = useTransactionTagsStore();
        await store.loadAllTags({ force: false });
        await store.hideTag({ tag: store.allTransactionTagsMap['a']!, hidden: true });
        expect(store.allTransactionTagsMap['a']?.hidden).toBe(true);
        await store.hideTag({ tag: TransactionTag.of(tagInfo('missing')), hidden: false });
        await store.deleteTag({ tag: store.allTransactionTagsMap['a']! });
        expect(store.allTransactionTagsMap['a']).toBeUndefined();

        let remove!: () => void;
        const pendingDelete = store.deleteTag({
            tag: store.allTransactionTagsMap['b']!,
            beforeResolve: callback => { remove = callback; }
        });
        await Promise.resolve();
        expect(store.allTransactionTagsMap['b']).toBeDefined();
        remove();
        await pendingDelete;
        expect(store.allTransactionTagsMap['b']).toBeUndefined();
        await store.deleteTag({ tag: TransactionTag.of(tagInfo('not-loaded')) });
    });

    test.each([
        ['hide invalid', 'hide', invalid(), { message: 'Unable to update tag visibility' }],
        ['hide server', 'hide', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['hide unprocessed', 'hide', Promise.reject({ processed: false }), { message: 'Unable to update tag visibility' }],
        ['hide processed', 'hide', Promise.reject({ processed: true }), { processed: true }],
        ['delete invalid', 'delete', invalid(), { message: 'Unable to delete this tag' }],
        ['delete server', 'delete', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['delete unprocessed', 'delete', Promise.reject({ processed: false }), { message: 'Unable to delete this tag' }],
        ['delete processed', 'delete', Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s failure', async (_name, action, response, expected) => {
        const tag = TransactionTag.of(tagInfo('x'));
        if (action === 'hide') {
            mockServices.hideTransactionTag.mockReturnValueOnce(response as ApiResponse<any>);
            await expect(useTransactionTagsStore().hideTag({ tag, hidden: true })).rejects.toEqual(expected);
        } else {
            mockServices.deleteTransactionTag.mockReturnValueOnce(response as ApiResponse<any>);
            await expect(useTransactionTagsStore().deleteTag({ tag })).rejects.toEqual(expected);
        }
    });

    test('resets tag state', () => {
        const store = useTransactionTagsStore();
        store.updateTransactionTagListInvalidState(false);
        store.resetTransactionTags();
        expect(store.allTransactionTags).toEqual([]);
        expect(store.allTransactionTagsMap).toEqual({});
        expect(store.transactionTagListStateInvalid).toBe(true);
    });
});

describe('transaction template store behavior coverage', () => {
    test('loads templates per type, coalesces requests, derives visibility and caches', async () => {
        mockServices.getAllTransactionTemplates.mockReturnValue(ok([
            templateInfo('a'), templateInfo('b', TemplateType.Normal.type, TransactionType.Expense, true, 2)
        ]));
        const store = useTransactionTemplatesStore();
        const first = store.loadAllTemplates({ templateType: TemplateType.Normal.type, force: false });
        const second = store.loadAllTemplates({ templateType: TemplateType.Normal.type, force: false });
        await expect(Promise.all([first, second])).resolves.toHaveLength(2);
        expect(mockServices.getAllTransactionTemplates).toHaveBeenCalledTimes(1);
        await store.loadAllTemplates({ templateType: TemplateType.Normal.type, force: false });
        expect(mockServices.getAllTransactionTemplates).toHaveBeenCalledTimes(1);
        expect(store.allAvailableTemplatesCount[TemplateType.Normal.type]).toBe(2);
        expect(store.allVisibleTemplatesCount[TemplateType.Normal.type]).toBe(1);
        expect(store.allVisibleTemplates[TemplateType.Normal.type]?.map(item => item.id)).toEqual(['a']);

        mockServices.getAllTransactionTemplates.mockReturnValueOnce(ok([
            templateInfo('schedule', TemplateType.Schedule.type, TransactionType.Income)
        ]));
        await store.loadAllTemplates({ templateType: TemplateType.Schedule.type, force: false });
        expect(store.allTransactionTemplatesMap[TemplateType.Schedule.type]?.['schedule']).toBeDefined();
        mockServices.getAllTransactionTemplates.mockReturnValueOnce(ok([
            templateInfo('schedule', TemplateType.Schedule.type, TransactionType.Income)
        ]));
        await expect(store.loadAllTemplates({ templateType: TemplateType.Schedule.type, force: true })).rejects.toMatchObject({ isUpToDate: true });
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to retrieve template list' }, false],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }, false],
        ['unprocessed', Promise.reject({ processed: false }), { message: 'Unable to retrieve template list' }, false],
        ['processed', Promise.reject({ processed: true }), { processed: true }, false],
        ['force server', Promise.reject(responseError()), { error: { message: 'server detail' } }, true]
    ])('maps %s template load failure', async (_name, response, expected, force) => {
        mockServices.getAllTransactionTemplates.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useTransactionTemplatesStore().loadAllTemplates({ templateType: 1, force })).rejects.toEqual(expected);
    });

    test('gets template details and maps retrieval failures', async () => {
        const store = useTransactionTemplatesStore();
        mockServices.getTransactionTemplate.mockReturnValueOnce(ok(templateInfo('detail')));
        await expect(store.getTemplate({ templateId: 'detail' })).resolves.toMatchObject({ id: 'detail' });
        mockServices.getTransactionTemplate.mockReturnValueOnce(ok(templateInfo('typed')));
        await store.getTemplate({ templateId: 'typed', templateType: 2 });
        expect(mockServices.getTransactionTemplate).toHaveBeenLastCalledWith({ id: 'typed', templateType: 2 });
        mockServices.getTransactionTemplate.mockReturnValueOnce(invalid());
        await expect(store.getTemplate({ templateId: 'bad' })).rejects.toEqual({ message: 'Unable to retrieve template' });
        mockServices.getTransactionTemplate.mockRejectedValueOnce(responseError());
        await expect(store.getTemplate({ templateId: 'bad' })).rejects.toEqual({ error: { message: 'server detail' } });
        mockServices.getTransactionTemplate.mockRejectedValueOnce({ processed: false });
        await expect(store.getTemplate({ templateId: 'bad' })).rejects.toEqual({ message: 'Unable to retrieve template' });
        const processed = { processed: true };
        mockServices.getTransactionTemplate.mockRejectedValueOnce(processed);
        await expect(store.getTemplate({ templateId: 'bad' })).rejects.toBe(processed);
    });

    test('creates and updates supported templates and rejects unsupported transaction types', async () => {
        mockServices.getAllTransactionTemplates.mockReturnValue(ok([templateInfo('existing')]));
        const store = useTransactionTemplatesStore();
        await store.loadAllTemplates({ templateType: 1, force: false });
        const base = TransactionTemplate.ofTemplate(templateInfo('draft'));
        base.id = '';
        mockServices.addTransactionTemplate.mockReturnValueOnce(ok(templateInfo('created')));
        await store.saveTemplateContent({ template: base, isEdit: false, clientSessionId: 'session' });
        expect(store.allTransactionTemplatesMap[1]?.['created']).toBeDefined();

        mockServices.modifyTransactionTemplate.mockReturnValueOnce(ok({ ...templateInfo('existing'), name: 'updated' }));
        await store.saveTemplateContent({ template: store.allTransactionTemplatesMap[1]?.['existing']!, isEdit: true, clientSessionId: '' });
        expect(store.allTransactionTemplatesMap[1]?.['existing']?.name).toBe('updated');
        mockServices.modifyTransactionTemplate.mockReturnValueOnce(ok(templateInfo('map-only')));
        await store.saveTemplateContent({ template: TransactionTemplate.ofTemplate(templateInfo('map-only')), isEdit: true, clientSessionId: '' });
        expect(store.allTransactionTemplates[1]?.some(item => item.id === 'map-only')).toBe(false);

        const unsupported = TransactionTemplate.ofTemplate(templateInfo('investment', 1, TransactionType.Investment));
        await expect(store.saveTemplateContent({ template: unsupported, isEdit: false, clientSessionId: '' })).rejects.toEqual({ message: 'An error occurred' });
        const transfer = TransactionTemplate.ofTemplate(templateInfo('transfer', 1, TransactionType.Transfer));
        mockServices.addTransactionTemplate.mockReturnValueOnce(ok(templateInfo('transfer-created', 1, TransactionType.Transfer)));
        await store.saveTemplateContent({ template: transfer, isEdit: false, clientSessionId: 'session' });
        const income = TransactionTemplate.ofTemplate(templateInfo('income', 1, TransactionType.Income));
        mockServices.addTransactionTemplate.mockReturnValueOnce(ok(templateInfo('income-created', 1, TransactionType.Income)));
        await store.saveTemplateContent({ template: income, isEdit: false, clientSessionId: 'session' });
    });

    test.each([
        ['create invalid', false, invalid(), { message: 'Unable to add template' }],
        ['edit invalid', true, invalid(), { message: 'Unable to save template' }],
        ['server', false, Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['create unprocessed', false, Promise.reject({ processed: false }), { message: 'Unable to add template' }],
        ['edit unprocessed', true, Promise.reject({ processed: false }), { message: 'Unable to save template' }],
        ['processed', true, Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s template save failure', async (_name, edit, response, expected) => {
        const method = edit ? mockServices.modifyTransactionTemplate : mockServices.addTransactionTemplate;
        method.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useTransactionTemplatesStore().saveTemplateContent({
            template: TransactionTemplate.ofTemplate(templateInfo(edit ? 'edit' : 'new')),
            isEdit: edit,
            clientSessionId: 'session'
        })).rejects.toEqual(expected);
    });

    test('reorders and persists templates for loaded and absent type buckets', async () => {
        mockServices.getAllTransactionTemplates.mockReturnValue(ok([templateInfo('a'), templateInfo('b', 1, TransactionType.Expense, false, 2)]));
        const store = useTransactionTemplatesStore();
        await store.loadAllTemplates({ templateType: 1, force: false });
        await store.changeTemplateDisplayOrder({ templateType: 1, templateId: 'a', from: 0, to: 1 });
        expect(store.allTransactionTemplates[1]?.map(item => item.id)).toEqual(['b', 'a']);
        await expect(store.changeTemplateDisplayOrder({ templateType: 9, templateId: 'a', from: 0, to: 0 })).rejects.toBeDefined();
        await expect(store.changeTemplateDisplayOrder({ templateType: 1, templateId: 'missing', from: 0, to: 0 })).rejects.toBeDefined();
        await expect(store.changeTemplateDisplayOrder({ templateType: 1, templateId: 'a', from: 1, to: 9 })).rejects.toBeDefined();

        mockServices.moveTransactionTemplate.mockReturnValue(ok(true));
        await store.updateTemplateDisplayOrders({ templateType: 1 });
        expect(mockServices.moveTransactionTemplate).toHaveBeenLastCalledWith({
            templateType: 1,
            newDisplayOrders: [{ id: 'b', displayOrder: 1 }, { id: 'a', displayOrder: 2 }]
        });
        await store.updateTemplateDisplayOrders({ templateType: 9 });
        expect(mockServices.moveTransactionTemplate).toHaveBeenLastCalledWith({ templateType: 9, newDisplayOrders: [] });
    });

    test.each([
        ['invalid', invalid(), { message: 'Unable to move template' }],
        ['server', Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['unprocessed', Promise.reject({ processed: false }), { message: 'Unable to move template' }],
        ['processed', Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s template move failure', async (_name, response, expected) => {
        mockServices.moveTransactionTemplate.mockReturnValueOnce(response as ApiResponse<any>);
        await expect(useTransactionTemplatesStore().updateTemplateDisplayOrders({ templateType: 1 })).rejects.toEqual(expected);
    });

    test('hides/deletes templates with direct and deferred mutation', async () => {
        mockServices.getAllTransactionTemplates.mockReturnValue(ok([templateInfo('a'), templateInfo('b')]));
        mockServices.hideTransactionTemplate.mockReturnValue(ok(true));
        mockServices.deleteTransactionTemplate.mockReturnValue(ok(true));
        const store = useTransactionTemplatesStore();
        await store.loadAllTemplates({ templateType: 1, force: false });
        await store.hideTemplate({ template: store.allTransactionTemplatesMap[1]?.['a']!, hidden: true });
        expect(store.allTransactionTemplatesMap[1]?.['a']?.hidden).toBe(true);
        await store.hideTemplate({ template: TransactionTemplate.ofTemplate(templateInfo('unknown', 9)), hidden: false });
        await store.deleteTemplate({ template: store.allTransactionTemplatesMap[1]?.['a']! });
        expect(store.allTransactionTemplatesMap[1]?.['a']).toBeUndefined();

        let remove!: () => void;
        const pendingDelete = store.deleteTemplate({
            template: store.allTransactionTemplatesMap[1]?.['b']!,
            beforeResolve: callback => { remove = callback; }
        });
        await Promise.resolve();
        expect(store.allTransactionTemplatesMap[1]?.['b']).toBeDefined();
        remove();
        await pendingDelete;
        expect(store.allTransactionTemplatesMap[1]?.['b']).toBeUndefined();
        await store.deleteTemplate({ template: TransactionTemplate.ofTemplate(templateInfo('missing', 9)) });
    });

    test.each([
        ['hide invalid', 'hide', true, invalid(), { message: 'Unable to hide this template' }],
        ['unhide invalid', 'hide', false, invalid(), { message: 'Unable to unhide this template' }],
        ['hide server', 'hide', true, Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['hide unprocessed', 'hide', true, Promise.reject({ processed: false }), { message: 'Unable to hide this template' }],
        ['unhide unprocessed', 'hide', false, Promise.reject({ processed: false }), { message: 'Unable to unhide this template' }],
        ['hide processed', 'hide', true, Promise.reject({ processed: true }), { processed: true }],
        ['delete invalid', 'delete', false, invalid(), { message: 'Unable to delete this template' }],
        ['delete server', 'delete', false, Promise.reject(responseError()), { error: { message: 'server detail' } }],
        ['delete unprocessed', 'delete', false, Promise.reject({ processed: false }), { message: 'Unable to delete this template' }],
        ['delete processed', 'delete', false, Promise.reject({ processed: true }), { processed: true }]
    ])('maps %s failure', async (_name, action, hidden, response, expected) => {
        const template = TransactionTemplate.ofTemplate(templateInfo('x'));
        if (action === 'hide') {
            mockServices.hideTransactionTemplate.mockReturnValueOnce(response as ApiResponse<any>);
            await expect(useTransactionTemplatesStore().hideTemplate({ template, hidden })).rejects.toEqual(expected);
        } else {
            mockServices.deleteTransactionTemplate.mockReturnValueOnce(response as ApiResponse<any>);
            await expect(useTransactionTemplatesStore().deleteTemplate({ template })).rejects.toEqual(expected);
        }
    });

    test('resets template state and invalidity map', () => {
        const store = useTransactionTemplatesStore();
        store.updateTransactionTemplateListInvalidState(1, false);
        store.resetTransactionTemplates();
        expect(store.allTransactionTemplates).toEqual({});
        expect(store.allTransactionTemplatesMap).toEqual({});
        expect(store.transactionTemplateListStatesInvalid).toEqual({});
    });
});
