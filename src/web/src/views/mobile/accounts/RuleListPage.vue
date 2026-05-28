<template>
    <f7-page :ptr="true" @ptr:refresh="reload">
        <f7-navbar>
            <f7-nav-left :back-link="tt('Back')"></f7-nav-left>
            <f7-nav-title :title="tt('Account Recognition Rules')"></f7-nav-title>
            <f7-nav-right>
                <f7-link icon-f7="plus" :class="{ disabled: accountOptions.length === 0 }" @click="openCreateSheet"></f7-link>
            </f7-nav-right>
        </f7-navbar>

        <f7-list strong inset dividers class="margin-vertical skeleton-text" v-if="loading">
            <f7-list-item title="Account Recognition Rules" after="0"></f7-list-item>
            <f7-list-item title="Rule Matching Expression" subtitle="OR={...}"></f7-list-item>
        </f7-list>

        <template v-else>
            <f7-block-title>{{ tt('Account Recognition Rules') }}</f7-block-title>
            <f7-list strong inset dividers class="margin-vertical" v-if="accountRules.length > 0">
                <f7-list-item
                    swipeout
                    :key="rule.id"
                    v-for="rule in orderedAccountRules"
                    :title="resolveAccountName(rule)"
                    :after="getRoleScopeLabel(rule.accountRoleScope)"
                    :subtitle="rule.ruleExpression"
                    :footer="formatRuleFooter(rule)"
                >
                    <f7-swipeout-actions right>
                        <f7-swipeout-button color="green" close :text="tt('Test')" @click="openTestSheet(rule)"></f7-swipeout-button>
                        <f7-swipeout-button color="orange" close :text="tt('Edit')" @click="openEditSheet(rule)"></f7-swipeout-button>
                        <f7-swipeout-button color="red" close @click="confirmDelete(rule)">
                            <f7-icon f7="trash"></f7-icon>
                        </f7-swipeout-button>
                    </f7-swipeout-actions>
                </f7-list-item>
            </f7-list>
            <f7-list strong inset dividers class="margin-vertical" v-else>
                <f7-list-item :title="tt('No account rules')"></f7-list-item>
            </f7-list>

        </template>

        <f7-sheet
            class="account-rule-edit-sheet"
            :opened="showEditSheet"
            swipe-to-close
            @sheet:closed="showEditSheet = false"
        >
            <f7-page-content>
                <f7-block-title>{{ editingRule ? tt('Edit Rule') : tt('Create Rule') }}</f7-block-title>
                <f7-list form strong inset dividers class="margin-vertical">
                    <f7-list-item
                        link="#"
                        no-chevron
                        class="list-item-with-header-and-title"
                        :class="{ disabled: isAccountLocked }"
                        :header="tt('Account')"
                        :title="selectedAccountName"
                        @click="showAccountPopup = !isAccountLocked"
                    >
                        <list-item-selection-popup
                            value-type="item"
                            key-field="id"
                            value-field="id"
                            title-field="name"
                            :title="tt('Account')"
                            :enable-filter="true"
                            :filter-placeholder="tt('Account')"
                            :filter-no-items-text="tt('No results')"
                            :items="accountOptions"
                            v-model:show="showAccountPopup"
                            v-model="ruleForm.accountId">
                        </list-item-selection-popup>
                    </f7-list-item>
                    <f7-list-input
                        type="text"
                        clear-button
                        :label="tt('Rule Name')"
                        :placeholder="autoRuleName"
                        v-model:value="ruleForm.name"
                    ></f7-list-input>
                    <f7-list-input
                        type="number"
                        :label="tt('Priority')"
                        :value="String(ruleForm.priority)"
                        @input="updatePriority"
                    ></f7-list-input>
                    <f7-list-item
                        link="#"
                        no-chevron
                        class="list-item-with-header-and-title"
                        :header="tt('Account Role')"
                        :title="getRoleScopeLabel(ruleForm.accountRoleScope)"
                        @click="showRoleScopePopup = true"
                    >
                        <list-item-selection-popup
                            value-type="item"
                            key-field="value"
                            value-field="value"
                            title-field="title"
                            :title="tt('Account Role')"
                            :items="roleScopeItems"
                            v-model:show="showRoleScopePopup"
                            v-model="ruleForm.accountRoleScope">
                        </list-item-selection-popup>
                    </f7-list-item>
                    <f7-list-item
                        link="#"
                        no-chevron
                        class="list-item-with-header-and-title"
                        :header="tt('Transaction Type')"
                        :title="getTransactionScopeLabel(ruleForm.transactionTypeScope)"
                        @click="showTransactionScopePopup = true"
                    >
                        <list-item-selection-popup
                            value-type="item"
                            key-field="value"
                            value-field="value"
                            title-field="title"
                            :title="tt('Transaction Type')"
                            :items="transactionScopeItems"
                            v-model:show="showTransactionScopePopup"
                            v-model="ruleForm.transactionTypeScope">
                        </list-item-selection-popup>
                    </f7-list-item>
                    <f7-list-input
                        type="textarea"
                        :label="tt('Rule Matching Expression')"
                        placeholder="OR={wechat,pay}"
                        v-model:value="ruleForm.ruleExpression"
                    ></f7-list-input>
                    <f7-list-item :title="tt('Enable Regex')">
                        <f7-toggle :checked="ruleForm.regexEnabled" @toggle:change="ruleForm.regexEnabled = $event"></f7-toggle>
                    </f7-list-item>
                    <f7-list-item :title="tt('Enabled')">
                        <f7-toggle :checked="ruleForm.enabled" @toggle:change="ruleForm.enabled = $event"></f7-toggle>
                    </f7-list-item>
                </f7-list>

                <f7-block-title>{{ tt('Field Scope') }}</f7-block-title>
                <f7-list strong inset dividers class="margin-vertical account-rule-field-list">
                    <f7-list-item
                        checkbox
                        :key="option.value"
                        v-for="option in fieldScopeItems"
                        :title="option.title"
                        :checked="ruleForm.fieldScope.includes(option.value)"
                        @change="toggleFieldScope(option.value, $event)"
                    ></f7-list-item>
                </f7-list>

                <f7-block class="grid grid-cols-2 grid-gap">
                    <f7-button large :disabled="saving" @click="showEditSheet = false">{{ tt('Cancel') }}</f7-button>
                    <f7-button large fill :preloader="saving" :loading="saving" @click="saveRule">{{ tt('Save') }}</f7-button>
                </f7-block>
            </f7-page-content>
        </f7-sheet>

        <f7-sheet
            class="account-rule-test-sheet"
            :opened="showTestSheet"
            swipe-to-close
            @sheet:closed="showTestSheet = false"
        >
            <f7-page-content>
                <f7-block-title>{{ tt('Test Rule') }}</f7-block-title>
                <f7-list form strong inset dividers class="margin-vertical">
                    <f7-list-input
                        type="textarea"
                        :label="tt('Text to test')"
                        :placeholder="tt('Enter parser, counterparty, payment method or description text')"
                        v-model:value="testText"
                    ></f7-list-input>
                    <f7-list-item
                        link="#"
                        no-chevron
                        class="list-item-with-header-and-title"
                        :header="tt('Account Role')"
                        :title="getRoleScopeLabel(testRoleScope)"
                        @click="showTestRoleScopePopup = true"
                    >
                        <list-item-selection-popup
                            value-type="item"
                            key-field="value"
                            value-field="value"
                            title-field="title"
                            :title="tt('Account Role')"
                            :items="roleScopeItems"
                            v-model:show="showTestRoleScopePopup"
                            v-model="testRoleScope">
                        </list-item-selection-popup>
                    </f7-list-item>
                    <f7-list-item
                        link="#"
                        no-chevron
                        class="list-item-with-header-and-title"
                        :header="tt('Transaction Type')"
                        :title="getTransactionScopeLabel(testTransactionScope)"
                        @click="showTestTransactionScopePopup = true"
                    >
                        <list-item-selection-popup
                            value-type="item"
                            key-field="value"
                            value-field="value"
                            title-field="title"
                            :title="tt('Transaction Type')"
                            :items="transactionScopeItems"
                            v-model:show="showTestTransactionScopePopup"
                            v-model="testTransactionScope">
                        </list-item-selection-popup>
                    </f7-list-item>
                    <f7-list-item v-if="testResult !== null" :title="testResult ? tt('Match!') : tt('No match')" />
                </f7-list>
                <f7-block class="grid grid-cols-2 grid-gap">
                    <f7-button large :disabled="testing" @click="showTestSheet = false">{{ tt('Close') }}</f7-button>
                    <f7-button large fill :preloader="testing" :loading="testing" @click="runTest">{{ tt('Test') }}</f7-button>
                </f7-block>
            </f7-page-content>
        </f7-sheet>

        <f7-actions close-by-outside-click close-on-escape :opened="showDeleteActionSheet" @actions:closed="showDeleteActionSheet = false">
            <f7-actions-group>
                <f7-actions-label>{{ tt('Are you sure you want to delete rule') }} "{{ deletingRule?.name }}"?</f7-actions-label>
                <f7-actions-button color="red" @click="deleteRule">{{ tt('Delete') }}</f7-actions-button>
            </f7-actions-group>
            <f7-actions-group>
                <f7-actions-button bold close>{{ tt('Cancel') }}</f7-actions-button>
            </f7-actions-group>
        </f7-actions>
    </f7-page>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import type { Router } from 'framework7/types';

