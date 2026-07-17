/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockDatePickerApi = { switchView: jest.fn() };
const mockDropdown = { parentElement: { id: 'dropdown-parent' } };
const mockScrollToSelectedItem = jest.fn();
const mockGetFinalDateRange = jest.fn();
const mockCalendarAlternateDate = jest.fn();
const mockNumeralDefault = { id: 'default' };
const mockNumeralExplicit = { id: 'explicit' };
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

jest.mock('vue', () => ({
    ...actualVue,
    useTemplateRef: (name: string) => actualVue.ref(
        name === 'datetimepicker' ? mockDatePickerApi : mockDropdown
    )
}));

jest.mock('@vuepic/vue-datepicker', () => ({ __esModule: true, default: {} }));
jest.mock('vuetify', () => ({
    useTheme: () => ({ global: { name: actualVue.ref('dark') } })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({ currentUserFirstDayOfWeek: 1 })
}));
jest.mock('@/core/numeral.ts', () => ({
    NumeralSystem: {
        Default: mockNumeralDefault,
        valueOf: (value: number) => value === 2 ? mockNumeralExplicit : undefined
    }
}));
jest.mock('@/core/text.ts', () => ({
    TextDirection: { LTR: 'ltr', RTL: 'rtl' }
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, options?: unknown) => options ? `tt:${key}:${JSON.stringify(options)}` : `tt:${key}`,
        getAllMinWeekdayNames: () => ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'],
        getCurrentCalendarDisplayType: () => ({ secondaryCalendarType: 'lunar' }),
        getCurrentNumeralSystemType: () => mockNumeralDefault,
        isLongDateMonthAfterYear: () => true,
        isLongTime24HourFormat: () => false,
        getCalendarDisplayShortYearFromUnixTime: (value: number, numeral?: unknown) => `year:${value}:${(numeral as any)?.id ?? 'none'}`,
        getCalendarDisplayShortMonthFromUnixTime: (value: number, numeral?: unknown) => `month:${value}:${(numeral as any)?.id ?? 'none'}`,
        getCalendarDisplayDayOfMonthFromUnixTime: (value: number, numeral?: unknown) => `day:${value}:${(numeral as any)?.id ?? 'none'}`,
        getCalendarAlternateDate: (...args: unknown[]) => mockCalendarAlternateDate(...args),
        getCurrentLanguageTextDirection: () => 'ltr'
    })
}));
jest.mock('@/lib/common.ts', () => ({
    isDefined: (value: unknown) => value !== undefined && value !== null,
    isArray: Array.isArray,
    arrangeArrayWithNewStartIndex: (items: unknown[], index: number) => [...items.slice(index), ...items.slice(0, index)],
    arrayContainsFieldValue: (items: Array<Record<string, unknown>>, field: string, value: unknown) => (
        items.some(item => item[field] === value)
    )
}));
jest.mock('@/lib/datetime.ts', () => ({
    getAllowedYearRange: () => [1900, 2100],
    getYearMonthDayDateTime: (year: number, month: number, day: number) => ({
        getUnixTime: () => year * 10_000 + month * 100 + day
    }),
    getLocalDatetimeFromUnixTime: (value: number) => new Date(value * 1000),
    getDummyUnixTimeForLocalUsage: (value: number, timezone: number, browser: number) => value + timezone + browser,
    getTimezoneOffsetMinutes: () => 60,
    getBrowserTimezoneOffsetMinutes: () => -30,
    getYear0BasedMonthObjectFromUnixTime: () => ({ year: 2026, month0base: 6 }),
    getThisMonthFirstUnixTime: () => 123
}));
jest.mock('@/components/base/DateRangeSelectionBase.ts', () => ({
    useDateRangeSelectionBase: () => ({
        dateRange: actualVue.ref([new Date('2026-01-01T00:00:00Z'), new Date('2026-01-31T00:00:00Z')]),
        beginDateTime: actualVue.computed(() => 'begin'),
        endDateTime: actualVue.computed(() => 'end'),
        presetRanges: actualVue.computed(() => [{ label: 'month', range: [] }]),
        getFinalDateRange: () => mockGetFinalDateRange()
    })
}));
jest.mock('@/core/theme.ts', () => ({
    isDarkApplicationTheme: (name: string) => name === 'dark'
}));
jest.mock('@/lib/color.ts', () => ({
    getColorsInRows: (colors: string[], count: number) => {
        const rows: any[][] = [];
        for (let index = 0; index < colors.length; index += count) {
            rows.push(colors.slice(index, index + count).map(color => ({ id: color, color })));
        }
        return rows;
    },
    getDisplayColor: (value: unknown) => `display:${typeof value === 'object' ? (value as any).color : value}`
}));
jest.mock('@/lib/icon.ts', () => ({
    getIconsInRows: (icons: Record<string, unknown>, count: number) => {
        const values = Object.entries(icons).map(([id, value]) => ({ id, ...(value as object) }));
        const rows: any[][] = [];
        for (let index = 0; index < values.length; index += count) rows.push(values.slice(index, index + count));
        return rows;
    }
}));
jest.mock('@/lib/ui/desktop.ts', () => ({
    scrollToSelectedItem: (...args: unknown[]) => mockScrollToSelectedItem(...args)
}));
jest.mock('@mdi/js', () => ({ mdiSquareRounded: 'square', mdiCheck: 'check' }));

