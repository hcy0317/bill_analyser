import { beforeEach, describe, expect, jest, test } from '@jest/globals';
const { nextTick, reactive } = jest.requireActual('vue') as typeof import('@/../node_modules/vue');

import { ChartDataAggregationType, ChartDateAggregationType } from '@/core/statistics.ts';
import { usePieChartBase, type CommonPieChartProps } from '@/components/base/PieChartBase.ts';
import { useTrendsChartBase } from '@/components/base/TrendsChartBase.ts';
import { useMonthlyTrendsChartBase } from '@/components/base/MonthlyTrendsChartBase.ts';
import { useAccountBalanceTrendsChartBase } from '@/components/base/AccountBalanceTrendsChartBase.ts';

type DateRange = { minUnixTime: number; maxUnixTime: number };

let mockDayRanges: DateRange[] = [];
let mockPeriodRanges: DateRange[] = [];
const mockGetAllDays = jest.fn((_start: number, _end: number) => mockDayRanges);
const mockGetRangesByYearMonth = jest.fn((_start: unknown, _end: unknown, _fiscal: number, _aggregation: number) => mockPeriodRanges);
const mockGetRangesFromItems = jest.fn((_items: unknown[], _start: string, _end: string, _fiscal: number, _aggregation: number) => mockPeriodRanges);