import { useI18n } from '@/locales/helpers.ts';
import { useI18nUIComponents, showLoading, hideLoading } from '@/lib/ui/mobile.ts';
import services from '@/lib/services.ts';
import type { Account } from '@/models/account.ts';
import {
    ACCOUNT_RULE_FIELD_SCOPE_OPTIONS,
    ACCOUNT_RULE_ROLE_SCOPE_OPTIONS,
    ACCOUNT_RULE_TRANSACTION_SCOPE_OPTIONS,
    accountRuleToForm,
    buildAccountRulePayload,
    buildAccountRuleTestContext,
    createDefaultAccountRuleForm,
    getAccountRuleLabel,
    normalizeAccountRuleItem,
    type AccountRuleFieldScope,
    type AccountRuleForm,
    type AccountRuleItem,
    type AccountRuleRoleScope,
    type AccountRuleTestResult,
    type AccountRuleTransactionTypeScope,
} from '@/models/account_rule.ts';
import { useAccountsStore } from '@/stores/account.ts';

interface AccountRuleAccountOption {
    id: string;
    name: string;
}

const props = defineProps<{
    f7route: Router.Route;
    f7router: Router.Router;
}>();

const { tt } = useI18n();
const { showToast } = useI18nUIComponents();
const accountsStore = useAccountsStore();

