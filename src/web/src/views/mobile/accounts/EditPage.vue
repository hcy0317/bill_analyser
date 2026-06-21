<template src="./edit-page/EditPage.template.html"></template>

<script setup lang="ts">import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import { ref, computed, watch } from 'vue';
import type { Router } from 'framework7/types';
import { useI18n } from '@/locales/helpers.ts';
import { useI18nUIComponents, showLoading, hideLoading } from '@/lib/ui/mobile.ts';
import { useAccountEditPageBase } from '@/views/base/accounts/AccountEditPageBase.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { itemAndIndex } from '@/core/base.ts';
import type { LocalizedCurrencyInfo } from '@/core/currency.ts';
import { AccountType } from '@/core/account.ts';
import { ALL_ACCOUNT_ICONS } from '@/consts/icon.ts';
import { ALL_ACCOUNT_COLORS } from '@/consts/color.ts';
import { TRANSACTION_MIN_AMOUNT, TRANSACTION_MAX_AMOUNT } from '@/consts/transaction.ts';
import type { Account } from '@/models/account.ts';
import { isDefined, findDisplayNameByType } from '@/lib/common.ts';
import { generateRandomUUID } from '@/lib/misc.ts';
import {
    getTimezoneOffsetMinutes,
    getBrowserTimezoneOffsetMinutes,
    getActualUnixTimeForStore
} from '@/lib/datetime.ts';
interface AccountContext {
    showIconSelectionSheet: boolean;
    showColorSelectionSheet: boolean;
    showCurrencyPopup: boolean;
    showCreditCardStatementDatePopup: boolean;
    showBalanceSheet: boolean;
    showBalanceDateTimeSheet: boolean;
    balanceDateTimeSheetMode: string;
}

const props = defineProps<{
    f7route: Router.Route;
    f7router: Router.Router;
}>();

const {
    tt,
    getAllCurrencies,
    getCurrencyName,
    formatUnixTimeToLongDate,
    formatUnixTimeToLongTime,
    formatAmountToLocalizedNumeralsWithCurrency
} = useI18n();

const { showAlert, showToast, routeBackOnError } = useI18nUIComponents();

const {
    editAccountId,
    clientSessionId,
    loading,
    submitting,
    account,
    subAccounts,
    title,
    saveButtonTitle,
    inputEmptyProblemMessage,
    inputIsEmpty,
    allAccountCategories,
    allAccountTypes,
    allAvailableMonthDays,
    isAccountSupportCreditCardStatementDate,
    getAccountCreditCardStatementDate,
    isNewAccount,
    addSubAccount,
    setAccount
} = useAccountEditPageBase();

const accountsStore = useAccountsStore();

const DEFAULT_ACCOUNT_CONTEXT: AccountContext = {
    showIconSelectionSheet: false,
    showColorSelectionSheet: false,
    showCurrencyPopup: false,
    showCreditCardStatementDatePopup: false,
    showBalanceSheet: false,
    showBalanceDateTimeSheet: false,
    balanceDateTimeSheetMode: 'time'
};

const accountContext = ref<AccountContext>(Object.assign({}, DEFAULT_ACCOUNT_CONTEXT));
const subAccountContexts = ref<AccountContext[]>([]);
const subAccountToDelete = ref<Account | null>(null);
const loadingError = ref<unknown | null>(null);
const showAccountCategorySheet = ref<boolean>(false);
const showAccountTypeSheet = ref<boolean>(false);
const showMoreActionSheet = ref<boolean>(false);
const showDeleteActionSheet = ref<boolean>(false);

const allCurrencies = computed<LocalizedCurrencyInfo[]>(() => getAllCurrencies());

function formatAccountDisplayBalance(selectedAccount: Account): string {
    const balance = account.value.isLiability ? -selectedAccount.balanceCents : selectedAccount.balanceCents;
    return formatAmountToLocalizedNumeralsWithCurrency(balance, selectedAccount.currency);
}

function formatAccountBalanceDate(account: Account): string {
    if (!isDefined(account.balanceTime)) {
        return '';
    }

    return formatUnixTimeToLongDate(getActualUnixTimeForStore(account.balanceTime, getTimezoneOffsetMinutes(), getBrowserTimezoneOffsetMinutes()));
}

function formatAccountBalanceTime(account: Account): string {
    if (!isDefined(account.balanceTime)) {
        return '';
    }

    return formatUnixTimeToLongTime(getActualUnixTimeForStore(account.balanceTime, getTimezoneOffsetMinutes(), getBrowserTimezoneOffsetMinutes()));
}

