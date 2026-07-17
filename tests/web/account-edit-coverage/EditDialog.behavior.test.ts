import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockAccountType = {
    SingleAccount: { type: 1 },
    MultiSubAccounts: { type: 2 },
} as const;

class MockAccount {
    public id: string;
    public name: string;
    public parentId: string;
    public category: number;
    public type: number;
    public icon: string;
    public color: string;
    public currency: string;
    public balanceCents: number;
    public balanceTime?: number;
    public comment: string;
    public creditCardStatementDate?: number;
    public displayOrder: number;
    public visible: boolean;
    public subAccounts?: MockAccount[];
    public liability: boolean;

    public constructor(overrides: Partial<MockAccount> = {}) {
        this.id = overrides.id ?? '';
        this.name = overrides.name ?? '';
        this.parentId = overrides.parentId ?? '';
        this.category = overrides.category ?? 1;
        this.type = overrides.type ?? mockAccountType.SingleAccount.type;
        this.icon = overrides.icon ?? 'cash';
        this.color = overrides.color ?? '#123456';
        this.currency = overrides.currency ?? 'CNY';
        this.balanceCents = overrides.balanceCents ?? 0;
        this.balanceTime = overrides.balanceTime ?? 1_700_000_000;
        this.comment = overrides.comment ?? '';
        this.creditCardStatementDate = overrides.creditCardStatementDate;
        this.displayOrder = overrides.displayOrder ?? 0;
        this.visible = overrides.visible ?? true;
        this.subAccounts = overrides.subAccounts;
        this.liability = overrides.liability ?? false;
    }

    public static createNewAccount(currency: string, balanceTime: number): MockAccount {
        return new MockAccount({ currency, balanceTime });
    }

    public get isLiability(): boolean {
        return this.liability;
    }

    public fillFrom(other: MockAccount): void {
        for (const key of [
            'id', 'name', 'parentId', 'category', 'type', 'icon', 'color', 'currency',
            'balanceCents', 'balanceTime', 'comment', 'creditCardStatementDate',
            'displayOrder', 'visible', 'liability',
        ] as const) {
            (this as any)[key] = other[key];
        }
    }

    public equals(other: MockAccount): boolean {
        return this.id === other.id
            && this.name === other.name
            && this.parentId === other.parentId
            && this.category === other.category
            && this.type === other.type
            && this.icon === other.icon
            && this.color === other.color
            && this.currency === other.currency
            && this.balanceCents === other.balanceCents
            && this.balanceTime === other.balanceTime
            && this.comment === other.comment
            && this.creditCardStatementDate === other.creditCardStatementDate
            && this.visible === other.visible
            && this.liability === other.liability;
    }

    public setSuitableIcon(_oldCategory: number, newCategory: number): void {
        this.icon = `category-${newCategory}`;
    }

    public createNewSubAccount(currency: string, balanceTime: number): MockAccount {
        return new MockAccount({
            parentId: this.id,
            category: 0,
            type: mockAccountType.SingleAccount.type,
            icon: this.icon,
            color: this.color,
            currency,
            balanceTime,
        });
    }
}

type MockBase = ReturnType<typeof createMockBase>;

let mockLastBase: MockBase;
let mockMountedCallback: (() => void) | undefined;
let mockUnmountedCallback: (() => void) | undefined;

const mockTemplateRefs = new Map<string, any>();
const mockTemplateEvents: Array<{ name: string; handler: (...args: any[]) => any }> = [];
const mockGenerateUuid = jest.fn<() => string>();
const mockAccountsStore = {
    getAccount: jest.fn<(...args: any[]) => Promise<MockAccount>>(),
    saveAccount: jest.fn<(...args: any[]) => Promise<MockAccount>>(),
};
const mockServices = {
    getAccountRules: jest.fn<(...args: any[]) => Promise<any>>(),
    createAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
    updateAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteAccountRule: jest.fn<(...args: any[]) => Promise<any>>(),
};

function createAccount(overrides: Partial<MockAccount> = {}): MockAccount {
    return new MockAccount(overrides);
}

function createMockBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const editAccountId = ref(null as string | null);
    const account = ref(MockAccount.createNewAccount('CNY', 1_700_000_000));
    const subAccounts = ref([] as MockAccount[]);

    const getProblem = (target: MockAccount, isSubAccount: boolean): string | null => {
        if (!isSubAccount && !target.category) return 'Account category cannot be blank';
        if (!isSubAccount && !target.type) return 'Account type cannot be blank';
        if (!target.name) return 'Account name cannot be blank';
        if (target.type === mockAccountType.SingleAccount.type && !target.currency) {
            return 'Account currency cannot be blank';
        }
        return null;
    };

    const inputEmptyProblemMessage = computed(() => {
        const parentProblem = getProblem(account.value, false);
        if (parentProblem) return parentProblem;
        if (account.value.type === mockAccountType.MultiSubAccounts.type) {
            for (const subAccount of subAccounts.value) {
                const childProblem = getProblem(subAccount, true);
                if (childProblem) return childProblem;
            }
        }
        return null;
    });

    const isNewAccount = (target: MockAccount): boolean => target.id === '' || target.id === '0';
    const addSubAccount = (): boolean => {
        if (account.value.type !== mockAccountType.MultiSubAccounts.type) return false;
        subAccounts.value.push(account.value.createNewSubAccount('CNY', 1_700_000_000));
        return true;
    };
    const setAccount = (source: MockAccount): void => {
        account.value.fillFrom(source);
        subAccounts.value = (source.subAccounts ?? []).map(child => createAccount(child));
    };

    return {
        editAccountId,
        clientSessionId: ref(''),
        loading: ref(false),
        submitting: ref(false),
        account,
        subAccounts,
        title: computed(() => editAccountId.value ? 'Edit Account' : 'Add Account'),
        saveButtonTitle: computed(() => editAccountId.value ? 'Save' : 'Add'),
        inputEmptyProblemMessage,
        inputIsEmpty: computed(() => !!inputEmptyProblemMessage.value),
        allAccountCategories: ref([
            { type: 1, displayName: 'Cash', defaultAccountIconId: 'cash' },
            { type: 2, displayName: 'Credit Card', defaultAccountIconId: 'credit-card' },
        ]),
        allAccountTypes: ref([
            { type: mockAccountType.SingleAccount.type, displayName: 'Single' },
            { type: mockAccountType.MultiSubAccounts.type, displayName: 'Multi' },
        ]),
        allAvailableMonthDays: ref([{ day: 0, displayName: 'Not set' }, { day: 12, displayName: '12th' }]),
        isAccountSupportCreditCardStatementDate: computed(() => account.value.category === 2),
        isNewAccount,
        addSubAccount,
        setAccount,
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
                        handler: handler as (...args: any[]) => any,
                    });
                }
            }

            return () => h('div', attrs, Object.entries(slots).flatMap(([slotName, slot]) => {
                if (typeof slot !== 'function') return [];
                if (slotName === 'activator') {
                    return (slot as (value: unknown) => unknown[])({ props: { role: 'button' } });
                }
                if (slotName === 'item') {
                    return (slot as (value: unknown) => unknown[])({
                        props: { role: 'option' },
                        item: {
                            value: 1,
                            title: 'Cash',
                            raw: { defaultAccountIconId: 'cash' },
                        },
                    });
                }
                return (slot as () => unknown[])();
            }));
        },
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
        },
    };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => key }),
}));

jest.mock('@/views/base/accounts/AccountEditPageBase.ts', () => ({
    useAccountEditPageBase: () => {
        mockLastBase = createMockBase();
        return mockLastBase;
    },
}));

jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({ currentUserDefaultCurrency: 'CNY' }),
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/models/account.ts', () => ({ Account: MockAccount }));
jest.mock('@/core/account.ts', () => ({ AccountType: mockAccountType }));
jest.mock('@/core/base.ts', () => ({
    itemAndIndex: (items: unknown[]) => items.map((item, index) => [item, index]),
}));
jest.mock('@/consts/icon.ts', () => ({ ALL_ACCOUNT_ICONS: [{ id: 'cash' }, { id: 'bank' }] }));
jest.mock('@/consts/color.ts', () => ({ ALL_ACCOUNT_COLORS: [{ value: '#123456' }, { value: '#abcdef' }] }));
jest.mock('@/lib/common.ts', () => ({ isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value) }));
jest.mock('@/lib/datetime.ts', () => ({ getCurrentUnixTime: () => 1_700_000_000 }));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => mockGenerateUuid() }));
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/models/account_rule.ts', () => ({
    createDefaultAccountRuleForm: (accountId: number | string = '') => ({
        accountId: String(accountId),
        priority: 100,
        ruleExpression: '',
        regexEnabled: false,
        enabled: true,
    }),
    normalizeAccountRuleItem: (item: any) => ({
        id: Number(item.id),
        accountId: Number(item.accountId ?? item.account_id),
        name: String(item.name ?? ''),
        priority: Number(item.priority ?? 100),
        ruleExpression: String(item.ruleExpression ?? item.rule_expression ?? ''),
        regexEnabled: !!(item.regexEnabled ?? item.regex_enabled),
        enabled: item.enabled !== false,
    }),
    accountRuleToForm: (item: any) => ({
        accountId: String(item.accountId),
        priority: item.priority,
        ruleExpression: item.ruleExpression,
        regexEnabled: item.regexEnabled,
        enabled: item.enabled,
    }),
    buildAccountRulePayload: (form: any, name: string) => ({
        accountId: Number(form.accountId),
        name,
        priority: form.priority,
        ruleExpression: form.ruleExpression.trim(),
        regexEnabled: form.regexEnabled,
        enabled: form.enabled,
    }),
}));

for (const componentPath of [
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
    '@/components/common/CategoryRuleBuilderFields.vue',
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: createCaptureStub('AccountEditCoverageChildStub'),
    }));
}

const EditDialog = require('@/views/desktop/accounts/list/dialogs/EditDialog.vue').default as any;

