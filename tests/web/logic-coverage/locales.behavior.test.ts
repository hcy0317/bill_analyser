import { afterAll, beforeEach, describe, expect, jest, test } from '@jest/globals';
import moment from 'moment-timezone';

import enMessages from '@/locales/en.json';

const mockLocale = { value: 'en' };
const mockFallbackOnlyKeys = new Set(['Fallback Message']);

function localeValue(key: string): unknown {
    let current: unknown = enMessages;

    for (const part of key.split('.')) {
        if (!current || typeof current !== 'object' || !Object.prototype.hasOwnProperty.call(current, part)) {
            return undefined;
        }

        current = (current as Record<string, unknown>)[part];
    }

    return current;
}

function interpolate(value: string, parameters?: Record<string, unknown>): string {
    if (!parameters) {
        return value;
    }

    return value.replace(/\{(\w+)\}/g, (_match, key: string) => String(parameters[key] ?? `{${key}}`));
}

const mockT = jest.fn((key: string, parameters?: Record<string, unknown>) => {
    const value = localeValue(key);
    return interpolate(typeof value === 'string' ? value : key, parameters);
});
const mockHasLocaleMessage = jest.fn((key: string, requestedLocale?: string) => {
    if (mockFallbackOnlyKeys.has(key)) {
        return requestedLocale === 'en';
    }

    return localeValue(key) !== undefined;
});

const mockSettingsStore = {
    appSettings: {
        currencySortByInExchangeRatesPage: 1
    }
};

const mockUserStore = {
    currentUserCalendarDisplayType: 1,
    currentUserCurrencyDisplayType: 2,
    currentUserDateDisplayType: 1,
    currentUserDecimalSeparator: 1,
    currentUserDefaultCurrency: 'CNY',
    currentUserDigitGrouping: 2,
    currentUserDigitGroupingSymbol: 2,
    currentUserFiscalYearFormat: 4,
    currentUserFiscalYearStart: 0x0101,
    currentUserLongDateFormat: 1,
    currentUserLongTimeFormat: 1,
    currentUserNumeralSystem: 1,
    currentUserShortDateFormat: 1,
    currentUserShortTimeFormat: 1
};

const mockGetExchangedAmount = jest.fn<(amount: number, from: string, to: string) => number | undefined>();
const mockSetLocale = jest.fn<(language: string) => void>();
const mockSetSessionLanguage = jest.fn<(language: string) => void>();
const mockLoggerInfo = jest.fn<(message: string) => void>();
const mockLoggerWarn = jest.fn<(message: string) => void>();
let mockSessionLanguage = '';

jest.mock('vue-i18n', () => ({
    useI18n: () => ({
        t: mockT,
        te: mockHasLocaleMessage,
        locale: mockLocale
    })
}));

jest.mock('@/stores/setting.ts', () => ({
    useSettingsStore: () => mockSettingsStore
}));

jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => mockUserStore
}));

jest.mock('@/stores/exchangeRates.ts', () => ({
    useExchangeRatesStore: () => ({
        getExchangedAmount: mockGetExchangedAmount
    })
}));

jest.mock('@/lib/settings.ts', () => ({
    getSessionCurrentLanguageKey: () => mockSessionLanguage,
    setSessionCurrentLanguageKey: (language: string) => mockSetSessionLanguage(language)
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        setLocale: (language: string) => mockSetLocale(language)
    }
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        info: (message: string) => mockLoggerInfo(message),
        warn: (message: string) => mockLoggerWarn(message)
    }
}));

import { AccountCategory, AccountType } from '@/core/account.ts';
import {
    CalendarDisplayType,
    CalendarType,
    DateDisplayType
} from '@/core/calendar.ts';
import { CategoryType } from '@/core/category.ts';
import { CoordinateDisplayType } from '@/core/coordinate.ts';
import { CurrencySortingType } from '@/core/currency.ts';
import {
    DateRange,
    DateRangeScene,
    LongDateFormat,
    LongTimeFormat,
    ShortDateFormat,
    ShortTimeFormat,
    WeekDay
} from '@/core/datetime.ts';
import { FiscalYearFormat, FiscalYearStart } from '@/core/fiscalyear.ts';
import {
    DecimalSeparator,
    DigitGroupingSymbol,
    DigitGroupingType,
    NumeralSystem
} from '@/core/numeral.ts';
import { StatisticsAnalysisType } from '@/core/statistics.ts';
import { TextDirection } from '@/core/text.ts';
import { KnownErrorCode } from '@/consts/api.ts';
import { DISPLAY_HIDDEN_AMOUNT, INCOMPLETE_AMOUNT_SUFFIX } from '@/consts/numeral.ts';
import { ALL_LANGUAGES, DEFAULT_LANGUAGE } from '@/locales/index.ts';
import { Account, type AccountInfoResponse } from '@/models/account.ts';
import {
    getI18nOptions,
    getRtlLocales,
    useI18n
} from '@/locales/helpers.ts';

const htmlAttributes = new Map<string, string>();
const mockHtmlElement = {
    setAttribute: jest.fn((name: string, value: string) => htmlAttributes.set(name, value)),
    getAttribute: jest.fn((name: string) => htmlAttributes.get(name) ?? null),
    removeAttribute: jest.fn((name: string) => htmlAttributes.delete(name))
};
const mockReplaceLocation = jest.fn<(url: string) => void>();

function installBrowser(language = 'en-US', search = ''): void {
    const location = {
        href: `https://example.test/${search}`,
        search,
        replace: mockReplaceLocation
    };
    const windowValue = {
        navigator: {
            browserLanguage: language,
            language
        },
        location
    };

    Object.defineProperty(globalThis, 'window', { configurable: true, value: windowValue, writable: true });
    Object.defineProperty(globalThis, 'location', { configurable: true, value: location, writable: true });
    Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: {
            querySelector: jest.fn((selector: string) => selector === 'html' ? mockHtmlElement : null)
        },
        writable: true
    });
}