function init(): void {
    const query = props.f7route.query;
    clientSessionId.value = generateRandomUUID();

    if (query['id']) {
        loading.value = true;

        editAccountId.value = query['id'];

        accountsStore.getAccount({
            accountId: editAccountId.value
        }).then(response => {
            setAccount(response);
            subAccountContexts.value = [];

            for (let i = 0; i < subAccounts.value.length; i++) {
                subAccountContexts.value.push(Object.assign({}, DEFAULT_ACCOUNT_CONTEXT));
            }

            loading.value = false;
        }).catch(error => {
            if (error.processed) {
                loading.value = false;
            } else {
                loadingError.value = error;
                showToast(error.message || error);
            }
        });
    } else {
        loading.value = false;
    }
}

/** 中文说明：移动端保存账户和子账户草稿，提交期间使用移动端全局 loading 避免重复保存。 */ function save(): void {
    const router = props.f7router;
    const problemMessage = inputEmptyProblemMessage.value;

    if (problemMessage) {
        showAlert(problemMessage);
        return;
    }

    submitting.value = true;
    showLoading(() => submitting.value);

    accountsStore.saveAccount({
        account: account.value,
        subAccounts: subAccounts.value,
        isEdit: !!editAccountId.value,
        clientSessionId: clientSessionId.value
    }).then(() => {
        submitting.value = false;
        hideLoading();

        if (!editAccountId.value) {
            showToast('You have added a new account');
        } else {
            showToast('You have saved this account');
        }

        router.back();
    }).catch(error => {
        submitting.value = false;
        hideLoading();

        if (!error.processed) {
            showToast(error.message || error);
        }
    });
}

/** 中文说明：新增子账户时同步创建对应的移动端弹层上下文。 */ function addSubAccountAndContext(): void {
    if (addSubAccount()) {
        subAccountContexts.value.push(Object.assign({}, DEFAULT_ACCOUNT_CONTEXT));
    }
}

// 中文说明：外置模板的子账户上下文访问器；兜底只修复异常索引空洞，不改变正常交互状态模型。
function subAccountContext(index: number): AccountContext { return subAccountContexts.value[index] ?? (subAccountContexts.value[index] = Object.assign({}, DEFAULT_ACCOUNT_CONTEXT)); }

/** 中文说明：移动端分两步删除子账户，首次打开确认面板，确认后同步删除上下文数组。 */ function removeSubAccount(currentSubAccount: Account | null, confirm: boolean): void {
    if (!currentSubAccount) {
        showAlert('An error occurred');
        return;
    }

    if (!confirm) {
        subAccountToDelete.value = currentSubAccount;
        showDeleteActionSheet.value = true;
        return;
    }

    showDeleteActionSheet.value = false;
    subAccountToDelete.value = null;

    for (const [subAccount, index] of itemAndIndex(subAccounts.value)) {
        if (subAccount === currentSubAccount) {
            subAccounts.value.splice(index, 1);
            subAccountContexts.value.splice(index, 1);
        }
    }
}

function showDateTimeDialog(accountContext: AccountContext, sheetMode: string): void {
    accountContext.balanceDateTimeSheetMode = sheetMode;
    accountContext.showBalanceDateTimeSheet = true;
}

function onPageAfterIn(): void {
    routeBackOnError(props.f7router, loadingError);
}

watch(() => account.value.type, () => {
    if (subAccounts.value.length < 1) {
        addSubAccountAndContext();
    }
});

init();

useExternalTemplateBindings(ref, computed, watch, useI18n, useI18nUIComponents, showLoading, hideLoading, useAccountEditPageBase, useAccountsStore, itemAndIndex, AccountType, ALL_ACCOUNT_ICONS, ALL_ACCOUNT_COLORS, TRANSACTION_MIN_AMOUNT, TRANSACTION_MAX_AMOUNT, isDefined, findDisplayNameByType, generateRandomUUID, getTimezoneOffsetMinutes, getBrowserTimezoneOffsetMinutes, getActualUnixTimeForStore, props, tt, getAllCurrencies, getCurrencyName, formatUnixTimeToLongDate, formatUnixTimeToLongTime, formatAmountToLocalizedNumeralsWithCurrency, showAlert, showToast, routeBackOnError, editAccountId, clientSessionId, loading, submitting, account, subAccounts, title, saveButtonTitle, inputEmptyProblemMessage, inputIsEmpty, allAccountCategories, allAccountTypes, allAvailableMonthDays, isAccountSupportCreditCardStatementDate, getAccountCreditCardStatementDate, isNewAccount, addSubAccount, setAccount, accountsStore, DEFAULT_ACCOUNT_CONTEXT, accountContext, subAccountContexts, subAccountToDelete, loadingError, showAccountCategorySheet, showAccountTypeSheet, showMoreActionSheet, showDeleteActionSheet, allCurrencies, formatAccountDisplayBalance, formatAccountBalanceDate, formatAccountBalanceTime, init, save, addSubAccountAndContext, subAccountContext, removeSubAccount, showDateTimeDialog, onPageAfterIn);
</script>

<style src="./edit-page/EditPage.css"></style>
