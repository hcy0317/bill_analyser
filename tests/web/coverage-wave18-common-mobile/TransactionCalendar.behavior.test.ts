/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockLongWeekdays = ['Sun-long', 'Mon-long', 'Tue-long', 'Wed-long', 'Thu-long', 'Fri-long', 'Sat-long'];
const mockShortWeekdays = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'];
const mockAlternateDates = jest.fn<(...args: any[]) => any>();
const mockDisplayDay = jest.fn<(...args: any[]) => string>();
const mockFormatAmount = jest.fn<(...args: any[]) => string>();
const mockArrange = jest.fn<(...args: any[]) => any[]>();
const mockGetUnixTimeFromLocalDatetime = jest.fn<(...args: any[]) => number>();
const mockGetActualUnixTimeForStore = jest.fn<(...args: any[]) => number>();
const mockParseDateTimeFromUnixTime = jest.fn<(...args: any[]) => any>();
const mockGetYearMonthDayDateTime = jest.fn<(...args: any[]) => any>();
const mockUserStore = actualVue.reactive({ currentUserFirstDayOfWeek: 1 });
let mockParsedDay = 5;
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        getAllLongWeekdayNames: () => mockLongWeekdays,
        getAllShortWeekdayNames: () => mockShortWeekdays,
        getCalendarDisplayDayOfMonthFromUnixTime: (...args: any[]) => mockDisplayDay(...args),
        getCalendarAlternateDates: (...args: any[]) => mockAlternateDates(...args),
        formatAmountToLocalizedNumeralsWithCurrency: (...args: any[]) => mockFormatAmount(...args)
    })
}));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/consts/numeral.ts', () => ({ INCOMPLETE_AMOUNT_SUFFIX: '~incomplete' }));
jest.mock('@/lib/common.ts', () => ({
    arrangeArrayWithNewStartIndex: (...args: any[]) => mockArrange(...args)
}));
jest.mock('@/lib/datetime.ts', () => ({
    getTimezoneOffsetMinutes: () => 480,
    getBrowserTimezoneOffsetMinutes: () => 60,
    getUnixTimeFromLocalDatetime: (...args: any[]) => mockGetUnixTimeFromLocalDatetime(...args),
    getActualUnixTimeForStore: (...args: any[]) => mockGetActualUnixTimeForStore(...args),
    parseDateTimeFromUnixTime: (...args: any[]) => mockParseDateTimeFromUnixTime(...args),
    getYearMonthDayDateTime: (...args: any[]) => mockGetYearMonthDayDateTime(...args)
}));

const TransactionCalendar = require('@/components/common/TransactionCalendar.vue').default as any;

function props(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        modelValue: '2026-07-05',
        isDarkMode: false,
        defaultCurrency: 'CNY',
        minDate: new Date(2026, 6, 1),
        maxDate: new Date(2026, 6, 31),
        weekDayNameType: 'long',
        dailyTotalAmountsCents: {
            5: {
                incomeCents: 12_300,
                expenseCents: -4_500,
                incompleteIncome: true,
                incompleteExpense: false
            }
        },
        readonly: false,
        calendarClass: 'calendar-extra',
        dayHasTransactionClass: 'has-transaction',
        ...overrides
    };
}

function setup(input: Record<string, unknown>): {
    bindings: any;
    emit: jest.Mock;
    props: Record<string, unknown>;
} {
    const reactiveProps = actualVue.reactive(input);
    const emit = jest.fn();
    const bindings = TransactionCalendar.setup(reactiveProps, {
        attrs: {}, slots: {}, emit, expose: jest.fn()
    });
    return { bindings, emit, props: reactiveProps };
}

function render(runtime: ReturnType<typeof setup>): any {
    return TransactionCalendar.render(
        {}, [], runtime.props, actualVue.proxyRefs(runtime.bindings), {}, {}
    );
}

beforeAll(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    consoleWarnSpy.mockRestore();
});

