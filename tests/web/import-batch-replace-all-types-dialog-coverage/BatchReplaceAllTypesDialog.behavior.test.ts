import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockCategoryType = { Income: 2, Expense: 3, Transfer: 4 } as const;
const mockSnackbar = { showError: jest.fn<(...args: any[]) => void>() };
const mockLoggerError = jest.fn<(...args: any[]) => void>();
const mockOpenTextFileContent = jest.fn<(...args: any[]) => Promise<string>>();
const mockStartDownloadFile = jest.fn<(...args: any[]) => void>();
const mockParseFromJson = jest.fn<(content: string) => MockReplaceRules | null>();
const mockFormatFileName = jest.fn<(name: string) => string>();
const mockCreateBlob = jest.fn<(content: string) => unknown>();

let mockSlotItemHidden = false;

class MockReplaceRule {
    dataType: string;
    sourceValue: string;
    targetId: string;

    constructor(dataType: string, sourceValue: string, targetId: string) {
        this.dataType = dataType;
        this.sourceValue = sourceValue;
        this.targetId = targetId;
    }

    static of(dataType: string, sourceValue: string, targetId: string): MockReplaceRule {
        return new MockReplaceRule(dataType, sourceValue, targetId);
    }
}

class MockReplaceRules {
    private readonly rules: MockReplaceRule[];

    constructor(rules: MockReplaceRule[]) {
        this.rules = rules;
    }

    static parseFromJson(content: string): MockReplaceRules | null {
        return mockParseFromJson(content);
    }

    static of(rules: MockReplaceRule[]): MockReplaceRules {
        return new MockReplaceRules(rules);
    }

    getRules(): MockReplaceRule[] {
        return this.rules;
    }

    toJson(): string {
        return JSON.stringify({ rules: this.rules });
    }
}

const mockExpensePrimary = {
    id: 'expense-primary',
    name: 'Food',
    subCategories: [{ id: 'expense-dining', name: 'Dining' }]
};
const mockIncomePrimary = {
    id: 'income-primary',
    name: 'Income',
    subCategories: [{ id: 'income-salary', name: 'Salary' }]
};
const mockTransferPrimary = {
    id: 'transfer-primary',
    name: 'Transfer',
    subCategories: [{ id: 'transfer-bank', name: 'Bank Transfer' }]
};

const mockVisibleAccount = { id: 'wallet', name: 'Wallet', category: 1, hidden: false };
const mockSecondAccount = { id: 'bank', name: 'Bank', category: 2, hidden: false };
const mockVisibleTag = { id: 'tag-food', name: 'Food Tag', hidden: false };
const mockHiddenTag = { id: 'tag-hidden', name: 'Hidden Tag', hidden: true };

const mockSettingsStore = (jest.requireActual('vue') as any).reactive({
    appSettings: { showAccountBalance: true }
});
const mockAccountsStore = (jest.requireActual('vue') as any).reactive({
    allPlainAccounts: [mockVisibleAccount, mockSecondAccount],
    allVisiblePlainAccounts: [mockVisibleAccount, mockSecondAccount],
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>()
});
const mockCategoryStore = (jest.requireActual('vue') as any).reactive({
    allTransactionCategories: {
        [mockCategoryType.Expense]: [mockExpensePrimary],
        [mockCategoryType.Income]: [mockIncomePrimary],
        [mockCategoryType.Transfer]: [mockTransferPrimary]
    } as Record<number, any[]>,
    hasAvailableExpenseCategories: true,
    hasAvailableIncomeCategories: true,
    hasAvailableTransferCategories: true,
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>()
});
const mockTagStore = (jest.requireActual('vue') as any).reactive({
    allTransactionTags: [mockVisibleTag, mockHiddenTag],
    loadAllTags: jest.fn<(...args: any[]) => Promise<void>>()
});

