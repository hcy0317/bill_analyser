import { beforeEach, describe, expect, jest, test } from '@jest/globals';
const { nextTick } = jest.requireActual('vue') as typeof import('@/../node_modules/vue');

import { NumeralSystem } from '@/core/numeral.ts';
import { FiscalYearStart } from '@/core/fiscalyear.ts';
import { DateRange } from '@/core/datetime.ts';
import { useDateRangeSelectionBase } from '@/components/base/DateRangeSelectionBase.ts';
import { useMonthRangeSelectionBase } from '@/components/base/MonthRangeSelectionBase.ts';
import { useDateTimeSelectionBase } from '@/components/base/DateTimeSelectionBase.ts';
import { useFiscalYearStartSelectionBase } from '@/components/base/FiscalYearStartSelectionBase.ts';
import { useScheduleFrequencySelectionBase } from '@/components/base/ScheduleFrequencySelectionBase.ts';
import { useLanguageSelectButtonBase } from '@/components/base/LanguageSelectBase.ts';

const mockUserStore = {
    currentUserFirstDayOfWeek: 1,
    currentUserFiscalYearStart: FiscalYearStart.Default.value
};

const mockSettingsStore = {
    updateLocalizedDefaultSettings: jest.fn<(value: unknown) => void>()
};

let mockNumeralSystem = NumeralSystem.WesternArabicNumerals;
let mockUnixTimeOverride: number | undefined;
let mockCurrentLanguage = 'en-US';
let mockMeridiemIndicatorFirst = true;

const mockSetLanguage = jest.fn<(value: string) => Record<string, unknown>>((value) => ({ locale: value }));
const mockGetDateRangeByDateType = jest.fn((type: number, _firstDay: number, _fiscalStart: number) => ({
    minTime: type * 100,
    maxTime: type * 100 + 50,
    dateType: type
}));

jest.mock('@/stores/user.ts', () => ({
    __esModule: true,
    useUserStore: () => mockUserStore
}));

jest.mock('@/stores/setting.ts', () => ({
    __esModule: true,
    useSettingsStore: () => mockSettingsStore
}));

jest.mock('@/locales/helpers.ts', () => ({
    __esModule: true,
    useI18n: () => ({
        tt: (value: string) => `tt:${value}`,
        ti: (value: string, translate: boolean) => translate ? `ti:${value}` : value,
        formatUnixTimeToLongDateTime: (value: number) => `datetime:${value}`,
        formatUnixTimeToGregorianLikeLongYearMonth: (value: number) => `month:${value}`,
        getAllMeridiemIndicators: () => [{ name: 'AM', value: 0 }, { name: 'PM', value: 1 }],
        getCurrentNumeralSystemType: () => mockNumeralSystem,
        isLongTime24HourFormat: () => false,
        isLongTimeMeridiemIndicatorFirst: () => mockMeridiemIndicatorFirst,
        isLongTimeHourTwoDigits: () => true,
        isLongTimeMinuteTwoDigits: () => false,
        isLongTimeSecondTwoDigits: () => true,
        formatGregorianTextualMonthDayToGregorianLikeLongMonthDay: (value: string, numeral: NumeralSystem) => `${numeral.type}:${value}`,
        getAllWeekDays: (firstDay: number) => [{ type: firstDay, displayName: `weekday:${firstDay}` }],
        getAllTransactionScheduledFrequencyTypes: () => [{ type: 1, displayName: 'daily' }],
        getMonthdayShortName: (day: number) => `day:${day}`,
        getCurrentLanguageTag: () => mockCurrentLanguage,
        getCurrentLanguageDisplayName: () => `current:${mockCurrentLanguage}`,
        getAllLanguageOptions: (includeSystem: boolean) => includeSystem
            ? [{ languageTag: '', displayName: 'System' }, { languageTag: 'en-US', displayName: 'English' }]
            : [{ languageTag: 'en-US', displayName: 'English' }],
        getLanguageInfo: (value: string) => value === 'zh-Hans'
            ? { languageTag: value, displayName: '简体中文' }
            : undefined,
        setLanguage: (value: string) => {
            mockCurrentLanguage = value;
            return mockSetLanguage(value);
        }
    })
}));