jest.mock('@/locales/helpers.ts', () => ({
    __esModule: true,
    useI18n: () => ({
        tt: (value: string) => `tt:${value}`,
        formatAmountToLocalizedNumeralsWithCurrency: (value: number, currency?: string) => `${currency ?? 'none'}:${value}`,
        formatPercentToLocalizedNumerals: (value: number, digits: number, tiny: string) => `${value.toFixed(digits)}%:${tiny}`,
        formatUnixTimeToShortDate: (value: number) => `D${value}`,
        formatUnixTimeToGregorianLikeShortYear: (value: number) => `Y${value}`,
        formatUnixTimeToGregorianLikeShortYearMonth: (value: number) => `M${value}`,
        formatUnixTimeToGregorianLikeYearQuarter: (value: number) => `Q${value}`,
        formatUnixTimeToGregorianLikeFiscalYear: (value: number) => `FY${value}`
    })
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

jest.mock('@/lib/numeral.ts', () => ({
    __esModule: true,
    sumAmounts: (values: number[]) => values.reduce((sum, value) => sum + value, 0)
}));

jest.mock('@/lib/datetime.ts', () => ({
    __esModule: true,
    getYearMonthDayDateTime: (year: number, month: number, day: number) => ({
        getUnixTime: () => year * 10_000 + month * 100 + day
    }),
    getGregorianCalendarYearAndMonthFromUnixTime: (value: number) => ({ year: Math.trunc(value / 100), month: value % 100 }),
    getAllDaysStartAndEndUnixTimes: (start: number, end: number) => mockGetAllDays(start, end),
    getYearFirstUnixTimeBySpecifiedUnixTime: () => 1_000,
    getQuarterFirstUnixTimeBySpecifiedUnixTime: () => 3_000,
    getMonthFirstUnixTimeBySpecifiedUnixTime: () => 4_000,
    getDayFirstUnixTimeBySpecifiedUnixTime: (value: number) => Math.trunc(value / 100) * 100,
    getFiscalYearStartUnixTime: () => 2_000
}));

jest.mock('@/lib/statistics.ts', () => ({
    __esModule: true,
    getAllDateRangesByYearMonthRange: (start: unknown, end: unknown, fiscal: number, aggregation: number) => mockGetRangesByYearMonth(start, end, fiscal, aggregation),
    getAllDateRangesFromItems: (items: unknown[], start: string, end: string, fiscal: number, aggregation: number) => mockGetRangesFromItems(items, start, end, fiscal, aggregation)
}));

describe('pie chart behavior', () => {
    test('filters, normalizes, colors, and formats visible slices', () => {
        const props: CommonPieChartProps = {
            items: [
                { code: 'a', label: 'Alpha', amount: -50, share: 12.5, color: '112233' },
                { code: 'b', label: 'Beta', amount: 30 },
                { code: 'c', label: 'Hidden', amount: 20, hidden: true },
                { code: 'd', label: 'Tiny', amount: 1 },
                { code: 'e', label: 'Zero', amount: 0 },
                { code: 'f', label: 'Invalid', amount: '10' }
            ],
            idField: 'code',
            nameField: 'label',
            valueField: 'amount',
            percentField: 'share',
            colorField: 'color',
            hiddenField: 'hidden',
            minValidPercent: 0.02,
            defaultCurrency: 'CNY'
        };
        const base = usePieChartBase(props);

        expect(base.validItems.value).toHaveLength(2);
        expect(base.validItems.value[0]).toMatchObject({
            id: 'a',
            name: 'a',
            displayName: 'Alpha',
            value: 50,
            percent: 12.5,
            actualPercent: 50 / 81,
            color: '#112233',
            displayPercent: '12.50%:&lt;0.01',
            displayValue: 'CNY:50'
        });
        expect(base.validItems.value[1]).toMatchObject({
            id: 'b',
            name: 'b',
            displayName: 'Beta',
            value: 30,
            percent: 30 / 81 * 100,
            displayValue: 'CNY:30'
        });
    });

    test('falls back to names/default colors and resets selection when items change', async () => {
        const props = reactive<CommonPieChartProps>({
            items: [{ name: 'Only', value: 10 }],
            nameField: 'name',
            valueField: 'value'
        });
        const base = usePieChartBase(props);

        expect(base.validItems.value[0]).toMatchObject({ id: 'Only', name: 'Only', percent: 100, actualPercent: 1 });
        expect(base.validItems.value[0]?.color).toMatch(/^#/);
        base.selectedIndex.value = 5;
        props.items = [{ name: 'Next', value: 20 }];
        await nextTick();
        expect(base.selectedIndex.value).toBe(0);
        expect(base.validItems.value[0]?.displayValue).toBe('none:20');
    });

    test('returns no slices when every source value is invalid or hidden', () => {
        const base = usePieChartBase({
            items: [{ name: 'Zero', value: 0 }, { name: 'Hidden', value: 2, hidden: true }],
            nameField: 'name',
            valueField: 'value',
            hiddenField: 'hidden'
        });
        expect(base.validItems.value).toEqual([]);
    });
});

describe('generic trends chart behavior', () => {
    beforeEach(() => {
        mockDayRanges = [{ minUnixTime: 10, maxUnixTime: 19 }];
        mockPeriodRanges = [{ minUnixTime: 20, maxUnixTime: 29 }];
        mockGetAllDays.mockClear();
        mockGetRangesByYearMonth.mockClear();
        mockGetRangesFromItems.mockClear();
    });

    test('uses explicit daily bounds for day aggregation', () => {
        const props = {
            chartMode: 'daily',
            items: [],
            startTime: 100,
            endTime: 200,
            startYearMonth: undefined,
            endYearMonth: undefined,
            fiscalYearStart: 257,
            sortingType: 0,
            dataAggregationType: ChartDataAggregationType.Sum,
            dateAggregationType: ChartDateAggregationType.Day.type,
            nameField: 'name',
            valueField: 'value',
            translateName: true
        };
        const base = useTrendsChartBase(props as never);

        expect(base.allDateRanges.value).toEqual(mockDayRanges);
        expect(mockGetAllDays).toHaveBeenCalledWith(100, 200);
        expect(base.getItemName('income')).toBe('tt:income');
    });

    test('derives daily bounds from nested items for non-day aggregation', () => {
        const props = {
            chartMode: 'daily',
            items: [{ items: [
                { year: 2024, month: 3, day: 5 },
                { year: 2023, month: 12, day: 31 },
                { year: 2025, month: 1, day: 1 }
            ] }],
            startTime: 0,
            endTime: 0,
            startYearMonth: undefined,
            endYearMonth: undefined,
            fiscalYearStart: 257,
            sortingType: 0,
            dataAggregationType: ChartDataAggregationType.Sum,
            dateAggregationType: ChartDateAggregationType.Month.type,
            nameField: 'name',
            valueField: 'value',
            translateName: false
        };
        const base = useTrendsChartBase(props as never);

        expect(base.allDateRanges.value).toEqual(mockPeriodRanges);
        expect(mockGetRangesByYearMonth).toHaveBeenCalledTimes(1);
        expect(base.getItemName('expense')).toBe('expense');

        const noDatedItems = useTrendsChartBase({
            ...props,
            dateAggregationType: ChartDateAggregationType.Day.type,
            items: [{ items: [] }]
        } as never);
        expect(noDatedItems.allDateRanges.value).toEqual(mockDayRanges);
    });

    test('keeps empty derived daily ranges valid and handles monthly/unknown modes', () => {
        const emptyDaily = useTrendsChartBase({
            chartMode: 'daily', items: [], startTime: 0, endTime: 0,
            startYearMonth: undefined, endYearMonth: undefined, fiscalYearStart: 257,
            sortingType: 0, dataAggregationType: ChartDataAggregationType.Sum,
            dateAggregationType: ChartDateAggregationType.Day.type, nameField: 'name', valueField: 'value'
        } as never);
        expect(emptyDaily.allDateRanges.value).toEqual(mockDayRanges);
        expect(mockGetAllDays).toHaveBeenCalledWith(0, 0);

        const monthly = useTrendsChartBase({
            chartMode: 'monthly', items: [{ items: [] }], startYearMonth: '2024-01', endYearMonth: '2024-12',
            startTime: undefined, endTime: undefined, fiscalYearStart: 257,
            sortingType: 0, dataAggregationType: ChartDataAggregationType.Sum,
            dateAggregationType: ChartDateAggregationType.Year.type, nameField: 'name', valueField: 'value'
        } as never);
        expect(monthly.allDateRanges.value).toEqual(mockPeriodRanges);
        expect(mockGetRangesFromItems).toHaveBeenCalledTimes(1);

        const unknown = useTrendsChartBase({ chartMode: 'weekly' } as never);
        expect(unknown.allDateRanges.value).toEqual([]);
    });

    test('monthly compatibility facade delegates ranges and translates names', () => {
        const props = {
            items: [{ items: [{ year: 2024, month: 1, value: 3 }] }],
            startYearMonth: '2024-01', endYearMonth: '2024-02', fiscalYearStart: 257,
            sortingType: 0, dateAggregationType: ChartDateAggregationType.Month.type,
            nameField: 'name', valueField: 'value', translateName: true
        };
        const base = useMonthlyTrendsChartBase(props as never);

        expect(base.allDateRanges.value).toEqual(mockPeriodRanges);
        expect(base.getItemName('cash')).toBe('tt:cash');
        props.translateName = false;
        expect(base.getItemName('cash')).toBe('cash');
    });
});

function reconciliation(time: number, opening: number, closing: number) {
    return {
        time,
        accountOpeningBalanceCents: opening,
        accountClosingBalanceCents: closing
    };
}

function account(kind: 'asset' | 'liability' | 'neutral') {
    return {
        name: kind,
        isAsset: kind === 'asset',
        isLiability: kind === 'liability'
    };
}

describe('account balance trends behavior', () => {
    beforeEach(() => {
        mockDayRanges = [];
        mockPeriodRanges = [];
        mockGetAllDays.mockClear();
        mockGetRangesByYearMonth.mockClear();
    });

    test('returns empty chart data when items are absent or have invalid times', () => {
        const empty = useAccountBalanceTrendsChartBase({
            items: undefined,
            dateAggregationType: ChartDateAggregationType.Day.type,
            fiscalYearStart: 257,
            account: account('asset') as never
        });
        expect(empty.allDateRanges.value).toEqual([]);
        expect(empty.allDataItems.value).toEqual([]);
        expect(empty.allDisplayDateRanges.value).toEqual([]);

        const invalid = useAccountBalanceTrendsChartBase({
            items: [reconciliation(0, 10, 20)] as never,
            dateAggregationType: ChartDateAggregationType.Day.type,
            fiscalYearStart: 257,
            account: account('asset') as never
        });
        expect(invalid.allDateRanges.value).toEqual([]);
        expect(invalid.allDataItems.value).toEqual([]);
    });

    test('sorts daily transactions and aggregates consecutive no-change days', () => {
        mockDayRanges = [
            { minUnixTime: 100, maxUnixTime: 199 },
            { minUnixTime: 200, maxUnixTime: 299 },
            { minUnixTime: 300, maxUnixTime: 399 },
            { minUnixTime: 400, maxUnixTime: 499 }
        ];
        const base = useAccountBalanceTrendsChartBase({
            items: [
                reconciliation(120, 1_100, 900),
                reconciliation(110, 1_000, 1_100),
                reconciliation(410, 900, 1_300)
            ] as never,
            dateAggregationType: ChartDateAggregationType.Day.type,
            fiscalYearStart: 257,
            account: account('asset') as never
        });

        expect(base.allDateRanges.value).toEqual(mockDayRanges);
        expect(base.allDataItems.value).toEqual([
            {
                displayDate: 'D100', openingBalanceCents: 1_000, closingBalanceCents: 900,
                minimumBalanceCents: 900, maximumBalanceCents: 1_100,
                medianBalanceCents: 900, averageBalanceCents: 1_000
            },
            {
                displayDate: 'D200', displayDateRange: 'D200 ~ D300',
                openingBalanceCents: 900, closingBalanceCents: 900,
                minimumBalanceCents: 900, maximumBalanceCents: 900,
                medianBalanceCents: 900, averageBalanceCents: 900
            },
            {
                displayDate: 'D400', openingBalanceCents: 900, closingBalanceCents: 1_300,
                minimumBalanceCents: 1_300, maximumBalanceCents: 1_300,
                medianBalanceCents: 1_300, averageBalanceCents: 1_300
            }
        ]);
        expect(base.allDisplayDateRanges.value).toEqual(['D100', 'D200 ~ D300', 'D400']);
    });

    test.each([
        [ChartDateAggregationType.Year.type, 1_000, 'Y1000'],
        [ChartDateAggregationType.FiscalYear.type, 2_000, 'FY2000'],
        [ChartDateAggregationType.Quarter.type, 3_000, 'Q3000'],
        [ChartDateAggregationType.Month.type, 4_000, 'M4000']
    ])('groups and labels period aggregation %s', (aggregation, rangeStart, label) => {
        mockPeriodRanges = [{ minUnixTime: rangeStart, maxUnixTime: rangeStart + 99 }];
        const base = useAccountBalanceTrendsChartBase({
            items: [reconciliation(555, 100, 150)] as never,
            dateAggregationType: aggregation,
            fiscalYearStart: 257,
            account: account('neutral') as never
        });

        expect(base.allDataItems.value).toHaveLength(1);
        expect(base.allDataItems.value[0]).toMatchObject({
            displayDate: label,
            openingBalanceCents: 100,
            closingBalanceCents: 150
        });
    });

    test('negates liability balances while preserving raw extrema relationships', () => {
        mockPeriodRanges = [{ minUnixTime: 4_000, maxUnixTime: 4_099 }];
        const base = useAccountBalanceTrendsChartBase({
            items: [reconciliation(510, 100, 200), reconciliation(520, 200, 300)] as never,
            dateAggregationType: ChartDateAggregationType.Month.type,
            fiscalYearStart: 257,
            account: account('liability') as never
        });

        expect(base.allDataItems.value[0]).toMatchObject({
            openingBalanceCents: -100,
            closingBalanceCents: -300,
            minimumBalanceCents: -200,
            maximumBalanceCents: -300,
            medianBalanceCents: -300,
            averageBalanceCents: -250
        });
    });

    test('fails closed for unsupported aggregation values', () => {
        mockPeriodRanges = [{ minUnixTime: 9_000, maxUnixTime: 9_099 }];
        const base = useAccountBalanceTrendsChartBase({
            items: [reconciliation(555, 100, 150)] as never,
            dateAggregationType: 999,
            fiscalYearStart: 257,
            account: account('asset') as never
        });
        expect(base.allDataItems.value).toEqual([]);
        expect(base.allDisplayDateRanges.value).toEqual([]);
    });
});
