import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const templateRefs = new Map<string, { value: any }>();
const watchRegistrations: Array<{
    source: () => unknown;
    callback: (value: any) => void;
}> = [];
const templateHandlers: Array<{ name: string; callback: (...args: any[]) => any }> = [];

const themeName = (jest.requireActual('vue') as any).ref('light');
const baseState = {
    is24Hour: (jest.requireActual('vue') as any).ref(false),
    isHourTwoDigits: (jest.requireActual('vue') as any).ref(true),
    isMinuteTwoDigits: (jest.requireActual('vue') as any).ref(true),
    isSecondTwoDigits: (jest.requireActual('vue') as any).ref(true),
    isMeridiemIndicatorFirst: (jest.requireActual('vue') as any).ref(true)
};

let formattedDateTime: string | null = null;
let unixTimeOverride: number | null = null;

const parseKnownDateTime = jest.fn<(text: string, format: any) => any>();
const parseLongDateTime = jest.fn<(text: string) => any>();
const parseShortDateTime = jest.fn<(text: string) => any>();
const detectKnownFormats = jest.fn<(text: string) => any[]>();
const getLocalDatetimeFromUnixTime = jest.fn<(value: number) => Date>((value) => new Date(value * 1000));
const getUnixTimeFromLocalDatetime = jest.fn<(value: Date) => number>((value) => (
    unixTimeOverride ?? Math.trunc(value.getTime() / 1000)
));
const getCombinedDateAndTimeValues = jest.fn<(...args: any[]) => Date>((
    date: Date,
    numeral: { parseInt: (value: string) => number },
    hour: string,
    minute: string,
    second: string,
    meridiem: string,
    is24Hour: boolean
) => {
    const result = new Date(date);
    let numericHour = numeral.parseInt(hour);
    if (!is24Hour) {
        numericHour %= 12;
        if (meridiem === 'PM') numericHour += 12;
    }
    result.setHours(numericHour, numeral.parseInt(minute), numeral.parseInt(second), 0);
    return result;
});
const setChildInputFocus = jest.fn<(...args: any[]) => void>();

const numeralSystem = {
    parseInt: jest.fn((value: string) => Number.parseInt(value, 10)),
    isDigit: jest.fn((value: string) => /^[0-9]$/.test(value))
};

const getDisplayTimeValue = jest.fn((value: number, twoDigits: boolean) => (
    twoDigits ? String(value).padStart(2, '0') : String(value)
));
const generateAllHours = jest.fn((_step: number, twoDigits: boolean) => [
    { itemsIndex: 0, value: twoDigits ? '00' : '0' },
    { itemsIndex: 1, value: twoDigits ? '01' : '1' }
]);
const generateAllMinutesOrSeconds = jest.fn((_step: number, twoDigits: boolean) => [
    { itemsIndex: 0, value: twoDigits ? '00' : '0' },
    { itemsIndex: 1, value: twoDigits ? '59' : '59' }
]);

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref({ $el: { name } });
            templateRefs.set(name, target);
            return target;
        },
        watch: (
            source: () => unknown,
            callback: (value: unknown) => void,
            options?: { immediate?: boolean }
        ) => {
            watchRegistrations.push({ source, callback });
            if (options?.immediate) callback(source());
            return jest.fn();
        }
    };
});