const initialAccountId = normalizeRouteAccountId(props.f7route.query['accountId']);
const loading = ref(false);
const saving = ref(false);
const testing = ref(false);
const accountRules = ref<AccountRuleItem[]>([]);
const showEditSheet = ref(false);
const editingRule = ref<AccountRuleItem | null>(null);
const ruleForm = ref<AccountRuleForm>(createDefaultAccountRuleForm(initialAccountId));
const showAccountPopup = ref(false);
const showRoleScopePopup = ref(false);
const showTransactionScopePopup = ref(false);
const showTestSheet = ref(false);
const testRuleId = ref(0);
const testText = ref('');
const testRoleScope = ref<AccountRuleRoleScope>('any');
const testTransactionScope = ref<AccountRuleTransactionTypeScope>('all');
const testResult = ref<boolean | null>(null);
const showTestRoleScopePopup = ref(false);
const showTestTransactionScopePopup = ref(false);
const showDeleteActionSheet = ref(false);
const deletingRule = ref<AccountRuleItem | null>(null);

const isAccountLocked = computed(() => initialAccountId !== null);
const allAccounts = computed<Account[]>(() => accountsStore.allMixedPlainAccounts as Account[]);
const accountOptions = computed<AccountRuleAccountOption[]>(() => allAccounts.value.map(account => ({
    id: String(account.id),
    name: account.name,
})));
const selectedAccountName = computed(() => (
    accountOptions.value.find(option => option.id === ruleForm.value.accountId)?.name || tt('Account')
));
const autoRuleName = computed(() => `${selectedAccountName.value} · ${tt('Account Rule')}`);
const orderedAccountRules = computed(() => [...accountRules.value].sort((firstRule, secondRule) => (
    firstRule.priority - secondRule.priority
    || resolveAccountName(firstRule).localeCompare(resolveAccountName(secondRule), 'zh-Hans')
    || firstRule.id - secondRule.id
)));
const roleScopeItems = computed(() => ACCOUNT_RULE_ROLE_SCOPE_OPTIONS.map(option => ({
    title: tt(option.titleKey),
    value: option.value,
})));
const transactionScopeItems = computed(() => ACCOUNT_RULE_TRANSACTION_SCOPE_OPTIONS.map(option => ({
    title: tt(option.titleKey),
    value: option.value,
})));
const fieldScopeItems = computed(() => ACCOUNT_RULE_FIELD_SCOPE_OPTIONS.map(option => ({
    title: tt(option.titleKey),
    value: option.value,
})));

