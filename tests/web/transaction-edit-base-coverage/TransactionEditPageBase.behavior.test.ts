import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;

const mockGetCurrentUnixTime = jest.fn(() => 1_700_000_000);
const mockGetTimezoneOffsetMinutes = jest.fn((timezone: string) => {
    if (timezone === 'Asia/Shanghai') return 480;
    if (timezone === 'UTC') return 0;
    return -60;
});
const mockGetUtcOffsetByUtcOffsetMinutes = jest.fn((minutes: number) => (
    minutes === 480 ? '+08:00' : minutes === 0 ? '+00:00' : '-01:00'
));
const mockGetExchangedAmountByRate = jest.fn((amountCents: number, _fromRate: number, _toRate: number) => (
    amountCents === 999 ? 0 : amountCents / 2 + 0.9
));
const mockGetAdaptiveAmountRate = jest.fn((
    sourceAmountCents: number,
    destinationAmountCents: number,
    fromRate?: { rate?: number },
    toRate?: { rate?: number },
) => {
    if (!sourceAmountCents || !fromRate?.rate || !toRate?.rate) return '';
    return `rate:${sourceAmountCents}:${destinationAmountCents}`;
});
const mockFormatAmount = jest.fn((amountCents: number, currency: string) => `${currency}:${amountCents}`);
const mockCategorizeAccounts = jest.fn((accounts: any[], showBalance: boolean) => ([{
    category: 1,
    accounts,
    showBalance,
}]));

const mockNumeralSystem = {
    replaceWesternArabicDigitsToLocalizedDigits: jest.fn((value: string) => `localized(${value})`),
};
const mockTimezones = [
    { name: 'Asia/Shanghai', utcOffsetMinutes: 480 },
    { name: 'UTC', utcOffsetMinutes: 0 },
];
const mockSettingsStore: any = actualVue.reactive({
    appSettings: {
        timeZone: 'Asia/Shanghai',
        showAccountBalance: true,
    },
});
const mockUserStore: any = actualVue.reactive({
    currentUserDefaultCurrency: 'CNY',
    currentUserDefaultAccountId: 'cash',
    currentUserFirstDayOfWeek: 2,
    currentUserCoordinateDisplayType: 1,
});
const mockAccountsStore: any = actualVue.reactive({
    allPlainAccounts: [] as any[],
    allVisiblePlainAccounts: [] as any[],
    allAccountsMap: {} as Record<string, any>,
});
const mockCategoryStore: any = actualVue.reactive({
    allTransactionCategories: { 2: [], 3: [], 4: [], 5: [] } as Record<number, any[]>,
    allTransactionCategoriesMap: {} as Record<string, any>,
    hasAvailableExpenseCategories: true,
    hasAvailableIncomeCategories: true,
    hasAvailableTransferCategories: true,
    hasAvailableInvestmentCategories: false,
});
const mockTagStore: any = actualVue.reactive({
    allTransactionTags: [{ id: 'tag-1', name: 'Daily' }],
    allTransactionTagsMap: { 'tag-1': { id: 'tag-1', name: 'Daily' } },
});
const mockTransactionStore: any = {
    getTransactionPictureUrl: jest.fn((picture?: { pictureId: string } | null) => (
        picture ? `/pictures/${picture.pictureId}` : undefined
    )),
    setTransactionSuitableDestinationAmount: jest.fn(),
};
const mockExchangeRatesStore: any = actualVue.reactive({
    latestExchangeRateMap: {
        CNY: { rate: 1 },
        USD: { rate: 7.2 },
        EUR: { rate: 7.8 },
    } as Record<string, { rate: number }>,
});

Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: Object.assign(globalThis.navigator ?? {}, { geolocation: { getCurrentPosition: jest.fn() } }),
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getAllTimezones: (includeUtc: boolean) => includeUtc ? mockTimezones : [],
        getCurrentNumeralSystemType: () => mockNumeralSystem,
        getTimezoneDifferenceDisplayText: (utcOffset: number) => `difference:${utcOffset}`,
        formatAmountToLocalizedNumeralsWithCurrency: mockFormatAmount,
        getAdaptiveAmountRate: mockGetAdaptiveAmountRate,
        getCategorizedAccountsWithDisplayBalance: mockCategorizeAccounts,
    }),
}));

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionStore }));
jest.mock('@/stores/exchangeRates.ts', () => ({ useExchangeRatesStore: () => mockExchangeRatesStore }));

