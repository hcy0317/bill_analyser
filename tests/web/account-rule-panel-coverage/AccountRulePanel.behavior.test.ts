import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mountedCallbacks: Array<() => unknown> = [];
const templateRefs = new Map<string, { value: unknown }>();
const templateHandlers: Array<(event?: unknown) => unknown> = [];

const mockServices = {
    createAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
    getAccountRules: jest.fn<(...args: any[]) => Promise<any>>(),
    reorderAccountRules: jest.fn<(...args: any[]) => Promise<any>>(),
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

const mockAccountsStore: any = {
    allMixedPlainAccounts: [parentAccount, childAccount, bankAccount],
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => unknown) => mountedCallbacks.push(callback),
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            templateRefs.set(name, target);
            return target;
        },
    };
});

jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => mockAccountsStore,
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => key }),
}));

function createCaptureComponent(name: string): Record<string, unknown> {
    return {
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [attribute, value] of Object.entries(attrs)) {
                if (attribute.startsWith('on') && typeof value === 'function') {
                    templateHandlers.push(value as (event?: unknown) => unknown);
                }
            }
            const { h } = jest.requireActual('vue') as any;
            return () => h('div', { 'data-stub': name }, Object.entries(slots).flatMap(([slotName, slot]) => {
                if (typeof slot !== 'function') return [];
                try {
                    return (slot as (props?: any) => unknown[])({
                        props: { role: 'button' },
                        item: { enabled: true },
                        name: slotName,
                    });
                } catch {
                    return [];
                }
            }));
        },
    };
}

jest.mock('@/components/desktop/ItemIcon.vue', () => ({
    __esModule: true,
    default: createCaptureComponent('ItemIconCoverageStub'),
}));
jest.mock('@/components/desktop/SettingsJsonImportExportButton.vue', () => ({
    __esModule: true,
    default: createCaptureComponent('SettingsImportExportCoverageStub'),
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: createCaptureComponent('SnackBarCoverageStub'),
}));
jest.mock('@/views/desktop/pairingcenter/components/RuleExpressionDisplay.vue', () => ({
    __esModule: true,
    default: createCaptureComponent('RuleExpressionDisplayCoverageStub'),
}));
jest.mock('@/views/desktop/pairingcenter/components/AccountRuleDialogs.vue', () => ({
    __esModule: true,
    default: createCaptureComponent('AccountRuleDialogsCoverageStub'),
}));

const AccountRulePanel = require('@/views/desktop/pairingcenter/components/AccountRulePanel.vue').default as any;

function createRawRule(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        id: 1,
        account_id: 11,
        account_name: 'Wallet',
        account_type: 1,
        name: 'Wallet merchant',
        priority: 10,
        rule_expression: 'coffee !refund',
        regex_enabled: false,
        enabled: true,
        applied_count: 4,
        match_count: 5,
        ...overrides,
    };
}

function successResponse(result: unknown = undefined): any {
    return { data: { success: true, result } };
}

function setupPanel(
    overrides: Record<string, unknown> = {},
    reactiveProps = false,
): { bindings: any; exposed: Record<string, unknown>; props: any } {
    const baseProps = {
        accountId: null,
        title: '',
        hideHeader: false,
        headerActionsTarget: '',
        showSettingsBundleControls: true,
        embedded: false,
        ...overrides,
    };
    const props = reactiveProps
        ? (jest.requireActual('vue') as any).reactive(baseProps)
        : baseProps;
    const exposed: Record<string, unknown> = {};
    const bindings = AccountRulePanel.setup(props, {
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value),
    });
    return { bindings, exposed, props };
}

function setSnackbar(value: { showMessage: jest.Mock }): void {
    const target = templateRefs.get('snackbar');
    if (!target) throw new Error('missing snackbar template ref');
    target.value = value;
}

async function flushAsync(): Promise<void> {
    await Promise.resolve();
    await new Promise(resolve => setImmediate(resolve));
    await (jest.requireActual('vue') as any).nextTick();
}

