import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockTransactionType = {
    ModifyBalance: 1,
    Income: 2,
    Expense: 3,
    Transfer: 4
} as const;

const mockKnownFileType = {
    CSV: 'csv',
    TSV: 'tsv'
} as const;

const mockStatisticsAnalysisType = {
    AssetTrends: 'asset-trends'
} as const;

const mockTt = jest.fn((key: string, params?: Record<string, unknown>) => (
    params?.['nickname'] ? `tt:${key}:${String(params['nickname'])}` : `tt:${key}`
));
const mockGetChartTypes = jest.fn(() => [{ type: 1, displayName: 'Balance' }]);
const mockGetAggregationTypes = jest.fn((_analysisType: unknown) => [{ type: 2, displayName: 'Month' }]);
const mockFormatLongDateTime = jest.fn((time: number, sourceOffset?: number, targetOffset?: number) => (
    `long-datetime:${time}:${String(sourceOffset)}:${String(targetOffset)}`
));
const mockFormatLongDate = jest.fn((time: number, sourceOffset?: number, targetOffset?: number) => (
    `long-date:${time}:${String(sourceOffset)}:${String(targetOffset)}`
));
const mockFormatShortTime = jest.fn((time: number, sourceOffset?: number, targetOffset?: number) => (
    `short-time:${time}:${String(sourceOffset)}:${String(targetOffset)}`
));
const mockFormatGregorianDateTime = jest.fn((time: number) => `gregorian:${time}`);
const mockFormatWesternAmount = jest.fn((amountCents: number) => `western:${amountCents}`);
const mockFormatLocalizedAmount = jest.fn(
    (amountCents: number, currency: string) => `localized:${currency}:${amountCents}`
);
const mockGetUtcOffset = jest.fn((offset: number) => (offset >= 0 ? `+${offset}` : String(offset)));
const mockGetTimezoneOffset = jest.fn((_timeZone: string) => 480);
const mockParseDateTime = jest.fn((time: number, sourceOffset: number, targetOffset: number) => ({
    getUnixTime: () => time + sourceOffset - targetOffset
}));
const mockReplaceAll = jest.fn((value: string, search: string, replacement: string) => (
    value.split(search).join(replacement)
));

const mockSettingsStore = (jest.requireActual('vue') as any).reactive({
    appSettings: { timeZone: 'Asia/Shanghai' }
});
const mockUserStore = (jest.requireActual('vue') as any).reactive({
    currentUserFirstDayOfWeek: 1,
    currentUserFiscalYearStart: 4,
    currentUserDefaultCurrency: 'CNY',
    currentUserNickname: ''
});
const mockAccountsStore = (jest.requireActual('vue') as any).reactive({
    allAccountsMap: {} as Record<string, any>
});
const mockCategoriesStore = (jest.requireActual('vue') as any).reactive({
    allTransactionCategoriesMap: {} as Record<string, any>
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: mockTt,
        getAllAccountBalanceTrendChartTypes: mockGetChartTypes,
        getAllStatisticsDateAggregationTypesWithShortName: mockGetAggregationTypes,
        formatUnixTimeToLongDateTime: mockFormatLongDateTime,
        formatUnixTimeToLongDate: mockFormatLongDate,
        formatUnixTimeToShortTime: mockFormatShortTime,
        formatUnixTimeToGregorianDefaultDateTime: mockFormatGregorianDateTime,
        formatAmountToWesternArabicNumeralsWithoutDigitGrouping: mockFormatWesternAmount,
        formatAmountToLocalizedNumeralsWithCurrency: mockFormatLocalizedAmount
    })
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoriesStore
}));
jest.mock('@/core/transaction.ts', () => ({ TransactionType: mockTransactionType }));
jest.mock('@/core/statistics.ts', () => ({ StatisticsAnalysisType: mockStatisticsAnalysisType }));
jest.mock('@/core/file.ts', () => ({ KnownFileType: mockKnownFileType }));
jest.mock('@/lib/common.ts', () => ({
    replaceAll: (...args: [string, string, string]) => mockReplaceAll(...args)
}));
jest.mock('@/lib/datetime.ts', () => ({
    getUtcOffsetByUtcOffsetMinutes: (offset: number) => mockGetUtcOffset(offset),
    getTimezoneOffsetMinutes: (timeZone: string) => mockGetTimezoneOffset(timeZone),
    parseDateTimeFromUnixTime: (time: number, sourceOffset: number, targetOffset: number) => (
        mockParseDateTime(time, sourceOffset, targetOffset)
    )
}));