const DateTimePicker = require('@/components/common/DateTimePicker.vue').default as any;
const MonthPicker = require('@/components/common/MonthPicker.vue').default as any;
const AmountInputDialog = require('@/components/desktop/AmountInputDialog.vue').default as any;
const ColorSelect = require('@/components/desktop/ColorSelect.vue').default as any;
const DateRangeSelectionDialog = require('@/components/desktop/DateRangeSelectionDialog.vue').default as any;
const IconSelect = require('@/components/desktop/IconSelect.vue').default as any;
const MonthSelectionDialog = require('@/components/desktop/MonthSelectionDialog.vue').default as any;

interface Runtime {
    props: Record<string, any>;
    bindings: any;
    emit: jest.Mock;
    expose: jest.Mock;
}

function setup(component: any, input: Record<string, unknown>): Runtime {
    const props = actualVue.reactive(input) as Record<string, any>;
    const emit = jest.fn();
    const expose = jest.fn();
    const bindings = component.setup(props, { attrs: {}, slots: {}, emit, expose });
    return { props, bindings, emit, expose };
}

function render(component: any, runtime: Runtime): any {
    return component.render({}, [], runtime.props, actualVue.proxyRefs(runtime.bindings), {}, {});
}

beforeAll(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    consoleWarnSpy.mockRestore();
});

beforeEach(() => {
    jest.clearAllMocks();
    mockCalendarAlternateDate.mockReturnValue({ displayDate: 'Lunar 1' });
});

