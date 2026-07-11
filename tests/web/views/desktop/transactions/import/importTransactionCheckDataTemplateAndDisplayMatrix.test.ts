import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockLoadAllCategories = jest.fn();
const mockLoadAllAccounts = jest.fn();

const categoryFixtures = [
    { id: 'expense-parent', parentId: '0', name: 'Food', type: 3, icon: 'food', color: '#ff8800', hidden: false, subCategories: [] as any[] },
    { id: 'income-parent', parentId: '0', name: 'Salary', type: 2, icon: 'income', color: '#00aa66', hidden: false, subCategories: [] as any[] },
    { id: 'transfer-parent', parentId: '0', name: 'Transfer', type: 4, icon: 'transfer', color: '#0088ff', hidden: false, subCategories: [] as any[] },
    { id: 'investment-parent', parentId: '0', name: 'Investment', type: 5, icon: 'investment', color: '#8866ff', hidden: false, subCategories: [] as any[] }
];
const expenseChild = {
    id: 'expense-child',
    parentId: 'expense-parent',
    name: 'Cafe',
    type: 3,
    icon: 'coffee',
    color: '#ff8800',
    hidden: false,
    subCategories: []
};
categoryFixtures[0]!.subCategories = [expenseChild];
const categoryMap = Object.fromEntries([...categoryFixtures, expenseChild].map(category => [category.id, category]));

const wallet = {
    id: 'wallet',
    name: 'Wallet',
    currency: 'CNY',
    category: 1,
    icon: 'wallet',
    color: '#0088ff',
    hidden: false
};
const bank = {
    id: 'bank',
    name: 'Bank',
    currency: 'USD',
    category: 2,
    icon: 'bank',
    color: '#00aa66',
    hidden: false
};
const knownTag = { id: 'known-tag', name: 'Known Tag', hidden: false };

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null)
    };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, params?: Record<string, unknown>) => params ? `${key}:${JSON.stringify(params)}` : key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatUnixTimeToLongDateTime: (value: unknown, offset?: unknown) => `DATE:${String(value)}:${String(offset ?? '')}`,
        formatAmountToLocalizedNumeralsWithCurrency: (value: unknown, currency: string) => `${currency}:${String(value)}`,
        getCategorizedAccountsWithDisplayBalance: (accounts: any[]) => [
            { category: 1, accounts: accounts.filter(account => account.category === 1) },
            { category: 2, accounts: accounts.filter(account => account.category === 2) }
        ]
    })
}));
jest.mock('@/stores/setting.ts', () => ({
    useSettingsStore: () => ({ appSettings: { showAccountBalance: false, timeZone: 'Asia/Shanghai' } })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({
        currentUserFirstDayOfWeek: 1,
        currentUserFiscalYearStart: 1,
        currentUserDefaultCurrency: 'CNY',
        currentUserCoordinateDisplayType: 0,
        currentUserCashTransferCategoryId: ''
    })
}));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({
        allPlainAccounts: [wallet, bank],
        allVisiblePlainAccounts: [wallet, bank],
        allAccountsMap: { wallet, bank },
        allAccounts: [wallet, bank],
        loadAllAccounts: mockLoadAllAccounts
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: {
            2: [categoryFixtures[1]],
            3: [categoryFixtures[0]],
            4: [categoryFixtures[2]],
            5: [categoryFixtures[3]]
        },
        allTransactionCategoriesMap: categoryMap,
        loadAllCategories: mockLoadAllCategories
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({
        allTransactionTags: [knownTag],
        allTransactionTagsMap: { 'known-tag': knownTag }
    })
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: { getLLMMemoryEvents: mockGetLLMMemoryEvents }
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isTransactionFromAIImageRecognitionEnabled: () => true
}));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => 'matrix-e-token' }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    showLoading: jest.fn(),
    hideLoading: jest.fn(),
    useI18nUIComponents: () => ({
        showAlert: jest.fn(),
        showConfirm: jest.fn(),
        showToast: jest.fn(),
        routeBackOnError: jest.fn()
    })
}));
jest.mock('@/views/desktop/transactions/import/check-data-tab/useImportCheckDataMenus.ts', () => ({
    useImportCheckDataMenus: () => ({ filterMenus: [], toolMenus: [] })
}));
jest.mock('@/views/desktop/transactions/import/check-data-tab/useImportCheckDataBatchActions.ts', () => ({
    useImportCheckDataBatchActions: () => ({
        clearSelectedRecurringMatches: jest.fn(),
        convertTransactionType: jest.fn(),
        showBatchAddDialog: jest.fn(),
        showBatchCreateInvalidItemDialog: jest.fn(),
        showBatchReplaceDialog: jest.fn(),
        showReplaceAllTypesDialog: jest.fn(),
        showReplaceInvalidItemDialog: jest.fn()
    })
}));