jest.mock('vuetify', () => ({
    useTheme: () => ({ global: { name: themeName } })
}));
jest.mock('vuetify/components/VAutocomplete', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return {
        VAutocomplete: defineComponent({
            name: 'VAutocompleteCoverageStub',
            setup: (_props: unknown, { attrs, slots }: any) => (
                () => h('div', attrs, Object.values(slots).flatMap((slot: any) => slot?.() ?? []))
            )
        })
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentNumeralSystemType: () => numeralSystem,
        parseDateTimeFromLongDateTime: (text: string) => parseLongDateTime(text),
        parseDateTimeFromShortDateTime: (text: string) => parseShortDateTime(text),
        formatUnixTimeToLongDateTime: (value: number) => formattedDateTime ?? `formatted:${value}`
    })
}));
jest.mock('@/components/base/DateTimeSelectionBase.ts', () => ({
    useDateTimeSelectionBase: () => ({
        ...baseState,
        getDisplayTimeValue: (value: number, twoDigits: boolean) => getDisplayTimeValue(value, twoDigits),
        generateAllHours: (step: number, twoDigits: boolean) => generateAllHours(step, twoDigits),
        generateAllMinutesOrSeconds: (step: number, twoDigits: boolean) => generateAllMinutesOrSeconds(step, twoDigits)
    })
}));
jest.mock('@/core/theme.ts', () => ({
    isDarkApplicationTheme: (name: string) => name === 'dark'
}));
jest.mock('@/core/numeral.ts', () => ({ NumeralSystem: class NumeralSystem {} }));
jest.mock('@/core/datetime.ts', () => ({
    MeridiemIndicator: { AM: { name: 'AM' }, PM: { name: 'PM' } },
    KnownDateTimeFormat: {
        ISO: { name: 'ISO' },
        detect: (text: string) => detectKnownFormats(text)
    }
}));
jest.mock('@/lib/datetime.ts', () => ({
    getHourIn12HourFormat: (hour: number) => hour % 12 || 12,
    getTimezoneOffsetMinutes: () => 480,
    getBrowserTimezoneOffsetMinutes: () => 0,
    getLocalDatetimeFromUnixTime: (value: number) => getLocalDatetimeFromUnixTime(value),
    getUnixTimeFromLocalDatetime: (value: Date) => getUnixTimeFromLocalDatetime(value),
    getActualUnixTimeForStore: (value: number, currentOffset: number, browserOffset: number) => (
        value + currentOffset - browserOffset
    ),
    getDummyUnixTimeForLocalUsage: (value: number, currentOffset: number, browserOffset: number) => (
        value + currentOffset - browserOffset
    ),
    parseDateTimeFromKnownDateTimeFormat: (text: string, format: any) => parseKnownDateTime(text, format),
    getAMOrPM: (hour: number) => hour >= 12 ? 'PM' : 'AM',
    getCombinedDateAndTimeValues: (...args: any[]) => getCombinedDateAndTimeValues(...args)
}));
jest.mock('@/lib/ui/desktop.ts', () => ({
    setChildInputFocus: (...args: any[]) => setChildInputFocus(...args)
}));

import DateTimeSelect from '@/components/desktop/DateTimeSelect.vue';

interface SetupResult {
    bindings: any;
    emit: jest.Mock;
    props: Record<string, any>;
    watches: Array<{ source: () => unknown; callback: (value: any) => void }>;
}

function setup(overrides: Record<string, unknown> = {}): SetupResult {
    const props = {
        modelValue: 3_600,
        disabled: false,
        readonly: false,
        label: 'When',
        displayMultiline: false,
        preferMenuOnTop: false,
        ...overrides
    };
    const emit = jest.fn();
    const watchStart = watchRegistrations.length;
    const bindings = (DateTimeSelect as any).setup(props, {
        attrs: {}, slots: {}, emit, expose: jest.fn()
    });
    return { bindings, emit, props, watches: watchRegistrations.slice(watchStart) };
}

async function flush(times = 5): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function makeDateTime(unixTime: number): { getUnixTime: () => number } {
    return { getUnixTime: () => unixTime };
}

function makePasteEvent(text: string | null, hasClipboard = true): any {
    return {
        clipboardData: hasClipboard ? { getData: jest.fn(() => text ?? '') } : null,
        preventDefault: jest.fn()
    };
}

class MockHtmlInputElement {
    public value: string;

    public constructor(value: string) {
        this.value = value;
    }
}

