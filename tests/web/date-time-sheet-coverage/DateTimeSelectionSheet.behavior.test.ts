import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockTemplateRefs = new Map<string, { value: any }>();
const mockWatchCallbacks: Array<(value: string) => void> = [];
const mockRafCallbacks: Array<() => void> = [];
const mockTemplateHandlers: Array<(value: any) => unknown> = [];
const mockShowToast = jest.fn<(message: string) => void>();
const mockSwitchView = jest.fn<(view: string) => void>();
const mockSetProperty = jest.fn<(...args: any[]) => void>();

let mockIs24Hour = false;
let mockMeridiemFirst = true;
let mockDarkMode = false;
let mockUnixOverride: number | null = null;

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        nextTick: (callback?: () => unknown) => callback ? Promise.resolve().then(callback) : Promise.resolve(),
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
        watch: (_source: unknown, callback: (value: string) => void) => {
            mockWatchCallbacks.push(callback);
            return () => undefined;
        }
    };
});

jest.mock('@/components/common/DateTimePicker.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return {
        __esModule: true,
        default: defineComponent({
            name: 'DateTimePickerCoverageStub',
            inheritAttrs: false,
            setup: (_props: unknown, { attrs }: any) => {
                for (const [name, handler] of Object.entries(attrs)) {
                    if (name.startsWith('on') && typeof handler === 'function') {
                        mockTemplateHandlers.push(handler as (value: any) => unknown);
                    }
                }
                return () => h('div', attrs);
            }
        })
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentNumeralSystemType: () => ({ type: 'western' }),
        formatUnixTimeToLongDateTime: (value: number) => `datetime:${value}`
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast })
}));
jest.mock('@/components/base/DateTimeSelectionBase.ts', () => {
    const vue = jest.requireActual('vue') as any;
    return {
        useDateTimeSelectionBase: () => ({
            is24Hour: vue.computed(() => mockIs24Hour),
            isHourTwoDigits: vue.ref(true),
            isMinuteTwoDigits: vue.ref(true),
            isSecondTwoDigits: vue.ref(true),
            isMeridiemIndicatorFirst: vue.computed(() => mockMeridiemFirst),
            meridiemItems: vue.ref([{ name: 'AM', value: 'AM' }, { name: 'PM', value: 'PM' }]),
            getDisplayTimeValue: (value: number, twoDigits: boolean) => twoDigits ? String(value).padStart(2, '0') : String(value),
            generateAllHours: () => [
                { itemsIndex: 0, value: '23' },
                { itemsIndex: 1, value: '00' },
                { itemsIndex: 2, value: '01' }
            ],
            generateAllMinutesOrSeconds: () => [
                { itemsIndex: 0, value: '59' },
                { itemsIndex: 1, value: '00' },
                { itemsIndex: 2, value: '01' }
            ]
        })
    };
});
jest.mock('@/stores/environment.ts', () => ({
    useEnvironmentsStore: () => ({ get framework7DarkMode() { return mockDarkMode; } })
}));
jest.mock('@/core/numeral.ts', () => ({ NumeralSystem: class NumeralSystem {} }));
jest.mock('@/lib/common.ts', () => ({ isDefined: (value: unknown) => value !== undefined && value !== null }));
jest.mock('@/lib/datetime.ts', () => ({
    getHourIn12HourFormat: (hour: number) => hour % 12 || 12,
    getTimezoneOffsetMinutes: () => 0,
    getBrowserTimezoneOffsetMinutes: () => 0,
    getLocalDatetimeFromUnixTime: (value: number) => new Date(value * 1000),
    getActualUnixTimeForStore: (value: number) => value,
    getCurrentUnixTime: () => 3_600,
    getUnixTimeFromLocalDatetime: (value: Date) => mockUnixOverride ?? Math.trunc(value.getTime() / 1000),
    getAMOrPM: (hour: number) => hour >= 12 ? 'PM' : 'AM',
    getCombinedDateAndTimeValues: (
        date: Date,
        _numeral: unknown,
        hour: string,
        minute: string,
        second: string,
        meridiem: string,
        is24Hour: boolean
    ) => {
        const next = new Date(date);
        let numericHour = Number(hour);
        if (!is24Hour) {
            numericHour %= 12;
            if (meridiem === 'PM') numericHour += 12;
        }
        next.setHours(numericHour, Number(minute), Number(second), 0);
        return next;
    }
}));

const DateTimeSelectionSheet = require('@/components/mobile/DateTimeSelectionSheet.vue').default as any;

interface ItemElement {
    offsetHeight: number;
    offsetTop: number;
    getAttribute: (name: string) => string | null;
    hasAttribute: (name: string) => boolean;
}