for (const componentPath of [
    '@/components/desktop/PaginationButtons.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/transactions/import/tabs/ImportPreviewSignalCell.vue',
    '@/views/desktop/transactions/import/dialogs/BatchReplaceDialog.vue',
    '@/views/desktop/transactions/import/dialogs/BatchReplaceAllTypesDialog.vue',
    '@/views/desktop/transactions/import/dialogs/BatchCreateDialog.vue',
    '@/views/desktop/transactions/import/dialogs/ImportLearningSuggestionDialog.vue',
    '@/views/desktop/categories/list/dialogs/EditDialog.vue',
    '@/views/desktop/accounts/list/dialogs/EditDialog.vue'
]) {
    jest.mock(componentPath, () => {
        const { defineComponent, h } = jest.requireActual('vue') as any;
        return {
            __esModule: true,
            default: defineComponent({
                name: 'MatrixEImportedStub',
                inheritAttrs: false,
                setup: (_props: unknown, { attrs, slots }: any) => () => h(
                    'div',
                    attrs,
                    Object.values(slots).flatMap(slot => typeof slot === 'function' ? (slot as any)({}) : [])
                )
            })
        };
    });
}

const { createSSRApp, defineComponent, h, proxyRefs } = jest.requireActual('vue') as any;
const { renderToString } = jest.requireActual('vue/server-renderer') as any;

import { ImportTransaction } from '@/models/imported_transaction.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';

type UiAction = {
    component: string;
    event: string;
    label: string;
    handler: (...args: any[]) => unknown;
};

const uiActions: UiAction[] = [];
let forcedNamedSlotItem: ImportTransaction | null = null;

function getNamedUiAction(label: string, occurrence = 0, event = 'onClick'): UiAction {
    const matches = uiActions.filter(action => action.event === event && action.label.includes(label));
    expect(matches.length).toBeGreaterThan(occurrence);
    return matches[occurrence]!;
}

function getBoundUiAction(handlerName: string, event = 'onClick'): UiAction {
    const action = uiActions.find(candidate => (
        candidate.event === event && String(candidate.handler).includes(handlerName)
    ));
    expect(action).toBeDefined();
    return action!;
}

function vnodeText(value: unknown): string {
    if (value === null || value === undefined || typeof value === 'boolean') {
        return '';
    }
    if (typeof value === 'string' || typeof value === 'number') {
        return String(value);
    }
    if (Array.isArray(value)) {
        return value.map(vnodeText).join(' ');
    }
    if (typeof value === 'object') {
        const vnode = value as { children?: unknown; props?: Record<string, unknown> };
        return [vnodeText(vnode.children), vnodeText(vnode.props?.['text']), vnodeText(vnode.props?.['title'])]
            .filter(Boolean)
            .join(' ');
    }
    return '';
}

function createUiStub(component: string): any {
    return defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => {
            const payload = {
                props: {},
                index: 0,
                item: { value: 'known-tag', raw: { hidden: false }, title: 'Known Tag' }
            };
            const children = Object.values(slots).flatMap(slot => (
                typeof slot === 'function' ? (slot as any)(payload) : []
            ));
            const label = [vnodeText(children), vnodeText(attrs['title']), vnodeText(attrs['text'])]
                .filter(Boolean)
                .join(' ')
                .trim();
            for (const [event, value] of Object.entries(attrs)) {
                if (!event.startsWith('on')) {
                    continue;
                }
                for (const handler of Array.isArray(value) ? value : [value]) {
                    if (typeof handler === 'function') {
                        uiActions.push({ component, event, label, handler: handler as (...args: any[]) => unknown });
                    }
                }
            }
            return h(component, attrs, children);
        }
    });
}

const DataTableStub = defineComponent({
    props: { items: { type: Array, default: () => [] } },
    setup: (props: any, { slots }: any) => () => {
        const children: unknown[] = [
            slots.default?.(),
            slots['header.data-table-select']?.(),
            slots.bottom?.()
        ];
        const renderedItems = forcedNamedSlotItem ? [forcedNamedSlotItem] : props.items;
        for (const item of renderedItems) {
            for (const [name, slot] of Object.entries(slots)) {
                if (name.startsWith('item.') && typeof slot === 'function') {
                    children.push((slot as any)({ item }));
                }
            }
        }
        return h('div', { 'data-testid': 'matrix-e-data-table' }, children);
    }
});

function registerUiStubs(app: any): void {
    app.component('v-data-table', DataTableStub);
    for (const name of [
        'v-dialog', 'v-card', 'v-card-title', 'v-card-text', 'v-card-actions', 'v-btn', 'v-btn-group',
        'v-chip', 'v-list', 'v-list-item', 'v-list-item-title', 'v-list-item-subtitle', 'v-checkbox',
        'v-menu', 'v-divider', 'v-select', 'v-autocomplete', 'v-text-field', 'v-tabs', 'v-tab',
        'v-spacer', 'v-icon', 'v-alert', 'v-progress-circular', 'two-column-select', 'icon-select',
        'date-range-selection-dialog', 'amount-input', 'item-icon'
    ]) {
        app.component(name, createUiStub(name));
    }
    app.config.warnHandler = () => undefined;
}

