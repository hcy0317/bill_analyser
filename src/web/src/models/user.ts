import { CalendarDisplayType, DateDisplayType } from '@/core/calendar.ts';
import { LongDateFormat, ShortDateFormat, LongTimeFormat, ShortTimeFormat } from '@/core/datetime.ts';
import { NumeralSystem, DecimalSeparator, DigitGroupingSymbol, DigitGroupingType } from '@/core/numeral.ts';
import { CurrencyDisplayType } from '@/core/currency.ts';
import { CoordinateDisplayType } from '@/core/coordinate.ts';
import { PresetAmountColor } from '@/core/color.ts';
import type { LocalizedPresetCategory } from '@/core/category.ts';
import { TransactionEditScopeType } from '@/core/transaction.ts';
import { FiscalYearFormat, FiscalYearStart } from '@/core/fiscalyear';

export class User {
    public username: string = '';
    public password: string = '';
    public confirmPassword: string = '';
    public email: string = '';
    public nickname: string = '';
    public language: string;
    public defaultCurrency: string;
    public firstDayOfWeek: number;

    public defaultAccountId: string = EMPTY_USER_BASIC_INFO.defaultAccountId;
    public transactionEditScope: number = EMPTY_USER_BASIC_INFO.transactionEditScope;
    public fiscalYearStart: number = EMPTY_USER_BASIC_INFO.fiscalYearStart;
    public calendarDisplayType: number = EMPTY_USER_BASIC_INFO.calendarDisplayType;
    public dateDisplayType: number = EMPTY_USER_BASIC_INFO.dateDisplayType;
    public longDateFormat: number = EMPTY_USER_BASIC_INFO.longDateFormat;
    public shortDateFormat: number = EMPTY_USER_BASIC_INFO.shortDateFormat;
    public longTimeFormat: number = EMPTY_USER_BASIC_INFO.longTimeFormat;
    public shortTimeFormat: number = EMPTY_USER_BASIC_INFO.shortTimeFormat;
    public fiscalYearFormat: number = EMPTY_USER_BASIC_INFO.fiscalYearFormat;
    public currencyDisplayType: number = EMPTY_USER_BASIC_INFO.currencyDisplayType;
    public numeralSystem: number = EMPTY_USER_BASIC_INFO.numeralSystem;
    public decimalSeparator: number = EMPTY_USER_BASIC_INFO.decimalSeparator;
    public digitGroupingSymbol: number = EMPTY_USER_BASIC_INFO.digitGroupingSymbol;
    public digitGrouping: number = EMPTY_USER_BASIC_INFO.digitGrouping;
    public coordinateDisplayType: number = EMPTY_USER_BASIC_INFO.coordinateDisplayType;
    public expenseAmountColor: number = EMPTY_USER_BASIC_INFO.expenseAmountColor;
    public incomeAmountColor: number = EMPTY_USER_BASIC_INFO.incomeAmountColor;
    public cashAccountId: string = EMPTY_USER_BASIC_INFO.cashAccountId;
    public cashTransferCategoryId: string = EMPTY_USER_BASIC_INFO.cashTransferCategoryId;
    public importLearningEnabled: boolean = EMPTY_USER_BASIC_INFO.importLearningEnabled;
    public investmentPlatformKeywords: string[] = [...EMPTY_USER_BASIC_INFO.investmentPlatformKeywords];
    public investmentProductKeywords: string[] = [...EMPTY_USER_BASIC_INFO.investmentProductKeywords];
    public investmentExcludeKeywords: string[] = [...EMPTY_USER_BASIC_INFO.investmentExcludeKeywords];

    private constructor(language: string, defaultCurrency: string, firstDayOfWeek: number) {
        this.language = language;
        this.defaultCurrency = defaultCurrency;
        this.firstDayOfWeek = firstDayOfWeek;
    }

    public fillFrom(user: User | UserBasicInfo | UserProfileResponse): void {
        this.username = user.username;
        this.email = user.email;
        this.nickname = user.nickname;
        this.defaultAccountId = user.defaultAccountId;
        this.transactionEditScope = user.transactionEditScope;
        this.language = user.language;
        this.defaultCurrency = user.defaultCurrency;
        this.firstDayOfWeek = user.firstDayOfWeek;
        this.fiscalYearStart = user.fiscalYearStart;
        this.calendarDisplayType = user.calendarDisplayType;
        this.dateDisplayType = user.dateDisplayType;
        this.longDateFormat = user.longDateFormat;
        this.shortDateFormat = user.shortDateFormat;
        this.longTimeFormat = user.longTimeFormat;
        this.shortTimeFormat = user.shortTimeFormat;
        this.fiscalYearFormat = user.fiscalYearFormat;
        this.currencyDisplayType = user.currencyDisplayType;
        this.numeralSystem = user.numeralSystem;
        this.decimalSeparator = user.decimalSeparator;
        this.digitGroupingSymbol = user.digitGroupingSymbol;
        this.digitGrouping = user.digitGrouping;
        this.coordinateDisplayType = user.coordinateDisplayType;
        this.expenseAmountColor = user.expenseAmountColor;
        this.incomeAmountColor = user.incomeAmountColor;
        this.cashAccountId = user.cashAccountId;
        this.cashTransferCategoryId = user.cashTransferCategoryId;
        this.importLearningEnabled = user.importLearningEnabled;
        this.investmentPlatformKeywords = [...(user.investmentPlatformKeywords || [])];
        this.investmentProductKeywords = [...(user.investmentProductKeywords || [])];
        this.investmentExcludeKeywords = [...(user.investmentExcludeKeywords || [])];
    }