function createBindings(): any {
    mockTemplateRefs.clear();
    const exposed: Record<string, unknown> = {};
    const bindings = EditDialog.setup({}, {
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value),
    });
    expect(exposed).toEqual({ open: bindings.open });
    return bindings;
}

function installDialogRefs(bindings: any): {
    confirm: { open: jest.Mock<(...args: any[]) => Promise<any>> };
    snackbar: { showError: jest.Mock; showMessage: jest.Mock };
} {
    const confirm = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const snackbar = { showError: jest.fn(), showMessage: jest.fn() };
    bindings.confirmDialog.value = confirm;
    bindings.snackbar.value = snackbar;
    return { confirm, snackbar };
}

function successResponse(result: unknown = undefined): any {
    return { data: { success: true, result } };
}

async function flushAsync(times = 6): Promise<void> {
    const { nextTick } = jest.requireActual('vue') as any;
    for (let index = 0; index < times; index++) await Promise.resolve();
    await nextTick();
    await new Promise(resolve => setImmediate(resolve));
}

async function settleCancelled(bindings: any, pending: Promise<any>): Promise<unknown> {
    const settled = pending.then(value => value, error => error);
    bindings.cancel();
    return settled;
}

async function renderDialog(mutator: (bindings: any) => void): Promise<{ html: string; bindings: any }> {
    const { createSSRApp } = require('vue') as any;
    const { renderToString } = require('vue/server-renderer') as any;
    const { readFileSync } = require('node:fs') as typeof import('node:fs');
    const { resolve } = require('node:path') as typeof import('node:path');
    let bindings: any;
    const { render: _emptyExternalTemplateRender, ...scriptOnlyDialog } = EditDialog;
    const RuntimeDialog = {
        ...scriptOnlyDialog,
        template: readFileSync(resolve(
            __dirname,
            '../../../src/web/src/views/desktop/accounts/list/dialogs/edit-dialog/EditDialog.template.html',
        ), 'utf8'),
        setup(props: any, context: any) {
            bindings = EditDialog.setup(props, context);
            mutator(bindings);
            return { ...bindings };
        },
    };
    const app = createSSRApp(RuntimeDialog);
    for (const componentName of [
        'v-dialog', 'v-card', 'v-card-text', 'v-form', 'v-row', 'v-col', 'v-select',
        'v-list-item', 'v-list-item-title', 'v-tabs', 'v-tab', 'v-window', 'v-window-item',
        'v-text-field', 'v-autocomplete', 'v-textarea', 'v-switch', 'v-tooltip', 'v-btn',
        'v-icon', 'v-menu', 'v-list', 'v-progress-circular', 'v-progress-linear', 'v-alert',
        'item-icon', 'icon-select', 'color-select', 'currency-select', 'amount-input',
        'date-time-select', 'category-rule-builder-fields', 'confirm-dialog', 'snack-bar',
    ]) {
        app.component(componentName, createCaptureStub(`AccountEditCoverage${componentName}`));
    }
    app.config.warnHandler = () => undefined;
    return { html: await renderToString(app), bindings };
}

beforeEach(() => {
    jest.resetAllMocks();
    mockTemplateRefs.clear();
    mockTemplateEvents.length = 0;
    mockMountedCallback = undefined;
    mockUnmountedCallback = undefined;
    mockGenerateUuid.mockReturnValue('account-session');
    mockAccountsStore.getAccount.mockResolvedValue(createAccount({ id: '10', name: 'Loaded account' }));
    mockAccountsStore.saveAccount.mockResolvedValue(createAccount({ id: '20', name: 'Saved account', balanceCents: 12_345 }));
    mockServices.getAccountRules.mockResolvedValue(successResponse([]));
    mockServices.createAccountRule.mockResolvedValue(successResponse({ id: 201 }));
    mockServices.updateAccountRule.mockResolvedValue(successResponse({ id: 202 }));
    mockServices.deleteAccountRule.mockResolvedValue(successResponse());

    Object.defineProperty(globalThis, 'HTMLInputElement', {
        configurable: true,
        value: class HTMLInputElement {},
        writable: true,
    });
    Object.defineProperty(globalThis, 'HTMLTextAreaElement', {
        configurable: true,
        value: class HTMLTextAreaElement {},
        writable: true,
    });
    Object.assign(globalThis.window, {
        addEventListener: jest.fn(),
        removeEventListener: jest.fn(),
    });
});