function normalizeRouteAccountId(value: unknown): number | null {
    const accountId = Number.parseInt(String(value ?? ''), 10);
    return Number.isFinite(accountId) && accountId > 0 ? accountId : null;
}

function requireApiSuccess<T>(response: { data?: { success?: boolean; result: T } }, fallback: string): T {
    if (response.data?.success) {
        return response.data.result;
    }

    throw new Error(fallback);
}

function getRequestErrorMessage(error: unknown, fallback: string): string {
    return error instanceof Error && error.message ? tt(error.message) : fallback;
}

function resolveAccount(rule: AccountRuleItem): Account | null {
    return allAccounts.value.find(account => String(account.id) === String(rule.accountId)) ?? null;
}

function resolveAccountName(rule: AccountRuleItem): string {
    return resolveAccount(rule)?.name || rule.accountName || tt('Account');
}

function getRoleScopeLabel(value: string): string {
    return getAccountRuleLabel(ACCOUNT_RULE_ROLE_SCOPE_OPTIONS, value, tt);
}

function getTransactionScopeLabel(value: string): string {
    return getAccountRuleLabel(ACCOUNT_RULE_TRANSACTION_SCOPE_OPTIONS, value, tt);
}

function getFieldScopeLabel(value: string): string {
    return getAccountRuleLabel(ACCOUNT_RULE_FIELD_SCOPE_OPTIONS, value, tt);
}

function formatRuleFooter(rule: AccountRuleItem): string {
    const fields = rule.fieldScope.slice(0, 3).map(getFieldScopeLabel).join(' / ');
    const typeScope = getTransactionScopeLabel(rule.transactionTypeScope);
    return `${typeScope} · ${fields} · ${tt('Matched')} ${rule.matchCount || rule.appliedCount}`;
}

function updatePriority(event: Event): void {
    const input = event.target as HTMLInputElement | null;
    const priority = Number(input?.value ?? 100);
    ruleForm.value.priority = Number.isFinite(priority) ? priority : 100;
}

function toggleFieldScope(field: AccountRuleFieldScope, event: Event): void {
    const input = event.target as HTMLInputElement | null;
    const nextFields = new Set(ruleForm.value.fieldScope);
    if (input?.checked) {
        nextFields.add(field);
    } else {
        nextFields.delete(field);
    }
    ruleForm.value.fieldScope = Array.from(nextFields);
}

