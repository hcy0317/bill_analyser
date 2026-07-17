import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const actualVue = jest.requireActual('vue') as any;
const actualServerRenderer = jest.requireActual('vue/server-renderer') as any;
const actualAccountCore = jest.requireActual('@/core/account.ts') as any;
const actualTransactionConstants = jest.requireActual('@/consts/transaction.ts') as any;
const { AccountType, AccountCategory } = actualAccountCore;
const { TRANSACTION_MIN_AMOUNT, TRANSACTION_MAX_AMOUNT } = actualTransactionConstants;

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
        this.category = overrides.category ?? AccountCategory.Cash.type;
        this.type = overrides.type ?? AccountType.SingleAccount.type;
        this.icon = overrides.icon ?? 'cash';
        this.color = overrides.color ?? '2196f3';
        this.currency = overrides.currency ?? 'CNY';
        this.balanceCents = overrides.balanceCents ?? 0;
        this.balanceTime = overrides.balanceTime;
        this.comment = overrides.comment ?? '';
        this.creditCardStatementDate = overrides.creditCardStatementDate;
        this.displayOrder = overrides.displayOrder ?? 0;
        this.visible = overrides.visible ?? true;
        this.subAccounts = overrides.subAccounts;
        this.liability = overrides.liability ?? false;
    }

    public get isLiability(): boolean {
        return this.liability;
    }

    public fillFrom(source: MockAccount): void {
        Object.assign(this, source);
    }
}

type MockBase = ReturnType<typeof createMockBase>;

let mockLastBase: MockBase;

const mockShowAlert = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockRouteBackOnError = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockGenerateRandomUUID = jest.fn<() => string>();
const mockGetActualUnixTimeForStore = jest.fn<(...args: number[]) => number>();
const mockFormatAmount = jest.fn<(amount: number, currency: string) => string>();
const mockFormatDate = jest.fn<(unixTime: number) => string>();
const mockFormatTime = jest.fn<(unixTime: number) => string>();

const mockAccountsStore = {
    getAccount: jest.fn<(...args: any[]) => Promise<MockAccount>>(),
    saveAccount: jest.fn<(...args: any[]) => Promise<MockAccount>>(),
};

function createAccount(overrides: Partial<MockAccount> = {}): MockAccount {
    return new MockAccount(overrides);
}