describe('desktop account EditDialog production-loaded state and helpers', () => {
    test('projects main/sub-account selection, presentation fields, and validation state', async () => {
        const bindings = createBindings();

        expect(bindings.selectedAccount.value).toBe(bindings.account.value);
        expect(bindings.selectedAccountRuleAccountId.value).toBeNull();
        expect(bindings.canManageSelectedAccountRules.value).toBe(false);
        expect(bindings.autoSelectedAccountRuleName.value).toBe('Account - Account Rule');
        expect(bindings.accountAmountTitle.value).toBe('Account Balance');
        expect(bindings.isAccountModified.value).toBe(false);
        expect(bindings.inputEmptyProblemMessage.value).toBe('Account name cannot be blank');

        bindings.account.value.name = 'Main wallet';
        bindings.account.value.icon = 'bank';
        bindings.account.value.color = '#abcdef';
        bindings.account.value.currency = 'USD';
        bindings.account.value.balanceCents = 12_345;
        expect(bindings.isAccountModified.value).toBe(true);
        expect(bindings.inputEmptyProblemMessage.value).toBeNull();

        bindings.account.value.liability = true;
        expect(bindings.accountAmountTitle.value).toBe('Account Outstanding Balance');
        bindings.account.value.type = mockAccountType.MultiSubAccounts.type;
        await flushAsync();
        expect(bindings.subAccounts.value).toHaveLength(1);
        const child = bindings.subAccounts.value[0];
        child.id = '11';
        child.name = 'USD child';
        child.currency = 'USD';
        child.icon = 'bank';
        child.color = '#abcdef';
        child.balanceCents = 9_876;
        bindings.currentAccountIndex.value = 0;
        expect(bindings.selectedAccount.value).toBe(child);
        expect(bindings.accountAmountTitle.value).toBe('Sub-account Outstanding Balance');
        bindings.account.value.liability = false;
        expect(bindings.accountAmountTitle.value).toBe('Sub-account Balance');

        bindings.editAccountId.value = '10';
        expect(bindings.selectedAccountRuleAccountId.value).toBe(11);
        expect(bindings.canManageSelectedAccountRules.value).toBe(true);
        expect(bindings.autoSelectedAccountRuleName.value).toBe('USD child - Account Rule');
        child.id = 'not-a-number';
        expect(bindings.selectedAccountRuleAccountId.value).toBeNull();
        child.id = '-3';
        expect(bindings.selectedAccountRuleAccountId.value).toBeNull();
        child.id = '0';
        expect(bindings.canManageSelectedAccountRules.value).toBe(false);
    });

    test('normalizes rule builder values and exercises request helper branches', () => {
        const bindings = createBindings();
        bindings.selectedAccountRuleBuilderModel.value = {
            priority: '7' as any,
            ruleExpression: 42 as any,
            regexEnabled: 1 as any,
            enabled: false,
        };
        expect(bindings.selectedAccountRuleDraft.value).toMatchObject({
            priority: 7,
            ruleExpression: '42',
            regexEnabled: true,
            enabled: false,
        });
        expect(bindings.selectedAccountRuleBuilderModel.value).toEqual({
            priority: 7,
            ruleExpression: '42',
            regexEnabled: true,
            enabled: false,
        });

        bindings.selectedAccountRuleBuilderModel.value = {
            priority: Number.NaN,
            ruleExpression: null as any,
            regexEnabled: 0 as any,
            enabled: undefined as any,
        };
        expect(bindings.selectedAccountRuleDraft.value).toMatchObject({
            priority: 100,
            ruleExpression: '',
            regexEnabled: false,
            enabled: true,
        });

        expect(bindings.requireApiSuccess(successResponse('ok'), 'fallback')).toBe('ok');
        expect(() => bindings.requireApiSuccess({ data: { success: false, result: null } }, 'fallback'))
            .toThrow('fallback');
        expect(bindings.getRequestErrorMessage(new Error('boom'), 'fallback')).toBe('boom');
        expect(bindings.getRequestErrorMessage({ message: 'plain boom' }, 'fallback')).toBe('plain boom');
        expect(bindings.getRequestErrorMessage({ message: 3 }, 'fallback')).toBe('fallback');
        expect(bindings.getRequestErrorMessage(null, 'fallback')).toBe('fallback');
        expect(bindings.isProcessedError({ processed: true })).toBe(true);
        expect(bindings.isProcessedError({ processed: false })).toBe(false);
        expect(bindings.isProcessedError(null)).toBe(false);

        expect(bindings.getSortedAccountRules([
            { id: 5, priority: 20 },
            { id: 9, priority: 10 },
            { id: 2, priority: 10 },
        ]).map((rule: any) => rule.id)).toStrictEqual([2, 9, 5]);
        expect(bindings.buildSelectedAccountRulePayload(12)).toBeNull();
        bindings.selectedAccountRuleDraft.value.ruleExpression = ' coffee ';
        bindings.account.value.name = ' Wallet ';
        expect(bindings.buildSelectedAccountRulePayload(12)).toMatchObject({
            accountId: 12,
            name: 'Wallet - Account Rule',
            ruleExpression: 'coffee',
        });
    });
});

