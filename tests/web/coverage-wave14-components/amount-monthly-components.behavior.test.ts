/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const { proxyRefs, reactive } = actualVue;

const mockShowToast = jest.fn();
const mockUpdateTransactionFilter = jest.fn();
const mockUpdateTransactionInvalid = jest.fn();
const mockLoggerWarn = jest.fn();
const mockIsString = jest.fn((value: unknown) => typeof value === 'string');
const mockGetColors = jest.fn<(expense: string, income: string, dark?: boolean) => {
    incomeAmountColor: string;
    expenseAmountColor: string;
}>(() => ({
    incomeAmountColor: '#00aa00',
    expenseAmountColor: '#cc0000',
}));

const amountFilterTypes = {
    GreaterThan: { type: 'gt', name: 'Greater Than', paramCount: 1 },
    LessThan: { type: 'lt', name: 'Less Than', paramCount: 1 },
    Between: { type: 'bt', name: 'Between', paramCount: 2 },
    NotBetween: { type: 'nb', name: 'Not Between', paramCount: 2 },
    Equal: { type: 'eq', name: 'Equal', paramCount: 1 },
};
const allAmountFilterTypes = Object.values(amountFilterTypes);
const mockSettingsStore = reactive({ appSettings: { showAmountInHomePage: true } });
const mockUserStore = reactive({
    currentUserDefaultCurrency: 'CNY',
    currentUserExpenseAmountColor: 'red',
    currentUserIncomeAmountColor: 'green',
});
let mockTextDirection = 'ltr';

