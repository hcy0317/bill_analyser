import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;

const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();

const mockServices = {
    createAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
    getAccountRules: jest.fn<(...args: any[]) => Promise<any>>(),
    testAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
    updateAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
};

const parentAccount = {
    id: '10',
    parentId: '',
    name: 'Cash',
    category: 1,
    displayOrder: 1,
    icon: 'cash',
    color: '112233',
};
const childAccount = {
    id: '11',
    parentId: '10',
    name: 'Wallet',
    category: 1,
    displayOrder: 2,
    icon: 'wallet',
    color: '445566',
};
const bankAccount = {
    id: '20',
    parentId: '',
    name: 'Bank',
    category: 3,
    displayOrder: 3,
    icon: 'bank',
    color: '778899',
};
const allAccounts = [parentAccount, childAccount, bankAccount];

const mockAccountsStore = actualVue.reactive({
    allMixedPlainAccounts: [...allAccounts] as any[],
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args),
}));
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => mockAccountsStore,
}));

const RuleListPage = require('@/views/mobile/accounts/RuleListPage.vue').default as any;

function rawRule(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        id: 7,
        account_id: 11,
        account_name: 'Wallet',
        account_type: 1,
        name: 'Wallet merchant',
        priority: 10,
        rule_expression: 'coffee !refund',
        regex_enabled: false,
        enabled: true,
        applied_count: 4,
        match_count: 0,
        ...overrides,
    };
}

const initialRules = [
    rawRule(),
    rawRule({
        id: 8,
        account_id: 20,
        account_name: 'Bank',
        account_type: 3,
        name: '',
        priority: 20,
        rule_expression: '^salary$',
        regex_enabled: true,
        applied_count: 1,
        match_count: 5,
    }),
];

function successResponse(result: unknown = undefined): any {
    return { data: { success: true, result } };
}

function failedResponse(): any {
    return { data: { success: false, result: null } };
}

function setup(accountId?: unknown): { bindings: any; router: any } {
    const router = { back: jest.fn(), navigate: jest.fn() };
    const query = arguments.length > 0 ? { accountId } : {};
    const bindings = RuleListPage.setup(
        { f7route: { query }, f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() },
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(accountId?: unknown): {
    app: any;
    root: any;
    router: any;
    state: any;
} {
    const { createRenderer, defineComponent, h } = actualVue;
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
        createElement(type: string) {
            return createHostNode(type);
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
        },
    });
    const SlotHost = defineComponent({
        name: 'MobileAccountRuleSlotHost',
        inheritAttrs: false,
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h(
                'stub',
                attrs,
                Object.values(slots).flatMap((slot: any) => slot?.() ?? []),
            );
        },
    });
    const router = { back: jest.fn(), navigate: jest.fn() };
    const query = arguments.length > 0 ? { accountId } : {};
    const app = renderer.createApp(RuleListPage, { f7route: { query }, f7router: router });
    app.config.warnHandler = () => undefined;
    for (const name of [
        'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
        'f7-list', 'f7-list-item', 'f7-button', 'f7-icon', 'f7-sheet', 'f7-page-content',
        'f7-block-title', 'f7-list-input', 'f7-toggle', 'f7-block', 'f7-actions',
        'f7-actions-group', 'f7-actions-label', 'f7-actions-button',
        'list-item-selection-popup',
    ]) app.component(name, SlotHost);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, router, state: vm.$.setupState };
}

function collectHostNodes(node: any, nodes: any[], seen = new Set<any>()): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    nodes.push(node);
    for (const child of node.children ?? []) collectHostNodes(child, nodes, seen);
}

function collectHostCallbacks(
    node: any,
    callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }>,
    seen = new Set<any>(),
): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    for (const [name, value] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        for (const candidate of Array.isArray(value) ? value : [value]) {
            if (typeof candidate === 'function') {
                callbacks.push({ name, callback: candidate as (...args: any[]) => unknown });
            }
        }
    }
    for (const child of node.children ?? []) collectHostCallbacks(child, callbacks, seen);
}