jest.mock('@/lib/datetime.ts', () => {
    const actual = jest.requireActual('@/lib/datetime.ts') as any;
    return {
        ...actual,
        getCurrentUnixTime: mockGetCurrentUnixTime,
        getTimezoneOffsetMinutes: mockGetTimezoneOffsetMinutes,
        getUtcOffsetByUtcOffsetMinutes: mockGetUtcOffsetByUtcOffsetMinutes,
    };
});

jest.mock('@/lib/numeral.ts', () => {
    const actual = jest.requireActual('@/lib/numeral.ts') as any;
    return {
        ...actual,
        getExchangedAmountByRate: mockGetExchangedAmountByRate,
    };
});

const {
    GeoLocationStatus,
    TransactionEditPageMode,
    TransactionEditPageType,
    useTransactionEditPageBase,
} = require('@/views/base/transactions/TransactionEditPageBase.ts') as any;
const { TransactionType } = require('@/core/transaction.ts') as any;
const { TemplateType } = require('@/core/template.ts') as any;
const { Transaction } = require('@/models/transaction.ts') as any;
const { TransactionTemplate } = require('@/models/transaction_template.ts') as any;
const { TransactionPicture } = require('@/models/transaction_picture_info.ts') as any;
const { DISPLAY_HIDDEN_AMOUNT } = require('@/consts/numeral.ts') as any;

function createAccount(id: string, name: string, currency: string): any {
    return {
        id,
        name,
        parentId: '',
        category: 1,
        type: 1,
        icon: 'cash',
        color: '112233',
        currency,
        balanceCents: 12_345,
        comment: '',
        displayOrder: 1,
        hidden: false,
    };
}

function setupBase(
    type: string = TransactionEditPageType.Transaction,
    mode?: string,
    transactionDefaultType?: number,
): any {
    return useTransactionEditPageBase(type, mode, transactionDefaultType);
}

async function flushWatchers(): Promise<void> {
    await actualVue.nextTick();
    await Promise.resolve();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockSettingsStore.appSettings.timeZone = 'Asia/Shanghai';
    mockSettingsStore.appSettings.showAccountBalance = true;
    mockUserStore.currentUserDefaultCurrency = 'CNY';
    mockUserStore.currentUserDefaultAccountId = 'cash';
    mockUserStore.currentUserFirstDayOfWeek = 2;
    mockUserStore.currentUserCoordinateDisplayType = 1;

    const cash = createAccount('cash', 'Cash Wallet', 'CNY');
    const usd = createAccount('usd', 'US Wallet', 'USD');
    const eur = createAccount('eur', 'Euro Wallet', 'EUR');
    mockAccountsStore.allPlainAccounts = [cash, usd, eur];
    mockAccountsStore.allVisiblePlainAccounts = [cash, usd];
    mockAccountsStore.allAccountsMap = { cash, usd, eur };

    mockCategoryStore.allTransactionCategories = { 2: [{ id: 'income-salary' }], 3: [{ id: 'expense-food' }], 4: [], 5: [] };
    mockCategoryStore.allTransactionCategoriesMap = { 'expense-food': { id: 'expense-food' } };
    mockCategoryStore.hasAvailableExpenseCategories = true;
    mockCategoryStore.hasAvailableIncomeCategories = true;
    mockCategoryStore.hasAvailableTransferCategories = true;
    mockCategoryStore.hasAvailableInvestmentCategories = false;
    mockTagStore.allTransactionTags = [{ id: 'tag-1', name: 'Daily' }];
    mockTagStore.allTransactionTagsMap = { 'tag-1': { id: 'tag-1', name: 'Daily' } };
    mockExchangeRatesStore.latestExchangeRateMap = {
        CNY: { rate: 1 },
        USD: { rate: 7.2 },
        EUR: { rate: 7.8 },
    };
});