const mockOpenOptions = {
    expenseCategoryNames: [{ name: 'Old Dining', value: 'old-expense' }],
    incomeCategoryNames: [{ name: 'Old Salary', value: 'old-income' }],
    transferCategoryNames: [{ name: 'Old Transfer', value: 'old-transfer' }],
    accountNames: [{ name: 'Old Wallet', value: 'old-account' }],
    tagNames: [{ name: 'Old Tag', value: 'old-tag' }]
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(mockSnackbar)
    };
});
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: { name: 'SnackBar' }
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCategorizedAccountsWithDisplayBalance: (accounts: any[], showBalance: boolean) => ([{
            category: 1,
            accounts,
            showBalance
        }])
    })
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/core/category.ts', () => ({ CategoryType: mockCategoryType }));
jest.mock('@/core/import_transaction.ts', () => ({
    ImportTransactionReplaceRule: MockReplaceRule,
    ImportTransactionReplaceRules: MockReplaceRules
}));
jest.mock('@/core/file.ts', () => ({
    KnownFileType: {
        JSON: {
            contentType: '.json,application/json',
            formatFileName: (name: string) => mockFormatFileName(name),
            createBlob: (content: string) => mockCreateBlob(content)
        }
    }
}));
jest.mock('@/models/account.ts', () => ({
    Account: {
        findAccountNameById: (accounts: any[], id: string) => accounts.find(account => account.id === id)?.name
    }
}));
jest.mock('@/lib/category.ts', () => ({
    getTransactionPrimaryCategoryName: (id: string, categories: any[] | undefined) => (
        categories?.find(category => category.id === id)?.name ?? ''
    ),
    getTransactionSecondaryCategoryName: (id: string, categories: any[] | undefined) => {
        if (id === 'missing-target') return '';
        for (const category of categories ?? []) {
            const child = category.subCategories?.find((item: any) => item.id === id);
            if (child) return child.name;
        }
        return '';
    }
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { error: (...args: any[]) => mockLoggerError(...args) }
}));
jest.mock('@/lib/ui/common.ts', () => ({
    openTextFileContent: (...args: any[]) => mockOpenTextFileContent(...args),
    startDownloadFile: (...args: any[]) => mockStartDownloadFile(...args)
}));
jest.mock('@mdi/js', () => ({
    mdiRefresh: 'refresh',
    mdiDotsVertical: 'dots',
    mdiFolderOpenOutline: 'folder-open',
    mdiContentSaveOutline: 'content-save',
    mdiPound: 'pound'
}));

import BatchReplaceAllTypesDialog from '@/views/desktop/transactions/import/dialogs/BatchReplaceAllTypesDialog.vue';

function setup(): { bindings: any; exposed: any } {
    let exposed: any = null;
    const bindings = (BatchReplaceAllTypesDialog as any).setup(
        {},
        {
            attrs: {}, slots: {}, emit: jest.fn(),
            expose(value: any) {
                exposed = value;
            }
        }
    );
    return { bindings, exposed };
}

async function flush(times = 10): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(): { app: any; root: any; state: any } {
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
            const slotProps = {
                props: {},
                item: { title: 'Template Tag', value: 'tag-food', raw: { hidden: mockSlotItemHidden } }
            };
            return () => h('stub', attrs, Object.values(slots).flatMap((slot: any) => slot?.(slotProps) ?? []));
        }
    });
    const app = renderer.createApp(BatchReplaceAllTypesDialog as any);
    app.config.warnHandler = () => undefined;
    for (const name of [
        'v-dialog', 'v-card', 'v-btn', 'v-progress-circular', 'v-icon', 'v-tooltip', 'v-menu', 'v-list',
        'v-list-item', 'v-card-text', 'v-row', 'v-col', 'v-table', 'v-select', 'v-autocomplete',
        'two-column-select', 'v-chip', 'v-list-item-title', 'snack-bar'
    ]) app.component(name, SlotHost);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState };
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
        if (name === 'onUpdate:modelValue') callback('tag');
        else callback();
        await flush(3);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockSlotItemHidden = false;
    mockSettingsStore.appSettings.showAccountBalance = true;
    mockAccountsStore.allPlainAccounts = [mockVisibleAccount, mockSecondAccount];
    mockAccountsStore.allVisiblePlainAccounts = [mockVisibleAccount, mockSecondAccount];
    mockCategoryStore.allTransactionCategories = {
        [mockCategoryType.Expense]: [mockExpensePrimary],
        [mockCategoryType.Income]: [mockIncomePrimary],
        [mockCategoryType.Transfer]: [mockTransferPrimary]
    };
    mockCategoryStore.hasAvailableExpenseCategories = true;
    mockCategoryStore.hasAvailableIncomeCategories = true;
    mockCategoryStore.hasAvailableTransferCategories = true;
    mockTagStore.allTransactionTags = [mockVisibleTag, mockHiddenTag];
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockTagStore.loadAllTags.mockResolvedValue(undefined);
    mockOpenTextFileContent.mockResolvedValue('{"unitTest":true}');
    mockParseFromJson.mockReturnValue(new MockReplaceRules([
        MockReplaceRule.of('tag', 'Old Tag', 'tag-food')
    ]));
    mockFormatFileName.mockReturnValue('unit-test-rules.json');
    mockCreateBlob.mockImplementation(content => ({ kind: 'unit-test-blob', content }));
});

