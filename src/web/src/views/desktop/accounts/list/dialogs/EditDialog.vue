<template src="./edit-dialog/EditDialog.template.html"></template>

<script setup lang="ts">
import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import CategoryRuleBuilderFields from '@/components/common/CategoryRuleBuilderFields.vue';

import { ref, computed, useTemplateRef, watch, onMounted, onUnmounted } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { useAccountEditPageBase } from '@/views/base/accounts/AccountEditPageBase.ts';

import { useUserStore } from '@/stores/user.ts';
import { useAccountsStore } from '@/stores/account.ts';

import { itemAndIndex } from '@/core/base.ts';
import { AccountType } from '@/core/account.ts';
import { ALL_ACCOUNT_ICONS } from '@/consts/icon.ts';
import { ALL_ACCOUNT_COLORS } from '@/consts/color.ts';
import { Account } from '@/models/account.ts';
import {
    accountRuleToForm,
    buildAccountRulePayload,
    createDefaultAccountRuleForm,
    normalizeAccountRuleItem,
    type AccountRuleForm,
    type AccountRuleItem,
} from '@/models/account_rule.ts';

import { isNumber } from '@/lib/common.ts';
import { getCurrentUnixTime } from '@/lib/datetime.ts';
import { generateRandomUUID } from '@/lib/misc.ts';
import services from '@/lib/services.ts';

import {
    mdiDotsVertical,
    mdiCreditCardPlusOutline,
    mdiDeleteOutline
} from '@mdi/js';

interface AccountEditResponse {
    message: string;
    id?: string;
    account?: Account;
}

interface AccountRuleBuilderModel {
    priority: number;
    ruleExpression: string;
    regexEnabled: boolean;
    enabled: boolean;
}

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();
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
    isNewAccount,
    addSubAccount,
    setAccount
} = useAccountEditPageBase();

const userStore = useUserStore();
const accountsStore = useAccountsStore();

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');

const showState = ref<boolean>(false);
const activeTab = ref<string>('account');
const currentAccountIndex = ref<number>(-1);
const accountRuleLoading = ref<boolean>(false);
const accountRuleLoadFailed = ref<boolean>(false);
const primaryAccountRuleId = ref<number | null>(null);
const selectedAccountRuleDraft = ref<AccountRuleForm>(createDefaultAccountRuleForm());

let accountRuleLoadRequestId = 0;

const selectedAccount = computed<Account>(() => {
    if (currentAccountIndex.value < 0) {
        return account.value;
    }

    return subAccounts.value[currentAccountIndex.value] as Account;
});

const selectedAccountRuleAccountId = computed<number | null>(() => {
    const accountId = Number.parseInt(String(selectedAccount.value.id || ''), 10);
    return Number.isFinite(accountId) && accountId > 0 ? accountId : null;
});
const canManageSelectedAccountRules = computed<boolean>(() => (
    !!editAccountId.value
    && !isNewAccount(selectedAccount.value)
    && selectedAccountRuleAccountId.value !== null
));
const autoSelectedAccountRuleName = computed<string>(() => {
    const accountName = (selectedAccount.value.name || '').trim() || tt('Account');
    return `${accountName} - ${tt('Account Rule')}`;
});
const selectedAccountRuleBuilderModel = computed<AccountRuleBuilderModel>({
    get: () => ({
        priority: selectedAccountRuleDraft.value.priority,
        ruleExpression: selectedAccountRuleDraft.value.ruleExpression,
        regexEnabled: selectedAccountRuleDraft.value.regexEnabled,
        enabled: selectedAccountRuleDraft.value.enabled,
    }),
    set: (value) => {
        const parsedPriority = Number(value.priority ?? 100);

        selectedAccountRuleDraft.value = {
            ...selectedAccountRuleDraft.value,
            priority: Number.isFinite(parsedPriority) ? parsedPriority : 100,
            ruleExpression: String(value.ruleExpression ?? ''),
            regexEnabled: !!value.regexEnabled,
            enabled: value.enabled !== false,
        };
    },
});

const accountAmountTitle = computed<string>(() => {
    if (currentAccountIndex.value < 0) {
        return account.value.isLiability ? tt('Account Outstanding Balance') : tt('Account Balance');
    } else {
        return account.value.isLiability ? tt('Sub-account Outstanding Balance') : tt('Sub-account Balance');
    }
});

const isAccountModified = computed<boolean>(() => {
    if (!editAccountId.value) {
        return !account.value.equals(Account.createNewAccount(userStore.currentUserDefaultCurrency, account.value.balanceTime ?? getCurrentUnixTime()));
    } else {
        return true;
    }
});

