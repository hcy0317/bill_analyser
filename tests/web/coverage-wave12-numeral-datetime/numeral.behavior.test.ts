import {
    DecimalSeparator,
    DigitGroupingSymbol,
    DigitGroupingType,
    NumeralSystem,
    type NumberFormatOptions
} from '@/core/numeral.ts';
import {
    appendDigitGroupingSymbolAndDecimalSeparator,
    formatAmount,
    formatExchangeRateAmount,
    formatHiddenAmount,
    formatNumber,
    formatPercent,
    getAdaptiveDisplayAmountRate,
    getAmountWithDecimalNumberCount,
    getExchangedAmountByRate,
    parseAmount,
    sumAmounts
} from '@/lib/numeral.ts';

const westernOptions: NumberFormatOptions = {
    numeralSystem: NumeralSystem.WesternArabicNumerals,
    digitGrouping: DigitGroupingType.ThousandsSeparator,
    digitGroupingSymbol: DigitGroupingSymbol.Comma.symbol,
    decimalSeparator: DecimalSeparator.Dot.symbol
};

const commaDecimalOptions: NumberFormatOptions = {
    ...westernOptions,
    digitGroupingSymbol: DigitGroupingSymbol.Dot.symbol,
    decimalSeparator: DecimalSeparator.Comma.symbol
};

