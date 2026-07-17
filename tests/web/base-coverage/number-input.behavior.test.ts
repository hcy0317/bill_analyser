import { beforeEach, describe, expect, jest, test } from '@jest/globals';
const { nextTick, reactive } = jest.requireActual('vue') as typeof import('@/../node_modules/vue');

import { NumeralSystem } from '@/core/numeral.ts';
import {
    useCommonNumberInputBase,
    type CommonNumberInputProps
} from '@/components/base/CommonNumberInputBase.ts';
import { useNumberInputBase, type NumberInputProps } from '@/components/base/NumberInputBase.ts';

let mockNumeralSystem = NumeralSystem.WesternArabicNumerals;
let mockDecimalSeparator = '.';
let mockGroupingSymbol = ',';
const mockLoggerWarn = jest.fn();

jest.mock('@/locales/helpers.ts', () => ({
    __esModule: true,
    useI18n: () => ({
        getCurrentNumeralSystemType: () => mockNumeralSystem,
        getCurrentDecimalSeparator: () => mockDecimalSeparator,
        getCurrentDigitGroupingSymbol: () => mockGroupingSymbol
    })
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: (...args: unknown[]) => mockLoggerWarn(...args),
        error: jest.fn()
    }
}));

interface KeyboardEventCase {
    event: KeyboardEvent;
    target: HTMLInputElement;
    preventDefault: ReturnType<typeof jest.fn>;
}

function keyboardCase(key: string, value: string, overrides: Partial<KeyboardEvent> = {}): KeyboardEventCase {
    const target = { value } as HTMLInputElement;
    const preventDefault = jest.fn();
    return {
        target,
        preventDefault,
        event: {
            key,
            target,
            preventDefault,
            altKey: false,
            ctrlKey: false,
            metaKey: false,
            ...overrides
        } as unknown as KeyboardEvent
    };
}

function clipboardCase(text: string | null): { event: ClipboardEvent; preventDefault: ReturnType<typeof jest.fn> } {
    const preventDefault = jest.fn();
    const clipboardData = text === null ? null : { getData: () => text };
    return {
        preventDefault,
        event: { clipboardData, preventDefault } as unknown as ClipboardEvent
    };
}