describe('TransactionEditPageBase production-loaded state and model creation', () => {
    test('projects stores, locale settings, accounts, categories, tags, and defaults', () => {
        const bindings = setupBase();

        expect(bindings.isSupportGeoLocation).toBe(true);
        expect(bindings.mode.value).toBe(TransactionEditPageMode.Add);
        expect(bindings.transaction.value).toBeInstanceOf(Transaction);
        expect(bindings.transaction.value.type).toBe(TransactionType.Expense);
        expect(bindings.transaction.value.time).toBe(1_700_000_000);
        expect(bindings.transaction.value.timeZone).toBe('Asia/Shanghai');
        expect(bindings.transaction.value.utcOffset).toBe(480);
        expect(bindings.numeralSystem.value).toBe(mockNumeralSystem);
        expect(bindings.currentTimezoneOffsetMinutes.value).toBe(480);
        expect(bindings.showAccountBalance.value).toBe(true);
        expect(bindings.defaultCurrency.value).toBe('CNY');
        expect(bindings.defaultAccountId.value).toBe('cash');
        expect(bindings.firstDayOfWeek.value).toBe(2);
        expect(bindings.coordinateDisplayType.value).toBe(1);
        expect(bindings.allTimezones.value).toStrictEqual(mockTimezones);
        expect(bindings.allAccounts.value).toHaveLength(3);
        expect(bindings.allVisibleAccounts.value).toHaveLength(2);
        expect(bindings.allAccountsMap.value.usd.currency).toBe('USD');
        expect(bindings.allVisibleCategorizedAccounts.value[0]).toMatchObject({ showBalance: true });
        expect(mockCategorizeAccounts).toHaveBeenCalledWith(mockAccountsStore.allVisiblePlainAccounts, true);
        expect(bindings.allCategories.value[3]).toHaveLength(1);
        expect(bindings.allCategoriesMap.value['expense-food']).toBeDefined();
        expect(bindings.allTags.value).toHaveLength(1);
        expect(bindings.allTagsMap.value['tag-1']).toBeDefined();
        expect(bindings.firstVisibleAccountId.value).toBe('cash');
        expect(bindings.hasAvailableExpenseCategories.value).toBe(true);
        expect(bindings.hasAvailableIncomeCategories.value).toBe(true);
        expect(bindings.hasAvailableTransferCategories.value).toBe(true);
        expect(bindings.hasAvailableInvestmentCategories.value).toBe(false);

        mockAccountsStore.allVisiblePlainAccounts = [];
        expect(bindings.firstVisibleAccountId.value).toBeUndefined();
        mockAccountsStore.allVisiblePlainAccounts = undefined;
        expect(bindings.firstVisibleAccountId.value).toBeUndefined();
    });

    test('creates transaction and template models for supported default types', () => {
        const bindings = setupBase(TransactionEditPageType.Transaction, TransactionEditPageMode.Edit, TransactionType.Income);
        expect(bindings.mode.value).toBe(TransactionEditPageMode.Edit);
        expect(bindings.transaction.value.type).toBe(TransactionType.Income);
        expect(bindings.createNewTransactionModel(TransactionType.Income).type).toBe(TransactionType.Income);
        expect(bindings.createNewTransactionModel(TransactionType.Transfer).type).toBe(TransactionType.Transfer);
        expect(bindings.createNewTransactionModel(TransactionType.Investment).type).toBe(TransactionType.Expense);
        expect(bindings.createNewTransactionModel().type).toBe(TransactionType.Expense);

        const templateBindings = setupBase(TransactionEditPageType.Template);
        expect(templateBindings.transaction.value).toBeInstanceOf(TransactionTemplate);
        expect(templateBindings.createNewTransactionModel(TransactionType.Transfer)).toBeInstanceOf(TransactionTemplate);
        expect(mockGetCurrentUnixTime).toHaveBeenCalled();
        expect(mockGetTimezoneOffsetMinutes).toHaveBeenCalledWith('Asia/Shanghai');
    });

    test('covers transaction, normal-template, scheduled-template, and button titles', () => {
        const transactionBindings = setupBase();
        expect(transactionBindings.title.value).toBe('Add Transaction');
        expect(transactionBindings.saveButtonTitle.value).toBe('Add');
        expect(transactionBindings.cancelButtonTitle.value).toBe('Cancel');
        transactionBindings.mode.value = TransactionEditPageMode.Edit;
        expect(transactionBindings.title.value).toBe('Edit Transaction');
        expect(transactionBindings.saveButtonTitle.value).toBe('Save');
        transactionBindings.mode.value = TransactionEditPageMode.View;
        expect(transactionBindings.title.value).toBe('Transaction Detail');
        expect(transactionBindings.cancelButtonTitle.value).toBe('Close');

        const templateBindings = setupBase(TransactionEditPageType.Template);
        templateBindings.transaction.value.templateType = TemplateType.Normal.type;
        expect(templateBindings.title.value).toBe('Add Transaction Template');
        templateBindings.mode.value = TransactionEditPageMode.Edit;
        expect(templateBindings.title.value).toBe('Edit Transaction Template');
        templateBindings.transaction.value.templateType = TemplateType.Schedule.type;
        templateBindings.mode.value = TransactionEditPageMode.Add;
        expect(templateBindings.title.value).toBe('Add Scheduled Transaction');
        templateBindings.mode.value = TransactionEditPageMode.Edit;
        expect(templateBindings.title.value).toBe('Edit Scheduled Transaction');
        templateBindings.mode.value = TransactionEditPageMode.View;
        expect(templateBindings.title.value).toBe('');
        templateBindings.transaction.value.templateType = 999;
        expect(templateBindings.title.value).toBe('');
    });

    test('allows pictures only for editable transactions below the fixed count limit', () => {
        const bindings = setupBase();
        expect(bindings.canAddTransactionPicture.value).toBe(true);

        const pictures = Array.from({ length: 10 }, (_, index) => TransactionPicture.of({
            pictureId: `picture-${index}`,
            originalUrl: `/picture-${index}.jpg`,
        }));
        bindings.transaction.value.setPictures(pictures);
        expect(bindings.canAddTransactionPicture.value).toBe(false);
        bindings.mode.value = TransactionEditPageMode.View;
        expect(bindings.canAddTransactionPicture.value).toBe(false);
        bindings.mode.value = TransactionEditPageMode.Edit;
        bindings.transaction.value.setPictures([]);
        expect(bindings.canAddTransactionPicture.value).toBe(true);

        const templateBindings = setupBase(TransactionEditPageType.Template);
        expect(templateBindings.canAddTransactionPicture.value).toBe(false);
    });
});