    public toRegisterRequest(categories?: LocalizedPresetCategory[]): UserRegisterRequest {
        return {
            username: this.username,
            email: this.email,
            nickname: this.nickname,
            password: this.password,
            language: this.language,
            defaultCurrency: this.defaultCurrency,
            firstDayOfWeek: this.firstDayOfWeek,
            categories: categories
        };
    }

    public toProfileUpdateRequest(currentPassword?: string): UserProfileUpdateRequest {
        return {
            email: this.email,
            nickname: this.nickname,
            password: this.password,
            oldPassword: currentPassword,
            defaultAccountId: this.defaultAccountId,
            transactionEditScope: this.transactionEditScope,
            language: this.language,
            defaultCurrency: this.defaultCurrency,
            firstDayOfWeek: this.firstDayOfWeek,
            fiscalYearStart: this.fiscalYearStart,
            calendarDisplayType: this.calendarDisplayType,
            dateDisplayType: this.dateDisplayType,
            longDateFormat: this.longDateFormat,
            shortDateFormat: this.shortDateFormat,
            longTimeFormat: this.longTimeFormat,
            shortTimeFormat: this.shortTimeFormat,
            fiscalYearFormat: this.fiscalYearFormat,
            currencyDisplayType: this.currencyDisplayType,
            numeralSystem: this.numeralSystem,
            decimalSeparator: this.decimalSeparator,
            digitGroupingSymbol: this.digitGroupingSymbol,
            digitGrouping: this.digitGrouping,
            coordinateDisplayType: this.coordinateDisplayType,
            expenseAmountColor: this.expenseAmountColor,
            incomeAmountColor: this.incomeAmountColor,
            cashAccountId: this.cashAccountId,
            cashTransferCategoryId: this.cashTransferCategoryId,
            importLearningEnabled: this.importLearningEnabled,
            investmentPlatformKeywords: this.investmentPlatformKeywords,
            investmentProductKeywords: this.investmentProductKeywords,
            investmentExcludeKeywords: this.investmentExcludeKeywords
        };
    }

    public static of(userInfo: UserBasicInfo): User {
        const user = new User(userInfo.language, userInfo.defaultCurrency, userInfo.firstDayOfWeek);
        user.defaultAccountId = userInfo.defaultAccountId;
        user.transactionEditScope = userInfo.transactionEditScope;
        user.fiscalYearStart = userInfo.fiscalYearStart;
        user.calendarDisplayType = userInfo.calendarDisplayType;
        user.dateDisplayType = userInfo.dateDisplayType;
        user.longDateFormat = userInfo.longDateFormat;
        user.shortDateFormat = userInfo.shortDateFormat;
        user.longTimeFormat = userInfo.longTimeFormat;
        user.shortTimeFormat = userInfo.shortTimeFormat;
        user.fiscalYearFormat = userInfo.fiscalYearFormat;
        user.currencyDisplayType = userInfo.currencyDisplayType;
        user.numeralSystem = userInfo.numeralSystem;
        user.decimalSeparator = userInfo.decimalSeparator;
        user.digitGroupingSymbol = userInfo.digitGroupingSymbol;
        user.digitGrouping = userInfo.digitGrouping;
        user.coordinateDisplayType = userInfo.coordinateDisplayType;
        user.expenseAmountColor = userInfo.expenseAmountColor;
        user.incomeAmountColor = userInfo.incomeAmountColor;
        user.cashAccountId = userInfo.cashAccountId;
        user.cashTransferCategoryId = userInfo.cashTransferCategoryId;
        user.importLearningEnabled = userInfo.importLearningEnabled;
        user.investmentPlatformKeywords = [...(userInfo.investmentPlatformKeywords || [])];
        user.investmentProductKeywords = [...(userInfo.investmentProductKeywords || [])];
        user.investmentExcludeKeywords = [...(userInfo.investmentExcludeKeywords || [])];

        return user;
    }

    public static createNewUser(language: string, defaultCurrency: string, firstDayOfWeek: number): User {
        return new User(language, defaultCurrency, firstDayOfWeek);
    }
}