describe('DateTimePicker production behavior', () => {
    function props(overrides: Record<string, unknown> = {}): Record<string, unknown> {
        return {
            modelValue: new Date('2026-07-16T00:00:00Z'),
            datetimePickerClass: 'custom',
            isDarkMode: true,
            enableTimePicker: true,
            showAlternateDates: true,
            ...overrides
        };
    }

    test('derives calendar settings and formats scalar, range, null, and explicit numeral values', () => {
        const runtime = setup(DateTimePicker, props());
        expect(runtime.bindings.yearRange).toEqual([1900, 2100]);
        expect(runtime.bindings.dayNames.value).toEqual(['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su']);
        expect(runtime.bindings.firstDayOfWeek.value).toBe(1);
        expect(runtime.bindings.isYearFirst.value).toBe(true);
        expect(runtime.bindings.is24Hour.value).toBe(false);
        expect(runtime.bindings.alternateCalendarType.value).toBe('lunar');
        expect(runtime.bindings.actualNumeralSystem.value).toBe(mockNumeralDefault);
        expect(runtime.bindings.isDateRange.value).toBe(false);
        expect(runtime.bindings.getDisplayYear(2026)).toContain('20260101');
        expect(runtime.bindings.getDisplayMonth(6)).toContain('20260701');
        expect(runtime.bindings.getDisplayDay(new Date('2026-07-16T00:00:00Z'))).toContain('20260716');
        expect(runtime.bindings.getAlternateDate(new Date('2026-07-16T00:00:00Z'))).toBe('Lunar 1');
        runtime.bindings.dateTime.value = null;
        expect(runtime.emit).toHaveBeenCalledWith('update:modelValue', null);
        runtime.bindings.switchView('year');
        expect(mockDatePickerApi.switchView).toHaveBeenCalledWith('year');
        expect(runtime.expose).toHaveBeenCalledWith(expect.objectContaining({ switchView: expect.any(Function) }));

        const range = setup(DateTimePicker, props({
            modelValue: [new Date('2025-01-01T00:00:00Z'), new Date('2026-01-01T00:00:00Z')],
            numeralSystem: 2
        }));
        expect(range.bindings.actualNumeralSystem.value).toBe(mockNumeralExplicit);
        expect(range.bindings.isDateRange.value).toBe(true);
        expect(range.bindings.getDisplayMonth(0)).toContain('20250101');

        const invalidNumeral = setup(DateTimePicker, props({ numeralSystem: 9 }));
        expect(invalidNumeral.bindings.actualNumeralSystem.value).toBe(mockNumeralDefault);
        const empty = setup(DateTimePicker, props({ modelValue: null, showAlternateDates: false }));
        expect(empty.bindings.getAlternateDate(new Date())).toBeUndefined();
        expect(empty.bindings.getDisplayMonth(0)).toContain(`${new Date().getFullYear()}0101`);
    });

    test('executes all generated slots and alternate-date branches', () => {
        const runtime = setup(DateTimePicker, props());
        const vnode = render(DateTimePicker, runtime);
        expect(vnode.children.year({ value: 2026 })).toBeDefined();
        expect(vnode.children['year-overlay-value']({ value: 2026 })).toBeDefined();
        expect(vnode.children.month({ value: 6 })).toBeDefined();
        expect(vnode.children['month-overlay-value']({ value: 6 })).toBeDefined();
        expect(vnode.children.day({ date: new Date('2026-07-16T00:00:00Z') })).toBeDefined();
        const toggle = jest.fn();
        expect(vnode.children['am-pm-button']({ toggle, value: 'pm' })).toBeDefined();

        mockCalendarAlternateDate.mockReturnValue(undefined);
        expect(vnode.children.day({ date: new Date('2026-07-17T00:00:00Z') })).toBeDefined();
        runtime.props['noSwipeAndScroll'] = true;
        runtime.props['showAlternateDates'] = false;
        expect(render(DateTimePicker, runtime)).toBeDefined();
    });
});

describe('MonthPicker production behavior', () => {
    test('converts scalar and range models in both directions and executes slots', () => {
        const scalar = setup(MonthPicker, {
            modelValue: { year: 2026, month0base: 6 },
            monthPickerClass: 'month',
            isDarkMode: false,
            clearable: true
        });
        expect(scalar.bindings.dateTime.value).toEqual({ year: 2026, month: 6 });
        scalar.bindings.dateTime.value = { year: 2027, month: 2 };
        expect(scalar.emit).toHaveBeenCalledWith('update:modelValue', { year: 2027, month0base: 2 });
        expect(scalar.bindings.isDateRange.value).toBe(false);
        expect(scalar.bindings.getDisplayYear(2026)).toContain('20260101');
        expect(scalar.bindings.getDisplayMonth(0)).toContain('20260101');
        const vnode = render(MonthPicker, scalar);
        expect(vnode.children.year({ value: 2026 })).toBeDefined();
        expect(vnode.children['year-overlay-value']({ value: 2026 })).toBeDefined();
        expect(vnode.children.month({ value: 0 })).toBeDefined();
        expect(vnode.children['month-overlay-value']({ value: 0 })).toBeDefined();

        const range = setup(MonthPicker, {
            modelValue: [{ year: 2025, month0base: 0 }, { year: 2026, month0base: 1 }],
            isDarkMode: true
        });
        expect(range.bindings.dateTime.value).toEqual([
            { year: 2025, month: 0 }, { year: 2026, month: 1 }
        ]);
        range.bindings.dateTime.value = [{ year: 2024, month: 3 }, { year: 2025, month: 4 }];
        expect(range.emit).toHaveBeenCalledWith('update:modelValue', [
            { year: 2024, month0base: 3 }, { year: 2025, month0base: 4 }
        ]);
        expect(range.bindings.isDateRange.value).toBe(true);
        expect(range.bindings.getDisplayMonth(1)).toContain('20250201');
    });
});

