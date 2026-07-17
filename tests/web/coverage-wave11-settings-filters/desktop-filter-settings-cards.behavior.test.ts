/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockVue = jest.requireActual('vue') as any;
const { reactive, ref } = mockVue;
const mockTemplateRefs = new Map<string, any>();

const mockAccountBaseStates: any[] = [];
const mockCategoryBaseStates: any[] = [];
const mockTagBaseStates: any[] = [];

const mockLoadAllAccounts = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadAllCategories = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadAllTags = jest.fn<(...args: any[]) => Promise<void>>();

const mockAccountsStore = {
    allAccountsMap: { cash: { id: 'cash' }, bank: { id: 'bank' } },
    loadAllAccounts: (options: any) => mockLoadAllAccounts(options),
};
const mockCategoriesStore = {
    allTransactionCategoriesMap: { food: { id: 'food' }, salary: { id: 'salary' } },
    loadAllCategories: (options: any) => mockLoadAllCategories(options),
};
const mockTagsStore = {
    allTransactionTagsMap: { food: { id: 'food' }, travel: { id: 'travel' } },
    loadAllTags: (options: any) => mockLoadAllTags(options),
};

const mockSelectAccountOrSubAccounts = jest.fn();
const mockSelectAllAccounts = jest.fn();
const mockSelectNoneAccounts = jest.fn();
const mockSelectInvertAccounts = jest.fn();
const mockSelectAllVisibleAccounts = jest.fn();

const mockSelectAllSubCategories = jest.fn();
const mockSelectAllCategories = jest.fn();
const mockSelectNoneCategories = jest.fn();
const mockSelectInvertCategories = jest.fn();
const mockSelectAllVisibleCategories = jest.fn();

const mockSelectAllTags = jest.fn();
const mockSelectNoneTags = jest.fn();
const mockSelectInvertTags = jest.fn();
const mockSelectAllVisibleTags = jest.fn();

function createAccountBaseState(): any {
    const state = {
        loading: ref(true),
        showHidden: ref(false),
        filterAccountIds: ref({} as Record<string, boolean>),
        title: ref('Account Filter'),
        applyText: ref('Apply'),
        allowHiddenAccount: ref(false),
        allCategorizedAccounts: ref([]),
        hasAnyAvailableAccount: ref(true),
        hasAnyVisibleAccount: ref(true),
        isAccountChecked: jest.fn(),
        loadFilterAccountIds: jest.fn(() => true),
        saveFilterAccountIds: jest.fn(() => true),
    };
    mockAccountBaseStates.push(state);
    return state;
}

function createCategoryBaseState(): any {
    const state = {
        loading: ref(true),
        showHidden: ref(false),
        filterCategoryIds: ref({} as Record<string, boolean>),
        title: ref('Category Filter'),
        applyText: ref('Apply'),
        allTransactionCategories: ref([]),
        hasAnyAvailableCategory: ref(true),
        hasAnyVisibleCategory: ref(true),
        hasAvailableCategory: jest.fn(() => true),
        isCategoryChecked: jest.fn(),
        getCategoryTypeName: jest.fn((type: number) => `type:${type}`),
        loadFilterCategoryIds: jest.fn(() => true),
        saveFilterCategoryIds: jest.fn(() => false),
    };
    mockCategoryBaseStates.push(state);
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
        allTags: ref([]),
        allTagFilterTypes: ref([{ value: 'include', name: 'Include' }]),
        hasAnyAvailableTag: ref(true),
        hasAnyVisibleTag: ref(true),
        loadFilterTagIds: jest.fn(() => true),
        saveFilterTagIds: jest.fn(() => true),
    };
    mockTagBaseStates.push(state);
    return state;
}

jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: { name: 'SnackBarStub' },
}));
jest.mock('vue', () => ({
    ...jest.requireActual('vue') as object,
    useTemplateRef: (name: string) => mockTemplateRefs.get(name) ?? ref(null),
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/views/base/settings/AccountFilterSettingPageBase.ts', () => ({
    useAccountFilterSettingPageBase: jest.fn(() => createAccountBaseState()),
}));
jest.mock('@/views/base/settings/CategoryFilterSettingPageBase.ts', () => ({
    useCategoryFilterSettingPageBase: jest.fn(() => createCategoryBaseState()),
}));
jest.mock('@/views/base/settings/TransactionTagFilterSettingPageBase.ts', () => ({
    useTransactionTagFilterSettingPageBase: jest.fn(() => createTagBaseState()),
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoriesStore,
}));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagsStore }));
jest.mock('@/core/account.ts', () => ({
    AccountType: { MultiSubAccounts: { type: 8 } },
    AccountCategory: { values: () => [{ type: 1 }, { type: 2 }, { type: 3 }] },
}));
jest.mock('@/core/category.ts', () => ({
    CategoryType: { Income: 1, Expense: 2, Transfer: 3 },
}));
jest.mock('@/lib/account.ts', () => ({
    selectAccountOrSubAccounts: (...args: any[]) => mockSelectAccountOrSubAccounts(...args),
    selectAll: (...args: any[]) => mockSelectAllAccounts(...args),
    selectNone: (...args: any[]) => mockSelectNoneAccounts(...args),
    selectInvert: (...args: any[]) => mockSelectInvertAccounts(...args),
    selectAllVisible: (...args: any[]) => mockSelectAllVisibleAccounts(...args),
    isAccountOrSubAccountsAllChecked: jest.fn(() => false),
    isAccountOrSubAccountsHasButNotAllChecked: jest.fn(() => false),
}));
jest.mock('@/lib/category.ts', () => ({
    selectAllSubCategories: (...args: any[]) => mockSelectAllSubCategories(...args),
    selectAll: (...args: any[]) => mockSelectAllCategories(...args),
    selectNone: (...args: any[]) => mockSelectNoneCategories(...args),
    selectInvert: (...args: any[]) => mockSelectInvertCategories(...args),
    selectAllVisible: (...args: any[]) => mockSelectAllVisibleCategories(...args),
    isSubCategoriesAllChecked: jest.fn(() => false),
    isSubCategoriesHasButNotAllChecked: jest.fn(() => false),
}));
jest.mock('@/lib/common.ts', () => ({
    selectAll: (...args: any[]) => mockSelectAllTags(...args),
    selectNone: (...args: any[]) => mockSelectNoneTags(...args),
    selectInvert: (...args: any[]) => mockSelectInvertTags(...args),
    selectAllVisible: (...args: any[]) => mockSelectAllVisibleTags(...args),
}));

const AccountFilterSettingsCard = require(
    '@/views/desktop/common/cards/AccountFilterSettingsCard.vue'
).default as any;
const CategoryFilterSettingsCard = require(
    '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue'
).default as any;
const TransactionTagFilterSettingsCard = require(
    '@/views/desktop/common/cards/TransactionTagFilterSettingsCard.vue'
).default as any;

function setup(component: any, initialProps: Record<string, unknown>): any {
    const props = reactive(initialProps);
    const emit = jest.fn();
    const showError = jest.fn();
    mockTemplateRefs.set('snackbar', ref({ showError }));
    const bindings = component.setup(props, {
        attrs: {},
        slots: {},
        emit,
        expose: jest.fn(),
    });
    return { bindings, emit, props, showError };
}

async function flushPromises(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockAccountBaseStates.length = 0;
    mockCategoryBaseStates.length = 0;
    mockTagBaseStates.length = 0;
    mockTemplateRefs.clear();
    mockLoadAllAccounts.mockResolvedValue(undefined);
    mockLoadAllCategories.mockResolvedValue(undefined);
    mockLoadAllTags.mockResolvedValue(undefined);
});