jest.mock('vue', () => ({ ...actualVue }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, params?: Record<string, unknown>) => params ? `${key}:${JSON.stringify(params)}` : `tt:${key}`,
        formatAmountToLocalizedNumeralsWithCurrency: (value: unknown, currency = 'CNY') => `${currency}:${String(value)}`,
        getCurrentLanguageTextDirection: () => mockTextDirection,
        formatUnixTimeToGregorianLikeShortMonth: (value: number) => `month:${value}`,
    }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast }),
}));
jest.mock('@/stores/transaction.ts', () => ({
    useTransactionsStore: () => ({
        updateTransactionListFilter: (...args: unknown[]) => mockUpdateTransactionFilter(...args),
        updateTransactionListInvalidState: (...args: unknown[]) => mockUpdateTransactionInvalid(...args),
    }),
}));
jest.mock('@/core/numeral.ts', () => ({
    AmountFilterType: {
        ...amountFilterTypes,
        values: () => allAmountFilterTypes,
        valueOf: (value: string) => allAmountFilterTypes.find(item => item.type === value),
    },
}));
jest.mock('@/consts/transaction.ts', () => ({
    TRANSACTION_MIN_AMOUNT: -999_999,
    TRANSACTION_MAX_AMOUNT: 999_999,
}));
jest.mock('@/lib/common.ts', () => ({ isString: (value: unknown) => mockIsString(value) }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { warn: (...args: unknown[]) => mockLoggerWarn(...args) },
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/core/text.ts', () => ({ TextDirection: { LTR: 'ltr', RTL: 'rtl' } }));
jest.mock('@/core/transaction.ts', () => ({
    TransactionType: { Income: 1, Expense: 2 },
}));
jest.mock('@/consts/numeral.ts', () => ({
    DISPLAY_HIDDEN_AMOUNT: '***',
    INCOMPLETE_AMOUNT_SUFFIX: '~',
}));
jest.mock('@/lib/ui/common.ts', () => ({
    getExpenseAndIncomeAmountColor: (expense: string, income: string, dark?: boolean) => mockGetColors(expense, income, dark),
}));

const AmountFilterPage = require('@/views/mobile/transactions/AmountFilterPage.vue').default as any;
const MonthlyIncomeAndExpenseCard = require('@/views/desktop/overview/cards/MonthlyIncomeAndExpenseCard.vue').default as any;

function setup(component: any, props: Record<string, unknown>): { bindings: any; emit: jest.Mock; props: any } {
    const emit = jest.fn();
    const reactiveProps = reactive(props);
    const bindings = component.setup(reactiveProps, {
        attrs: {}, slots: {}, emit, expose: jest.fn(),
    });
    return { bindings, emit, props: reactiveProps };
}

function render(component: any, bindings: any): unknown {
    return component.render({}, [], {}, proxyRefs(bindings), {}, {});
}

function amountProps(query: Record<string, unknown> = {}): {
    props: Record<string, unknown>;
    router: { back: jest.Mock };
} {
    const router = { back: jest.fn() };
    return {
        props: { f7route: { query }, f7router: router },
        router,
    };
}

function monthlyData(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        monthStartTime: 1_700_000_000,
        incomeAmountCents: 12_000,
        expenseAmountCents: 8_000,
        incompleteIncomeAmount: false,
        incompleteExpenseAmount: false,
        ...overrides,
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockIsString.mockImplementation((value: unknown) => typeof value === 'string');
    mockUpdateTransactionFilter.mockReturnValue(true);
    mockSettingsStore.appSettings.showAmountInHomePage = true;
    mockTextDirection = 'ltr';
});

describe('AmountFilterPage production behavior', () => {
    test.each([
        ['gt', 'gt:123', 123, 0],
        ['bt', 'bt:100:300', 100, 300],
        ['nb', 'nb:-200:500', -200, 500],
        ['gt', 'gt:100:extra', 0, 0],
        ['bt', 'bt:100', 0, 0],
    ])('initializes %s from query value %s', (type, value, expected1, expected2) => {
        const { props } = amountProps({ type, value });
        const { bindings } = setup(AmountFilterPage, props);
        expect(bindings.type.value).toBe(type);
        expect(bindings.amount1.value).toBe(expected1);
        expect(bindings.amount2.value).toBe(expected2);
        expect(render(AmountFilterPage, bindings)).toBeDefined();
    });

    test('handles missing/non-string values and logs unexpected split failures', () => {
        const missing = setup(AmountFilterPage, amountProps({}).props).bindings;
        expect(missing.type.value).toBe('');
        expect(missing.amount1.value).toBe(0);

        const badValue = { split: () => { throw new Error('split failed'); } };
        mockIsString.mockReturnValueOnce(true);
        const broken = setup(AmountFilterPage, amountProps({ type: 'gt', value: badValue }).props).bindings;
        expect(broken.amount1.value).toBe(0);
        expect(mockLoggerWarn).toHaveBeenCalledWith(
            expect.stringContaining('cannot parse amount from filter value'),
            expect.any(Error),
        );
    });

    test('projects parameter counts and amount headers for every filter family', () => {
        const state = setup(AmountFilterPage, amountProps({ type: 'gt' }).props).bindings;
        expect(state.getAmountFilterParameterCount('missing')).toBe(0);
        expect(state.amountCount.value).toBe(1);
        expect(state.amount1Header.value).toBe('tt:Minimum Amount');

        state.type.value = 'lt';
        expect(state.amount1Header.value).toBe('tt:Maximum Amount');
        state.type.value = 'eq';
        expect(state.amount1Header.value).toBe('tt:Amount');
        state.type.value = 'bt';
        expect(state.amountCount.value).toBe(2);
        expect(state.amount1Header.value).toBe('tt:Minimum Amount');
        expect(state.amount2Header.value).toBe('tt:Maximum Amount');
        state.type.value = 'nb';
        expect(state.amount2Header.value).toBe('tt:Maximum Amount');
        state.type.value = 'missing';
        expect(state.amount2Header.value).toBe('tt:Amount');
    });

    test('applies one-value filters and invalidates only changed transaction state', () => {
        const { props, router } = amountProps({ type: 'gt', value: 'gt:123' });
        const { bindings } = setup(AmountFilterPage, props);
        bindings.confirm();
        expect(mockUpdateTransactionFilter).toHaveBeenCalledWith({ amountFilterCents: 'gt:123' });
        expect(mockUpdateTransactionInvalid).toHaveBeenCalledWith(true);
        expect(router.back).toHaveBeenCalledTimes(1);

        jest.clearAllMocks();
        mockUpdateTransactionFilter.mockReturnValue(false);
        const unchanged = amountProps({ type: 'lt', value: 'lt:50' });
        setup(AmountFilterPage, unchanged.props).bindings.confirm();
        expect(mockUpdateTransactionInvalid).not.toHaveBeenCalled();
        expect(unchanged.router.back).toHaveBeenCalledTimes(1);
    });

    test('rejects reversed ranges, applies valid ranges, and backs out of invalid types', () => {
        const reversed = amountProps({ type: 'bt', value: 'bt:300:100' });
        setup(AmountFilterPage, reversed.props).bindings.confirm();
        expect(mockShowToast).toHaveBeenCalledWith('Incorrect amount range');
        expect(mockUpdateTransactionFilter).not.toHaveBeenCalled();
        expect(reversed.router.back).not.toHaveBeenCalled();

        jest.clearAllMocks();
        const valid = amountProps({ type: 'nb', value: 'nb:100:300' });
        setup(AmountFilterPage, valid.props).bindings.confirm();
        expect(mockUpdateTransactionFilter).toHaveBeenCalledWith({ amountFilterCents: 'nb:100:300' });
        expect(valid.router.back).toHaveBeenCalledTimes(1);

        jest.clearAllMocks();
        const invalid = amountProps({ type: 'missing', value: '' });
        setup(AmountFilterPage, invalid.props).bindings.confirm();
        expect(mockUpdateTransactionFilter).not.toHaveBeenCalled();
        expect(invalid.router.back).toHaveBeenCalledTimes(1);
    });
});

describe('MonthlyIncomeAndExpenseCard production behavior', () => {
    function cardProps(overrides: Record<string, unknown> = {}): Record<string, unknown> {
        return {
            loading: false,
            data: [monthlyData()],
            disabled: false,
            isDarkMode: false,
            enableClickItem: true,
            ...overrides,
        };
    }

    test('detects empty, zero-only, positive, and negative datasets', () => {
        const state = setup(MonthlyIncomeAndExpenseCard, cardProps({ data: [] }));
        expect(state.bindings.hasAnyData.value).toBe(false);
        state.props.data = [monthlyData({ incomeAmountCents: 0, expenseAmountCents: 0 })];
        expect(state.bindings.hasAnyData.value).toBe(false);
        state.props.data = [monthlyData({ incomeAmountCents: -1, expenseAmountCents: 0 })];
        expect(state.bindings.hasAnyData.value).toBe(true);
        state.props.data = [monthlyData({ incomeAmountCents: 0, expenseAmountCents: -1 })];
        expect(state.bindings.hasAnyData.value).toBe(true);
        state.props.data = [monthlyData({ incomeAmountCents: 1, expenseAmountCents: 0 })];
        expect(state.bindings.hasAnyData.value).toBe(true);
        state.props.data = [monthlyData({ incomeAmountCents: 0, expenseAmountCents: 1 })];
        expect(state.bindings.hasAnyData.value).toBe(true);
    });

    test('builds light LTR chart axes, extrema, colors, and signed series', () => {
        const data = [
            monthlyData({ monthStartTime: 1, incomeAmountCents: 10_000, expenseAmountCents: 4_000 }),
            monthlyData({ monthStartTime: 2, incomeAmountCents: -2_000, expenseAmountCents: -12_000 }),
        ];
        const { bindings } = setup(MonthlyIncomeAndExpenseCard, cardProps({ data }));
        const options = bindings.chartOptions.value as any;
        expect(options.xAxis[0]).toMatchObject({ data: ['month:1', 'month:2'], inverse: false });
        expect(options.yAxis[0].min).toBeLessThan(-2_000);
        expect(options.yAxis[0].max).toBe(12_000);
        expect(options.series[0].data).toEqual([10_000, -2_000]);
        expect(options.series[1].data).toEqual([-4_000, 12_000]);
        expect(options.series[0].itemStyle.color).toBe('#00aa00');
        expect(options.series[1].itemStyle.color).toBe('#cc0000');
        expect(mockGetColors).toHaveBeenCalledWith('red', 'green', false);
        expect(render(MonthlyIncomeAndExpenseCard, bindings)).toBeDefined();
    });

    test('builds dark RTL tooltip and formats visible, hidden, and incomplete amounts', () => {
        mockTextDirection = 'rtl';
        const data = [monthlyData({
            monthStartTime: 3,
            incomeAmountCents: 500,
            expenseAmountCents: 200,
            incompleteIncomeAmount: true,
            incompleteExpenseAmount: false,
        })];
        const { bindings } = setup(MonthlyIncomeAndExpenseCard, cardProps({ data, isDarkMode: true }));
        const options = bindings.chartOptions.value as any;
        expect(options.xAxis[0].inverse).toBe(true);
        expect(options.tooltip).toMatchObject({
            backgroundColor: '#333', borderColor: '#333', textStyle: { color: '#eee' },
        });
        expect(bindings.getDisplayIncomeAmount(data[0])).toBe('CNY:500~');
        expect(bindings.getDisplayExpenseAmount(data[0])).toBe('CNY:200');

        const tooltip = options.tooltip.formatter([
            { dataIndex: 0, seriesId: 'seriesIncome', name: 'March' },
            { dataIndex: 0, seriesId: 'seriesExpense', name: 'March' },
            { dataIndex: 0, seriesId: 'unknown', name: 'March' },
        ]);
        expect(tooltip).toContain('March');
        expect(tooltip).toContain('CNY:500~');
        expect(tooltip).toContain('CNY:200');

        mockSettingsStore.appSettings.showAmountInHomePage = false;
        expect(bindings.getDisplayIncomeAmount(data[0])).toBe('CNY:***');
    });

    test('omits absent tooltip series and tolerates an empty parameter list', () => {
        const { bindings } = setup(MonthlyIncomeAndExpenseCard, cardProps());
        const formatter = (bindings.chartOptions.value as any).tooltip.formatter;
        expect(formatter([{ dataIndex: 0, seriesId: 'seriesIncome', name: 'Only income' }]))
            .toContain('Only income');
        expect(formatter([{ dataIndex: 0, seriesId: 'seriesExpense', name: 'Only expense' }]))
            .toContain('Only expense');
        expect(formatter([])).toContain('<tbody></tbody>');
    });

    test('emits income/expense clicks and ignores disabled, non-series, missing, and unknown clicks', () => {
        const { bindings, emit, props } = setup(MonthlyIncomeAndExpenseCard, cardProps());
        bindings.clickItem({ componentType: 'series', dataIndex: 0, seriesId: 'seriesIncome' });
        bindings.clickItem({ componentType: 'series', dataIndex: 0, seriesId: 'seriesExpense' });
        expect(emit.mock.calls).toEqual([
            ['click', { transactionType: 1, monthStartTime: 1_700_000_000 }],
            ['click', { transactionType: 2, monthStartTime: 1_700_000_000 }],
        ]);

        props.enableClickItem = false;
        bindings.clickItem({ componentType: 'series', dataIndex: 0, seriesId: 'seriesIncome' });
        props.enableClickItem = true;
        bindings.clickItem({ componentType: 'axis', dataIndex: 0, seriesId: 'seriesIncome' });
        bindings.clickItem({ componentType: 'series', dataIndex: 99, seriesId: 'seriesIncome' });
        bindings.clickItem({ componentType: 'series', dataIndex: 0, seriesId: 'unknown' });
        expect(emit).toHaveBeenCalledTimes(2);
    });

    test('renders loading, empty, and populated production template states', () => {
        const state = setup(MonthlyIncomeAndExpenseCard, cardProps({ loading: true, data: [] }));
        expect(render(MonthlyIncomeAndExpenseCard, state.bindings)).toBeDefined();
        state.props.loading = false;
        expect(render(MonthlyIncomeAndExpenseCard, state.bindings)).toBeDefined();
        state.props.data = [monthlyData()];
        expect(render(MonthlyIncomeAndExpenseCard, state.bindings)).toBeDefined();
    });
});