let resolveFunc: ((value: AccountEditResponse) => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

function resetSelectedAccountRuleEditor(accountId: number | null = selectedAccountRuleAccountId.value): void {
    accountRuleLoadRequestId += 1;
    accountRuleLoading.value = false;
    accountRuleLoadFailed.value = false;
    primaryAccountRuleId.value = null;
    selectedAccountRuleDraft.value = createDefaultAccountRuleForm(accountId ?? '');
}

function requireApiSuccess<T>(response: { data?: { success?: boolean; result: T } }, fallback: string): T {
    if (response.data?.success) {
        return response.data.result;
    }

    throw new Error(fallback);
}

function getRequestErrorMessage(err: unknown, fallback: string): string {
    if (err instanceof Error && err.message) {
        return tt(err.message);
    }

    if (err && typeof err === 'object' && 'message' in err && typeof err.message === 'string') {
        return tt(err.message);
    }

    return fallback;
}

function isProcessedError(err: unknown): boolean {
    return !!(
        err
        && typeof err === 'object'
        && 'processed' in err
        && err.processed === true
    );
}

function getSortedAccountRules(rules: AccountRuleItem[]): AccountRuleItem[] {
    return [...rules].sort((firstRule, secondRule) => (
        firstRule.priority - secondRule.priority
        || firstRule.id - secondRule.id
    ));
}

async function loadSelectedAccountRule(accountId: number | null = selectedAccountRuleAccountId.value): Promise<void> {
    if (!showState.value || !canManageSelectedAccountRules.value || accountId === null) {
        resetSelectedAccountRuleEditor(accountId);
        return;
    }

    const requestId = ++accountRuleLoadRequestId;
    accountRuleLoading.value = true;
    accountRuleLoadFailed.value = false;
    selectedAccountRuleDraft.value = {
        ...selectedAccountRuleDraft.value,
        accountId: String(accountId),
    };

    try {
        const result = requireApiSuccess<Record<string, unknown>[]>(
            await services.getAccountRules(accountId),
            tt('Failed to load account rules')
        );

        if (requestId !== accountRuleLoadRequestId || accountId !== selectedAccountRuleAccountId.value) {
            return;
        }

        const rules = getSortedAccountRules((result ?? []).map(item => normalizeAccountRuleItem(item)));
        const primaryRule = rules[0] ?? null;

        primaryAccountRuleId.value = primaryRule?.id ?? null;
        selectedAccountRuleDraft.value = primaryRule
            ? {
                ...accountRuleToForm(primaryRule),
                accountId: String(accountId),
            }
            : createDefaultAccountRuleForm(accountId);
    } catch (err: unknown) {
        if (requestId !== accountRuleLoadRequestId) {
            return;
        }

        primaryAccountRuleId.value = null;
        selectedAccountRuleDraft.value = createDefaultAccountRuleForm(accountId);
        accountRuleLoadFailed.value = true;
        snackbar.value?.showError(getRequestErrorMessage(err, tt('Failed to load account rules')));
    } finally {
        if (requestId === accountRuleLoadRequestId) {
            accountRuleLoading.value = false;
        }
    }
}

function buildSelectedAccountRulePayload(accountId: number): ReturnType<typeof buildAccountRulePayload> | null {
    const ruleExpression = selectedAccountRuleDraft.value.ruleExpression.trim();

    if (!ruleExpression) {
        return null;
    }

    return buildAccountRulePayload({
        ...selectedAccountRuleDraft.value,
        accountId: String(accountId),
    }, autoSelectedAccountRuleName.value);
}

async function syncSelectedAccountRule(accountId: number): Promise<void> {
    const payload = buildSelectedAccountRulePayload(accountId);

    if (!payload) {
        if (primaryAccountRuleId.value !== null) {
            requireApiSuccess(
                await services.deleteAccountRule(primaryAccountRuleId.value),
                tt('Failed to delete rule')
            );
            primaryAccountRuleId.value = null;
        }
        return;
    }

    if (primaryAccountRuleId.value !== null) {
        requireApiSuccess(
            await services.updateAccountRule(primaryAccountRuleId.value, payload),
            tt('Failed to save rule')
        );
        return;
    }

    const createdRule = requireApiSuccess<Partial<AccountRuleItem>>(
        await services.createAccountRule(payload),
        tt('Failed to save rule')
    );

    if (createdRule.id !== undefined && createdRule.id !== null) {
        primaryAccountRuleId.value = Number(createdRule.id);
    }
}

function open(options?: { id?: string, currentAccount?: Account, category?: number }): Promise<AccountEditResponse> {
    showState.value = true;
    loading.value = true;
    submitting.value = false;
    resetSelectedAccountRuleEditor();

    const newAccount = Account.createNewAccount(userStore.currentUserDefaultCurrency, getCurrentUnixTime());
    account.value.fillFrom(newAccount);
    subAccounts.value = [];
    currentAccountIndex.value = -1;
    clientSessionId.value = generateRandomUUID();

    if (options && options.id) {
        if (options.currentAccount) {
            setAccount(options.currentAccount);
        }

        editAccountId.value = options.id;
        accountsStore.getAccount({
            accountId: editAccountId.value
        }).then(response => {
            setAccount(response);
            loading.value = false;
        }).catch(error => {
            loading.value = false;
            showState.value = false;

            if (!error.processed) {
                if (rejectFunc) {
                    rejectFunc(error);
                }
            }
        });
    } else {
        if (options && isNumber(options.category)) {
            account.value.category = options.category;
            account.value.setSuitableIcon(1, options.category);
        }

        editAccountId.value = null;
        loading.value = false;
    }

    return new Promise<AccountEditResponse>((resolve, reject) => {
        resolveFunc = resolve;
        rejectFunc = reject;
    });
}

async function save(): Promise<void> {
    const problemMessage = inputEmptyProblemMessage.value;

    if (problemMessage) {
        snackbar.value?.showMessage(problemMessage);
        return;
    }

    submitting.value = true;

    const wasEdit = !!editAccountId.value;
    const selectedRuleAccountId = selectedAccountRuleAccountId.value;
    const canSyncSelectedRule = (
        canManageSelectedAccountRules.value
        && !accountRuleLoadFailed.value
        && selectedRuleAccountId !== null
    );

    try {
        const savedAccount = await accountsStore.saveAccount({
            account: account.value,
            subAccounts: subAccounts.value,
            isEdit: wasEdit,
            clientSessionId: clientSessionId.value
        });

        let message = 'You have saved this account';

        if (!wasEdit) {
            message = 'You have added a new account';
        }

        if (canSyncSelectedRule) {
            await syncSelectedAccountRule(selectedRuleAccountId);
        }

        resolveFunc?.({ message, id: savedAccount.id, account: savedAccount });
        showState.value = false;
    } catch (error: unknown) {
        if (!isProcessedError(error)) {
            snackbar.value?.showError(getRequestErrorMessage(error, tt('Unable to save account')));
        }
    } finally {
        submitting.value = false;
    }
}

function removeSubAccount(currentSubAccount: Account): void {
    confirmDialog.value?.open('Are you sure you want to remove this sub-account?').then(() => {
        for (const [subAccount, index] of itemAndIndex(subAccounts.value)) {
            if (subAccount === currentSubAccount) {
                subAccounts.value.splice(index, 1);

                if (currentAccountIndex.value >= subAccounts.value.length) {
                    currentAccountIndex.value = subAccounts.value.length - 1;
                }
            }
        }
    });
}

function cancel(): void {
    rejectFunc?.();
    showState.value = false;
}

function onShowDateTimeError(error: string): void {
    snackbar.value?.showError(error);
}

watch(() => account.value.type, () => {
    if (subAccounts.value.length < 1) {
        addSubAccount();
    }
});

watch(
    () => [showState.value, selectedAccountRuleAccountId.value, canManageSelectedAccountRules.value] as const,
    ([visible, accountId, canManage]) => {
        if (!visible || !canManage || accountId === null) {
            resetSelectedAccountRuleEditor(accountId);
            return;
        }

        void loadSelectedAccountRule(accountId);
    }
);

function onKeydown(e: KeyboardEvent): void {
    if (!showState.value) {
        return;
    }

    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
    }

    if (e.key === 'Enter') {
        save();
        e.preventDefault();
    } else if (e.key === 'Backspace') {
        cancel();
        e.preventDefault();
    }
}

