<template src="./list/ListPage.template.html"></template>

<script setup lang="ts">import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import EditDialog from './list/dialogs/EditDialog.vue';
import ReconciliationStatementDialog from './list/dialogs/ReconciliationStatementDialog.vue';
import MoveAllTransactionsDialog from '@/views/desktop/accounts/list/dialogs/MoveAllTransactionsDialog.vue';
import ClearAllTransactionsDialog from '@/views/desktop/accounts/list/dialogs/ClearAllTransactionsDialog.vue';
import AccountFilterSettingsCard from '@/views/desktop/common/cards/AccountFilterSettingsCard.vue';
import SettingsJsonImportExportButton from '@/components/desktop/SettingsJsonImportExportButton.vue';
import { ref, computed, onMounted, useTemplateRef, watch } from 'vue';
import { useDisplay } from 'vuetify';
import { useI18n } from '@/locales/helpers.ts';
import { useAccountListPageBase } from '@/views/base/accounts/AccountListPageBase.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { DateRange, DateRangeScene, type LocalizedDateRange, type TimeRangeAndDateType } from '@/core/datetime.ts';
import { AccountType, AccountCategory } from '@/core/account.ts';
import type { Account } from '@/models/account.ts';
import { isNumber } from '@/lib/common.ts';
import { getDateRangeByDateType, getDateRangeByBillingCycleDateType } from '@/lib/datetime.ts';
import {
    mdiEyeOutline,
    mdiEyeOffOutline,
    mdiCalculatorVariantOutline,
    mdiRefresh,
    mdiSquareRounded,
    mdiMenu,
    mdiPencilOutline,
    mdiDotsHorizontalCircleOutline,
    mdiSwapHorizontal,
    mdiEraser,
    mdiDeleteOutline,
    mdiListBoxOutline,
    mdiInvoiceListOutline,
    mdiDrag,
    mdiDotsVertical
} from '@mdi/js';
type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;
type EditDialogType = InstanceType<typeof EditDialog>;
type ReconciliationStatementDialogType = InstanceType<typeof ReconciliationStatementDialog>;
type MoveAllTransactionsDialogType = InstanceType<typeof MoveAllTransactionsDialog>;
type ClearAllTransactionsDialogType = InstanceType<typeof ClearAllTransactionsDialog>;
const display = useDisplay();
const { tt, getAllDateRanges, getCurrencyName, joinMultiText } = useI18n();

const {
    loading,
    showHidden,
    displayOrderModified,
    showAccountBalance,
    firstDayOfWeek,
    fiscalYearStart,
    allAccounts,
    allCategorizedAccountsMap,
    allAccountCount,
    netAssets,
    totalAssets,
    totalLiabilities,
    accountCategoryTotalBalance,
    accountBalance
} = useAccountListPageBase();

const accountsStore = useAccountsStore();

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const editDialog = useTemplateRef<EditDialogType>('editDialog');
const reconciliationStatementDialog = useTemplateRef<ReconciliationStatementDialogType>('reconciliationStatementDialog');
const moveAllTransactionsDialog = useTemplateRef<MoveAllTransactionsDialogType>('moveAllTransactionsDialog');
const clearAllTransactionsDialog = useTemplateRef<ClearAllTransactionsDialogType>('clearAllTransactionsDialog');

const activeAccountCategoryType = ref<number>(AccountCategory.Default.type);
const activeTab = ref<string>('accountPage');
const activeSubAccount = ref<Record<string, string>>({});
const accountToShowReconciliationStatement = ref<Account | null>(null);
const alwaysShowNav = ref<boolean>(display.mdAndUp.value);
const showNav = ref<boolean>(display.mdAndUp.value);
const showAccountsIncludedInTotalDialog = ref<boolean>(false);
const showCustomDateRangeDialog = ref<boolean>(false);
let reloadRequestId = 0;

const hasAnyVisibleAccount = computed<boolean>(() => accountsStore.allVisibleAccountsCount > 0);
const activeAccountCategory = computed<AccountCategory | undefined>(() => AccountCategory.valueOf(activeAccountCategoryType.value));
const activeAccountCategoryTotalBalance = computed<string>(() => accountCategoryTotalBalance(activeAccountCategory.value));

const activeAccountCategoryVisibleAccountCount = computed<number>(() => {
    if (!activeAccountCategory.value) {
        return 0;
    }

    const categorizedAccounts = allCategorizedAccountsMap.value[activeAccountCategory.value.type];

    if (!categorizedAccounts || !categorizedAccounts.accounts || !categorizedAccounts.accounts.length) {
        return 0;
    }

    if (showHidden.value) {
        return categorizedAccounts.accounts.length;
    }

    let visibleCount = 0;

    for (const account of categorizedAccounts.accounts) {
        if (!account.hidden) {
            visibleCount++;
        }
    }

    return visibleCount;
});