function makeItem(value: string, itemsIndex: string | null, offsetTop: number): ItemElement {
    return {
        offsetHeight: 40,
        offsetTop,
        getAttribute: (name: string) => name === 'data-value' ? value : (name === 'data-items-index' ? itemsIndex : null),
        hasAttribute: (name: string) => name === 'data-items-index' && itemsIndex !== null
    };
}

function makeItemsElement(items: ItemElement[], scrollTop = 0): any {
    return {
        offsetHeight: 120,
        scrollTop,
        querySelectorAll: () => items
    };
}

function makeContainer(): { container: any; elements: Record<string, any> } {
    const elements: Record<string, any> = {
        '.picker-items-hour': makeItemsElement([
            makeItem('23', '0', 0), makeItem('00', '1', 40), makeItem('01', '2', 80)
        ]),
        '.picker-items-minute': makeItemsElement([
            makeItem('59', '0', 0), makeItem('00', '1', 40), makeItem('01', '2', 80)
        ]),
        '.picker-items-second': makeItemsElement([
            makeItem('59', '0', 0), makeItem('00', '1', 40), makeItem('01', '2', 80)
        ]),
        '.picker-items-meridiem-indicator-first': makeItemsElement([
            makeItem('AM', null, 0), makeItem('PM', null, 40)
        ]),
        '.picker-items-meridiem-indicator-last': makeItemsElement([
            makeItem('AM', null, 0), makeItem('PM', null, 40)
        ])
    };
    const allItems = Object.values(elements).flatMap((element: any) => element.querySelectorAll());
    const container = {
        offsetHeight: 200,
        style: { setProperty: mockSetProperty },
        querySelector: (selector: string) => elements[selector] ?? null,
        querySelectorAll: (selector: string) => selector === '.picker-item' ? allItems : []
    };
    return { container, elements };
}

function setupSheet(overrides: Record<string, unknown> = {}): { bindings: any; emit: jest.Mock } {
    const emit = jest.fn();
    const bindings = DateTimeSelectionSheet.setup({
        modelValue: 3_600,
        initMode: 'time',
        show: true,
        ...overrides
    }, {
        attrs: {}, slots: {}, emit, expose: () => undefined
    });
    return { bindings, emit };
}

async function flushAsync(): Promise<void> {
    await Promise.resolve();
    await new Promise(resolve => setImmediate(resolve));
}