describe('TransactionEditPageBase production-loaded labels and cents display', () => {
    test('derives amount and account labels for every transaction type', () => {
        const bindings = setupBase();
        const expectations = [
            [TransactionType.Expense, 'Expense Amount', 'Account'],
            [TransactionType.Income, 'Income Amount', 'Account'],
            [TransactionType.Transfer, 'Transfer Out Amount', 'Source Account'],
            [TransactionType.Investment, 'Amount', 'Account'],
        ];
        for (const [type, amountName, accountTitle] of expectations) {
            bindings.transaction.value.type = type;
            expect(bindings.sourceAmountName.value).toBe(amountName);
            expect(bindings.sourceAccountTitle.value).toBe(accountTitle);
        }
    });

    test('keeps source amounts in cents through exchange display and hidden-amount formatting', () => {
        const bindings = setupBase();
        bindings.transaction.value.sourceAccountId = '';
        bindings.transaction.value.sourceAmountCents = 12_345;
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');

        bindings.transaction.value.sourceAccountId = 'cash';
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');
        bindings.transaction.value.sourceAccountId = 'usd';
        bindings.transaction.value.sourceAmountCents = 0;
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');
        bindings.transaction.value.sourceAmountCents = 12_345;
        bindings.transaction.value.hideAmount = true;
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');
        bindings.transaction.value.hideAmount = false;

        mockExchangeRatesStore.latestExchangeRateMap = { CNY: { rate: 1 } };
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');
        mockExchangeRatesStore.latestExchangeRateMap = { USD: { rate: 0 }, CNY: { rate: 1 } };
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');
        mockExchangeRatesStore.latestExchangeRateMap = { USD: { rate: 7.2 } };
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');
        mockExchangeRatesStore.latestExchangeRateMap = { USD: { rate: 7.2 }, CNY: { rate: 0 } };
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');

        mockExchangeRatesStore.latestExchangeRateMap = { USD: { rate: 7.2 }, CNY: { rate: 1 } };
        bindings.transaction.value.sourceAmountCents = 999;
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount');
        bindings.transaction.value.sourceAmountCents = 12_345;
        expect(bindings.sourceAmountTitle.value).toBe('Expense Amount (CNY:6173)');
        expect(mockGetExchangedAmountByRate).toHaveBeenLastCalledWith(12_345, 7.2, 1);
        expect(mockFormatAmount).toHaveBeenCalledWith(6_173, 'CNY');

        expect(bindings.getDisplayAmount(98_765, false, 'USD')).toBe('USD:98765');
        expect(bindings.getDisplayAmount(98_765, true, 'USD')).toBe(`USD:${DISPLAY_HIDDEN_AMOUNT}`);
        expect(mockFormatAmount).toHaveBeenCalledWith(98_765, 'USD');
        expect(mockFormatAmount).toHaveBeenCalledWith(DISPLAY_HIDDEN_AMOUNT, 'USD');
    });

    test('derives transfer-in rate text without converting either cents field', () => {
        const bindings = setupBase();
        bindings.transaction.value.type = TransactionType.Transfer;
        bindings.transaction.value.sourceAmountCents = 12_345;
        bindings.transaction.value.destinationAmountCents = 67_890;
        expect(bindings.transferInAmountTitle.value).toBe('Transfer In Amount');

        bindings.transaction.value.sourceAccountId = 'cash';
        bindings.transaction.value.destinationAccountId = 'cash';
        expect(bindings.transferInAmountTitle.value).toBe('Transfer In Amount');

        bindings.transaction.value.destinationAccountId = 'usd';
        mockExchangeRatesStore.latestExchangeRateMap = { CNY: { rate: 1 } };
        expect(bindings.transferInAmountTitle.value).toBe('Transfer In Amount');

        mockExchangeRatesStore.latestExchangeRateMap = { CNY: { rate: 1 }, USD: { rate: 7.2 } };
        expect(bindings.transferInAmountTitle.value).toBe('Transfer In Amount (rate:12345:67890)');
        expect(mockGetAdaptiveAmountRate).toHaveBeenLastCalledWith(
            12_345,
            67_890,
            { rate: 1 },
            { rate: 7.2 },
        );
    });

    test('projects account names, currencies, timezone text, and geo-location status', () => {
        const bindings = setupBase();
        expect(bindings.sourceAccountName.value).toBe('None');
        expect(bindings.destinationAccountName.value).toBe('None');
        expect(bindings.sourceAccountCurrency.value).toBe('CNY');
        expect(bindings.destinationAccountCurrency.value).toBe('CNY');

        bindings.transaction.value.sourceAccountId = 'usd';
        bindings.transaction.value.destinationAccountId = 'eur';
        expect(bindings.sourceAccountName.value).toBe('US Wallet');
        expect(bindings.destinationAccountName.value).toBe('Euro Wallet');
        expect(bindings.sourceAccountCurrency.value).toBe('USD');
        expect(bindings.destinationAccountCurrency.value).toBe('EUR');
        bindings.transaction.value.sourceAccountId = 'missing';
        bindings.transaction.value.destinationAccountId = 'missing';
        expect(bindings.sourceAccountName.value).toBe('');
        expect(bindings.destinationAccountName.value).toBe('');
        expect(bindings.sourceAccountCurrency.value).toBe('CNY');
        expect(bindings.destinationAccountCurrency.value).toBe('CNY');

        bindings.transaction.value.utcOffset = 480;
        expect(bindings.transactionDisplayTimezone.value).toBe('UTClocalized(+08:00)');
        expect(bindings.transactionTimezoneTimeDifference.value).toBe('difference:480');
        expect(mockGetUtcOffsetByUtcOffsetMinutes).toHaveBeenCalledWith(480);

        bindings.geoLocationStatus.value = GeoLocationStatus.Success;
        expect(bindings.geoLocationStatusInfo.value).toBe('');
        bindings.geoLocationStatus.value = GeoLocationStatus.Getting;
        expect(bindings.geoLocationStatusInfo.value).toBe('Getting Location...');
        bindings.geoLocationStatus.value = GeoLocationStatus.Error;
        expect(bindings.geoLocationStatusInfo.value).toBe('No Location');
        bindings.geoLocationStatus.value = null;
        expect(bindings.geoLocationStatusInfo.value).toBe('No Location');
    });
});

