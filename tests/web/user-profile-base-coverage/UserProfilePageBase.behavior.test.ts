import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockUpdateLocalizedDefaultSettings = jest.fn();
const mockResetTransactionOverview = jest.fn();
const mockSetExpenseAndIncomeAmountColor = jest.fn();
const mockGetCategorizedAccounts = jest.fn((accounts: any[]) => [{ category: 'visible', accounts }]);
const mockSetLanguage = jest.fn((language: string) => ({ language, localeDefaults: true }));

let mockDefaultFirstDayName = 'Monday';
let mockLanguageText = '语言';

const mockAllPlainAccounts = [{ id: 'all-1' }, { id: 'hidden-2', hidden: true }];
const mockAllVisibleAccounts = [{ id: 'all-1' }];

const mockOptionCalls = {
    getAllWeekDays: jest.fn(() => [{ type: 1, displayName: 'Monday' }]),
    getAllCalendarDisplayTypes: jest.fn(() => [{ type: 1, displayName: 'Gregorian' }]),
    getAllDateDisplayTypes: jest.fn(() => [{ type: 1, displayName: 'Date' }]),
    getAllLongDateFormats: jest.fn((_numeralSystem?: unknown, _calendarType?: unknown) => [{ type: 1, displayName: 'Long Date' }]),
    getAllShortDateFormats: jest.fn(() => [{ type: 1, displayName: 'Short Date' }]),
    getAllLongTimeFormats: jest.fn(() => [{ type: 1, displayName: 'Long Time' }]),
    getAllShortTimeFormats: jest.fn(() => [{ type: 1, displayName: 'Short Time' }]),
    getAllFiscalYearFormats: jest.fn(() => [{ type: 1, displayName: 'Fiscal' }]),
    getAllCurrencyDisplayTypes: jest.fn((_numeralSystem?: unknown, _decimalSeparator?: unknown) => [{ type: 1, displayName: 'Currency' }]),
    getAllNumeralSystemTypes: jest.fn(() => [{ type: 1, displayName: 'Numeral' }]),
    getAllDecimalSeparators: jest.fn(() => [{ type: 1, displayName: 'Decimal' }]),
    getAllDigitGroupingSymbols: jest.fn(() => [{ type: 1, displayName: 'Grouping Symbol' }]),
    getAllDigitGroupingTypes: jest.fn((_numeralSystem?: unknown, _digitGroupingSymbol?: unknown) => [
        { type: 1, displayName: 'Enabled', enabled: true },
        { type: 2, displayName: 'Disabled', enabled: false }
    ]),
    getAllCoordinateDisplayTypes: jest.fn(() => [{ type: 1, displayName: 'Coordinate' }]),
    getAllExpenseAmountColors: jest.fn(() => [{ type: 1, displayName: 'Expense' }]),
    getAllIncomeAmountColors: jest.fn(() => [{ type: 1, displayName: 'Income' }]),
    getAllTransactionEditScopeTypes: jest.fn(() => [{ type: 1, displayName: 'Scope' }])
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key === 'Language' ? mockLanguageText : `tt:${key}`,
        getDefaultCurrency: () => 'CNY',
        getDefaultFirstDayOfWeek: () => mockDefaultFirstDayName,
        ...mockOptionCalls,
        getLocaleDefaultDateDisplayType: () => ({ type: 9, calendarType: 'locale-calendar' }),
        getLocaleDefaultNumeralSystemType: () => ({ type: 9, name: 'locale-numeral' }),
        getLocaleDefaultDecimalSeparator: () => ({ symbol: ',' }),
        getLocaleDefaultDigitGroupingSymbol: () => ({ symbol: ' ' }),
        setLanguage: mockSetLanguage
    })
}));
jest.mock('@/stores/setting.ts', () => ({
    useSettingsStore: () => ({ updateLocalizedDefaultSettings: mockUpdateLocalizedDefaultSettings })
}));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({
        allPlainAccounts: mockAllPlainAccounts,
        allVisiblePlainAccounts: mockAllVisibleAccounts
    })
}));
jest.mock('@/stores/overview.ts', () => ({
    useOverviewStore: () => ({ resetTransactionOverview: mockResetTransactionOverview })
}));
jest.mock('@/core/calendar.ts', () => ({
    DateDisplayType: {
        valueOf: (type: number) => type === 1 ? { type: 1, calendarType: 'gregorian' } : undefined
    }
}));
jest.mock('@/core/datetime.ts', () => ({
    WeekDay: {
        parse: (name: string) => name === 'Monday' ? { type: 1 } : undefined,
        DefaultFirstDay: { type: 0 }
    }
}));
jest.mock('@/core/numeral.ts', () => ({
    NumeralSystem: {
        valueOf: (type: number) => type === 1 ? { type: 1, name: 'western' } : undefined
    },
    DecimalSeparator: {
        valueOf: (type: number) => type === 1 ? { symbol: '.' } : undefined
    },
    DigitGroupingSymbol: {
        valueOf: (type: number) => type === 1 ? { symbol: ',' } : undefined
    }
}));
jest.mock('@/models/user.ts', () => {
    const profileFields = [
        'username', 'email', 'nickname', 'defaultAccountId', 'transactionEditScope', 'language',
        'defaultCurrency', 'firstDayOfWeek', 'fiscalYearStart', 'calendarDisplayType', 'dateDisplayType',
        'longDateFormat', 'shortDateFormat', 'longTimeFormat', 'shortTimeFormat', 'fiscalYearFormat',
        'currencyDisplayType', 'numeralSystem', 'decimalSeparator', 'digitGroupingSymbol', 'digitGrouping',
        'coordinateDisplayType', 'expenseAmountColor', 'incomeAmountColor', 'cashAccountId',
        'cashTransferCategoryId', 'importLearningEnabled', 'investmentPlatformKeywords',
        'investmentProductKeywords', 'investmentExcludeKeywords'
    ];
    class User {
        public username = '';
        public password = '';
        public confirmPassword = '';
        public email = '';
        public nickname = '';
        public defaultAccountId = '';
        public transactionEditScope = 0;
        public language: string;
        public defaultCurrency: string;
        public firstDayOfWeek: number;
        public fiscalYearStart = 1;
        public calendarDisplayType = 1;
        public dateDisplayType = 1;
        public longDateFormat = 1;
        public shortDateFormat = 1;
        public longTimeFormat = 1;
        public shortTimeFormat = 1;
        public fiscalYearFormat = 1;
        public currencyDisplayType = 1;
        public numeralSystem = 1;
        public decimalSeparator = 1;
        public digitGroupingSymbol = 1;
        public digitGrouping = 1;
        public coordinateDisplayType = 1;
        public expenseAmountColor = 1;
        public incomeAmountColor = 2;
        public cashAccountId = '';
        public cashTransferCategoryId = '';
        public importLearningEnabled = true;
        public investmentPlatformKeywords: string[] = [];
        public investmentProductKeywords: string[] = [];
        public investmentExcludeKeywords: string[] = [];

        public constructor(language: string, defaultCurrency: string, firstDayOfWeek: number) {
            this.language = language;
            this.defaultCurrency = defaultCurrency;
            this.firstDayOfWeek = firstDayOfWeek;
        }

        public fillFrom(source: Record<string, any>): void {
            for (const field of profileFields) {
                const value = source[field];
                (this as any)[field] = Array.isArray(value) ? [...value] : value;
            }
        }

        public static createNewUser(language: string, defaultCurrency: string, firstDayOfWeek: number): User {
            return new User(language, defaultCurrency, firstDayOfWeek);
        }
    }
    return { User };
});
jest.mock('@/lib/ui/common.ts', () => ({
    setExpenseAndIncomeAmountColor: mockSetExpenseAndIncomeAmountColor
}));
jest.mock('@/lib/account.ts', () => ({ getCategorizedAccounts: mockGetCategorizedAccounts }));