describe('desktop account EditDialog production-loaded open and rule loading', () => {
    test('opens add mode with explicit category defaults and resets reusable state', async () => {
        const bindings = createBindings();
        installDialogRefs(bindings);
        bindings.subAccounts.value = [createAccount({ id: 'old-child' })];
        bindings.currentAccountIndex.value = 0;
        bindings.submitting.value = true;
        bindings.selectedAccountRuleDraft.value.ruleExpression = 'old';

        const pending = bindings.open({ category: 2 });
        expect(bindings.showState.value).toBe(true);
        expect(bindings.loading.value).toBe(false);
        expect(bindings.submitting.value).toBe(false);
        expect(bindings.editAccountId.value).toBeNull();
        expect(bindings.clientSessionId.value).toBe('account-session');
        expect(bindings.account.value.category).toBe(2);
        expect(bindings.account.value.icon).toBe('category-2');
        expect(bindings.subAccounts.value).toStrictEqual([]);
        expect(bindings.currentAccountIndex.value).toBe(-1);
        expect(bindings.selectedAccountRuleDraft.value.ruleExpression).toBe('');
        expect(await settleCancelled(bindings, pending)).toBeUndefined();

        const noCategoryBindings = createBindings();
        const noCategory = noCategoryBindings.open({ category: '2' as any });
        expect(noCategoryBindings.account.value.category).toBe(1);
        expect(await settleCancelled(noCategoryBindings, noCategory)).toBeUndefined();

        const noOptionsBindings = createBindings();
        const noOptions = noOptionsBindings.open();
        expect(noOptionsBindings.editAccountId.value).toBeNull();
        expect(await settleCancelled(noOptionsBindings, noOptions)).toBeUndefined();
    });

    test('opens edit mode from optimistic current data then replaces it with store data', async () => {
        const bindings = createBindings();
        installDialogRefs(bindings);
        const current = createAccount({ id: '10', name: 'Optimistic', balanceCents: 1_234 });
        const loaded = createAccount({
            id: '10',
            name: 'Loaded',
            icon: 'bank',
            color: '#abcdef',
            currency: 'EUR',
            balanceCents: 56_789,
            subAccounts: [createAccount({ id: '11', name: 'Child', currency: 'EUR', balanceCents: 321 })],
        });
        mockAccountsStore.getAccount.mockResolvedValueOnce(loaded);
        const pending = bindings.open({ id: '10', currentAccount: current });
        expect(bindings.account.value.name).toBe('Optimistic');
        expect(bindings.loading.value).toBe(true);
        expect(mockAccountsStore.getAccount).toHaveBeenCalledWith({ accountId: '10' });
        await flushAsync();
        expect(bindings.loading.value).toBe(false);
        expect(bindings.account.value.name).toBe('Loaded');
        expect(bindings.account.value.balanceCents).toBe(56_789);
        expect(bindings.subAccounts.value).toHaveLength(1);
        expect(await settleCancelled(bindings, pending)).toBeUndefined();
    });

    test('rejects unprocessed load failures and quietly closes processed failures', async () => {
        const failedBindings = createBindings();
        mockAccountsStore.getAccount.mockRejectedValueOnce({ processed: false, message: 'load failed' });
        const failed = failedBindings.open({ id: '10' });
        await expect(failed).rejects.toMatchObject({ message: 'load failed' });
        expect(failedBindings.loading.value).toBe(false);
        expect(failedBindings.showState.value).toBe(false);

        const processedBindings = createBindings();
        mockAccountsStore.getAccount.mockRejectedValueOnce({ processed: true });
        const processed = processedBindings.open({ id: '10' });
        await flushAsync();
        expect(processedBindings.showState.value).toBe(false);
        expect(processedBindings.loading.value).toBe(false);
        expect(await settleCancelled(processedBindings, processed)).toBeUndefined();
    });

    test('loads the sorted primary account rule and handles empty, failed, and stale requests', async () => {
        const bindings = createBindings();
        const { snackbar } = installDialogRefs(bindings);
        bindings.showState.value = true;
        bindings.editAccountId.value = '10';
        bindings.account.value.id = '10';
        bindings.account.value.name = 'Checking';
        await flushAsync();

        mockServices.getAccountRules.mockResolvedValueOnce(successResponse([
            { id: 9, account_id: 10, priority: 30, rule_expression: 'later', regex_enabled: false, enabled: true },
            { id: 2, account_id: 10, priority: 5, rule_expression: 'first', regex_enabled: true, enabled: false },
        ]));
        await bindings.loadSelectedAccountRule();
        expect(mockServices.getAccountRules).toHaveBeenCalledWith(10);
        expect(bindings.primaryAccountRuleId.value).toBe(2);
        expect(bindings.selectedAccountRuleDraft.value).toMatchObject({
            accountId: '10',
            priority: 5,
            ruleExpression: 'first',
            regexEnabled: true,
            enabled: false,
        });
        expect(bindings.accountRuleLoading.value).toBe(false);

        mockServices.getAccountRules.mockResolvedValueOnce(successResponse(null));
        await bindings.loadSelectedAccountRule(10);
        expect(bindings.primaryAccountRuleId.value).toBeNull();
        expect(bindings.selectedAccountRuleDraft.value.accountId).toBe('10');

        mockServices.getAccountRules.mockResolvedValueOnce({ data: { success: false, result: [] } });
        await bindings.loadSelectedAccountRule(10);
        expect(bindings.accountRuleLoadFailed.value).toBe(true);
        expect(snackbar.showError).toHaveBeenLastCalledWith('Failed to load account rules');

        mockServices.getAccountRules.mockRejectedValueOnce({ message: 'network rules' });
        await bindings.loadSelectedAccountRule(10);
        expect(snackbar.showError).toHaveBeenLastCalledWith('network rules');

        let resolveSlow: ((value: any) => void) | undefined;
        mockServices.getAccountRules.mockImplementationOnce(() => new Promise(resolve => {
            resolveSlow = resolve;
        }));
        const stale = bindings.loadSelectedAccountRule(10);
        bindings.resetSelectedAccountRuleEditor(11);
        resolveSlow?.(successResponse([{ id: 99, account_id: 10, priority: 1 }]));
        await stale;
        expect(bindings.primaryAccountRuleId.value).toBeNull();

        bindings.showState.value = false;
        await bindings.loadSelectedAccountRule(10);
        expect(bindings.selectedAccountRuleDraft.value.accountId).toBe('10');
        bindings.showState.value = true;
        bindings.editAccountId.value = null;
        await bindings.loadSelectedAccountRule(10);
        await bindings.loadSelectedAccountRule(null);
    });
});