import { useReconciliationStatementPageBase } from '@/views/base/accounts/ReconciliationStatementPageBase.ts';

function account(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'cash',
        name: 'Cash',
        currency: 'CNY',
        isLiability: false,
        balanceCents: 12_345,
        ...overrides
    };
}

function transaction(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'tx-1',
        type: mockTransactionType.Expense,
        categoryId: 'food',
        sourceAccountId: 'cash',
        destinationAccountId: '',
        sourceAmountCents: 12_345,
        destinationAmountCents: 0,
        accountClosingBalanceCents: 54_321,
        time: 1_700_000_000,
        utcOffset: 480,
        comment: 'Lunch, noodles\tshop',
        ...overrides
    };
}

function statements(overrides: Record<string, unknown> = {}): any {
    return {
        totalInflowsCents: 100_001,
        totalOutflowsCents: 20_002,
        netFlowCents: 79_999,
        openingBalanceCents: 12_345,
        closingBalanceCents: 92_344,
        transactions: [],
        ...overrides
    };
}

function setup(): ReturnType<typeof useReconciliationStatementPageBase> {
    return useReconciliationStatementPageBase();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockSettingsStore.appSettings.timeZone = 'Asia/Shanghai';
    mockUserStore.currentUserFirstDayOfWeek = 1;
    mockUserStore.currentUserFiscalYearStart = 4;
    mockUserStore.currentUserDefaultCurrency = 'CNY';
    mockUserStore.currentUserNickname = '';
    mockAccountsStore.allAccountsMap = {
        cash: account(),
        bank: account({ id: 'bank', name: 'Bank', currency: 'USD' }),
        card: account({ id: 'card', name: 'Credit Card', currency: 'EUR', isLiability: true }),
        emptyName: account({ id: 'emptyName', name: '', currency: 'JPY' })
    };
    mockCategoriesStore.allTransactionCategoriesMap = {
        food: { id: 'food', name: 'Food' },
        blank: { id: 'blank', name: '' }
    };
    mockGetTimezoneOffset.mockReturnValue(480);
});

describe('reconciliation statement base refs and computed contracts', () => {
    test('exposes default refs, user settings, filter options, maps, dates, and fallback account state', () => {
        const base = setup();
        expect(base.accountId.value).toBe('');
        expect(base.startTime.value).toBe(0);
        expect(base.endTime.value).toBe(0);
        expect(base.reconciliationStatements.value).toBeUndefined();
        expect(base.firstDayOfWeek.value).toBe(1);
        expect(base.fiscalYearStart.value).toBe(4);
        expect(base.currentTimezoneOffsetMinutes.value).toBe(480);
        expect(base.defaultCurrency.value).toBe('CNY');
        expect(base.allChartTypes.value).toStrictEqual([{ type: 1, displayName: 'Balance' }]);
        expect(base.allDateAggregationTypes.value).toStrictEqual([{ type: 2, displayName: 'Month' }]);
        expect(mockGetAggregationTypes).toHaveBeenCalledWith(mockStatisticsAnalysisType.AssetTrends);
        expect(base.allAccountsMap.value).toBe(mockAccountsStore.allAccountsMap);
        expect(base.allCategoriesMap.value).toBe(mockCategoriesStore.allTransactionCategoriesMap);
        expect(base.currentAccount.value).toBeUndefined();
        expect(base.currentAccountCurrency.value).toBe('CNY');
        expect(base.isCurrentLiabilityAccount.value).toBe(false);

        base.startTime.value = 100;
        base.endTime.value = 200;
        expect(base.displayStartDateTime.value).toBe('long-datetime:100:undefined:undefined');
        expect(base.displayEndDateTime.value).toBe('long-datetime:200:undefined:undefined');
    });

    test('reacts to account, timezone, user, and nickname changes', () => {
        const base = setup();
        base.accountId.value = 'card';
        expect(base.currentAccount.value).toBe(mockAccountsStore.allAccountsMap['card']);
        expect(base.currentAccountCurrency.value).toBe('EUR');
        expect(base.isCurrentLiabilityAccount.value).toBe(true);

        mockUserStore.currentUserFirstDayOfWeek = 0;
        mockUserStore.currentUserFiscalYearStart = 7;
        mockUserStore.currentUserDefaultCurrency = 'USD';
        mockSettingsStore.appSettings.timeZone = 'UTC';
        mockGetTimezoneOffset.mockReturnValue(0);
        expect(base.firstDayOfWeek.value).toBe(0);
        expect(base.fiscalYearStart.value).toBe(7);
        expect(base.defaultCurrency.value).toBe('USD');
        expect(base.currentTimezoneOffsetMinutes.value).toBe(0);

        expect(base.exportFileName.value).toBe('tt:dataExport.defaultExportReconciliationStatementsFileName');
        mockUserStore.currentUserNickname = 'Synthetic User';
        expect(base.exportFileName.value).toBe(
            'tt:dataExport.exportReconciliationStatementsFileName:Synthetic User'
        );
    });

    test('formats summary balances from integer cents for asset and liability accounts', () => {
        const base = setup();
        expect(base.displayTotalInflows.value).toBe('localized:CNY:0');
        expect(base.displayTotalOutflows.value).toBe('localized:CNY:0');
        expect(base.displayTotalBalance.value).toBe('localized:CNY:0');
        expect(base.displayOpeningBalance.value).toBe('localized:CNY:0');
        expect(base.displayClosingBalance.value).toBe('localized:CNY:0');

        base.accountId.value = 'cash';
        base.reconciliationStatements.value = statements();
        expect(base.displayTotalInflows.value).toBe('localized:CNY:100001');
        expect(base.displayTotalOutflows.value).toBe('localized:CNY:20002');
        expect(base.displayTotalBalance.value).toBe('localized:CNY:79999');
        expect(base.displayOpeningBalance.value).toBe('localized:CNY:12345');
        expect(base.displayClosingBalance.value).toBe('localized:CNY:92344');

        base.accountId.value = 'card';
        expect(base.displayOpeningBalance.value).toBe('localized:EUR:-12345');
        expect(base.displayClosingBalance.value).toBe('localized:EUR:-92344');
        expect(mockFormatLocalizedAmount).toHaveBeenCalledWith(12_345, 'CNY');
        expect(mockFormatLocalizedAmount).not.toHaveBeenCalledWith(123.45, expect.anything());
        expect(mockFormatLocalizedAmount).not.toHaveBeenCalledWith(1_234_500, expect.anything());
    });
});