jest.mock('@/lib/datetime.ts', () => ({
    __esModule: true,
    getCurrentUnixTime: () => 2_000,
    getTodayFirstUnixTime: () => 1_000,
    getThisYearFirstUnixTime: () => 10_000,
    getThisYearLastUnixTime: () => 20_000,
    getLocalDatetimeFromUnixTime: (value: number) => new Date(value * 1_000),
    getUnixTimeFromLocalDatetime: (value: Date) => mockUnixTimeOverride ?? Math.trunc(value.getTime() / 1_000),
    getDummyUnixTimeForLocalUsage: (value: number, timezone: number, browserTimezone: number) => value + timezone - browserTimezone,
    getActualUnixTimeForStore: (value: number, timezone: number, browserTimezone: number) => value - timezone + browserTimezone,
    getTimezoneOffsetMinutes: () => 60,
    getBrowserTimezoneOffsetMinutes: () => 0,
    getDateRangeByDateType: (type: number, firstDay: number, fiscalStart: number) => mockGetDateRangeByDateType(type, firstDay, fiscalStart),
    getYear0BasedMonthObjectFromUnixTime: (value: number) => value === 10_000
        ? { year: 2025, month0base: 0 }
        : { year: 2025, month0base: 5 },
    getYear0BasedMonthObjectFromString: (value: string) => {
        const match = /^(\d{4})-(\d{2})$/.exec(value);
        return match ? { year: Number(match[1]), month0base: Number(match[2]) - 1 } : undefined;
    },
    getYearMonthStringFromYear0BasedMonthObject: (value: { year: number; month0base: number }) => `${value.year}-${String(value.month0base + 1).padStart(2, '0')}`,
    getYearMonthFirstUnixTime: (value: { year: number; month0base: number }) => value.year * 100 + value.month0base,
    getYearMonthLastUnixTime: (value: { year: number; month0base: number }) => value.year * 100 + value.month0base + 1
}));

describe('date and month range selection behavior', () => {
    beforeEach(() => {
        mockUnixTimeOverride = undefined;
        mockGetDateRangeByDateType.mockClear();
    });

    test('uses explicit date bounds, formats them, and builds all preset ranges', () => {
        const base = useDateRangeSelectionBase({ show: true, minTime: 100, maxTime: 300 });

        expect(base.beginDateTime.value).toBe('datetime:100');
        expect(base.endDateTime.value).toBe('datetime:300');
        expect(base.presetRanges.value).toHaveLength(6);
        expect(base.presetRanges.value.map(item => item.label)).toEqual([
            DateRange.ThisWeek,
            DateRange.ThisMonth,
            DateRange.ThisYear,
            DateRange.LastYear,
            DateRange.ThisFiscalYear,
            DateRange.LastFiscalYear
        ].map(item => `tt:${item.name}`));
        expect(mockGetDateRangeByDateType).toHaveBeenCalledTimes(6);
        expect(base.getFinalDateRange()).toEqual({ minUnixTime: 100, maxUnixTime: 300 });
    });

    test('falls back to today/current bounds and handles incomplete or too-early dates', () => {
        const base = useDateRangeSelectionBase({ show: true, minTime: 0, maxTime: 0 });

        expect(base.beginDateTime.value).toBe('datetime:1000');
        expect(base.endDateTime.value).toBe('datetime:2000');

        base.dateRange.value = [undefined as unknown as Date, new Date(0)];
        expect(base.getFinalDateRange()).toBeNull();

        base.dateRange.value = [new Date(0), new Date(1_000)];
        mockUnixTimeOverride = -1;
        expect(() => base.getFinalDateRange()).toThrow('Date is too early');
    });

    test('uses valid textual month bounds and returns canonical year-month values', () => {
        const base = useMonthRangeSelectionBase({ show: true, minTime: '2023-03', maxTime: '2024-11' });

        expect(base.beginDateTime.value).toBe('month:202302');
        expect(base.endDateTime.value).toBe('month:202411');
        expect(base.getFinalMonthRange()).toEqual({ minYearMonth: '2023-03', maxYearMonth: '2024-11' });
    });

    test('falls back for malformed months and rejects incomplete or invalid values', () => {
        const missing = useMonthRangeSelectionBase({ show: true });
        expect(missing.dateRange.value).toEqual([{ year: 2025, month0base: 0 }, { year: 2025, month0base: 5 }]);

        const base = useMonthRangeSelectionBase({ show: true, minTime: 'bad' as '2020-01', maxTime: 'also-bad' as '2020-01' });
        expect(base.dateRange.value).toEqual([{ year: 2025, month0base: 0 }, { year: 2025, month0base: 5 }]);

        base.dateRange.value = [undefined as unknown as { year: number; month0base: number }, { year: 2025, month0base: 0 }];
        expect(base.getFinalMonthRange()).toBeNull();

        base.dateRange.value = [{ year: 0, month0base: 0 }, { year: 2025, month0base: 0 }];
        expect(() => base.getFinalMonthRange()).toThrow('Date is too early');
        base.dateRange.value = [{ year: 2025, month0base: 0 }, { year: 2025, month0base: -1 }];
        expect(() => base.getFinalMonthRange()).toThrow('Date is too early');
    });
});