onMounted(() => {
    window.addEventListener('keydown', onKeydown);
});

onUnmounted(() => {
    window.removeEventListener('keydown', onKeydown);
});

defineExpose({
    open
});

useExternalTemplateBindings(ConfirmDialog, SnackBar, CategoryRuleBuilderFields, ref, computed, useTemplateRef, watch, onMounted, onUnmounted, useI18n, useAccountEditPageBase, useUserStore, useAccountsStore, itemAndIndex, AccountType, ALL_ACCOUNT_ICONS, ALL_ACCOUNT_COLORS, Account, accountRuleToForm, buildAccountRulePayload, createDefaultAccountRuleForm, normalizeAccountRuleItem, isNumber, getCurrentUnixTime, generateRandomUUID, services, mdiDotsVertical, mdiCreditCardPlusOutline, mdiDeleteOutline, tt, editAccountId, clientSessionId, loading, submitting, account, subAccounts, title, saveButtonTitle, inputEmptyProblemMessage, inputIsEmpty, allAccountCategories, allAccountTypes, allAvailableMonthDays, isAccountSupportCreditCardStatementDate, isNewAccount, addSubAccount, setAccount, userStore, accountsStore, confirmDialog, snackbar, showState, activeTab, currentAccountIndex, accountRuleLoading, accountRuleLoadFailed, primaryAccountRuleId, selectedAccountRuleDraft, accountRuleLoadRequestId, selectedAccount, selectedAccountRuleAccountId, canManageSelectedAccountRules, autoSelectedAccountRuleName, selectedAccountRuleBuilderModel, accountAmountTitle, isAccountModified, resolveFunc, rejectFunc, resetSelectedAccountRuleEditor, requireApiSuccess, getRequestErrorMessage, isProcessedError, getSortedAccountRules, loadSelectedAccountRule, buildSelectedAccountRulePayload, syncSelectedAccountRule, open, save, removeSubAccount, cancel, onShowDateTimeError, onKeydown);
</script>

<style scoped src="./edit-dialog/EditDialog.css"></style>