async function invokeHostCallbacks(
    callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }>,
): Promise<void> {
    for (const { name, callback } of callbacks) {
        try {
            let result: unknown;
            if (name === 'onPtr:refresh') {
                result = callback(jest.fn());
            } else if (name === 'onInput') {
                result = callback({ target: { value: '33' } });
            } else if (name === 'onToggle:change') {
                result = callback(true);
            } else if (name === 'onUpdate:modelValue') {
                result = callback('11');
            } else if (name === 'onUpdate:show') {
                result = callback(false);
            } else {
                result = callback();
            }
            await Promise.resolve(result);
        } catch {
            // Generated Framework7 wrappers accept heterogeneous payloads and current sheet state.
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockAccountsStore.allMixedPlainAccounts = [...allAccounts];
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockServices.getAccountRules.mockResolvedValue(successResponse(initialRules));
    mockServices.createAccountRule.mockResolvedValue(successResponse({ id: 31 }));
    mockServices.updateAccountRule.mockResolvedValue(successResponse({ id: 7 }));
    mockServices.deleteAccountRule.mockResolvedValue(successResponse());
    mockServices.testAccountRule.mockResolvedValue(successResponse({ matched: true }));
});

describe('mobile account RuleListPage initialization and projections', () => {
    test('loads a route-scoped rule list and groups accounts without widening user scope', async () => {
        const { bindings } = setup('11');
        expect(bindings.loading.value).toBe(true);
        expect(bindings.isAccountLocked.value).toBe(true);
        expect(bindings.ruleForm.value.accountId).toBe('11');

        await flush();

        expect(bindings.loading.value).toBe(false);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockServices.getAccountRules).toHaveBeenCalledWith(11);
        expect(bindings.allAccounts.value).toStrictEqual(allAccounts);
        expect(bindings.accountOptions.value).toStrictEqual([
            { id: '10', name: 'Cash' },
            { id: '11', name: 'Wallet' },
            { id: '20', name: 'Bank' },
        ]);
        expect(bindings.selectedAccountName.value).toBe('Wallet');
        expect(bindings.autoRuleName.value).toBe('Wallet · tt:Account Rule');
        expect(bindings.accountRules.value).toHaveLength(2);
        expect(bindings.accountRuleGroups.value).toHaveLength(2);
        expect(bindings.accountRuleGroups.value.flatMap((group: any) => group.accounts)).toEqual(
            expect.arrayContaining([
                expect.objectContaining({ displayName: 'Cash / Wallet', ruleCount: 1 }),
                expect.objectContaining({ displayName: 'Bank', ruleCount: 1 }),
            ]),
        );
    });

    test('normalizes route ids, empty account state, selected-name fallback, and priority input', async () => {
        const { bindings } = setup();
        await flush();

        expect(bindings.normalizeRouteAccountId(undefined)).toBeNull();
        expect(bindings.normalizeRouteAccountId(null)).toBeNull();
        expect(bindings.normalizeRouteAccountId('bad-id')).toBeNull();
        expect(bindings.normalizeRouteAccountId('0')).toBeNull();
        expect(bindings.normalizeRouteAccountId('20')).toBe(20);
        expect(bindings.isAccountLocked.value).toBe(false);
        expect(mockServices.getAccountRules).toHaveBeenCalledWith(undefined);

        bindings.ruleForm.value.accountId = 'missing';
        expect(bindings.selectedAccountName.value).toBe('tt:Account');
        expect(bindings.autoRuleName.value).toBe('tt:Account · tt:Account Rule');

        bindings.updatePriority({ target: { value: '42' } } as any);
        expect(bindings.ruleForm.value.priority).toBe(42);
        bindings.updatePriority({ target: { value: 'Infinity' } } as any);
        expect(bindings.ruleForm.value.priority).toBe(100);
        bindings.updatePriority({ target: null } as any);
        expect(bindings.ruleForm.value.priority).toBe(100);

        mockAccountsStore.allMixedPlainAccounts = [];
        bindings.openCreateSheet();
        expect(bindings.ruleForm.value.accountId).toBe('');
        expect(bindings.showEditSheet.value).toBe(true);
        expect(bindings.accountRuleGroups.value).toHaveLength(1);
        expect(bindings.accountRuleGroups.value[0]).toMatchObject({
            categoryName: 'tt:Account',
            ruleCount: 2,
        });
    });

    test('opens create, edit, test, and delete surfaces with isolated form state', async () => {
        const { bindings } = setup();
        await flush();
        const rule = bindings.accountRules.value[0];

        bindings.openCreateSheet();
        expect(bindings.editingRule.value).toBeNull();
        expect(bindings.ruleForm.value).toMatchObject({ accountId: '10', priority: 100, enabled: true });

        bindings.openEditSheet(rule);
        expect(bindings.editingRule.value).toBe(rule);
        expect(bindings.ruleForm.value).toMatchObject({
            accountId: '11',
            name: 'Wallet merchant',
            ruleExpression: 'coffee !refund',
        });

        bindings.openTestSheet(rule);
        expect(bindings.testRuleId.value).toBe(7);
        expect(bindings.testText.value).toBe('');
        expect(bindings.testResult.value).toBeNull();
        expect(bindings.showTestSheet.value).toBe(true);

        bindings.confirmDelete(rule);
        expect(bindings.deletingRule.value).toBe(rule);
        expect(bindings.showDeleteActionSheet.value).toBe(true);
    });
});