function openCreateSheet(): void {
    editingRule.value = null;
    ruleForm.value = createDefaultAccountRuleForm(initialAccountId ?? accountOptions.value[0]?.id ?? '');
    showEditSheet.value = true;
}

function openEditSheet(rule: AccountRuleItem): void {
    editingRule.value = rule;
    ruleForm.value = accountRuleToForm(rule);
    showEditSheet.value = true;
}

function openTestSheet(rule: AccountRuleItem): void {
    testRuleId.value = rule.id;
    testText.value = '';
    testRoleScope.value = rule.accountRoleScope;
    testTransactionScope.value = rule.transactionTypeScope;
    testResult.value = null;
    showTestSheet.value = true;
}

function confirmDelete(rule: AccountRuleItem): void {
    deletingRule.value = rule;
    showDeleteActionSheet.value = true;
}

async function saveRule(): Promise<void> {
    saving.value = true;
    showLoading(() => saving.value);
    try {
        const payload = buildAccountRulePayload(ruleForm.value, autoRuleName.value);
        if (editingRule.value) {
            requireApiSuccess(
                await services.updateAccountRule(editingRule.value.id, payload),
                tt('Failed to save rule')
            );
            showToast('Rule updated');
        } else {
            requireApiSuccess(
                await services.createAccountRule(payload),
                tt('Failed to save rule')
            );
            showToast('Rule created');
        }
        showEditSheet.value = false;
        await loadRules();
    } catch (error: unknown) {
        showToast(getRequestErrorMessage(error, tt('Failed to save rule')));
    } finally {
        saving.value = false;
        hideLoading();
    }
}

async function runTest(): Promise<void> {
    if (!testText.value.trim()) {
        return;
    }

    testing.value = true;
    showLoading(() => testing.value);
    try {
        const result = requireApiSuccess<AccountRuleTestResult>(
            await services.testAccountRule(
                testRuleId.value,
                buildAccountRuleTestContext(testText.value, testRoleScope.value, testTransactionScope.value)
            ),
            tt('Test failed')
        );
        testResult.value = !!result?.matched;
    } catch (error: unknown) {
        testResult.value = null;
        showToast(getRequestErrorMessage(error, tt('Test failed')));
    } finally {
        testing.value = false;
        hideLoading();
    }
}

async function deleteRule(): Promise<void> {
    if (!deletingRule.value) {
        return;
    }

    const targetRule = deletingRule.value;
    showDeleteActionSheet.value = false;
    showLoading();
    try {
        requireApiSuccess(
            await services.deleteAccountRule(targetRule.id),
            tt('Failed to delete rule')
        );
        deletingRule.value = null;
        showToast('Rule deleted');
        await loadRules();
    } catch (error: unknown) {
        showToast(getRequestErrorMessage(error, tt('Failed to delete rule')));
    } finally {
        hideLoading();
    }
}

async function loadRules(): Promise<void> {
    const result = requireApiSuccess<Record<string, unknown>[]>(
        await services.getAccountRules(initialAccountId ?? undefined),
        tt('Failed to load account rules')
    );
    accountRules.value = (result ?? []).map(item => normalizeAccountRuleItem(item));
}

async function loadAll(): Promise<void> {
    loading.value = true;
    try {
        await Promise.all([
            accountsStore.loadAllAccounts({ force: false }),
            loadRules(),
        ]);
    } catch (error: unknown) {
        showToast(getRequestErrorMessage(error, tt('Failed to load account rules')));
    } finally {
        loading.value = false;
    }
}

function reload(done?: () => void): void {
    loadAll().finally(() => done?.());
}

void loadAll();
</script>

<style scoped>
.account-rule-edit-sheet,
.account-rule-test-sheet {
    height: min(86vh, 720px);
}

.account-rule-field-list {
    --f7-list-item-min-height: 42px;
}
</style>
