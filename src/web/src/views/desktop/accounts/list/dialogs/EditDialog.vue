<template>
    <v-dialog
        class="account-edit-dialog"
        width="calc(100vw - 32px)"
        :max-width="account.type === AccountType.MultiSubAccounts.type ? 1000 : 800"
        :persistent="isAccountModified"
        v-model="showState"
    >
        <v-card class="account-edit-card pa-2 pa-sm-4 pa-md-8">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center">
                        <h4 class="text-h4">{{ tt(title) }}</h4>
                        <v-progress-circular indeterminate size="22" class="ms-2" v-if="loading"></v-progress-circular>
                    </div>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2" :icon="true"
                           :disabled="loading || submitting || account.type !== AccountType.MultiSubAccounts.type">
                        <v-icon :icon="mdiDotsVertical" />
                        <v-menu activator="parent">
                            <v-list>
                                <v-list-item :prepend-icon="mdiCreditCardPlusOutline"
                                             :title="tt('Add Sub-account')"
                                             @click="addSubAccount"></v-list-item>
                            </v-list>
                        </v-menu>
                    </v-btn>
                </div>
            </template>
            <v-card-text class="account-edit-content d-flex flex-column flex-md-row mt-md-4 pt-0">
                <div class="account-edit-tabs mb-4" v-if="account.type === AccountType.MultiSubAccounts.type">
                    <v-tabs direction="vertical" :disabled="loading || submitting" v-model="currentAccountIndex">
                        <v-tab :value="-1">
                            <span>{{ tt('Main Account') }}</span>
                        </v-tab>
                        <template v-if="account.type === AccountType.MultiSubAccounts.type">
                            <v-tab :key="idx" :value="idx" v-for="(subAccount, idx) in subAccounts">
                                <span>{{ tt('Sub Account') + ' #' + (idx + 1) }}</span>
                                <v-btn class="ms-2" color="error" size="24" variant="text"
                                       :icon="mdiDeleteOutline"
                                       @click="removeSubAccount(subAccount)"></v-btn>
                            </v-tab>
                        </template>
                    </v-tabs>
                </div>

                <v-window class="account-edit-window d-flex flex-grow-1 disable-tab-transition w-100-window-container"
                          :class="{ 'ms-md-5': account.type === AccountType.MultiSubAccounts.type }"
                          v-model="activeTab">
                    <v-window-item value="account">
                        <v-form class="mt-2">
                            <v-row>
                                <v-col cols="12" md="12" v-if="account.type === AccountType.SingleAccount.type || currentAccountIndex < 0">
                                    <v-select
                                        item-title="displayName"
                                        item-value="type"
                                        persistent-placeholder
                                        :disabled="loading || submitting"
                                        :label="tt('Account Category')"
                                        :placeholder="tt('Account Category')"
                                        :items="allAccountCategories"
                                        :no-data-text="tt('No results')"
                                        v-model="selectedAccount.category"
                                    >
                                        <template #item="{ props, item }">
                                            <v-list-item :value="item.value" v-bind="props">
                                                <template #title>
                                                    <v-list-item-title>
                                                        <div class="d-flex align-center">
                                                            <ItemIcon icon-type="account"
                                                                      :icon-id="item.raw.defaultAccountIconId"
                                                                      v-if="item.raw" />
                                                            <span class="ms-2">{{ item.title }}</span>
                                                        </div>
                                                    </v-list-item-title>
                                                </template>
                                            </v-list-item>
                                        </template>
                                    </v-select>
                                </v-col>
                                <v-col cols="12" md="12" v-if="account.type === AccountType.SingleAccount.type || currentAccountIndex < 0">
                                    <v-select
                                        item-title="displayName"
                                        item-value="type"
                                        persistent-placeholder
                                        :disabled="loading || submitting || !!editAccountId"
                                        :label="tt('Account Type')"
                                        :placeholder="tt('Account Type')"
                                        :items="allAccountTypes"
                                        :no-data-text="tt('No results')"
                                        v-model="selectedAccount.type"
                                    />
                                </v-col>
                                <v-col cols="12" md="12">
                                    <v-text-field
                                        type="text"
                                        persistent-placeholder
                                        :disabled="loading || submitting"
                                        :label="currentAccountIndex < 0 ? tt('Account Name') : tt('Sub-account Name')"
                                        :placeholder="currentAccountIndex < 0 ? tt('Your account name') : tt('Your sub-account name')"
                                        v-model="selectedAccount.name"
                                    />
                                </v-col>
                                <v-col cols="12" md="6">
                                    <icon-select icon-type="account"
                                                 :all-icon-infos="ALL_ACCOUNT_ICONS"
                                                 :label="currentAccountIndex < 0 ? tt('Account Icon') : tt('Sub-account Icon')"
                                                 :color="selectedAccount.color"
                                                 :disabled="loading || submitting"
                                                 v-model="selectedAccount.icon" />
                                </v-col>
                                <v-col cols="12" md="6">
                                    <color-select :all-color-infos="ALL_ACCOUNT_COLORS"
                                                  :label="currentAccountIndex < 0 ? tt('Account Color') : tt('Sub-account Color')"
                                                  :disabled="loading || submitting"
                                                  v-model="selectedAccount.color" />
                                </v-col>
                                <v-col cols="12" :md="currentAccountIndex < 0 && isAccountSupportCreditCardStatementDate ? 6 : 12" v-if="account.type === AccountType.SingleAccount.type || currentAccountIndex >= 0">
                                    <currency-select :disabled="loading || submitting || (!!editAccountId && !isNewAccount(selectedAccount))"
                                                     :label="tt('Currency')"
                                                     :placeholder="tt('Currency')"
                                                     v-model="selectedAccount.currency" />
                                </v-col>
                                <v-col cols="12" :md="account.type === AccountType.SingleAccount.type || currentAccountIndex >= 0 ? 6 : 12" v-if="currentAccountIndex < 0 && isAccountSupportCreditCardStatementDate">
                                    <v-autocomplete
                                        item-title="displayName"
                                        item-value="day"
                                        auto-select-first
                                        persistent-placeholder
                                        :disabled="loading || submitting"
                                        :label="tt('Statement Date')"
                                        :placeholder="tt('Statement Date')"
                                        :items="allAvailableMonthDays"
                                        :no-data-text="tt('No results')"
                                        v-model="account.creditCardStatementDate"
                                    ></v-autocomplete>
                                </v-col>
                                <v-col cols="12" :md="(!editAccountId || isNewAccount(selectedAccount)) && selectedAccount.balanceCents ? 6 : 12"
                                       v-if="account.type === AccountType.SingleAccount.type || currentAccountIndex >= 0">
                                    <amount-input :disabled="loading || submitting || (!!editAccountId && !isNewAccount(selectedAccount))"
                                                  :persistent-placeholder="true"
                                                  :currency="selectedAccount.currency"
                                                  :show-currency="true"
                                                  :flip-negative="account.isLiability"
                                                  :label="accountAmountTitle"
                                                  :placeholder="accountAmountTitle"
                                                  v-model="selectedAccount.balanceCents"/>
                                </v-col>
                                <v-col cols="12" md="6" v-show="selectedAccount.balanceCents"
                                       v-if="(!editAccountId || isNewAccount(selectedAccount)) && (account.type === AccountType.SingleAccount.type || currentAccountIndex >= 0)">
                                    <date-time-select
                                        :disabled="loading || submitting"
                                        :label="tt('Balance Time')"
                                        v-model="selectedAccount.balanceTime"
                                        @error="onShowDateTimeError" />
                                </v-col>
                                <v-col
                                    v-if="canManageSelectedAccountRules"
                                    cols="12"
                                    md="12"
                                    class="account-rule-section"
                                >
                                    <v-progress-linear
                                        v-if="accountRuleLoading"
                                        indeterminate
                                        color="primary"
                                        class="mb-3"
                                    />
                                    <v-alert
                                        v-if="accountRuleLoadFailed"
                                        type="warning"
                                        variant="tonal"
                                        density="compact"
                                        class="mb-3"
                                    >
                                        {{ tt('Failed to load account rules') }}
                                    </v-alert>
                                    <category-rule-builder-fields
                                        v-model="selectedAccountRuleBuilderModel"
                                        :auto-rule-name="autoSelectedAccountRuleName"
                                        :disabled="loading || submitting || accountRuleLoading || accountRuleLoadFailed"
                                        title="Account Matching"
                                    />
                                </v-col>
                                <v-col cols="12" md="12">
                                    <v-textarea
                                        type="text"
                                        persistent-placeholder
                                        rows="3"
                                        :disabled="loading || submitting"
                                        :label="tt('Description')"
                                        :placeholder="currentAccountIndex < 0 ? tt('Your account description (optional)') : tt('Your sub-account description (optional)')"
                                        v-model="selectedAccount.comment"
                                    />
                                </v-col>
                                <v-col class="py-0" cols="12" md="12" v-if="editAccountId && !isNewAccount(selectedAccount)">
                                    <v-switch :disabled="loading || submitting"
                                              :label="tt('Visible')" v-model="selectedAccount.visible"/>
                                </v-col>
                            </v-row>
                        </v-form>
                    </v-window-item>
                </v-window>
            </v-card-text>
            <v-card-text class="overflow-y-visible">
                <div class="w-100 d-flex justify-center mt-2 mt-sm-4 mt-md-6 gap-4">
                    <v-tooltip :disabled="!inputIsEmpty" :text="inputEmptyProblemMessage ? tt(inputEmptyProblemMessage) : ''">
                        <template v-slot:activator="{ props }">
                            <div v-bind="props" class="d-inline-block">
                                <v-btn :disabled="inputIsEmpty || loading || submitting" @click="save">
                                    {{ tt(saveButtonTitle) }}
                                    <v-progress-circular indeterminate size="22" class="ms-2" v-if="submitting"></v-progress-circular>
                                </v-btn>
                            </div>
                        </template>
                    </v-tooltip>
                    <v-btn color="secondary" variant="tonal"
                           :disabled="loading || submitting" @click="cancel">{{ tt('Cancel') }}</v-btn>
                </div>
            </v-card-text>
        </v-card>
    </v-dialog>

    <confirm-dialog ref="confirmDialog"/>
    <snack-bar ref="snackbar" />
</template>

<script setup lang="ts">
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
</script>

<style scoped>
.account-edit-card,
.account-edit-content,
.account-edit-window {
    min-width: 0;
    max-width: 100%;
}

.account-edit-card {
    overflow: hidden;
}

.account-edit-content {
    overflow-x: hidden;
}

.account-edit-tabs {
    flex: 0 0 180px;
    min-width: 0;
    max-width: 100%;
}

.account-rule-section {
    min-width: 0;
    max-width: 100%;
    overflow-x: hidden;
}

.account-rule-section :deep(.category-rule-builder),
.account-rule-section :deep(.category-rule-builder__content),
.account-rule-section :deep(.rule-expression-input),
.account-rule-section :deep(.rule-expression-groups) {
    min-width: 0;
    max-width: 100%;
}

@media (max-width: 960px) {
    .account-edit-tabs {
        flex-basis: auto;
        width: 100%;
    }
}
</style>
