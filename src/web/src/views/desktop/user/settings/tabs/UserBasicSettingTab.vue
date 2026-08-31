<template src="./basic/UserBasicSettingTab.template.html"></template>

<script setup lang="ts">
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import DefaultImportDirectorySettingsCard from '@/views/desktop/app/settings/tabs/DefaultImportDirectorySettingsCard.vue';
import { ref, computed, useTemplateRef } from 'vue';
import { useI18n } from '@/locales/helpers.ts';
import { useUserProfilePageBase } from '@/views/base/users/UserProfilePageBase.ts';
import { useRootStore } from '@/stores/index.ts';
import { useUserStore } from '@/stores/user.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { CategoryType } from '@/core/category.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import { SUPPORTED_IMAGE_EXTENSIONS } from '@/consts/file.ts';
import type { UserProfileResponse } from '@/models/user.ts';
import { Account } from '@/models/account.ts';
import { createAccountCategorySelectionTexts } from './basic/accountCategoryLabels.ts';

import { generateRandomUUID } from '@/lib/misc.ts';
import { isUserVerifyEmailEnabled } from '@/lib/server_settings.ts';
import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';

import {
    mdiAccount,
    mdiAccountEditOutline
} from '@mdi/js';

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();

const {
    newProfile,
    oldProfile,
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
    setCurrentUserProfile,
    reset,
    doAfterProfileUpdate
} = useUserProfilePageBase();

const rootStore = useRootStore();
const userStore = useUserStore();
const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();

const allCategories = computed<Record<number, TransactionCategory[]>>(() => transactionCategoriesStore.allTransactionCategories);
const hasAvailableTransferCategories = computed<boolean>(() => transactionCategoriesStore.hasAvailableTransferCategories);
const {
    defaultAccountSelectionText,
    cashAccountSelectionText,
    cashTransferCategoryPrimaryText,
    cashTransferCategorySecondaryText
} = createAccountCategorySelectionTexts({
    allAccounts,
    allCategories,
    newProfile,
    tt
});

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const avatarInput = useTemplateRef<HTMLInputElement>('avatarInput');

const avatarUrl = ref<string>('');
const avatarProvider = ref<string | undefined>('');
const avatarNoCacheId = ref<string>('');

const currentUserAvatar = computed<string | null>(() => userStore.getUserAvatarUrl(avatarUrl.value, avatarNoCacheId.value));

function init(): void {
    loading.value = true;

    const promises = [
        accountsStore.loadAllAccounts({ force: false }),
        transactionCategoriesStore.loadAllCategories({ force: false }),
        userStore.getCurrentUserProfile()
    ];

    Promise.all(promises).then(responses => {
        const profile = responses[2] as UserProfileResponse;
        setCurrentUserProfile(profile);
        avatarUrl.value = profile.avatar;
        avatarProvider.value = profile.avatarProvider;
        loading.value = false;
    }).catch(error => {
        oldProfile.value.nickname = '';
        oldProfile.value.email = '';
        newProfile.value.nickname = '';
        newProfile.value.email = '';
        loading.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function save(): void {
    const problemMessage = inputIsNotChangedProblemMessage.value || inputInvalidProblemMessage.value || extendInputInvalidProblemMessage.value || langAndRegionInputInvalidProblemMessage.value;

    if (problemMessage) {
        snackbar.value?.showMessage(problemMessage);
        return;
    }

    saving.value = true;

    rootStore.updateUserProfile(newProfile.value.toProfileUpdateRequest(undefined, oldProfile.value)).then(response => {
        saving.value = false;

        doAfterProfileUpdate(response.user);
        snackbar.value?.showMessage('Your profile has been successfully updated');
    }).catch(error => {
        saving.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function updateAvatar(event: Event): void {
    if (!event || !event.target) {
        return;
    }

    const el = event.target as HTMLInputElement;

    if (!el.files || !el.files.length || !el.files[0]) {
        return;
    }

    const avatarFile = el.files[0] as File;

    el.value = '';

    saving.value = true;

    userStore.updateUserAvatar({ avatarFile }).then(response => {
        saving.value = false;

        if (response) {
            avatarUrl.value = response.avatar;
            avatarProvider.value = response.avatarProvider;
            avatarNoCacheId.value = generateRandomUUID();
            setCurrentUserProfile(response);
        }

        snackbar.value?.showMessage('Your avatar has been successfully updated');
    }).catch(error => {
        saving.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function removeAvatar(): void {
    confirmDialog.value?.open('Are you sure you want to remove avatar?').then(() => {
        saving.value = true;

        userStore.removeUserAvatar().then(response => {
            saving.value = false;

            if (response) {
                avatarUrl.value = response.avatar;
                avatarProvider.value = response.avatarProvider;
                setCurrentUserProfile(response);
            }

            snackbar.value?.showMessage('Your profile has been successfully updated');
        }).catch(error => {
            saving.value = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    });
}

function resendVerifyEmail(): void {
    resending.value = true;

    rootStore.resendVerifyEmailByLoginedUser().then(() => {
        resending.value = false;
        snackbar.value?.showMessage('Validation email has been sent');
    }).catch(error => {
        resending.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function showOpenAvatarDialog(): void {
    avatarInput.value?.click();
}

useExternalTemplateBindings(ConfirmDialog, SnackBar, DefaultImportDirectorySettingsCard, CategoryType, SUPPORTED_IMAGE_EXTENSIONS, Account, isUserVerifyEmailEnabled, mdiAccount, mdiAccountEditOutline, tt, newProfile, oldProfile, emailVerified, loading, resending, saving, allAccounts, allVisibleAccounts, allVisibleCategorizedAccounts, allWeekDays, allCalendarDisplayTypes, allDateDisplayTypes, allLongDateFormats, allShortDateFormats, allLongTimeFormats, allShortTimeFormats, allFiscalYearFormats, allCurrencyDisplayTypes, allNumeralSystemTypes, allDecimalSeparators, allDigitGroupingSymbols, allDigitGroupingTypes, allCoordinateDisplayTypes, allExpenseAmountColorTypes, allIncomeAmountColorTypes, allTransactionEditScopeTypes, languageTitle, supportDigitGroupingSymbol, inputIsNotChanged, inputIsInvalid, reset, allCategories, hasAvailableTransferCategories, defaultAccountSelectionText, cashAccountSelectionText, cashTransferCategoryPrimaryText, cashTransferCategorySecondaryText, confirmDialog, snackbar, avatarInput, avatarUrl, avatarProvider, currentUserAvatar, save, updateAvatar, removeAvatar, resendVerifyEmail, showOpenAvatarDialog);

init();
</script>

<style src="./basic/UserBasicSettingTab.css"></style>