export interface UserBasicInfo {
    readonly id?: number;  // 🆕 用户ID（后端auth.py返回）
    readonly username: string;
    readonly email: string;
    readonly nickname: string;
    readonly avatar: string;
    readonly avatarProvider?: string;
    readonly defaultAccountId: string;
    readonly transactionEditScope: number;
    readonly language: string;
    readonly defaultCurrency: string;
    readonly firstDayOfWeek: number;
    readonly fiscalYearStart: number;
    readonly calendarDisplayType: number;
    readonly dateDisplayType: number;
    readonly longDateFormat: number;
    readonly shortDateFormat: number;
    readonly longTimeFormat: number;
    readonly shortTimeFormat: number;
    readonly fiscalYearFormat: number;
    readonly currencyDisplayType: number;
    readonly numeralSystem: number;
    readonly decimalSeparator: number;
    readonly digitGroupingSymbol: number;
    readonly digitGrouping: number;
    readonly coordinateDisplayType: number;
    readonly expenseAmountColor: number;
    readonly incomeAmountColor: number;
    readonly cashAccountId: string;
    readonly cashTransferCategoryId: string;
    readonly importLearningEnabled: boolean;
    readonly investmentPlatformKeywords: string[];
    readonly investmentProductKeywords: string[];
    readonly investmentExcludeKeywords: string[];
    readonly emailVerified: boolean;
}

export interface UserLoginRequest {
    readonly loginName: string;
    readonly password: string;
}

export interface UserRegisterRequest {
    readonly username: string;
    readonly email: string;
    readonly nickname: string;
    readonly password: string;
    readonly language: string;
    readonly defaultCurrency: string;
    readonly firstDayOfWeek: number;
    readonly categories?: LocalizedPresetCategory[];
}

export interface UserVerifyEmailResponse {
    readonly newToken?: string;
    readonly user: UserBasicInfo;
    readonly notificationContent?: string;
}

export interface UserResendVerifyEmailRequest {
    readonly email: string;
    readonly password: string;
}

export interface UserProfileUpdateRequest {
    readonly email?: string;
    readonly nickname?: string;
    readonly password?: string;
    readonly oldPassword?: string;
    readonly defaultAccountId?: string;
    readonly transactionEditScope?: number;
    readonly language?: string;
    readonly defaultCurrency?: string;
    readonly firstDayOfWeek?: number;
    readonly fiscalYearStart?: number;
    readonly calendarDisplayType?: number;
    readonly dateDisplayType?: number;
    readonly longDateFormat?: number;
    readonly shortDateFormat?: number;
    readonly longTimeFormat?: number;
    readonly shortTimeFormat?: number;
    readonly fiscalYearFormat?: number;
    readonly currencyDisplayType?: number;
    readonly numeralSystem?: number;
    readonly decimalSeparator?: number;
    readonly digitGroupingSymbol?: number;
    readonly digitGrouping?: number;
    readonly coordinateDisplayType?: number;
    readonly expenseAmountColor?: number;
    readonly incomeAmountColor?: number;
    readonly cashAccountId?: string;
    readonly cashTransferCategoryId?: string;
    readonly importLearningEnabled?: boolean;
    readonly investmentPlatformKeywords?: string[];
    readonly investmentProductKeywords?: string[];
    readonly investmentExcludeKeywords?: string[];
}

export interface UserProfileUpdateResponse {
    readonly user: UserBasicInfo;
    readonly newToken?: string;
}

export interface UserProfileResponse extends UserBasicInfo {
    readonly noPassword?: boolean;
    readonly lastLoginAt: number;
}

export const EMPTY_USER_BASIC_INFO: UserBasicInfo = {
    username: '',
    email: '',
    nickname: '',
    avatar: '',
    avatarProvider: undefined,
    defaultAccountId: '',
    transactionEditScope: TransactionEditScopeType.All.type,
    language: '',
    defaultCurrency: '',
    firstDayOfWeek: -1,
    fiscalYearStart: FiscalYearStart.Default.value,
    calendarDisplayType: CalendarDisplayType.Default.type,
    dateDisplayType: DateDisplayType.Default.type,
    longDateFormat: LongDateFormat.Default.type,
    shortDateFormat: ShortDateFormat.Default.type,
    longTimeFormat: LongTimeFormat.Default.type,
    shortTimeFormat: ShortTimeFormat.Default.type,
    fiscalYearFormat: FiscalYearFormat.Default.type,
    currencyDisplayType: CurrencyDisplayType.Default.type,
    numeralSystem: NumeralSystem.Default.type,
    decimalSeparator: DecimalSeparator.LanguageDefaultType,
    digitGroupingSymbol: DigitGroupingSymbol.LanguageDefaultType,
    digitGrouping: DigitGroupingType.LanguageDefaultType,
    coordinateDisplayType: CoordinateDisplayType.Default.type,
    expenseAmountColor: PresetAmountColor.DefaultExpenseColor.type,
    incomeAmountColor: PresetAmountColor.DefaultIncomeColor.type,
    cashAccountId: '',
    cashTransferCategoryId: '',
    importLearningEnabled: true,
    investmentPlatformKeywords: [],
    investmentProductKeywords: [],
    investmentExcludeKeywords: [],
    emailVerified: false
}