describe('AmountInputDialog production behavior', () => {
    test('opens with full and default options and resolves or rejects actions', async () => {
        const runtime = setup(AmountInputDialog, {});
        const resolved = runtime.bindings.open({
            title: 'Amount', text: 'Body', textI18nOptions: { count: 2 },
            inputLabel: 'Value', inputPlaceholder: 'Enter', color: 'success', currency: 'CNY', initAmount: 42
        });
        expect(runtime.bindings.titleContent.value).toBe('tt:Amount');
        expect(runtime.bindings.textContent.value).toContain('tt:Body');
        expect(runtime.bindings.inputLabelContent.value).toBe('tt:Value');
        expect(runtime.bindings.inputPlaceholderContent.value).toBe('tt:Enter');
        expect(runtime.bindings.finalColor.value).toBe('success');
        expect(runtime.bindings.amount.value).toBe(42);
        runtime.bindings.confirm();
        await expect(resolved).resolves.toBe(42);
        expect(runtime.emit).toHaveBeenCalledWith('update:show', false);

        const rejected = runtime.bindings.open({});
        expect(runtime.bindings.titleContent.value).toBe('tt:global.app.title');
        expect(runtime.bindings.textContent.value).toBe('');
        expect(runtime.bindings.inputLabelContent.value).toBeUndefined();
        expect(runtime.bindings.inputPlaceholderContent.value).toBeUndefined();
        expect(runtime.bindings.finalColor.value).toBe('primary');
        expect(runtime.bindings.amount.value).toBe(0);
        runtime.bindings.cancel();
        await expect(rejected).rejects.toBeUndefined();
        expect(render(AmountInputDialog, runtime)).toBeDefined();

        const fresh = setup(AmountInputDialog, {});
        fresh.bindings.confirm();
        fresh.bindings.cancel();
        expect(fresh.emit).toHaveBeenCalledTimes(2);
    });
});

function selectionProps(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        modelValue: null,
        disabled: false,
        label: 'Select',
        columnCount: 2,
        allColorInfos: ['red', 'blue', 'green'],
        iconType: 'account',
        color: 'red',
        allIconInfos: {
            wallet: { icon: 'wallet' },
            card: { icon: 'card' },
            cash: { icon: 'cash' }
        },
        ...overrides
    };
}

describe('ColorSelect and IconSelect production behavior', () => {
    test.each([
        ['color', ColorSelect, 'red'],
        ['icon', IconSelect, 'wallet']
    ])('%s selector computes rows, selects values, scrolls, and renders slot states', async (_name, component, selected) => {
        const runtime = setup(component, selectionProps());
        const valueBinding = component === ColorSelect ? runtime.bindings.color : runtime.bindings.icon;
        const rowsBinding = component === ColorSelect ? runtime.bindings.allColorRows : runtime.bindings.allIconRows;
        const select = component === ColorSelect ? runtime.bindings.selectColor : runtime.bindings.selectIcon;
        expect(valueBinding.value).toBe('');
        expect(rowsBinding.value).toHaveLength(2);
        expect(runtime.bindings.hasSelectedIcon(rowsBinding.value[0])).toBe(false);
        select(selected);
        expect(runtime.emit).toHaveBeenCalledWith('update:modelValue', selected);
        expect(runtime.bindings.menuOpen.value).toBe(false);
        runtime.bindings.onMenuStateChanged(false);
        runtime.bindings.onMenuStateChanged(true);
        await actualVue.nextTick();
        expect(mockScrollToSelectedItem).toHaveBeenCalledWith(
            mockDropdown.parentElement, null, '.row-has-selected-item'
        );
        const vnode = render(component, runtime);
        expect(vnode.children.selection({ item: { raw: selected } })).toBeDefined();
        expect(vnode.children['no-data']()).toBeDefined();

        runtime.props['modelValue'] = selected;
        expect(runtime.bindings.hasSelectedIcon(rowsBinding.value[0])).toBe(true);
        expect(render(component, runtime).children['no-data']()).toBeDefined();
    });
});