describe('mobile account RuleListPage create and edit behavior', () => {
    test('creates a rule with only the canonical account-scoped payload', async () => {
        const { bindings } = setup();
        await flush();
        mockServices.getAccountRules.mockClear();

        bindings.openCreateSheet();
        Object.assign(bindings.ruleForm.value, {
            accountId: '11',
            name: '',
            priority: 25,
            ruleExpression: ' merchant ',
            regexEnabled: true,
            enabled: false,
        });

        const pending = bindings.saveRule();
        expect(bindings.saving.value).toBe(true);
        const loadingPredicate = mockShowLoading.mock.calls.at(-1)?.[0] as () => boolean;
        expect(loadingPredicate()).toBe(true);
        await pending;

        expect(mockServices.createAccountRule).toHaveBeenCalledWith({
            account_id: 11,
            name: 'Wallet · tt:Account Rule',
            priority: 25,
            rule_expression: 'merchant',
            regex_enabled: true,
            enabled: false,
        });
        const payload = mockServices.createAccountRule.mock.calls[0]?.[0] as Record<string, unknown>;
        expect(Object.keys(payload).sort()).toStrictEqual([
            'account_id', 'enabled', 'name', 'priority', 'regex_enabled', 'rule_expression',
        ]);
        expect(payload).not.toHaveProperty('accountRoleScope');
        expect(payload).not.toHaveProperty('transactionTypeScope');
        expect(payload).not.toHaveProperty('fieldScope');
        expect(payload).not.toHaveProperty('token');
        expect(bindings.showEditSheet.value).toBe(false);
        expect(bindings.saving.value).toBe(false);
        expect(loadingPredicate()).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('Rule created');
        expect(mockServices.getAccountRules).toHaveBeenCalledWith(undefined);
        expect(mockHideLoading).toHaveBeenCalled();
    });

    test('updates the selected rule id without changing the canonical payload boundary', async () => {
        const { bindings } = setup('11');
        await flush();
        bindings.openEditSheet(bindings.accountRules.value[0]);
        bindings.ruleForm.value.name = 'Updated wallet rule';

        await bindings.saveRule();

        expect(mockServices.updateAccountRule).toHaveBeenCalledWith(7, expect.objectContaining({
            account_id: 11,
            name: 'Updated wallet rule',
            rule_expression: 'coffee !refund',
        }));
        expect(mockServices.createAccountRule).not.toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('Rule updated');
        expect(mockServices.getAccountRules).toHaveBeenLastCalledWith(11);
    });

    test('reports validation, structured, unsuccessful, and opaque save failures without leaking objects', async () => {
        const { bindings } = setup();
        await flush();
        bindings.openCreateSheet();
        bindings.ruleForm.value.accountId = '';
        bindings.ruleForm.value.ruleExpression = 'merchant';
        await bindings.saveRule();
        expect(mockServices.createAccountRule).not.toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('tt:Account is required');

        bindings.ruleForm.value.accountId = '11';
        bindings.ruleForm.value.ruleExpression = '';
        await bindings.saveRule();
        expect(mockShowToast).toHaveBeenCalledWith('tt:Expression is required');

        bindings.ruleForm.value.ruleExpression = 'merchant';
        mockServices.createAccountRule.mockResolvedValueOnce(failedResponse());
        await bindings.saveRule();
        expect(mockShowToast).toHaveBeenCalledWith('tt:tt:Failed to save rule');

        mockServices.createAccountRule.mockRejectedValueOnce(new Error('upstream denied'));
        await bindings.saveRule();
        expect(mockShowToast).toHaveBeenCalledWith('tt:upstream denied');

        mockServices.createAccountRule.mockRejectedValueOnce({
            internalCredential: 'credential-canary',
            userId: 999,
        });
        await bindings.saveRule();
        expect(mockShowToast).toHaveBeenCalledWith('tt:Failed to save rule');
        expect(mockShowToast.mock.calls.flat().join(' ')).not.toContain('credential-canary');
        expect(bindings.getRequestErrorMessage(new Error(''), 'safe fallback')).toBe('safe fallback');
        expect(bindings.saving.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
    });
});

describe('mobile account RuleListPage test and delete behavior', () => {
    test('skips blank tests and submits a normalized rule context for matched and unmatched results', async () => {
        const { bindings } = setup();
        await flush();
        const rule = bindings.accountRules.value[0];
        bindings.openTestSheet(rule);

        bindings.testText.value = '   ';
        await bindings.runTest();
        expect(mockServices.testAccountRule).not.toHaveBeenCalled();

        bindings.testText.value = '  merchant text  ';
        const matched = bindings.runTest();
        expect(bindings.testing.value).toBe(true);
        const loadingPredicate = mockShowLoading.mock.calls.at(-1)?.[0] as () => boolean;
        expect(loadingPredicate()).toBe(true);
        await matched;
        expect(mockServices.testAccountRule).toHaveBeenCalledWith(7, {
            context: {
                text: 'merchant text',
                counterparty: 'merchant text',
                paymentMethod: 'merchant text',
                description: 'merchant text',
                parserId: 'merchant text',
                parserLabel: 'merchant text',
            },
        });
        const context = mockServices.testAccountRule.mock.calls[0]?.[1] as Record<string, unknown>;
        expect(context).not.toHaveProperty('userId');
        expect(context).not.toHaveProperty('token');
        expect(bindings.testResult.value).toBe(true);
        expect(bindings.testing.value).toBe(false);
        expect(loadingPredicate()).toBe(false);

        mockServices.testAccountRule.mockResolvedValueOnce(successResponse({ matched: false }));
        await bindings.runTest();
        expect(bindings.testResult.value).toBe(false);
        mockServices.testAccountRule.mockResolvedValueOnce(successResponse(undefined));
        await bindings.runTest();
        expect(bindings.testResult.value).toBe(false);
    });

    test('clears test results and emits only safe messages for every test failure shape', async () => {
        const { bindings } = setup();
        await flush();
        bindings.openTestSheet(bindings.accountRules.value[0]);
        bindings.testText.value = 'merchant';
        bindings.testResult.value = true;

        mockServices.testAccountRule.mockResolvedValueOnce(failedResponse());
        await bindings.runTest();
        expect(bindings.testResult.value).toBeNull();
        expect(mockShowToast).toHaveBeenCalledWith('tt:tt:Test failed');

        mockServices.testAccountRule.mockRejectedValueOnce(new Error('test unavailable'));
        await bindings.runTest();
        expect(mockShowToast).toHaveBeenCalledWith('tt:test unavailable');

        mockServices.testAccountRule.mockRejectedValueOnce({
            authorizationHeader: 'credential-canary',
        });
        await bindings.runTest();
        expect(mockShowToast).toHaveBeenCalledWith('tt:Test failed');
        expect(mockShowToast.mock.calls.flat().join(' ')).not.toContain('credential-canary');
        expect(bindings.testing.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
    });

    test('deletes the selected rule, reloads the route scope, and handles an empty selection', async () => {
        const { bindings } = setup('11');
        await flush();
        await bindings.deleteRule();
        expect(mockServices.deleteAccountRule).not.toHaveBeenCalled();

        bindings.confirmDelete(bindings.accountRules.value[0]);
        await bindings.deleteRule();

        expect(mockServices.deleteAccountRule).toHaveBeenCalledWith(7);
        expect(bindings.deletingRule.value).toBeNull();
        expect(bindings.showDeleteActionSheet.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('Rule deleted');
        expect(mockServices.getAccountRules).toHaveBeenLastCalledWith(11);
        expect(mockShowLoading).toHaveBeenCalledWith();
        expect(mockHideLoading).toHaveBeenCalled();
    });

    test('keeps deletion state recoverable and redacts opaque service failures', async () => {
        const { bindings } = setup();
        await flush();
        const rule = bindings.accountRules.value[0];

        bindings.confirmDelete(rule);
        mockServices.deleteAccountRule.mockResolvedValueOnce(failedResponse());
        await bindings.deleteRule();
        expect(bindings.deletingRule.value).toBe(rule);
        expect(mockShowToast).toHaveBeenCalledWith('tt:tt:Failed to delete rule');

        bindings.confirmDelete(rule);
        mockServices.deleteAccountRule.mockRejectedValueOnce(new Error('delete denied'));
        await bindings.deleteRule();
        expect(mockShowToast).toHaveBeenCalledWith('tt:delete denied');

        bindings.confirmDelete(rule);
        mockServices.deleteAccountRule.mockRejectedValueOnce({
            sessionSecret: 'credential-canary',
        });
        await bindings.deleteRule();
        expect(mockShowToast).toHaveBeenCalledWith('tt:Failed to delete rule');
        expect(mockShowToast.mock.calls.flat().join(' ')).not.toContain('credential-canary');
    });
});

describe('mobile account RuleListPage loading, refresh, and production template', () => {
    test('handles null lists, API failures, account failures, and pull-to-refresh completion', async () => {
        mockServices.getAccountRules.mockResolvedValueOnce(successResponse(null));
        const empty = setup();
        await flush();
        expect(empty.bindings.accountRules.value).toStrictEqual([]);

        mockServices.getAccountRules.mockResolvedValueOnce(failedResponse());
        const unsuccessful = setup();
        await flush();
        expect(unsuccessful.bindings.loading.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('tt:tt:Failed to load account rules');

        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce(new Error('account load denied'));
        const accountFailure = setup();
        await flush();
        expect(accountFailure.bindings.loading.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('tt:account load denied');

        mockServices.getAccountRules.mockRejectedValueOnce({
            privateScope: 'credential-canary',
        });
        const opaque = setup();
        await flush();
        expect(opaque.bindings.loading.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('tt:Failed to load account rules');
        expect(mockShowToast.mock.calls.flat().join(' ')).not.toContain('credential-canary');

        mockServices.getAccountRules.mockResolvedValue(successResponse(initialRules));
        const done = jest.fn();
        empty.bindings.reload(done);
        await flush();
        expect(done).toHaveBeenCalledTimes(1);
        empty.bindings.reload();
        await flush();
        expect(empty.bindings.loading.value).toBe(false);
    });

    test('renders and invokes loading, grouped, empty, sheet, action, toggle, and refresh branches', async () => {
        const mounted = mountWithHostRenderer();
        try {
            expect(mounted.root.children.length).toBeGreaterThan(0);
            await flush();

            const groupedNodes: any[] = [];
            collectHostNodes(mounted.root, groupedNodes);
            expect(groupedNodes.some(node => node.props?.['data-testid'] === 'mobile.account-rules.page')).toBe(true);
            expect(groupedNodes.some(node => (
                node.props?.backLink === 'tt:Back' || node.props?.['back-link'] === 'tt:Back'
            ))).toBe(true);
            expect(groupedNodes.some(node => (
                node.props?.enableFilter === true || node.props?.['enable-filter'] === true
            ))).toBe(true);

            mounted.state.showEditSheet = true;
            mounted.state.editingRule = mounted.state.accountRules[0];
            mounted.state.showAccountPopup = true;
            mounted.state.showTestSheet = true;
            mounted.state.testText = 'merchant';
            mounted.state.testResult = true;
            mounted.state.showDeleteActionSheet = true;
            mounted.state.deletingRule = mounted.state.accountRules[0];
            await actualVue.nextTick();

            const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
            collectHostCallbacks(mounted.root, callbacks);
            expect(callbacks.length).toBeGreaterThan(12);
            await invokeHostCallbacks(callbacks);

            mounted.state.showEditSheet = true;
            mounted.state.editingRule = null;
            mounted.state.showTestSheet = true;
            mounted.state.testResult = false;
            mounted.state.showDeleteActionSheet = true;
            mounted.state.deletingRule = null;
            await actualVue.nextTick();

            mounted.state.testResult = null;
            mounted.state.loading = true;
            await actualVue.nextTick();
            mounted.state.loading = false;
            mounted.state.accountRules = [];
            mockAccountsStore.allMixedPlainAccounts = [];
            await actualVue.nextTick();

            const emptyNodes: any[] = [];
            collectHostNodes(mounted.root, emptyNodes);
            expect(emptyNodes.some(node => node.props?.class?.disabled === true || node.props?.class === 'disabled')).toBe(true);
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }

        const locked = mountWithHostRenderer('11');
        try {
            await flush();
            locked.state.showEditSheet = true;
            locked.state.editingRule = locked.state.accountRules[0];
            await actualVue.nextTick();
            const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
            collectHostCallbacks(locked.root, callbacks);
            await invokeHostCallbacks(callbacks);
            expect(locked.state.isAccountLocked).toBe(true);
            expect(mockServices.getAccountRules).toHaveBeenCalledWith(11);
        } finally {
            locked.app.unmount();
        }
    });
});