describe('common number input keyboard behavior', () => {
    beforeEach(() => {
        mockNumeralSystem = NumeralSystem.WesternArabicNumerals;
        mockDecimalSeparator = '.';
        mockGroupingSymbol = ',';
        mockLoggerWarn.mockClear();
    });

    function createBase(props: CommonNumberInputProps = { modelValue: 0 }, maxDecimals = 2) {
        const parseNumber = jest.fn((value: string) => {
            if (value === 'throw') {
                throw new Error('bad number');
            }
            return Number(value);
        });
        const formatNumber = jest.fn((value: number) => `formatted:${value}`);
        const getValidFormattedValue = jest.fn((_value: number, text: string, _hasDecimalSeparator: boolean) => text === '12' ? 'normalized' : text);
        return {
            parseNumber,
            formatNumber,
            getValidFormattedValue,
            base: useCommonNumberInputBase(props, maxDecimals, '0', parseNumber, formatNumber, getValidFormattedValue)
        };
    }

    test.each(['ArrowLeft', 'ArrowRight', 'Home', 'End', 'Tab', 'Backspace', 'Delete', 'Del', 'F1', 'F12'])(
        'allows the navigation/control key %s',
        (key) => {
            const { base } = createBase();
            const input = keyboardCase(key, '1');
            base.onKeyUpDown(input.event);
            expect(input.preventDefault).not.toHaveBeenCalled();
        }
    );

    test('allows modifier shortcuts and blocks editing for readonly or disabled inputs', () => {
        const props: CommonNumberInputProps = { modelValue: 0 };
        const { base } = createBase(props);

        for (const modifier of [{ altKey: true }, { ctrlKey: true }, { metaKey: true }]) {
            const input = keyboardCase('x', '', modifier);
            base.onKeyUpDown(input.event);
            expect(input.preventDefault).not.toHaveBeenCalled();
        }

        props.readonly = true;
        const readonlyInput = keyboardCase('1', '1');
        base.onKeyUpDown(readonlyInput.event);
        expect(readonlyInput.preventDefault).toHaveBeenCalledTimes(1);
        props.readonly = false;
        props.disabled = true;
        const disabledInput = keyboardCase('1', '1');
        base.onKeyUpDown(disabledInput.event);
        expect(disabledInput.preventDefault).toHaveBeenCalledTimes(1);
    });

    test('rejects invalid characters and decimal separators for integer inputs', () => {
        const { base } = createBase({ modelValue: 0 }, 0);
        const invalid = keyboardCase('x', '1x');
        base.onKeyUpDown(invalid.event);
        expect(invalid.preventDefault).toHaveBeenCalledTimes(1);

        const decimal = keyboardCase('.', '1.');
        base.onKeyUpDown(decimal.event);
        expect(decimal.preventDefault).toHaveBeenCalledTimes(1);
    });

    test('returns safely when a valid key event has no target', () => {
        const { base } = createBase();
        const preventDefault = jest.fn();
        base.onKeyUpDown({
            key: '1',
            target: null,
            preventDefault,
            altKey: false,
            ctrlKey: false,
            metaKey: false
        } as unknown as KeyboardEvent);
        expect(preventDefault).not.toHaveBeenCalled();
    });

    test('removes grouping and duplicate minus or decimal characters', () => {
        const { base } = createBase();
        const duplicateMinus = keyboardCase('-', '1,2-3-');
        base.onKeyUpDown(duplicateMinus.event);
        expect(duplicateMinus.target.value).toBe('12-3');
        expect(base.currentValue.value).toBe('12-3');

        const duplicateDecimal = keyboardCase('.', '1.2.');
        base.onKeyUpDown(duplicateDecimal.event);
        expect(duplicateDecimal.target.value).toBe('1.2');
        expect(base.currentValue.value).toBe('1.2');
    });

    test('adds a leading zero before positive and negative decimal fractions', () => {
        const { base } = createBase();
        const positive = keyboardCase('.', '.');
        base.onKeyUpDown(positive.event);
        expect(positive.target.value).toBe('0.');

        const negative = keyboardCase('.', '-.');
        base.onKeyUpDown(negative.event);
        expect(negative.target.value).toBe('-0.');
    });

    test('trims leading zeros and excess decimal digits', () => {
        const { base } = createBase();
        const positive = keyboardCase('1', '001');
        base.onKeyUpDown(positive.event);
        expect(positive.target.value).toBe('1');

        const negative = keyboardCase('1', '-001');
        base.onKeyUpDown(negative.event);
        expect(negative.target.value).toBe('-1');

        const fraction = keyboardCase('4', '1.234');
        base.onKeyUpDown(fraction.event);
        expect(fraction.target.value).toBe('1.23');

        const integerBase = createBase({ modelValue: 0 }, 0).base;
        const integer = keyboardCase('2', '12.3');
        integerBase.onKeyUpDown(integer.event);
        expect(integer.target.value).toBe('12');
    });

    test('applies valid formatting and recovers from parser errors', () => {
        const { base, getValidFormattedValue } = createBase();
        const normalized = keyboardCase('2', '12');
        base.onKeyUpDown(normalized.event);
        expect(normalized.target.value).toBe('normalized');
        expect(getValidFormattedValue).toHaveBeenCalledWith(12, '12', false);

        const throwing = keyboardCase('w', 'throw');
        base.onKeyUpDown(throwing.event);
        expect(throwing.target.value).toBe('throw');

        const parseThrowing = useCommonNumberInputBase(
            { modelValue: 0 },
            2,
            '0',
            () => { throw new Error('bad number'); },
            value => String(value),
            (_value, text) => text
        );
        const errorInput = keyboardCase('1', '1');
        parseThrowing.onKeyUpDown(errorInput.event);
        expect(errorInput.target.value).toBe('0');
        expect(mockLoggerWarn).toHaveBeenCalledTimes(1);
    });

    test('validates paste availability, content, and formatted value', () => {
        const props: CommonNumberInputProps = { modelValue: 0 };
        const { base, parseNumber, formatNumber, getValidFormattedValue } = createBase(props);

        const missing = clipboardCase(null);
        base.onPaste(missing.event);
        expect(missing.preventDefault).toHaveBeenCalledTimes(1);

        const empty = clipboardCase('');
        base.onPaste(empty.event);
        expect(empty.preventDefault).toHaveBeenCalledTimes(1);

        props.disabled = true;
        const disabled = clipboardCase('12');
        base.onPaste(disabled.event);
        expect(parseNumber).not.toHaveBeenCalled();
        props.disabled = false;
        props.readonly = true;
        const readonly = clipboardCase('12');
        base.onPaste(readonly.event);
        expect(parseNumber).not.toHaveBeenCalled();

        props.readonly = false;
        const valid = clipboardCase('12.5');
        base.onPaste(valid.event);
        expect(parseNumber).toHaveBeenCalledWith('12.5');
        expect(formatNumber).toHaveBeenCalledWith(12.5);
        expect(getValidFormattedValue).toHaveBeenLastCalledWith(12.5, 'formatted:12.5', true);
        expect(base.currentValue.value).toBe('formatted:12.5');
    });
});