describe('time, fiscal-year, and schedule selection behavior', () => {
    beforeEach(() => {
        mockNumeralSystem = NumeralSystem.WesternArabicNumerals;
        mockMeridiemIndicatorFirst = true;
    });

    test('generates 12-hour and 24-hour picker values with localized digits', () => {
        const base = useDateTimeSelectionBase();

        expect(base.is24Hour.value).toBe(false);
        expect(base.isHourTwoDigits.value).toBe(true);
        expect(base.isMinuteTwoDigits.value).toBe(false);
        expect(base.isSecondTwoDigits.value).toBe(true);
        expect(base.isMeridiemIndicatorFirst.value).toBe(true);
        expect(base.meridiemItems.value).toHaveLength(2);
        expect(base.getDisplayTimeValue(7, true)).toBe('07');
        expect(base.generateAllHours(2, false)).toHaveLength(24);
        expect(base.generateAllHours(1, true)[0]).toEqual({ value: '12', itemsIndex: 0 });
        expect(base.generateAllMinutesOrSeconds(2, true)).toHaveLength(120);
        expect(base.generateAllMinutesOrSeconds(1, true)[9]).toEqual({ value: '09', itemsIndex: 0 });

        mockNumeralSystem = NumeralSystem.EasternArabicNumerals;
        const localizedBase = useDateTimeSelectionBase();
        localizedBase.is24Hour.value = true;
        const hours = localizedBase.generateAllHours(1, true);
        expect(hours).toHaveLength(24);
        expect(hours[0]?.value).toBe('٠٠');
        expect(hours[23]?.value).toBe('٢٣');

        mockMeridiemIndicatorFirst = false;
        expect(useDateTimeSelectionBase().isMeridiemIndicatorFirst.value).toBe(false);
    });

    test('normalizes fiscal-year props and exposes date, display, and leap-day behavior', () => {
        const aprilFirst = FiscalYearStart.of(4, 1)!;
        const base = useFiscalYearStartSelectionBase({ modelValue: aprilFirst.value, numeralSystem: NumeralSystem.EasternArabicNumerals.type });

        expect(base.selectedFiscalYearStart.value).toBe(aprilFirst.value);
        expect(base.selectedFiscalYearStartValue.value.getMonth()).toBe(3);
        expect(base.displayFiscalYearStartDate.value).toBe(`${NumeralSystem.EasternArabicNumerals.type}:04-01`);
        expect(base.allowedMinDate.value.getTime()).toBe(10_000_000);
        expect(base.allowedMaxDate.value.getTime()).toBe(20_000_000);
        expect(base.disabledDates(new Date(2024, 1, 29))).toBe(true);
        expect(base.disabledDates(new Date(2024, 1, 28))).toBe(false);

        base.selectedFiscalYearStartValue.value = new Date(2025, 6, 15);
        expect(base.selectedFiscalYearStart.value).toBe(FiscalYearStart.of(7, 15)!.value);
        base.selectedFiscalYearStartValue.value = new Date(2024, 1, 29);
        expect(base.selectedFiscalYearStart.value).toBe(FiscalYearStart.Default.value);
    });

    test('uses defaults for missing/invalid fiscal-year values and numeral systems', () => {
        const missing = useFiscalYearStartSelectionBase({});
        expect(missing.selectedFiscalYearStart.value).toBe(FiscalYearStart.Default.value);

        const invalid = useFiscalYearStartSelectionBase({ modelValue: 999_999, numeralSystem: 999_999 });
        expect(invalid.selectedFiscalYearStart.value).toBe(FiscalYearStart.Default.value);
        invalid.selectedFiscalYearStart.value = 999_999;
        expect(invalid.selectedFiscalYearStartValue.value.getMonth()).toBe(0);
        expect(invalid.displayFiscalYearStartDate.value).toBe(`${NumeralSystem.Default.type}:01-01`);

        mockNumeralSystem = NumeralSystem.PersianDigits;
        const localeBased = useFiscalYearStartSelectionBase({ modelValue: FiscalYearStart.Default.value });
        expect(localeBased.displayFiscalYearStartDate.value).toBe(`${NumeralSystem.PersianDigits.type}:01-01`);
    });

    test('builds schedule options and parses sorted frequency values', () => {
        const base = useScheduleFrequencySelectionBase();

        expect(base.allTransactionScheduledFrequencyTypes.value).toEqual([{ type: 1, displayName: 'daily' }]);
        expect(base.allWeekDays.value).toEqual([{ type: 1, displayName: 'weekday:1' }]);
        expect(base.allAvailableMonthDays.value).toHaveLength(28);
        expect(base.allAvailableMonthDays.value[27]).toEqual({ day: 28, displayName: 'day:28' });
        expect(base.getFrequencyValues('9,,2,5')).toEqual([2, 5, 9]);
        expect(base.getFrequencyValues('')).toEqual([]);
    });
});