async function renderPanel(
    props: Record<string, unknown>,
    mutate: (bindings: any) => void,
): Promise<string> {
    const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
    const { renderToString } = require('vue/server-renderer') as any;
    const RuntimePanel = {
        ...AccountRulePanel,
        setup(runtimeProps: any, context: any) {
            const bindings = AccountRulePanel.setup(runtimeProps, context);
            mutate(bindings);
            return bindings;
        },
    };
    templateHandlers.length = 0;
    const app = createSSRApp(RuntimePanel, {
        accountId: null,
        title: '',
        hideHeader: false,
        headerActionsTarget: '',
        showSettingsBundleControls: true,
        embedded: false,
        ...props,
    });
    const UiStub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [attribute, value] of Object.entries(attrs)) {
                if (attribute.startsWith('on') && typeof value === 'function') {
                    templateHandlers.push(value as (event?: unknown) => unknown);
                }
            }
            return () => h('div', attrs, Object.entries(slots).flatMap(([slotName, slot]) => {
                if (typeof slot !== 'function') return [];
                try {
                    return (slot as (slotProps?: any) => unknown[])({
                        props: { role: 'button' },
                        item: { enabled: true },
                        name: slotName,
                    });
                } catch {
                    return [];
                }
            }));
        },
    });
    for (const name of [
        'v-row', 'v-col', 'v-card', 'v-card-title', 'v-icon', 'v-spacer', 'v-btn',
        'v-progress-linear', 'v-progress-circular', 'v-alert', 'v-chip', 'v-table',
        'v-switch', 'v-tooltip',
    ]) {
        app.component(name, UiStub);
    }
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

