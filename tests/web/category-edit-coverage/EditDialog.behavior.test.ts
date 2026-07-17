import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockCategoryType = {
    Income: 2,
    Expense: 3,
    Transfer: 4,
    Investment: 5
} as const;

class MockTransactionCategory {
    public id: string;
    public name: string;
    public parentId: string;
    public type: number;
    public icon: string;
    public color: string;
    public comment: string;
    public displayOrder: number;
    public visible: boolean;
    public ruleExpression: string;
    public categoryRules: any[];

    public constructor(overrides: Partial<MockTransactionCategory> = {}) {
        this.id = overrides.id ?? '';
        this.name = overrides.name ?? '';
        this.parentId = overrides.parentId ?? '0';
        this.type = overrides.type ?? mockCategoryType.Income;
        this.icon = overrides.icon ?? 'default-icon';
        this.color = overrides.color ?? '#123456';
        this.comment = overrides.comment ?? '';
        this.displayOrder = overrides.displayOrder ?? 0;
        this.visible = overrides.visible ?? true;
        this.ruleExpression = overrides.ruleExpression ?? '';
        this.categoryRules = overrides.categoryRules ?? [];
    }

    public static createNewCategory(type?: number, parentId?: string): MockTransactionCategory {
        return new MockTransactionCategory({
            type: type ?? mockCategoryType.Income,
            parentId: parentId || '0'
        });
    }

    public fillFrom(other: MockTransactionCategory): void {
        Object.assign(this, other);
    }

    public equals(other: MockTransactionCategory): boolean {
        return this.id === other.id
            && this.name === other.name
            && this.parentId === other.parentId
            && this.type === other.type
            && this.icon === other.icon
            && this.color === other.color
            && this.comment === other.comment
            && this.displayOrder === other.displayOrder
            && this.visible === other.visible
            && this.ruleExpression === other.ruleExpression;
    }
}

type MockBase = ReturnType<typeof createMockBase>;

let mockLastBase: MockBase;
let mockMountedCallback: (() => void) | undefined;
let mockUnmountedCallback: (() => void) | undefined;

const mockTemplateRefs = new Map<string, any>();
const mockTemplateEvents: Array<{ name: string; handler: (...args: any[]) => any }> = [];
const mockGenerateUuid = jest.fn<() => string>();
const mockAxios = {
    get: jest.fn<(...args: any[]) => Promise<any>>(),
    isAxiosError: jest.fn<(error: unknown) => boolean>()
};
const mockCategoryStore = {
    getCategory: jest.fn<(...args: any[]) => Promise<MockTransactionCategory>>(),
    saveCategory: jest.fn<(...args: any[]) => Promise<MockTransactionCategory>>()
};
const mockServices = {
    createCategoryRule: jest.fn<(...args: any[]) => Promise<any>>(),
    updateCategoryRule: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteCategoryRule: jest.fn<(...args: any[]) => Promise<any>>()
};

function createMockBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const category = ref(MockTransactionCategory.createNewCategory());
    const editCategoryId = ref(null as string | null);

    return {
        editCategoryId,
        clientSessionId: ref(''),
        loading: ref(false),
        submitting: ref(false),
        category,
        allAvailableCategories: ref([
            new MockTransactionCategory({ id: 'parent-1', name: 'Food', type: mockCategoryType.Expense }),
            new MockTransactionCategory({ id: 'parent-2', name: 'Salary', type: mockCategoryType.Income })
        ]),
        title: computed(() => editCategoryId.value ? 'Edit Category' : (
            category.value.parentId === '0' ? 'Add Primary Category' : 'Add Secondary Category'
        )),
        saveButtonTitle: computed(() => editCategoryId.value ? 'Save' : 'Add'),
        inputEmptyProblemMessage: computed(() => category.value.name ? null : 'Category name cannot be blank'),
        inputIsEmpty: computed(() => !category.value.name)
    };
}

function createCaptureStub(name: string): any {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [eventName, handler] of Object.entries(attrs)) {
                if (eventName.startsWith('on') && typeof handler === 'function') {
                    mockTemplateEvents.push({
                        name: eventName,
                        handler: handler as (...args: any[]) => any
                    });
                }
            }

            return () => h('div', attrs, Object.entries(slots).flatMap(([slotName, slot]) => {
                if (typeof slot !== 'function') {
                    return [];
                }
                if (slotName === 'activator') {
                    return (slot as (value: unknown) => unknown[])({ props: { title: 'activator' } });
                }
                if (slotName === 'item') {
                    return (slot as (value: unknown) => unknown[])({
                        props: { title: 'item' },
                        item: { raw: { icon: 'food', color: '#abcdef', name: 'Food' } }
                    });
                }
                return (slot as () => unknown[])();
            }));
        }
    });
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
        onMounted: (callback: () => void) => {
            mockMountedCallback = callback;
        },
        onUnmounted: (callback: () => void) => {
            mockUnmountedCallback = callback;
        }
    };
});