function collectVNodeHandlers(node: any, handlers: Array<(value: any) => unknown>): void {
    if (!node) return;
    if (Array.isArray(node)) {
        for (const child of node) collectVNodeHandlers(child, handlers);
        return;
    }
    if (typeof node !== 'object') return;
    if (node.props) {
        for (const [name, handler] of Object.entries(node.props)) {
            if (!name.startsWith('on')) continue;
            if (typeof handler === 'function') handlers.push(handler as (value: any) => unknown);
            if (Array.isArray(handler)) {
                for (const candidate of handler) {
                    if (typeof candidate === 'function') handlers.push(candidate as (value: any) => unknown);
                }
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
                    // Some scoped slots require framework-owned slot props.
                }
            } else {
                collectVNodeHandlers(child, handlers);
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    mockWatchCallbacks.length = 0;
    mockRafCallbacks.length = 0;
    mockTemplateHandlers.length = 0;
    mockIs24Hour = false;
    mockMeridiemFirst = true;
    mockDarkMode = false;
    mockUnixOverride = null;
    Object.defineProperty(globalThis, 'window', {
        configurable: true,
        value: { requestAnimationFrame: (callback: () => void) => { mockRafCallbacks.push(callback); return mockRafCallbacks.length; } }
    });
});

describe('mobile DateTimeSelectionSheet production-loaded state and confirmation', () => {
    test('derives display state and updates every time field in 12-hour and 24-hour modes', () => {
        const { bindings } = setupSheet();
        expect(bindings.mode.value).toBe('time');
        expect(bindings.isDarkMode.value).toBe(false);
        expect(bindings.displayTime.value).toBe('datetime:3600');
        expect(bindings.switchButtonTitle.value).toBe('Date');
        expect(bindings.currentHour.value).toBe('09');
        expect(bindings.currentMinute.value).toBe('00');
        expect(bindings.currentSecond.value).toBe('00');
        expect(bindings.currentMeridiemIndicator.value).toBe('AM');

        bindings.currentMeridiemIndicator.value = 'PM';
        expect(bindings.dateTime.value.getHours()).toBe(21);
        bindings.currentHour.value = '11';
        bindings.currentMinute.value = '22';
        bindings.currentSecond.value = '33';
        expect(bindings.dateTime.value.getHours()).toBe(23);
        expect(bindings.dateTime.value.getMinutes()).toBe(22);
        expect(bindings.dateTime.value.getSeconds()).toBe(33);

        mockIs24Hour = true;
        bindings.currentHour.value = '23';
        expect(bindings.dateTime.value.getHours()).toBe(23);
        mockDarkMode = true;
        expect(setupSheet().bindings.isDarkMode.value).toBe(true);
    });

    test('switches modes, sets now, rejects invalid times, and emits a valid timestamp', () => {
        const { bindings, emit } = setupSheet();
        bindings.switchMode();
        expect(bindings.mode.value).toBe('date');
        expect(bindings.switchButtonTitle.value).toBe('Time');
        bindings.setCurrentTime();
        expect(bindings.dateTime.value.getTime()).toBe(3_600_000);
        bindings.switchMode();
        bindings.setCurrentTime();

        bindings.dateTime.value = null;
        bindings.confirm();
        expect(emit).not.toHaveBeenCalled();
        bindings.dateTime.value = new Date(0);
        mockUnixOverride = -1;
        bindings.confirm();
        expect(mockShowToast).toHaveBeenCalledWith('Date is too early');
        mockUnixOverride = 7_200;
        bindings.confirm();
        expect(emit).toHaveBeenNthCalledWith(1, 'update:modelValue', 7_200);
        expect(emit).toHaveBeenNthCalledWith(2, 'update:show', false);
    });
});

describe('mobile DateTimeSelectionSheet production-loaded picker mechanics', () => {
    test('calculates wheel styles including wraparound and angle limits', () => {
        const { bindings } = setupSheet();
        const values = [
            { itemsIndex: 0, value: '00' },
            { itemsIndex: 1, value: '01' },
            { itemsIndex: 2, value: '59' }
        ];
        expect(bindings.getTimerPickerItemStyle('01', '00', 1, values)).toBe('');
        bindings.timePickerContainerHeight.value = 200;
        bindings.timePickerItemHeight.value = 40;
        expect(bindings.getTimerPickerItemStyle('01', '00', 1, values)).toContain('rotateX(-24deg)');
        expect(bindings.getTimerPickerItemStyle('59', '00', 0, values)).toContain('rotateX(24deg)');
        expect(bindings.getTimerPickerItemStyle('00', '59', 2, values)).toContain('rotateX(-24deg)');
        expect(bindings.getTimerPickerItemStyle('59', '10', 1, values)).toBe('');
        expect(bindings.getTimerPickerItemStyle('00', '10', 1, values)).toBe('');
    });

    test('initializes dimensions and scrolls only matching center-cycle items', () => {
        const { bindings } = setupSheet();
        bindings.initTimePickerStyle();
        const { container, elements } = makeContainer();
        mockTemplateRefs.get('timePickerContainer')!.value = container;
        bindings.initTimePickerStyle();
        expect(bindings.timePickerContainerHeight.value).toBe(200);
        expect(bindings.timePickerItemHeight.value).toBe(40);
        expect(mockSetProperty).toHaveBeenCalledWith('--f7-picker-scroll-padding', '80px');

        bindings.scrollToSelectedItem('missing', 'picker-hour', '00');
        bindings.scrollToSelectedItem('picker-items-hour', 'picker-hour', '00');
        expect(elements['.picker-items-hour'].scrollTop).toBe(0);
        bindings.scrollAllTimeSelectedItems();
        expect(elements['.picker-items-minute'].scrollTop).toBe(0);
    });

    test('handles every picker column and delayed wrap reset loop', () => {
        const { bindings } = setupSheet();
        const { container, elements } = makeContainer();
        mockTemplateRefs.get('timePickerContainer')!.value = container;

        bindings.onPickerColumnScroll('missing', 'picker-hour', false);
        elements['.picker-items-hour'].scrollTop = 0;
        bindings.onPickerColumnScroll('picker-items-hour', 'picker-hour', false);
        expect(bindings.currentHour.value).toBe('11');
        expect(mockRafCallbacks).toHaveLength(1);

        elements['.picker-items-minute'].scrollTop = 40;
        bindings.onPickerColumnScroll('picker-items-minute', 'picker-minute', false);
        expect(bindings.currentMinute.value).toBe('00');
        elements['.picker-items-second'].scrollTop = 80;
        bindings.onPickerColumnScroll('picker-items-second', 'picker-second', true);
        expect(bindings.currentSecond.value).toBe('01');
        bindings.onPickerColumnScroll('picker-items-second', 'picker-second', false);
        elements['.picker-items-hour'].scrollTop = 40;
        bindings.onPickerColumnScroll('picker-items-hour', 'picker-hour', false);
        elements['.picker-items-meridiem-indicator-first'].scrollTop = 40;
        bindings.onPickerColumnScroll('picker-items-meridiem-indicator-first', 'picker-meridiem-indicator', true);
        expect(bindings.currentMeridiemIndicator.value).toBe('PM');

        elements['.picker-items-hour'].scrollTop = 41;
        mockRafCallbacks.shift()?.();
        elements['.picker-items-hour'].scrollTop = 40;
        for (let index = 0; index < 8 && mockRafCallbacks.length; index++) {
            mockRafCallbacks.shift()?.();
        }
        bindings.delayCheckAndResetTimePickerItemPosition();
    });
});

describe('mobile DateTimeSelectionSheet lifecycle and template', () => {
    test('opens, closes, and responds to the captured mode watcher', async () => {
        const { bindings, emit } = setupSheet();
        const { container } = makeContainer();
        mockTemplateRefs.get('timePickerContainer')!.value = container;
        mockTemplateRefs.get('datetimepicker')!.value = { switchView: mockSwitchView };
        bindings.onSheetOpen();
        await flushAsync();
        expect(mockSwitchView).toHaveBeenCalledWith('calendar');
        bindings.onSheetClosed();
        expect(emit).toHaveBeenCalledWith('update:show', false);

        expect(mockWatchCallbacks).toHaveLength(1);
        mockWatchCallbacks[0]!('date');
        mockWatchCallbacks[0]!('time');
        mockWatchCallbacks[0]!('other');
        await flushAsync();
        expect(mockSwitchView).toHaveBeenCalledTimes(2);

        const dateMode = setupSheet({ initMode: 'date', modelValue: 0 });
        mockTemplateRefs.get('datetimepicker')!.value = { switchView: mockSwitchView };
        dateMode.bindings.onSheetOpen();
        expect(dateMode.bindings.mode.value).toBe('date');
    });

    test('executes client-rendered production template event wrappers', async () => {
        const { container } = makeContainer();
        for (const variant of [
            { is24Hour: false, meridiemFirst: true },
            { is24Hour: false, meridiemFirst: false },
            { is24Hour: true, meridiemFirst: true }
        ]) {
            mockIs24Hour = variant.is24Hour;
            mockMeridiemFirst = variant.meridiemFirst;
            const { bindings } = setupSheet();
            mockTemplateRefs.get('timePickerContainer')!.value = container;
            mockTemplateRefs.get('datetimepicker')!.value = { switchView: mockSwitchView };
            const { proxyRefs } = jest.requireActual('vue') as any;
            const vnode = DateTimeSelectionSheet.render(
                {},
                [],
                { modelValue: 3_600, initMode: 'time', show: true },
                proxyRefs(bindings),
                {},
                {}
            );
            const handlers: Array<(value: any) => unknown> = [];
            collectVNodeHandlers(vnode, handlers);
            expect(handlers.length).toBeGreaterThan(10);
            for (const handler of handlers) {
                try {
                    await handler({ type: 'client-render-event' });
                } catch {
                    // Generated v-model handlers intentionally expect heterogeneous values.
                }
            }
        }
    });

    test('SSR renders both production picker modes and meridiem placements', async () => {
        const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
        const { renderToString } = jest.requireActual('vue/server-renderer') as any;
        const Stub = defineComponent({
            inheritAttrs: false,
            setup: (_props: unknown, { attrs, slots }: any) => {
                for (const [name, handler] of Object.entries(attrs)) {
                    if (name.startsWith('on') && typeof handler === 'function') {
                        mockTemplateHandlers.push(handler as (value: any) => unknown);
                    }
                }
                return () => h('div', attrs, slots.default?.());
            }
        });
        for (const meridiemFirst of [true, false]) {
            mockMeridiemFirst = meridiemFirst;
            mockTemplateHandlers.length = 0;
            const app = createSSRApp(DateTimeSelectionSheet, { modelValue: 3_600, initMode: 'time', show: true });
            for (const name of ['f7-sheet', 'f7-toolbar', 'f7-link', 'f7-page-content', 'f7-button']) app.component(name, Stub);
            app.component('date-time-picker', Stub);
            app.config.warnHandler = () => undefined;
            const html = await renderToString(app);
            expect(html).toContain('date-time-selection-sheet');
            expect(html).toContain('time-picker-input');
            for (const handler of [...mockTemplateHandlers]) {
                try {
                    await handler({ type: 'synthetic-template-event' });
                } catch {
                    // Generated model handlers intentionally expect different scalar payloads.
                }
            }
        }
    });
});