async function invokeCapturedTemplateHandlers(): Promise<void> {
    const values: unknown[] = [
        undefined,
        true,
        false,
        ['rule:1:0'],
        { key: 'Enter', stopPropagation: jest.fn(), preventDefault: jest.fn() },
        { key: ' ', stopPropagation: jest.fn(), preventDefault: jest.fn() },
    ];
    for (const handler of [...templateHandlers]) {
        for (const value of values) {
            try {
                await handler(value);
            } catch {
                // Generated v-model/event wrappers intentionally receive different value shapes.
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mountedCallbacks.length = 0;
    templateRefs.clear();
    templateHandlers.length = 0;
    mockAccountsStore.allMixedPlainAccounts = [parentAccount, childAccount, bankAccount];
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockServices.getAccountRules.mockResolvedValue(successResponse([
        createRawRule(),
        createRawRule({
            id: 2,
            account_id: 20,
            account_name: 'Bank',
            account_type: 3,
            name: 'Bank regex',
            priority: 20,
            rule_expression: '^salary$',
            regex_enabled: true,
            enabled: false,
        }),
    ]));
    mockServices.createAccountRule.mockResolvedValue(successResponse({ id: 3 }));
    mockServices.updateAccountRule.mockResolvedValue(successResponse({ id: 1 }));
    mockServices.deleteAccountRule.mockResolvedValue(successResponse());
    mockServices.reorderAccountRules.mockResolvedValue(successResponse());
    mockServices.testAccountRule.mockResolvedValue(successResponse({ matched: true }));
});

describe('AccountRulePanel production-loaded setup and grouping', () => {
    test('loads accounts and normalized rules on mount and exposes refresh', async () => {
        const { bindings, exposed } = setupPanel();

        expect(exposed['refresh']).toEqual(expect.any(Function));
        expect(mountedCallbacks).toHaveLength(1);
        expect(bindings.title.value).toBe('');
        expect(bindings.hasHeaderActionsTarget.value).toBe(false);
        expect(bindings.headerActionsTarget.value).toBe('body');
        expect(bindings.isAccountLocked.value).toBe(false);
        expect(bindings.accountOptions.value).toHaveLength(3);

        await mountedCallbacks[0]!();
        await flushAsync();

        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockServices.getAccountRules).toHaveBeenCalledWith(undefined);
        expect(bindings.loading.value).toBe(false);
        expect(bindings.error.value).toBeNull();
        expect(bindings.accountRules.value).toHaveLength(2);
        expect(bindings.orderedAccountRules.value.map((rule: any) => rule.id)).toStrictEqual([1, 2]);
        expect(bindings.accountRuleGroups.value).toHaveLength(2);
        expect(bindings.expandedCategoryKeys.value).toStrictEqual(
            bindings.accountRuleGroups.value.map((group: any) => group.key),
        );
        expect(bindings.getOrderedRuleIndex(bindings.accountRules.value[1])).toBe(1);
        expect(bindings.getRuleExpressionGroups(bindings.accountRules.value[0])).not.toHaveLength(0);
    });

    test('projects locked props, rule builder state, selection, and category expansion', async () => {
        const { bindings } = setupPanel({
            accountId: 11,
            title: 'Wallet rules',
            headerActionsTarget: '#actions',
            embedded: true,
        });
        await bindings.fetchAll();
        await flushAsync();

        expect(bindings.title.value).toBe('Wallet rules');
        expect(bindings.hasHeaderActionsTarget.value).toBe(true);
        expect(bindings.headerActionsTarget.value).toBe('#actions');
        expect(bindings.isAccountLocked.value).toBe(true);
        expect(mockServices.getAccountRules).toHaveBeenCalledWith(11);

        bindings.openCreateDialog();
        expect(bindings.editingRule.value).toBeNull();
        expect(bindings.ruleForm.value.accountId).toBe('11');
        expect(bindings.autoRuleName.value).toBe('Wallet · Account Rule');
        expect(bindings.showEditDialog.value).toBe(true);

        expect(bindings.ruleBuilderModel.value).toMatchObject({
            priority: 100,
            ruleExpression: '',
            regexEnabled: false,
            enabled: true,
        });
        bindings.ruleBuilderModel.value = {
            priority: 8,
            ruleExpression: 'wallet',
            regexEnabled: true,
            enabled: false,
        };
        expect(bindings.ruleForm.value).toMatchObject({
            priority: 8,
            ruleExpression: 'wallet',
            regexEnabled: true,
            enabled: false,
        });

        const key = bindings.accountRuleGroups.value[0].key;
        expect(bindings.isCategoryExpanded(key)).toBe(true);
        bindings.toggleCategoryGroup(key);
        expect(bindings.isCategoryExpanded(key)).toBe(false);
        bindings.toggleCategoryGroup(key);
        expect(bindings.isCategoryExpanded(key)).toBe(true);
    });

    test('reacts to account prop changes and keeps a locked open form aligned', async () => {
        const { bindings, props } = setupPanel({ accountId: null }, true);
        bindings.showEditDialog.value = true;
        props.accountId = 20;
        await flushAsync();

        expect(bindings.ruleForm.value.accountId).toBe('20');
        expect(mockServices.getAccountRules).toHaveBeenCalledWith(20);

        props.accountId = null;
        await flushAsync();
        expect(mockServices.getAccountRules).toHaveBeenCalledWith(undefined);
    });

    test('keeps loading cleanup truthful when the account load rejects', async () => {
        const { bindings } = setupPanel();
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce(new Error('accounts unavailable'));

        await expect(bindings.fetchAll()).rejects.toThrow('accounts unavailable');
        expect(bindings.loading.value).toBe(false);
    });
});

describe('AccountRulePanel production-loaded CRUD and API failures', () => {
    test('creates and updates rules, refreshes the list, and reports success', async () => {
        const { bindings } = setupPanel();
        const snackbar = { showMessage: jest.fn() };
        setSnackbar(snackbar);
        await bindings.fetchAll();

        bindings.openCreateDialog();
        bindings.ruleForm.value = {
            accountId: '11',
            name: '',
            priority: 30,
            ruleExpression: ' bakery ',
            regexEnabled: false,
            enabled: true,
        };
        await bindings.saveRule();
        expect(mockServices.createAccountRule).toHaveBeenCalledWith({
            account_id: 11,
            name: 'Wallet · Account Rule',
            priority: 30,
            rule_expression: 'bakery',
            regex_enabled: false,
            enabled: true,
        });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Rule created', undefined);
        expect(bindings.showEditDialog.value).toBe(false);
        expect(bindings.saving.value).toBe(false);

        const firstRule = bindings.accountRules.value[0];
        bindings.openEditDialog(firstRule);
        expect(bindings.editingRule.value.id).toBe(1);
        expect(bindings.ruleForm.value.accountId).toBe('11');
        bindings.ruleForm.value.ruleExpression = 'coffee shop';
        await bindings.saveRule();
        expect(mockServices.updateAccountRule).toHaveBeenCalledWith(1, expect.objectContaining({
            rule_expression: 'coffee shop',
        }));
        expect(snackbar.showMessage).toHaveBeenCalledWith('Rule updated', undefined);
    });

    test('surfaces validation, failed envelopes, thrown errors, and non-error fallbacks', async () => {
        const { bindings } = setupPanel();

        bindings.ruleForm.value.accountId = '';
        await bindings.saveRule();
        expect(bindings.error.value).toBe('Account is required');
        expect(bindings.saving.value).toBe(false);

        bindings.ruleForm.value.accountId = '11';
        bindings.ruleForm.value.ruleExpression = 'coffee';
        mockServices.createAccountRule.mockResolvedValueOnce({ data: { success: false, result: null } });
        await bindings.saveRule();
        expect(bindings.error.value).toBe('Failed to save rule');

        mockServices.createAccountRule.mockRejectedValueOnce(new Error('create unavailable'));
        await bindings.saveRule();
        expect(bindings.error.value).toBe('create unavailable');

        mockServices.createAccountRule.mockRejectedValueOnce({ status: 503 });
        await bindings.saveRule();
        expect(bindings.error.value).toBe('Failed to save rule');
    });

    test('deletes only a confirmed rule and handles delete failures', async () => {
        const { bindings } = setupPanel();
        const snackbar = { showMessage: jest.fn() };
        setSnackbar(snackbar);
        await bindings.fetchAll();

        await bindings.doDelete();
        expect(mockServices.deleteAccountRule).not.toHaveBeenCalled();

        bindings.confirmDelete(bindings.accountRules.value[0]);
        expect(bindings.showDeleteDialog.value).toBe(true);
        await bindings.doDelete();
        expect(mockServices.deleteAccountRule).toHaveBeenCalledWith(1);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Rule deleted', undefined);
        expect(bindings.showDeleteDialog.value).toBe(false);
        expect(bindings.deleting.value).toBe(false);

        bindings.confirmDelete(bindings.accountRules.value[0]);
        mockServices.deleteAccountRule.mockResolvedValueOnce({ data: { success: false, result: null } });
        await bindings.doDelete();
        expect(bindings.error.value).toBe('Failed to delete rule');

        mockServices.deleteAccountRule.mockRejectedValueOnce(new Error('delete unavailable'));
        await bindings.doDelete();
        expect(bindings.error.value).toBe('delete unavailable');
    });

    test('opens and runs rule tests for matched, unmatched, and failed responses', async () => {
        const { bindings } = setupPanel();
        await bindings.fetchAll();

        bindings.openTestDialog(bindings.accountRules.value[0]);
        expect(bindings.testRuleId.value).toBe(1);
        expect(bindings.testRuleName.value).toBe('Wallet merchant');
        expect(bindings.testText.value).toBe('');
        expect(bindings.testResult.value).toBeNull();
        await bindings.runTest();
        expect(mockServices.testAccountRule).not.toHaveBeenCalled();

        bindings.testText.value = ' coffee shop ';
        await bindings.runTest();
        expect(mockServices.testAccountRule).toHaveBeenCalledWith(1, {
            context: {
                text: 'coffee shop',
                counterparty: 'coffee shop',
                paymentMethod: 'coffee shop',
                description: 'coffee shop',
                parserId: 'coffee shop',
                parserLabel: 'coffee shop',
            },
        });
        expect(bindings.testResult.value).toBe(true);
        expect(bindings.testing.value).toBe(false);

        mockServices.testAccountRule.mockResolvedValueOnce(successResponse({ matched: null }));
        await bindings.runTest();
        expect(bindings.testResult.value).toBe(false);

        mockServices.testAccountRule.mockResolvedValueOnce({ data: { success: false, result: null } });
        await bindings.runTest();
        expect(bindings.testResult.value).toBeNull();
        expect(bindings.error.value).toBe('Test failed');

        mockServices.testAccountRule.mockRejectedValueOnce(new Error('test unavailable'));
        await bindings.runTest();
        expect(bindings.error.value).toBe('test unavailable');
    });
});

describe('AccountRulePanel production-loaded optimistic actions', () => {
    test('toggles enabled state optimistically and prevents duplicate or no-op requests', async () => {
        const { bindings } = setupPanel();
        await bindings.fetchAll();
        const firstRule = bindings.accountRules.value[0];

        await bindings.toggleEnabled(firstRule, true);
        expect(mockServices.updateAccountRule).not.toHaveBeenCalled();

        bindings.togglingRuleIds.value = [firstRule.id];
        expect(bindings.isRuleToggling(firstRule.id)).toBe(true);
        await bindings.toggleEnabled(firstRule, false);
        expect(mockServices.updateAccountRule).not.toHaveBeenCalled();
        bindings.togglingRuleIds.value = [];

        await bindings.toggleEnabled(firstRule, false);
        expect(mockServices.updateAccountRule).toHaveBeenCalledWith(1, { enabled: false });
        expect(bindings.isRuleToggling(firstRule.id)).toBe(false);
        expect(bindings.error.value).toBeNull();
    });

    test('rolls back a failed enabled-state update and accepts truthy switch values', async () => {
        const { bindings } = setupPanel();
        await bindings.fetchAll();
        const previousRules = bindings.accountRules.value;
        const firstRule = bindings.accountRules.value[0];

        mockServices.updateAccountRule.mockResolvedValueOnce({ data: { success: false, result: null } });
        await bindings.toggleEnabled(firstRule, false);
        expect(bindings.accountRules.value).toBe(previousRules);
        expect(bindings.error.value).toBe('Failed to toggle rule');

        const secondRule = bindings.accountRules.value[1];
        mockServices.updateAccountRule.mockRejectedValueOnce(new Error('toggle unavailable'));
        await bindings.toggleEnabled(secondRule, 1);
        expect(bindings.error.value).toBe('toggle unavailable');
        expect(bindings.togglingRuleIds.value).toStrictEqual([]);
    });

    test('reorders rules in both directions and ignores invalid boundaries', async () => {
        const { bindings } = setupPanel();
        await bindings.fetchAll();
        const [firstRule, secondRule] = bindings.orderedAccountRules.value;

        await bindings.moveRule(firstRule, -1);
        await bindings.moveRule(secondRule, 1);
        await bindings.moveRule({ ...firstRule, id: 999 }, 1);
        expect(mockServices.reorderAccountRules).not.toHaveBeenCalled();

        await bindings.moveRule(firstRule, 1);
        expect(mockServices.reorderAccountRules).toHaveBeenCalledWith([2, 1]);
        expect(bindings.reordering.value).toBe(false);

        mockServices.reorderAccountRules.mockResolvedValueOnce({ data: { success: false, result: null } });
        await bindings.moveRule(secondRule, -1);
        expect(bindings.error.value).toBe('Failed to reorder rules');

        mockServices.reorderAccountRules.mockRejectedValueOnce(new Error('reorder unavailable'));
        await bindings.moveRule(firstRule, 1);
        expect(bindings.error.value).toBe('reorder unavailable');
    });
});

describe('AccountRulePanel production-loaded template', () => {
    test('renders loading/error/empty/header variants and executes template event wrappers', async () => {
        const emptyHtml = await renderPanel({ title: 'Rules', embedded: true }, bindings => {
            bindings.loading.value = true;
            bindings.error.value = 'load failed';
            bindings.accountRules.value = [];
        });
        expect(emptyHtml).toContain('Rules');
        expect(emptyHtml).toContain('load failed');
        expect(emptyHtml).toContain('No account rules');
        await invokeCapturedTemplateHandlers();

        const hiddenHeaderHtml = await renderPanel({ hideHeader: true, showSettingsBundleControls: false }, bindings => {
            bindings.accountRules.value = [];
        });
        expect(hiddenHeaderHtml).toContain('Account Recognition Rules');
    });

    test('renders grouped account rules, external actions, dialog bindings, and row actions', async () => {
        const html = await renderPanel({
            headerActionsTarget: '#actions',
            accountId: 11,
        }, bindings => {
            const first = createRawRule();
            const second = createRawRule({
                id: 2,
                priority: 20,
                name: '',
                account_name: '',
                rule_expression: '^wallet$',
                regex_enabled: true,
                enabled: false,
            });
            const { normalizeAccountRuleItem } = require('@/models/account_rule.ts') as any;
            bindings.accountRules.value = [
                normalizeAccountRuleItem(first),
                normalizeAccountRuleItem(second),
            ];
            bindings.expandedCategoryKeys.value = ['category:1'];
            bindings.error.value = null;
        });

        expect(html).toContain('Rule Matching Expression');
        expect(html).toContain('Wallet');
        expect(templateHandlers.length).toBeGreaterThan(0);
        await invokeCapturedTemplateHandlers();
    });
});