describe('reconciliation statement row display contracts', () => {
    test('names every transaction type and transfer direction', () => {
        const base = setup();
        base.accountId.value = 'cash';
        const cases = [
            [transaction({ type: mockTransactionType.ModifyBalance }), 'tt:Modify Balance'],
            [transaction({ type: mockTransactionType.Income }), 'tt:Income'],
            [transaction({ type: mockTransactionType.Expense }), 'tt:Expense'],
            [transaction({ type: mockTransactionType.Transfer, destinationAccountId: 'cash' }), 'tt:Transfer In'],
            [transaction({ type: mockTransactionType.Transfer, sourceAccountId: 'cash', destinationAccountId: 'bank' }), 'tt:Transfer Out'],
            [transaction({ type: mockTransactionType.Transfer, sourceAccountId: 'bank', destinationAccountId: 'card' }), 'tt:Transfer'],
            [transaction({ type: 999 }), 'tt:Unknown']
        ] as const;
        for (const [item, expected] of cases) {
            expect(base.getDisplayTransactionType(item)).toBe(expected);
        }
    });

    test('formats row date, time, timezone, and source or destination currency', () => {
        const base = setup();
        const item = transaction({ sourceAccountId: 'bank', destinationAccountId: 'card', utcOffset: -300 });
        expect(base.getDisplayDateTime(item)).toBe('long-datetime:1700000000:-300:480');
        expect(base.getDisplayDate(item)).toBe('long-date:1700000000:-300:480');
        expect(base.getDisplayTime(item)).toBe('short-time:1700000000:-300:480');
        expect(base.getDisplayTimezone(item)).toBe('UTC-300');
        expect(base.getDisplaySourceAmount(item)).toBe('localized:USD:12345');
        expect(base.getDisplayDestinationAmount(item)).toBe('localized:EUR:0');

        const missing = transaction({ sourceAccountId: 'missing', destinationAccountId: 'missing', sourceAmountCents: -500, destinationAmountCents: 700 });
        expect(base.getDisplaySourceAmount(missing)).toBe('localized:CNY:-500');
        expect(base.getDisplayDestinationAmount(missing)).toBe('localized:CNY:700');
    });

    test('formats closing balances for incoming transfers, source accounts, liabilities, and missing accounts', () => {
        const base = setup();
        base.accountId.value = 'card';
        expect(base.getDisplayAccountBalance(transaction({
            type: mockTransactionType.Transfer,
            sourceAccountId: 'cash',
            destinationAccountId: 'card',
            accountClosingBalanceCents: 12_345
        }))).toBe('localized:EUR:-12345');

        base.accountId.value = 'cash';
        expect(base.getDisplayAccountBalance(transaction({
            type: mockTransactionType.Transfer,
            sourceAccountId: 'bank',
            destinationAccountId: 'cash',
            accountClosingBalanceCents: 22_222
        }))).toBe('localized:CNY:22222');
        expect(base.getDisplayAccountBalance(transaction({
            type: mockTransactionType.Expense,
            sourceAccountId: 'card',
            accountClosingBalanceCents: 33_333
        }))).toBe('localized:EUR:-33333');
        expect(base.getDisplayAccountBalance(transaction({
            type: mockTransactionType.Expense,
            sourceAccountId: 'bank',
            accountClosingBalanceCents: 44_444
        }))).toBe('localized:USD:44444');
        expect(base.getDisplayAccountBalance(transaction({
            type: mockTransactionType.Expense,
            sourceAccountId: 'missing',
            accountClosingBalanceCents: 55_555
        }))).toBe('localized:CNY:55555');
    });
});