type TransactionOptions = {
    type?: number;
    selected?: boolean;
    valid?: boolean;
    categoryId?: string;
    categoryName?: string;
    sourceAccountId?: string;
    sourceAccountName?: string;
    destinationAccountId?: string;
    destinationAccountName?: string;
    utcOffset?: number;
    tagIds?: string[];
    tagNames?: string[];
    counterparty?: string;
    paymentMethod?: string;
    comment?: string;
    manuallyAnnotated?: boolean;
    geoLocation?: { latitude: number; longitude: number } | null;
    historyRewrite?: boolean;
};

function createTransaction(id: number, options: TransactionOptions = {}): ImportTransaction {
    const transaction = ImportTransaction.of({
        type: options.type ?? 3,
        categoryId: options.categoryId ?? 'expense-child',
        originalCategoryName: options.categoryName ?? 'Cafe',
        time: 1_788_480_000 + id,
        utcOffset: options.utcOffset ?? 480,
        sourceAccountId: options.sourceAccountId ?? 'wallet',
        originalSourceAccountName: options.sourceAccountName ?? 'Wallet',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: options.destinationAccountId ?? '',
        originalDestinationAccountName: options.destinationAccountName ?? '',
        originalDestinationAccountCurrency: 'USD',
        sourceAmountCents: -1_000 - id,
        destinationAmountCents: 2_000 + id,
        tagIds: options.tagIds ?? ['known-tag'],
        originalTagNames: options.tagNames ?? ['Known Tag'],
        counterparty: options.counterparty ?? `Merchant ${id}`,
        paymentMethod: options.paymentMethod ?? 'Card',
        comment: options.comment ?? `Memo ${id}`,
        selected: options.selected ?? true,
        geoLocation: options.geoLocation ?? undefined,
        matching: {
            parser: { id: 'fixture', tags: ['matrix-e'] },
            transfer: { review_status: '', reviewed_type: '', suppressed: false },
            learning: { review_status: '', source: 'model', model_version: 'matrix-e' },
            llm: { review_status: '' },
            reconciliation: options.historyRewrite ? {
                planned_operation: 'update_history',
                history_bill_id: id,
                history_bill_version: 2,
                operation_id: `operation-${id}`,
                acknowledgement_token: `ack-${id}`,
                destructive_ack_required: true,
                notice: 'History rewrite notice'
            } : undefined,
            annotation: options.historyRewrite ? { history_rewrite_notice: 'History rewrite notice' } : {}
        }
    } as never, id);
    Object.assign(transaction, {
        index: id,
        valid: options.valid ?? true,
        selected: options.selected ?? true,
        isManuallyAnnotated: options.manuallyAnnotated ?? false
    });
    if (options.historyRewrite) {
        transaction.matching!.reconciliation = {
            planned_operation: 'update_history',
            history_bill_id: id,
            history_bill_version: 2,
            operation_id: `operation-${id}`,
            acknowledgement_token: `ack-${id}`,
            destructive_ack_required: true,
            notice: 'History rewrite notice'
        } as never;
        transaction.matching!.annotation = { history_rewrite_notice: 'History rewrite notice' } as never;
    }
    (transaction as ImportTransaction & { _previewId: number })._previewId = id;
    return transaction;
}

type RenderOptions = {
    transactions: ImportTransaction[];
    disabled?: boolean;
    serverPaged?: boolean;
    total?: number;
    metadata?: Record<string, unknown> | null;
    sessionId?: string;
    configure?: (bindings: any) => void;
};

async function renderDesktop(options: RenderOptions): Promise<{ html: string; bindings: any; emit: ReturnType<typeof jest.fn> }> {
    uiActions.length = 0;
    forcedNamedSlotItem = null;
    const emit = jest.fn();
    let bindings: any;
    const props = {
        importTransactions: options.transactions,
        disabled: options.disabled ?? false,
        sessionId: options.sessionId ?? 'matrix-e-session',
        serverPaged: options.serverPaged ?? false,
        totalImportTransactionCount: options.total ?? options.transactions.length,
        previewMetadata: options.metadata ?? null
    };
    const Harness = defineComponent({
        setup: () => {
            bindings = (ImportTransactionCheckDataTab as any).setup(props, { emit, expose: jest.fn() });
            options.configure?.(bindings);
            const renderBindings = proxyRefs(bindings);
            return () => (ImportTransactionCheckDataTab as any).render(
                renderBindings,
                [],
                props,
                renderBindings,
                {},
                {}
            );
        }
    });
    const app = createSSRApp(Harness);
    registerUiStubs(app);
    const html = await renderToString(app);
    return { html, bindings, emit };
}

function createBindings(options: RenderOptions, emit = jest.fn()): any {
    return (ImportTransactionCheckDataTab as any).setup({
        importTransactions: options.transactions,
        disabled: options.disabled ?? false,
        sessionId: options.sessionId ?? 'matrix-e-session',
        serverPaged: options.serverPaged ?? false,
        totalImportTransactionCount: options.total ?? options.transactions.length,
        previewMetadata: options.metadata ?? null
    }, { emit, expose: jest.fn() });
}

