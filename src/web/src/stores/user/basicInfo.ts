import { ref, computed } from 'vue';

import { type WeekDayValue, WeekDay } from '@/core/datetime.ts';
import { FiscalYearStart } from '@/core/fiscalyear.ts';
import {
    type UserBasicInfo,
    User,
    EMPTY_USER_BASIC_INFO,
    normalizeUserBasicInfo
} from '@/models/user.ts';

import { isNumber } from '@/lib/common.ts';
import {
    getCurrentUserInfo,
    updateCurrentUserInfo,
    clearCurrentUserInfo
} from '@/lib/userstate.ts';

type UserBasicInfoSettings = {
    localeDefaultSettings: {
        currency: string;
        firstDayOfWeek: WeekDayValue;
    };
};

type AvatarUrlResolver = (userInfoOrAvatarUrl: UserBasicInfo | string | null, disableBrowserCache: boolean | string) => string | null;

export function createUserBasicInfoState(settingsStore: UserBasicInfoSettings, getUserAvatarUrl: AvatarUrlResolver) {
    const currentUserBasicInfo = ref<UserBasicInfo | null>(getCurrentUserInfo());

    const currentUserNickname = computed<string | null>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.nickname || userInfo.username || null;
    });

    const currentUserAvatar = computed<string | null>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return getUserAvatarUrl(userInfo, false);
    });

    const currentUserDefaultAccountId = computed<string>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.defaultAccountId;
    });

    const currentUserLanguage = computed<string>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.language;
    });

    const currentUserDefaultCurrency = computed<string>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.defaultCurrency || settingsStore.localeDefaultSettings.currency;
    });

    const currentUserFirstDayOfWeek = computed<WeekDayValue>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return isNumber(userInfo.firstDayOfWeek) && WeekDay.valueOf(userInfo.firstDayOfWeek) ? userInfo.firstDayOfWeek as WeekDayValue : settingsStore.localeDefaultSettings.firstDayOfWeek;
    });

    const currentUserFiscalYearStart = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return isNumber(userInfo.fiscalYearStart) && FiscalYearStart.valueOf(userInfo.fiscalYearStart) ? userInfo.fiscalYearStart : EMPTY_USER_BASIC_INFO.fiscalYearStart;
    });

    const currentUserCalendarDisplayType = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.calendarDisplayType;
    });

    const currentUserDateDisplayType = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.dateDisplayType;
    });

    const currentUserLongDateFormat = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.longDateFormat;
    });

    const currentUserShortDateFormat = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.shortDateFormat;
    });

    const currentUserLongTimeFormat = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.longTimeFormat;
    });

    const currentUserShortTimeFormat = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.shortTimeFormat;
    });

    const currentUserFiscalYearFormat = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.fiscalYearFormat;
    });

    const currentUserCurrencyDisplayType = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.currencyDisplayType;
    });

    const currentUserNumeralSystem = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.numeralSystem;
    });

    const currentUserDecimalSeparator = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.decimalSeparator;
    });

    const currentUserDigitGroupingSymbol = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.digitGroupingSymbol;
    });

    const currentUserDigitGrouping = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.digitGrouping;
    });

    const currentUserCoordinateDisplayType = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.coordinateDisplayType;
    });

    const currentUserExpenseAmountColor = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.expenseAmountColor;
    });

    const currentUserIncomeAmountColor = computed<number>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.incomeAmountColor;
    });

    const currentUserCashAccountId = computed<string>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.cashAccountId;
    });

    const currentUserCashTransferCategoryId = computed<string>(() => {
        const userInfo = currentUserBasicInfo.value || EMPTY_USER_BASIC_INFO;
        return userInfo.cashTransferCategoryId;
    });

    function generateNewUserModel(language: string): User {
        return User.createNewUser(language, settingsStore.localeDefaultSettings.currency, settingsStore.localeDefaultSettings.firstDayOfWeek);
    }

    function storeUserBasicInfo(userInfo: UserBasicInfo): void {
        const normalizedUserInfo = normalizeUserBasicInfo(userInfo);
        currentUserBasicInfo.value = normalizedUserInfo;
        updateCurrentUserInfo(normalizedUserInfo);
    }

    function resetUserBasicInfo(): void {
        currentUserBasicInfo.value = null;
        clearCurrentUserInfo();
    }

    return {
        currentUserBasicInfo,
        currentUserNickname,
        currentUserAvatar,
        currentUserDefaultAccountId,
        currentUserLanguage,
        currentUserDefaultCurrency,
        currentUserFirstDayOfWeek,
        currentUserFiscalYearStart,
        currentUserCalendarDisplayType,
        currentUserDateDisplayType,
        currentUserLongDateFormat,
        currentUserShortDateFormat,
        currentUserLongTimeFormat,
        currentUserShortTimeFormat,
        currentUserFiscalYearFormat,
        currentUserCurrencyDisplayType,
        currentUserNumeralSystem,
        currentUserDecimalSeparator,
        currentUserDigitGroupingSymbol,
        currentUserDigitGrouping,
        currentUserCoordinateDisplayType,
        currentUserExpenseAmountColor,
        currentUserIncomeAmountColor,
        currentUserCashAccountId,
        currentUserCashTransferCategoryId,
        generateNewUserModel,
        storeUserBasicInfo,
        resetUserBasicInfo
    };
}