describe('language selection behavior', () => {
    beforeEach(() => {
        mockCurrentLanguage = 'en-US';
        mockSetLanguage.mockClear();
        mockSettingsStore.updateLocalizedDefaultSettings.mockClear();
    });

    test('updates the actual locale and persists localized defaults', async () => {
        const emit = jest.fn<(event: 'update:modelValue', value: string) => void>();
        const base = useLanguageSelectButtonBase({ includeSystemDefault: true }, emit);

        expect(base.allLanguages.value).toHaveLength(2);
        expect(base.currentLocale.value).toBe('en-US');
        expect(base.currentLanguageName.value).toBe('current:en-US');
        expect(base.isLanguageSelected('en-US')).toBe(true);

        base.updateLanguage('fr-FR');
        await nextTick();
        expect(mockSetLanguage).toHaveBeenCalledWith('fr-FR');
        expect(mockSettingsStore.updateLocalizedDefaultSettings).toHaveBeenCalledWith({ locale: 'fr-FR' });
        expect(base.currentLocale.value).toBe('en-US');
        expect(base.isLanguageSelected('fr-FR')).toBe(false);
        expect(emit).not.toHaveBeenCalled();
    });

    test('uses model values without mutating application locale', () => {
        const emit = jest.fn<(event: 'update:modelValue', value: string) => void>();
        const { reactive } = jest.requireActual('vue') as typeof import('@/../node_modules/vue');
        const props = reactive({ useModelValue: true, modelValue: 'zh-Hans' });
        const base = useLanguageSelectButtonBase(props, emit);

        expect(base.allLanguages.value).toHaveLength(1);
        expect(base.currentLanguageName.value).toBe('简体中文');
        expect(base.isLanguageSelected('zh-Hans')).toBe(true);
        expect(base.isLanguageSelected('en-US')).toBe(false);
        base.updateLanguage('en-US');
        expect(emit).toHaveBeenCalledWith('update:modelValue', 'en-US');

        props.modelValue = 'unknown';
        expect(base.currentLanguageName.value).toBe('');
    });
});
