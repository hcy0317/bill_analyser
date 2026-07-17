import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const transactionType = {
    ModifyBalance: 1,
    Income: 2,
    Expense: 3,
    Transfer: 4,
    Investment: 5
} as const;

const categoryType = {
    Expense: 3,
    Income: 2,
    Transfer: 4,
    Investment: 5
} as const;

class MockTransaction {
    public type: number = transactionType.Expense;
    public time = 1_800_000_000;
    public timeZone = 'Asia/Shanghai';
    public utcOffset = 480;
    public expenseCategoryId = '';
    public incomeCategoryId = '';
    public transferCategoryId = '';
    public investmentCategoryId = '';
    public sourceAccountId = '';
    public destinationAccountId = '';
    public sourceAmountCents = 0;
    public destinationAmountCents = 0;
    public hideAmount = false;
    public tagIds: string[] = [];
    public comment = '';

    public static createNewTransaction(type: number, time: number, timeZone: string, utcOffset: number): MockTransaction {
        const transaction = new MockTransaction();
        transaction.type = type;
        transaction.time = time;
        transaction.timeZone = timeZone;
        transaction.utcOffset = utcOffset;
        return transaction;
    }

    public getCategoryId(): string {
        if (this.type === transactionType.Expense) return this.expenseCategoryId;
        if (this.type === transactionType.Income) return this.incomeCategoryId;
        if (this.type === transactionType.Transfer) return this.transferCategoryId;
        if (this.type === transactionType.Investment) return this.investmentCategoryId;
        return '';
    }

    public setCategoryId(categoryId: string): void {
        if (this.type === transactionType.Expense) this.expenseCategoryId = categoryId;
        if (this.type === transactionType.Income) this.incomeCategoryId = categoryId;
        if (this.type === transactionType.Transfer) this.transferCategoryId = categoryId;
        if (this.type === transactionType.Investment) this.investmentCategoryId = categoryId;
    }
}

const expenseCategory = { id: 'food', name: 'Food' };
const incomeCategory = { id: 'salary', name: 'Salary' };
const transferCategory = { id: 'move', name: 'Move' };
const investmentCategory = { id: 'fund', name: 'Fund' };

const settingsStore = {
    appSettings: {
        timeZone: 'Asia/Shanghai',
        showAccountBalance: true
    }
};

const userStore = {
    currentUserDefaultCurrency: 'CNY'
};

const accountsStore = {
    allVisiblePlainAccounts: [
        { id: 'wallet', name: 'Wallet', currency: 'CNY' },
        { id: 'bank', name: 'Bank', currency: 'USD' },
        { id: 'broker', name: 'Broker', currency: 'EUR' }
    ],
    allAccountsMap: {
        wallet: { id: 'wallet', currency: 'CNY' },
        bank: { id: 'bank', currency: 'USD' },
        broker: { id: 'broker', currency: 'EUR' }
    } as Record<string, { id: string; currency: string }>,
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>()
};

const categoriesStore = {
    allTransactionCategories: {
        [categoryType.Expense]: [expenseCategory],
        [categoryType.Income]: [incomeCategory],
        [categoryType.Transfer]: [transferCategory],
        [categoryType.Investment]: [investmentCategory]
    } as Record<number, Array<{ id: string; name: string }>>,
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>()
};

const tagsStore = {
    allVisibleTags: [
        { id: 'daily', name: 'Daily' },
        { id: 'work', name: 'Work' }
    ],
    loadAllTags: jest.fn<(...args: any[]) => Promise<void>>()
};

const transactionsStore = {
    saveTransactions: jest.fn<(...args: any[]) => Promise<any[]>>()
};

const categorizeAccounts = jest.fn((accounts: any[], showBalance: boolean) => [{
    id: 'cash',
    name: `Cash:${showBalance}`,
    accounts
}]);

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        nextTick: (callback?: () => void) => {
            callback?.();
            return Promise.resolve();
        },
        useTemplateRef: () => actual.ref(null)
    };
});

jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: (..._bindings: unknown[]) => undefined
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => values
            ? `${key}:${JSON.stringify(values)}`
            : key,
        getCategorizedAccountsWithDisplayBalance: (accounts: any[], showBalance: boolean) => categorizeAccounts(accounts, showBalance)
    })
}));

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => settingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => userStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => accountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => categoriesStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => tagsStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => transactionsStore }));

jest.mock('@/core/category.ts', () => ({ CategoryType: categoryType }));
jest.mock('@/core/transaction.ts', () => ({ TransactionType: transactionType }));
jest.mock('@/models/transaction.ts', () => ({ Transaction: MockTransaction }));