jest.mock('axios', () => ({
    __esModule: true,
    default: mockAxios
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => key })
}));

jest.mock('@/views/base/categories/CategoryEditPageBase.ts', () => ({
    useCategoryEditPageBase: () => {
        mockLastBase = createMockBase();
        return mockLastBase;
    }
}));

jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoryStore
}));

jest.mock('@/models/transaction_category.ts', () => ({
    TransactionCategory: MockTransactionCategory
}));

jest.mock('@/core/category.ts', () => ({ CategoryType: mockCategoryType }));
jest.mock('@/consts/icon.ts', () => ({ ALL_CATEGORY_ICONS: [{ id: 'food' }] }));
jest.mock('@/consts/color.ts', () => ({ ALL_CATEGORY_COLORS: [{ value: '#abcdef' }] }));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => mockGenerateUuid() }));
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));

for (const componentPath of [
    '@/components/desktop/ColorSelect.vue',
    '@/components/desktop/IconSelect.vue',
    '@/components/common/CategoryRuleBuilderFields.vue',
    '@/components/desktop/ItemIcon.vue',
    '@/components/desktop/SnackBar.vue'
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: createCaptureStub('CategoryEditCoverageChildStub')
    }));
}

const EditDialog = require('@/views/desktop/categories/list/dialogs/EditDialog.vue').default as any;

function createCategory(overrides: Partial<MockTransactionCategory> = {}): MockTransactionCategory {
    return new MockTransactionCategory(overrides);
}

function createBindings(): any {
    mockTemplateRefs.clear();
    const exposed: Record<string, unknown> = {};
    const bindings = EditDialog.setup({}, {
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value)
    });
    expect(exposed).toEqual({ open: bindings.open });
    return bindings;
}

function installSnackbar(bindings: any): { showError: jest.Mock; showMessage: jest.Mock } {
    const snackbar = { showError: jest.fn(), showMessage: jest.fn() };
    bindings.snackbar.value = snackbar;
    return snackbar;
}

async function flushAsync(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) {
        await Promise.resolve();
    }
    await new Promise(resolve => setImmediate(resolve));
}

async function cancelPending(bindings: any, pending: Promise<any>): Promise<unknown> {
    const rejection = pending.then(
        value => value,
        error => error
    );
    bindings.cancel();
    return rejection;
}

function successfulRuleList(rules: any[] = []): any {
    return { data: { success: true, data: rules } };
}

function successfulService(result: any = {}): any {
    return { data: { success: true, result } };
}

async function renderDialog(mutator: (bindings: any) => void): Promise<{ html: string; bindings: any }> {
    const { createSSRApp } = require('vue') as any;
    const { renderToString } = require('vue/server-renderer') as any;
    let bindings: any;
    const RuntimeDialog = {
        ...EditDialog,
        setup(props: any, context: any) {
            bindings = EditDialog.setup(props, context);
            mutator(bindings);
            return bindings;
        }
    };
    const app = createSSRApp(RuntimeDialog);
    for (const componentName of [
        'v-dialog', 'v-card', 'v-card-text', 'v-form', 'v-row', 'v-col', 'v-text-field',
        'v-select', 'v-list-item', 'v-progress-linear', 'v-textarea', 'v-switch', 'v-tooltip',
        'v-btn', 'v-progress-circular'
    ]) {
        app.component(componentName, createCaptureStub(`CategoryEditCoverage${componentName}`));
    }
    app.config.warnHandler = () => undefined;

    return {
        html: await renderToString(app),
        bindings
    };
}