describe('DateRangeSelectionDialog production behavior', () => {
    function props(overrides: Record<string, unknown> = {}): Record<string, unknown> {
        return {
            show: true,
            title: 'Range',
            hint: 'Hint',
            minTime: 10,
            maxTime: 20,
            ...overrides
        };
    }

    test('confirms, ignores empty ranges, reports Error only, cancels, watches, and renders', async () => {
        const runtime = setup(DateRangeSelectionDialog, props());
        expect(runtime.bindings.isDarkMode.value).toBe(true);
        expect(runtime.bindings.showState.value).toBe(true);
        runtime.bindings.showState.value = false;
        expect(runtime.emit).toHaveBeenCalledWith('update:show', false);

        mockGetFinalDateRange.mockReturnValue({ minUnixTime: 11, maxUnixTime: 22 });
        runtime.bindings.confirm();
        expect(runtime.emit).toHaveBeenCalledWith('dateRange:change', 11, 22);
        mockGetFinalDateRange.mockReturnValue(undefined);
        runtime.bindings.confirm();
        mockGetFinalDateRange.mockImplementation(() => { throw new Error('bad range'); });
        runtime.bindings.confirm();
        expect(runtime.emit).toHaveBeenCalledWith('error', 'bad range');
        mockGetFinalDateRange.mockImplementation(() => { throw 'ignored'; });
        runtime.bindings.confirm();
        runtime.bindings.cancel();
        expect(runtime.emit).toHaveBeenCalledWith('update:show', false);

        runtime.props['minTime'] = 100;
        runtime.props['maxTime'] = 200;
        await actualVue.nextTick();
        expect(runtime.bindings.dateRange.value[0]).toEqual(new Date(130_000));
        expect(runtime.bindings.dateRange.value[1]).toEqual(new Date(230_000));
        runtime.props['minTime'] = 0;
        runtime.props['maxTime'] = 0;
        await actualVue.nextTick();
        expect(render(DateRangeSelectionDialog, runtime)).toBeDefined();
    });
});

describe('MonthSelectionDialog production behavior', () => {
    test('validates month bounds, updates models from both watchers, and renders', async () => {
        const runtime = setup(MonthSelectionDialog, {
            modelValue: { year: 2025, month0base: 5 },
            title: 'Month',
            hint: 'Hint',
            show: false,
            persistent: true
        });
        expect(runtime.bindings.isDarkMode.value).toBe(true);
        expect(runtime.bindings.showState.value).toBe(false);
        runtime.bindings.showState.value = true;
        expect(runtime.emit).toHaveBeenCalledWith('update:show', true);

        runtime.bindings.monthValue.value = { year: 0, month0base: 0 };
        runtime.bindings.confirm();
        runtime.bindings.monthValue.value = { year: 2025, month0base: -1 };
        runtime.bindings.confirm();
        expect(runtime.emit).toHaveBeenCalledWith('error', 'Date is too early');
        runtime.bindings.monthValue.value = { year: 2026, month0base: 6 };
        runtime.bindings.confirm();
        expect(runtime.emit).toHaveBeenCalledWith('update:modelValue', { year: 2026, month0base: 6 });
        runtime.bindings.cancel();
        expect(runtime.emit).toHaveBeenCalledWith('update:show', false);

        runtime.props['modelValue'] = { year: 2027, month0base: 7 };
        await actualVue.nextTick();
        expect(runtime.bindings.monthValue.value).toEqual({ year: 2027, month0base: 7 });
        runtime.props['show'] = true;
        await actualVue.nextTick();
        expect(runtime.bindings.monthValue.value).toEqual({ year: 2027, month0base: 7 });
        runtime.props['modelValue'] = undefined;
        runtime.props['show'] = false;
        await actualVue.nextTick();
        runtime.props['show'] = true;
        await actualVue.nextTick();
        expect(render(MonthSelectionDialog, runtime)).toBeDefined();
    });
});