jest.mock('@/lib/datetime.ts', () => ({
    getCurrentUnixTime: () => 1_800_000_000,
    getTimezoneOffsetMinutes: (timezone: string) => timezone === 'Asia/Shanghai' ? 480 : 0
}));

for (const componentPath of [
    '@/components/desktop/AmountInput.vue',
    '@/components/desktop/DateTimeSelect.vue',
    '@/components/desktop/SnackBar.vue',
    '@/components/desktop/TwoColumnSelect.vue'
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: { name: 'BatchEntryCoverageStub' }
    }));
}

import BatchManualEntryDialog from '@/views/desktop/transactions/list/dialogs/BatchManualEntryDialog.vue';

function setupDialog(): any {
    const expose = jest.fn();
    const bindings = (BatchManualEntryDialog as any).setup({}, { expose });
    expect(expose).toHaveBeenCalledWith({ open: bindings.open });
    return bindings;
}

function makeRow(bindings: any, overrides: Partial<MockTransaction> = {}): any {
    const row = bindings.createTransactionRow();
    Object.assign(row.transaction, overrides);
    return row;
}

function makeValidExpense(bindings: any, overrides: Partial<MockTransaction> = {}): any {
    const row = makeRow(bindings, {
        type: transactionType.Expense,
        expenseCategoryId: 'food',
        sourceAccountId: 'wallet',
        sourceAmountCents: 1234,
        ...overrides
    });
    return row;
}

async function flushPromises(times = 5): Promise<void> {
    for (let index = 0; index < times; index++) {
        await Promise.resolve();
    }
}

function render(bindings: any): unknown {
    const { proxyRefs } = jest.requireActual('vue') as any;
    const exposed = proxyRefs(bindings);
    return (BatchManualEntryDialog as any).render(exposed, [], {}, exposed, {}, {});
}

class FakeHTMLElement {
    public attributes: Record<string, string> = {};
    public closestResult: unknown = null;
    public focused = false;
    public clicked = false;

    public closest(_selector: string): unknown {
        return this.closestResult;
    }

    public getAttribute(name: string): string | null {
        return this.attributes[name] ?? null;
    }

    public focus(): void {
        this.focused = true;
    }

    public click(): void {
        this.clicked = true;
    }
}

class FakeHTMLInputElement extends FakeHTMLElement {
    public selectionStart: number | null = 0;
    public selectionEnd: number | null = 0;
    public value = '';
    public selected = false;

    public select(): void {
        this.selected = true;
    }
}

class FakeHTMLTextAreaElement extends FakeHTMLInputElement {}

function keyboardEvent(key: string, target: unknown = null, modifiers: Record<string, boolean> = {}): any {
    return {
        key,
        target,
        altKey: false,
        ctrlKey: false,
        metaKey: false,
        preventDefault: jest.fn(),
        stopPropagation: jest.fn(),
        ...modifiers
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    settingsStore.appSettings.timeZone = 'Asia/Shanghai';
    settingsStore.appSettings.showAccountBalance = true;
    userStore.currentUserDefaultCurrency = 'CNY';
    accountsStore.allVisiblePlainAccounts = [
        { id: 'wallet', name: 'Wallet', currency: 'CNY' },
        { id: 'bank', name: 'Bank', currency: 'USD' },
        { id: 'broker', name: 'Broker', currency: 'EUR' }
    ];
    categoriesStore.allTransactionCategories = {
        [categoryType.Expense]: [expenseCategory],
        [categoryType.Income]: [incomeCategory],
        [categoryType.Transfer]: [transferCategory],
        [categoryType.Investment]: [investmentCategory]
    };
    tagsStore.allVisibleTags = [
        { id: 'daily', name: 'Daily' },
        { id: 'work', name: 'Work' }
    ];
    accountsStore.loadAllAccounts.mockResolvedValue(undefined);
    categoriesStore.loadAllCategories.mockResolvedValue(undefined);
    tagsStore.loadAllTags.mockResolvedValue(undefined);
    transactionsStore.saveTransactions.mockResolvedValue([]);

    Object.defineProperty(globalThis, 'HTMLElement', { configurable: true, value: FakeHTMLElement });
    Object.defineProperty(globalThis, 'HTMLInputElement', { configurable: true, value: FakeHTMLInputElement });
    Object.defineProperty(globalThis, 'HTMLTextAreaElement', { configurable: true, value: FakeHTMLTextAreaElement });
    Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: { querySelector: jest.fn(() => null) }
    });
    Object.defineProperty(globalThis.window, 'setTimeout', {
        configurable: true,
        value: (callback: () => void) => {
            callback();
            return 1;
        }
    });
});

