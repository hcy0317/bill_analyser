/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, describe, expect, jest, test } from '@jest/globals';

const { proxyRefs } = jest.requireActual('vue') as any;

jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: { name: 'SnackBarStub' },
}));
jest.mock('@/locales/helpers.ts', () => ({ useI18n: jest.fn() }));
jest.mock('@/views/base/settings/AccountFilterSettingPageBase.ts', () => ({
    useAccountFilterSettingPageBase: jest.fn(),
}));
jest.mock('@/views/base/settings/CategoryFilterSettingPageBase.ts', () => ({
    useCategoryFilterSettingPageBase: jest.fn(),
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: jest.fn() }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: jest.fn() }));
jest.mock('@/core/account.ts', () => ({
    AccountType: { MultiSubAccounts: { type: 8 } },
    AccountCategory: { values: () => [] },
}));
jest.mock('@/core/category.ts', () => ({ CategoryType: { Income: 1, Expense: 2, Transfer: 3 } }));
jest.mock('@/lib/account.ts', () => ({
    selectAccountOrSubAccounts: jest.fn(),
    selectAllVisible: jest.fn(),
    selectAll: jest.fn(),
    selectNone: jest.fn(),
    selectInvert: jest.fn(),
    isAccountOrSubAccountsAllChecked: jest.fn(),
    isAccountOrSubAccountsHasButNotAllChecked: jest.fn(),
}));
jest.mock('@/lib/category.ts', () => ({
    selectAllSubCategories: jest.fn(),
    selectAllVisible: jest.fn(),
    selectAll: jest.fn(),
    selectNone: jest.fn(),
    selectInvert: jest.fn(),
    isSubCategoriesAllChecked: jest.fn(),
    isSubCategoriesHasButNotAllChecked: jest.fn(),
}));

const AccountFilterSettingsCard = require(
    '@/views/desktop/common/cards/AccountFilterSettingsCard.vue'
).default as any;
const CategoryFilterSettingsCard = require(
    '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue'
).default as any;

let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

beforeAll(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    consoleWarnSpy.mockRestore();
});

interface RenderSummary {
    handlers: number;
    slots: number;
    text: string[];
}

function visitVNode(node: any, summary: RenderSummary, seen = new Set<any>()): void {
    if (node == null) return;
    if (typeof node === 'function') {
        try {
            summary.slots += 1;
            visitVNode(node({}), summary, seen);
        } catch {
            // Framework-owned scoped slots may require component-specific scope values.
        }
        return;
    }
    if (seen.has(node)) return;
    if (typeof node === 'string' || typeof node === 'number') {
        summary.text.push(String(node));
        return;
    }
    if (Array.isArray(node)) {
        for (const child of node) visitVNode(child, summary, seen);
        return;
    }
    if (typeof node !== 'object') return;
    seen.add(node);

    for (const [name, value] of Object.entries(node.props ?? {})) {
        if (name.startsWith('on') && typeof value === 'function') summary.handlers += 1;
    }

    visitVNode(node.children, summary, seen);
    if (node.children && typeof node.children === 'object' && !Array.isArray(node.children)) {
        for (const slot of Object.values(node.children)) {
            if (typeof slot !== 'function') continue;
            try {
                summary.slots += 1;
                visitVNode((slot as (scope?: any) => unknown)({}), summary, seen);
            } catch {
                // Framework-owned scoped slots may require component-specific scope values.
            }
        }
    }
}

function render(component: any, props: Record<string, unknown>, bindings: Record<string, unknown>): RenderSummary {
    const summary: RenderSummary = { handlers: 0, slots: 0, text: [] };
    const vnode = component.render({}, [], props, proxyRefs(bindings), {}, {});
    expect(vnode).toBeTruthy();
    visitVNode(vnode, summary);
    return summary;
}

function commonBindings(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        tt: (key: string) => `tt:${key}`,
        mdiDotsVertical: 'dots',
        mdiSelectAll: 'all',
        mdiSelect: 'none',
        mdiSelectInverse: 'inverse',
        mdiEyeOutline: 'show',
        mdiEyeOffOutline: 'hide',
        loading: false,
        showHidden: false,
        title: 'Filter',
        applyText: 'Apply',
        save: jest.fn(),
        cancel: jest.fn(),
        ...overrides,
    };
}

const visibleSubAccount = {
    id: 'sub-visible', name: 'Visible subaccount', type: 1, hidden: false, icon: 'cash', color: '#111111'
};
const hiddenSubAccount = {
    id: 'sub-hidden', name: 'Hidden subaccount', type: 1, hidden: true, icon: 'cash', color: '#222222'
};
const parentAccount = {
    id: 'parent', name: 'Parent account', type: 8, hidden: false, icon: 'wallet', color: '#333333'
};
const hiddenAccount = {
    id: 'hidden', name: 'Hidden account', type: 1, hidden: true, icon: 'bank', color: '#444444'
};
const accountCategory = {
    category: 1,
    name: 'Cash',
    allVisibleAccountCount: 1,
    firstVisibleAccountIndex: 0,
    allAccounts: [parentAccount, hiddenAccount],
    allSubAccounts: { parent: [visibleSubAccount, hiddenSubAccount] },
    allVisibleSubAccountCounts: { parent: 1 },
    allFirstVisibleSubAccountIndexes: { parent: 0 },
};