describe('numeral formatting behavior coverage', () => {
    test('sums empty, positive, and signed amount collections', () => {
        expect(sumAmounts([])).toBe(0);
        expect(sumAmounts([100, -25, 5])).toBe(80);
    });

    test('normalizes grouping, decimal separators, signs, hidden values, and localized digits', () => {
        expect(appendDigitGroupingSymbolAndDecimalSeparator('', westernOptions)).toBe('');
        expect(appendDigitGroupingSymbolAndDecimalSeparator('1234.56', westernOptions)).toBe('1,234.56');
        expect(appendDigitGroupingSymbolAndDecimalSeparator('-1234.5', westernOptions)).toBe('-1,234.5');
        expect(formatHiddenAmount('***', westernOptions)).toBe('*.**');
        expect(appendDigitGroupingSymbolAndDecimalSeparator('1234,56', commaDecimalOptions)).toBe('1.234,56');

        const easternOptions = {
            ...commaDecimalOptions,
            numeralSystem: NumeralSystem.EasternArabicNumerals
        };
        expect(appendDigitGroupingSymbolAndDecimalSeparator('١٢٣٤٫٥٦', easternOptions)).toBe('١.٢٣٤,٥٦');
        expect(() => appendDigitGroupingSymbolAndDecimalSeparator('12.3x', westernOptions))
            .toThrow('not a valid textual number');
    });

    test('parses integers and every supported decimal-length branch', () => {
        expect(parseAmount(null as unknown as string, westernOptions)).toBe(0);
        expect(parseAmount('', westernOptions)).toBe(0);
        expect(parseAmount('-', westernOptions)).toBe(0);
        expect(parseAmount('123', westernOptions)).toBe(12300);
        expect(parseAmount('1,234.56', westernOptions)).toBe(123456);
        expect(parseAmount('.5', westernOptions)).toBe(50);
        expect(parseAmount('12.', westernOptions)).toBe(1200);
        expect(parseAmount('12.3', westernOptions)).toBe(1230);
        expect(parseAmount('12.34', westernOptions)).toBe(1234);
        expect(parseAmount('12.3456', westernOptions)).toBe(1234);
        expect(parseAmount('-12.34', westernOptions)).toBe(-1234);

        const easternOptions = {
            ...commaDecimalOptions,
            numeralSystem: NumeralSystem.EasternArabicNumerals
        };
        expect(parseAmount('١٢,٣٤', easternOptions)).toBe(1234);
    });

    test('formats amounts across size, precision, trimming, grouping, and sign branches', () => {
        expect(() => formatAmount(Number.NaN, westernOptions)).toThrow('is not amount number');
        expect(() => formatAmount(1.5, westernOptions)).toThrow('is not amount number');
        expect(formatAmount(0, westernOptions)).toBe('0.00');
        expect(formatAmount(1, westernOptions)).toBe('0.01');
        expect(formatAmount(12, westernOptions)).toBe('0.12');
        expect(formatAmount(123, westernOptions)).toBe('1.23');
        expect(formatAmount(123456, westernOptions)).toBe('1,234.56');
        expect(formatAmount(-123456, westernOptions)).toBe('-1,234.56');
        expect(formatAmount(123456, { ...westernOptions, digitGrouping: undefined } as unknown as NumberFormatOptions)).toBe('1234.56');
        expect(formatAmount(1200, { ...westernOptions, decimalNumberCount: 0 })).toBe('12');
        expect(formatAmount(1230, { ...westernOptions, decimalNumberCount: 0 })).toBe('12.3');
        expect(formatAmount(1234, { ...westernOptions, decimalNumberCount: 1 })).toBe('12.34');
        expect(formatAmount(1200, { ...westernOptions, trimTailZero: true })).toBe('12');
        expect(formatAmount(1230, { ...westernOptions, trimTailZero: true })).toBe('12.3');
        expect(formatAmount(1203, { ...westernOptions, trimTailZero: true })).toBe('12.03');
        expect(formatAmount(1234, { ...westernOptions, decimalNumberCount: 99 })).toBe('12.34');
        expect(formatAmount(1234, { ...westernOptions, decimalNumberCount: Number.NaN })).toBe('12.34');

        expect(formatAmount(1234, {
            ...westernOptions,
            numeralSystem: NumeralSystem.PersianDigits,
            decimalSeparator: DecimalSeparator.Comma.symbol
        })).toBe('۱۲,۳۴');
    });

    test('formats generic numbers and low-precision percentages', () => {
        expect(formatNumber(1234.567, westernOptions)).toBe('1,234.567');
        expect(formatNumber(1234.567, westernOptions, 2)).toBe('1,234.56');
        expect(formatPercent(0.001, 2, '<0.01', westernOptions)).toBe('<0.01%');
        expect(formatPercent(0.001, 2, '<0.01', commaDecimalOptions)).toBe('<0,01%');
        expect(formatPercent(12.349, 2, '<0.01', westernOptions)).toBe('12.34%');
        expect(formatPercent(-0.001, 2, '<0.01', westernOptions)).toBe('0%');
    });

    test('applies display decimal-count truncation without changing the stored unit', () => {
        expect(getAmountWithDecimalNumberCount(12345, 0)).toBe(12300);
        expect(getAmountWithDecimalNumberCount(12345, 1)).toBe(12340);
        expect(getAmountWithDecimalNumberCount(12345, 2)).toBe(12345);
    });

    test('formats integer and fractional exchange rates with adaptive precision', () => {
        expect(formatExchangeRateAmount(2, westernOptions)).toBe('2');
        expect(formatExchangeRateAmount(100.234567, westernOptions)).toBe('100.23');
        expect(formatExchangeRateAmount(0.000123456, westernOptions)).toBe('0.0001234');
        expect(formatExchangeRateAmount(0, commaDecimalOptions)).toBe('0');
    });

    test('builds adaptive rate labels and falls back to supplied exchange rates', () => {
        expect(getAdaptiveDisplayAmountRate(0, 0, westernOptions)).toBeNull();
        expect(getAdaptiveDisplayAmountRate(2, 1, westernOptions)).toBe('2 : 1');
        expect(getAdaptiveDisplayAmountRate(1, 2, westernOptions)).toBe('1 : 2');
        expect(getAdaptiveDisplayAmountRate(1, 1, westernOptions, { rate: '3' }, { rate: '1.5' }))
            .toBe('2 : 1');
        expect(getAdaptiveDisplayAmountRate(0, 0, westernOptions, { rate: '' }, { rate: '1' })).toBeNull();
        expect(getAdaptiveDisplayAmountRate(0, 0, westernOptions, { rate: '1' }, { rate: '' })).toBeNull();
    });

    test('converts amounts for valid finite exchange rates', () => {
        expect(getExchangedAmountByRate(100, '2', '3')).toBe(150);
    });
});
