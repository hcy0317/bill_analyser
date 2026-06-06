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

                <Teleport defer :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">
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
                    </tr>
                    </thead>
                    <tbody v-if="accountRuleGroups.length > 0">
                    <template v-for="categoryGroup in accountRuleGroups" :key="categoryGroup.key">
                        <tr class="account-rule-category-row">
                            <td :colspan="accountRuleTableColumnCount">
                                <div class="account-rule-category-header">
                                    <v-btn
                                        icon
                                        variant="text"
                                        size="x-small"
                                        @click="toggleCategoryGroup(categoryGroup.key)"
                                    >
                                        <v-icon :icon="isCategoryExpanded(categoryGroup.key) ? mdiChevronDown : mdiChevronRight" />
                                    </v-btn>
                                    <span class="font-weight-medium">{{ categoryGroup.categoryName }}</span>
                                    <v-chip size="x-small" variant="tonal">{{ tt('Rules') }} {{ categoryGroup.ruleCount }}</v-chip>
                                    <v-chip size="x-small" variant="tonal">{{ tt('Matched') }} {{ categoryGroup.matchCount }}</v-chip>
                                </div>
                            </td>
                        </tr>
                        <tr
                            v-for="accountGroup in categoryGroup.accounts"
                            v-show="isCategoryExpanded(categoryGroup.key)"
                            :key="accountGroup.key"
                        >
                            <td class="account-rule-column-account">
                                <div class="account-rule-account-cell" :title="accountGroup.displayName">
                                    <ItemIcon
                                        icon-type="account"
                                        size="24px"
                                        :icon-id="accountGroup.icon"
                                        :color="accountGroup.color"
                                    />
                                    <div class="account-rule-account-text">
                                        <span class="text-truncate">{{ accountGroup.accountName }}</span>
                                        <span v-if="accountGroup.parentAccountName" class="text-caption text-medium-emphasis text-truncate">
                                            {{ accountGroup.parentAccountName }}
                                        </span>
                                    </div>
                                </div>
                            </td>
                            <td class="account-rule-column-expression">
                                <div
                                    v-for="rule in accountGroup.rules"
                                    :key="rule.id"
                                    class="account-rule-expression-item"
                                >
                                    <div class="account-rule-expression-main">
                                        <div class="account-rule-expression-title-row">
                                            <span class="font-weight-medium">{{ rule.name || accountGroup.accountName }}</span>
                                            <v-chip size="x-small" variant="tonal">{{ tt('Priority') }} {{ rule.priority }}</v-chip>
                                            <v-chip size="x-small" :color="rule.regexEnabled ? 'info' : undefined" variant="tonal">
                                                {{ rule.regexEnabled ? tt('Use Regex') : tt('Plain Text') }}
                                            </v-chip>
                                            <v-chip size="x-small" variant="tonal">{{ tt('Matched') }} {{ rule.matchCount || rule.appliedCount }}</v-chip>
                                        </div>
                                        <rule-expression-display
                                            :rule-id="rule.id"
                                            :groups="getRuleExpressionGroups(rule)"
                                            :expanded-clause-keys="expandedExpressionClauseKeys"
                                            :max-collapsed-terms="4"
                                            @update:expanded-clause-keys="expandedExpressionClauseKeys = $event"
                                        />
                                    </div>
                                    <div class="account-rule-expression-actions">
                                        <div class="d-inline-flex" @click.stop>
                                            <v-switch
                                                :model-value="rule.enabled"
                                                density="compact"
                                                hide-details
                                                color="success"
                                                :disabled="isRuleToggling(rule.id)"
                                                @update:model-value="toggleEnabled(rule, $event)"
                                            />
                                        </div>
                                        <v-tooltip :text="tt('Move Up')" location="top">
                                            <template #activator="{ props }">
                                                <v-btn
                                                    v-bind="props"
                                                    icon
                                                    variant="text"
                                                    size="small"
                                                    :disabled="getOrderedRuleIndex(rule) === 0 || reordering"
                                                    @click="moveRule(rule, -1)"
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
                                                    :disabled="getOrderedRuleIndex(rule) >= orderedAccountRules.length - 1 || reordering"
                                                    @click="moveRule(rule, 1)"
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
                                                    @click="openTestDialog(rule)"
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
                                                    @click="openEditDialog(rule)"
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
                                                    @click="confirmDelete(rule)"
                                                >
                                                    <v-icon :icon="mdiDeleteOutline" size="small" />
                                                </v-btn>
                                            </template>
                                        </v-tooltip>
                                    </div>
                                </div>
                            </td>
                        </tr>
                    </template>
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

    <v-dialog v-model="showEditDialog" max-width="720" persistent>
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
                <v-alert v-if="testResult !== null" :type="testResult ? 'success' : 'warning'" class="mt-3">
                    {{ testResult ? tt('Match!') : tt('No match') }}
                </v-alert>
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
    mdiChevronDown,
    mdiChevronRight,
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
    accountRuleToForm,
    buildAccountRuleGroups,
    buildAccountRulePayload,
    buildAccountRuleTestContext,
    createDefaultAccountRuleForm,
    normalizeAccountRuleItem,
    type AccountRuleForm,
    type AccountRuleItem,
    type AccountRuleTestResult,
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
const expandedCategoryKeys = ref<string[]>([]);
const showEditDialog = ref(false);
const editingRule = ref<AccountRuleItem | null>(null);
const ruleForm = ref<AccountRuleForm>(createDefaultAccountRuleForm(props.accountId));
const showTestDialog = ref(false);
const testRuleId = ref<number>(0);
const testRuleName = ref('');
const testText = ref('');
const testResult = ref<boolean | null>(null);
const showDeleteDialog = ref(false);
const deletingRule = ref<AccountRuleItem | null>(null);
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const accountRuleTableColumnCount = 2;

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
        || firstRule.accountId - secondRule.accountId
        || firstRule.id - secondRule.id
    ))
));
const accountRuleGroups = computed(() => buildAccountRuleGroups(accountRules.value, allAccounts.value, tt));
const selectedAccountOption = computed(() => accountOptions.value.find(option => option.id === ruleForm.value.accountId) ?? null);
const autoRuleName = computed(() => `${selectedAccountOption.value?.name || tt('Account')} · ${tt('Account Rule')}`);
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