describe('TransactionEditPageBase production-loaded validation and mutations', () => {
    test('reports required fields for every transaction type and accepts complete transactions', () => {
        const bindings = setupBase();
        expect(bindings.inputEmptyProblemMessage.value).toBe('Transaction category cannot be blank');
        expect(bindings.inputIsEmpty.value).toBe(true);
        bindings.transaction.value.expenseCategoryId = 'expense-food';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Transaction account cannot be blank');
        bindings.transaction.value.sourceAccountId = 'cash';
        expect(bindings.inputEmptyProblemMessage.value).toBeNull();

        bindings.transaction.value.type = TransactionType.Income;
        expect(bindings.inputEmptyProblemMessage.value).toBe('Transaction category cannot be blank');
        bindings.transaction.value.incomeCategoryId = 'income-salary';
        expect(bindings.inputEmptyProblemMessage.value).toBeNull();
        bindings.transaction.value.sourceAccountId = '';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Transaction account cannot be blank');

        bindings.transaction.value.type = TransactionType.Transfer;
        expect(bindings.inputEmptyProblemMessage.value).toBe('Transaction category cannot be blank');
        bindings.transaction.value.transferCategoryId = 'transfer-general';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Source account cannot be blank');
        bindings.transaction.value.sourceAccountId = 'cash';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Destination account cannot be blank');
        bindings.transaction.value.destinationAccountId = 'usd';
        expect(bindings.inputEmptyProblemMessage.value).toBeNull();
        expect(bindings.inputIsEmpty.value).toBe(false);

        bindings.transaction.value.type = TransactionType.Investment;
        bindings.transaction.value.sourceAccountId = '';
        bindings.transaction.value.destinationAccountId = '';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Transaction category cannot be blank');
        bindings.transaction.value.investmentCategoryId = 'investment-fund';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Source account cannot be blank');
        bindings.transaction.value.sourceAccountId = 'cash';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Destination account cannot be blank');
        bindings.transaction.value.destinationAccountId = 'investment';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Investment amount cannot be blank');
        bindings.transaction.value.destinationAmountCents = 12_345;
        expect(bindings.inputEmptyProblemMessage.value).toBeNull();
        expect(bindings.inputIsEmpty.value).toBe(false);
    });

    test('validates transaction fields before requiring a transaction-template name', () => {
        const bindings = setupBase(TransactionEditPageType.Template);
        expect(bindings.inputEmptyProblemMessage.value).toBe('Transaction category cannot be blank');
        bindings.transaction.value.expenseCategoryId = 'expense-food';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Transaction account cannot be blank');
        bindings.transaction.value.sourceAccountId = 'cash';
        expect(bindings.inputEmptyProblemMessage.value).toBe('Template name cannot be blank');
        bindings.transaction.value.name = 'Monthly food';
        expect(bindings.inputEmptyProblemMessage.value).toBeNull();
    });

    test('swaps account ids and raw cents independently and together', () => {
        const bindings = setupBase();
        bindings.transaction.value.sourceAccountId = 'cash';
        bindings.transaction.value.destinationAccountId = 'usd';
        bindings.transaction.value.sourceAmountCents = 12_345;
        bindings.transaction.value.destinationAmountCents = 67_890;

        bindings.swapTransactionData(true, false);
        expect(bindings.transaction.value.sourceAccountId).toBe('usd');
        expect(bindings.transaction.value.destinationAccountId).toBe('cash');
        expect(bindings.transaction.value.sourceAmountCents).toBe(12_345);
        expect(bindings.transaction.value.destinationAmountCents).toBe(67_890);

        bindings.swapTransactionData(false, true);
        expect(bindings.transaction.value.sourceAmountCents).toBe(67_890);
        expect(bindings.transaction.value.destinationAmountCents).toBe(12_345);
        bindings.swapTransactionData(false, false);
        bindings.swapTransactionData(true, true);
        expect(bindings.transaction.value.sourceAccountId).toBe('cash');
        expect(bindings.transaction.value.destinationAccountId).toBe('usd');
        expect(bindings.transaction.value.sourceAmountCents).toBe(12_345);
        expect(bindings.transaction.value.destinationAmountCents).toBe(67_890);
    });

    test('delegates picture URLs at the store boundary', () => {
        const bindings = setupBase();
        const picture = { pictureId: 'receipt', originalUrl: '/receipt.jpg' };
        expect(bindings.getTransactionPictureUrl(picture)).toBe('/pictures/receipt');
        expect(bindings.getTransactionPictureUrl(null)).toBeUndefined();
        expect(mockTransactionStore.getTransactionPictureUrl).toHaveBeenCalledWith(picture);
        expect(mockTransactionStore.getTransactionPictureUrl).toHaveBeenCalledWith(null);
    });
});