function reload(force: boolean): void {
    const currentRequestId = ++reloadRequestId;
    loading.value = true;

    const loadAccounts = () => accountsStore.loadAllAccounts({
        force: force
    });

    const promise = force
        ? accountsStore.syncAllAccountBalances({ refreshAccounts: false }).then(loadAccounts)
        : loadAccounts();

    promise.then(() => {
        if (currentRequestId !== reloadRequestId) {
            return;
        }

        loading.value = false;
        displayOrderModified.value = false;

        if (allAccounts.value) {
            for (const account of allAccounts.value) {
                if (account.type === AccountType.MultiSubAccounts.type && !activeSubAccount.value[account.id]) {
                    activeSubAccount.value[account.id] = '';
                }
            }
        }

        if (force) {
            snackbar.value?.showMessage('Account list has been updated');
        }
    }).catch(error => {
        if (currentRequestId !== reloadRequestId) {
            return;
        }

        loading.value = false;

        if (error && error.isUpToDate) {
            displayOrderModified.value = false;
        }

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function hasAccount(accountCategory: AccountCategory): boolean {
    return accountsStore.hasAccount(accountCategory, !showHidden.value);
}

function accountCurrency(account: Account): string | null {
    if (account.type === AccountType.SingleAccount.type) {
        return getCurrencyName(account.currency);
    } else if (account.type === AccountType.MultiSubAccounts.type) {
        const subAccountCurrencies = account.getSubAccountCurrencies(showHidden.value, activeSubAccount.value[account.id])
            .map(currencyCode => getCurrencyName(currencyCode));
        return joinMultiText(subAccountCurrencies);
    } else {
        return null;
    }
}

function accountReconciliationStatementDateRanges(account: Account): LocalizedDateRange[] {
    return getAllDateRanges(DateRangeScene.Normal, true, !!accountsStore.getAccountStatementDate(account.id));
}

function addAccountForCategory(category: number): void {
    editDialog.value?.open({
        category
    }).then(result => {
        if (result && result.message) {
            snackbar.value?.showMessage(result.message);
        }
    }).catch(error => {
        if (error) {
            snackbar.value?.showError(error);
        }
    });
}

function addAccountForCurrentCategory(): void {
    addAccountForCategory(activeAccountCategoryType.value);
}

function edit(account: Account): void {
    editDialog.value?.open({
        id: account.id,
        currentAccount: account
    }).then(result => {
        if (result && result.message) {
            snackbar.value?.showMessage(result.message);
        }

        if (accountsStore.accountListStateInvalid && !loading.value) {
            reload(false);
        }
    }).catch(error => {
        if (error) {
            snackbar.value?.showError(error);
        }
    });
}

/** 中文说明：打开账户对账明细；自定义日期会先缓存账户上下文，再由日期弹窗回填范围。 */ function showReconciliationStatementCustomDateRangeDialog(account: Account, dateRangeType?: number): void {
    if (!isNumber(dateRangeType) || dateRangeType === DateRange.Custom.type) {
        accountToShowReconciliationStatement.value = account;
        showCustomDateRangeDialog.value = true;
        return;
    }

    let dateRange: TimeRangeAndDateType | null = null;

    if (DateRange.isBillingCycle(dateRangeType)) {
        dateRange = getDateRangeByBillingCycleDateType(dateRangeType, firstDayOfWeek.value, fiscalYearStart.value, accountsStore.getAccountStatementDate(account.id));
    } else {
        dateRange = getDateRangeByDateType(dateRangeType, firstDayOfWeek.value, fiscalYearStart.value);
    }

    if (!dateRange) {
        return;
    }

    reconciliationStatementDialog.value?.open({
        accountId: account.id,
        startTime: dateRange.minTime,
        endTime: dateRange.maxTime
    });
}

/** 中文说明：触发账户流水迁移弹窗，成功后按 store invalid 标记刷新账户列表和余额。 */ function moveAllTransactions(account: Account): void {
    moveAllTransactionsDialog.value?.open(account).then(() => {
        snackbar.value?.showMessage('All transactions in this account has been moved.');

        if (accountsStore.accountListStateInvalid && !loading.value) {
            reload(false);
        }
    });
}

/** 中文说明：触发账户流水清理弹窗，成功后按 store invalid 标记刷新账户列表和余额。 */ function clearAllTransactions(account: Account): void {
    clearAllTransactionsDialog.value?.open(account).then(() => {
        snackbar.value?.showMessage('All transactions in this account has been cleared');

        if (accountsStore.accountListStateInvalid && !loading.value) {
            reload(false);
        }
    });
}

/** 中文说明：切换账户或子账户隐藏状态；隐藏当前子账户时清空选中项以避免展示失效引用。 */ function hide(account: Account, targetAccount: Account, hidden: boolean): void {
    loading.value = true;

    accountsStore.hideAccount({
        account: targetAccount,
        hidden: hidden
    }).then(() => {
        if (hidden && !showHidden.value && activeSubAccount.value[account.id]) {
            activeSubAccount.value[account.id] = '';
        }

        loading.value = false;
    }).catch(error => {
        loading.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

/** 中文说明：删除账户或当前选中的子账户，父子账户删除路径在这里分流。 */ function remove(account: Account): void {
    if (activeSubAccount.value[account.id]) {
        const subAccount: Account | null = account.getSubAccount(activeSubAccount.value[account.id]);

        if (!subAccount) {
            snackbar.value?.showMessage('Unable to delete this sub-account');
            return;
        }

        confirmDialog.value?.open('Are you sure you want to delete this sub-account?').then(() => {
            loading.value = true;

            accountsStore.deleteSubAccount({
                subAccount: subAccount
            }).then(() => {
                activeSubAccount.value[account.id] = '';
                loading.value = false;
            }).catch(error => {
                loading.value = false;

                if (!error.processed) {
                    snackbar.value?.showError(error);
                }
            });
        });
    } else {
        confirmDialog.value?.open('Are you sure you want to delete this account?').then(() => {
            loading.value = true;

            accountsStore.deleteAccount({
                account: account
            }).then(() => {
                loading.value = false;
            }).catch(error => {
                loading.value = false;

                if (!error.processed) {
                    snackbar.value?.showError(error);
                }
            });
        });
    }
}

/** 中文说明：把拖拽后的账户顺序持久化到 store/API，未发生排序修改时不发请求。 */ function saveSortResult(): void {
    if (!displayOrderModified.value) {
        return;
    }

    loading.value = true;

    accountsStore.updateAccountDisplayOrders().then(() => {
        loading.value = false;
        displayOrderModified.value = false;
    }).catch(error => {
        loading.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

/** 中文说明：接收拖拽组件的移动事件并更新账户排序草稿，等待用户保存后再持久化。 */ function onMove(event: { moved: { element: { id: string }, oldIndex: number, newIndex: number } }): void {
    if (!event || !event.moved) {
        return;
    }

    const moveEvent = event.moved;

    if (!moveEvent.element || !moveEvent.element.id) {
        snackbar.value?.showMessage('Unable to move account');
        return;
    }

    accountsStore.changeAccountDisplayOrder({
        accountId: moveEvent.element.id,
        from: moveEvent.oldIndex,
        to: moveEvent.newIndex,
        updateListOrder: false,
        updateGlobalListOrder: true
    }).then(() => {
        displayOrderModified.value = true;
    }).catch(error => {
        snackbar.value?.showError(error);
    });
}

/** 中文说明：自定义对账日期范围回填后打开对账弹窗，并清理临时账户上下文。 */ function onCustomDateRangeChanged(minUnixTime: number, maxUnixTime: number): void {
    if (!accountToShowReconciliationStatement.value) {
        snackbar.value?.showMessage('An error occurred');
        return;
    }

    showCustomDateRangeDialog.value = false;

    reconciliationStatementDialog.value?.open({
        accountId: accountToShowReconciliationStatement.value.id,
        startTime: minUnixTime,
        endTime: maxUnixTime
    });

    accountToShowReconciliationStatement.value = null;
}

function onShowDateRangeError(message: string): void {
    snackbar.value?.showError(message);
}

watch(() => display.mdAndUp.value, (newValue) => {
    alwaysShowNav.value = newValue;

    if (!showNav.value) {
        showNav.value = newValue;
    }
});

onMounted(() => {
    reload(false);
});

useExternalTemplateBindings(ConfirmDialog, SnackBar, EditDialog, ReconciliationStatementDialog, MoveAllTransactionsDialog, ClearAllTransactionsDialog, AccountFilterSettingsCard, SettingsJsonImportExportButton, ref, computed, onMounted, useTemplateRef, watch, useDisplay, useI18n, useAccountListPageBase, useAccountsStore, DateRange, DateRangeScene, AccountType, AccountCategory, isNumber, getDateRangeByDateType, getDateRangeByBillingCycleDateType, mdiEyeOutline, mdiEyeOffOutline, mdiCalculatorVariantOutline, mdiRefresh, mdiSquareRounded, mdiMenu, mdiPencilOutline, mdiDotsHorizontalCircleOutline, mdiSwapHorizontal, mdiEraser, mdiDeleteOutline, mdiListBoxOutline, mdiInvoiceListOutline, mdiDrag, mdiDotsVertical, display, tt, getAllDateRanges, getCurrencyName, joinMultiText, loading, showHidden, displayOrderModified, showAccountBalance, firstDayOfWeek, fiscalYearStart, allAccounts, allCategorizedAccountsMap, allAccountCount, netAssets, totalAssets, totalLiabilities, accountCategoryTotalBalance, accountBalance, accountsStore, confirmDialog, snackbar, editDialog, reconciliationStatementDialog, moveAllTransactionsDialog, clearAllTransactionsDialog, activeAccountCategoryType, activeTab, activeSubAccount, accountToShowReconciliationStatement, alwaysShowNav, showNav, showAccountsIncludedInTotalDialog, showCustomDateRangeDialog, reloadRequestId, hasAnyVisibleAccount, activeAccountCategory, activeAccountCategoryTotalBalance, activeAccountCategoryVisibleAccountCount, reload, hasAccount, accountCurrency, accountReconciliationStatementDateRanges, addAccountForCategory, addAccountForCurrentCategory, edit, showReconciliationStatementCustomDateRangeDialog, moveAllTransactions, clearAllTransactions, hide, remove, saveSortResult, onMove, onCustomDateRangeChanged, onShowDateRangeError);
</script>

<style src="./list/ListPage.css"></style>