beforeEach(() => {
    jest.resetAllMocks();
    mockTemplateRefs.clear();
    mockTemplateEvents.length = 0;
    mockMountedCallback = undefined;
    mockUnmountedCallback = undefined;
    mockGenerateUuid.mockReturnValue('category-session');
    mockAxios.isAxiosError.mockImplementation(error => !!error && typeof error === 'object' && (error as any).isAxiosError === true);
    mockAxios.get.mockResolvedValue(successfulRuleList());
    mockCategoryStore.getCategory.mockResolvedValue(createCategory({ id: 'category-1', name: 'Loaded' }));
    mockCategoryStore.saveCategory.mockResolvedValue(createCategory({ id: 'saved-1', name: 'Saved' }));
    mockServices.createCategoryRule.mockResolvedValue(successfulService({ id: 101 }));
    mockServices.updateCategoryRule.mockResolvedValue(successfulService({ id: 101 }));
    mockServices.deleteCategoryRule.mockResolvedValue(successfulService());

    Object.defineProperty(globalThis, 'HTMLInputElement', {
        configurable: true,
        value: class HTMLInputElement {},
        writable: true
    });
    Object.defineProperty(globalThis, 'HTMLTextAreaElement', {
        configurable: true,
        value: class HTMLTextAreaElement {},
        writable: true
    });
    Object.assign(globalThis.window, {
        addEventListener: jest.fn(),
        removeEventListener: jest.fn()
    });
});

