<template src="./profile/UserProfilePage.template.html"></template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import type { Router } from 'framework7/types';
import type { LanguageOption } from '@/locales/index.ts';
import { useI18n } from '@/locales/helpers.ts';
import { useI18nUIComponents, showLoading, hideLoading } from '@/lib/ui/mobile.ts';
import { useUserProfilePageBase } from '@/views/base/users/UserProfilePageBase.ts';

import { useRootStore } from '@/stores/index.ts';
import { useUserStore } from '@/stores/user.ts';
import { useAccountsStore } from '@/stores/account.ts';

import { TextDirection } from '@/core/text.ts';
import { NumeralSystem } from '@/core/numeral.ts';
import type { LocalizedCurrencyInfo } from '@/core/currency.ts';

import type { UserProfileResponse } from '@/models/user.ts';
import { Account } from '@/models/account.ts';

import { findDisplayNameByType } from '@/lib/common.ts';
import { isUserVerifyEmailEnabled } from '@/lib/server_settings.ts';
import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import { createMobileProfileLabels } from './profile/mobileProfileLabels.ts';

const props = defineProps<{
    f7router: Router.Router;
}>();

const {
    tt,
    getCurrentLanguageTextDirection,
    getAllLanguageOptions,
    getAllCurrencies,
    getCurrencyName,
    formatFiscalYearStartToGregorianLikeLongMonth
} = useI18n();

const { showAlert, showToast, routeBackOnError } = useI18nUIComponents();

const {
    newProfile,
    emailVerified,
    loading,
    resending,
    saving,
    allAccounts,
    allVisibleAccounts,
    allVisibleCategorizedAccounts,
    allWeekDays,
    allCalendarDisplayTypes,
    allDateDisplayTypes,
    allLongDateFormats,
    allShortDateFormats,
    allLongTimeFormats,
    allShortTimeFormats,
    allFiscalYearFormats,
    allCurrencyDisplayTypes,
    allNumeralSystemTypes,
    allDecimalSeparators,
    allDigitGroupingSymbols,
    allDigitGroupingTypes,
    allCoordinateDisplayTypes,
    allExpenseAmountColorTypes,
    allIncomeAmountColorTypes,
    allTransactionEditScopeTypes,
    languageTitle,
    supportDigitGroupingSymbol,
    inputIsNotChangedProblemMessage,
    inputInvalidProblemMessage,
    langAndRegionInputInvalidProblemMessage,
    extendInputInvalidProblemMessage,
    inputIsNotChanged,
    inputIsInvalid,
    langAndRegionInputIsInvalid,
    extendInputIsInvalid,
    setCurrentUserProfile,
    doAfterProfileUpdate
} = useUserProfilePageBase();

const rootStore = useRootStore();
const userStore = useUserStore();
const accountsStore = useAccountsStore();

const currentPassword = ref<string>('');
const currentNoPassword = ref<boolean>(false);
const loadingError = ref<unknown | null>(null);
const showInputPasswordSheet = ref<boolean>(false);
const showAccountSheet = ref<boolean>(false);
const showEditableTransactionRangePopup = ref<boolean>(false);
const showLanguagePopup = ref<boolean>(false);
const showDefaultCurrencyPopup = ref<boolean>(false);
const showFirstDayOfWeekPopup = ref<boolean>(false);
const showFiscalYearStartSheet = ref<boolean>(false);
const showCalendarDisplayTypePopup = ref<boolean>(false);
const showDateDisplayTypePopup = ref<boolean>(false);
const showLongDateFormatPopup = ref<boolean>(false);
const showShortDateFormatPopup = ref<boolean>(false);
const showLongTimeFormatPopup = ref<boolean>(false);
const showShortTimeFormatPopup = ref<boolean>(false);
const showFiscalYearFormatPopup = ref<boolean>(false);
const showCurrencyDisplayTypePopup = ref<boolean>(false);
const showNumberSystemPopup = ref<boolean>(false);
const showDigitGroupingPopup = ref<boolean>(false);
const showDigitGroupingSymbolPopup = ref<boolean>(false);
const showDecimalSeparatorPopup = ref<boolean>(false);
const showCoordinateDisplayTypePopup = ref<boolean>(false);
const showExpenseAmountColorPopup = ref<boolean>(false);
const showIncomeAmountColorPopup = ref<boolean>(false);
const showMoreActionSheet = ref<boolean>(false);

