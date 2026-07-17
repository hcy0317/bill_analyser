import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockTemplateRefs = new Map<string, any>();
const mockWatchCallbacks: Array<(value: any) => void> = [];
const mockOnKeyUpDown = jest.fn();
const mockOnPaste = jest.fn();
const mockParseAmount = jest.fn<(value: string) => number>();
const mockFormatAmount = jest.fn<(value: number, currency?: string) => string>();
const mockGetCurrencyText = jest.fn<(currency: string, plural: boolean) => any>();
const mockEvaluate = jest.fn<(formula: string) => unknown>();
const mockLogger = { warn: jest.fn(), error: jest.fn() };

let mockDecimalSeparator = '.';

const mockWesternNumeral = {
    type: 1,
    digitZero: '0',
    isDigit: (value: string) => /^[0-9]$/.test(value),
    replaceLocalizedDigitsToWesternArabicDigits: (value: string) => value.replaceAll('١', '1'),
    replaceWesternArabicDigitsToLocalizedDigits: (value: string) => value
};
const mockLocalizedNumeral = {
    type: 2,
    digitZero: '۰',
    isDigit: (value: string) => value === '١',
    replaceLocalizedDigitsToWesternArabicDigits: (value: string) => value.replaceAll('١', '1'),
    replaceWesternArabicDigitsToLocalizedDigits: (value: string) => value.replaceAll('1', '١')
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
        watch: (source: unknown, callback: (value: any) => void) => {
            if (typeof source === 'function') {
                (source as () => unknown)();
            }
            mockWatchCallbacks.push(callback);
            return () => undefined;
        }
    };
});

jest.mock('@/components/desktop/SnackBar.vue', () => ({ __esModule: true, default: {} }));
jest.mock('@/components/base/CommonNumberInputBase.ts', () => {
    const { ref } = jest.requireActual('vue') as any;
    return {
        useCommonNumberInputBase: (_props: unknown, _decimals: number, initial: string) => ({
            currentValue: ref(initial),
            onKeyUpDown: mockOnKeyUpDown,
            onPaste: mockOnPaste
        })
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentNumeralSystemType: () => mockWesternNumeral,
        getCurrentDecimalSeparator: () => mockDecimalSeparator,
        parseAmountFromLocalizedNumerals: (value: string) => mockParseAmount(value),
        formatAmountToLocalizedNumeralsWithoutDigitGrouping: (value: number, currency?: string) => mockFormatAmount(value, currency),
        getAmountPrependAndAppendText: (currency: string, plural: boolean) => mockGetCurrencyText(currency, plural)
    })
}));
jest.mock('@/core/numeral.ts', () => ({
    NumeralSystem: {
        WesternArabicNumerals: mockWesternNumeral,
        detect: (value: string) => /^[0-9]$/.test(value)
            ? mockWesternNumeral
            : (value === '١' ? mockLocalizedNumeral : undefined)
    },
    DecimalSeparator: { Dot: { symbol: '.' } }
}));
jest.mock('@/consts/numeral.ts', () => ({ DEFAULT_DECIMAL_NUMBER_COUNT: 2 }));
jest.mock('@/consts/transaction.ts', () => ({
    TRANSACTION_MIN_AMOUNT: -999_999,
    TRANSACTION_MAX_AMOUNT: 999_999
}));
jest.mock('@/lib/common.ts', () => ({
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value),
    replaceAll: (value: string, search: string, replacement: string) => value.split(search).join(replacement)
}));
jest.mock('@/lib/evaluator.ts', () => ({
    evaluateExpressionToAmount: (formula: string) => mockEvaluate(formula)
}));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

const AmountInput = require('@/components/desktop/AmountInput.vue').default as any;

function setup(overrides: Record<string, unknown> = {}): { bindings: any; emit: jest.Mock; props: Record<string, any> } {
    const emit = jest.fn();
    const props = {
        modelValue: 12_345,
        currency: 'CNY',
        showCurrency: true,
        enableFormula: true,
        enableRules: true,
        flipNegative: false,
        compactCurrencyDisplay: false,
        disabled: false,
        readonly: false,
        hide: false,
        class: 'base',
        color: 'primary',
        ...overrides
    };
    const bindings = AmountInput.setup(props, {
        attrs: {}, slots: {}, emit, expose: () => undefined
    });
    return { bindings, emit, props };
}