describe('TransactionEditPageBase production-loaded reactive cents and timezone flow', () => {
    test('passes unchanged old/new source cents to destination inference only while editable', async () => {
        const bindings = setupBase();
        bindings.transaction.value.sourceAmountCents = 1_234;
        await flushWatchers();
        expect(mockTransactionStore.setTransactionSuitableDestinationAmount).not.toHaveBeenCalled();

        bindings.loading.value = false;
        bindings.mode.value = TransactionEditPageMode.View;
        bindings.transaction.value.sourceAmountCents = 2_345;
        await flushWatchers();
        expect(mockTransactionStore.setTransactionSuitableDestinationAmount).not.toHaveBeenCalled();

        bindings.mode.value = TransactionEditPageMode.Edit;
        bindings.transaction.value.type = TransactionType.Transfer;
        bindings.transaction.value.sourceAmountCents = 3_456;
        await flushWatchers();
        expect(mockTransactionStore.setTransactionSuitableDestinationAmount).toHaveBeenLastCalledWith(
            bindings.transaction.value,
            2_345,
            3_456,
        );
    });

    test('copies destination cents to source only for editable expense and income transactions', async () => {
        const bindings = setupBase();
        bindings.loading.value = false;
        bindings.transaction.value.type = TransactionType.Expense;
        bindings.transaction.value.destinationAmountCents = 4_567;
        await flushWatchers();
        expect(bindings.transaction.value.sourceAmountCents).toBe(4_567);

        bindings.transaction.value.type = TransactionType.Income;
        bindings.transaction.value.destinationAmountCents = 5_678;
        await flushWatchers();
        expect(bindings.transaction.value.sourceAmountCents).toBe(5_678);

        bindings.transaction.value.type = TransactionType.Transfer;
        bindings.transaction.value.destinationAmountCents = 6_789;
        await flushWatchers();
        expect(bindings.transaction.value.sourceAmountCents).toBe(5_678);

        bindings.mode.value = TransactionEditPageMode.View;
        bindings.transaction.value.type = TransactionType.Expense;
        bindings.transaction.value.destinationAmountCents = 7_890;
        await flushWatchers();
        expect(bindings.transaction.value.sourceAmountCents).toBe(5_678);

        bindings.mode.value = TransactionEditPageMode.Edit;
        bindings.loading.value = true;
        bindings.transaction.value.destinationAmountCents = 8_901;
        await flushWatchers();
        expect(bindings.transaction.value.sourceAmountCents).toBe(5_678);
    });

    test('updates utc offset for known timezones and leaves unknown names unchanged', async () => {
        const bindings = setupBase();
        bindings.transaction.value.timeZone = 'UTC';
        await flushWatchers();
        expect(bindings.transaction.value.utcOffset).toBe(0);

        bindings.transaction.value.timeZone = 'Unknown/Zone';
        await flushWatchers();
        expect(bindings.transaction.value.utcOffset).toBe(0);
        bindings.transaction.value.timeZone = 'Asia/Shanghai';
        await flushWatchers();
        expect(bindings.transaction.value.utcOffset).toBe(480);
    });
});
