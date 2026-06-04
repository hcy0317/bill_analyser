<template>
    <v-row class="match-height account-recognition-rule-panel">
        <v-col cols="12">
            <v-card :variant="props.embedded ? 'flat' : undefined">
                <v-card-title v-if="!props.hideHeader" class="d-flex align-center">
                    <v-icon :icon="mdiBookAccountOutline" class="me-2" />
                    <span>{{ title || tt('Account Recognition Rules') }}</span>
                    <v-spacer />
                    <v-btn variant="outlined" :disabled="loading" @click="fetchAll">
                        <v-icon start :icon="mdiRefresh" />
                        {{ tt('Refresh') }}
                    </v-btn>
                </v-card-title>

                <v-progress-linear v-if="loading" indeterminate color="primary" />

                <v-alert v-if="error" type="error" closable class="ma-4" @click:close="error = null">
                    {{ error }}
                </v-alert>

                <Teleport :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">
                    <div
                        class="account-rule-table-toolbar d-flex flex-column flex-lg-row align-lg-center ga-3"
                        :class="{
                            'account-rule-table-toolbar--local pa-4': !hasHeaderActionsTarget,
                            'account-rule-table-toolbar--external': hasHeaderActionsTarget,
                        }"
                    >
                        <div class="account-rule-section-actions d-flex align-center flex-wrap ga-2">
                            <div v-if="!hasHeaderActionsTarget" class="account-rule-section-title d-flex align-center ga-2">
                                <span class="font-weight-medium">{{ tt('Account Recognition Rules') }}</span>
                                <v-chip size="x-small" variant="tonal">{{ accountRules.length }}</v-chip>
                            </div>
                            <v-btn
                                color="primary"
                                :disabled="loading || accountOptions.length === 0"
                                @click="openCreateDialog"
                            >
                                <v-icon start :icon="mdiPlus" />
                                {{ tt('Add Rule') }}
                            </v-btn>
                            <settings-json-import-export-button
                                v-if="props.showSettingsBundleControls"
                                section-key="accountRecognitionRules"
                                filename-prefix="account-recognition-rules"
                                :disabled="loading"
                                @imported="fetchAll"
                            />
                            <v-btn
                                color="default"
                                variant="text"
                                density="compact"
                                size="24"
                                :icon="true"
                                :loading="loading"
                                :disabled="loading"
                                @click="fetchAll"
                            >
                                <template #loader>
                                    <v-progress-circular indeterminate size="20" />
                                </template>
                                <v-icon :icon="mdiRefresh" size="24" />
                                <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                            </v-btn>
                        </div>
                    </div>
                </Teleport>

                <v-table class="account-rule-table" density="compact" hover>
                    <thead>
                    <tr>
                        <th class="account-rule-column-account">
                            <div class="account-rule-header-cell">{{ tt('Account') }}</div>
                        </th>
                        <th class="account-rule-column-expression">
                            <div class="account-rule-header-cell">{{ tt('Rule Matching Expression') }}</div>
                        </th>
                        <th class="account-rule-column-scope">
                            <div class="account-rule-header-cell">{{ tt('Account Role') }}</div>
                        </th>
                        <th class="account-rule-column-scope">
                            <div class="account-rule-header-cell">{{ tt('Transaction Type') }}</div>
                        </th>
                        <th class="account-rule-column-fields">
                            <div class="account-rule-header-cell">{{ tt('Field Scope') }}</div>
                        </th>
                        <th class="account-rule-column-regex text-no-wrap">
                            <div class="account-rule-header-cell">{{ tt('Use Regex') }}</div>
                        </th>
                        <th class="account-rule-column-enabled text-no-wrap">
                            <div class="account-rule-header-cell">{{ tt('Enabled') }}</div>
                        </th>
                        <th class="account-rule-column-applied text-no-wrap">
                            <div class="account-rule-header-cell">{{ tt('Matched') }}</div>
                        </th>
                        <th class="account-rule-column-actions text-no-wrap">
                            <div class="account-rule-header-cell">{{ tt('Actions') }}</div>
                        </th>
                    </tr>
                    </thead>
                    <tbody v-if="accountRules.length > 0">
                    <tr v-for="(item, index) in orderedAccountRules" :key="item.id">
                        <td class="account-rule-column-account">
                            <div class="d-flex align-center" :title="resolveAccountName(item)">
                                <ItemIcon
                                    icon-type="account"
                                    size="24px"
                                    :icon-id="resolveAccountIcon(item)"
                                    :color="resolveAccountColor(item)"
                                />
                                <span class="ms-2 text-truncate">{{ resolveAccountName(item) }}</span>
                            </div>
                        </td>
                        <td class="account-rule-column-expression">
                            <rule-expression-display
                                :rule-id="item.id"
                                :groups="getRuleExpressionGroups(item)"
                                :expanded-clause-keys="expandedExpressionClauseKeys"
                                :max-collapsed-terms="4"
                                @update:expanded-clause-keys="expandedExpressionClauseKeys = $event"
                            />
                        </td>
                        <td class="account-rule-column-scope">
                            <v-chip size="x-small" variant="tonal" color="primary">
                                {{ getRoleScopeLabel(item.accountRoleScope) }}
                            </v-chip>
                        </td>
                        <td class="account-rule-column-scope">
                            <v-chip size="x-small" variant="tonal">
                                {{ getTransactionScopeLabel(item.transactionTypeScope) }}
                            </v-chip>
                        </td>
                        <td class="account-rule-column-fields">
                            <div class="account-rule-field-chips">
                                <v-chip
                                    v-for="field in getVisibleFieldScope(item)"
                                    :key="field"
                                    size="x-small"
                                    variant="tonal"
                                >
                                    {{ getFieldScopeLabel(field) }}
                                </v-chip>
                                <v-chip
                                    v-if="item.fieldScope.length > maxVisibleFieldChips"
                                    size="x-small"
                                    variant="outlined"
                                >
                                    +{{ item.fieldScope.length - maxVisibleFieldChips }}
                                </v-chip>
                            </div>
                        </td>
                        <td class="account-rule-column-regex">
                            <v-icon
                                :icon="item.regexEnabled ? mdiCheckCircle : mdiCloseCircle"
                                :color="item.regexEnabled ? 'info' : 'grey'"
                                size="small"
                            />
                        </td>
                        <td class="account-rule-column-enabled">
                            <div class="d-inline-flex" @click.stop>
                                <v-switch
                                    :model-value="item.enabled"
                                    density="compact"
                                    hide-details
                                    color="success"
                                    :disabled="isRuleToggling(item.id)"
                                    @update:model-value="toggleEnabled(item, $event)"
                                />
                            </div>
                        </td>
                        <td class="account-rule-column-applied">{{ item.matchCount || item.appliedCount }}</td>
                        <td class="account-rule-column-actions">
                            <div class="account-rule-actions-row">
                                <v-tooltip :text="tt('Move Up')" location="top">
                                    <template #activator="{ props }">
                                        <v-btn
                                            v-bind="props"
                                            icon
                                            variant="text"
                                            size="small"
                                            :disabled="index === 0 || reordering"
                                            @click="moveRule(item, -1)"
                                        >
                                            <v-icon :icon="mdiArrowUp" size="small" />
                                        </v-btn>
                                    </template>
                                </v-tooltip>
                                <v-tooltip :text="tt('Move Down')" location="top">
                                    <template #activator="{ props }">
                                        <v-btn
                                            v-bind="props"
                                            icon
                                            variant="text"
                                            size="small"
                                            :disabled="index >= orderedAccountRules.length - 1 || reordering"
                                            @click="moveRule(item, 1)"
                                        >
                                            <v-icon :icon="mdiArrowDown" size="small" />
                                        </v-btn>
                                    </template>
                                </v-tooltip>
                                <v-tooltip :text="tt('Test')" location="top">
                                    <template #activator="{ props }">
                                        <v-btn
                                            v-bind="props"
                                            icon
                                            variant="text"
                                            size="small"
                                            color="success"
                                            @click="openTestDialog(item)"
                                        >
                                            <v-icon :icon="mdiTestTube" size="small" />
                                        </v-btn>
                                    </template>
                                </v-tooltip>
                                <v-tooltip :text="tt('Edit')" location="top">
                                    <template #activator="{ props }">
                                        <v-btn
                                            v-bind="props"
                                            icon
                                            variant="text"
                                            size="small"
                                            @click="openEditDialog(item)"
                                        >
                                            <v-icon :icon="mdiPencilOutline" size="small" />
                                        </v-btn>
                                    </template>
                                </v-tooltip>
                                <v-tooltip :text="tt('Delete')" location="top">
                                    <template #activator="{ props }">
                                        <v-btn
                                            v-bind="props"
                                            icon
                                            variant="text"
                                            size="small"
                                            color="error"
                                            @click="confirmDelete(item)"
                                        >
                                            <v-icon :icon="mdiDeleteOutline" size="small" />
                                        </v-btn>
                                    </template>
                                </v-tooltip>
                            </div>
                        </td>
                    </tr>
                    </tbody>
                    <tbody v-else>
                    <tr>
                        <td :colspan="accountRuleTableColumnCount" class="text-center text-medium-emphasis py-8">
                            {{ tt('No account rules') }}
                        </td>
                    </tr>
                    </tbody>
                </v-table>
            </v-card>
        </v-col>
    </v-row>

    <v-dialog v-model="showEditDialog" max-width="780" persistent>
        <v-card>
            <v-card-title>{{ editingRule ? tt('Edit Rule') : tt('Create Rule') }}</v-card-title>
            <v-card-text>
                <v-form>
                    <v-row>
                        <v-col cols="12" md="7">
                            <v-select
                                v-model="ruleForm.accountId"
                                :items="accountOptions"
                                item-title="name"
                                item-value="id"
                                density="comfortable"
                                variant="outlined"
                                :label="tt('Account')"
                                :disabled="saving || isAccountLocked"
                            >
                                <template #item="{ props: itemProps, item }">
                                    <v-list-item v-bind="itemProps" :title="item.raw.name">
                                        <template #prepend>
                                            <ItemIcon icon-type="account" :icon-id="item.raw.icon" :color="item.raw.color" />
                                        </template>
                                    </v-list-item>
                                </template>
                            </v-select>
                        </v-col>
                        <v-col cols="12" md="5">
                            <v-text-field
                                v-model.number="ruleForm.priority"
                                type="number"
                                density="comfortable"
                                variant="outlined"
                                :label="tt('Priority')"
                                :disabled="saving"
                            />
                        </v-col>
                        <v-col cols="12" md="6">
                            <v-select
                                v-model="ruleForm.accountRoleScope"
                                :items="roleScopeItems"
                                density="comfortable"
                                variant="outlined"
                                :label="tt('Account Role')"
                                :disabled="saving"
                            />
                        </v-col>
                        <v-col cols="12" md="6">
                            <v-select
                                v-model="ruleForm.transactionTypeScope"
                                :items="transactionScopeItems"
                                density="comfortable"
                                variant="outlined"
                                :label="tt('Transaction Type')"
                                :disabled="saving"
                            />
                        </v-col>
                        <v-col cols="12">
                            <v-select
                                v-model="ruleForm.fieldScope"
                                :items="fieldScopeItems"
                                density="comfortable"
                                variant="outlined"
                                multiple
                                chips
                                closable-chips
                                :label="tt('Field Scope')"
                                :disabled="saving"
                            />
                        </v-col>
                        <v-col cols="12">
                            <category-rule-builder-fields
                                v-model="ruleBuilderModel"
                                :auto-rule-name="autoRuleName"
                                :disabled="saving"
                                title="Account Matching"
                            />
                        </v-col>
                    </v-row>
                </v-form>
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showEditDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="primary" :loading="saving" @click="saveRule">{{ tt('Save') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <v-dialog v-model="showTestDialog" max-width="560">
        <v-card>
            <v-card-title>{{ tt('Test Rule') }}: {{ testRuleName }}</v-card-title>
            <v-card-text>
                <v-textarea
                    v-model="testText"
                    rows="3"
                    :label="tt('Text to test')"
                    :placeholder="tt('Enter parser, counterparty, payment method or description text')"
                />
                <v-row>
                    <v-col cols="12" md="6">
                        <v-select
                            v-model="testRoleScope"
                            :items="roleScopeItems"
                            density="comfortable"
                            variant="outlined"
                            :label="tt('Account Role')"
                        />
                    </v-col>
                    <v-col cols="12" md="6">
                        <v-select
                            v-model="testTransactionScope"
                            :items="transactionScopeItems"
                            density="comfortable"
                            variant="outlined"
                            :label="tt('Transaction Type')"
                        />
                    </v-col>
                </v-row>
                <v-alert v-if="testResult !== null" :type="testResult ? 'success' : 'warning'" class="mt-3">
                    {{ testResult ? tt('Match!') : tt('No match') }}
                </v-alert>
                <div v-if="testMatchedFields.length > 0" class="account-rule-field-chips mt-3">
                    <v-chip v-for="field in testMatchedFields" :key="field" size="small" variant="tonal">
                        {{ getFieldScopeLabel(field) }}
                    </v-chip>
                </div>
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showTestDialog = false">{{ tt('Close') }}</v-btn>
                <v-btn color="primary" :loading="testing" @click="runTest">{{ tt('Test') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <v-dialog v-model="showDeleteDialog" max-width="400">
        <v-card>
            <v-card-title>{{ tt('Delete Rule') }}</v-card-title>
            <v-card-text>
                {{ tt('Are you sure you want to delete rule') }} "{{ deletingRule?.name }}"?
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showDeleteDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="error" :loading="deleting" @click="doDelete">{{ tt('Delete') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <snack-bar ref="snackbar" />
</template>

<script setup lang="ts">
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue';
import {
    mdiArrowDown,
    mdiArrowUp,
    mdiBookAccountOutline,
    mdiCheckCircle,
    mdiCloseCircle,
    mdiDeleteOutline,
    mdiPencilOutline,
    mdiPlus,
    mdiRefresh,
    mdiTestTube,
} from '@mdi/js';

import CategoryRuleBuilderFields from '@/components/common/CategoryRuleBuilderFields.vue';
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import SettingsJsonImportExportButton from '@/components/desktop/SettingsJsonImportExportButton.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import services from '@/lib/services.ts';
import { useI18n } from '@/locales/helpers.ts';
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
    type AccountRuleTestResult,
    type AccountRuleTransactionTypeScope,
    type AccountRuleRoleScope,
} from '@/models/account_rule.ts';
import { useAccountsStore } from '@/stores/account.ts';
import RuleExpressionDisplay from './RuleExpressionDisplay.vue';
import {
    buildRuleExpressionDisplayGroups,
    type RuleExpressionDisplayGroup,
} from './ruleExpressionDisplay.ts';

interface AccountRuleAccountOption {
    id: string;
    name: string;
    icon: string;
    color: string;
}

const props = withDefaults(defineProps<{
    accountId?: number | null;
    title?: string;
    hideHeader?: boolean;
    headerActionsTarget?: string;
    showSettingsBundleControls?: boolean;
    embedded?: boolean;
}>(), {
    accountId: null,
    title: '',
    hideHeader: false,
    headerActionsTarget: '',
    showSettingsBundleControls: true,
    embedded: false,
});

const { tt } = useI18n();
const accountsStore = useAccountsStore();
type SnackBarType = InstanceType<typeof SnackBar>;

const loading = ref(false);
const saving = ref(false);
const deleting = ref(false);
const testing = ref(false);
const reordering = ref(false);
const error = ref<string | null>(null);
const accountRules = ref<AccountRuleItem[]>([]);
const togglingRuleIds = ref<number[]>([]);
const expandedExpressionClauseKeys = ref<string[]>([]);
const showEditDialog = ref(false);
const editingRule = ref<AccountRuleItem | null>(null);
const ruleForm = ref<AccountRuleForm>(createDefaultAccountRuleForm(props.accountId));
const showTestDialog = ref(false);
const testRuleId = ref<number>(0);
const testRuleName = ref('');
const testText = ref('');
const testRoleScope = ref<AccountRuleRoleScope>('any');
const testTransactionScope = ref<AccountRuleTransactionTypeScope>('all');
const testResult = ref<boolean | null>(null);
const testMatchedFields = ref<string[]>([]);
const showDeleteDialog = ref(false);
const deletingRule = ref<AccountRuleItem | null>(null);
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const maxVisibleFieldChips = 3;
const accountRuleTableColumnCount = 9;

const title = computed(() => props.title);
const hasHeaderActionsTarget = computed(() => Boolean(props.headerActionsTarget));
const headerActionsTarget = computed(() => props.headerActionsTarget || 'body');
const isAccountLocked = computed(() => props.accountId !== null && props.accountId !== undefined);
const allAccounts = computed<Account[]>(() => accountsStore.allMixedPlainAccounts as Account[]);
const accountOptions = computed<AccountRuleAccountOption[]>(() => allAccounts.value.map(account => ({
    id: String(account.id),
    name: account.name,
    icon: account.icon,
    color: account.color,
})));
const orderedAccountRules = computed<AccountRuleItem[]>(() => (
    [...accountRules.value].sort((firstRule, secondRule) => (
        firstRule.priority - secondRule.priority
        || resolveAccountName(firstRule).localeCompare(resolveAccountName(secondRule), 'zh-Hans')
        || firstRule.id - secondRule.id
    ))
));
const selectedAccountOption = computed(() => accountOptions.value.find(option => option.id === ruleForm.value.accountId) ?? null);
const autoRuleName = computed(() => `${selectedAccountOption.value?.name || tt('Account')} · ${tt('Account Rule')}`);
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
const ruleBuilderModel = computed({
    get: () => ({
        priority: ruleForm.value.priority,
        ruleExpression: ruleForm.value.ruleExpression,
        regexEnabled: ruleForm.value.regexEnabled,
        enabled: ruleForm.value.enabled,
    }),
    set: (value: {
        priority: number;
        ruleExpression: string;
        regexEnabled: boolean;
        enabled: boolean;
    }) => {
        ruleForm.value = {
            ...ruleForm.value,
            priority: value.priority,
            ruleExpression: value.ruleExpression,
            regexEnabled: value.regexEnabled,
            enabled: value.enabled,
        };
    },
});

function resolveAccount(rule: AccountRuleItem): Account | null {
    return allAccounts.value.find(account => String(account.id) === String(rule.accountId)) ?? null;
}

function resolveAccountName(rule: AccountRuleItem): string {
    return resolveAccount(rule)?.name || rule.accountName || tt('Account');
}

function resolveAccountIcon(rule: AccountRuleItem): string {
    return resolveAccount(rule)?.icon || '';
}

function resolveAccountColor(rule: AccountRuleItem): string {
    return resolveAccount(rule)?.color || '';
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

function getVisibleFieldScope(rule: AccountRuleItem): AccountRuleFieldScope[] {
    return rule.fieldScope.slice(0, maxVisibleFieldChips);
}

function getRuleExpressionGroups(rule: AccountRuleItem): RuleExpressionDisplayGroup[] {
    return buildRuleExpressionDisplayGroups(rule.ruleExpression, {
        empty: tt('Empty'),
        raw: tt('Raw'),
    });
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

    return fallback;
}

function showSuccessMessage(message: string, options?: Record<string, unknown>): void {
    snackbar.value?.showMessage(message, options);
}

function isRuleToggling(ruleId: number): boolean {
    return togglingRuleIds.value.includes(ruleId);
}

function setRuleToggling(ruleId: number, enabled: boolean): void {
    togglingRuleIds.value = enabled
        ? [...new Set([...togglingRuleIds.value, ruleId])]
        : togglingRuleIds.value.filter(item => item !== ruleId);
}

function openCreateDialog(): void {
    editingRule.value = null;
    ruleForm.value = createDefaultAccountRuleForm(props.accountId ?? accountOptions.value[0]?.id ?? '');
    showEditDialog.value = true;
}

function openEditDialog(item: AccountRuleItem): void {
    editingRule.value = item;
    ruleForm.value = accountRuleToForm(item);
    showEditDialog.value = true;
}

function confirmDelete(item: AccountRuleItem): void {
    deletingRule.value = item;
    showDeleteDialog.value = true;
}

async function saveRule(): Promise<void> {
    saving.value = true;
    error.value = null;
    try {
        const payload = buildAccountRulePayload(ruleForm.value, autoRuleName.value);
        if (editingRule.value) {
            requireApiSuccess(
                await services.updateAccountRule(editingRule.value.id, payload),
                tt('Failed to save rule')
            );
            showSuccessMessage('Rule updated');
        } else {
            requireApiSuccess(
                await services.createAccountRule(payload),
                tt('Failed to save rule')
            );
            showSuccessMessage('Rule created');
        }
        showEditDialog.value = false;
        await fetchAccountRules();
    } catch (err: unknown) {
        error.value = getRequestErrorMessage(err, tt('Failed to save rule'));
    } finally {
        saving.value = false;
    }
}

async function toggleEnabled(item: AccountRuleItem, nextEnabled: unknown): Promise<void> {
    if (isRuleToggling(item.id)) {
        return;
    }

    const normalizedNextEnabled = !!nextEnabled;
    if (normalizedNextEnabled === item.enabled) {
        return;
    }

    const previousRules = accountRules.value;
    error.value = null;
    setRuleToggling(item.id, true);
    accountRules.value = accountRules.value.map(rule => rule.id === item.id
        ? { ...rule, enabled: normalizedNextEnabled }
        : rule
    );
    try {
        requireApiSuccess(
            await services.updateAccountRule(item.id, { enabled: normalizedNextEnabled }),
            tt('Failed to toggle rule')
        );
        await fetchAccountRules();
    } catch (err: unknown) {
        accountRules.value = previousRules;
        error.value = getRequestErrorMessage(err, tt('Failed to toggle rule'));
    } finally {
        setRuleToggling(item.id, false);
    }
}

async function moveRule(item: AccountRuleItem, direction: -1 | 1): Promise<void> {
    const currentIndex = orderedAccountRules.value.findIndex(rule => rule.id === item.id);
    const targetIndex = currentIndex + direction;
    if (currentIndex < 0 || targetIndex < 0 || targetIndex >= orderedAccountRules.value.length) {
        return;
    }

    const nextRules = [...orderedAccountRules.value];
    const [movedRule] = nextRules.splice(currentIndex, 1);
    if (!movedRule) {
        return;
    }
    nextRules.splice(targetIndex, 0, movedRule);

    reordering.value = true;
    error.value = null;
    try {
        requireApiSuccess(
            await services.reorderAccountRules(nextRules.map(rule => rule.id)),
            tt('Failed to reorder rules')
        );
        await fetchAccountRules();
    } catch (err: unknown) {
        error.value = getRequestErrorMessage(err, tt('Failed to reorder rules'));
    } finally {
        reordering.value = false;
    }
}

async function doDelete(): Promise<void> {
    if (!deletingRule.value) {
        return;
    }

    deleting.value = true;
    error.value = null;
    try {
        requireApiSuccess(
            await services.deleteAccountRule(deletingRule.value.id),
            tt('Failed to delete rule')
        );
        showDeleteDialog.value = false;
        showSuccessMessage('Rule deleted');
        await fetchAccountRules();
    } catch (err: unknown) {
        error.value = getRequestErrorMessage(err, tt('Failed to delete rule'));
    } finally {
        deleting.value = false;
    }
}

function openTestDialog(item: AccountRuleItem): void {
    testRuleId.value = item.id;
    testRuleName.value = item.name || resolveAccountName(item);
    testText.value = '';
    testRoleScope.value = item.accountRoleScope;
    testTransactionScope.value = item.transactionTypeScope;
    testResult.value = null;
    testMatchedFields.value = [];
    showTestDialog.value = true;
}

async function runTest(): Promise<void> {
    if (!testText.value.trim()) {
        return;
    }

    testing.value = true;
    error.value = null;
    try {
        const result = requireApiSuccess<AccountRuleTestResult>(
            await services.testAccountRule(
                testRuleId.value,
                buildAccountRuleTestContext(testText.value, testRoleScope.value, testTransactionScope.value)
            ),
            tt('Test failed')
        );
        testResult.value = !!result?.matched;
        testMatchedFields.value = result?.matchedFields ?? [];
    } catch (err: unknown) {
        testResult.value = null;
        testMatchedFields.value = [];
        error.value = getRequestErrorMessage(err, tt('Test failed'));
    } finally {
        testing.value = false;
    }
}

async function fetchAccountRules(): Promise<void> {
    try {
        const result = requireApiSuccess<Record<string, unknown>[]>(
            await services.getAccountRules(props.accountId ?? undefined),
            tt('Failed to load account rules')
        );
        accountRules.value = (result ?? []).map(item => normalizeAccountRuleItem(item));
    } catch (err: unknown) {
        error.value = getRequestErrorMessage(err, tt('Failed to load account rules'));
    }
}

async function fetchAll(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
        await Promise.all([
            accountsStore.loadAllAccounts({ force: false }),
            fetchAccountRules(),
        ]);
    } finally {
        loading.value = false;
    }
}

watch(
    () => props.accountId,
    () => {
        if (showEditDialog.value && isAccountLocked.value) {
            ruleForm.value.accountId = props.accountId ? String(props.accountId) : '';
        }
        void fetchAccountRules();
    }
);

onMounted(() => fetchAll());

defineExpose({
    refresh: fetchAll,
});
</script>

<style scoped>
.account-rule-section-actions {
    flex: 0 0 auto;
}

.account-recognition-rule-panel,
.account-recognition-rule-panel :deep(.v-col) {
    min-width: 0;
    max-width: 100%;
}

.account-rule-table-toolbar--external {
    padding: 0;
}

.account-rule-table-toolbar--external .account-rule-section-actions {
    min-height: 32px;
}

.account-rule-section-title {
    min-height: 38px;
}

.account-rule-table {
    table-layout: fixed;
}

.account-rule-table :deep(.v-table__wrapper) {
    max-width: 100%;
    overflow-x: auto;
}

.account-rule-table :deep(th),
.account-rule-table :deep(td) {
    vertical-align: middle;
}

.account-rule-table :deep(th) {
    white-space: nowrap;
    text-align: center;
    padding-block: 8px;
}

.account-rule-header-cell {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 32px;
    width: 100%;
}

.account-rule-column-account {
    width: 190px;
}

.account-rule-column-expression {
    min-width: 320px;
}

.account-rule-column-scope {
    width: 128px;
    text-align: center;
}

.account-rule-column-fields {
    width: 210px;
}

.account-rule-column-regex {
    width: 118px;
    text-align: center;
}

.account-rule-column-enabled {
    width: 104px;
    text-align: center;
}

.account-rule-column-applied {
    width: 84px;
    text-align: center;
}

.account-rule-column-actions {
    width: 214px;
    text-align: center;
}

.account-rule-actions-row {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
    flex-wrap: nowrap;
}

.account-rule-field-chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
}

@media (max-width: 960px) {
    .account-rule-table {
        table-layout: auto;
    }
}
</style>