import { useUserProfilePageBase } from '@/views/base/users/UserProfilePageBase.ts';

function profile(overrides: Record<string, unknown> = {}): any {
    return {
        id: 7,
        username: 'alice',
        email: 'alice@example.test',
        nickname: 'Alice',
        avatar: 'avatar-id-only',
        avatarProvider: 'local',
        defaultAccountId: 'account-1',
        transactionEditScope: 2,
        language: 'zh-Hans',
        defaultCurrency: 'CNY',
        firstDayOfWeek: 1,
        fiscalYearStart: 4,
        calendarDisplayType: 1,
        dateDisplayType: 1,
        longDateFormat: 2,
        shortDateFormat: 3,
        longTimeFormat: 4,
        shortTimeFormat: 5,
        fiscalYearFormat: 6,
        currencyDisplayType: 7,
        numeralSystem: 1,
        decimalSeparator: 1,
        digitGroupingSymbol: 1,
        digitGrouping: 1,
        coordinateDisplayType: 8,
        expenseAmountColor: 9,
        incomeAmountColor: 10,
        cashAccountId: 'cash-1',
        cashTransferCategoryId: 'transfer-1',
        importLearningEnabled: true,
        investmentPlatformKeywords: ['broker'],
        investmentProductKeywords: ['fund'],
        investmentExcludeKeywords: ['fee'],
        emailVerified: true,
        ...overrides
    };
}