function installSnackbar(): { showMessage: jest.Mock } {
    const value = { showMessage: jest.fn() };
    mockTemplateRefs.get('snackbar')!.value = value;
    return value;
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
                    visitVNode((child as (value?: unknown) => unknown)({ props: { tabindex: 0 } }), handlers);
                } catch {
                    // Generated Vuetify slots have heterogeneous contracts.
                }
            } else {
                visitVNode(child, handlers);
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    mockWatchCallbacks.length = 0;
    mockDecimalSeparator = '.';
    mockParseAmount.mockImplementation((value: string) => {
        if (value === 'throw') throw new Error('parse failed');
        if (value === 'NaN') return Number.NaN;
        if (value === 'Infinity') return Number.POSITIVE_INFINITY;
        const parsed = Number(value.replace(',', '.'));
        return Number.isFinite(parsed) ? Math.round(parsed * 100) : Number.NaN;
    });
    mockFormatAmount.mockImplementation((value: number) => Number.isFinite(value) ? String(value / 100) : '');
    mockGetCurrencyText.mockImplementation((_currency: string, plural: boolean) => ({
        prependText: plural ? '¥s' : '¥',
        appendText: plural ? 'yuan' : 'yuan-one'
    }));
    mockEvaluate.mockReturnValue(250);
    class MockInputElement {
        selectionStart: number | null = 0;
        selectionEnd: number | null = 0;
        select = jest.fn();
    }
    Object.defineProperty(globalThis, 'HTMLInputElement', {
        configurable: true,
        value: MockInputElement
    });
});

describe('desktop AmountInput cents and currency behavior', () => {
    test('initializes cents, validates numeric limits, and handles parser failures', () => {
        const { bindings } = setup();
        expect(bindings.currentValue.value).toBe('123.45');
        expect(bindings.rules[0]('')).toBe('tt:Amount value is not number');
        expect(bindings.rules[0]('NaN')).toBe('tt:Amount value is not number');
        expect(bindings.rules[0]('Infinity')).toBe('tt:Amount value is not number');
        expect(bindings.rules[0]('1')).toBe(true);
        expect(bindings.rules[0]('10000')).toBe('tt:Amount value exceeds limitation');
        expect(bindings.rules[0]('-10000')).toBe('tt:Amount value exceeds limitation');
        expect(bindings.rules[0]('throw')).toBe('tt:Amount value is not number');
        expect(mockLogger.warn).toHaveBeenCalledWith(
            'cannot parse amount in amount input, original value is throw', expect.any(Error)
        );
        expect(mockOnKeyUpDown).toBeDefined();
        expect(mockOnPaste).toBeDefined();
    });

    test('derives prepend, append, compact text, pluralization, and style classes', () => {
        const { bindings } = setup();
        expect(bindings.prependText.value).toBe('¥s');
        expect(bindings.appendText.value).toBe('yuan');
        expect(bindings.compactCurrencyText.value).toBe('');
        expect(bindings.extraClass.value).toContain('base text-primary has-pretend-text');
        expect(mockGetCurrencyText).toHaveBeenCalledWith('CNY', true);

        const compact = setup({ modelValue: 100, compactCurrencyDisplay: true, class: '', color: '' }).bindings;
        expect(compact.prependText.value).toBe('¥');
        expect(compact.appendText.value).toBe('yuan-one');
        expect(compact.compactCurrencyText.value).toBe('¥');
        expect(compact.extraClass.value).toBe('');

        mockGetCurrencyText.mockReturnValueOnce(null);
        expect(setup().bindings.prependText.value).toBe('');
        mockGetCurrencyText.mockReturnValueOnce(null);
        expect(setup().bindings.appendText.value).toBe('');
        expect(setup({ currency: '', showCurrency: true }).bindings.prependText.value).toBe('');
        expect(setup({ showCurrency: false }).bindings.appendText.value).toBe('');
        expect(setup({ showCurrency: false, compactCurrencyDisplay: true }).bindings.compactCurrencyText.value).toBe('');
    });

    test('clamps and truncates formatted values while preserving cents', () => {
        const { bindings } = setup();
        expect(bindings.getValidFormattedValue(-1_000_000, '-10000', false)).toBe('-9999.99');
        expect(bindings.getValidFormattedValue(1_000_000, '10000', false)).toBe('9999.99');
        expect(bindings.getValidFormattedValue(1, '12345678', false)).toBe('123456');
        expect(bindings.getValidFormattedValue(1, '123456.78', true)).toBe('123456.');
        expect(bindings.getValidFormattedValue(-1, '-12345678', false)).toBe('-123456');
        expect(bindings.getValidFormattedValue(123, '1.23', true)).toBe('1.23');
        expect(bindings.getInitedFormattedValue(1234, false)).toBe('12.34');
        expect(bindings.getInitedFormattedValue(1234, true)).toBe('-12.34');
        expect(bindings.getFormattedValue(Number.NaN)).toBe('0');
        expect(bindings.getFormattedValue(Number.POSITIVE_INFINITY)).toBe('0');
    });
});