const allLanguages = computed<LanguageOption[]>(() => getAllLanguageOptions(true));
const allCurrencies = computed<LocalizedCurrencyInfo[]>(() => getAllCurrencies());
const {
    currentLanguageName,
    currentDayOfWeekName
} = createMobileProfileLabels({
    allLanguages,
    allWeekDays,
    newProfile,
    tt
});

function init(): void {
    loading.value = true;

    const promises = [
        accountsStore.loadAllAccounts({ force: false }),
        userStore.getCurrentUserProfile()
    ];

    Promise.all(promises).then(responses => {
        const profile = responses[1] as UserProfileResponse;
        setCurrentUserProfile(profile);
        currentNoPassword.value = !!profile.noPassword;
        loading.value = false;
    }).catch(error => {
        if (error.processed) {
            loading.value = false;
        } else {
            loadingError.value = error;
            showToast(error.message || error);
        }
    });
}

function save(confirm?: boolean): void {
    const router = props.f7router;

    showInputPasswordSheet.value = false;

    const problemMessage = inputIsNotChangedProblemMessage.value || inputInvalidProblemMessage.value || extendInputInvalidProblemMessage.value || langAndRegionInputInvalidProblemMessage.value;

    if (problemMessage) {
        showAlert(problemMessage);
        return;
    }

    if (newProfile.value.password && !confirm) {
        showInputPasswordSheet.value = true;
        return;
    }

    const oldTextDirection: TextDirection = getCurrentLanguageTextDirection();

    saving.value = true;
    showLoading(() => saving.value);

    rootStore.updateUserProfile(newProfile.value.toProfileUpdateRequest(currentPassword.value)).then(response => {
        saving.value = false;
        hideLoading();
        currentPassword.value = '';

        doAfterProfileUpdate(response.user);
        showToast('Your profile has been successfully updated');

        if (oldTextDirection === getCurrentLanguageTextDirection()) {
            router.back(); // if text direction is changed, the page will be reloaded, so it don't need to go back
        }
    }).catch(error => {
        saving.value = false;
        hideLoading();
        currentPassword.value = '';

        if (!error.processed) {
            showToast(error.message || error);
        }
    });
}

function resendVerifyEmail(): void {
    resending.value = true;
    showLoading(() => resending.value);

    rootStore.resendVerifyEmailByLoginedUser().then(() => {
        resending.value = false;
        hideLoading();

        showToast('Validation email has been sent');
    }).catch(error => {
        resending.value = false;
        hideLoading();

        if (!error.processed) {
            showToast(error.message || error);
        }
    });
}

function onPageAfterIn(): void {
    routeBackOnError(props.f7router, loadingError);
}

useExternalTemplateBindings(props, tt, getCurrencyName, formatFiscalYearStartToGregorianLikeLongMonth, findDisplayNameByType, isUserVerifyEmailEnabled, TextDirection, NumeralSystem, Account, newProfile, emailVerified, loading, resending, saving, allAccounts, allVisibleAccounts, allVisibleCategorizedAccounts, allWeekDays, allCalendarDisplayTypes, allDateDisplayTypes, allLongDateFormats, allShortDateFormats, allLongTimeFormats, allShortTimeFormats, allFiscalYearFormats, allCurrencyDisplayTypes, allNumeralSystemTypes, allDecimalSeparators, allDigitGroupingSymbols, allDigitGroupingTypes, allCoordinateDisplayTypes, allExpenseAmountColorTypes, allIncomeAmountColorTypes, allTransactionEditScopeTypes, languageTitle, supportDigitGroupingSymbol, inputIsNotChanged, inputIsInvalid, langAndRegionInputIsInvalid, extendInputIsInvalid, currentPassword, currentNoPassword, loadingError, showInputPasswordSheet, showAccountSheet, showEditableTransactionRangePopup, showLanguagePopup, showDefaultCurrencyPopup, showFirstDayOfWeekPopup, showFiscalYearStartSheet, showCalendarDisplayTypePopup, showDateDisplayTypePopup, showLongDateFormatPopup, showShortDateFormatPopup, showLongTimeFormatPopup, showShortTimeFormatPopup, showFiscalYearFormatPopup, showCurrencyDisplayTypePopup, showNumberSystemPopup, showDigitGroupingPopup, showDigitGroupingSymbolPopup, showDecimalSeparatorPopup, showCoordinateDisplayTypePopup, showExpenseAmountColorPopup, showIncomeAmountColorPopup, showMoreActionSheet, allLanguages, allCurrencies, currentLanguageName, currentDayOfWeekName, save, resendVerifyEmail, onPageAfterIn);

init();
</script>