function setCleanProfile(bindings: ReturnType<typeof useUserProfilePageBase>, overrides: Record<string, unknown> = {}): any {
    const value = profile(overrides);
    bindings.setCurrentUserProfile(value);
    return value;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockDefaultFirstDayName = 'Monday';
    mockLanguageText = '语言';
});

describe('UserProfilePageBase production-loaded behavior', () => {
    test('initialization and option computeds cover parsed defaults, account visibility, locale values, and fallbacks', () => {
        const bindings = useUserProfilePageBase();
        expect(bindings.newProfile.value).toMatchObject({ defaultCurrency: 'CNY', firstDayOfWeek: 1 });
        expect(bindings.oldProfile.value).toMatchObject({ defaultCurrency: 'CNY', firstDayOfWeek: 1 });
        expect(bindings.loading.value).toBe(false);
        expect(bindings.resending.value).toBe(false);
        expect(bindings.saving.value).toBe(false);
        expect(bindings.emailVerified.value).toBe(false);

        expect(bindings.allAccounts.value).toBe(mockAllPlainAccounts);
        expect(bindings.allVisibleAccounts.value).toBe(mockAllVisibleAccounts);
        expect(bindings.allVisibleCategorizedAccounts.value).toEqual([{ category: 'visible', accounts: mockAllVisibleAccounts }]);
        expect(mockGetCategorizedAccounts).toHaveBeenCalledWith(mockAllVisibleAccounts);

        expect(bindings.allWeekDays.value).toHaveLength(1);
        expect(bindings.allCalendarDisplayTypes.value).toHaveLength(1);
        expect(bindings.allDateDisplayTypes.value).toHaveLength(1);
        expect(bindings.allLongDateFormats.value).toHaveLength(1);
        expect(bindings.allShortDateFormats.value).toHaveLength(1);
        expect(bindings.allLongTimeFormats.value).toHaveLength(1);
        expect(bindings.allShortTimeFormats.value).toHaveLength(1);
        expect(bindings.allFiscalYearFormats.value).toHaveLength(1);
        expect(bindings.allCurrencyDisplayTypes.value).toHaveLength(1);
        expect(bindings.allNumeralSystemTypes.value).toHaveLength(1);
        expect(bindings.allDecimalSeparators.value).toHaveLength(1);
        expect(bindings.allDigitGroupingSymbols.value).toHaveLength(1);
        expect(bindings.allDigitGroupingTypes.value).toHaveLength(2);
        expect(bindings.allCoordinateDisplayTypes.value).toHaveLength(1);
        expect(bindings.allExpenseAmountColorTypes.value).toHaveLength(1);
        expect(bindings.allIncomeAmountColorTypes.value).toHaveLength(1);
        expect(bindings.allTransactionEditScopeTypes.value).toHaveLength(1);
        expect(mockOptionCalls.getAllLongDateFormats).toHaveBeenCalledWith(
            { type: 1, name: 'western' },
            'gregorian'
        );
        expect(mockOptionCalls.getAllCurrencyDisplayTypes).toHaveBeenCalledWith(
            { type: 1, name: 'western' },
            '.'
        );
        expect(mockOptionCalls.getAllDigitGroupingTypes).toHaveBeenCalledWith(
            { type: 1, name: 'western' },
            ','
        );

        bindings.newProfile.value.dateDisplayType = 999;
        bindings.newProfile.value.numeralSystem = 999;
        bindings.newProfile.value.decimalSeparator = 999;
        bindings.newProfile.value.digitGroupingSymbol = 999;
        expect(bindings.allLongDateFormats.value).toEqual(expect.any(Array));
        expect(mockOptionCalls.getAllLongDateFormats).toHaveBeenLastCalledWith(
            { type: 9, name: 'locale-numeral' },
            'locale-calendar'
        );
        expect(bindings.allCurrencyDisplayTypes.value).toEqual(expect.any(Array));
        expect(mockOptionCalls.getAllCurrencyDisplayTypes).toHaveBeenLastCalledWith(
            { type: 9, name: 'locale-numeral' },
            ','
        );
        expect(bindings.allDigitGroupingTypes.value).toEqual(expect.any(Array));
        expect(mockOptionCalls.getAllDigitGroupingTypes).toHaveBeenLastCalledWith(
            { type: 9, name: 'locale-numeral' },
            ' '
        );

        mockDefaultFirstDayName = 'Unknown';
        expect(useUserProfilePageBase().newProfile.value.firstDayOfWeek).toBe(0);
    });

    test('language title and digit-grouping support cover translated, default, enabled, disabled, and absent options', () => {
        const translated = useUserProfilePageBase();
        expect(translated.languageTitle.value).toBe('语言 / Language');
        translated.newProfile.value.digitGrouping = 1;
        expect(translated.supportDigitGroupingSymbol.value).toBe(true);
        translated.newProfile.value.digitGrouping = 2;
        expect(translated.supportDigitGroupingSymbol.value).toBe(false);
        translated.newProfile.value.digitGrouping = 999;
        expect(translated.supportDigitGroupingSymbol.value).toBe(false);

        mockLanguageText = 'Language';
        expect(useUserProfilePageBase().languageTitle.value).toBe('Language');
    });

    test.each([
        ['email', 'bob@example.test'],
        ['nickname', 'Bob'],
        ['defaultAccountId', 'account-2'],
        ['transactionEditScope', 9],
        ['language', 'en'],
        ['defaultCurrency', 'USD'],
        ['fiscalYearStart', 8],
        ['firstDayOfWeek', 3],
        ['calendarDisplayType', 2],
        ['dateDisplayType', 2],
        ['longDateFormat', 12],
        ['shortDateFormat', 13],
        ['longTimeFormat', 14],
        ['shortTimeFormat', 15],
        ['fiscalYearFormat', 16],
        ['currencyDisplayType', 17],
        ['numeralSystem', 2],
        ['decimalSeparator', 2],
        ['digitGroupingSymbol', 2],
        ['digitGrouping', 2],
        ['coordinateDisplayType', 18],
        ['expenseAmountColor', 19],
        ['incomeAmountColor', 20],
        ['cashAccountId', 'cash-2'],
        ['cashTransferCategoryId', 'transfer-2'],
        ['importLearningEnabled', false]
    ] as const)('detects a changed %s field', (field, changedValue) => {
        const bindings = useUserProfilePageBase();
        setCleanProfile(bindings);
        (bindings.newProfile.value as any)[field] = changedValue;
        expect(bindings.inputIsNotChangedProblemMessage.value).toBeNull();
        expect(bindings.inputIsNotChanged.value).toBe(false);
    });

    test.each([
        ['investmentPlatformKeywords', ['another-platform']],
        ['investmentProductKeywords', ['another-product']],
        ['investmentExcludeKeywords', ['another-exclusion']]
    ] as const)('detects changed normalized %s', (field, changedValue) => {
        const bindings = useUserProfilePageBase();
        setCleanProfile(bindings);
        (bindings.newProfile.value as any)[field] = changedValue;
        expect(bindings.inputIsNotChangedProblemMessage.value).toBeNull();
    });

    test('unchanged detection trims keyword arrays and covers empty and password-only edge cases', () => {
        const initial = useUserProfilePageBase();
        expect(initial.inputIsNotChangedProblemMessage.value).toBe('Nothing has been modified');
        expect(initial.inputIsNotChanged.value).toBe(true);

        const unchanged = useUserProfilePageBase();
        setCleanProfile(unchanged);
        unchanged.newProfile.value.investmentPlatformKeywords = [' broker ', '', null as never];
        unchanged.newProfile.value.investmentProductKeywords = [' fund '];
        unchanged.newProfile.value.investmentExcludeKeywords = [' fee ', '   '];
        expect(unchanged.inputIsNotChangedProblemMessage.value).toBe('Nothing has been modified');

        unchanged.newProfile.value.investmentPlatformKeywords = undefined as never;
        expect(unchanged.inputIsNotChangedProblemMessage.value).toBeNull();

        const missingPassword = useUserProfilePageBase();
        setCleanProfile(missingPassword);
        missingPassword.newProfile.value.confirmPassword = 'confirmation';
        expect(missingPassword.inputIsNotChangedProblemMessage.value).toBe('Password cannot be blank');

        const missingConfirmation = useUserProfilePageBase();
        setCleanProfile(missingConfirmation);
        missingConfirmation.newProfile.value.password = 'new-password';
        expect(missingConfirmation.inputIsNotChangedProblemMessage.value).toBe('Password confirmation cannot be blank');

        missingConfirmation.newProfile.value.confirmPassword = 'new-password';
        expect(missingConfirmation.inputIsNotChangedProblemMessage.value).toBeNull();
    });

    test('invalid-state messages cover password mismatch and every required profile field', () => {
        const bindings = useUserProfilePageBase();
        setCleanProfile(bindings);

        bindings.newProfile.value.password = 'one';
        bindings.newProfile.value.confirmPassword = 'two';
        expect(bindings.inputInvalidProblemMessage.value).toBe('Password and password confirmation do not match');
        expect(bindings.inputIsInvalid.value).toBe(true);

        bindings.newProfile.value.password = '';
        bindings.newProfile.value.confirmPassword = '';
        bindings.newProfile.value.email = '';
        expect(bindings.inputInvalidProblemMessage.value).toBe('Email address cannot be blank');

        bindings.newProfile.value.email = 'alice@example.test';
        bindings.newProfile.value.nickname = '';
        expect(bindings.inputInvalidProblemMessage.value).toBe('Nickname cannot be blank');

        bindings.newProfile.value.nickname = 'Alice';
        bindings.newProfile.value.defaultCurrency = '';
        expect(bindings.inputInvalidProblemMessage.value).toBe('Default currency cannot be blank');
        expect(bindings.langAndRegionInputInvalidProblemMessage.value).toBe('Default currency cannot be blank');
        expect(bindings.langAndRegionInputIsInvalid.value).toBe(true);

        bindings.newProfile.value.defaultCurrency = 'CNY';
        expect(bindings.inputInvalidProblemMessage.value).toBeNull();
        expect(bindings.inputIsInvalid.value).toBe(false);
        expect(bindings.langAndRegionInputInvalidProblemMessage.value).toBeNull();
        expect(bindings.langAndRegionInputIsInvalid.value).toBe(false);
        expect(bindings.extendInputInvalidProblemMessage.value).toBeNull();
        expect(bindings.extendInputIsInvalid.value).toBe(false);
    });

    test('set and reset synchronize allowed profile fields while keeping arrays isolated and secrets out of the base API', () => {
        const bindings = useUserProfilePageBase();
        const source = profile({
            password: 'must-not-be-copied',
            confirmPassword: 'must-not-be-copied',
            accessToken: 'must-not-be-exposed'
        });
        bindings.setCurrentUserProfile(source);

        expect(bindings.emailVerified.value).toBe(true);
        expect(bindings.oldProfile.value).toMatchObject({ username: 'alice', email: 'alice@example.test' });
        expect(bindings.newProfile.value).toMatchObject({ username: 'alice', email: 'alice@example.test' });
        expect(bindings.newProfile.value.password).toBe('');
        expect(bindings.newProfile.value.confirmPassword).toBe('');
        expect((bindings.newProfile.value as any).accessToken).toBeUndefined();
        expect(bindings.newProfile.value.investmentPlatformKeywords).not.toBe(source.investmentPlatformKeywords);
        expect(bindings.newProfile.value.investmentPlatformKeywords).not.toBe(bindings.oldProfile.value.investmentPlatformKeywords);

        bindings.newProfile.value.email = 'changed@example.test';
        bindings.newProfile.value.investmentPlatformKeywords.push('temporary');
        bindings.reset();
        expect(bindings.newProfile.value.email).toBe('alice@example.test');
        expect(bindings.newProfile.value.investmentPlatformKeywords).toEqual(['broker']);
        expect(Object.keys(bindings)).not.toEqual(expect.arrayContaining([
            'accessToken', 'refreshToken', 'save', 'uploadAvatar', 'deleteAvatar'
        ]));
    });

    test('post-update synchronization resets overview only for week changes and applies locale and color settings', () => {
        const bindings = useUserProfilePageBase();
        setCleanProfile(bindings);
        jest.clearAllMocks();

        bindings.doAfterProfileUpdate(null as never);
        expect(mockSetLanguage).not.toHaveBeenCalled();
        expect(mockResetTransactionOverview).not.toHaveBeenCalled();

        const sameWeek = profile({ nickname: 'Updated', firstDayOfWeek: 1, language: 'en' });
        bindings.doAfterProfileUpdate(sameWeek);
        expect(mockResetTransactionOverview).not.toHaveBeenCalled();
        expect(mockSetLanguage).toHaveBeenCalledWith('en');
        expect(mockUpdateLocalizedDefaultSettings).toHaveBeenCalledWith({ language: 'en', localeDefaults: true });
        expect(mockSetExpenseAndIncomeAmountColor).toHaveBeenCalledWith(9, 10);
        expect(bindings.newProfile.value.nickname).toBe('Updated');

        jest.clearAllMocks();
        const changedWeek = profile({ firstDayOfWeek: 5, language: 'fr', expenseAmountColor: 21, incomeAmountColor: 22 });
        bindings.doAfterProfileUpdate(changedWeek);
        expect(mockResetTransactionOverview).toHaveBeenCalledTimes(1);
        expect(mockSetLanguage).toHaveBeenCalledWith('fr');
        expect(mockUpdateLocalizedDefaultSettings).toHaveBeenCalledWith({ language: 'fr', localeDefaults: true });
        expect(mockSetExpenseAndIncomeAmountColor).toHaveBeenCalledWith(21, 22);
        expect(bindings.emailVerified.value).toBe(true);
    });
});