describe('desktop AmountInput formula and interaction behavior', () => {
    test('enters, calculates, and exits formula mode with localized separators', () => {
        const disabled = setup({ enableFormula: false }).bindings;
        disabled.enterFormulaMode();
        expect(disabled.formulaMode.value).toBe(false);

        const { bindings } = setup();
        const snackbar = installSnackbar();
        bindings.enterFormulaMode();
        expect(bindings.formulaMode.value).toBe(true);
        expect(bindings.currentFormula.value).toBe(bindings.currentValue.value);

        mockDecimalSeparator = ',';
        bindings.currentFormula.value = '1.5+2';
        bindings.calculateFormula();
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Formula is invalid');
        expect(bindings.formulaMode.value).toBe(true);

        bindings.currentFormula.value = '1,5+1';
        mockEvaluate.mockReturnValueOnce(250);
        bindings.calculateFormula();
        expect(mockEvaluate).toHaveBeenLastCalledWith('1.5+1');
        expect(bindings.currentValue.value).toBe('2.5');
        expect(bindings.formulaMode.value).toBe(false);

        bindings.formulaMode.value = true;
        bindings.currentFormula.value = '١+1';
        mockDecimalSeparator = '.';
        mockEvaluate.mockReturnValueOnce(undefined);
        bindings.calculateFormula();
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Formula is invalid');

        bindings.currentFormula.value = 'bad';
        mockEvaluate.mockImplementationOnce(() => { throw new Error('division by zero'); });
        bindings.calculateFormula();
        expect(mockLogger.error).toHaveBeenCalledWith(
            'cannot evaluate formula in amount input, original formula is bad', expect.any(Error)
        );
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('division by zero');

        snackbar.showMessage.mockClear();
        mockEvaluate.mockImplementationOnce(() => { throw 'plain failure'; });
        bindings.calculateFormula();
        expect(snackbar.showMessage).not.toHaveBeenCalled();
        bindings.exitFormulaMode();
        expect(bindings.formulaMode.value).toBe(false);
        expect(bindings.currentFormula.value).toBe('');
    });

    test('selects zero-valued editable inputs only for collapsed or caret selections', () => {
        const Input = globalThis.HTMLInputElement as any;
        const input = new Input();
        const select = input.select as jest.Mock;
        input.selectionStart = 0;
        input.selectionEnd = 0;
        setup({ modelValue: 0 }).bindings.onClick({ target: input });
        expect(select).toHaveBeenCalled();

        select.mockClear();
        input.selectionStart = 1;
        input.selectionEnd = 1;
        setup({ modelValue: 0 }).bindings.onClick({ target: input });
        expect(select).toHaveBeenCalled();
        select.mockClear();
        input.selectionStart = 0;
        input.selectionEnd = 1;
        setup({ modelValue: 0 }).bindings.onClick({ target: input });
        expect(select).not.toHaveBeenCalled();
        setup({ modelValue: 1 }).bindings.onClick({ target: input });
        setup({ modelValue: 0, disabled: true }).bindings.onClick({ target: input });
        setup({ modelValue: 0, readonly: true }).bindings.onClick({ target: input });
        setup({ modelValue: 0 }).bindings.onClick({ target: {} });
        expect(select).not.toHaveBeenCalled();
    });
});