describe('desktop account EditDialog production-loaded saves and destructive actions', () => {
    test('validates before saving and resolves add mode with integer cents unchanged', async () => {
        const bindings = createBindings();
        const { snackbar } = installDialogRefs(bindings);
        const pending = bindings.open();
        await bindings.save();
        expect(snackbar.showMessage).toHaveBeenCalledWith('Account name cannot be blank');
        expect(mockAccountsStore.saveAccount).not.toHaveBeenCalled();

        bindings.account.value.name = 'Cash account';
        bindings.account.value.balanceCents = 12_345;
        bindings.account.value.icon = 'cash';
        bindings.account.value.color = '#123456';
        bindings.account.value.currency = 'CNY';
        const saved = createAccount({
            id: '20', name: 'Cash account', balanceCents: 12_345,
            icon: 'cash', color: '#123456', currency: 'CNY',
        });
        mockAccountsStore.saveAccount.mockResolvedValueOnce(saved);
        await bindings.save();
        await expect(pending).resolves.toEqual({
            message: 'You have added a new account',
            id: '20',
            account: saved,
        });
        expect(mockAccountsStore.saveAccount).toHaveBeenCalledWith({
            account: expect.objectContaining({ balanceCents: 12_345, currency: 'CNY' }),
            subAccounts: [],
            isEdit: false,
            clientSessionId: 'account-session',
        });
        expect(bindings.showState.value).toBe(false);
        expect(bindings.submitting.value).toBe(false);
    });

    test('synchronizes create, update, and delete rule variants during edit saves', async () => {
        const createCaseBindings = createBindings();
        installDialogRefs(createCaseBindings);
        const createPending = createCaseBindings.open({
            id: '10',
            currentAccount: createAccount({ id: '10', name: 'Checking' }),
        });
        await flushAsync();
        createCaseBindings.selectedAccountRuleDraft.value.ruleExpression = ' coffee ';
        createCaseBindings.primaryAccountRuleId.value = null;
        mockAccountsStore.saveAccount.mockResolvedValueOnce(createAccount({ id: '10', name: 'Checking' }));
        await createCaseBindings.save();
        await expect(createPending).resolves.toMatchObject({ message: 'You have saved this account', id: '10' });
        expect(mockServices.createAccountRule).toHaveBeenCalledWith(expect.objectContaining({
            accountId: 10,
            ruleExpression: 'coffee',
        }));
        expect(createCaseBindings.primaryAccountRuleId.value).toBeNull();

        const updateBindings = createBindings();
        installDialogRefs(updateBindings);
        mockAccountsStore.getAccount.mockResolvedValueOnce(createAccount({ id: '12', name: 'Savings' }));
        const updatePending = updateBindings.open({
            id: '12',
            currentAccount: createAccount({ id: '12', name: 'Savings' }),
        });
        await flushAsync();
        updateBindings.selectedAccountRuleDraft.value.ruleExpression = 'salary';
        updateBindings.primaryAccountRuleId.value = 33;
        mockAccountsStore.saveAccount.mockResolvedValueOnce(createAccount({ id: '12', name: 'Savings' }));
        await updateBindings.save();
        await expect(updatePending).resolves.toMatchObject({ id: '12' });
        expect(mockServices.updateAccountRule).toHaveBeenCalledWith(33, expect.objectContaining({
            accountId: 12,
            ruleExpression: 'salary',
        }));

        const deleteBindings = createBindings();
        installDialogRefs(deleteBindings);
        mockAccountsStore.getAccount.mockResolvedValueOnce(createAccount({ id: '13', name: 'Old bank' }));
        const deletePending = deleteBindings.open({
            id: '13',
            currentAccount: createAccount({ id: '13', name: 'Old bank' }),
        });
        await flushAsync();
        deleteBindings.selectedAccountRuleDraft.value.ruleExpression = '   ';
        deleteBindings.primaryAccountRuleId.value = 44;
        mockAccountsStore.saveAccount.mockResolvedValueOnce(createAccount({ id: '13', name: 'Old bank' }));
        await deleteBindings.save();
        await expect(deletePending).resolves.toMatchObject({ id: '13' });
        expect(mockServices.deleteAccountRule).toHaveBeenCalledWith(44);
        expect(deleteBindings.primaryAccountRuleId.value).toBeNull();

        await deleteBindings.syncSelectedAccountRule(13);
        expect(mockServices.deleteAccountRule).toHaveBeenCalledTimes(1);
    });

    test('covers rule responses without IDs and failed API envelopes', async () => {
        const bindings = createBindings();
        bindings.selectedAccountRuleDraft.value.ruleExpression = 'merchant';
        mockServices.createAccountRule.mockResolvedValueOnce(successResponse({ id: 88 }));
        await bindings.syncSelectedAccountRule(10);
        expect(bindings.primaryAccountRuleId.value).toBe(88);
        bindings.primaryAccountRuleId.value = null;
        mockServices.createAccountRule.mockResolvedValueOnce(successResponse({}));
        await bindings.syncSelectedAccountRule(10);
        expect(bindings.primaryAccountRuleId.value).toBeNull();
        mockServices.createAccountRule.mockResolvedValueOnce(successResponse({ id: null }));
        await bindings.syncSelectedAccountRule(10);
        expect(bindings.primaryAccountRuleId.value).toBeNull();

        bindings.primaryAccountRuleId.value = 9;
        mockServices.updateAccountRule.mockResolvedValueOnce({ data: { success: false, result: null } });
        await expect(bindings.syncSelectedAccountRule(10)).rejects.toThrow('Failed to save rule');

        bindings.selectedAccountRuleDraft.value.ruleExpression = '';
        mockServices.deleteAccountRule.mockResolvedValueOnce({ data: { success: false, result: null } });
        await expect(bindings.syncSelectedAccountRule(10)).rejects.toThrow('Failed to delete rule');
    });

    test('reports unprocessed save failures, suppresses processed ones, and skips failed rule loading', async () => {
        const failedBindings = createBindings();
        const failedRefs = installDialogRefs(failedBindings);
        const failedPending = failedBindings.open();
        failedBindings.account.value.name = 'Fails';
        mockAccountsStore.saveAccount.mockRejectedValueOnce(new Error('save exploded'));
        await failedBindings.save();
        expect(failedRefs.snackbar.showError).toHaveBeenCalledWith('save exploded');
        expect(failedBindings.submitting.value).toBe(false);
        expect(failedBindings.showState.value).toBe(true);
        expect(await settleCancelled(failedBindings, failedPending)).toBeUndefined();

        const objectBindings = createBindings();
        const objectRefs = installDialogRefs(objectBindings);
        const objectPending = objectBindings.open();
        objectBindings.account.value.name = 'Fails object';
        mockAccountsStore.saveAccount.mockRejectedValueOnce({ message: 'object failure' });
        await objectBindings.save();
        expect(objectRefs.snackbar.showError).toHaveBeenCalledWith('object failure');
        expect(await settleCancelled(objectBindings, objectPending)).toBeUndefined();

        const fallbackBindings = createBindings();
        const fallbackRefs = installDialogRefs(fallbackBindings);
        const fallbackPending = fallbackBindings.open();
        fallbackBindings.account.value.name = 'Fails null';
        mockAccountsStore.saveAccount.mockRejectedValueOnce(null);
        await fallbackBindings.save();
        expect(fallbackRefs.snackbar.showError).toHaveBeenCalledWith('Unable to save account');
        expect(await settleCancelled(fallbackBindings, fallbackPending)).toBeUndefined();

        const processedBindings = createBindings();
        const processedRefs = installDialogRefs(processedBindings);
        const processedPending = processedBindings.open();
        processedBindings.account.value.name = 'Processed';
        mockAccountsStore.saveAccount.mockRejectedValueOnce({ processed: true });
        await processedBindings.save();
        expect(processedRefs.snackbar.showError).not.toHaveBeenCalled();
        expect(await settleCancelled(processedBindings, processedPending)).toBeUndefined();

        const skipBindings = createBindings();
        installDialogRefs(skipBindings);
        const skipPending = skipBindings.open({
            id: '15',
            currentAccount: createAccount({ id: '15', name: 'Skip rules' }),
        });
        await flushAsync();
        skipBindings.accountRuleLoadFailed.value = true;
        skipBindings.selectedAccountRuleDraft.value.ruleExpression = 'should not sync';
        mockServices.createAccountRule.mockClear();
        mockAccountsStore.saveAccount.mockResolvedValueOnce(createAccount({ id: '15', name: 'Skip rules' }));
        await skipBindings.save();
        await expect(skipPending).resolves.toMatchObject({ id: '15' });
        expect(mockServices.createAccountRule).not.toHaveBeenCalled();
    });

    test('removes selected sub-accounts after confirmation and maintains selection bounds', async () => {
        const bindings = createBindings();
        const { confirm } = installDialogRefs(bindings);
        const first = createAccount({ id: '11', name: 'First' });
        const second = createAccount({ id: '12', name: 'Second' });
        bindings.subAccounts.value = [first, second];
        bindings.currentAccountIndex.value = 1;
        confirm.open.mockResolvedValue(undefined);

        bindings.removeSubAccount(bindings.subAccounts.value[1]);
        await flushAsync();
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to remove this sub-account?');
        expect(bindings.subAccounts.value).toStrictEqual([first]);
        expect(bindings.currentAccountIndex.value).toBe(0);

        bindings.removeSubAccount(createAccount({ id: 'missing' }));
        await flushAsync();
        expect(bindings.subAccounts.value).toStrictEqual([first]);
        bindings.currentAccountIndex.value = -1;
        bindings.removeSubAccount(bindings.subAccounts.value[0]);
        await flushAsync();
        expect(bindings.subAccounts.value).toStrictEqual([]);
        expect(bindings.currentAccountIndex.value).toBe(-1);

        const noRefBindings = createBindings();
        noRefBindings.removeSubAccount(first);
    });
});