function accountBindings(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return commonBindings({
        allowHiddenAccount: true,
        hasAnyAvailableAccount: true,
        hasAnyVisibleAccount: true,
        allCategorizedAccounts: [accountCategory],
        expandAccountCategories: [1],
        filterAccountIds: { parent: true },
        AccountType: { MultiSubAccounts: { type: 8 } },
        isAccountOrSubAccountsAllChecked: jest.fn(() => true),
        isAccountOrSubAccountsHasButNotAllChecked: jest.fn(() => false),
        isAccountChecked: jest.fn(() => true),
        updateAccountOrSubAccountsSelected: jest.fn(),
        updateAccountSelected: jest.fn(),
        selectAllAccounts: jest.fn(),
        selectNoneAccounts: jest.fn(),
        selectInvertAccounts: jest.fn(),
        selectAllVisibleAccounts: jest.fn(),
        ...overrides,
    });
}

describe('AccountFilterSettingsCard production template', () => {
    test('renders the dialog loading state with only dialog actions', () => {
        const summary = render(
            AccountFilterSettingsCard,
            { type: 'overview', dialogMode: true },
            accountBindings({ loading: true, showHidden: false })
        );
        expect(summary.handlers).toBe(2);
    });

    test('renders inline visible and hidden parent/subaccount rows', () => {
        const visible = render(
            AccountFilterSettingsCard,
            { type: 'overview', dialogMode: false },
            accountBindings({ showHidden: false })
        );
        const hidden = render(
            AccountFilterSettingsCard,
            { type: 'overview', dialogMode: false },
            accountBindings({ showHidden: true })
        );
        expect(visible.handlers).toBeGreaterThanOrEqual(3);
        expect(hidden.handlers).toBeGreaterThanOrEqual(visible.handlers);
    });

    test('renders the empty dialog state without hidden-account support', () => {
        const summary = render(
            AccountFilterSettingsCard,
            { type: 'overview', dialogMode: true },
            accountBindings({
                allowHiddenAccount: false,
                hasAnyAvailableAccount: false,
                hasAnyVisibleAccount: false,
                allCategorizedAccounts: [],
            })
        );
        expect(summary.text.join(' ')).toContain('tt:No available account');
    });
});

const visibleSubCategory = {
    id: 'sub-visible', name: 'Visible subcategory', hidden: false, icon: 'food', color: '#111111'
};
const hiddenSubCategory = {
    id: 'sub-hidden', name: 'Hidden subcategory', hidden: true, icon: 'food', color: '#222222'
};
const primaryCategory = {
    id: 'primary', name: 'Primary category', hidden: false, icon: 'food', color: '#333333'
};
const hiddenCategory = {
    id: 'hidden', name: 'Hidden category', hidden: true, icon: 'food', color: '#444444'
};
const transactionType = {
    type: 1,
    allCategories: [primaryCategory, hiddenCategory],
    firstVisibleCategoryIndex: 0,
    allSubCategories: { primary: [visibleSubCategory, hiddenSubCategory] },
    allVisibleSubCategoryCounts: { primary: 1 },
    allFirstVisibleSubCategoryIndexes: { primary: 0 },
};

function categoryBindings(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return commonBindings({
        hasAnyAvailableCategory: true,
        hasAnyVisibleCategory: true,
        hasAvailableCategory: { 1: true },
        allTransactionCategories: [transactionType],
        expandCategoryTypes: [1],
        filterCategoryIds: { primary: true },
        getCategoryTypeName: (type: number) => `type:${type}`,
        isSubCategoriesAllChecked: jest.fn(() => true),
        isSubCategoriesHasButNotAllChecked: jest.fn(() => false),
        isCategoryChecked: jest.fn(() => true),
        updateAllSubCategoriesSelected: jest.fn(),
        updateCategorySelected: jest.fn(),
        selectAllCategories: jest.fn(),
        selectNoneCategories: jest.fn(),
        selectInvertCategories: jest.fn(),
        selectAllVisibleCategories: jest.fn(),
        ...overrides,
    });
}

describe('CategoryFilterSettingsCard production template', () => {
    test('renders the dialog loading state with only dialog actions', () => {
        const summary = render(
            CategoryFilterSettingsCard,
            { type: 'overview', dialogMode: true },
            categoryBindings({ loading: true, showHidden: false })
        );
        expect(summary.handlers).toBe(2);
    });

    test('renders inline visible and hidden category trees', () => {
        const visible = render(
            CategoryFilterSettingsCard,
            { type: 'overview', dialogMode: false },
            categoryBindings({ showHidden: false })
        );
        const hidden = render(
            CategoryFilterSettingsCard,
            { type: 'overview', dialogMode: false },
            categoryBindings({ showHidden: true })
        );
        expect(visible.handlers).toBeGreaterThanOrEqual(3);
        expect(hidden.handlers).toBeGreaterThanOrEqual(visible.handlers);
    });

    test('renders an empty category type and disabled dialog controls', () => {
        const emptyType = { ...transactionType, allCategories: [], allSubCategories: {} };
        const summary = render(
            CategoryFilterSettingsCard,
            { type: 'overview', dialogMode: true },
            categoryBindings({
                hasAnyAvailableCategory: false,
                hasAnyVisibleCategory: false,
                hasAvailableCategory: { 1: false },
                allTransactionCategories: [emptyType],
            })
        );
        expect(summary.text.join(' ')).toContain('tt:No available category');
    });
});