beforeEach(() => {
    jest.clearAllMocks();
    mockParsedDay = 5;
    mockUserStore.currentUserFirstDayOfWeek = 1;
    mockAlternateDates.mockReturnValue([
        { year: 2026, month: 7, day: 5, displayDate: 'Alt 5' },
        { year: 2026, month: 7, day: 6, displayDate: 'Alt 6' }
    ]);
    mockArrange.mockImplementation((items: string[], start: number) => [
        ...items.slice(start), ...items.slice(0, start)
    ]);
    mockFormatAmount.mockImplementation((amount, currency) => `${currency}:${amount}`);
    mockGetUnixTimeFromLocalDatetime.mockReturnValue(100);
    mockGetActualUnixTimeForStore.mockReturnValue(200);
    mockParseDateTimeFromUnixTime.mockImplementation(() => ({
        getGregorianCalendarDay: () => mockParsedDay
    }));
    mockGetYearMonthDayDateTime.mockImplementation((year, month, day) => ({
        getUnixTime: () => year * 10_000 + month * 100 + day
    }));
    mockDisplayDay.mockImplementation(value => `day:${value}`);
});

describe('TransactionCalendar production-loaded state', () => {
    test('projects weekdays, alternate dates, model updates, day amounts, and timezone lookups', () => {
        const runtime = setup(props());
        expect(runtime.bindings.firstDayOfWeek.value).toBe(1);
        expect(runtime.bindings.dayNames.value).toEqual([
            'Mon-long', 'Tue-long', 'Wed-long', 'Thu-long', 'Fri-long', 'Sat-long', 'Sun-long'
        ]);
        expect(runtime.bindings.alternateDates.value).toEqual({
            '2026-7-5': 'Alt 5',
            '2026-7-6': 'Alt 6'
        });
        expect(mockAlternateDates).toHaveBeenCalledWith({ year: 2026, month1base: 7 });

        runtime.bindings.dateTime.value = '2026-07-06';
        expect(runtime.emit).toHaveBeenCalledWith('update:modelValue', '2026-07-06');
        expect(runtime.bindings.noTransactionInMonthDay(new Date(2026, 6, 5))).toBe(false);
        mockParsedDay = 6;
        expect(runtime.bindings.noTransactionInMonthDay(new Date(2026, 6, 6))).toBe(true);
        expect(mockGetActualUnixTimeForStore).toHaveBeenCalledWith(100, 480, 60);

        expect(runtime.bindings.getDisplayMonthTotalAmount(123, 'USD', '+', true))
            .toBe('+USD:123~incomplete');
        expect(runtime.bindings.getDisplayMonthTotalAmount(-45, false, '', false))
            .toBe('false:-45');
        expect(runtime.bindings.getDisplayDay(new Date(2026, 6, 5))).toBe('day:20260705');
    });

    test('covers invalid date inputs, absent locale calendars, short weekdays, and no amount map', () => {
        expect(setup(props({ modelValue: '' })).bindings.alternateDates.value).toBeUndefined();
        expect(setup(props({ modelValue: '2026-07' })).bindings.alternateDates.value).toBeUndefined();
        mockAlternateDates.mockReturnValueOnce(undefined);
        expect(setup(props()).bindings.alternateDates.value).toBeUndefined();

        mockUserStore.currentUserFirstDayOfWeek = 0;
        const short = setup(props({ weekDayNameType: 'short', dailyTotalAmountsCents: undefined }));
        expect(short.bindings.dayNames.value).toEqual(mockShortWeekdays);
        expect(short.bindings.noTransactionInMonthDay(new Date())).toBe(true);
    });

    test('executes populated and empty day-slot template branches', () => {
        const populated = setup(props());
        const root = render(populated);
        root.props['onUpdate:modelValue']('2026-07-08');
        expect(populated.emit).toHaveBeenCalledWith('update:modelValue', '2026-07-08');
        const daySlot = root.children.day as (value: { day: string; date: Date }) => unknown;
        expect(daySlot({ day: '5', date: new Date(2026, 6, 5) })).toBeDefined();

        const empty = setup(props({
            modelValue: '',
            dailyTotalAmountsCents: undefined,
            dayHasTransactionClass: undefined,
            calendarClass: undefined
        }));
        const emptyRoot = render(empty);
        expect(emptyRoot.children.day({ day: '7', date: new Date(2026, 6, 7) })).toBeDefined();
    });
});