describe('reconciliation statement export contracts', () => {
    test('exports CSV headers and empty state for an asset account', () => {
        const base = setup();
        base.accountId.value = 'cash';
        const exported = base.getExportedData(mockKnownFileType.CSV as any);
        expect(exported).toBe([
            'tt:Transaction Time',
            'tt:Type',
            'tt:Category',
            'tt:Amount',
            'tt:Account',
            'tt:Account Balance',
            'tt:Description'
        ].join(',') + '\n');
    });

    test('exports CSV rows, sanitizes commas, and preserves integer cents through display formatting', () => {
        const base = setup();
        base.accountId.value = 'cash';
        base.reconciliationStatements.value = statements({
            transactions: [
                transaction(),
                transaction({
                    id: 'modify',
                    type: mockTransactionType.ModifyBalance,
                    categoryId: 'missing',
                    sourceAccountId: 'missing',
                    comment: '',
                    sourceAmountCents: -9_876,
                    accountClosingBalanceCents: -1_234
                })
            ]
        });

        const exported = base.getExportedData(mockKnownFileType.CSV as any);
        expect(exported).toContain('gregorian:1700000000,tt:Expense,Food,western:12345,Cash,western:54321,Lunch  noodles\tshop');
        expect(exported).toContain('tt:Modify Balance,tt:Modify Balance,western:-9876,,western:-1234,');
        expect(mockFormatWesternAmount).toHaveBeenCalledWith(12_345);
        expect(mockFormatWesternAmount).not.toHaveBeenCalledWith(123.45);
        expect(mockFormatWesternAmount).not.toHaveBeenCalledWith(1_234_500);
        expect(mockReplaceAll).toHaveBeenCalledWith('Lunch, noodles\tshop', ',', ' ');
    });

    test('exports liability TSV rows, incoming destination amounts, transfer accounts, and tab-safe descriptions', () => {
        const base = setup();
        base.accountId.value = 'card';
        base.reconciliationStatements.value = statements({
            transactions: [transaction({
                type: mockTransactionType.Transfer,
                sourceAccountId: 'cash',
                destinationAccountId: 'card',
                destinationAmountCents: 67_890,
                accountClosingBalanceCents: 45_678,
                comment: 'Card\tpayment'
            })]
        });

        const exported = base.getExportedData(mockKnownFileType.TSV as any);
        expect(exported.split('\n')[0]).toContain('tt:Account Outstanding Balance');
        expect(exported).toContain('tt:Transfer In\tFood\twestern:67890\tCash → Credit Card\twestern:-45678\tCard payment');
        expect(mockReplaceAll).toHaveBeenCalledWith('Card\tpayment', '\t', ' ');
    });

    test('uses comma fallback for unknown file types and handles blank category, account, and description fields', () => {
        const base = setup();
        base.accountId.value = 'cash';
        base.reconciliationStatements.value = statements({
            transactions: [transaction({
                type: mockTransactionType.Transfer,
                categoryId: 'blank',
                sourceAccountId: 'missing',
                destinationAccountId: 'emptyName',
                comment: undefined,
                destinationAmountCents: 777
            })]
        });

        const exported = base.getExportedData('unknown' as any);
        expect(exported).toContain('tt:Transfer,,western:12345, → ,western:54321,');
        expect(mockReplaceAll).not.toHaveBeenCalled();
    });
});