describe('BatchReplaceAllTypesDialog state and selection contracts', () => {
    test('opens with reset rules and sources, then cancels through the exposed API', async () => {
        const { bindings, exposed } = setup();
        bindings.rules.value = [MockReplaceRule.of('account', 'stale', 'bank')];
        bindings.newRule.value = MockReplaceRule.of('tag', 'stale', 'tag-food');

        const result = bindings.open(mockOpenOptions);
        expect(exposed.open).toBe(bindings.open);
        expect(bindings.rules.value).toStrictEqual([]);
        expect(bindings.newRule.value).toStrictEqual(MockReplaceRule.of('expenseCategory', '', ''));
        expect(bindings.sourceExpenseCategoryNames.value).toStrictEqual(mockOpenOptions.expenseCategoryNames);
        expect(bindings.sourceIncomeCategoryNames.value).toStrictEqual(mockOpenOptions.incomeCategoryNames);
        expect(bindings.sourceTransferCategoryNames.value).toStrictEqual(mockOpenOptions.transferCategoryNames);
        expect(bindings.sourceAccountNames.value).toStrictEqual(mockOpenOptions.accountNames);
        expect(bindings.sourceTagNames.value).toStrictEqual(mockOpenOptions.tagNames);
        expect(bindings.showState.value).toBe(true);

        bindings.cancel();
        await expect(result).rejects.toBeUndefined();
        expect(bindings.showState.value).toBe(false);
    });

    test('exposes reactive accounts, categories, tags, availability, and balance grouping', () => {
        const { bindings } = setup();
        expect(bindings.showAccountBalance.value).toBe(true);
        expect(bindings.allAccounts.value).toStrictEqual([mockVisibleAccount, mockSecondAccount]);
        expect(bindings.allVisibleAccounts.value).toStrictEqual([mockVisibleAccount, mockSecondAccount]);
        expect(bindings.allVisibleCategorizedAccounts.value).toStrictEqual([{
            category: 1,
            accounts: [mockVisibleAccount, mockSecondAccount],
            showBalance: true
        }]);
        expect(bindings.allCategories.value[mockCategoryType.Expense]).toStrictEqual([mockExpensePrimary]);
        expect(bindings.allTags.value).toStrictEqual([mockVisibleTag, mockHiddenTag]);
        expect(bindings.hasAvailableExpenseCategories.value).toBe(true);
        expect(bindings.hasAvailableIncomeCategories.value).toBe(true);
        expect(bindings.hasAvailableTransferCategories.value).toBe(true);

        mockSettingsStore.appSettings.showAccountBalance = false;
        mockAccountsStore.allVisiblePlainAccounts = [mockVisibleAccount];
        mockCategoryStore.hasAvailableExpenseCategories = false;
        mockCategoryStore.hasAvailableIncomeCategories = false;
        mockCategoryStore.hasAvailableTransferCategories = false;
        expect(bindings.showAccountBalance.value).toBe(false);
        expect(bindings.allVisibleCategorizedAccounts.value[0].showBalance).toBe(false);
        expect(bindings.hasAvailableExpenseCategories.value).toBe(false);
        expect(bindings.hasAvailableIncomeCategories.value).toBe(false);
        expect(bindings.hasAvailableTransferCategories.value).toBe(false);
    });

    test('selects the exact source list and empty-state message for every data type', () => {
        const { bindings } = setup();
        bindings.open(mockOpenOptions);
        const cases = [
            ['expenseCategory', mockOpenOptions.expenseCategoryNames, 'tt:No available category'],
            ['incomeCategory', mockOpenOptions.incomeCategoryNames, 'tt:No available category'],
            ['transferCategory', mockOpenOptions.transferCategoryNames, 'tt:No available category'],
            ['account', mockOpenOptions.accountNames, 'tt:No available account'],
            ['tag', mockOpenOptions.tagNames, 'tt:No available tag'],
            ['unknown', [], '']
        ] as const;
        for (const [dataType, expectedItems, expectedText] of cases) {
            bindings.newRule.value = MockReplaceRule.of(dataType, '', '');
            expect(bindings.sourceItems.value).toStrictEqual(expectedItems);
            expect(bindings.noSourceItemText.value).toBe(expectedText);
        }
    });

    test('formats rule types and targets across all categories, accounts, tags, and fallbacks', () => {
        const { bindings } = setup();
        const rules = [
            MockReplaceRule.of('expenseCategory', 'old', 'expense-dining'),
            MockReplaceRule.of('incomeCategory', 'old', 'income-salary'),
            MockReplaceRule.of('transferCategory', 'old', 'transfer-bank'),
            MockReplaceRule.of('account', 'old', 'wallet'),
            MockReplaceRule.of('tag', 'old', 'tag-food'),
            MockReplaceRule.of('unknown', 'old', 'missing-target')
        ];
        expect(rules.map(rule => bindings.getRuleTypeDisplayName(rule))).toStrictEqual([
            'tt:Expense Category', 'tt:Income Category', 'tt:Transfer Category', 'tt:Account',
            'tt:Transaction Tag', ''
        ]);
        expect(rules.map(rule => bindings.getRuleTargetValueDisplayName(rule))).toStrictEqual([
            'Dining', 'Salary', 'Bank Transfer', 'Wallet', 'Food Tag', ''
        ]);

        expect(bindings.getRuleTargetValueDisplayName(MockReplaceRule.of('tag', 'old', 'missing-target'))).toBe('');
        expect(bindings.getRuleTargetValueDisplayName(MockReplaceRule.of('account', 'old', 'missing-target'))).toBe('');
        expect(bindings.getRuleTargetValueDisplayName(MockReplaceRule.of('expenseCategory', 'old', 'missing-target'))).toBe('');
        expect(bindings.getAccountDisplayName()).toBe('tt:None');
        expect(bindings.getAccountDisplayName('bank')).toBe('Bank');
    });

    test('validates new rules, adds exact identity mappings, removes by index, and resets the editor', () => {
        const { bindings } = setup();
        bindings.addNewRule();
        bindings.newRule.value = MockReplaceRule.of('', 'old', 'target');
        bindings.addNewRule();
        bindings.newRule.value = MockReplaceRule.of('tag', 'old', '');
        bindings.addNewRule();
        expect(bindings.rules.value).toStrictEqual([]);

        const rule = MockReplaceRule.of('account', 'Old Wallet', 'wallet');
        bindings.newRule.value = rule;
        bindings.addNewRule();
        expect(bindings.rules.value).toStrictEqual([rule]);
        expect(bindings.newRule.value).toStrictEqual(MockReplaceRule.of('expenseCategory', '', ''));
        expect(JSON.stringify(bindings.rules.value)).not.toMatch(/amount|cents|date/i);

        bindings.rules.value.push(MockReplaceRule.of('tag', 'Old Tag', 'tag-food'));
        bindings.removeRule(0);
        expect(bindings.rules.value).toStrictEqual([MockReplaceRule.of('tag', 'Old Tag', 'tag-food')]);
        bindings.removeRule(99);
        expect(bindings.rules.value).toHaveLength(1);
    });

    test('confirms empty or populated rule sets and safely handles absent resolvers', async () => {
        const { bindings } = setup();
        bindings.confirm();
        bindings.cancel();
        expect(bindings.showState.value).toBe(false);

        const emptyResult = bindings.open(mockOpenOptions);
        bindings.confirm();
        await expect(emptyResult).resolves.toStrictEqual({ rules: [] });
        expect(bindings.showState.value).toBe(false);

        const populatedResult = bindings.open(mockOpenOptions);
        const rule = MockReplaceRule.of('expenseCategory', 'Old Dining', 'expense-dining');
        bindings.rules.value = [rule];
        bindings.confirm();
        await expect(populatedResult).resolves.toStrictEqual({ rules: [rule] });
        expect(JSON.stringify((await populatedResult).rules)).not.toMatch(/amount|cents|date/i);
    });
});