function accountResponse(overrides: Partial<AccountInfoResponse> = {}): AccountInfoResponse {
    return {
        id: 'account-1',
        name: 'Account',
        parentId: '0',
        category: AccountCategory.Cash.type,
        type: AccountType.SingleAccount.type,
        icon: AccountCategory.Cash.defaultAccountIconId,
        color: '#112233',
        currency: 'CNY',
        balanceCents: 10_000,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function account(overrides: Partial<AccountInfoResponse> = {}): Account {
    return Account.of(accountResponse(overrides));
}

function resetUserPreferences(): void {
    Object.assign(mockUserStore, {
        currentUserCalendarDisplayType: CalendarDisplayType.Gregorian.type,
        currentUserCurrencyDisplayType: 2,
        currentUserDateDisplayType: DateDisplayType.Gregorian.type,
        currentUserDecimalSeparator: DecimalSeparator.Dot.type,
        currentUserDefaultCurrency: 'CNY',
        currentUserDigitGrouping: DigitGroupingType.ThousandsSeparator.type,
        currentUserDigitGroupingSymbol: DigitGroupingSymbol.Comma.type,
        currentUserFiscalYearFormat: FiscalYearFormat.EndYYYY.type,
        currentUserFiscalYearStart: FiscalYearStart.JanuaryFirstDay.value,
        currentUserLongDateFormat: LongDateFormat.YYYYMMDD.type,
        currentUserLongTimeFormat: LongTimeFormat.HHMMSS.type,
        currentUserNumeralSystem: NumeralSystem.WesternArabicNumerals.type,
        currentUserShortDateFormat: ShortDateFormat.YYYYMMDD.type,
        currentUserShortTimeFormat: ShortTimeFormat.HHMM.type
    });
}

beforeEach(() => {
    jest.clearAllMocks();
    mockLocale.value = 'en';
    mockSessionLanguage = '';
    mockSettingsStore.appSettings.currencySortByInExchangeRatesPage = 1;
    mockGetExchangedAmount.mockImplementation(amount => amount * 2);
    htmlAttributes.clear();
    resetUserPreferences();
    installBrowser();
    moment.tz.setDefault('UTC');
});

afterAll(() => {
    moment.tz.setDefault();
});

describe('locale bootstrap and translation behavior', () => {
    test('builds Vue i18n options and reports configured RTL languages', () => {
        const options = getI18nOptions() as Record<string, unknown>;

        expect(options).toMatchObject({
            legacy: false,
            locale: DEFAULT_LANGUAGE,
            fallbackLocale: DEFAULT_LANGUAGE,
            formatFallbackMessages: true
        });
        expect((options['messages'] as Record<string, object>)['en']).toBeDefined();
        expect(getRtlLocales()).toStrictEqual({});

        const temporaryLanguage = {
            name: 'Test RTL',
            displayName: 'Test RTL',
            alternativeLanguageTag: 'xx-RTL',
            textDirection: 'rtl' as const,
            content: {}
        };
        ALL_LANGUAGES['xx-RTL'] = temporaryLanguage;

        try {
            expect(getRtlLocales()).toStrictEqual({ 'xx-RTL': true });
        } finally {
            delete ALL_LANGUAGES['xx-RTL'];
        }
    });

    test('translates known keys, preserves runtime text, and localizes structured errors', () => {
        const i18n = useI18n();

        expect(i18n.tm('All')).toBe('All');
        expect(i18n.tm('Fallback Message')).toBe('Fallback Message');
        expect(mockHasLocaleMessage).toHaveBeenCalledWith('Fallback Message', DEFAULT_LANGUAGE);
        expect(i18n.tm('Untranslated runtime detail')).toBe('Untranslated runtime detail');
        expect(i18n.tm('format.misc.loginWithCustomProvider', { name: 'Acme' })).toBe('Log in with Acme');
        expect(i18n.tm('   ')).toBe('   ');

        expect(i18n.ti(undefined, true)).toBe('');
        expect(i18n.ti('All', true)).toBe('All');
        expect(i18n.ti('runtime', false)).toBe('runtime');

        expect(i18n.te('All')).toBe('All');
        expect(i18n.te({
            error: {
                success: false,
                errorCode: KnownErrorCode.ApiNotFound,
                message: 'not found',
                path: '/api/auth/register'
            }
        })).toBe('User registration is disabled');
        expect(i18n.te({
            error: {
                success: false,
                errorCode: KnownErrorCode.ValidatorError,
                message: 'parameter "currency" must be less than 4 characters',
                path: '/api/example'
            }
        })).toBe('Currency must be at most 4 characters');
        expect(i18n.te({
            error: {
                success: false,
                errorCode: KnownErrorCode.ValidatorError,
                message: 'unknown validator detail',
                path: '/api/example'
            }
        })).toBe('error.unknown validator detail');
        expect(i18n.te({
            error: {
                success: false,
                errorCode: KnownErrorCode.UserEmailNotVerified,
                message: 'email pending',
                path: '/api/example'
            }
        })).toBe('error.email pending');
        expect(i18n.te({} as never)).toBe('');
    });

    test('joins localized text and chooses current-language or default server content', () => {
        const i18n = useI18n();

        expect(i18n.joinMultiText([])).toBe('');
        expect(i18n.joinMultiText(null as unknown as string[])).toBe('');
        expect(i18n.joinMultiText(['One', 'Two'])).toBe('One, Two');
        expect(i18n.getServerMultiLanguageConfigContent(null as unknown as Record<string, string>)).toBe('');
        expect(i18n.getServerMultiLanguageConfigContent({ en: 'English', default: 'Default' })).toBe('English');
        expect(i18n.getServerMultiLanguageConfigContent({ default: 'Default' })).toBe('Default');
        expect(i18n.getServerMultiLanguageConfigContent({ en: 1 as unknown as string, default: '' })).toBe('');
    });
});

describe('language metadata and localized option catalogs', () => {
    test('normalizes aliases and exposes current language metadata and direction', () => {
        const i18n = useI18n();

        expect(i18n.getLanguageInfo(' zh_CN ')).toBe(ALL_LANGUAGES['zh-Hans']);
        expect(i18n.getLanguageInfo('ZH-tw')).toBe(ALL_LANGUAGES['zh-Hant']);
        expect(i18n.getLanguageInfo('EN')).toBe(ALL_LANGUAGES['en']);
        expect(i18n.getLanguageInfo('')).toBeUndefined();
        expect(i18n.getLanguageInfo('   ')).toBeUndefined();
        expect(i18n.getLanguageInfo('unknown')).toBeUndefined();
        expect(i18n.getCurrentLanguageTag()).toBe('en');
        expect(i18n.getCurrentLanguageInfo()).toBe(ALL_LANGUAGES['en']);
        expect(i18n.getCurrentLanguageDisplayName()).toBe('English');
        expect(i18n.getCurrentLanguageTextDirection()).toBe(TextDirection.LTR);

        const originalDirection = ALL_LANGUAGES['en']!.textDirection;
        (ALL_LANGUAGES['en'] as { textDirection: 'ltr' | 'rtl' }).textDirection = 'rtl';
        try {
            expect(i18n.getCurrentLanguageTextDirection()).toBe(TextDirection.RTL);
        } finally {
            (ALL_LANGUAGES['en'] as { textDirection: 'ltr' | 'rtl' }).textDirection = originalDirection;
        }

        mockLocale.value = 'missing';
        expect(i18n.getCurrentLanguageInfo()).toBe(ALL_LANGUAGES['en']);
    });

    test('returns sorted language, currency, calendar, numeral, and domain option catalogs', () => {
        const i18n = useI18n();
        const languageOptions = i18n.getAllLanguageOptions(false);
        const languageOptionsWithDefault = i18n.getAllLanguageOptions(true);

        expect(languageOptions).toHaveLength(Object.keys(ALL_LANGUAGES).length);
        expect(languageOptions.map(option => option.languageTag)).toStrictEqual(
            [...languageOptions.map(option => option.languageTag)].sort((left, right) => left.localeCompare(right))
        );
        expect(languageOptionsWithDefault[0]).toStrictEqual({
            languageTag: '',
            displayName: '',
            nativeDisplayName: 'System Default'
        });
        expect(i18n.getAllEnableDisableOptions()).toStrictEqual([
            { value: true, displayName: 'Enable' },
            { value: false, displayName: 'Disable' }
        ]);
        expect(i18n.getAllCurrencies()).toEqual(expect.arrayContaining([
            { currencyCode: 'CNY', displayName: 'Chinese Yuan' },
            { currencyCode: 'USD', displayName: 'United States Dollar' }
        ]));
        expect(i18n.getAllMeridiemIndicators()).toEqual(expect.arrayContaining([
            { name: 'AM', value: 'AM' },
            { name: 'PM', value: 'PM' }
        ]));
        expect(i18n.getAllLongMonthNames()).toHaveLength(12);
        expect(i18n.getAllShortMonthNames()).toHaveLength(12);
        expect(i18n.getAllLongWeekdayNames()).toHaveLength(7);
        expect(i18n.getAllShortWeekdayNames()).toHaveLength(7);
        expect(i18n.getAllMinWeekdayNames()).toHaveLength(7);
        expect(i18n.getAllWeekDays()).toHaveLength(7);
        expect(i18n.getAllWeekDays(WeekDay.Monday.type)[0]!.type).toBe(WeekDay.Monday.type);

        expect(i18n.getAllCalendarDisplayTypes()).toHaveLength(CalendarDisplayType.values().length + 1);
        expect(i18n.getAllDateDisplayTypes()).toHaveLength(DateDisplayType.values().length + 1);
        expect(i18n.getAllNumeralSystemTypes()).toHaveLength(NumeralSystem.values().length + 1);
        expect(i18n.getAllDecimalSeparators()).toHaveLength(DecimalSeparator.values().length + 1);
        expect(i18n.getAllDigitGroupingSymbols()).toHaveLength(DigitGroupingSymbol.values().length + 1);
        expect(i18n.getAllDigitGroupingTypes(NumeralSystem.WesternArabicNumerals, ',')).toHaveLength(DigitGroupingType.values().length + 1);
        expect(i18n.getAllCurrencyDisplayTypes(NumeralSystem.WesternArabicNumerals, '.')).toHaveLength(12);
        expect(i18n.getLocaleDefaultCalendarDisplayType()).toBe(CalendarDisplayType.Gregorian);
        expect(i18n.getLocaleDefaultDateDisplayType()).toBe(DateDisplayType.Gregorian);
        expect(i18n.getLocaleDefaultNumeralSystemType()).toBe(NumeralSystem.WesternArabicNumerals);
        expect(i18n.getLocaleDefaultDecimalSeparator()).toBe(DecimalSeparator.Dot);
        expect(i18n.getLocaleDefaultDigitGroupingSymbol()).toBe(DigitGroupingSymbol.Comma);
        expect(i18n.getLocaleDefaultDigitGroupingType()).toBe(DigitGroupingType.ThousandsSeparator);
        expect(i18n.getLocaleFiscalYearFormatType()).toBe(FiscalYearFormat.EndYYYY);

        const arrayCatalogs = [
            i18n.getAllCurrencySortingTypes(),
            i18n.getAllCoordinateDisplayTypes(),
            i18n.getAllExpenseAmountColors(),
            i18n.getAllIncomeAmountColors(),
            i18n.getAllAccountCategories(),
            i18n.getAllAccountTypes(),
            i18n.getAllCategoricalChartTypes(false),
            i18n.getAllCategoricalChartTypes(true),
            i18n.getAllTrendChartTypes(),
            i18n.getAllAccountBalanceTrendChartTypes(),
            i18n.getAllStatisticsChartDataTypes(StatisticsAnalysisType.CategoricalAnalysis, false),
            i18n.getAllStatisticsSortingTypes(),
            i18n.getAllStatisticsDateAggregationTypes(StatisticsAnalysisType.TrendAnalysis),
            i18n.getAllStatisticsDateAggregationTypesWithShortName(StatisticsAnalysisType.TrendAnalysis),
            i18n.getAllTransactionEditScopeTypes(),
            i18n.getAllTransactionTagFilterTypes(),
            i18n.getAllTransactionScheduledFrequencyTypes(),
            i18n.getAllImportTransactionColumnTypes()
        ];

        for (const catalog of arrayCatalogs) {
            expect(catalog.length).toBeGreaterThan(0);
            expect(catalog[0]).toEqual(expect.objectContaining({ type: expect.any(Number) }));
        }

        const coordinateDefault = CoordinateDisplayType.Default as unknown as { name: string };
        const originalCoordinateName = coordinateDefault.name;
        coordinateDefault.name = '';
        try {
            expect(i18n.getAllCoordinateDisplayTypes()[0]!.displayName).toBe('System Default');
        } finally {
            coordinateDefault.name = originalCoordinateName;
        }
    });

    test('falls back from invalid localized defaults while honoring explicit user choices', () => {
        const i18n = useI18n();
        const originalImplementation = mockT.getMockImplementation()!;
        mockT.mockImplementation((key: string, parameters?: Record<string, unknown>) => {
            if (key.startsWith('default.')) {
                return 'invalid-default';
            }
            return originalImplementation(key, parameters);
        });

        try {
            mockUserStore.currentUserCalendarDisplayType = 999;
            mockUserStore.currentUserDateDisplayType = 999;
            mockUserStore.currentUserNumeralSystem = 999;
            mockUserStore.currentUserDecimalSeparator = 999;
            mockUserStore.currentUserDigitGroupingSymbol = 999;
            mockUserStore.currentUserDigitGrouping = 999;
            mockUserStore.currentUserFiscalYearFormat = 999;
            mockUserStore.currentUserCurrencyDisplayType = 999;

            expect(i18n.getLocaleDefaultCalendarDisplayType()).toBe(CalendarDisplayType.Default);
            expect(i18n.getLocaleDefaultDateDisplayType()).toBe(DateDisplayType.Default);
            expect(i18n.getLocaleDefaultNumeralSystemType()).toBe(NumeralSystem.Default);
            expect(i18n.getLocaleDefaultDecimalSeparator()).toBe(DecimalSeparator.Default);
            expect(i18n.getLocaleDefaultDigitGroupingSymbol()).toBe(DigitGroupingSymbol.Default);
            expect(i18n.getLocaleDefaultDigitGroupingType()).toBe(DigitGroupingType.Default);
            expect(i18n.getLocaleFiscalYearFormatType()).toBe(FiscalYearFormat.Default);
            expect(i18n.getCurrentCalendarDisplayType()).toBe(CalendarDisplayType.Default);
            expect(i18n.getCurrentDateDisplayType()).toBe(DateDisplayType.Default);
            expect(i18n.getCurrentNumeralSystemType()).toBe(NumeralSystem.Default);
            expect(i18n.getCurrentDecimalSeparator()).toBe(DecimalSeparator.Default.symbol);
            expect(i18n.getCurrentDigitGroupingSymbol()).toBe(DigitGroupingSymbol.Default.symbol);
            expect(i18n.getCurrentDigitGroupingType()).toBe(DigitGroupingType.Default);
            expect(i18n.getCurrentFiscalYearFormatType()).toBe(FiscalYearFormat.Default.type);

            expect(i18n.getAllCalendarDisplayTypes()[0]!.displayName).toContain('Gregorian');
            expect(i18n.getAllDateDisplayTypes()[0]!.displayName).toContain('Gregorian');
            expect(i18n.getAllNumeralSystemTypes()[0]!.displayName).toContain(NumeralSystem.Default.textualAllDigits);
            expect(i18n.getAllDecimalSeparators()[0]!.symbol).toBe(DecimalSeparator.Default.symbol);
            expect(i18n.getAllDigitGroupingSymbols()[0]!.symbol).toBe(DigitGroupingSymbol.Default.symbol);
            expect(i18n.getAllDigitGroupingTypes(NumeralSystem.Default, ',')[0]!.enabled).toBe(DigitGroupingType.Default.enabled);
            expect(i18n.getAllCurrencyDisplayTypes(NumeralSystem.Default, '.')[0]).toBeDefined();
            expect(i18n.getAllFiscalYearFormats(NumeralSystem.Default, CalendarType.Gregorian)[0]).toBeDefined();
            expect(i18n.formatAmountToLocalizedNumeralsWithCurrency(12345, 'CNY')).toContain('¥');
        } finally {
            mockT.mockImplementation(originalImplementation);
        }
    });
});

describe('date, calendar, and timezone formatting', () => {
    test('returns localized names, formats, ranges, and date parsing results', () => {
        const i18n = useI18n();
        const timestamp = moment.utc('2026-07-15T13:45:30Z').unix();

        expect(i18n.getMonthShortName('July')).toBe('Jul');
        expect(i18n.getMonthLongName('July')).toBe('July');
        expect(i18n.getMonthdayOrdinal(15)).toBe('15th');
        expect(i18n.getMonthdayShortName(15)).toBe('15th day');
        expect(i18n.getWeekdayShortName(WeekDay.Wednesday)).toBe('Wed');
        expect(i18n.getWeekdayLongName(WeekDay.Wednesday)).toBe('Wednesday');
        expect(i18n.getMultiMonthdayShortNames(null as unknown as number[])).toBe('');
        expect(i18n.getMultiMonthdayShortNames([1])).toBe('1th day');
        expect(i18n.getMultiMonthdayShortNames([1, 15])).toBe('1th, 15th days');
        expect(i18n.getMultiWeekdayLongNames([WeekDay.Monday.type, WeekDay.Wednesday.type], WeekDay.Monday.type)).toBe('Monday, Wednesday');
        expect(i18n.getAllLocalizedDigits()).toStrictEqual(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']);

        expect(i18n.getAllLongDateFormats(NumeralSystem.WesternArabicNumerals, CalendarType.Gregorian)).toHaveLength(LongDateFormat.values().length + 1);
        expect(i18n.getAllShortDateFormats(NumeralSystem.WesternArabicNumerals, CalendarType.Gregorian)).toHaveLength(ShortDateFormat.values().length + 1);
        expect(i18n.getAllLongTimeFormats(NumeralSystem.WesternArabicNumerals)).toHaveLength(LongTimeFormat.values().length + 1);
        expect(i18n.getAllShortTimeFormats(NumeralSystem.WesternArabicNumerals)).toHaveLength(ShortTimeFormat.values().length + 1);
        expect(i18n.getAllFiscalYearFormats(NumeralSystem.WesternArabicNumerals, CalendarType.Gregorian)).toHaveLength(FiscalYearFormat.values().length + 1);

        expect(i18n.getCalendarDisplayShortYearFromUnixTime(timestamp)).toBe('2026');
        expect(i18n.getCalendarDisplayShortMonthFromUnixTime(timestamp)).toBe('Jul');
        expect(i18n.getCalendarDisplayDayOfMonthFromUnixTime(timestamp)).toBe('15');
        expect(i18n.parseDateTimeFromLongDateTime('2026 July 15 13:45:30')).toBeTruthy();
        expect(i18n.parseDateTimeFromShortDateTime('2026-7-15 13:45')).toBeTruthy();

        const formatters = [
            i18n.formatUnixTimeToLongDateTime,
            i18n.formatUnixTimeToShortDateTime,
            i18n.formatUnixTimeToLongDate,
            i18n.formatUnixTimeToShortDate,
            i18n.formatUnixTimeToLongMonthDay,
            i18n.formatUnixTimeToShortMonthDay,
            i18n.formatUnixTimeToLongTime,
            i18n.formatUnixTimeToShortTime,
            i18n.formatUnixTimeToGregorianLikeLongYear,
            i18n.formatUnixTimeToGregorianLikeShortYear,
            i18n.formatUnixTimeToGregorianLikeLongYearMonth,
            i18n.formatUnixTimeToGregorianLikeShortYearMonth,
            i18n.formatUnixTimeToGregorianLikeLongMonth,
            i18n.formatUnixTimeToGregorianLikeShortMonth,
            i18n.formatUnixTimeToGregorianDefaultDateTime
        ];

        for (const format of formatters) {
            expect(format(timestamp, 0, 0)).toEqual(expect.any(String));
        }

        expect(i18n.formatGregorianTextualYearMonthDayToLongDate('2026-07-15')).toBe('2026 July 15');
        expect(i18n.formatGregorianTextualMonthDayToGregorianLikeLongMonthDay('07-15')).toBe('July 15');
        expect(i18n.formatUnixTimeToGregorianLikeYearQuarter(timestamp)).toBe('Q3 2026');
        expect(i18n.formatYearQuarterToGregorianLikeYearQuarter(2026, 4)).toBe('Q4 2026');
        expect(i18n.formatYearQuarterToGregorianLikeYearQuarter(2026, 0)).toBe('');
        expect(i18n.formatUnixTimeToGregorianLikeFiscalYear(timestamp)).toBe('FY 2026');
        expect(i18n.formatGregorianYearToGregorianLikeFiscalYear(2026)).toBe('FY 2026');
        expect(i18n.formatFiscalYearStartToGregorianLikeLongMonth(FiscalYearStart.of(4, 1)!.value)).toContain('April');
        expect(i18n.formatFiscalYearStartToGregorianLikeLongMonth(999)).toContain('January');

        expect(i18n.isLongDateMonthAfterYear()).toBe(true);
        expect(i18n.isShortDateMonthAfterYear()).toBe(true);
        expect(i18n.isLongTime24HourFormat()).toBe(true);
        expect(i18n.isLongTimeMeridiemIndicatorFirst()).toBe(false);
        expect(i18n.isShortTime24HourFormat()).toBe(true);
        expect(i18n.isShortTimeMeridiemIndicatorFirst()).toBe(false);
        expect(i18n.isLongTimeHourTwoDigits()).toBe(true);
        expect(i18n.isLongTimeMinuteTwoDigits()).toBe(true);
        expect(i18n.isLongTimeSecondTwoDigits()).toBe(true);

        mockUserStore.currentUserLongDateFormat = LongDateFormat.MMDDYYYY.type;
        mockUserStore.currentUserShortDateFormat = ShortDateFormat.DDMMYYYY.type;
        mockUserStore.currentUserLongTimeFormat = LongTimeFormat.AHHMMSS.type;
        mockUserStore.currentUserShortTimeFormat = ShortTimeFormat.AHHMM.type;
        expect(i18n.isLongDateMonthAfterYear()).toBe(false);
        expect(i18n.isShortDateMonthAfterYear()).toBe(false);
        expect(i18n.isLongTime24HourFormat()).toBe(false);
        expect(i18n.isLongTimeMeridiemIndicatorFirst()).toBe(true);
        expect(i18n.isShortTime24HourFormat()).toBe(false);
        expect(i18n.isShortTimeMeridiemIndicatorFirst()).toBe(true);
        expect(i18n.isLongTimeHourTwoDigits()).toBe(true);

        mockUserStore.currentUserDateDisplayType = DateDisplayType.Buddhist.type;
        expect(i18n.formatUnixTimeToGregorianLikeLongYear(timestamp)).toEqual(expect.any(String));
        mockUserStore.currentUserDateDisplayType = DateDisplayType.Persian.type;
        expect(i18n.formatUnixTimeToGregorianLikeLongYear(timestamp)).toBe('2026');

        mockUserStore.currentUserFiscalYearStart = 0;
        expect(i18n.getAllFiscalYearFormats(NumeralSystem.Default, CalendarType.Persian)).toHaveLength(FiscalYearFormat.values().length + 1);
    });

    test('selects date ranges and describes timezone differences across all branches', () => {
        const i18n = useI18n();
        const oneDay = moment.utc('2026-07-15T00:00:00Z').unix();
        const oneDayEnd = moment.utc('2026-07-15T23:59:59Z').unix();
        const anotherDay = moment.utc('2026-07-17T23:59:59Z').unix();
        const anotherYear = moment.utc('2027-01-02T23:59:59Z').unix();

        expect(i18n.getAllDateRanges(DateRangeScene.Normal)).not.toEqual(expect.arrayContaining([
            expect.objectContaining({ type: DateRange.Custom.type })
        ]));
        expect(i18n.getAllDateRanges(DateRangeScene.Normal, true, true)).toEqual(expect.arrayContaining([
            expect.objectContaining({ type: DateRange.Custom.type }),
            expect.objectContaining({ isBillingCycle: true })
        ]));
        expect(i18n.getAllRecentMonthDateRanges(false, false)).toHaveLength(12);
        expect(i18n.getAllRecentMonthDateRanges(true, true)).toHaveLength(14);

        expect(i18n.formatDateRange(DateRange.All.type, 0, 0)).toBe('All');
        expect(i18n.formatDateRange(DateRange.ThisMonth.type, oneDay, oneDayEnd)).toBe('This month');
        expect(i18n.formatDateRange(DateRange.Custom.type, oneDay, oneDayEnd)).toBe('2026-7-15');
        expect(i18n.formatDateRange(DateRange.Custom.type, oneDay, anotherDay)).toBe('2026-7-15 ~ 7-17');
        expect(i18n.formatDateRange(DateRange.Custom.type, oneDay, anotherYear)).toBe('2026-7-15 ~ 2027-1-2');

        const yearStart = moment.utc('2025-01-01T00:00:00Z').unix();
        const yearEnd = moment.utc('2026-12-31T23:59:59Z').unix();
        expect(i18n.formatDateRange(DateRange.Custom.type, yearStart, yearEnd)).toBe('2025 ~ 2026');
        expect(i18n.formatDateRange(DateRange.Custom.type, yearStart, moment.utc('2025-12-31T23:59:59Z').unix())).toBe('2025');

        const monthStart = moment.utc('2026-05-01T00:00:00Z').unix();
        const monthEnd = moment.utc('2026-06-30T23:59:59Z').unix();
        expect(i18n.formatDateRange(DateRange.Custom.type, monthStart, monthEnd)).toBe('2026-5 ~ 2026-6');
        expect(i18n.formatDateRange(DateRange.Custom.type, monthStart, moment.utc('2026-05-31T23:59:59Z').unix())).toBe('2026-5');

        expect(i18n.getTimezoneDifferenceDisplayText(150)).toContain('ahead');
        expect(i18n.getTimezoneDifferenceDisplayText(120)).toContain('ahead');
        expect(i18n.getTimezoneDifferenceDisplayText(-150)).toContain('behind');
        expect(i18n.getTimezoneDifferenceDisplayText(-120)).toContain('behind');
        expect(i18n.getTimezoneDifferenceDisplayText(0)).toBe('Same time as default timezone');

        expect(i18n.getAllTimezones()).toEqual(expect.arrayContaining([
            expect.objectContaining({ name: 'Etc/GMT', utcOffset: '' })
        ]));
        expect(i18n.getAllTimezones(true)).toEqual(expect.arrayContaining([
            expect.objectContaining({ name: '', displayName: 'System Default' })
        ]));
        expect(i18n.getAllTimezoneTypesUsedForStatistics('Asia/Shanghai')).toHaveLength(2);
    });

    test('projects Chinese and Persian alternate calendars and handles absent secondary calendars', () => {
        const i18n = useI18n();
        const yearMonth = { year: 2026, month1base: 7 };
        const day = { year: 2026, month: 7, day: 15 };

        mockUserStore.currentUserCalendarDisplayType = CalendarDisplayType.Gregorian.type;
        expect(i18n.getCalendarAlternateDates(yearMonth)).toBeUndefined();
        expect(i18n.getCalendarAlternateDate(day)).toBeUndefined();

        mockUserStore.currentUserCalendarDisplayType = CalendarDisplayType.GregorianWithChinese.type;
        expect(i18n.getCalendarAlternateDates(yearMonth)).toHaveLength(31);
        expect(i18n.getCalendarAlternateDate(day)).toEqual(expect.objectContaining({ year: 2026, month: 7, day: 15 }));
        expect(i18n.getCalendarAlternateDates({ year: 0, month1base: 0 })).toBeUndefined();
        expect(i18n.getCalendarAlternateDate({ year: 0, month: 0, day: 0 })).toBeUndefined();

        mockUserStore.currentUserCalendarDisplayType = CalendarDisplayType.GregorianWithPersian.type;
        const persianDates = i18n.getCalendarAlternateDates(yearMonth);
        expect(persianDates).toHaveLength(31);
        expect(persianDates![0]!.displayDate).toEqual(expect.any(String));
        expect(i18n.getCalendarAlternateDate(day)).toEqual(expect.objectContaining({ year: 2026, month: 7, day: 15 }));

        const persianDisplayType = CalendarDisplayType.GregorianWithPersian as unknown as { secondaryCalendarType?: CalendarType };
        const originalSecondaryCalendar = persianDisplayType.secondaryCalendarType;
        persianDisplayType.secondaryCalendarType = CalendarType.Buddhist;
        try {
            expect(i18n.getCalendarAlternateDates(yearMonth)).toBeUndefined();
            expect(i18n.getCalendarAlternateDate(day)).toBeUndefined();
        } finally {
            persianDisplayType.secondaryCalendarType = originalSecondaryCalendar;
        }
    });
});

describe('number, currency, import, and account projections', () => {
    test('formats localized numbers, amounts, currency variants, and adaptive rates', () => {
        const i18n = useI18n();

        expect(i18n.parseAmountFromLocalizedNumerals('1,234.56')).toBe(123456);
        expect(i18n.parseAmountFromWesternArabicNumerals('12.34')).toBe(1234);
        expect(i18n.formatAmountToLocalizedNumerals(123456, 'CNY')).toBe('1,234.56');
        expect(i18n.formatAmountToWesternArabicNumerals(123456, 'CNY')).toBe('1,234.56');
        expect(i18n.formatAmountToLocalizedNumeralsWithoutDigitGrouping(123456, 'CNY')).toBe('1234.56');
        expect(i18n.formatAmountToWesternArabicNumeralsWithoutDigitGrouping(123456, 'CNY')).toBe('1234.56');

        expect(i18n.formatAmountToLocalizedNumeralsWithCurrency(123456, false)).toBe('1,234.56');
        expect(i18n.formatAmountToLocalizedNumeralsWithCurrency(123456, 'CNY')).toContain('¥');
        expect(i18n.formatAmountToLocalizedNumeralsWithCurrency(100, 'USD')).toContain('$');
        expect(i18n.formatAmountToLocalizedNumeralsWithCurrency(-100, 'USD')).toContain('$');
        expect(i18n.formatAmountToLocalizedNumeralsWithCurrency({ value: 123456, suffix: '+' }, 'CNY')).toMatch(/\+$/);
        expect(i18n.formatAmountToLocalizedNumeralsWithCurrency(DISPLAY_HIDDEN_AMOUNT, 'CNY')).toContain('*');
        expect(i18n.formatAmountToLocalizedNumeralsWithCurrency(DISPLAY_HIDDEN_AMOUNT, false)).toContain('*');
        expect(i18n.formatAmountToLocalizedNumeralsWithCurrency({ invalid: true } as never, 'CNY')).toBe('');
        expect(i18n.formatAmountToWesternArabicNumeralsWithCurrency(123456)).toContain('¥');

        expect(i18n.formatNumberToLocalizedNumerals(12.3456, 2)).toBe('12.34');
        expect(i18n.formatNumberToWesternArabicNumerals(12.3456, 2)).toBe('12.34');
        expect(i18n.formatPercentToLocalizedNumerals(0.125, 1, '<0.1%')).toBe('0.1%');
        expect(i18n.formatPercentToWesternArabicNumerals(0.125, 1, '<0.1%')).toBe('0.1%');
        expect(i18n.formatExchangeRateAmountToWesternArabicNumerals(7.1234)).toEqual(expect.any(String));
        expect(i18n.appendDigitGroupingSymbolAndDecimalSeparator('1234.5')).toBe('1,234.5');
        expect(i18n.getAdaptiveAmountRate(100, 200)).toEqual(expect.any(String));
        expect(i18n.getAmountPrependAndAppendText('CNY', false)).toEqual(expect.objectContaining({ prependText: expect.any(String) }));
        expect(i18n.getAmountPrependAndAppendText('XXX', true)).toEqual(expect.objectContaining({ prependText: expect.any(String) }));
        expect(i18n.getCurrencyName('')).toBe('');
        expect(i18n.getCurrencyName('CNY')).toBe('Chinese Yuan');
    });

    test('localizes preset categories, exchange-rate ordering, and import-file metadata', () => {
        const i18n = useI18n();
        const allCategories = i18n.getAllTransactionDefaultCategories(0, 'en');

        expect(allCategories[CategoryType.Income]!.length).toBeGreaterThan(0);
        expect(allCategories[CategoryType.Expense]!.length).toBeGreaterThan(0);
        expect(allCategories[CategoryType.Transfer]!.length).toBeGreaterThan(0);
        expect(allCategories[CategoryType.Investment]!.length).toBeGreaterThan(0);
        expect(i18n.getAllTransactionDefaultCategories(CategoryType.Expense, 'en')).toHaveProperty(String(CategoryType.Expense));

        const exchangeRates = {
            dataSource: 'test',
            referenceUrl: 'https://example.test',
            updateTime: 1,
            baseCurrency: 'CNY',
            exchangeRates: [
                { currency: 'USD', rate: '7.20' },
                { currency: 'EUR', rate: '7.80' },
                { currency: 'JPY', rate: '0.05' }
            ]
        };

        expect(i18n.getAllDisplayExchangeRates()).toStrictEqual([]);
        expect(i18n.getAllDisplayExchangeRates({ ...exchangeRates, exchangeRates: undefined as never })).toStrictEqual([]);
        mockSettingsStore.appSettings.currencySortByInExchangeRatesPage = CurrencySortingType.Name.type;
        expect(i18n.getAllDisplayExchangeRates(exchangeRates)[0]!.currencyDisplayName).toBe('Euro');
        mockSettingsStore.appSettings.currencySortByInExchangeRatesPage = CurrencySortingType.CurrencyCode.type;
        expect(i18n.getAllDisplayExchangeRates(exchangeRates).map(rate => rate.currencyCode)).toStrictEqual(['EUR', 'JPY', 'USD']);
        mockSettingsStore.appSettings.currencySortByInExchangeRatesPage = CurrencySortingType.ExchangeRate.type;
        expect(i18n.getAllDisplayExchangeRates(exchangeRates).map(rate => rate.rate)).toStrictEqual(['0.05', '7.20', '7.80']);
        const equalAndAscendingRates = {
            ...exchangeRates,
            exchangeRates: [
                { currency: 'JPY', rate: '0.05' },
                { currency: 'USD', rate: '7.20' },
                { currency: 'EUR', rate: '7.80' },
                { currency: 'GBP', rate: '7.80' }
            ]
        };
        expect(i18n.getAllDisplayExchangeRates(equalAndAscendingRates).map(rate => rate.rate)).toStrictEqual(['0.05', '7.20', '7.80', '7.80']);

        expect(i18n.getAllSupportedImportFileCagtegoryAndTypes()).toEqual(expect.arrayContaining([
            expect.objectContaining({ displayCategoryName: expect.any(String), fileTypes: expect.any(Array) })
        ]));
        mockLocale.value = 'zh-Hans';
        expect(i18n.getAllSupportedImportFileCagtegoryAndTypes().flatMap(category => category.fileTypes).some(file => file.document?.language === 'zh_Hans')).toBe(true);
        mockLocale.value = 'zh-Hant';
        expect(i18n.getAllSupportedImportFileCagtegoryAndTypes().flatMap(category => category.fileTypes).some(file => file.document?.language === 'zh_Hans')).toBe(true);
    });

    test('groups account balances, converts foreign currencies, and marks incomplete totals', () => {
        const i18n = useI18n();
        const accounts = [
            account({ id: 'cash', category: AccountCategory.Cash.type, currency: 'CNY', balanceCents: 10_000 }),
            account({ id: 'foreign', category: AccountCategory.Cash.type, currency: 'USD', balanceCents: 2_000 }),
            account({ id: 'credit', category: AccountCategory.CreditCard.type, currency: 'CNY', balanceCents: 3_000 }),
            account({ id: 'foreign-credit', category: AccountCategory.CreditCard.type, currency: 'USD', balanceCents: 1_000 })
        ];

        const visible = i18n.getCategorizedAccountsWithDisplayBalance(accounts, true);
        expect(visible.flatMap(category => category.accounts).map(item => item.displayBalance)).toEqual(expect.arrayContaining([
            expect.stringContaining('¥'),
            expect.stringContaining('$')
        ]));
        expect(visible.find(category => category.category === AccountCategory.Cash.type)!.displayBalance).toContain('¥');
        expect(visible.find(category => category.category === AccountCategory.CreditCard.type)!.displayBalance).toContain('-');

        mockGetExchangedAmount.mockReturnValueOnce(undefined);
        const incomplete = i18n.getCategorizedAccountsWithDisplayBalance(accounts, true);
        expect(incomplete.find(category => category.category === AccountCategory.Cash.type)!.displayBalance).toContain(INCOMPLETE_AMOUNT_SUFFIX);

        const hidden = i18n.getCategorizedAccountsWithDisplayBalance(accounts, false);
        expect(hidden.every(category => category.displayBalance === DISPLAY_HIDDEN_AMOUNT)).toBe(true);
        expect(hidden.flatMap(category => category.accounts).every(item => item.displayBalance === DISPLAY_HIDDEN_AMOUNT)).toBe(true);
    });
});

describe('locale mutations and OAuth presentation', () => {
    test('chooses built-in, OIDC, and generic OAuth provider names and login labels', () => {
        const i18n = useI18n();

        expect(i18n.getLocalizedOAuth2ProviderName('oidc', { en: 'Company SSO' })).toBe('Company SSO');
        expect(i18n.getLocalizedOAuth2ProviderName('oidc', {})).toBe('Connect ID');
        expect(i18n.getLocalizedOAuth2ProviderName('github', {})).toBe('GitHub');
        expect(i18n.getLocalizedOAuth2ProviderName('unknown', {})).toBe('OAuth 2.0');
        expect(i18n.getLocalizedOAuth2LoginText('oidc', { en: 'Company SSO' })).toBe('Log in with Company SSO');
        expect(i18n.getLocalizedOAuth2LoginText('oidc', {})).toBe('Log in with Connect ID');
        expect(i18n.getLocalizedOAuth2LoginText('github', {})).toBe('Log in with GitHub');
        expect(i18n.getLocalizedOAuth2LoginText('unknown', {})).toBe('Log in with OAuth 2.0');
    });

    test('applies aliases, persists locale state, and avoids reapplying the current language', () => {
        const i18n = useI18n();

        expect(i18n.setLanguage('zh_CN', true)).toStrictEqual({ currency: 'USD', firstDayOfWeek: WeekDay.Sunday.type });
        expect(mockLocale.value).toBe('zh-Hans');
        expect(mockSetSessionLanguage).toHaveBeenCalledWith('zh-Hans');
        expect(mockSetLocale).toHaveBeenCalledWith('zh-Hans');
        expect(htmlAttributes.get('lang')).toBe('zh-Hans');
        expect(htmlAttributes.has('dir')).toBe(false);

        expect(i18n.setLanguage('zh-Hans')).toBeNull();
        expect(mockLoggerInfo).toHaveBeenCalledWith('Current locale is already zh-Hans');

        expect(i18n.setLanguage('not-a-language', true)).toStrictEqual({ currency: 'USD', firstDayOfWeek: WeekDay.Sunday.type });
        expect(mockLocale.value).toBe('en');
        expect(mockLoggerWarn).toHaveBeenCalled();
    });

    test('derives browser language through exact, alias, script, macro, and default fallbacks', () => {
        let i18n = useI18n();
        Object.defineProperty(globalThis, 'window', { configurable: true, value: undefined, writable: true });
        expect(i18n.setLanguage(null, true)).toEqual(expect.objectContaining({ currency: 'USD' }));
        expect(mockLocale.value).toBe('en');

        Object.defineProperty(globalThis, 'window', { configurable: true, value: {}, writable: true });
        mockLocale.value = 'de';
        expect(i18n.setLanguage(null, true)).toEqual(expect.objectContaining({ currency: 'USD' }));
        expect(mockLocale.value).toBe('en');

        const scenarios = [
            ['en', 'en'],
            ['zh_CN', 'zh-Hans'],
            ['zh-Hans-CN', 'zh-Hans'],
            ['zh-CN-extra', 'zh-Hans'],
            ['zh-Unknown', 'zh-Hans'],
            ['pt-BR-extra', 'pt-BR'],
            ['pt-XX', 'pt-BR'],
            ['en-US', 'en'],
            ['unknown-ZZ', 'en'],
            ['', 'en']
        ] as const;

        for (const [browserLanguage, expected] of scenarios) {
            installBrowser(browserLanguage);
            mockLocale.value = 'de';
            i18n = useI18n();
            expect(i18n.setLanguage(null, true)).toEqual(expect.objectContaining({ currency: 'USD' }));
            expect(mockLocale.value).toBe(expected);
        }
    });

    test('updates dynamic and static text direction without performing unsafe navigation', () => {
        const i18n = useI18n();
        const originalDirection = ALL_LANGUAGES['en']!.textDirection;

        htmlAttributes.set('dir', 'rtl');
        expect(i18n.setLanguage('en', true)).not.toBeNull();
        expect(htmlAttributes.has('dir')).toBe(false);

        htmlAttributes.set('data-dir-mode', 'static');
        installBrowser('en', '?rtl');
        expect(i18n.setLanguage('en', true)).not.toBeNull();
        expect(mockReplaceLocation).toHaveBeenCalledTimes(1);

        mockReplaceLocation.mockClear();
        (ALL_LANGUAGES['en'] as { textDirection: 'ltr' | 'rtl' }).textDirection = 'rtl';
        try {
            htmlAttributes.delete('data-dir-mode');
            expect(i18n.setLanguage('en', true)).not.toBeNull();
            expect(htmlAttributes.get('dir')).toBe('rtl');

            htmlAttributes.set('data-dir-mode', 'static');
            installBrowser('en', '');
            expect(i18n.setLanguage('en', true)).not.toBeNull();
            expect(mockReplaceLocation).toHaveBeenCalledTimes(1);

            mockReplaceLocation.mockClear();
            installBrowser('en', '?rtl');
            expect(i18n.setLanguage('en', true)).not.toBeNull();
            expect(mockReplaceLocation).not.toHaveBeenCalled();
        } finally {
            (ALL_LANGUAGES['en'] as { textDirection: 'ltr' | 'rtl' }).textDirection = originalDirection;
        }

        mockReplaceLocation.mockClear();
        expect(i18n.setLanguage('missing-language', true)).not.toBeNull();
        expect(mockReplaceLocation).not.toHaveBeenCalled();
    });

    test('uses localized meridiem callbacks and falls back when the locale weekday default is invalid', () => {
        const i18n = useI18n();
        expect(i18n.setLanguage('en', true)).not.toBeNull();

        mockUserStore.currentUserLongTimeFormat = LongTimeFormat.AHHMMSS.type;
        expect(i18n.formatUnixTimeToLongTime(moment.utc('2026-07-15T01:00:00Z').unix())).toContain('AM');
        expect(i18n.formatUnixTimeToLongTime(moment.utc('2026-07-15T13:00:00Z').unix())).toContain('PM');

        const originalImplementation = mockT.getMockImplementation()!;
        mockT.mockImplementation((key: string, parameters?: Record<string, unknown>) => {
            if (key === 'default.firstDayOfWeek') {
                return 'NotAWeekday';
            }
            return originalImplementation(key, parameters);
        });
        try {
            expect(i18n.setLanguage('en', true)).toEqual({
                currency: 'USD',
                firstDayOfWeek: WeekDay.DefaultFirstDay.type
            });
        } finally {
            mockT.mockImplementation(originalImplementation);
        }
    });

    test('initializes locale from user, session, or browser and applies explicit or browser timezone', () => {
        installBrowser('en-US');
        mockSessionLanguage = 'de';
        let i18n = useI18n();

        expect(i18n.initLocale('zh_CN', 'Asia/Shanghai')).toEqual(expect.objectContaining({ currency: 'USD' }));
        expect(mockLocale.value).toBe('zh-Hans');
        expect(mockLoggerInfo).toHaveBeenCalledWith('Current timezone is Asia/Shanghai');

        mockLocale.value = 'en';
        i18n = useI18n();
        expect(i18n.initLocale(undefined, '')).not.toBeNull();
        expect(mockLocale.value).toBe('de');
        expect(mockLoggerInfo).toHaveBeenCalledWith(expect.stringContaining('No timezone is set'));

        mockSessionLanguage = 'missing';
        mockLocale.value = 'de';
        i18n = useI18n();
        expect(i18n.initLocale(undefined, 'UTC')).not.toBeNull();
        expect(mockLocale.value).toBe('en');

        expect(() => i18n.setTimeZone('Asia/Tokyo')).not.toThrow();
        expect(() => i18n.setTimeZone('')).not.toThrow();
    });
});