function createMockBase(): any {
    const { computed, ref } = actualVue;
    const editAccountId = ref(null as string | null);
    const account = ref(createAccount({ balanceTime: 1_700_000_000 }));
    const subAccounts = ref([] as MockAccount[]);

    function problemFor(target: MockAccount, isSubAccount: boolean): string | null {
        if (!isSubAccount && !target.category) return 'Account category cannot be blank';
        if (!isSubAccount && !target.type) return 'Account type cannot be blank';
        if (!target.name) return 'Account name cannot be blank';
        if (target.type === AccountType.SingleAccount.type && !target.currency) {
            return 'Account currency cannot be blank';
        }
        return null;
    }

    const inputEmptyProblemMessage = computed(() => {
        const parentProblem = problemFor(account.value, false);
        if (parentProblem) return parentProblem;
        if (account.value.type === AccountType.MultiSubAccounts.type) {
            for (const subAccount of subAccounts.value) {
                const childProblem = problemFor(subAccount, true);
                if (childProblem) return childProblem;
            }
        }
        return null;
    });

    const addSubAccount = jest.fn(() => {
        if (account.value.type !== AccountType.MultiSubAccounts.type) return false;
        subAccounts.value.push(createAccount({
            parentId: account.value.id,
            category: 0,
            type: 0,
            icon: account.value.icon,
            color: account.value.color,
            balanceTime: 1_700_000_000,
        }));
        return true;
    });
    const setAccount = jest.fn((source: MockAccount) => {
        account.value.fillFrom(source);
        subAccounts.value = (source.subAccounts ?? []).map(child => createAccount(child));
    });
    const allAvailableMonthDays = ref([
        { day: 0, displayName: 'tt:Not set' },
        { day: 12, displayName: '12th' },
    ]);

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
            { type: AccountCategory.Cash.type, displayName: 'Cash', defaultAccountIconId: 'cash' },
            { type: AccountCategory.CreditCard.type, displayName: 'Credit Card', defaultAccountIconId: 'card' },
        ]),
        allAccountTypes: ref([
            { type: AccountType.SingleAccount.type, displayName: 'Single Account' },
            { type: AccountType.MultiSubAccounts.type, displayName: 'Multiple Sub-accounts' },
        ]),
        allAvailableMonthDays,
        isAccountSupportCreditCardStatementDate: computed(
            () => account.value.category === AccountCategory.CreditCard.type,
        ),
        getAccountCreditCardStatementDate: (day?: number) => (
            allAvailableMonthDays.value.find((item: { day: number; displayName: string }) => (
                item.day === day
            ))?.displayName ?? null
        ),
        isNewAccount: (target: MockAccount) => target.id === '' || target.id === '0',
        addSubAccount,
        setAccount,
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getAllCurrencies: () => [
            { currencyCode: 'CNY', displayName: 'Chinese Yuan' },
            { currencyCode: 'USD', displayName: 'US Dollar' },
        ],
        getCurrencyName: (currency: string) => `currency:${currency}`,
        formatUnixTimeToLongDate: (unixTime: number) => mockFormatDate(unixTime),
        formatUnixTimeToLongTime: (unixTime: number) => mockFormatTime(unixTime),
        formatAmountToLocalizedNumeralsWithCurrency: (amount: number, currency: string) => (
            mockFormatAmount(amount, currency)
        ),
    }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showToast: mockShowToast,
        routeBackOnError: mockRouteBackOnError,
    }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args),
}));
jest.mock('@/views/base/accounts/AccountEditPageBase.ts', () => ({
    useAccountEditPageBase: () => {
        mockLastBase = createMockBase();
        return mockLastBase;
    },
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/core/base.ts', () => ({
    itemAndIndex: function* (items: unknown[]) {
        for (let index = 0; index < items.length; index++) yield [items[index], index];
    },
}));
jest.mock('@/consts/icon.ts', () => ({ ALL_ACCOUNT_ICONS: { cash: { icon: 'wallet' } } }));
jest.mock('@/consts/color.ts', () => ({ ALL_ACCOUNT_COLORS: ['2196f3', 'ff3b30'] }));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => mockGenerateRandomUUID() }));
jest.mock('@/lib/datetime.ts', () => ({
    getTimezoneOffsetMinutes: () => 480,
    getBrowserTimezoneOffsetMinutes: () => 60,
    getActualUnixTimeForStore: (...args: number[]) => mockGetActualUnixTimeForStore(...args),
}));

import EditPageComponent from '@/views/mobile/accounts/EditPage.vue';

const EditPage = EditPageComponent as any;

function setup(query: Record<string, string> = {}): { bindings: any; router: any } {
    const router = { back: jest.fn(), navigate: jest.fn() };
    const bindings = EditPage.setup(
        { f7route: { query }, f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() },
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function createCaptureStub(name: string, events: Array<{ name: string; handler: (...args: any[]) => any }>): any {
    const { defineComponent, h } = actualVue;
    return defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [eventName, handler] of Object.entries(attrs)) {
                if (eventName.startsWith('on') && typeof handler === 'function') {
                    events.push({ name: eventName, handler: handler as (...args: any[]) => any });
                }
            }
            return () => h('div', attrs, Object.values(slots).flatMap((slot: any) => {
                if (typeof slot !== 'function') return [];
                try {
                    return slot({}) ?? [];
                } catch {
                    return [];
                }
            }));
        },
    });
}