describe('BatchReplaceAllTypesDialog reload and file contracts', () => {
    test('reloads all identity stores with force and clears loading after success', async () => {
        const { bindings } = setup();
        bindings.reload();
        expect(bindings.loading.value).toBe(true);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: true });
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledWith({ force: true });
        expect(mockTagStore.loadAllTags).toHaveBeenCalledWith({ force: true });
        await flush();
        expect(bindings.loading.value).toBe(false);
    });

    test('handles processed, readable, raw, and missing-snackbar reload failures', async () => {
        const { bindings } = setup();
        const handled = { processed: true, message: 'handled reload' };
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce(handled);
        bindings.reload();
        await flush();
        expect(mockSnackbar.showError).not.toHaveBeenCalledWith(handled);

        const readable = { processed: false, message: 'reload failed' };
        mockCategoryStore.loadAllCategories.mockRejectedValueOnce(readable);
        bindings.reload();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith(readable);

        mockTagStore.loadAllTags.mockRejectedValueOnce('raw reload failure');
        bindings.reload();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith('raw reload failure');

        bindings.snackbar.value = null;
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'no snackbar' });
        bindings.reload();
        await flush();
        expect(bindings.loading.value).toBe(false);
    });

    test('loads valid replacement rules and reports parse or file errors', async () => {
        const { bindings } = setup();
        const parsedRules = [MockReplaceRule.of('tag', 'Old Tag', 'tag-food')];
        mockParseFromJson.mockReturnValueOnce(new MockReplaceRules(parsedRules));
        bindings.loadReplaceRuleFile();
        await flush();
        expect(mockOpenTextFileContent).toHaveBeenCalledWith({ allowedExtensions: '.json,application/json' });
        expect(mockParseFromJson).toHaveBeenCalledWith('{"unitTest":true}');
        expect(bindings.rules.value).toStrictEqual(parsedRules);

        mockParseFromJson.mockReturnValueOnce(null);
        bindings.loadReplaceRuleFile();
        await flush();
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to parse replace rule file');
        expect(mockSnackbar.showError).toHaveBeenCalledWith('Replace rule file is invalid');

        const fileError = new Error('synthetic file read failure');
        mockOpenTextFileContent.mockRejectedValueOnce(fileError);
        bindings.loadReplaceRuleFile();
        await flush();
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to open replace rule file', fileError);
        expect(mockSnackbar.showError).toHaveBeenCalledWith('Replace rule file is invalid');

        bindings.snackbar.value = null;
        mockParseFromJson.mockReturnValueOnce(null);
        bindings.loadReplaceRuleFile();
        await flush();
        mockOpenTextFileContent.mockRejectedValueOnce('raw file error');
        bindings.loadReplaceRuleFile();
        await flush();
    });

    test('serializes the current identity rules to a JSON download without transaction amounts', () => {
        const { bindings } = setup();
        bindings.rules.value = [MockReplaceRule.of('account', 'Old Wallet', 'wallet')];
        bindings.saveReplaceRuleFile();
        const expectedJson = JSON.stringify({ rules: bindings.rules.value });
        expect(mockFormatFileName).toHaveBeenCalledWith('tt:dataExport.defaultImportReplaceRuleFileName');
        expect(mockCreateBlob).toHaveBeenCalledWith(expectedJson);
        expect(mockStartDownloadFile).toHaveBeenCalledWith(
            'unit-test-rules.json',
            { kind: 'unit-test-blob', content: expectedJson }
        );
        expect(expectedJson).not.toMatch(/amount|cents|date/i);
    });
});

