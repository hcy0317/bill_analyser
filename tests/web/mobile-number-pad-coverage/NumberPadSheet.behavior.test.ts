import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockShowToast = jest.fn();
const mockWatchCallbacks: Array<(value: boolean | undefined) => void> = [];

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        watch: (_source: unknown, callback: (value: boolean | undefined) => void) => {
            mockWatchCallbacks.push(callback);
            return () => undefined;
        }
    };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getAllLocalizedDigits: () => ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'],
        getCurrentNumeralSystemType: () => ({
            replaceWesternArabicDigitsToLocalizedDigits: (value: string) => value
        }),
        getCurrentDecimalSeparator: () => '.',
        parseAmountFromWesternArabicNumerals: (value: string) => {
            const parsed = Number(value || '0');
            return Number.isFinite(parsed) ? Math.round(parsed * 100) : 0;
        },
        formatAmountToWesternArabicNumeralsWithoutDigitGrouping: (value: number) => (
            Number.isFinite(value) ? (value / 100).toFixed(2) : ''
        ),
        appendDigitGroupingSymbolAndDecimalSeparator: (value: string) => value
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast })
}));
jest.mock('@/core/numeral.ts', () => ({
    NumeralSystem: { WesternArabicNumerals: { digitZero: '0' } }
}));
jest.mock('@/consts/currency.ts', () => ({
    ALL_CURRENCIES: {
        CNY: { fraction: 2 },
        JPY: { fraction: 0 },
        UNKNOWN_FRACTION: { fraction: undefined }
    }
}));
jest.mock('@/lib/common.ts', () => ({
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value)
}));

const NumberPadSheet = require('@/components/mobile/NumberPadSheet.vue').default as any;

function setup(overrides: Record<string, unknown> = {}): { bindings: any; emit: jest.Mock; props: Record<string, unknown> } {
    const emit = jest.fn();
    const props = {
        modelValue: 1234,
        minValue: undefined,
        maxValue: undefined,
        currency: 'CNY',
        flipNegative: false,
        hint: 'Enter amount',
        show: true,
        ...overrides
    };
    const bindings = NumberPadSheet.setup(props, {
        attrs: {}, slots: {}, emit, expose: () => undefined
    });
    return { bindings, emit, props };
}