describe('BatchManualEntryDialog production-loaded behavior', () => {
    test('creates initial rows from open options and exposes reactive choices', () => {
        const bindings = setupDialog();

        expect(bindings.getInitialTransactionType(transactionType.Income)).toBe(transactionType.Income);
        expect(bindings.getInitialTransactionType('4')).toBe(transactionType.Transfer);
        expect(bindings.getInitialTransactionType('invalid')).toBe(transactionType.Expense);
        expect(bindings.getInitialTransactionType(0)).toBe(transactionType.Expense);

        const row = bindings.createTransactionRow({
            time: 1_900_000_000,
            type: transactionType.Transfer,
            categoryId: 'move',
            accountId: 'wallet',
            tagIds: 'daily, work, ,'
        });
        expect(row.transaction).toMatchObject({
            time: 1_900_000_000,
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            destinationAmountCents: 0,
            tagIds: ['daily', 'work']
        });
        expect(bindings.currentCategoryIds.value[row.id]).toBe('move');
        expect(bindings.isDestinationAmountSync(row.id)).toBe(true);
        expect(bindings.transactionTypeOptions.value.map((item: any) => item.value)).toEqual([3, 2, 4, 5]);
        expect(bindings.tagOptions.value).toEqual(tagsStore.allVisibleTags);
        expect(bindings.categorizedAccountOptions.value[0].accounts).toEqual(accountsStore.allVisiblePlainAccounts);
        expect(categorizeAccounts).toHaveBeenCalledWith(accountsStore.allVisiblePlainAccounts, true);
    });

    test('duplicates, appends, copies, and removes rows without sharing mutable fields', () => {
        const bindings = setupDialog();
        bindings.reset({ accountId: 'wallet', tagIds: 'daily' });
        const first = bindings.rows.value[0];
        Object.assign(first.transaction, {
            expenseCategoryId: 'food',
            sourceAmountCents: 1234,
            destinationAmountCents: 4321,
            destinationAccountId: 'bank',
            comment: 'lunch',
            hideAmount: true
        });

        bindings.appendRow();
        expect(bindings.rows.value[1].transaction.sourceAccountId).toBe('wallet');
        bindings.duplicateRow(first);
        const duplicate = bindings.rows.value[2];
        expect(duplicate.transaction).toMatchObject({
            sourceAmountCents: 1234,
            destinationAmountCents: 4321,
            destinationAccountId: 'bank',
            comment: 'lunch',
            hideAmount: true,
            tagIds: ['daily']
        });
        duplicate.transaction.tagIds.push('work');
        expect(first.transaction.tagIds).toEqual(['daily']);

        const count = bindings.rows.value.length;
        bindings.copyPreviousRow(0);
        expect(bindings.rows.value).toHaveLength(count);
        bindings.copyPreviousRow(1);
        expect(bindings.rows.value).toHaveLength(count + 1);
        expect(bindings.rows.value[2].transaction.sourceAccountId).toBe('wallet');

        const removedId = bindings.rows.value[1].id;
        bindings.removeRow(removedId);
        expect(bindings.rows.value.some((row: any) => row.id === removedId)).toBe(false);
        expect(bindings.currentCategoryIds.value[removedId]).toBeUndefined();
        expect(bindings.destinationAmountSyncState.value[removedId]).toBeUndefined();
    });

    test('changes type, category, and cents synchronization for expense, transfer, and investment rows', () => {
        const bindings = setupDialog();
        const row = makeValidExpense(bindings, {
            incomeCategoryId: 'salary',
            transferCategoryId: 'move',
            investmentCategoryId: 'fund',
            destinationAccountId: 'bank',
            destinationAmountCents: 999
        });
        bindings.rows.value = [row];
        bindings.currentCategoryIds.value[row.id] = 'food';

        const reactiveRow = bindings.rows.value[0];
        bindings.onTransactionTypeChanged(reactiveRow);
        expect(row.transaction).toMatchObject({
            expenseCategoryId: '',
            incomeCategoryId: '',
            transferCategoryId: '',
            investmentCategoryId: '',
            destinationAccountId: '0',
            destinationAmountCents: 0
        });
        expect(bindings.currentCategoryIds.value[row.id]).toBe('');
        expect(bindings.isDestinationAmountSync(row.id)).toBe(false);

        row.transaction.type = transactionType.Transfer;
        row.transaction.sourceAmountCents = 2500;
        bindings.onTransactionTypeChanged(reactiveRow);
        expect(row.transaction.destinationAmountCents).toBe(2500);
        expect(bindings.isDestinationAmountSync(row.id)).toBe(true);
        row.transaction.sourceAmountCents = 3456;
        bindings.onSourceAmountChanged(row);
        expect(row.transaction.destinationAmountCents).toBe(3456);

        row.transaction.destinationAmountCents = 1000;
        bindings.onDestinationAmountChanged(row);
        expect(bindings.isDestinationAmountSync(row.id)).toBe(false);
        row.transaction.destinationAmountCents = 3456;
        bindings.onDestinationAmountChanged(row);
        expect(bindings.isDestinationAmountSync(row.id)).toBe(true);

        row.transaction.type = transactionType.Investment;
        row.transaction.destinationAmountCents = 777;
        bindings.onTransactionTypeChanged(row);
        expect(bindings.isDestinationAmountSync(row.id)).toBe(false);
        expect(bindings.getDestinationAccountLabel(row.transaction)).toBe('Investment Account');
        bindings.onCategoryChanged(row.transaction, 'fund');
        expect(row.transaction.investmentCategoryId).toBe('fund');

        row.transaction.type = transactionType.Expense;
        bindings.onDestinationAmountChanged(row);
        bindings.onSourceAmountChanged(row);
        expect(bindings.isDestinationAmountSync(row.id)).toBe(false);
        expect(bindings.getDestinationAccountLabel(row.transaction)).toBe('Destination Account');
    });

    test('applies common fields with destination and non-destination cleanup', () => {
        const bindings = setupDialog();
        const source = makeRow(bindings, {
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            destinationAccountId: 'bank',
            sourceAmountCents: 2500,
            destinationAmountCents: 2600,
            tagIds: ['daily']
        });
        const target = makeRow(bindings, {
            type: transactionType.Expense,
            expenseCategoryId: 'old',
            sourceAmountCents: 1250,
            destinationAmountCents: 0
        });
        bindings.applyCommonFields(target.transaction, source.transaction, target.id);
        expect(target.transaction).toMatchObject({
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            expenseCategoryId: '',
            sourceAccountId: 'wallet',
            destinationAccountId: 'bank',
            destinationAmountCents: 1250,
            tagIds: ['daily']
        });

        target.transaction.destinationAmountCents = 9999;
        bindings.applyCommonFields(target.transaction, source.transaction, target.id);
        expect(target.transaction.destinationAmountCents).toBe(9999);

        const expense = makeValidExpense(bindings, { tagIds: ['work'] });
        bindings.applyCommonFields(target.transaction, expense.transaction, target.id);
        expect(target.transaction).toMatchObject({
            type: transactionType.Expense,
            expenseCategoryId: 'food',
            destinationAccountId: '0',
            destinationAmountCents: 0,
            tagIds: ['work']
        });
        expect(bindings.isDestinationAmountSync(target.id)).toBe(false);
    });

    test('fills only empty common fields and preserves existing cents, comments, tags, and accounts', () => {
        const bindings = setupDialog();
        const source = makeRow(bindings, {
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            destinationAccountId: 'bank',
            sourceAmountCents: 5000,
            destinationAmountCents: 5100,
            tagIds: ['daily'],
            comment: 'source'
        });
        const empty = makeRow(bindings);
        bindings.fillEmptyCommonFields(empty.transaction, source.transaction, empty.id);
        expect(empty.transaction).toMatchObject({
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            destinationAccountId: 'bank',
            sourceAmountCents: 5000,
            destinationAmountCents: 5100,
            tagIds: ['daily'],
            comment: 'source'
        });

        const populated = makeRow(bindings, {
            type: transactionType.Transfer,
            transferCategoryId: 'existing',
            sourceAccountId: 'broker',
            destinationAccountId: 'wallet',
            sourceAmountCents: 7000,
            destinationAmountCents: 7100,
            tagIds: ['work'],
            comment: 'keep'
        });
        bindings.fillEmptyCommonFields(populated.transaction, source.transaction, populated.id);
        expect(populated.transaction).toMatchObject({
            transferCategoryId: 'existing',
            sourceAccountId: 'broker',
            destinationAccountId: 'wallet',
            sourceAmountCents: 7000,
            destinationAmountCents: 7100,
            tagIds: ['work'],
            comment: 'keep'
        });

        const expenseSource = makeValidExpense(bindings);
        bindings.fillEmptyCommonFields(populated.transaction, expenseSource.transaction, populated.id);
        expect(bindings.isDestinationAmountSync(populated.id)).toBe(false);
    });

    test('applies and fills fields down or across all rows, including absent source guards', () => {
        const bindings = setupDialog();
        const source = makeValidExpense(bindings, { tagIds: ['daily'], comment: 'seed' });
        const second = makeRow(bindings, { sourceAmountCents: 999 });
        const third = makeRow(bindings);
        bindings.rows.value = [source, second, third];

        bindings.applyCommonFieldsDown(0);
        expect(second.transaction.expenseCategoryId).toBe('food');
        expect(third.transaction.sourceAccountId).toBe('wallet');

        second.transaction.sourceAccountId = 'bank';
        bindings.applyCommonFieldsToAll(1);
        expect(source.transaction.sourceAccountId).toBe('bank');
        expect(third.transaction.sourceAccountId).toBe('bank');

        third.transaction.sourceAccountId = '';
        third.transaction.comment = '';
        bindings.fillEmptyFieldsDown(1);
        expect(third.transaction.sourceAccountId).toBe('bank');
        bindings.fillEmptyFieldsToAll(2);

        const snapshot = bindings.rows.value.map((row: any) => row.transaction.sourceAccountId);
        bindings.applyCommonFieldsDown(99);
        bindings.applyCommonFieldsToAll(99);
        bindings.fillEmptyFieldsDown(99);
        bindings.fillEmptyFieldsToAll(99);
        expect(bindings.rows.value.map((row: any) => row.transaction.sourceAccountId)).toEqual(snapshot);
    });

    test('validates rows, reports issues, labels states, and keeps monetary fields in cents', () => {
        const bindings = setupDialog();
        const empty = makeRow(bindings);
        expect(bindings.isRowEmpty(empty)).toBe(true);
        expect(bindings.isRowValid(empty)).toBe(false);
        expect(bindings.getRowStatusLabel(empty)).toBe('Empty Row');
        expect(bindings.getRowStatusColor(empty)).toBe('default');

        empty.transaction.comment = 'incomplete';
        expect(bindings.isRowEmpty(empty)).toBe(false);
        expect(bindings.getRowIssues(empty)).toEqual(['Category', 'Source Account', 'Amount']);
        expect(bindings.getRowStatusLabel(empty)).toBe('Needs Required Fields');
        expect(bindings.getRowStatusColor(empty)).toBe('warning');

        const valid = makeValidExpense(bindings, { sourceAmountCents: 1234 });
        expect(bindings.isRowValid(valid)).toBe(true);
        expect(bindings.getRowIssues(valid)).toEqual([]);
        expect(bindings.getRowStatusLabel(valid)).toBe('Ready to Save');
        expect(bindings.getRowStatusColor(valid)).toBe('success');
        expect(valid.transaction.sourceAmountCents).toBe(1234);

        const transfer = makeRow(bindings, {
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            sourceAmountCents: 4567
        });
        expect(bindings.getRowIssues(transfer)).toEqual(['Destination Account', 'Destination Amount']);
        transfer.transaction.destinationAccountId = 'bank';
        transfer.transaction.destinationAmountCents = 4567;
        expect(bindings.isRowValid(transfer)).toBe(true);
        expect(transfer.transaction.destinationAmountCents).toBe(4567);

        bindings.rows.value = [empty, valid, transfer];
        expect(bindings.invalidNonEmptyRowCount.value).toBe(1);
    });

    test('selects category collections, account currencies, and available grid fields', () => {
        const bindings = setupDialog();
        expect(bindings.getCategoryItems(transactionType.Expense)).toEqual([expenseCategory]);
        expect(bindings.getCategoryItems(transactionType.Income)).toEqual([incomeCategory]);
        expect(bindings.getCategoryItems(transactionType.Transfer)).toEqual([transferCategory]);
        expect(bindings.getCategoryItems(transactionType.Investment)).toEqual([investmentCategory]);
        expect(bindings.getCategoryItems(99)).toEqual([]);
        categoriesStore.allTransactionCategories[categoryType.Income] = undefined as any;
        expect(bindings.getCategoryItems(transactionType.Income)).toEqual([]);

        const expense = makeValidExpense(bindings);
        expect(bindings.getSourceCurrency(expense.transaction)).toBe('CNY');
        expense.transaction.sourceAccountId = 'missing';
        expect(bindings.getSourceCurrency(expense.transaction)).toBe('CNY');
        userStore.currentUserDefaultCurrency = '';
        expect(bindings.getSourceCurrency(expense.transaction)).toBe('CNY');

        expense.transaction.destinationAccountId = 'bank';
        expect(bindings.getDestinationCurrency(expense.transaction)).toBe('USD');
        expense.transaction.destinationAccountId = 'missing';
        expect(bindings.getDestinationCurrency(expense.transaction)).toBe('CNY');

        expect(bindings.getAvailableFieldKeys(expense.transaction)).not.toContain('destinationAccount');
        expect(bindings.getFallbackFieldForTransaction(expense.transaction, 'destinationAccount')).toBe('sourceAccount');
        expect(bindings.getFallbackFieldForTransaction(expense.transaction, 'destinationAmountCents')).toBe('sourceAmountCents');
        expect(bindings.getFallbackFieldForTransaction(expense.transaction, 'comment')).toBe('comment');
        expense.transaction.type = transactionType.Transfer;
        expect(bindings.getAvailableFieldKeys(expense.transaction)).toEqual(bindings.BATCH_ENTRY_FIELD_ORDER);
        expect(bindings.getFallbackFieldForTransaction(expense.transaction, 'destinationAccount')).toBe('destinationAccount');
    });

    test('gates grid navigation for modifiers, overlays, expanded fields, and text cursors', () => {
        const bindings = setupDialog();
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowDown', null, { altKey: true }))).toBe(false);
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowDown', null, { ctrlKey: true }))).toBe(false);
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowDown', null, { metaKey: true }))).toBe(false);
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('Enter'))).toBe(false);

        const element = new FakeHTMLElement();
        element.closestResult = {};
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowDown', element))).toBe(false);
        element.closestResult = null;
        element.attributes['aria-expanded'] = 'true';
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowUp', element))).toBe(false);
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowLeft', element))).toBe(true);

        const input = new FakeHTMLInputElement();
        input.value = 'abcd';
        input.selectionStart = 2;
        input.selectionEnd = 2;
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowLeft', input))).toBe(false);
        input.selectionStart = 0;
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowLeft', input))).toBe(true);
        input.selectionEnd = 2;
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowRight', input))).toBe(false);
        input.selectionEnd = 4;
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowRight', input))).toBe(true);

        const textarea = new FakeHTMLTextAreaElement();
        textarea.value = '';
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowDown', textarea))).toBe(true);
    });

    test('focuses grid inputs and fallback cells while tolerating missing cells', () => {
        const bindings = setupDialog();
        bindings.focusBatchField(0, 'time');

        const input = new FakeHTMLInputElement();
        const clickTarget = new FakeHTMLElement();
        const inputCell = new FakeHTMLElement() as any;
        inputCell.querySelector = jest.fn((selector: string) => selector.startsWith('input') ? input : clickTarget);
        (globalThis.document.querySelector as jest.Mock).mockReturnValueOnce(inputCell);
        bindings.focusBatchField(1, 'comment');
        expect(input.focused).toBe(true);
        expect(input.selected).toBe(true);

        const fallbackCell = new FakeHTMLElement() as any;
        fallbackCell.querySelector = jest.fn((selector: string) => selector.startsWith('input') ? null : clickTarget);
        (globalThis.document.querySelector as jest.Mock).mockReturnValueOnce(fallbackCell);
        bindings.focusBatchField(2, 'category');
        expect(clickTarget.focused).toBe(true);
        expect(clickTarget.clicked).toBe(true);
    });

    test('moves horizontally and vertically across row-specific grid fields', () => {
        const bindings = setupDialog();
        const expense = makeValidExpense(bindings);
        const transfer = makeRow(bindings, {
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            destinationAccountId: 'bank',
            sourceAmountCents: 1000,
            destinationAmountCents: 1000
        });
        bindings.rows.value = [expense, transfer];
        const right = keyboardEvent('ArrowRight');
        bindings.onCellKeydown(0, 'type', right);
        expect(right.preventDefault).toHaveBeenCalled();
        expect(right.stopPropagation).toHaveBeenCalled();

        const down = keyboardEvent('ArrowDown');
        bindings.onCellKeydown(0, 'sourceAmountCents', down);
        expect(down.preventDefault).toHaveBeenCalled();

        const fallbackUp = keyboardEvent('ArrowUp');
        bindings.onCellKeydown(1, 'destinationAccount', fallbackUp);
        expect(fallbackUp.preventDefault).toHaveBeenCalled();

        for (const [rowIndex, field, event] of [
            [99, 'time', keyboardEvent('ArrowDown')],
            [0, 'time', keyboardEvent('ArrowLeft')],
            [0, 'tags', keyboardEvent('ArrowRight')],
            [0, 'time', keyboardEvent('ArrowUp')],
            [1, 'time', keyboardEvent('ArrowDown')],
            [0, 'time', keyboardEvent('Enter')]
        ] as const) {
            bindings.onCellKeydown(rowIndex, field, event);
        }
        expect(globalThis.document.querySelector).toHaveBeenCalledTimes(3);
    });

    test('opens with defaults, loads supporting data, and cancels the pending dialog promise', async () => {
        const bindings = setupDialog();
        const pending = bindings.open({
            type: transactionType.Income,
            categoryId: 'salary',
            accountId: 'bank',
            tagIds: 'work'
        });
        expect(bindings.showState.value).toBe(true);
        expect(bindings.submitting.value).toBe(false);
        expect(bindings.rows.value[0].transaction).toMatchObject({
            type: transactionType.Income,
            incomeCategoryId: 'salary',
            sourceAccountId: 'bank',
            tagIds: ['work']
        });
        expect(accountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(categoriesStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(tagsStore.loadAllTags).toHaveBeenCalledWith({ force: false });

        bindings.cancel();
        await expect(pending).rejects.toBeUndefined();
        expect(bindings.showState.value).toBe(false);

        accountsStore.loadAllAccounts.mockRejectedValueOnce(new Error('accounts unavailable'));
        categoriesStore.loadAllCategories.mockRejectedValueOnce(new Error('categories unavailable'));
        tagsStore.loadAllTags.mockRejectedValueOnce(new Error('tags unavailable'));
        const ignoredFailures = bindings.open();
        await flushPromises();
        bindings.cancel();
        await expect(ignoredFailures).rejects.toBeUndefined();
    });

    test('rejects empty and partially completed submissions with row-level feedback', () => {
        const bindings = setupDialog();
        const snackbar = { showError: jest.fn(), showMessage: jest.fn() };
        bindings.snackbar.value = snackbar;
        bindings.reset();

        bindings.submit();
        expect(snackbar.showError).toHaveBeenLastCalledWith('No valid transactions to save');
        expect(transactionsStore.saveTransactions).not.toHaveBeenCalled();

        bindings.rows.value[0].transaction.comment = 'not complete';
        bindings.submit();
        expect(snackbar.showError).toHaveBeenLastCalledWith('Please complete required fields in all non-empty rows');
        expect(transactionsStore.saveTransactions).not.toHaveBeenCalled();
    });

    test('submits every valid non-empty transaction with exact cents and resolves the open promise', async () => {
        const bindings = setupDialog();
        const pending = bindings.open();
        const expense = makeValidExpense(bindings, {
            sourceAmountCents: 1234,
            tagIds: ['daily'],
            comment: 'lunch'
        });
        const transfer = makeRow(bindings, {
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            destinationAccountId: 'bank',
            sourceAmountCents: 5678,
            destinationAmountCents: 5678
        });
        const empty = makeRow(bindings);
        bindings.rows.value = [expense, empty, transfer];
        transactionsStore.saveTransactions.mockResolvedValueOnce([{ id: 'a' }, { id: 'b' }]);

        bindings.submit();
        expect(bindings.submitting.value).toBe(true);
        expect(transactionsStore.saveTransactions).toHaveBeenCalledWith({
            transactions: [expense.transaction, transfer.transaction],
            clientSessionId: expect.stringMatching(/^batch-manual-/)
        });
        const payload = transactionsStore.saveTransactions.mock.calls[0]![0];
        expect(payload.transactions.map((transaction: MockTransaction) => transaction.sourceAmountCents)).toEqual([1234, 5678]);
        expect(payload.transactions[1].destinationAmountCents).toBe(5678);

        await flushPromises();
        await expect(pending).resolves.toEqual({
            message: 'Batch created transactions successfully:{"count":2}'
        });
        expect(bindings.submitting.value).toBe(false);
        expect(bindings.showState.value).toBe(false);
    });

    test('reports unprocessed API failures and silently preserves processed failures', async () => {
        const bindings = setupDialog();
        const snackbar = { showError: jest.fn(), showMessage: jest.fn() };
        bindings.snackbar.value = snackbar;
        bindings.rows.value = [makeValidExpense(bindings)];

        const unprocessed = new Error('network failed') as Error & { processed?: boolean };
        transactionsStore.saveTransactions.mockRejectedValueOnce(unprocessed);
        bindings.submit();
        await flushPromises();
        expect(bindings.submitting.value).toBe(false);
        expect(snackbar.showError).toHaveBeenCalledWith(unprocessed);

        snackbar.showError.mockClear();
        const processed = Object.assign(new Error('already shown'), { processed: true });
        transactionsStore.saveTransactions.mockRejectedValueOnce(processed);
        bindings.submit();
        await flushPromises();
        expect(snackbar.showError).not.toHaveBeenCalled();
    });

    test('renders empty, invalid, ready, transfer, and submitting template branches', () => {
        const bindings = setupDialog();
        bindings.rows.value = [makeRow(bindings)];
        expect(render(bindings)).toBeDefined();

        bindings.rows.value[0].transaction.comment = 'incomplete';
        expect(render(bindings)).toBeDefined();

        bindings.rows.value = [
            makeValidExpense(bindings),
            makeRow(bindings, {
                type: transactionType.Transfer,
                transferCategoryId: 'move',
                sourceAccountId: 'wallet',
                destinationAccountId: 'bank',
                sourceAmountCents: 1000,
                destinationAmountCents: 1000
            }),
            makeRow(bindings, {
                type: transactionType.Investment,
                investmentCategoryId: 'fund',
                sourceAccountId: 'bank',
                destinationAccountId: 'broker',
                sourceAmountCents: 2000,
                destinationAmountCents: 2100
            })
        ];
        bindings.submitting.value = true;
        expect(render(bindings)).toBeDefined();
    });

    test('covers fallback collections, sparse rows, strict zero guards, and no-listener completion', async () => {
        const bindings = setupDialog();
        (tagsStore as any).allVisibleTags = undefined;
        (accountsStore as any).allVisiblePlainAccounts = undefined;
        expect(bindings.tagOptions.value).toEqual([]);
        expect(bindings.categorizedAccountOptions.value[0].accounts).toEqual([]);

        const transferSource = makeRow(bindings, {
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            destinationAccountId: 'bank',
            sourceAmountCents: 2500,
            destinationAmountCents: 0,
            tagIds: []
        });
        const differentlyTyped = makeRow(bindings, {
            type: transactionType.Expense,
            expenseCategoryId: 'existing',
            sourceAccountId: '0',
            sourceAmountCents: -1,
            destinationAccountId: '0',
            destinationAmountCents: -1,
            tagIds: []
        });
        bindings.fillEmptyCommonFields(differentlyTyped.transaction, transferSource.transaction, differentlyTyped.id);
        expect(differentlyTyped.transaction.type).toBe(transactionType.Transfer);
        expect(differentlyTyped.transaction.destinationAmountCents).toBe(2500);

        const finalTarget = makeRow(bindings, { sourceAmountCents: 0, destinationAmountCents: 0 });
        bindings.applyCommonFields(finalTarget.transaction, transferSource.transaction, finalTarget.id);
        expect(finalTarget.transaction.destinationAmountCents).toBe(2500);

        const last = makeRow(bindings);
        bindings.rows.value = [transferSource, undefined, last] as any;
        bindings.applyCommonFieldsDown(0);
        bindings.applyCommonFieldsToAll(0);
        bindings.fillEmptyFieldsDown(0);
        bindings.fillEmptyFieldsToAll(0);

        const invalidAccount = makeValidExpense(bindings, { sourceAccountId: '0' });
        const invalidAmount = makeValidExpense(bindings, { sourceAmountCents: -1 });
        const invalidDestination = makeRow(bindings, {
            type: transactionType.Transfer,
            transferCategoryId: 'move',
            sourceAccountId: 'wallet',
            sourceAmountCents: 1,
            destinationAccountId: '0',
            destinationAmountCents: -1
        });
        expect(bindings.isRowValid(invalidAccount)).toBe(false);
        expect(bindings.isRowValid(invalidAmount)).toBe(false);
        expect(bindings.isRowValid(invalidDestination)).toBe(false);
        expect(bindings.getRowIssues(invalidDestination)).toEqual(['Destination Account', 'Destination Amount']);

        bindings.onCategoryChanged(invalidAccount.transaction, '');
        expect(invalidAccount.transaction.getCategoryId()).toBe('');
        const unknownField = keyboardEvent('ArrowRight');
        bindings.rows.value = [makeValidExpense(bindings)];
        bindings.onCellKeydown(0, 'destinationAccount', unknownField);
        expect(unknownField.preventDefault).not.toHaveBeenCalled();

        const nullableInput = new FakeHTMLInputElement();
        nullableInput.selectionStart = null;
        nullableInput.selectionEnd = null;
        (nullableInput as any).value = undefined;
        expect(bindings.shouldHandleGridNavigation(keyboardEvent('ArrowLeft', nullableInput))).toBe(true);

        bindings.cancel();
        bindings.rows.value = [makeValidExpense(bindings)];
        transactionsStore.saveTransactions.mockResolvedValueOnce([{ id: 'without-open' }]);
        bindings.submit();
        await flushPromises();
        expect(bindings.submitting.value).toBe(false);
    });
});