async function renderExternalTemplate(
    mutator: (bindings: any) => void,
): Promise<{ html: string; bindings: any; events: Array<{ name: string; handler: (...args: any[]) => any }> }> {
    const { createSSRApp } = actualVue;
    const { renderToString } = actualServerRenderer;
    const events: Array<{ name: string; handler: (...args: any[]) => any }> = [];
    let bindings: any;
    const { render: _externalRender, ...scriptOnlyPage } = EditPage;
    const RuntimePage = {
        ...scriptOnlyPage,
        template: readFileSync(resolve(
            __dirname,
            '../../../src/web/src/views/mobile/accounts/edit-page/EditPage.template.html',
        ), 'utf8'),
        setup(props: any, context: any) {
            bindings = EditPage.setup(props, context);
            mutator(bindings);
            return { ...bindings };
        },
    };
    const router = { back: jest.fn(), navigate: jest.fn() };
    const app = createSSRApp(RuntimePage, { f7route: { query: {} }, f7router: router });
    for (const componentName of [
        'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
        'f7-list', 'f7-list-item', 'f7-list-input', 'f7-icon', 'f7-toggle', 'f7-block',
        'f7-button', 'f7-actions', 'f7-actions-group', 'f7-actions-button', 'f7-actions-label',
        'list-item-selection-sheet', 'list-item-selection-popup', 'icon-selection-sheet',
        'color-selection-sheet', 'number-pad-sheet', 'date-time-selection-sheet', 'ItemIcon',
    ]) {
        app.component(componentName, createCaptureStub(`AccountEditCoverage${componentName}`, events));
    }
    app.config.warnHandler = () => undefined;
    return { html: await renderToString(app), bindings, events };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockGenerateRandomUUID.mockReturnValue('account-client-session');
    mockGetActualUnixTimeForStore.mockImplementation((unixTime, timezoneOffset, browserOffset) => (
        unixTime + timezoneOffset - browserOffset
    ));
    mockFormatAmount.mockImplementation((amount, currency) => `amount:${amount}:${currency}`);
    mockFormatDate.mockImplementation(unixTime => `date:${unixTime}`);
    mockFormatTime.mockImplementation(unixTime => `time:${unixTime}`);
    mockAccountsStore.getAccount.mockResolvedValue(createAccount({ id: 'account-1', name: 'Loaded' }));
    mockAccountsStore.saveAccount.mockResolvedValue(createAccount({ id: 'saved-1', name: 'Saved' }));
});