describe('AccountFilterSettingsCard production-loaded behavior', () => {
    test('initializes category expansion, loads filters, and reports invalid parameters', async () => {
        const first = setup(AccountFilterSettingsCard, { type: 'overview' });
        const firstBase = mockAccountBaseStates[0];
        expect(first.bindings.expandAccountCategories.value).toEqual([1, 2, 3]);
        expect(mockLoadAllAccounts).toHaveBeenCalledWith({ force: false });
        await flushPromises();
        expect(firstBase.loading.value).toBe(false);
        expect(firstBase.loadFilterAccountIds).toHaveBeenCalledTimes(1);
        expect(first.showError).not.toHaveBeenCalled();

        mockLoadAllAccounts.mockImplementationOnce(async () => {
            mockAccountBaseStates[1].loadFilterAccountIds.mockReturnValue(false);
        });
        const invalid = setup(AccountFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(invalid.showError).toHaveBeenCalledWith('Parameter Invalid');
    });

    test('owns only unprocessed loading errors and always clears loading', async () => {
        mockLoadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'load failed' });
        const visible = setup(AccountFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(mockAccountBaseStates[0].loading.value).toBe(false);
        expect(visible.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'load failed' }));

        mockLoadAllAccounts.mockRejectedValueOnce({ processed: true, message: 'already handled' });
        const processed = setup(AccountFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(mockAccountBaseStates[1].loading.value).toBe(false);
        expect(processed.showError).not.toHaveBeenCalled();
    });

    test('updates individual and bulk selections with inverse checkbox semantics', async () => {
        const page = setup(AccountFilterSettingsCard, {
            type: 'overview', autoSave: false,
        });
        await flushPromises();
        const base = mockAccountBaseStates[0];
        const account = { id: 'cash' };

        page.bindings.updateAccountOrSubAccountsSelected(account, true);
        page.bindings.updateAccountSelected(account, null);
        expect(mockSelectAccountOrSubAccounts).toHaveBeenCalledWith(base.filterAccountIds.value, account, false);
        expect(base.filterAccountIds.value).toEqual({ cash: true });

        page.bindings.selectAllAccounts();
        page.bindings.selectNoneAccounts();
        page.bindings.selectInvertAccounts();
        page.bindings.selectAllVisibleAccounts();
        expect(mockSelectAllAccounts).toHaveBeenCalledWith(
            base.filterAccountIds.value, mockAccountsStore.allAccountsMap, true,
        );
        expect(mockSelectNoneAccounts).toHaveBeenCalledWith(
            base.filterAccountIds.value, mockAccountsStore.allAccountsMap, true,
        );
        expect(mockSelectInvertAccounts).toHaveBeenCalledWith(
            base.filterAccountIds.value, mockAccountsStore.allAccountsMap, true,
        );
        expect(mockSelectAllVisibleAccounts).toHaveBeenCalledWith(
            base.filterAccountIds.value, mockAccountsStore.allAccountsMap,
        );
        expect(base.saveFilterAccountIds).not.toHaveBeenCalled();
    });

    test('auto-saves every mutation and exposes explicit save and cancel results', async () => {
        const page = setup(AccountFilterSettingsCard, {
            type: 'overview', autoSave: true,
        });
        await flushPromises();
        const base = mockAccountBaseStates[0];
        base.allowHiddenAccount.value = true;
        const account = { id: 'cash' };

        page.bindings.updateAccountOrSubAccountsSelected(account, false);
        page.bindings.updateAccountSelected(account, true);
        page.bindings.selectAllAccounts();
        page.bindings.selectNoneAccounts();
        page.bindings.selectInvertAccounts();
        page.bindings.selectAllVisibleAccounts();
        expect(base.saveFilterAccountIds).toHaveBeenCalledTimes(6);
        expect(mockSelectAllAccounts).toHaveBeenLastCalledWith(
            base.filterAccountIds.value, mockAccountsStore.allAccountsMap, false,
        );
        expect(page.emit).toHaveBeenCalledTimes(6);
        expect(page.emit).toHaveBeenLastCalledWith('settings:change', true);

        page.bindings.save();
        page.bindings.cancel();
        expect(base.saveFilterAccountIds).toHaveBeenCalledTimes(7);
        expect(page.emit).toHaveBeenNthCalledWith(7, 'settings:change', true);
        expect(page.emit).toHaveBeenNthCalledWith(8, 'settings:change', false);
    });
});