function getOrderedRuleIndex(rule: AccountRuleItem): number {
    return orderedAccountRules.value.findIndex(item => item.id === rule.id);
}

function isCategoryExpanded(key: string): boolean {
    return expandedCategoryKeys.value.includes(key);
}

function toggleCategoryGroup(key: string): void {
    expandedCategoryKeys.value = isCategoryExpanded(key)
        ? expandedCategoryKeys.value.filter(item => item !== key)
        : [...expandedCategoryKeys.value, key];
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
    testRuleName.value = item.name || item.accountName || tt('Account');
    testText.value = '';
    testResult.value = null;
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
                buildAccountRuleTestContext(testText.value)
            ),
            tt('Test failed')
        );
        testResult.value = !!result?.matched;
    } catch (err: unknown) {
        testResult.value = null;
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

watch(
    accountRuleGroups,
    (groups) => {
        const nextKeys = groups.map(group => group.key);
        expandedCategoryKeys.value = expandedCategoryKeys.value.length === 0
            ? nextKeys
            : nextKeys.filter(key => expandedCategoryKeys.value.includes(key) || !expandedCategoryKeys.value.length);
    },
    { immediate: true }
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
    width: 260px;
}

.account-rule-column-expression {
    min-width: 520px;
}

.account-rule-category-row td {
    background: rgba(var(--v-theme-primary), 0.08);
    padding-block: 6px;
}

.account-rule-category-header {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
}

.account-rule-account-cell {
    display: flex;
    align-items: center;
    min-width: 0;
}

.account-rule-account-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    margin-left: 8px;
}

.account-rule-expression-item {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding-block: 10px;
}

.account-rule-expression-item + .account-rule-expression-item {
    border-top: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.account-rule-expression-main {
    min-width: 0;
    flex: 1 1 auto;
}

.account-rule-expression-title-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 6px;
}

.account-rule-expression-actions {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 2px;
    flex: 0 0 auto;
    flex-wrap: nowrap;
}

@media (max-width: 960px) {
    .account-rule-table {
        table-layout: auto;
    }

    .account-rule-expression-item {
        flex-direction: column;
    }

    .account-rule-expression-actions {
        justify-content: flex-start;
    }
}
</style>