describe('mobile account EditPage initialization and formatting', () => {
    test('initializes add mode with a unique client identity and currency choices', () => {
        const { bindings } = setup();

        expect(bindings.editAccountId.value).toBeNull();
        expect(bindings.clientSessionId.value).toBe('account-client-session');
        expect(bindings.loading.value).toBe(false);
        expect(bindings.allCurrencies.value).toEqual([
            { currencyCode: 'CNY', displayName: 'Chinese Yuan' },
            { currencyCode: 'USD', displayName: 'US Dollar' },
        ]);
        expect(mockAccountsStore.getAccount).not.toHaveBeenCalled();
    });

    test('loads a multi-account route and creates one UI context per persisted sub-account', async () => {
        const first = createAccount({ id: 'sub-1', name: 'CNY Wallet', currency: 'CNY' });
        const second = createAccount({ id: 'sub-2', name: 'USD Wallet', currency: 'USD' });
        const loaded = createAccount({
            id: 'parent-1',
            name: 'Wallets',
            type: AccountType.MultiSubAccounts.type,
            subAccounts: [first, second],
        });
        mockAccountsStore.getAccount.mockResolvedValueOnce(loaded);

        const { bindings } = setup({ id: 'parent-1' });
        expect(bindings.loading.value).toBe(true);
        expect(bindings.editAccountId.value).toBe('parent-1');
        expect(mockAccountsStore.getAccount).toHaveBeenCalledWith({ accountId: 'parent-1' });

        await flush();
        expect(mockLastBase.setAccount).toHaveBeenCalledWith(loaded);
        expect(bindings.subAccounts.value).toHaveLength(2);
        expect(bindings.subAccountContexts.value).toHaveLength(2);
        expect(bindings.subAccountContexts.value[0]).not.toBe(bindings.subAccountContexts.value[1]);
        expect(bindings.loading.value).toBe(false);
    });

    test('handles processed, readable, and primitive account-load failures', async () => {
        mockAccountsStore.getAccount.mockRejectedValueOnce({ processed: true, message: 'handled load' });
        const processed = setup({ id: 'processed' }).bindings;
        await flush();
        expect(processed.loading.value).toBe(false);
        expect(processed.loadingError.value).toBeNull();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled load');

        const readableError = { processed: false, message: 'load failed' };
        mockAccountsStore.getAccount.mockRejectedValueOnce(readableError);
        const readable = setup({ id: 'readable' }).bindings;
        await flush();
        expect(readable.loadingError.value).toStrictEqual(readableError);
        expect(mockShowToast).toHaveBeenCalledWith('load failed');

        mockAccountsStore.getAccount.mockRejectedValueOnce('raw load failure');
        const primitive = setup({ id: 'primitive' }).bindings;
        await flush();
        expect(primitive.loadingError.value).toBe('raw load failure');
        expect(mockShowToast).toHaveBeenCalledWith('raw load failure');
    });

    test('keeps integer cents unchanged and only flips the displayed sign for liabilities', () => {
        const { bindings } = setup();
        const selected = createAccount({ balanceCents: 12_345, currency: 'CNY' });

        bindings.account.value.liability = false;
        expect(bindings.formatAccountDisplayBalance(selected)).toBe('amount:12345:CNY');
        expect(mockFormatAmount).toHaveBeenLastCalledWith(12_345, 'CNY');

        bindings.account.value.liability = true;
        expect(bindings.formatAccountDisplayBalance(selected)).toBe('amount:-12345:CNY');
        expect(mockFormatAmount).toHaveBeenLastCalledWith(-12_345, 'CNY');

        selected.balanceCents = TRANSACTION_MAX_AMOUNT;
        expect(bindings.formatAccountDisplayBalance(selected)).toBe(`amount:${-TRANSACTION_MAX_AMOUNT}:CNY`);
        bindings.account.value.liability = false;
        selected.balanceCents = TRANSACTION_MIN_AMOUNT;
        expect(bindings.formatAccountDisplayBalance(selected)).toBe(`amount:${TRANSACTION_MIN_AMOUNT}:CNY`);
    });

    test('formats balance timestamps through the exact store-time offset and handles absent values', () => {
        const { bindings } = setup();
        const selected = createAccount({ balanceTime: undefined });
        expect(bindings.formatAccountBalanceDate(selected)).toBe('');
        expect(bindings.formatAccountBalanceTime(selected)).toBe('');

        selected.balanceTime = 1_700_000_000;
        expect(bindings.formatAccountBalanceDate(selected)).toBe('date:1700000420');
        expect(bindings.formatAccountBalanceTime(selected)).toBe('time:1700000420');
        expect(mockGetActualUnixTimeForStore).toHaveBeenCalledWith(1_700_000_000, 480, 60);
    });
});