beforeEach(() => {
    jest.clearAllMocks();
    uiActions.length = 0;
    mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
    Object.defineProperty(globalThis, 'fetch', {
        configurable: true,
        writable: true,
        value: jest.fn()
    });
});

describe('desktop import Matrix-E template and display contracts', () => {
    test('named table slots render each type plus valid, missing, optional, and destination states', async () => {
        const rows = [
            createTransaction(1, { type: 1, categoryId: '', tagIds: [], tagNames: [], geoLocation: null }),
            createTransaction(2, { type: 2, categoryId: 'income-parent', utcOffset: 0, manuallyAnnotated: true }),
            createTransaction(3, {
                type: 3,
                categoryId: 'missing-category',
                categoryName: 'Imported Mystery',
                sourceAccountId: '',
                sourceAccountName: 'Imported Wallet',
                tagIds: ['missing-tag'],
                tagNames: ['Imported Tag'],
                counterparty: '',
                paymentMethod: '',
                comment: '',
                valid: false,
                geoLocation: { latitude: 31.23, longitude: 121.47 },
                historyRewrite: true
            }),
            createTransaction(4, {
                type: 4,
                categoryId: 'transfer-parent',
                destinationAccountId: 'bank',
                destinationAccountName: 'Bank'
            }),
            createTransaction(5, {
                type: 5,
                categoryId: 'investment-parent',
                destinationAccountId: '',
                destinationAccountName: 'Imported Brokerage'
            }),
            createTransaction(6, { type: 99, categoryId: '0', sourceAccountId: '0' }),
            ...[7, 8, 9, 10, 11, 12].map(id => createTransaction(id, { type: 3 }))
        ];

        const { html } = await renderDesktop({ transactions: rows, total: 12 });

        expect(html).toContain('Modify Balance');
        expect(html).toContain('Income');
        expect(html).toContain('Expense');
        expect(html).toContain('Transfer');
        expect(html).toContain('Investment');
        expect(html).toContain('Unknown');
        expect(html).toContain('Cafe');
        expect(html).toContain('Imported Mystery');
        expect(html).toContain('Wallet');
        expect(html).toContain('Bank');
        expect(html).toContain('Imported Brokerage');
        expect(html).toContain('Known Tag');
        expect(html).toContain('Imported Tag');
        expect(html).toContain('None');
        expect(html).toContain('UTC');
        expect(html).toContain('CNY:-1004');
        expect(html).toContain('USD:2004');
        expect(html).toContain('Transactions Per Page');
        expect(html).toContain('Manage Categories');
        expect(html).toContain('Manage Accounts');
    });

    test('editing slots render selectors, destination amounts, editable tags, and text fields', async () => {
        const transfer = createTransaction(20, {
            type: 4,
            categoryId: 'transfer-parent',
            destinationAccountId: 'bank',
            destinationAccountName: 'Bank',
            tagIds: ['known-tag', 'missing-tag'],
            tagNames: ['Known Tag', 'Imported Tag']
        });
        const { html, bindings } = await renderDesktop({
            transactions: [transfer],
            configure: current => {
                current.editingTransaction.value = current.tableTransactions.value[0];
                current.editingTags.value = [...current.tableTransactions.value[0].tagIds];
                forcedNamedSlotItem = current.editingTransaction.value;
            }
        });

        expect(bindings.editingTransaction.value._previewId).toBe(20);
        expect(html).toContain('Find category');
        expect(html).toContain('Add Primary Category');
        expect(html).toContain('Add Secondary Category');
        expect(html).toContain('Amount');
        expect(html).toContain('Destination Amount');
        expect(html).toContain('Find account');
        expect(html).toContain('Source Account');
        expect(html).toContain('Destination Account');
        expect(html).toContain('No available tag');
        expect(html).toContain('Counterparty');
        expect(html).toContain('Payment Method');
        expect(html).toContain('Description');

        const balance = createTransaction(21, { type: 1, categoryId: '', destinationAccountId: '' });
        const balanceRender = await renderDesktop({
            transactions: [balance],
            disabled: true,
            configure: current => {
                current.editingTransaction.value = current.tableTransactions.value[0];
                current.editingTags.value = [...current.tableTransactions.value[0].tagIds];
                forcedNamedSlotItem = current.editingTransaction.value;
            }
        });
        expect(balanceRender.html).toContain('<v-select density="compact" variant="plain" hide-details disabled');
        expect(balanceRender.html).toContain('<span>-</span>');
        expect(balanceRender.html).toContain('placeholder="Amount"');
        expect(balanceRender.html).not.toContain('placeholder="Category"');
    });

    test('named v-model bindings update each editable draft and dialog state explicitly', async () => {
        const transfer = createTransaction(22, {
            type: 4,
            categoryId: 'transfer-parent',
            destinationAccountId: 'bank',
            destinationAccountName: 'Bank'
        });
        const second = createTransaction(23);
        const { bindings } = await renderDesktop({
            transactions: [transfer, second],
            configure: current => {
                current.editingTransaction.value = current.tableTransactions.value[0];
                current.editingTags.value = [...current.tableTransactions.value[0].tagIds];
                forcedNamedSlotItem = current.editingTransaction.value;
            }
        });
        const setModel = async (bindingName: string, value: unknown): Promise<void> => {
            await getBoundUiAction(bindingName, 'onUpdate:modelValue').handler(value);
        };

        await setModel('item.selected', false);
        expect(bindings.editingTransaction.value.selected).toBe(false);

        await setModel('item.type', 5);
        await setModel('item.categoryId', 'investment-parent');
        await setModel('item.sourceAmountCents', -5050);
        await setModel('item.destinationAmountCents', 6060);
        await setModel('item.sourceAccountId', 'bank');
        await setModel('item.destinationAccountId', 'wallet');
        await setModel('editingTags', []);
        await setModel('item.counterparty', 'Updated Merchant');
        await setModel('item.paymentMethod', 'Updated Card');
        await setModel('item.comment', 'Updated Memo');
        expect(bindings.editingTransaction.value).toMatchObject({
            type: 5,
            categoryId: 'investment-parent',
            sourceAmountCents: -5050,
            destinationAmountCents: 6060,
            sourceAccountId: 'bank',
            destinationAccountId: 'wallet',
            counterparty: 'Updated Merchant',
            paymentMethod: 'Updated Card',
            comment: 'Updated Memo'
        });
        expect(bindings.editingTags.value).toEqual([]);

        await setModel('batchCategoryType', 2);
        await setModel('batchCategoryId', 'income-parent');
        await setModel('batchAccountId', 'bank');
        await setModel('currentDescriptionFilterValue', 'salary');
        await setModel('manageCategoryType', 5);
        await setModel('manageCategoryId', 'investment-parent');
        await setModel('manageAccountId', 'wallet');
        expect(bindings.batchCategoryType.value).toBe(2);
        expect(bindings.batchCategoryId.value).toBe('income-parent');
        expect(bindings.batchAccountId.value).toBe('bank');
        expect(bindings.currentDescriptionFilterValue.value).toBe('salary');
        expect(bindings.manageCategoryType.value).toBe(5);
        expect(bindings.manageCategoryId.value).toBe('investment-parent');
        expect(bindings.manageAccountId.value).toBe('wallet');

        for (const dialogBinding of [
            'showBatchCategoryDialog',
            'showBatchAccountDialog',
            'showCustomDescriptionDialog',
            'showCategorySelectDialog',
            'showAccountSelectDialog',
            'showRecurringCandidateDialog',
            'showAnnotationDialog'
        ]) {
            await setModel(dialogBinding, true);
            expect(bindings[dialogBinding].value).toBe(true);
        }
    });

    test('named buttons drive exact selection and management state without handler sweeping', async () => {
        const first = createTransaction(31, { selected: false });
        const second = createTransaction(32, { selected: true, categoryId: '', valid: false });
        const { bindings } = await renderDesktop({ transactions: [first, second] });

        const click = async (label: string): Promise<void> => {
            const action = uiActions.find(candidate => candidate.event === 'onClick' && candidate.label.includes(label));
            expect(action).toBeDefined();
            await action!.handler();
        };

        await click('Select All in This Page');
        expect([first.selected, second.selected]).toEqual([true, true]);
        await click('Select None in This Page');
        expect([first.selected, second.selected]).toEqual([false, false]);
        await click('Invert Selection in This Page');
        expect([first.selected, second.selected]).toEqual([true, true]);

        bindings.openCategoryManagement();
        expect(bindings.showCategorySelectDialog.value).toBe(true);
        expect(bindings.manageCategoryId.value).toBe('');
        bindings.openAccountManagement();
        expect(bindings.showAccountSelectDialog.value).toBe(true);
        expect(bindings.manageAccountId.value).toBe('');
    });

    test('named selection menu commands mutate only their documented row sets', async () => {
        const valid = createTransaction(33, { selected: false, valid: true });
        const invalid = createTransaction(34, { selected: false, valid: false, categoryId: '' });
        const annotation = createTransaction(35, { selected: false, valid: true, historyRewrite: true });
        await renderDesktop({ transactions: [valid, invalid, annotation] });

        await getNamedUiAction('Select All Valid Items').handler();
        expect([valid.selected, invalid.selected, annotation.selected]).toEqual([true, false, true]);

        valid.selected = false;
        annotation.selected = false;
        await getNamedUiAction('Select All Invalid Items').handler();
        expect([valid.selected, invalid.selected, annotation.selected]).toEqual([false, true, false]);

        invalid.selected = false;
        await getNamedUiAction('Select All Needs AI Annotation').handler();
        expect([valid.selected, invalid.selected, annotation.selected]).toEqual([false, true, false]);

        await getNamedUiAction('Select All', 0).handler();
        expect([valid.selected, invalid.selected, annotation.selected]).toEqual([true, true, true]);
        await getNamedUiAction('Select None', 0).handler();
        expect([valid.selected, invalid.selected, annotation.selected]).toEqual([false, false, false]);
        await getNamedUiAction('Invert Selection', 0).handler();
        expect([valid.selected, invalid.selected, annotation.selected]).toEqual([true, true, true]);
    });

    test('named management actions preserve create, edit, refresh, and rejected-dialog contracts', async () => {
        const row = createTransaction(36);
        const { bindings } = await renderDesktop({
            transactions: [row],
            configure: current => {
                current.showCategorySelectDialog.value = true;
                current.showAccountSelectDialog.value = true;
            }
        });
        const categoryOpen = jest.fn<(...args: Array<unknown>) => Promise<any>>();
        const accountOpen = jest.fn<(...args: Array<unknown>) => Promise<any>>();
        bindings.categoryEditDialog.value = { open: categoryOpen };
        bindings.accountEditDialog.value = { open: accountOpen };

        categoryOpen.mockResolvedValueOnce({ category: { id: 'created-primary' } });
        await getNamedUiAction('Add Primary Category').handler();
        await Promise.resolve();
        expect(bindings.manageCategoryId.value).toBe('created-primary');
        expect(mockLoadAllCategories).toHaveBeenCalledWith({ force: true });

        bindings.manageCategoryId.value = 'expense-parent';
        categoryOpen.mockResolvedValueOnce({ category: { id: 'created-secondary' } });
        await getNamedUiAction('Add Secondary Category').handler();
        await Promise.resolve();
        expect(bindings.manageCategoryId.value).toBe('created-secondary');

        accountOpen.mockResolvedValueOnce({ account: { id: 'bank' } });
        await getNamedUiAction('Add Account').handler();
        await Promise.resolve();
        expect(bindings.manageAccountId.value).toBe('bank');
        expect(mockLoadAllAccounts).toHaveBeenCalledWith({ force: true });

        bindings.manageCategoryId.value = 'expense-child';
        categoryOpen.mockResolvedValueOnce({});
        await getNamedUiAction('Edit', 0).handler();
        await Promise.resolve();
        expect(bindings.showCategorySelectDialog.value).toBe(false);
        expect(categoryOpen).toHaveBeenLastCalledWith(expect.objectContaining({
            id: 'expense-child',
            currentCategory: expenseChild
        }));

        bindings.manageAccountId.value = 'wallet';
        accountOpen.mockResolvedValueOnce({});
        await getNamedUiAction('Edit', 1).handler();
        await Promise.resolve();
        expect(bindings.showAccountSelectDialog.value).toBe(false);
        expect(accountOpen).toHaveBeenLastCalledWith(expect.objectContaining({
            id: 'wallet',
            currentAccount: wallet
        }));

        for (const action of [
            getNamedUiAction('Add Primary Category'),
            getNamedUiAction('Add Secondary Category'),
            getNamedUiAction('Add Account'),
            getNamedUiAction('Edit', 0),
            getNamedUiAction('Edit', 1)
        ]) {
            if (action.label.includes('Account') || action === getNamedUiAction('Edit', 1)) {
                accountOpen.mockRejectedValueOnce({ processed: true });
            } else {
                categoryOpen.mockRejectedValueOnce({ processed: true });
            }
            await action.handler();
            await Promise.resolve();
        }
    });

    test('named annotation and dialog actions perform exact state transitions', async () => {
        const annotated = createTransaction(37, {
            selected: true,
            valid: false,
            categoryId: '',
            historyRewrite: true
        });
        const regular = createTransaction(38, { selected: true });
        Object.assign(annotated, {
            recurringTemplateId: 'recurring-37',
            recurringTemplateName: 'Monthly Coffee',
            recurringCandidateCount: 1,
            recurringMatchScore: 0.9,
            recurringMatchReasons: 'date',
            recurringMatchedDate: '2026-07-10'
        });
        const { bindings } = await renderDesktop({
            transactions: [annotated, regular],
            sessionId: '',
            configure: current => {
                current.showAnnotationDialog.value = true;
                current.showBatchCategoryDialog.value = true;
                current.showBatchAccountDialog.value = true;
                current.showCustomDescriptionDialog.value = true;
                current.currentDescriptionFilterValue.value = 'coffee';
                current.showRecurringCandidateDialog.value = true;
                current.recurringCandidateTarget.value = annotated;
                current.recurringCandidates.value = [{
                    id: 'recurring-37',
                    name: 'Monthly Coffee',
                    matchScore: 0.9,
                    matchedOccurrenceDate: '2026-07-10',
                    matchReasons: ['date']
                }];
            }
        });

        await getNamedUiAction('Open Batch Category Editor').handler();
        expect(bindings.showAnnotationDialog.value).toBe(false);
        expect(bindings.showBatchCategoryDialog.value).toBe(true);

        bindings.showAnnotationDialog.value = true;
        await getNamedUiAction('Open Batch Account Editor').handler();
        expect(bindings.showAnnotationDialog.value).toBe(false);
        expect(bindings.showBatchAccountDialog.value).toBe(true);

        bindings.showAnnotationDialog.value = true;
        await getNamedUiAction('Edit First Transaction').handler();
        expect(bindings.showAnnotationDialog.value).toBe(false);
        expect(bindings.editingTransaction.value._previewId).toBe((annotated as any)._previewId);

        for (const row of bindings.tableTransactions.value) {
            row.selected = true;
        }
        bindings.batchCategoryType.value = 3;
        bindings.batchCategoryId.value = 'expense-child';
        await getBoundUiAction('applyBatchCategory').handler();
        expect(bindings.tableTransactions.value.map((row: ImportTransaction) => row.categoryId)).toEqual([
            'expense-child',
            'expense-child'
        ]);
        expect(bindings.showBatchCategoryDialog.value).toBe(false);

        bindings.batchAccountId.value = 'bank';
        await getBoundUiAction('applyBatchAccount').handler();
        expect(bindings.tableTransactions.value.map((row: ImportTransaction) => row.sourceAccountId)).toEqual([
            'bank',
            'bank'
        ]);
        expect(bindings.showBatchAccountDialog.value).toBe(false);

        await getNamedUiAction('OK').handler();
        expect(bindings.filters.value.description).toBe('coffee');
        expect(bindings.showCustomDescriptionDialog.value).toBe(false);

        await getBoundUiAction('selectedRecurringCandidateId').handler();
        expect(bindings.selectedRecurringCandidateId.value).toBe('recurring-37');
        await getNamedUiAction('Clear Scheduled Match').handler();
        expect(bindings.showRecurringCandidateDialog.value).toBe(true);
    });

    test('recurring and annotation dialogs render loading, candidate, empty, and reason branches', async () => {
        const target = createTransaction(40, { historyRewrite: true });
        Object.assign(target, {
            recurringTemplateId: 'recurring-1',
            recurringTemplateName: 'Monthly Coffee',
            recurringCandidateCount: 2,
            recurringMatchScore: 0.94,
            recurringMatchReasons: 'amount|date',
            recurringMatchedDate: '2026-07-10'
        });
        const candidate = {
            id: 'recurring-1',
            name: 'Monthly Coffee',
            matchScore: 0.94,
            matchedOccurrenceDate: '2026-07-10',
            matchReasons: ['amount', 'date']
        };

        const candidateRender = await renderDesktop({
            transactions: [target],
            configure: bindings => {
                bindings.showRecurringCandidateDialog.value = true;
                bindings.recurringCandidateTarget.value = target;
                bindings.recurringCandidateLoading.value = false;
                bindings.recurringCandidates.value = [candidate, { ...candidate, id: 'recurring-2', name: '' }];
                bindings.selectedRecurringCandidateId.value = 'recurring-1';
                bindings.showAnnotationDialog.value = true;
            }
        });
        expect(candidateRender.html).toContain('Monthly Coffee');
        expect(candidateRender.html).toContain('Best Candidate');
        expect(candidateRender.html).toContain('Best Candidate Reason');
        expect(candidateRender.html).toContain('Match Score');
        expect(candidateRender.html).toContain('Matched Date');
        expect(candidateRender.html).toContain('Match Reasons');
        expect(candidateRender.html).toContain('Open Batch Category Editor');

        candidateRender.bindings.closeRecurringCandidateDialog();
        expect(candidateRender.bindings.showRecurringCandidateDialog.value).toBe(false);
        expect(candidateRender.bindings.recurringCandidateTarget.value).toBeNull();
        expect(candidateRender.bindings.recurringCandidates.value).toEqual([]);
        expect(candidateRender.bindings.selectedRecurringCandidateId.value).toBe('');

        const loadingRender = await renderDesktop({
            transactions: [target],
            configure: bindings => {
                bindings.showRecurringCandidateDialog.value = true;
                bindings.recurringCandidateLoading.value = true;
            }
        });
        expect(loadingRender.html).toContain('v-progress-circular');

        const emptyRender = await renderDesktop({
            transactions: [target],
            configure: bindings => {
                bindings.showRecurringCandidateDialog.value = true;
                bindings.recurringCandidateLoading.value = false;
                bindings.recurringCandidates.value = [];
            }
        });
        expect(emptyRender.html).toContain('No Scheduled Candidates');
    });

    test('display, filter, selection, and paging closures preserve local and server contracts', () => {
        const localRows = [
            createTransaction(51, { selected: true, categoryId: 'expense-child', tagIds: ['known-tag'] }),
            createTransaction(52, {
                selected: false,
                categoryId: '',
                categoryName: 'Imported Mystery',
                sourceAccountId: '',
                sourceAccountName: 'Imported Wallet',
                destinationAccountId: 'bank',
                destinationAccountName: 'Bank',
                tagIds: ['missing-tag'],
                tagNames: ['Imported Tag']
            })
        ];
        const local = createBindings({ transactions: localRows, total: 30 });

        expect(local.getTablePageOptions()).toEqual([{ value: 10, name: '10' }]);
        expect(local.getTablePageOptions(4)).toEqual([{ value: -1, name: 'All' }]);
        expect(local.getTablePageOptions(30)).toEqual([
            { value: 5, name: '5' },
            { value: 10, name: '10' },
            { value: 15, name: '15' },
            { value: 20, name: '20' },
            { value: 25, name: '25' },
            { value: 30, name: '30' },
            { value: -1, name: 'All' }
        ]);
        expect(local.getDisplayDateTime(localRows[0])).toContain('DATE:');
        expect(local.getDisplayTimezone(createTransaction(53, { utcOffset: -300 }))).toContain('UTC');
        expect(local.getTransactionDisplayAmount(localRows[0])).toContain('CNY');
        expect(local.getTransactionDisplayDestinationAmount(localRows[0])).toBe('-');
        expect(local.getTransactionDisplayDestinationAmount(createTransaction(54, {
            type: 4,
            destinationAccountId: 'bank'
        }))).toContain('USD');
        expect(local.getSourceAccountTitle(createTransaction(55, { type: 3 }))).toBe('Account');
        expect(local.getSourceAccountTitle(createTransaction(56, { type: 4 }))).toBe('Source Account');
        expect(local.getSourceAccountDisplayName(localRows[0])).toBe('Wallet');
        expect(local.getSourceAccountDisplayName(createTransaction(57, { sourceAccountId: '' }))).toBe('None');
        expect(local.getDestinationAccountDisplayName(createTransaction(58, { destinationAccountId: 'bank' }))).toBe('Bank');
        expect(local.getDestinationAccountDisplayName(createTransaction(59, { destinationAccountId: '' }))).toBe('None');
        expect(local.allUsedCategoryNames.value).toEqual(['Cafe', 'Imported Mystery']);
        expect(local.allUsedAccountNames.value).toEqual(['Wallet', 'Imported Wallet', 'Bank']);
        expect(local.allUsedTagNames.value).toEqual(['Known Tag', 'Imported Tag']);
        expect(local.anyButNotAllTransactionSelected.value).toBe(true);
        expect(local.allTransactionSelected.value).toBe(false);
        expect(local.filteredImportTransactions.value).toEqual(localRows);
        expect(local.totalPageCount.value).toBe(1);

        local.filters.value.description = 'Memo 51';
        expect(local.isTransactionDisplayed(localRows[0])).toBe(true);
        expect(local.isTransactionDisplayed(localRows[1])).toBe(false);
        local.filters.value.description = null;
        local.filters.value.minDatetime = 100;
        expect(local.displayFilterCustomDateRange.value).toBe('');
        local.filters.value.maxDatetime = 200;
        expect(local.displayFilterCustomDateRange.value).toContain('DATE:100');

        const emit = jest.fn();
        const server = createBindings({
            transactions: localRows,
            serverPaged: true,
            total: 42,
            metadata: {
                counts: { total: 42, selected: 20, selected_invalid: 3, annotations: { 'needs-review': 5 } },
                facets: {
                    categories: [{ value: 'expense-child', label: 'Cafe', count: 10 }],
                    accounts: [{ value: 'wallet', label: 'Wallet', count: 9 }],
                    tags: [{ value: 'known-tag', label: 'Known Tag', count: 8 }]
                }
            }
        }, emit);
        expect(server.getTablePageOptions(42)).toEqual([
            { value: 10, name: '10' },
            { value: 20, name: '20' }
        ]);
        expect(server.allUsedCategoryNames.value).toEqual(['Cafe']);
        expect(server.allUsedAccountNames.value).toEqual(['Wallet']);
        expect(server.allUsedTagNames.value).toEqual(['Known Tag']);
        expect(server.selectedImportTransactionCount.value).toBe(20);
        expect(server.selectedInvalidTransactionCount.value).toBe(3);
        expect(server.annotationTransactionCount.value).toBe(5);
        expect(server.totalPageCount.value).toBe(5);

        emit.mockClear();
        server.updatePreviewTablePage(2);
        expect(emit).toHaveBeenCalledWith('requestPage', 2, 10, expect.objectContaining({
            sortBy: null,
            sortDirection: null,
            filters: {}
        }));
        emit.mockClear();
        server.updatePreviewTablePageSize(20);
        expect(emit).toHaveBeenCalledWith('requestPage', 1, 20, expect.any(Object));
        emit.mockClear();
        server.updatePreviewTableSort([{ key: 'sourceAmountCents', order: 'desc' }]);
        expect(emit).toHaveBeenCalledWith('requestPage', 1, 20, expect.objectContaining({
            sortBy: 'sourceAmountCents',
            sortDirection: 'desc'
        }));
    });
});