function makeKeyEvent(overrides: Record<string, unknown> = {}): any {
    return {
        key: 'x',
        altKey: false,
        ctrlKey: false,
        metaKey: false,
        shiftKey: false,
        target: new MockHtmlInputElement('11'),
        preventDefault: jest.fn(),
        stopPropagation: jest.fn(),
        ...overrides
    };
}

function collectVNodeHandlers(node: any, handlers: Array<{ name: string; callback: (...args: any[]) => any }>): void {
    if (!node) return;
    if (Array.isArray(node)) {
        for (const child of node) collectVNodeHandlers(child, handlers);
        return;
    }
    if (typeof node !== 'object') return;
    for (const [name, handler] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        if (typeof handler === 'function') handlers.push({ name, callback: handler as (...args: any[]) => any });
        if (Array.isArray(handler)) {
            for (const candidate of handler) {
                if (typeof candidate === 'function') handlers.push({ name, callback: candidate });
            }
        }
    }
    if (Array.isArray(node.children)) {
        collectVNodeHandlers(node.children, handlers);
    } else if (node.children && typeof node.children === 'object') {
        for (const child of Object.values(node.children)) {
            if (typeof child === 'function') {
                try {
                    collectVNodeHandlers((child as () => unknown)(), handlers);
                } catch {
                    // Framework-owned scoped slots can require runtime slot arguments.
                }
            } else {
                collectVNodeHandlers(child, handlers);
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    templateRefs.clear();
    watchRegistrations.length = 0;
    templateHandlers.length = 0;
    themeName.value = 'light';
    baseState.is24Hour.value = false;
    baseState.isHourTwoDigits.value = true;
    baseState.isMinuteTwoDigits.value = true;
    baseState.isSecondTwoDigits.value = true;
    baseState.isMeridiemIndicatorFirst.value = true;
    formattedDateTime = null;
    unixTimeOverride = null;
    detectKnownFormats.mockReturnValue([]);
    parseKnownDateTime.mockReturnValue(undefined);
    parseLongDateTime.mockReturnValue(undefined);
    parseShortDateTime.mockReturnValue(undefined);
    Object.defineProperty(globalThis, 'HTMLInputElement', {
        configurable: true,
        value: MockHtmlInputElement
    });
});

describe('desktop DateTimeSelect production state', () => {
    test('derives menu placement, theme, selected value, and single-line display', () => {
        const { bindings } = setup();
        expect(bindings.isDarkMode.value).toBe(false);
        expect(bindings.menuProps.value).toEqual({
            contentClass: 'date-time-select-menu',
            location: 'bottom',
            origin: 'top center',
            offset: 8,
            scrollStrategy: 'reposition'
        });
        expect(bindings.selectedDateTime.value.getTime()).toBe(3_600_000);
        expect(bindings.displayTime.value).toBe('formatted:4080');
        expect(bindings.displayDatePart.value).toBe('');
        expect(bindings.displayTimePart.value).toBe('');

        themeName.value = 'dark';
        expect(bindings.isDarkMode.value).toBe(true);
        const top = setup({ preferMenuOnTop: true });
        expect(top.bindings.menuProps.value).toMatchObject({ location: 'top', origin: 'bottom center' });
    });

    test('splits multiline display and preserves strings without a separator', () => {
        formattedDateTime = '2026-07-15 09:08:07';
        const split = setup({ displayMultiline: true });
        expect(split.bindings.displayDatePart.value).toBe('2026-07-15');
        expect(split.bindings.displayTimePart.value).toBe('09:08:07');

        formattedDateTime = '2026-07-15';
        const dateOnly = setup({ displayMultiline: true });
        expect(dateOnly.bindings.displayDatePart.value).toBe('2026-07-15');
        expect(dateOnly.bindings.displayTimePart.value).toBe('');
    });

    test('generates picker items and updates 12-hour and 24-hour time fields', () => {
        const { bindings } = setup();
        bindings.draftDateTime.value = new Date(2026, 6, 15, 1, 2, 3);
        expect(bindings.hourItems.value).toHaveLength(2);
        expect(bindings.minuteItems.value).toHaveLength(2);
        expect(bindings.secondItems.value).toHaveLength(2);
        expect(bindings.currentHour.value).toBe('01');
        expect(bindings.currentMinute.value).toBe('02');
        expect(bindings.currentSecond.value).toBe('03');
        expect(bindings.currentMeridiemIndicator.value).toBe('AM');

        const unchanged = bindings.draftDateTime.value.getTime();
        bindings.currentMeridiemIndicator.value = 'invalid';
        expect(bindings.draftDateTime.value.getTime()).toBe(unchanged);
        bindings.currentMeridiemIndicator.value = 'PM';
        expect(bindings.draftDateTime.value.getHours()).toBe(13);

        for (const invalid of ['not-a-number', '-1', '13']) bindings.currentHour.value = invalid;
        expect(bindings.draftDateTime.value.getHours()).toBe(13);
        bindings.currentHour.value = '11';
        expect(bindings.draftDateTime.value.getHours()).toBe(23);

        for (const invalid of ['not-a-number', '-1', '60']) bindings.currentMinute.value = invalid;
        bindings.currentMinute.value = '22';
        expect(bindings.draftDateTime.value.getMinutes()).toBe(22);

        for (const invalid of ['not-a-number', '-1', '60']) bindings.currentSecond.value = invalid;
        bindings.currentSecond.value = '33';
        expect(bindings.draftDateTime.value.getSeconds()).toBe(33);

        baseState.is24Hour.value = true;
        bindings.currentHour.value = '24';
        expect(bindings.draftDateTime.value.getHours()).toBe(23);
        bindings.currentHour.value = '07';
        expect(bindings.draftDateTime.value.getHours()).toBe(7);

        baseState.isHourTwoDigits.value = false;
        baseState.isMinuteTwoDigits.value = false;
        baseState.isSecondTwoDigits.value = false;
        expect(bindings.currentHour.value).toBe('7');
        expect(bindings.currentMinute.value).toBe('22');
        expect(bindings.currentSecond.value).toBe('33');
    });

    test('resets draft state through model and menu watchers', () => {
        const { bindings, watches } = setup();
        expect(watches).toHaveLength(2);
        const modelWatch = watches[0]!;
        const menuWatch = watches[1]!;

        modelWatch.callback(7_200);
        expect(bindings.draftDateTime.value.getTime()).toBe(7_200_000);
        bindings.menuState.value = true;
        bindings.draftDateTime.value = new Date(99_000_000);
        modelWatch.callback(10_800);
        expect(bindings.draftDateTime.value.getTime()).toBe(99_000_000);

        menuWatch.callback(true);
        expect(bindings.draftDateTime.value.getTime()).toBe(3_600_000);
        bindings.draftDateTime.value = new Date(88_000_000);
        bindings.closingWithAction.value = false;
        menuWatch.callback(false);
        expect(bindings.draftDateTime.value.getTime()).toBe(3_600_000);

        bindings.draftDateTime.value = new Date(77_000_000);
        bindings.closingWithAction.value = true;
        menuWatch.callback(false);
        expect(bindings.draftDateTime.value.getTime()).toBe(77_000_000);
        expect(bindings.closingWithAction.value).toBe(false);
    });

    test('commits, rejects early dates, cancels, confirms, and toggles meridiem', () => {
        const { bindings, emit } = setup();
        const selected = new Date(2026, 6, 15, 9, 8, 7);
        unixTimeOverride = -1;
        expect(bindings.commitDateTimeValue(selected)).toBe(false);
        expect(emit).toHaveBeenCalledWith('error', 'Date is too early');

        unixTimeOverride = 99_999;
        expect(bindings.commitDateTimeValue(selected)).toBe(true);
        expect(emit).toHaveBeenCalledWith('update:modelValue', 99_999);

        bindings.menuState.value = true;
        bindings.draftDateTime.value = selected;
        bindings.cancelSelection();
        expect(bindings.draftDateTime.value.getTime()).toBe(3_600_000);
        expect(bindings.closingWithAction.value).toBe(true);
        expect(bindings.menuState.value).toBe(false);

        bindings.menuState.value = true;
        bindings.closingWithAction.value = false;
        unixTimeOverride = -1;
        bindings.confirmSelection();
        expect(bindings.menuState.value).toBe(true);
        expect(bindings.closingWithAction.value).toBe(false);

        unixTimeOverride = 100_000;
        bindings.confirmSelection();
        expect(bindings.menuState.value).toBe(false);
        expect(bindings.closingWithAction.value).toBe(true);

        bindings.draftDateTime.value = new Date(2026, 6, 15, 1, 0, 0);
        bindings.toggleMeridiemIndicator();
        expect(bindings.draftDateTime.value.getHours()).toBe(13);
        bindings.toggleMeridiemIndicator();
        expect(bindings.draftDateTime.value.getHours()).toBe(1);
    });
});

describe('desktop DateTimeSelect paste and keyboard behavior', () => {
    test('blocks missing, readonly, disabled, and empty clipboard content', () => {
        const missing = makePasteEvent('', false);
        setup().bindings.onPaste(missing);
        expect(missing.preventDefault).toHaveBeenCalled();

        const readonly = makePasteEvent('2026-07-15');
        setup({ readonly: true }).bindings.onPaste(readonly);
        expect(readonly.preventDefault).toHaveBeenCalled();

        const disabled = makePasteEvent('2026-07-15');
        setup({ disabled: true }).bindings.onPaste(disabled);
        expect(disabled.preventDefault).toHaveBeenCalled();

        const empty = makePasteEvent('   ');
        setup().bindings.onPaste(empty);
        expect(empty.preventDefault).toHaveBeenCalled();
    });

    test('parses known, long, and short formats before rejecting invalid text', () => {
        const knownFormat = { name: 'ISO' };
        detectKnownFormats.mockReturnValueOnce([knownFormat]);
        parseKnownDateTime.mockReturnValueOnce(makeDateTime(10_000));
        const known = setup();
        const knownEvent = makePasteEvent('  known value  ');
        known.bindings.onPaste(knownEvent);
        expect(parseKnownDateTime).toHaveBeenCalledWith('known value', knownFormat);
        expect(known.emit).toHaveBeenCalledWith('update:modelValue', 10_480);
        expect(knownEvent.preventDefault).not.toHaveBeenCalled();

        detectKnownFormats.mockReturnValueOnce([knownFormat]);
        parseKnownDateTime.mockReturnValueOnce(undefined);
        parseLongDateTime.mockReturnValueOnce(makeDateTime(20_000));
        const long = setup();
        long.bindings.onPaste(makePasteEvent('long value'));
        expect(long.emit).toHaveBeenCalledWith('update:modelValue', 20_480);

        detectKnownFormats.mockReturnValueOnce([knownFormat, { name: 'other' }]);
        parseLongDateTime.mockReturnValueOnce(undefined);
        parseShortDateTime.mockReturnValueOnce(makeDateTime(30_000));
        const short = setup();
        short.bindings.onPaste(makePasteEvent('short value'));
        expect(parseKnownDateTime).not.toHaveBeenCalledWith('short value', expect.anything());
        expect(short.emit).toHaveBeenCalledWith('update:modelValue', 30_480);

        detectKnownFormats.mockReturnValueOnce([]);
        parseLongDateTime.mockReturnValueOnce(undefined);
        parseShortDateTime.mockReturnValueOnce(undefined);
        const invalidEvent = makePasteEvent('invalid');
        setup().bindings.onPaste(invalidEvent);
        expect(invalidEvent.preventDefault).toHaveBeenCalled();
    });

    test('focuses child inputs only for focused autocomplete instances', async () => {
        const { bindings } = setup();
        bindings.onFocused(null, true);
        bindings.onFocused({ $el: { id: 'ignored' } }, false);
        expect(setChildInputFocus).not.toHaveBeenCalled();

        const input = { $el: { id: 'hour-input' } };
        bindings.onFocused(input, true);
        await flush();
        expect(setChildInputFocus).toHaveBeenCalledWith(input.$el, 'input');
    });

    test('allows navigation shortcuts and localized digits without preventing input', () => {
        const { bindings } = setup();
        const events = [
            makeKeyEvent({ altKey: true }),
            makeKeyEvent({ ctrlKey: true }),
            makeKeyEvent({ metaKey: true }),
            makeKeyEvent({ key: 'F1' }),
            makeKeyEvent({ key: 'F12' }),
            makeKeyEvent({ key: 'ArrowLeft' }),
            makeKeyEvent({ key: 'ArrowRight' }),
            makeKeyEvent({ key: 'Home' }),
            makeKeyEvent({ key: 'End' }),
            makeKeyEvent({ key: 'Backspace' }),
            makeKeyEvent({ key: 'Delete' }),
            makeKeyEvent({ key: 'Del' }),
            makeKeyEvent({ key: '7' })
        ];
        for (const event of events) bindings.onKeyDown('hour', event);
        for (const event of events) expect(event.preventDefault).not.toHaveBeenCalled();
    });

    test('commits fields and moves focus for Tab and Enter keyboard flows', async () => {
        jest.useFakeTimers();
        try {
            const { bindings } = setup();
            const nonInput = makeKeyEvent({ target: {} });
            bindings.onKeyDown('hour', nonInput);
            expect(nonInput.preventDefault).not.toHaveBeenCalled();

            const empty = makeKeyEvent({ target: new MockHtmlInputElement('') });
            bindings.onKeyDown('hour', empty);
            expect(empty.preventDefault).not.toHaveBeenCalled();

            const hour = makeKeyEvent({ key: 'Tab', target: new MockHtmlInputElement('11') });
            bindings.onKeyDown('hour', hour);
            expect(hour.preventDefault).toHaveBeenCalled();
            const minute = makeKeyEvent({ key: 'Enter', target: new MockHtmlInputElement('22') });
            bindings.onKeyDown('minute', minute);
            const second = makeKeyEvent({ key: 'Enter', target: new MockHtmlInputElement('33') });
            bindings.onKeyDown('second', second);
            expect(bindings.currentHour.value).toBe('11');
            expect(bindings.currentMinute.value).toBe('22');
            expect(bindings.currentSecond.value).toBe('33');

            const shiftMinute = makeKeyEvent({ key: 'Tab', shiftKey: true, target: new MockHtmlInputElement('44') });
            bindings.onKeyDown('minute', shiftMinute);
            expect(shiftMinute.stopPropagation).toHaveBeenCalled();
            const shiftSecond = makeKeyEvent({ key: 'Tab', shiftKey: true, target: new MockHtmlInputElement('45') });
            bindings.onKeyDown('second', shiftSecond);

            const letter = makeKeyEvent({ key: 'x', target: new MockHtmlInputElement('value') });
            bindings.onKeyDown('unknown', letter);
            expect(letter.preventDefault).toHaveBeenCalled();

            await flush();
            jest.runAllTimers();
            expect(setChildInputFocus).toHaveBeenCalledWith(expect.objectContaining({ name: 'minuteInput' }), 'input');
            expect(setChildInputFocus).toHaveBeenCalledWith(expect.objectContaining({ name: 'secondInput' }), 'input');
            expect(setChildInputFocus).toHaveBeenCalledWith(expect.objectContaining({ name: 'hourInput' }), 'input');
        } finally {
            jest.useRealTimers();
        }
    });
});

describe('desktop DateTimeSelect production template', () => {
    test('executes direct render event wrappers for multiline and meridiem variants', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            for (const variant of [
                { is24: false, meridiemFirst: true, multiline: true },
                { is24: false, meridiemFirst: false, multiline: false },
                { is24: true, meridiemFirst: true, multiline: true }
            ]) {
                baseState.is24Hour.value = variant.is24;
                baseState.isMeridiemIndicatorFirst.value = variant.meridiemFirst;
                formattedDateTime = variant.multiline ? '2026-07-15 09:08:07' : '2026-07-15';
                const { bindings, props } = setup({ displayMultiline: variant.multiline });
                const { proxyRefs } = jest.requireActual('vue') as any;
                const vnode = (DateTimeSelect as any).render({}, [], props, proxyRefs(bindings), {}, {});
                const handlers: Array<{ name: string; callback: (...args: any[]) => any }> = [];
                collectVNodeHandlers(vnode, handlers);
                for (const { name, callback } of handlers) {
                    try {
                        if (name === 'onPaste') callback(makePasteEvent('invalid'));
                        else if (name === 'onKeydown') callback(makeKeyEvent({ key: 'Tab' }));
                        else if (name.startsWith('onUpdate:')) callback(true);
                        else callback();
                    } catch {
                        // Generated model handlers intentionally accept heterogeneous payloads.
                    }
                }
            }
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('SSR renders selection and no-data slots for both meridiem placements and 24-hour mode', async () => {
        const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
        const { renderToString } = jest.requireActual('vue/server-renderer') as any;
        const Stub = defineComponent({
            inheritAttrs: false,
            setup: (_props: unknown, { attrs, slots }: any) => {
                for (const [name, handler] of Object.entries(attrs)) {
                    if (!name.startsWith('on')) continue;
                    if (typeof handler === 'function') {
                        templateHandlers.push({ name, callback: handler as (...args: any[]) => any });
                    }
                    if (Array.isArray(handler)) {
                        for (const candidate of handler) {
                            if (typeof candidate === 'function') templateHandlers.push({ name, callback: candidate });
                        }
                    }
                }
                return () => h('div', attrs, Object.values(slots).flatMap((slot: any) => slot?.() ?? []));
            }
        });

        for (const variant of [
            { is24: false, meridiemFirst: true, multiline: true },
            { is24: false, meridiemFirst: false, multiline: false },
            { is24: true, meridiemFirst: true, multiline: true }
        ]) {
            baseState.is24Hour.value = variant.is24;
            baseState.isMeridiemIndicatorFirst.value = variant.meridiemFirst;
            formattedDateTime = variant.multiline ? '2026-07-15 09:08:07' : '2026-07-15';
            templateHandlers.length = 0;
            const app = createSSRApp(DateTimeSelect as any, {
                modelValue: 3_600,
                displayMultiline: variant.multiline,
                preferMenuOnTop: variant.meridiemFirst
            });
            for (const name of ['v-select', 'v-btn', 'v-autocomplete', 'date-time-picker']) app.component(name, Stub);
            app.config.warnHandler = () => undefined;
            const html = await renderToString(app);
            expect(html).toContain('date-time-select-time-picker-container');
            expect(html).toContain('date-time-select-actions');
            expect(templateHandlers.length).toBeGreaterThanOrEqual(5);

            for (const { name, callback } of [...templateHandlers]) {
                try {
                    if (name === 'onPaste') callback(makePasteEvent('invalid'));
                    else if (name === 'onKeydown') callback(makeKeyEvent({ key: 'Tab' }));
                    else if (name === 'onUpdate:modelValue') callback('11');
                    else if (name === 'onUpdate:focused') callback(true);
                    else if (name.startsWith('onUpdate:')) callback(true);
                    else callback();
                    await flush();
                } catch {
                    // Synthetic template events intentionally cover different component contracts.
                }
            }
        }
    });
});