describe('desktop account EditDialog production-loaded keyboard and template behavior', () => {
    test('forwards errors and manages keyboard shortcuts plus listener lifecycle', async () => {
        const bindings = createBindings();
        const { snackbar } = installDialogRefs(bindings);
        mockMountedCallback?.();
        expect(globalThis.window.addEventListener).toHaveBeenCalledWith('keydown', bindings.onKeydown);

        bindings.onShowDateTimeError('bad balance time');
        expect(snackbar.showError).toHaveBeenCalledWith('bad balance time');
        const preventDefault = jest.fn();
        bindings.onKeydown({ key: 'Enter', target: {}, preventDefault } as any);
        expect(preventDefault).not.toHaveBeenCalled();

        const pending = bindings.open();
        bindings.onKeydown({
            key: 'Enter',
            target: new (globalThis.HTMLInputElement as any)(),
            preventDefault,
        } as any);
        bindings.onKeydown({
            key: 'Backspace',
            target: new (globalThis.HTMLTextAreaElement as any)(),
            preventDefault,
        } as any);
        expect(preventDefault).not.toHaveBeenCalled();

        bindings.onKeydown({ key: 'Escape', target: {}, preventDefault } as any);
        expect(preventDefault).not.toHaveBeenCalled();
        bindings.onKeydown({ key: 'Enter', target: {}, preventDefault } as any);
        await flushAsync();
        expect(snackbar.showMessage).toHaveBeenCalledWith('Account name cannot be blank');
        expect(preventDefault).toHaveBeenCalledTimes(1);
        bindings.onKeydown({ key: 'Backspace', target: {}, preventDefault } as any);
        expect(preventDefault).toHaveBeenCalledTimes(2);
        expect(await pending.then((value: unknown) => value, (error: unknown) => error)).toBeUndefined();

        mockUnmountedCallback?.();
        expect(globalThis.window.removeEventListener).toHaveBeenCalledWith('keydown', bindings.onKeydown);
    });

    test('renders single, liability, and multi-account states and invokes generated handlers', async () => {
        const single = await renderDialog(bindings => {
            bindings.showState.value = true;
            bindings.account.value = createAccount({
                name: '',
                category: 2,
                type: mockAccountType.SingleAccount.type,
                currency: 'CNY',
                balanceCents: 12_345,
                liability: true,
            });
        });
        expect(single.html).toContain('Add Account');
        expect(single.html).toContain('Account Outstanding Balance');
        expect(single.html).toContain('Account name cannot be blank');

        const multi = await renderDialog(bindings => {
            bindings.showState.value = true;
            bindings.loading.value = true;
            bindings.submitting.value = true;
            bindings.editAccountId.value = '10';
            bindings.account.value = createAccount({
                id: '10',
                name: 'Parent',
                category: 1,
                type: mockAccountType.MultiSubAccounts.type,
                icon: 'bank',
                color: '#abcdef',
                currency: '',
            });
            bindings.subAccounts.value = [createAccount({
                id: '11',
                name: 'USD child',
                currency: 'USD',
                balanceCents: 9_876,
                icon: 'bank',
                color: '#abcdef',
                visible: false,
            })];
            bindings.currentAccountIndex.value = 0;
            bindings.accountRuleLoading.value = true;
            bindings.accountRuleLoadFailed.value = true;
        });
        expect(multi.html).toContain('Edit Account');
        expect(multi.html).toContain('Main Account');
        expect(multi.html).toContain('Sub Account #1');
        expect(multi.html).toContain('Account Matching');
        expect(multi.html).toContain('Failed to load account rules');
        expect(multi.html).toContain('Visible');

        const modelEvents = mockTemplateEvents.filter(event => event.name === 'onUpdate:modelValue');
        expect(modelEvents.length).toBeGreaterThanOrEqual(12);
        for (const event of modelEvents) {
            try {
                event.handler('template-update');
            } catch {
                // Different generated v-model wrappers intentionally expect different value shapes.
            }
        }
        for (const event of mockTemplateEvents.filter(item => item.name === 'onError')) {
            event.handler('template error');
        }
        await flushAsync();
    });
});