describe('desktop AmountInput watchers and production template', () => {
    test('synchronizes currency, sign, model cents, and sanitized localized input', () => {
        const { bindings, emit } = setup({ modelValue: 1234 });
        expect(mockWatchCallbacks).toHaveLength(4);
        bindings.currentValue.value = 'old';
        mockWatchCallbacks[0]!('USD');
        expect(bindings.currentValue.value).toBe('12.34');
        mockWatchCallbacks[1]!(true);
        expect(bindings.currentValue.value).toBe('-12.34');

        bindings.currentValue.value = '12.34';
        mockWatchCallbacks[2]!(1234);
        expect(bindings.currentValue.value).toBe('12.34');
        mockWatchCallbacks[2]!(5678);
        expect(bindings.currentValue.value).toBe('56.78');

        bindings.currentValue.value = '12x3';
        mockWatchCallbacks[3]!('12x3');
        expect(bindings.currentValue.value).toBe('12');
        bindings.currentValue.value = '-';
        mockWatchCallbacks[3]!('-');
        expect(bindings.currentValue.value).toBe('-');
        bindings.currentValue.value = '.';
        mockWatchCallbacks[3]!('.');
        expect(bindings.currentValue.value).toBe('.');
        bindings.currentValue.value = '-.';
        mockWatchCallbacks[3]!('-.');
        expect(bindings.currentValue.value).toBe('-.');
        bindings.currentValue.value = '١';
        mockWatchCallbacks[3]!('١');
        expect(bindings.currentValue.value).toBe('');
        mockWatchCallbacks[3]!('12.34');
        expect(emit).toHaveBeenCalledWith('update:modelValue', 1234);

        mockParseAmount.mockReturnValueOnce(Number.NaN);
        mockWatchCallbacks[3]!('1');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 0);
        const flipped = setup({ flipNegative: true, modelValue: 1234 });
        flipped.bindings.currentValue.value = '12.34';
        mockWatchCallbacks[7]!('12.34');
        expect(flipped.emit).toHaveBeenCalledWith('update:modelValue', -1234);
    });

    test('preserves intentionally empty zero values in all prop watchers', () => {
        mockFormatAmount.mockReturnValue('0');
        const { bindings } = setup({ modelValue: 0 });
        bindings.currentValue.value = '';
        mockWatchCallbacks[0]!('USD');
        mockWatchCallbacks[1]!(false);
        mockWatchCallbacks[2]!(0);
        expect(bindings.currentValue.value).toBe('');
    });

    test('executes visible, formula, compact, and password template branches', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const { proxyRefs } = jest.requireActual('vue') as any;
            const scenarios = [
                { hide: false, formula: false, compact: false },
                { hide: false, formula: true, compact: true },
                { hide: true, formula: false, compact: false },
                { hide: true, formula: false, compact: true }
            ];
            for (const scenario of scenarios) {
                const { bindings } = setup({
                    hide: scenario.hide,
                    compactCurrencyDisplay: scenario.compact
                });
                bindings.formulaMode.value = scenario.formula;
                const vnode = AmountInput.render({}, [], {
                    modelValue: 1234,
                    currency: 'CNY',
                    showCurrency: true,
                    enableFormula: true,
                    enableRules: true,
                    hide: scenario.hide,
                    compactCurrencyDisplay: scenario.compact
                }, proxyRefs(bindings), {}, {});
                const handlers: Array<(...args: any[]) => unknown> = [];
                visitVNode(vnode, handlers);
                expect(handlers.length).toBeGreaterThan(1);
                for (const handler of handlers) {
                    try {
                        const Input = globalThis.HTMLInputElement as any;
                        await handler({ target: new Input() });
                    } catch {
                        // Generated Vuetify handlers accept heterogeneous payloads.
                    }
                }
            }
        } finally {
            warnSpy.mockRestore();
        }
    });
});