describe('desktop category EditDialog production-loaded behavior coverage', () => {
    test('exposes dialog state and covers category/rule normalization helpers', () => {
        const bindings = createBindings();

        expect(bindings.isSecondaryCategory.value).toBe(false);
        expect(bindings.isCategoryModified.value).toBe(false);
        bindings.category.value.name = 'Changed';
        expect(bindings.isCategoryModified.value).toBe(true);
        bindings.editCategoryId.value = 'edit-1';
        expect(bindings.isCategoryModified.value).toBe(true);

        bindings.category.value.parentId = 'parent-1';
        bindings.category.value.name = '  Dining  ';
        bindings.categoryRuleBuilderModel.value = {
            priority: 7,
            ruleExpression: ' coffee ',
            regexEnabled: true,
            enabled: false
        };
        expect(bindings.categoryRuleDraft.value).toEqual({
            priority: 7,
            ruleExpression: ' coffee ',
            regexEnabled: true,
            enabled: false
        });
        expect(bindings.autoPrimaryRuleName.value).toBe('Food / Dining · P7');
        expect(bindings.getPrimaryRuleExpressionFromEditorState()).toBe('coffee');

        bindings.category.value.parentId = 'virtual_Custom';
        expect(bindings.resolvePrimaryCategoryName(bindings.category.value.parentId)).toBe('Custom');
        expect(bindings.autoPrimaryRuleName.value).toBe('Custom / Dining · P7');
        bindings.category.value.parentId = 'missing';
        bindings.category.value.name = '   ';
        expect(bindings.resolvePrimaryCategoryName('missing')).toBe('');
        expect(bindings.resolvePrimaryCategoryName('0')).toBe('');
        expect(bindings.resolvePrimaryCategoryName('')).toBe('');
        expect(bindings.autoPrimaryRuleName.value).toBe('Secondary Category · P7');

        expect(bindings.normalizeCategoryRuleDraft({
            priority: Number.NaN,
            ruleExpression: 12 as any,
            regexEnabled: 1 as any,
            enabled: undefined
        })).toEqual({
            priority: 100,
            ruleExpression: '12',
            regexEnabled: true,
            enabled: true
        });
        expect(bindings.normalizeCategoryRuleDraft(null)).toEqual(bindings.createEmptyCategoryRuleDraft());

        expect(bindings.getSortedCategoryRules([
            { id: 5, priority: 20 },
            { id: 3, priority: 10 },
            { id: 1, priority: 10 }
        ])).toEqual([
            { id: 1, priority: 10 },
            { id: 3, priority: 10 },
            { id: 5, priority: 20 }
        ]);
    });

    test('normalizes API error payloads and snackbar fallbacks', () => {
        const bindings = createBindings();
        const snackbar = installSnackbar(bindings);

        expect(bindings.extractPayloadMessage('direct')).toBe('direct');
        expect(bindings.extractPayloadMessage('')).toBeNull();
        expect(bindings.extractPayloadMessage(null)).toBeNull();
        expect(bindings.extractPayloadMessage({ error: { message: 'nested' } })).toBe('nested');
        expect(bindings.extractPayloadMessage({ error: { error: { error: 'too-deep' } } })).toBeNull();

        const axiosError = {
            isAxiosError: true,
            message: '',
            response: { data: { error: { message: 'axios nested' } } }
        };
        expect(bindings.getRequestErrorMessage(axiosError, 'fallback')).toBe('axios nested');
        expect(bindings.getRequestErrorMessage({ isAxiosError: true, message: 'axios message' }, 'fallback')).toBe('axios message');
        expect(bindings.getRequestErrorMessage({ isAxiosError: true }, 'fallback')).toBe('fallback');
        expect(bindings.getRequestErrorMessage(new Error('native error'), 'fallback')).toBe('native error');
        expect(bindings.getRequestErrorMessage({ message: 'object message' }, 'fallback')).toBe('object message');
        expect(bindings.getRequestErrorMessage({ error: { message: 'object nested' } }, 'fallback')).toBe('object nested');
        expect(bindings.getRequestErrorMessage(12, 'fallback')).toBe('fallback');

        bindings.showDialogError({ message: 'shown' }, 'fallback');
        expect(snackbar.showError).toHaveBeenCalledWith({ message: 'shown' });
        bindings.snackbar.value = null;
        bindings.showDialogError(null, 'silent fallback');

        expect(bindings.requireApiSuccess(successfulService({ id: 9 }), 'failed')).toEqual({ id: 9 });
        expect(() => bindings.requireApiSuccess({ data: { success: false } }, 'failed')).toThrow('failed');
        expect(() => bindings.requireApiSuccess({}, 'missing')).toThrow('missing');
    });

    test('opens every valid add mode, applies optional appearance, and rejects invalid types', async () => {
        for (const type of Object.values(mockCategoryType)) {
            const bindings = createBindings();
            const pending = bindings.open({
                parentId: 'parent-1',
                type,
                color: '#fedcba',
                icon: 'coffee'
            });
            expect(bindings.showState.value).toBe(true);
            expect(bindings.loading.value).toBe(false);
            expect(bindings.editCategoryId.value).toBeNull();
            expect(bindings.clientSessionId.value).toBe('category-session');
            expect(bindings.category.value).toMatchObject({
                parentId: 'parent-1', type, color: '#fedcba', icon: 'coffee'
            });
            expect(await cancelPending(bindings, pending)).toBeUndefined();
        }

        const withoutAppearance = createBindings();
        const pending = withoutAppearance.open({ parentId: '0', type: mockCategoryType.Expense });
        expect(withoutAppearance.category.value).toMatchObject({
            parentId: '0', icon: 'default-icon', color: '#123456'
        });
        expect(await cancelPending(withoutAppearance, pending)).toBeUndefined();

        const invalid = createBindings();
        await expect(invalid.open({ parentId: 'parent-1', type: 999 })).rejects.toBe('Parameter Invalid');
        expect(invalid.showState.value).toBe(false);
        expect(invalid.loading.value).toBe(false);

        const noMode = createBindings();
        const noModePending = noMode.open({});
        expect(noMode.loading.value).toBe(true);
        expect(await cancelPending(noMode, noModePending)).toBeUndefined();
    });

    test('loads an edited secondary category, preserves fallback appearance, and selects the canonical rule', async () => {
        const bindings = createBindings();
        const current = createCategory({
            id: '7',
            name: 'Current',
            parentId: 'parent-1',
            type: mockCategoryType.Expense,
            icon: 'kept-icon',
            color: '#654321'
        });
        mockCategoryStore.getCategory.mockResolvedValueOnce(createCategory({
            id: '7',
            name: 'Loaded child',
            parentId: 'parent-1',
            type: mockCategoryType.Expense,
            icon: '',
            color: ''
        }));
        mockAxios.get.mockResolvedValueOnce(successfulRuleList([
            { id: 22, category_id: 7, name: 'later', priority: 20, rule_expression: 'tea', regex_enabled: false, enabled: false },
            { id: 11, category_id: 7, name: 'primary', priority: 10, rule_expression: 'coffee', regex_enabled: true, enabled: true }
        ]));

        const pending = bindings.open({ id: '7', currentCategory: current });
        await flushAsync();

        expect(mockCategoryStore.getCategory).toHaveBeenCalledWith({ categoryId: '7' });
        expect(bindings.category.value.parentId).toBe('parent-1');
        expect(mockAxios.get).toHaveBeenCalledWith('category-rules/', {
            params: { category_id: 7, enabled_only: false }
        });
        expect(bindings.loading.value).toBe(false);
        expect(bindings.category.value).toMatchObject({
            name: 'Loaded child', icon: 'kept-icon', color: '#654321'
        });
        expect(bindings.primaryCategoryRuleId.value).toBe(11);
        expect(bindings.additionalCategoryRulesCount.value).toBe(1);
        expect(bindings.categoryRuleDraft.value).toEqual({
            priority: 10,
            ruleExpression: 'coffee',
            regexEnabled: true,
            enabled: true
        });
        expect(bindings.category.value.categoryRules).toHaveLength(2);
        expect(await cancelPending(bindings, pending)).toBeUndefined();
    });

    test('handles primary edit loads plus processed and unprocessed store failures', async () => {
        const primaryBindings = createBindings();
        mockCategoryStore.getCategory.mockResolvedValueOnce(createCategory({
            id: 'primary-1', name: 'Primary', parentId: '0', icon: 'loaded', color: '#ffffff'
        }));
        const primaryPending = primaryBindings.open({ id: 'primary-1' });
        await flushAsync();
        expect(mockAxios.get).not.toHaveBeenCalled();
        expect(primaryBindings.category.value.name).toBe('Primary');
        expect(await cancelPending(primaryBindings, primaryPending)).toBeUndefined();

        const failure = { processed: false, message: 'load failed' };
        const failedBindings = createBindings();
        mockCategoryStore.getCategory.mockRejectedValueOnce(failure);
        const failedPending = failedBindings.open({ id: 'failed-1' });
        await expect(failedPending).rejects.toBe(failure);
        expect(failedBindings.showState.value).toBe(false);
        expect(failedBindings.loading.value).toBe(false);

        const processedBindings = createBindings();
        mockCategoryStore.getCategory.mockRejectedValueOnce({ processed: true, message: 'already shown' });
        const processedPending = processedBindings.open({ id: 'failed-2' });
        await flushAsync();
        expect(processedBindings.showState.value).toBe(false);
        expect(await cancelPending(processedBindings, processedPending)).toBeUndefined();
    });

    test('resets, loads, and reports every category-rule load outcome', async () => {
        const bindings = createBindings();
        const snackbar = installSnackbar(bindings);

        bindings.ruleLoading.value = true;
        bindings.ruleLoadFailed.value = true;
        bindings.primaryCategoryRuleId.value = 44;
        bindings.additionalCategoryRulesCount.value = 5;
        await bindings.loadPrimaryCategoryRule(null);
        expect(bindings.ruleLoading.value).toBe(false);
        expect(bindings.primaryCategoryRuleId.value).toBeNull();

        bindings.category.value.parentId = '0';
        await bindings.loadPrimaryCategoryRule('12');
        expect(mockAxios.get).not.toHaveBeenCalled();

        bindings.category.value.parentId = 'parent-1';
        await bindings.loadPrimaryCategoryRule('not-a-number');
        expect(mockAxios.get).not.toHaveBeenCalled();

        mockAxios.get.mockResolvedValueOnce(successfulRuleList([]));
        await bindings.loadPrimaryCategoryRule('12');
        expect(bindings.primaryCategoryRuleId.value).toBeNull();
        expect(bindings.additionalCategoryRulesCount.value).toBe(0);
        expect(bindings.categoryRuleDraft.value).toEqual(bindings.createEmptyCategoryRuleDraft());

        mockAxios.get.mockResolvedValueOnce({ data: { success: false, error: 'server rule error' } });
        await bindings.loadPrimaryCategoryRule('12');
        expect(bindings.ruleLoadFailed.value).toBe(true);
        expect(snackbar.showError).toHaveBeenLastCalledWith({ message: 'server rule error' });

        mockAxios.get.mockRejectedValueOnce({ message: 'hidden error' });
        await bindings.loadPrimaryCategoryRule('12', { showError: false });
        expect(snackbar.showError).toHaveBeenCalledTimes(1);

        mockAxios.get.mockRejectedValueOnce(new Error('rethrow rule error'));
        await expect(bindings.loadPrimaryCategoryRule('12', {
            rethrowOnError: true,
            showError: false
        })).rejects.toThrow('rethrow rule error');
        expect(bindings.ruleLoading.value).toBe(false);
    });

    test('builds and synchronizes create, update, delete, and no-op primary rules', async () => {
        const bindings = createBindings();
        bindings.category.value.parentId = 'parent-1';

        bindings.categoryRuleDraft.value = {
            priority: 3,
            ruleExpression: '  coffee|tea  ',
            regexEnabled: true,
            enabled: false
        };
        expect(bindings.buildPrimaryCategoryRulePayload('bad-id')).toBeNull();
        expect(bindings.buildPrimaryCategoryRulePayload('91')).toEqual({
            category_id: 91,
            name: 'Food / Secondary Category · P3',
            priority: 3,
            rule_expression: 'coffee|tea',
            regex_enabled: true,
            enabled: false
        });

        await bindings.syncPrimaryCategoryRule('91');
        expect(mockServices.createCategoryRule).toHaveBeenCalledWith(expect.objectContaining({ category_id: 91 }));
        expect(bindings.primaryCategoryRuleId.value).toBe(101);

        await bindings.syncPrimaryCategoryRule('91');
        expect(mockServices.updateCategoryRule).toHaveBeenCalledWith(101, expect.objectContaining({
            rule_expression: 'coffee|tea'
        }));

        bindings.categoryRuleDraft.value.ruleExpression = '   ';
        await bindings.syncPrimaryCategoryRule('91');
        expect(mockServices.deleteCategoryRule).toHaveBeenCalledWith(101);
        expect(bindings.primaryCategoryRuleId.value).toBeNull();
        await bindings.syncPrimaryCategoryRule('91');
        expect(mockServices.deleteCategoryRule).toHaveBeenCalledTimes(1);

        bindings.categoryRuleDraft.value.ruleExpression = 'new rule';
        mockServices.createCategoryRule.mockResolvedValueOnce(successfulService({}));
        await bindings.syncPrimaryCategoryRule('91');
        expect(bindings.primaryCategoryRuleId.value).toBeNull();
        mockServices.createCategoryRule.mockResolvedValueOnce(successfulService({ id: null }));
        await bindings.syncPrimaryCategoryRule('91');
        expect(bindings.primaryCategoryRuleId.value).toBeNull();

        mockServices.createCategoryRule.mockResolvedValueOnce({ data: { success: false } });
        await expect(bindings.syncPrimaryCategoryRule('91')).rejects.toThrow('Failed to save category rule');
    });

    test('validates input and saves primary add/edit categories through the dialog promise', async () => {
        const invalid = createBindings();
        const invalidSnackbar = installSnackbar(invalid);
        await invalid.save();
        expect(invalidSnackbar.showMessage).toHaveBeenCalledWith('Category name cannot be blank');
        expect(mockCategoryStore.saveCategory).not.toHaveBeenCalled();

        const addBindings = createBindings();
        const addSnackbar = installSnackbar(addBindings);
        const addPending = addBindings.open({ parentId: '0', type: mockCategoryType.Expense });
        addBindings.category.value.name = 'Groceries';
        const added = createCategory({
            id: 'added-1', name: 'Groceries', parentId: '0', type: mockCategoryType.Expense
        });
        mockCategoryStore.saveCategory.mockResolvedValueOnce(added);
        await addBindings.save();
        await expect(addPending).resolves.toEqual({
            message: 'You have added a new category',
            id: 'added-1',
            category: added
        });
        expect(mockCategoryStore.saveCategory).toHaveBeenCalledWith({
            category: addBindings.category.value,
            isEdit: false,
            clientSessionId: 'category-session'
        });
        expect(addBindings.showState.value).toBe(false);
        expect(addBindings.submitting.value).toBe(false);
        expect(addSnackbar.showError).not.toHaveBeenCalled();

        const editBindings = createBindings();
        const editPending = editBindings.open({
            id: 'edit-1',
            currentCategory: createCategory({ id: 'edit-1', name: 'Before', parentId: '0' })
        });
        await flushAsync();
        editBindings.category.value.name = 'After';
        const edited = createCategory({ id: 'edit-1', name: 'After', parentId: '0' });
        mockCategoryStore.saveCategory.mockResolvedValueOnce(edited);
        await editBindings.save();
        await expect(editPending).resolves.toMatchObject({
            message: 'You have saved this category', id: 'edit-1'
        });
    });

    test('creates, reloads, updates, and deletes a secondary rule while saving', async () => {
        const addBindings = createBindings();
        installSnackbar(addBindings);
        const addPending = addBindings.open({ parentId: 'parent-1', type: mockCategoryType.Expense });
        addBindings.category.value.name = 'Cafe';
        addBindings.categoryRuleDraft.value = {
            priority: 8, ruleExpression: ' cafe ', regexEnabled: false, enabled: true
        };
        const added = createCategory({
            id: '1', name: 'Cafe', parentId: 'parent-1', type: mockCategoryType.Expense
        });
        mockCategoryStore.saveCategory.mockResolvedValueOnce(added);
        mockAxios.get.mockResolvedValueOnce(successfulRuleList([
            { id: 101, category_id: 1, name: 'rule', priority: 8, rule_expression: 'cafe', regex_enabled: false, enabled: true }
        ]));
        await addBindings.save();
        await expect(addPending).resolves.toMatchObject({ id: '1' });
        expect(mockServices.createCategoryRule).toHaveBeenCalledWith(expect.objectContaining({
            category_id: 1,
            rule_expression: 'cafe'
        }));
        expect(added.ruleExpression).toBe('cafe');

        const updateBindings = createBindings();
        installSnackbar(updateBindings);
        mockCategoryStore.getCategory.mockResolvedValueOnce(createCategory({
            id: '42', name: 'Child', parentId: 'parent-1', type: mockCategoryType.Expense
        }));
        mockAxios.get.mockResolvedValueOnce(successfulRuleList([
            { id: 7, category_id: 42, name: 'old', priority: 10, rule_expression: 'old', regex_enabled: false, enabled: true }
        ]));
        const updatePending = updateBindings.open({ id: '42' });
        await flushAsync();
        updateBindings.category.value.name = 'Child updated';
        updateBindings.categoryRuleDraft.value.ruleExpression = 'new';
        const updated = createCategory({
            id: '42', name: 'Child updated', parentId: 'parent-1', type: mockCategoryType.Expense
        });
        mockCategoryStore.saveCategory.mockResolvedValueOnce(updated);
        mockAxios.get.mockResolvedValueOnce(successfulRuleList([
            { id: 7, category_id: 42, name: 'new', priority: 10, rule_expression: 'new', regex_enabled: false, enabled: true }
        ]));
        await updateBindings.save();
        await expect(updatePending).resolves.toMatchObject({ id: '42' });
        expect(mockServices.updateCategoryRule).toHaveBeenCalledWith(7, expect.objectContaining({ rule_expression: 'new' }));

        const deleteBindings = createBindings();
        installSnackbar(deleteBindings);
        mockCategoryStore.getCategory.mockResolvedValueOnce(createCategory({
            id: '43', name: 'Child', parentId: 'parent-1', type: mockCategoryType.Expense
        }));
        mockAxios.get.mockResolvedValueOnce(successfulRuleList([
            { id: 8, category_id: 43, name: 'old', priority: 10, rule_expression: 'old', regex_enabled: false, enabled: true }
        ]));
        const deletePending = deleteBindings.open({ id: '43' });
        await flushAsync();
        deleteBindings.categoryRuleDraft.value.ruleExpression = '';
        const deletedRuleCategory = createCategory({
            id: '43', name: 'Child', parentId: 'parent-1', type: mockCategoryType.Expense
        });
        mockCategoryStore.saveCategory.mockResolvedValueOnce(deletedRuleCategory);
        mockAxios.get.mockResolvedValueOnce(successfulRuleList([]));
        await deleteBindings.save();
        await expect(deletePending).resolves.toMatchObject({ id: '43' });
        expect(mockServices.deleteCategoryRule).toHaveBeenCalledWith(8);
    });

    test('keeps the saved category open when store or rule synchronization fails', async () => {
        const storeFailureBindings = createBindings();
        const storeSnackbar = installSnackbar(storeFailureBindings);
        const storePending = storeFailureBindings.open({ parentId: '0', type: mockCategoryType.Expense });
        storeFailureBindings.category.value.name = 'Failure';
        mockCategoryStore.saveCategory.mockRejectedValueOnce({
            isAxiosError: true,
            response: { data: { error: 'save rejected' } }
        });
        await storeFailureBindings.save();
        expect(storeSnackbar.showError).toHaveBeenCalledWith({ message: 'save rejected' });
        expect(storeFailureBindings.showState.value).toBe(true);
        expect(storeFailureBindings.submitting.value).toBe(false);
        expect(await cancelPending(storeFailureBindings, storePending)).toBeUndefined();

        const ruleFailureBindings = createBindings();
        const ruleSnackbar = installSnackbar(ruleFailureBindings);
        const rulePending = ruleFailureBindings.open({ parentId: 'parent-1', type: mockCategoryType.Expense });
        ruleFailureBindings.category.value.name = 'Saved before rule failure';
        ruleFailureBindings.categoryRuleDraft.value.ruleExpression = 'rule';
        const saved = createCategory({
            id: '51', name: 'Saved before rule failure', parentId: 'parent-1', type: mockCategoryType.Expense
        });
        mockCategoryStore.saveCategory.mockResolvedValueOnce(saved);
        mockServices.createCategoryRule.mockResolvedValueOnce({ data: { success: false } });
        await ruleFailureBindings.save();
        expect(ruleFailureBindings.category.value.id).toBe('51');
        expect(ruleFailureBindings.editCategoryId.value).toBe('51');
        expect(ruleSnackbar.showError).toHaveBeenCalledWith({ message: 'Failed to save category rule' });
        expect(ruleFailureBindings.showState.value).toBe(true);
        expect(await cancelPending(ruleFailureBindings, rulePending)).toBeUndefined();

        mockServices.createCategoryRule.mockClear();
        const skippedRuleBindings = createBindings();
        const skippedPending = skippedRuleBindings.open({ parentId: 'parent-1', type: mockCategoryType.Expense });
        skippedRuleBindings.category.value.name = 'Skip rule';
        skippedRuleBindings.ruleLoadFailed.value = true;
        mockCategoryStore.saveCategory.mockResolvedValueOnce(createCategory({
            id: '52', name: 'Skip rule', parentId: 'parent-1', type: mockCategoryType.Expense
        }));
        await skippedRuleBindings.save();
        await expect(skippedPending).resolves.toMatchObject({ id: '52' });
        expect(mockServices.createCategoryRule).not.toHaveBeenCalled();
    });

    test('handles keyboard shortcuts and window listener lifecycle safely', async () => {
        const bindings = createBindings();
        const snackbar = installSnackbar(bindings);
        mockMountedCallback?.();
        expect(globalThis.window.addEventListener).toHaveBeenCalledWith('keydown', bindings.onKeydown);

        const preventDefault = jest.fn();
        bindings.onKeydown({ key: 'Enter', target: {}, preventDefault } as any);
        expect(preventDefault).not.toHaveBeenCalled();

        const pending = bindings.open({ parentId: '0', type: mockCategoryType.Income });
        bindings.onKeydown({
            key: 'Enter',
            target: new (globalThis.HTMLInputElement as any)(),
            preventDefault
        } as any);
        bindings.onKeydown({
            key: 'Backspace',
            target: new (globalThis.HTMLTextAreaElement as any)(),
            preventDefault
        } as any);
        expect(preventDefault).not.toHaveBeenCalled();

        bindings.onKeydown({ key: 'Escape', target: {}, preventDefault } as any);
        expect(preventDefault).not.toHaveBeenCalled();
        bindings.onKeydown({ key: 'Enter', target: {}, preventDefault } as any);
        await flushAsync();
        expect(snackbar.showMessage).toHaveBeenCalledWith('Category name cannot be blank');
        expect(preventDefault).toHaveBeenCalledTimes(1);

        bindings.onKeydown({ key: 'Backspace', target: {}, preventDefault } as any);
        expect(preventDefault).toHaveBeenCalledTimes(2);
        expect(await pending.then((value: unknown) => value, (error: unknown) => error)).toBeUndefined();

        mockUnmountedCallback?.();
        expect(globalThis.window.removeEventListener).toHaveBeenCalledWith('keydown', bindings.onKeydown);
    });

    test('renders primary and secondary template states and executes generated model handlers', async () => {
        const primary = await renderDialog(bindings => {
            bindings.showState.value = true;
            bindings.category.value.parentId = '0';
            bindings.category.value.name = '';
        });
        expect(primary.html).toContain('Add Primary Category');
        expect(primary.html).toContain('Category name cannot be blank');

        const secondary = await renderDialog(bindings => {
            bindings.showState.value = true;
            bindings.loading.value = true;
            bindings.submitting.value = true;
            bindings.ruleLoading.value = true;
            bindings.editCategoryId.value = '77';
            bindings.category.value = createCategory({
                id: '77', name: 'Dining', parentId: 'parent-1', type: mockCategoryType.Expense,
                icon: 'food', color: '#abcdef', comment: 'comment', visible: false, displayOrder: 9
            });
            bindings.categoryRuleDraft.value.ruleExpression = 'coffee';
        });
        expect(secondary.html).toContain('Edit Category');
        expect(secondary.html).toContain('Primary Category');
        expect(secondary.html).toContain('Category Matching');
        expect(secondary.html).toContain('Visible');

        const modelEvents = mockTemplateEvents.filter(event => event.name === 'onUpdate:modelValue');
        expect(modelEvents.length).toBeGreaterThanOrEqual(8);
        for (const event of modelEvents) {
            event.handler('template-update');
        }
        await flushAsync();
    });
});