describe('BatchReplaceAllTypesDialog production template', () => {
    test('renders identity variants, state combinations, slots, and visible event wrappers', async () => {
        const mounted = mountWithHostRenderer();
        const opened = mounted.state.open(mockOpenOptions);
        opened.catch(() => undefined);
        try {
            const { nextTick } = jest.requireActual('vue') as any;
            mounted.state.rules = [
                MockReplaceRule.of('expenseCategory', '', 'expense-dining'),
                MockReplaceRule.of('incomeCategory', 'Old Salary', 'income-salary'),
                MockReplaceRule.of('transferCategory', 'Old Transfer', 'transfer-bank'),
                MockReplaceRule.of('account', 'Old Wallet', 'wallet'),
                MockReplaceRule.of('tag', 'Old Tag', 'tag-food')
            ];
            mounted.state.newRule = MockReplaceRule.of('expenseCategory', 'Old Dining', 'expense-dining');
            await nextTick();

            const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(mounted.root, callbacks);
            await invokeHostCallbacks(callbacks);
            expect(callbacks.length).toBeGreaterThan(8);

            for (const [dataType, targetId] of [
                ['incomeCategory', 'income-salary'],
                ['transferCategory', 'transfer-bank'],
                ['account', 'wallet'],
                ['tag', 'tag-food']
            ] as const) {
                mounted.state.newRule = MockReplaceRule.of(dataType, '', targetId);
                mounted.state.loading = dataType === 'transferCategory';
                await nextTick();
                const phaseCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
                collectHostCallbacks(mounted.root, phaseCallbacks);
                for (const { name, callback } of phaseCallbacks) {
                    if (name === 'onUpdate:modelValue') callback(targetId);
                }
                expect(mounted.root.children.length).toBeGreaterThan(0);
            }

            mockSlotItemHidden = true;
            mounted.state.rules = [];
            mounted.state.newRule = MockReplaceRule.of('', '', '');
            mounted.state.loading = false;
            mockAccountsStore.allVisiblePlainAccounts = [];
            mockCategoryStore.hasAvailableExpenseCategories = false;
            mockCategoryStore.hasAvailableIncomeCategories = false;
            mockCategoryStore.hasAvailableTransferCategories = false;
            await nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