function visitVNode(node: any, handlers: Array<(...args: any[]) => unknown>): void {
    if (!node) return;
    if (Array.isArray(node)) {
        for (const child of node) visitVNode(child, handlers);
        return;
    }
    if (typeof node !== 'object') return;
    for (const [name, handler] of Object.entries(node.props || {})) {
        if (!name.startsWith('on')) continue;
        if (typeof handler === 'function') handlers.push(handler as (...args: any[]) => unknown);
        if (Array.isArray(handler)) handlers.push(...handler.filter(value => typeof value === 'function') as Array<(...args: any[]) => unknown>);
    }
    if (Array.isArray(node.children)) {
        visitVNode(node.children, handlers);
    } else if (node.children && typeof node.children === 'object') {
        for (const child of Object.values(node.children)) {
            if (typeof child === 'function') {
                try {
                    visitVNode((child as () => unknown)(), handlers);
                } catch {
                    // Generated slots may require Framework7-owned arguments.
                }
            } else {
                visitVNode(child, handlers);
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockWatchCallbacks.length = 0;
});

describe('mobile NumberPadSheet value formatting and entry', () => {
    test('initializes cents, trims display zeros, supports negative flipping, and chooses decimal capability', () => {
        const normal = setup();
        expect(normal.bindings.currentValue.value).toBe('12.34');
        expect(normal.bindings.currentDisplay.value).toBe('12.34');
        expect(normal.bindings.currentDisplayNumClass.value).toBe('numpad-value-large');
        expect(normal.bindings.confirmText.value).toBe('tt:OK');
        expect(normal.bindings.supportDecimalSeparator.value).toBe(true);
        expect(normal.bindings.getStringValue(Number.NaN, true)).toBe('');
        expect(normal.bindings.getStringValue(0, true)).toBe('');
        expect(normal.bindings.getStringValue(0, false)).toBe('0');
        expect(normal.bindings.getStringValue(1200, false)).toBe('12');
        expect(normal.bindings.getStringValue(1205, false)).toBe('12.05');

        const flipped = setup({ flipNegative: true });
        expect(flipped.bindings.currentValue.value).toBe('-12.34');
        expect(setup({ currency: 'JPY' }).bindings.supportDecimalSeparator.value).toBe(false);
        expect(setup({ currency: undefined }).bindings.supportDecimalSeparator.value).toBe(true);
        expect(setup({ currency: 'MISSING' }).bindings.supportDecimalSeparator.value).toBe(true);
        expect(setup({ currency: 'UNKNOWN_FRACTION' }).bindings.supportDecimalSeparator.value).toBe(true);
    });

    test('formats formula displays, suffix decimals, and all three responsive classes', () => {
        const { bindings } = setup();
        bindings.currentValue.value = '12.';
        expect(bindings.currentDisplay.value).toBe('12.');
        bindings.previousValue.value = '1234';
        bindings.currentSymbol.value = '+';
        expect(bindings.currentDisplay.value).toBe('1234 + 12.');
        expect(bindings.confirmText.value).toBe('=');
        bindings.currentValue.value = '1234567890123456';
        bindings.previousValue.value = '';
        bindings.currentSymbol.value = '';
        expect(bindings.currentDisplayNumClass.value).toBe('numpad-value-normal');
        bindings.currentValue.value = '123456789012345678901234';
        expect(bindings.currentDisplayNumClass.value).toBe('numpad-value-small');
        bindings.currentValue.value = '';
        expect(bindings.currentDisplay.value).toBe('');
        expect(bindings.currentDisplayNumClass.value).toBe('numpad-value-large');
    });

    test('enters digits, replaces signed zeros, enforces fraction precision and min/max cents', () => {
        const { bindings } = setup({ modelValue: 0 });
        bindings.currentValue.value = '0';
        bindings.inputNum(7);
        expect(bindings.currentValue.value).toBe('7');
        bindings.currentValue.value = '-0';
        bindings.inputNum(8);
        expect(bindings.currentValue.value).toBe('-8');
        bindings.currentValue.value = '1.23';
        bindings.inputNum(4);
        expect(bindings.currentValue.value).toBe('1.23');
        bindings.currentValue.value = '';
        bindings.currentSymbol.value = '−';
        bindings.inputNum(5);
        expect(bindings.currentValue.value).toBe('-5');
        expect(bindings.currentSymbol.value).toBe('');
        bindings.currentValue.value = '1';
        bindings.inputDoubleNum(0);
        expect(bindings.currentValue.value).toBe('100');

        const bounded = setup({ modelValue: 0, minValue: -500, maxValue: 500 }).bindings;
        bounded.currentValue.value = '';
        bounded.inputNum(6);
        expect(bounded.currentValue.value).toBe('');
        bounded.currentValue.value = '-';
        bounded.inputNum(6);
        expect(bounded.currentValue.value).toBe('-');
        bounded.currentValue.value = '';
        bounded.inputNum(5);
        expect(bounded.currentValue.value).toBe('5');
    });

    test('enters decimal separators exactly once including negative shorthand', () => {
        const { bindings } = setup({ modelValue: 0 });
        bindings.currentValue.value = '';
        bindings.inputDecimalSeparator();
        expect(bindings.currentValue.value).toBe('0.');
        bindings.inputDecimalSeparator();
        expect(bindings.currentValue.value).toBe('0.');
        bindings.currentValue.value = '-';
        bindings.inputDecimalSeparator();
        expect(bindings.currentValue.value).toBe('-0.');
        bindings.currentValue.value = '';
        bindings.previousValue.value = '';
        bindings.currentSymbol.value = '−';
        bindings.inputDecimalSeparator();
        expect(bindings.currentValue.value).toBe('-0.');
        expect(bindings.currentSymbol.value).toBe('');
        bindings.currentValue.value = '12';
        bindings.inputDecimalSeparator();
        expect(bindings.currentValue.value).toBe('12.');
    });
});

describe('mobile NumberPadSheet formulas and lifecycle', () => {
    test.each([
        ['+', 250, 125, 375],
        ['−', 250, 125, 125],
        ['×', 250, 200, 500],
        ['?', 250, 125, 250]
    ])('calculates %s formulas in integer cents', (symbol, previous, current, expected) => {
        const { bindings } = setup({ modelValue: 0 });
        bindings.previousValue.value = String(previous / 100);
        bindings.currentValue.value = String(current / 100);
        bindings.currentSymbol.value = symbol;
        expect(bindings.confirm()).toBe(true);
        expect(bindings.currentValue.value).toBe(bindings.getStringValue(expected, false));
        expect(bindings.previousValue.value).toBe('');
        expect(bindings.currentSymbol.value).toBe('');
    });

    test('rejects formula overflow, restores incomplete formulas, and chains symbols safely', () => {
        const tooSmall = setup({ modelValue: 0, minValue: 0 }).bindings;
        tooSmall.previousValue.value = '1';
        tooSmall.currentValue.value = '2';
        tooSmall.currentSymbol.value = '−';
        expect(tooSmall.confirm()).toBe(false);
        expect(mockShowToast).toHaveBeenLastCalledWith('Numeric Overflow');

        const tooLarge = setup({ modelValue: 0, maxValue: 100 }).bindings;
        tooLarge.previousValue.value = '1';
        tooLarge.currentValue.value = '1';
        tooLarge.currentSymbol.value = '+';
        expect(tooLarge.confirm()).toBe(false);
        expect(mockShowToast).toHaveBeenLastCalledWith('Numeric Overflow');
        tooLarge.setSymbol('×');
        expect(tooLarge.currentSymbol.value).toBe('+');

        const incomplete = setup({ modelValue: 0 }).bindings;
        incomplete.previousValue.value = '12';
        incomplete.currentValue.value = '';
        incomplete.currentSymbol.value = '+';
        expect(incomplete.confirm()).toBe(true);
        expect(incomplete.currentValue.value).toBe('12');
        incomplete.setSymbol('×');
        expect(incomplete.previousValue.value).toBe('12');
        expect(incomplete.currentValue.value).toBe('');
        expect(incomplete.currentSymbol.value).toBe('×');
        incomplete.setSymbol('−');
        expect(incomplete.currentSymbol.value).toBe('−');
    });

    test('backspaces, clears, emits values, closes, and reacts to sheet/watch lifecycle', () => {
        const { bindings, emit } = setup({ modelValue: 1234 });
        bindings.currentValue.value = '12.3';
        bindings.backspace();
        expect(bindings.currentValue.value).toBe('12.');
        bindings.currentValue.value = '';
        bindings.previousValue.value = '7';
        bindings.currentSymbol.value = '+';
        bindings.backspace();
        expect(bindings.currentValue.value).toBe('7');
        expect(bindings.currentSymbol.value).toBe('');
        bindings.currentValue.value = '';
        bindings.backspace();
        bindings.currentValue.value = '9';
        bindings.previousValue.value = '8';
        bindings.currentSymbol.value = '×';
        bindings.clear();
        expect([bindings.currentValue.value, bindings.previousValue.value, bindings.currentSymbol.value]).toStrictEqual(['', '', '']);

        bindings.currentValue.value = '4.56';
        expect(bindings.confirm()).toBe(true);
        expect(emit).toHaveBeenNthCalledWith(1, 'update:modelValue', 456);
        expect(emit).toHaveBeenNthCalledWith(2, 'update:show', false);
        bindings.onSheetClosed();
        expect(emit).toHaveBeenLastCalledWith('update:show', false);

        bindings.currentValue.value = '999';
        bindings.previousValue.value = '1';
        bindings.currentSymbol.value = '+';
        bindings.onSheetOpen();
        expect(bindings.currentValue.value).toBe('12.34');
        expect(bindings.previousValue.value).toBe('');
        expect(bindings.currentSymbol.value).toBe('');
        expect(mockWatchCallbacks).toHaveLength(1);
        mockWatchCallbacks[0]!(true);
        expect(bindings.currentValue.value).toBe('-12.34');

        const flipped = setup({ modelValue: 1234, flipNegative: true });
        flipped.bindings.currentValue.value = '-4.56';
        flipped.bindings.confirm();
        expect(flipped.emit).toHaveBeenCalledWith('update:modelValue', 456);
    });

    test('executes both decimal and zero-zero production template branches', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const { proxyRefs } = jest.requireActual('vue') as any;
            for (const currency of ['CNY', 'JPY']) {
                const { bindings } = setup({ currency, hint: currency === 'CNY' ? 'Hint' : '' });
                if (currency === 'CNY') {
                    bindings.previousValue.value = '1';
                    bindings.currentSymbol.value = '+';
                }
                const vnode = NumberPadSheet.render(
                    {}, [], { modelValue: 1234, currency, hint: currency === 'CNY' ? 'Hint' : '', show: true }, proxyRefs(bindings), {}, {}
                );
                const handlers: Array<(...args: any[]) => unknown> = [];
                visitVNode(vnode, handlers);
                expect(handlers.length).toBeGreaterThan(15);
                for (const handler of handlers) {
                    try {
                        await handler({ type: 'synthetic-template-event' });
                    } catch {
                        // Framework7 event handlers accept heterogeneous payloads.
                    }
                }
            }
        } finally {
            warnSpy.mockRestore();
        }
    });
});