describe('CategoryFilterSettingsCard production-loaded behavior', () => {
    test('initializes expanded types and covers successful, invalid, and failed loading', async () => {
        const success = setup(CategoryFilterSettingsCard, {
            type: 'overview', categoryTypes: '1,2',
        });
        expect(success.bindings.expandCategoryTypes.value).toEqual([1, 2, 3]);
        expect(mockLoadAllCategories).toHaveBeenCalledWith({ force: false });
        await flushPromises();
        expect(mockCategoryBaseStates[0].loading.value).toBe(false);

        mockLoadAllCategories.mockImplementationOnce(async () => {
            mockCategoryBaseStates[1].loadFilterCategoryIds.mockReturnValue(false);
        });
        const invalid = setup(CategoryFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(invalid.showError).toHaveBeenCalledWith('Parameter Invalid');

        mockLoadAllCategories.mockRejectedValueOnce({ processed: false, message: 'category failed' });
        const visible = setup(CategoryFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(mockCategoryBaseStates[2].loading.value).toBe(false);
        expect(visible.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'category failed' }));

        mockLoadAllCategories.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processed = setup(CategoryFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(processed.showError).not.toHaveBeenCalled();
    });

    test('guards missing categories and applies inverse selection semantics', async () => {
        const page = setup(CategoryFilterSettingsCard, {
            type: 'overview', autoSave: false,
        });
        await flushPromises();
        const base = mockCategoryBaseStates[0];
        const category = { id: 'food' };

        page.bindings.updateCategorySelected(null, true);
        expect(base.filterCategoryIds.value).toEqual({});
        page.bindings.updateCategorySelected(category, false);
        page.bindings.updateAllSubCategoriesSelected(category, null);
        expect(base.filterCategoryIds.value).toEqual({ food: true });
        expect(mockSelectAllSubCategories).toHaveBeenCalledWith(
            base.filterCategoryIds.value, true, category,
        );

        page.bindings.selectAllCategories();
        page.bindings.selectNoneCategories();
        page.bindings.selectInvertCategories();
        page.bindings.selectAllVisibleCategories();
        expect(mockSelectAllCategories).toHaveBeenCalledWith(
            base.filterCategoryIds.value, mockCategoriesStore.allTransactionCategoriesMap,
        );
        expect(mockSelectNoneCategories).toHaveBeenCalledWith(
            base.filterCategoryIds.value, mockCategoriesStore.allTransactionCategoriesMap,
        );
        expect(mockSelectInvertCategories).toHaveBeenCalledWith(
            base.filterCategoryIds.value, mockCategoriesStore.allTransactionCategoriesMap,
        );
        expect(mockSelectAllVisibleCategories).toHaveBeenCalledWith(
            base.filterCategoryIds.value, mockCategoriesStore.allTransactionCategoriesMap,
        );
        expect(base.saveFilterCategoryIds).not.toHaveBeenCalled();
    });

    test('auto-saves every mutation and emits the base save result', async () => {
        const page = setup(CategoryFilterSettingsCard, {
            type: 'overview', autoSave: true,
        });
        await flushPromises();
        const base = mockCategoryBaseStates[0];
        const category = { id: 'food' };

        page.bindings.updateCategorySelected(category, true);
        page.bindings.updateAllSubCategoriesSelected(category, false);
        page.bindings.selectAllCategories();
        page.bindings.selectNoneCategories();
        page.bindings.selectInvertCategories();
        page.bindings.selectAllVisibleCategories();
        expect(base.saveFilterCategoryIds).toHaveBeenCalledTimes(6);
        expect(page.emit).toHaveBeenCalledTimes(6);
        expect(page.emit).toHaveBeenLastCalledWith('settings:change', false);

        page.bindings.save();
        page.bindings.cancel();
        expect(base.saveFilterCategoryIds).toHaveBeenCalledTimes(7);
        expect(page.emit).toHaveBeenNthCalledWith(7, 'settings:change', false);
        expect(page.emit).toHaveBeenNthCalledWith(8, 'settings:change', false);
    });
});

describe('TransactionTagFilterSettingsCard production-loaded behavior', () => {
    test('initializes default expansion and covers all initialization outcomes', async () => {
        const success = setup(TransactionTagFilterSettingsCard, { type: 'overview' });
        expect(success.bindings.expandTagCategories.value).toEqual(['default']);
        expect(mockLoadAllTags).toHaveBeenCalledWith({ force: false });
        await flushPromises();
        expect(mockTagBaseStates[0].loading.value).toBe(false);

        mockLoadAllTags.mockImplementationOnce(async () => {
            mockTagBaseStates[1].loadFilterTagIds.mockReturnValue(false);
        });
        const invalid = setup(TransactionTagFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(invalid.showError).toHaveBeenCalledWith('Parameter Invalid');

        mockLoadAllTags.mockRejectedValueOnce({ processed: false, message: 'tag failed' });
        const visible = setup(TransactionTagFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(mockTagBaseStates[2].loading.value).toBe(false);
        expect(visible.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'tag failed' }));

        mockLoadAllTags.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processed = setup(TransactionTagFilterSettingsCard, { type: 'overview' });
        await flushPromises();
        expect(processed.showError).not.toHaveBeenCalled();
    });

    test('updates tag and bulk selection without saving when auto-save is disabled', async () => {
        const page = setup(TransactionTagFilterSettingsCard, {
            type: 'overview', autoSave: false,
        });
        await flushPromises();
        const base = mockTagBaseStates[0];
        const tag = { id: 'food' };

        page.bindings.updateTransactionTagSelected(tag, null);
        expect(base.filterTagIds.value).toEqual({ food: true });
        page.bindings.selectAllTransactionTags();
        page.bindings.selectNoneTransactionTags();
        page.bindings.selectInvertTransactionTags();
        page.bindings.selectAllVisibleTransactionTags();
        expect(mockSelectAllTags).toHaveBeenCalledWith(
            base.filterTagIds.value, mockTagsStore.allTransactionTagsMap,
        );
        expect(mockSelectNoneTags).toHaveBeenCalledWith(
            base.filterTagIds.value, mockTagsStore.allTransactionTagsMap,
        );
        expect(mockSelectInvertTags).toHaveBeenCalledWith(
            base.filterTagIds.value, mockTagsStore.allTransactionTagsMap,
        );
        expect(mockSelectAllVisibleTags).toHaveBeenCalledWith(
            base.filterTagIds.value, mockTagsStore.allTransactionTagsMap,
        );
        expect(base.saveFilterTagIds).not.toHaveBeenCalled();
    });

    test('auto-saves every mutation and supports explicit save and cancel', async () => {
        const page = setup(TransactionTagFilterSettingsCard, {
            type: 'overview', autoSave: true,
        });
        await flushPromises();
        const base = mockTagBaseStates[0];
        const tag = { id: 'food' };

        page.bindings.updateTransactionTagSelected(tag, true);
        page.bindings.selectAllTransactionTags();
        page.bindings.selectNoneTransactionTags();
        page.bindings.selectInvertTransactionTags();
        page.bindings.selectAllVisibleTransactionTags();
        expect(base.filterTagIds.value).toEqual({ food: false });
        expect(base.saveFilterTagIds).toHaveBeenCalledTimes(5);
        expect(page.emit).toHaveBeenCalledTimes(5);
        expect(page.emit).toHaveBeenLastCalledWith('settings:change', true);

        page.bindings.save();
        page.bindings.cancel();
        expect(base.saveFilterTagIds).toHaveBeenCalledTimes(6);
        expect(page.emit).toHaveBeenNthCalledWith(6, 'settings:change', true);
        expect(page.emit).toHaveBeenNthCalledWith(7, 'settings:change', false);
    });
});
