const { createSSRApp, h, ref } = jest.requireActual('vue');
const { renderToString } = jest.requireActual('vue/server-renderer');

import type { TransactionCategory } from '@/models/transaction_category.ts';
import { CategoryType } from '@/core/category.ts';

type BaseState = {
    loading: { value: boolean };
    primaryCategoryId: { value: string };
    currentPrimaryCategory: { value: TransactionCategory | undefined };
};

const mockBaseState: BaseState = {
    loading: ref(false),
    primaryCategoryId: ref('0'),
    currentPrimaryCategory: ref(undefined)
};

const mockStore = {
    allTransactionCategories: {} as Record<number, TransactionCategory[]>,
    allTransactionCategoriesMap: {} as Record<string, TransactionCategory>,
    transactionCategoryListStateInvalid: false,
    loadAllCategories: jest.fn(() => ({
        then: (resolve: (value: Record<number, TransactionCategory[]>) => void) => {
            resolve({});
            return { catch: () => undefined };
        }
    }))
};

jest.mock('@/views/base/categories/CategoryListPageBase.ts', () => ({
    useCategoryListPageBase: () => mockBaseState
}));

jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockStore
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentLanguageTextDirection: () => 'ltr'
    })
}));

jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: jest.fn(),
        showToast: jest.fn(),
        routeBackOnError: jest.fn()
    }),
    showLoading: jest.fn(),
    hideLoading: jest.fn(),
    onSwipeoutDeleted: jest.fn()
}));

const ListPage = require('@/views/mobile/categories/ListPage.vue').default;

function category(id: string, parentId = '0', type: CategoryType = CategoryType.Expense): TransactionCategory {
    return {
        id,
        parentId,
        type,
        name: id,
        icon: '1',
        color: 'ffffff',
        comment: '',
        displayOrder: 0,
        visible: true,
        subCategories: []
    } as unknown as TransactionCategory;
}

async function renderCategoryList({ id = '0', routeType = CategoryType.Expense, parent }: {
    id?: string;
    routeType?: CategoryType | number;
    parent?: TransactionCategory;
} = {}): Promise<string> {
    mockBaseState.loading.value = false;
    mockBaseState.primaryCategoryId.value = id;
    mockBaseState.currentPrimaryCategory.value = parent;
    mockStore.allTransactionCategories = {
        [CategoryType.Expense]: parent && parent.parentId === '0' ? [parent] : []
    };
    mockStore.allTransactionCategoriesMap = parent ? { [parent.id]: parent } : {};

    const app = createSSRApp(ListPage, {
        f7route: { query: { type: String(routeType), ...(id !== '0' ? { id } : {}) } },
        f7router: {}
    });
    app.config.compilerOptions.isCustomElement = (tag: string) => tag.startsWith('f7-');
    app.component('ItemIcon', { setup: () => () => h('span') });
    const passthrough = (_props: unknown, context: { attrs: Record<string, unknown>; slots: { default?: () => unknown } }) => () => h('a', context.attrs, context.slots.default?.());
    app.component('f7-list', { inheritAttrs: false, setup: (_props: unknown, context: { attrs: Record<string, unknown>; slots: { default?: () => unknown } }) => () => h('section', context.attrs, context.slots.default?.()) });
    app.component('f7-list-button', { inheritAttrs: false, setup: passthrough });
    return renderToString(app);
}

describe('mobile category list action interaction', () => {
    beforeEach(() => {
        mockStore.loadAllCategories.mockClear();
    });

    test('root page exposes primary action and disables secondary navigation', async () => {
        const html = await renderCategoryList();

        expect(html).toContain('data-testid="mobile.categories.action.add-primary"');
        expect(html).toContain('href="/category/add?type=3&amp;parentId=0"');
        expect(html).toContain('data-testid="mobile.categories.action.add-secondary"');
        expect(html).toContain('class="disabled"');
        expect(html).not.toContain('data-testid="mobile.categories.action.add-secondary" href=');
    });

    test('matching first-level parent creates a secondary URL with its parentId', async () => {
        const parent = category('p1');
        const html = await renderCategoryList({ id: 'p1', parent });
        const secondaryAction = html.match(/<a data-testid="mobile\.categories\.action\.add-secondary"[^>]*>/)?.[0] ?? '';

        expect(html).toContain('data-testid="mobile.categories.action.add-secondary"');
        expect(html).toContain('href="/category/add?type=3&amp;parentId=p1');
        expect(secondaryAction).not.toContain('disabled');
    });

    test.each([
        ['secondary id', category('s1', 'p1'), 's1'],
        ['wrong type', category('income-parent', '0', CategoryType.Income), 'income-parent']
    ])('does not navigate from %s', async (_label, parent, id) => {
        const html = await renderCategoryList({ id, parent });

        expect(html).toContain('data-testid="mobile.categories.action.add-secondary"');
        expect(html).toContain('class="disabled"');
        expect(html).not.toContain(`parentId=${id}`);
    });
});