describe('number input value and watcher behavior', () => {
    beforeEach(() => {
        mockNumeralSystem = NumeralSystem.WesternArabicNumerals;
        mockDecimalSeparator = '.';
        mockGroupingSymbol = ',';
    });

    test('formats initial/model values and emits sanitized numeric edits', async () => {
        const props = reactive<NumberInputProps>({ modelValue: 1.2, maxDecimalCount: 2 });
        const emit = jest.fn<(event: 'update:modelValue', value: number) => void>();
        const base = useNumberInputBase(props, emit);

        expect(base.currentValue.value).toBe('1.20');
        base.currentValue.value = '12.34ignored';
        await nextTick();
        expect(base.currentValue.value).toBe('12.34');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 12.34);

        props.modelValue = 9.5;
        await nextTick();
        expect(base.currentValue.value).toBe('9.50');
    });

    test('preserves empty zero edits and standalone sign/separator tokens', async () => {
        const props = reactive<NumberInputProps>({ modelValue: 1 });
        const emit = jest.fn<(event: 'update:modelValue', value: number) => void>();
        const base = useNumberInputBase(props, emit);

        base.currentValue.value = '';
        await nextTick();
        props.modelValue = 0;
        await nextTick();
        expect(base.currentValue.value).toBe('');

        base.currentValue.value = '-';
        await nextTick();
        expect(base.currentValue.value).toBe('-');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 0);
        base.currentValue.value = '.';
        await nextTick();
        expect(base.currentValue.value).toBe('.');
        base.currentValue.value = '-.';
        await nextTick();
        expect(base.currentValue.value).toBe('-.');
    });

    test('supports localized numerals, decimal separators, and grouping removal', async () => {
        mockNumeralSystem = NumeralSystem.EasternArabicNumerals;
        mockDecimalSeparator = ',';
        mockGroupingSymbol = '_';
        const props = reactive<NumberInputProps>({ modelValue: 12.5, maxDecimalCount: 1 });
        const emit = jest.fn<(event: 'update:modelValue', value: number) => void>();
        const base = useNumberInputBase(props, emit);

        expect(base.currentValue.value).toBe('١٢,٥');
        base.currentValue.value = '١_٢,٥';
        await nextTick();
        expect(base.currentValue.value).toBe('١');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 1);

        base.currentValue.value = '١٢,٥';
        await nextTick();
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 12.5);
    });

    test('clears foreign numeral systems and converts western digits to the active system', async () => {
        mockNumeralSystem = NumeralSystem.EasternArabicNumerals;
        const props = reactive<NumberInputProps>({ modelValue: 0 });
        const emit = jest.fn<(event: 'update:modelValue', value: number) => void>();
        const base = useNumberInputBase(props, emit);

        base.currentValue.value = '12';
        await nextTick();
        expect(base.currentValue.value).toBe('١٢');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 12);

        base.currentValue.value = NumeralSystem.PersianDigits.replaceWesternArabicDigitsToLocalizedDigits('45');
        await nextTick();
        expect(base.currentValue.value).toBe('');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 0);
    });

    test('clamps pasted values to min/max and accepts numeric prefixes', async () => {
        const props = reactive<NumberInputProps>({ modelValue: 5, minValue: 2, maxValue: 10 });
        const emit = jest.fn<(event: 'update:modelValue', value: number) => void>();
        const base = useNumberInputBase(props, emit);

        base.onPaste(clipboardCase('999').event);
        await nextTick();
        expect(base.currentValue.value).toBe('10');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 10);

        base.onPaste(clipboardCase('-5').event);
        await nextTick();
        expect(base.currentValue.value).toBe('2');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 2);

        base.onPaste(clipboardCase('8abc').event);
        await nextTick();
        expect(base.currentValue.value).toBe('8');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 8);
    });

    test('renders non-finite values as zero and emits zero for invalid edits', async () => {
        const emit = jest.fn<(event: 'update:modelValue', value: number) => void>();
        const nan = useNumberInputBase({ modelValue: Number.NaN }, emit);
        expect(nan.currentValue.value).toBe('0');

        const infinite = useNumberInputBase({ modelValue: Number.POSITIVE_INFINITY }, emit);
        expect(infinite.currentValue.value).toBe('0');
        infinite.currentValue.value = '--';
        await nextTick();
        expect(infinite.currentValue.value).toBe('');
        expect(emit).toHaveBeenLastCalledWith('update:modelValue', 0);
    });
});