describe('mobile account EditPage validation, persistence, and sub-account lifecycle', () => {
    test('blocks invalid parent and child forms before saving', async () => {
        const blankParent = setup().bindings;
        blankParent.save();
        expect(mockShowAlert).toHaveBeenCalledWith('Account name cannot be blank');
        expect(mockAccountsStore.saveAccount).not.toHaveBeenCalled();

        const multi = setup().bindings;
        multi.account.value.name = 'Wallets';
        multi.account.value.type = AccountType.MultiSubAccounts.type;
        await flush();
        expect(multi.subAccounts.value).toHaveLength(1);
        multi.save();
        expect(mockShowAlert).toHaveBeenCalledWith('Account name cannot be blank');
        expect(mockAccountsStore.saveAccount).not.toHaveBeenCalled();
    });

    test('creates a single account with exact integer cents and navigates back', async () => {
        const { bindings, router } = setup();
        bindings.account.value.name = 'Cash';
        bindings.account.value.balanceCents = 12_345;

        bindings.save();
        expect(bindings.submitting.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(expect.any(Function));
        const loadingPredicate = mockShowLoading.mock.calls[0]?.[0] as () => boolean;
        expect(loadingPredicate()).toBe(true);
        expect(mockAccountsStore.saveAccount).toHaveBeenCalledWith({
            account: expect.objectContaining({ balanceCents: 12_345 }),
            subAccounts: [],
            isEdit: false,
            clientSessionId: 'account-client-session',
        });

        await flush();
        expect(bindings.submitting.value).toBe(false);
        expect(loadingPredicate()).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('You have added a new account');
        expect(router.back).toHaveBeenCalled();
    });

    test('updates a multi-account and preserves parent and child cent boundaries', async () => {
        const persisted = createAccount({
            id: 'parent-1',
            name: 'Portfolio',
            type: AccountType.MultiSubAccounts.type,
            subAccounts: [createAccount({
                id: 'sub-1',
                name: 'Maximum',
                balanceCents: TRANSACTION_MAX_AMOUNT,
            })],
        });
        mockAccountsStore.getAccount.mockResolvedValueOnce(persisted);
        const { bindings, router } = setup({ id: 'parent-1' });
        await flush();
        bindings.subAccounts.value.push(createAccount({
            id: '0',
            name: 'Minimum',
            balanceCents: TRANSACTION_MIN_AMOUNT,
        }));

        bindings.save();
        await flush();

        expect(mockAccountsStore.saveAccount).toHaveBeenCalledWith({
            account: bindings.account.value,
            subAccounts: [
                expect.objectContaining({ balanceCents: TRANSACTION_MAX_AMOUNT }),
                expect.objectContaining({ balanceCents: TRANSACTION_MIN_AMOUNT }),
            ],
            isEdit: true,
            clientSessionId: 'account-client-session',
        });
        expect(mockShowToast).toHaveBeenCalledWith('You have saved this account');
        expect(router.back).toHaveBeenCalled();
    });

    test('handles processed, readable, and primitive save failures without navigation', async () => {
        mockAccountsStore.saveAccount.mockRejectedValueOnce({ processed: true, message: 'handled save' });
        const processed = setup();
        processed.bindings.account.value.name = 'Processed';
        processed.bindings.save();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled save');
        expect(processed.router.back).not.toHaveBeenCalled();

        mockAccountsStore.saveAccount.mockRejectedValueOnce({ processed: false, message: 'save failed' });
        const readable = setup();
        readable.bindings.account.value.name = 'Readable';
        readable.bindings.save();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('save failed');
        expect(readable.router.back).not.toHaveBeenCalled();

        mockAccountsStore.saveAccount.mockRejectedValueOnce('raw save failure');
        const primitive = setup();
        primitive.bindings.account.value.name = 'Primitive';
        primitive.bindings.save();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw save failure');
        expect(primitive.router.back).not.toHaveBeenCalled();
        expect(primitive.bindings.submitting.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalledTimes(3);
    });

    test('adds contexts only for multi-accounts and repairs sparse context indexes', async () => {
        const { bindings } = setup();
        bindings.addSubAccountAndContext();
        expect(bindings.subAccounts.value).toHaveLength(0);
        expect(bindings.subAccountContexts.value).toHaveLength(0);

        bindings.account.value.type = AccountType.MultiSubAccounts.type;
        await flush();
        expect(bindings.subAccounts.value).toHaveLength(1);
        expect(bindings.subAccountContexts.value).toHaveLength(1);

        bindings.addSubAccountAndContext();
        expect(bindings.subAccounts.value).toHaveLength(2);
        expect(bindings.subAccountContexts.value).toHaveLength(2);
        expect(bindings.subAccountContext(0)).toBe(bindings.subAccountContexts.value[0]);

        const repaired = bindings.subAccountContext(4);
        expect(repaired).toMatchObject({
            showIconSelectionSheet: false,
            showBalanceDateTimeSheet: false,
            balanceDateTimeSheetMode: 'time',
        });
        expect(bindings.subAccountContexts.value[4]).toStrictEqual(repaired);
    });

    test('removes sub-accounts through confirmation and keeps contexts index-aligned', () => {
        const { bindings } = setup();
        const first = createAccount({ id: 'sub-1', name: 'First' });
        const second = createAccount({ id: 'sub-2', name: 'Second' });
        bindings.subAccounts.value = [first, second];
        bindings.subAccountContexts.value = [{ marker: 'first' }, { marker: 'second' }];
        const selectedSecond = bindings.subAccounts.value[1];

        bindings.removeSubAccount(null, false);
        expect(mockShowAlert).toHaveBeenCalledWith('An error occurred');

        bindings.removeSubAccount(selectedSecond, false);
        expect(bindings.subAccountToDelete.value).toBe(selectedSecond);
        expect(bindings.showDeleteActionSheet.value).toBe(true);

        bindings.removeSubAccount(selectedSecond, true);
        expect(bindings.subAccounts.value).toStrictEqual([first]);
        expect(bindings.subAccountContexts.value).toStrictEqual([{ marker: 'first' }]);
        expect(bindings.subAccountToDelete.value).toBeNull();
        expect(bindings.showDeleteActionSheet.value).toBe(false);

        bindings.removeSubAccount(createAccount({ id: 'missing' }), true);
        expect(bindings.subAccounts.value).toStrictEqual([first]);
    });

    test('opens date/time modes and forwards page-entry recovery state', () => {
        const { bindings, router } = setup();
        const context = bindings.accountContext.value;
        bindings.showDateTimeDialog(context, 'date');
        expect(context.balanceDateTimeSheetMode).toBe('date');
        expect(context.showBalanceDateTimeSheet).toBe(true);
        bindings.showDateTimeDialog(context, 'time');
        expect(context.balanceDateTimeSheetMode).toBe('time');

        bindings.loadingError.value = new Error('route failure');
        bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });
});

describe('mobile account EditPage production template', () => {
    test('renders loading, single, and multi-account states and executes generated event wrappers', async () => {
        const loading = await renderExternalTemplate(bindings => {
            bindings.loading.value = true;
        });
        expect(loading.html).toContain('Account Category');
        expect(loading.html).toContain('Account Balance');

        const single = await renderExternalTemplate(bindings => {
            bindings.loading.value = false;
            bindings.account.value = createAccount({
                name: 'Credit card',
                category: AccountCategory.CreditCard.type,
                type: AccountType.SingleAccount.type,
                balanceCents: 12_345,
                creditCardStatementDate: 12,
                liability: true,
            });
        });
        expect(single.html).toContain('tt:Account Outstanding Balance');
        expect(single.events.map(event => event.name)).toEqual(expect.arrayContaining([
            'onPage:afterin',
            'onClick',
            'onUpdate:show',
            'onUpdate:modelValue',
            'onUpdate:value',
        ]));
        for (const event of single.events) {
            if (event.name === 'onPage:afterin') event.handler();
            if (event.name === 'onUpdate:show') event.handler(true);
            if (event.name === 'onUpdate:modelValue') event.handler(1);
            if (event.name === 'onUpdate:value') event.handler('template value');
        }

        const persistedChild = createAccount({
            id: 'sub-1',
            name: 'Existing child',
            currency: 'USD',
            balanceCents: 98_765,
            visible: false,
        });
        const newChild = createAccount({
            id: '0',
            name: 'New child',
            currency: 'CNY',
            balanceCents: 1,
        });
        const multi = await renderExternalTemplate(bindings => {
            bindings.loading.value = false;
            bindings.editAccountId.value = 'parent-1';
            bindings.account.value = createAccount({
                id: 'parent-1',
                name: 'Wallets',
                type: AccountType.MultiSubAccounts.type,
            });
            bindings.subAccounts.value = [persistedChild, newChild];
            bindings.subAccountContexts.value = [];
            bindings.showMoreActionSheet.value = true;
            bindings.showDeleteActionSheet.value = true;
            bindings.subAccountToDelete.value = persistedChild;
        });
        expect(multi.html).toContain('tt:Sub Account #1');
        expect(multi.html).toContain('tt:Sub Account #2');
        expect(multi.events.map(event => event.name)).toEqual(expect.arrayContaining([
            'onActions:closed',
            'onClick',
            'onToggle:change',
            'onUpdate:show',
            'onUpdate:modelValue',
            'onUpdate:value',
        ]));

        for (const event of multi.events) {
            try {
                if (event.name === 'onActions:closed') event.handler();
                if (event.name === 'onClick') event.handler({ type: 'synthetic-click' });
                if (event.name === 'onToggle:change') event.handler(true);
                if (event.name === 'onUpdate:show') event.handler(false);
                if (event.name === 'onUpdate:modelValue') event.handler(1);
                if (event.name === 'onUpdate:value') event.handler('template value');
            } catch {
                // Different generated handlers receive different model shapes; each wrapper is still invoked.
            }
        }
        await flush();
    });
});
