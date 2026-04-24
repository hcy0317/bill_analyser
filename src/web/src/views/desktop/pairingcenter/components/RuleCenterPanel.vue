<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-title class="d-flex align-center">
                    <v-icon :icon="mdiBookCogOutline" class="me-2" />
                    <span>{{ title || tt('Category and Recurring Rules') }}</span>
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

                <v-tabs v-model="activeTab" class="px-4">
                    <v-tab v-if="hasTab('rules')" value="rules">
                        <v-icon start :icon="mdiBookCogOutline" />
                        {{ tt('Category Rules') }} ({{ categoryRules.length }})
                    </v-tab>
                    <v-tab v-if="hasTab('learning')" value="learning">
                        <v-icon start :icon="mdiBrain" />
                        {{ tt('Learning Rules') }} ({{ overview.learningRuleCount }})
                    </v-tab>
                    <v-tab v-if="hasTab('recurring')" value="recurring">
                        <v-icon start :icon="mdiCalendarSync" />
                        {{ tt('Recurring Rules') }} ({{ overview.recurringRuleCount }})
                    </v-tab>
                </v-tabs>

                <v-tabs-window v-model="activeTab">
                    <!-- Category Rules -->
                    <v-tabs-window-item v-if="hasTab('rules')" value="rules">
                        <div class="d-flex align-center pa-4 ga-2">
                            <v-btn color="primary" :prepend-icon="mdiPlus" @click="openCreateDialog">
                                {{ tt('Add Rule') }}
                            </v-btn>
                            <v-btn
                                variant="outlined"
                                :prepend-icon="mdiDatabaseImportOutline"
                                :disabled="loading"
                                @click="migrateKeywords"
                            >
                                {{ tt('Import Rules from Legacy Keywords') }}
                            </v-btn>
                            <v-spacer />
                            <div class="rule-center-category-filter">
                                <two-column-select
                                    v-model="ruleCategoryFilterId"
                                    density="compact"
                                    variant="outlined"
                                    primary-key-field="id"
                                    primary-value-field="id"
                                    primary-title-field="name"
                                    primary-header-field="typeLabel"
                                    primary-icon-field="icon"
                                    primary-icon-type="category"
                                    primary-color-field="color"
                                    primary-hidden-field="hidden"
                                    primary-sub-items-field="subCategories"
                                    secondary-key-field="id"
                                    secondary-value-field="id"
                                    secondary-title-field="name"
                                    secondary-icon-field="icon"
                                    secondary-icon-type="category"
                                    secondary-color-field="color"
                                    secondary-hidden-field="hidden"
                                    :show-selection-primary-text="true"
                                    :custom-selection-primary-text="ruleCategoryFilterSelection.primaryText"
                                    :custom-selection-secondary-text="ruleCategoryFilterSelection.secondaryText"
                                    :enable-filter="true"
                                    :filter-placeholder="tt('Find category')"
                                    :filter-no-items-text="tt('No available category')"
                                    :no-item-text="tt('All Categories')"
                                    :items="categoryPickerItems"
                                    :label="tt('Category')"
                                />
                            </div>
                            <v-btn
                                v-if="ruleCategoryFilterId"
                                variant="text"
                                size="small"
                                @click="clearRuleCategoryFilter"
                            >
                                {{ tt('Clear') }}
                            </v-btn>
                        </div>
                        <v-data-table
                            :headers="ruleHeaders"
                            :items="filteredDisplayCategoryRules"
                            :items-per-page="20"
                            density="compact"
                            :sort-by="[{ key: 'priority', order: 'asc' }]"
                        >
                            <template #item.priority="{ item }">
                                <v-chip
                                    size="small"
                                    :color="item.priority <= 10 ? 'error' : item.priority <= 50 ? 'warning' : 'default'"
                                >
                                    {{ item.priority }}
                                </v-chip>
                            </template>
                            <template #item.category_display_name="{ item }">
                                <div
                                    class="d-flex align-center"
                                    :title="item.category_full_name"
                                >
                                    <ItemIcon
                                        v-if="item.category_icon && item.category_color"
                                        icon-type="category"
                                        size="24px"
                                        :icon-id="item.category_icon"
                                        :color="item.category_color"
                                    />
                                    <v-icon v-else size="24" :icon="mdiCloseCircle" color="grey" />
                                    <span class="ms-2">{{ item.category_display_name }}</span>
                                </div>
                            </template>
                            <template #item.regex_enabled="{ item }">
                                <v-icon
                                    :icon="item.regex_enabled ? mdiCheckCircle : mdiCloseCircle"
                                    :color="item.regex_enabled ? 'info' : 'grey'"
                                    size="small"
                                />
                            </template>
                            <template #item.enabled="{ item }">
                                <v-switch
                                    :model-value="item.enabled"
                                    density="compact"
                                    hide-details
                                    color="success"
                                    :disabled="isRuleToggling(item.id)"
                                    @click.stop
                                    @update:model-value="toggleEnabled(item, $event)"
                                />
                            </template>
                            <template #item.actions="{ item }">
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
                                <v-tooltip :text="tt('Test')" location="top">
                                    <template #activator="{ props }">
                                        <v-btn
                                            v-bind="props"
                                            icon
                                            variant="text"
                                            size="small"
                                            @click="openTestDialog(item)"
                                        >
                                            <v-icon :icon="mdiTestTube" size="small" />
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
                            </template>
                        </v-data-table>
                    </v-tabs-window-item>

                    <!-- Learning Rules -->
                    <v-tabs-window-item v-if="hasTab('learning')" value="learning">
                        <v-data-table
                            :headers="learningHeaders"
                            :items="overview.learningRules"
                            :items-per-page="20"
                            density="compact"
                        >
                            <template #item.enabled="{ item }">
                                <v-icon
                                    :icon="item.enabled ? mdiCheckCircle : mdiCloseCircle"
                                    :color="item.enabled ? 'success' : 'grey'"
                                    size="small"
                                />
                            </template>
                        </v-data-table>
                    </v-tabs-window-item>

                    <!-- Recurring Rules -->
                    <v-tabs-window-item v-if="hasTab('recurring')" value="recurring">
                        <v-data-table
                            :headers="recurringHeaders"
                            :items="overview.recurringRules"
                            :items-per-page="20"
                            density="compact"
                        >
                            <template #item.amount="{ item }">
                                ¥{{ Math.abs(item.amount || 0).toFixed(2) }}
                            </template>
                            <template #item.enabled="{ item }">
                                <v-icon
                                    :icon="item.enabled ? mdiCheckCircle : mdiCloseCircle"
                                    :color="item.enabled ? 'success' : 'grey'"
                                    size="small"
                                />
                            </template>
                        </v-data-table>
                    </v-tabs-window-item>
                </v-tabs-window>
            </v-card>
        </v-col>
    </v-row>

    <!-- Create / Edit Dialog -->
    <v-dialog v-model="showEditDialog" max-width="700" persistent>
        <v-card>
            <v-card-title>{{ editingRule ? tt('Edit Rule') : tt('Create Rule') }}</v-card-title>
            <v-card-text>
                <v-form ref="ruleFormRef">
                    <two-column-select
                        v-model="ruleForm.category_id"
                        density="comfortable"
                        variant="outlined"
                        primary-key-field="id"
                        primary-value-field="id"
                        primary-title-field="name"
                        primary-header-field="typeLabel"
                        primary-icon-field="icon"
                        primary-icon-type="category"
                        primary-color-field="color"
                        primary-hidden-field="hidden"
                        primary-sub-items-field="subCategories"
                        secondary-key-field="id"
                        secondary-value-field="id"
                        secondary-title-field="name"
                        secondary-icon-field="icon"
                        secondary-icon-type="category"
                        secondary-color-field="color"
                        secondary-hidden-field="hidden"
                        :show-selection-primary-text="true"
                        :custom-selection-primary-text="ruleCategorySelection.primaryText"
                        :custom-selection-secondary-text="ruleCategorySelection.secondaryText"
                        :enable-filter="true"
                        :filter-placeholder="tt('Find category')"
                        :filter-no-items-text="tt('No available category')"
                        :items="categoryPickerItems"
                        :label="tt('Category')"
                        class="mb-2"
                    />
                    <category-rule-builder-fields
                        v-model="ruleBuilderModel"
                        :auto-rule-name="autoRuleName"
                        :disabled="saving"
                        :title="tt('Canonical Category Rule')"
                    />
                </v-form>
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showEditDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="primary" :loading="saving" @click="saveRule">{{ tt('Save') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- Test Dialog -->
    <v-dialog v-model="showTestDialog" max-width="500">
        <v-card>
            <v-card-title>{{ tt('Test Rule') }}: {{ testRuleName }}</v-card-title>
            <v-card-text>
                <v-text-field
                    v-model="testText"
                    :label="tt('Text to test')"
                    :placeholder="tt('Enter description or counterparty text...')"
                    @keyup.enter="runTest"
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

    <!-- Delete Confirmation -->
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
import axios from 'axios';
import { ref, computed, onMounted, watch, useTemplateRef } from 'vue';
import {
    mdiBookCogOutline, mdiRefresh, mdiBrain, mdiCalendarSync,
    mdiCheckCircle, mdiCloseCircle, mdiPlus, mdiPencilOutline, mdiDeleteOutline,
    mdiTestTube, mdiDatabaseImportOutline,
} from '@mdi/js';
import type { ApiResponse, ErrorResponse } from '@/core/api.ts';
import services from '@/lib/services.ts';
import { useI18n } from '@/locales/helpers.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { CategoryType } from '@/core/category.ts';
import CategoryRuleBuilderFields from '@/components/common/CategoryRuleBuilderFields.vue';
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import TwoColumnSelect from '@/components/desktop/TwoColumnSelect.vue';

const categoryStore = useTransactionCategoriesStore();
const { tt } = useI18n();
type SnackBarType = InstanceType<typeof SnackBar>;
type RuleCenterPanelTab = 'rules' | 'learning' | 'recurring';

const props = defineProps<{
    initTab?: string;
    tabs?: RuleCenterPanelTab[];
    title?: string;
}>();

const loading = ref(false);
const saving = ref(false);
const deleting = ref(false);
const testing = ref(false);
const error = ref<string | null>(null);
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const visibleTabs = computed<RuleCenterPanelTab[]>(() => props.tabs && props.tabs.length > 0
    ? props.tabs
    : ['rules', 'learning', 'recurring']
);
const title = computed(() => props.title);

function hasTab(tab: RuleCenterPanelTab): boolean {
    return visibleTabs.value.includes(tab);
}

function normalizeTab(tab?: string): RuleCenterPanelTab {
    if ((tab === 'learning' || tab === 'recurring') && hasTab(tab)) {
        return tab;
    }

    return visibleTabs.value[0] ?? 'rules';
}

const activeTab = ref<RuleCenterPanelTab>(normalizeTab(props.initTab));

// ── Overview (existing tabs) ────────
interface LearningRuleOverviewItem {
    matchType: string;
    matchValue: string;
    learnedType: string;
    appliedCount: number;
    enabled: boolean;
}

interface RecurringRuleOverviewItem {
    name: string;
    amount: number | null;
    frequency: string;
    nextDate: string | null;
    enabled: boolean;
}

interface Overview {
    learningRules: LearningRuleOverviewItem[];
    learningRuleCount: number;
    categoryRuleCount: number;
    recurringRules: RecurringRuleOverviewItem[];
    recurringRuleCount: number;
    totalRuleCount: number;
}

const overview = ref<Overview>({
    learningRules: [], learningRuleCount: 0,
    categoryRuleCount: 0,
    recurringRules: [], recurringRuleCount: 0,
    totalRuleCount: 0,
});

// ── Category Rules ────────
interface CategoryRuleItem {
    id: number;
    name: string;
    category_id: number | null;
    category_name: string | null;
    sub_category_name: string | null;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
    applied_count: number;
}

interface DisplayCategoryRuleItem extends CategoryRuleItem {
    category_display_name: string;
    category_full_name: string;
    category_icon: string;
    category_color: string;
}

interface CategoryRuleForm {
    category_id: string;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
}

interface CategoryRulePayload {
    category_id: number;
    name: string;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
}

interface CategoryPickerSecondaryItem extends Record<string, unknown> {
    id: string;
    name: string;
    icon: string;
    color: string;
    hidden: boolean;
}

interface CategoryPickerPrimaryItem extends Record<string, unknown> {
    id: string;
    name: string;
    icon: string;
    color: string;
    hidden: boolean;
    typeLabel: string;
    subCategories: CategoryPickerSecondaryItem[];
}

interface ResolvedRuleCategorySelection {
    primaryText: string;
    secondaryText: string;
    label: string;
}

interface CategoryRuleTestResult {
    matched?: boolean | null;
}

interface CategoryKeywordMigrationResult {
    migrated?: number | string | null;
    migrated_count?: number | string | null;
    skipped?: number | string | null;
    skipped_count?: number | string | null;
}

const categoryRules = ref<CategoryRuleItem[]>([]);
const togglingRuleIds = ref<number[]>([]);

const ruleHeaders = computed(() => [
    { title: tt('Priority'), key: 'priority', sortable: true },
    { title: tt('Category'), key: 'category_display_name' },
    { title: tt('Expression'), key: 'rule_expression' },
    { title: tt('Regex'), key: 'regex_enabled', width: 80 },
    { title: tt('Enabled'), key: 'enabled', width: 100 },
    { title: tt('Applied'), key: 'applied_count', width: 80 },
    { title: tt('Actions'), key: 'actions', sortable: false, width: 140 },
]);

const learningHeaders = computed(() => [
    { title: tt('Match Type'), key: 'matchType' },
    { title: tt('Match Value'), key: 'matchValue' },
    { title: tt('Learned Type'), key: 'learnedType' },
    { title: tt('Applied'), key: 'appliedCount' },
    { title: tt('Enabled'), key: 'enabled' },
]);

const recurringHeaders = computed(() => [
    { title: tt('Name'), key: 'name' },
    { title: tt('Amount'), key: 'amount' },
    { title: tt('Frequency'), key: 'frequency' },
    { title: tt('Next Date'), key: 'nextDate' },
    { title: tt('Enabled'), key: 'enabled' },
]);

const displayCategoryRules = computed<DisplayCategoryRuleItem[]>(() => categoryRules.value.map(item => {
    const display = resolveCategoryDisplay(item);
    return {
        ...item,
        category_display_name: display.name,
        category_full_name: display.fullName,
        category_icon: display.icon,
        category_color: display.color,
    };
}));

// ── Category selector options ────────
function getCategoryTypeLabel(type: number): string {
    switch (type) {
        case CategoryType.Expense:
            return tt('Expense');
        case CategoryType.Income:
            return tt('Income');
        case CategoryType.Transfer:
            return tt('Transfer');
        case CategoryType.Investment:
            return tt('Investment');
        default:
            return tt('Category');
    }
}

const categoryPickerItems = computed<CategoryPickerPrimaryItem[]>(() => {
    const orderedTypes = [
        CategoryType.Expense,
        CategoryType.Income,
        CategoryType.Transfer,
        CategoryType.Investment,
    ];

    return orderedTypes.flatMap(type => (
        categoryStore.allTransactionCategories[type] || []
    ).map(primaryCategory => ({
        id: String(primaryCategory.id),
        name: primaryCategory.name,
        icon: primaryCategory.icon,
        color: primaryCategory.color,
        hidden: primaryCategory.hidden,
        typeLabel: getCategoryTypeLabel(primaryCategory.type),
        subCategories: (primaryCategory.subCategories || []).map(subCategory => ({
            id: String(subCategory.id),
            name: subCategory.name,
            icon: subCategory.icon,
            color: subCategory.color,
            hidden: subCategory.hidden,
        })),
    })));
});

function resolveRuleCategorySelection(categoryId: string): ResolvedRuleCategorySelection {
    const normalizedCategoryId = String(categoryId || '');

    for (const primaryCategory of categoryPickerItems.value) {
        if (primaryCategory.id === normalizedCategoryId) {
            return {
                primaryText: primaryCategory.name,
                secondaryText: '',
                label: primaryCategory.name,
            };
        }

        for (const secondaryCategory of primaryCategory.subCategories) {
            if (secondaryCategory.id === normalizedCategoryId) {
                return {
                    primaryText: primaryCategory.name,
                    secondaryText: secondaryCategory.name,
                    label: `${primaryCategory.name} / ${secondaryCategory.name}`,
                };
            }
        }
    }

    return {
        primaryText: '',
        secondaryText: '',
        label: '',
    };
}

function resolveCategoryDisplay(item: CategoryRuleItem): { name: string; fullName: string; icon: string; color: string } {
    const categoryId = item.category_id !== null && item.category_id !== undefined
        ? String(item.category_id)
        : '';
    const category = categoryId ? categoryStore.allTransactionCategoriesMap[categoryId] : null;
    const displayName = item.sub_category_name || category?.name || item.category_name || tt('Unassigned Category');
    const fullName = item.category_name && item.sub_category_name
        ? `${item.category_name} / ${item.sub_category_name}`
        : displayName;

    if (category) {
        return {
            name: displayName,
            fullName,
            icon: category.icon,
            color: String(category.color),
        };
    }

    if (item.sub_category_name) {
        return {
            name: item.sub_category_name,
            fullName,
            icon: '',
            color: '',
        };
    }

    if (item.category_name) {
        return {
            name: item.category_name,
            fullName,
            icon: '',
            color: '',
        };
    }

    return {
        name: tt('Unassigned Category'),
        fullName: tt('Unassigned Category'),
        icon: '',
        color: '',
    };
}

// ── Edit dialog state ────────
const showEditDialog = ref(false);
const editingRule = ref<CategoryRuleItem | null>(null);
const ruleFormRef = ref<unknown>(null);
const ruleCategoryFilterId = ref<string>('');
const ruleForm = ref<CategoryRuleForm>({
    category_id: '',
    priority: 100,
    rule_expression: '',
    regex_enabled: false,
    enabled: true,
});
const ruleCategorySelection = computed<ResolvedRuleCategorySelection>(() => resolveRuleCategorySelection(ruleForm.value.category_id));
const ruleCategoryFilterSelection = computed<ResolvedRuleCategorySelection>(() => resolveRuleCategorySelection(ruleCategoryFilterId.value));
const autoRuleName = computed(() => {
    const selectionLabel = ruleCategorySelection.value.label || tt('Unassigned Category');
    const priority = Number.isFinite(ruleForm.value.priority) ? ruleForm.value.priority : 0;
    return `${selectionLabel} · P${priority}`;
});

const filteredDisplayCategoryRules = computed<DisplayCategoryRuleItem[]>(() => {
    const filterId = String(ruleCategoryFilterId.value || '');
    if (!filterId) {
        return displayCategoryRules.value;
    }

    const selectedCategory = categoryStore.allTransactionCategoriesMap[filterId];
    const selectedIsPrimary = !!selectedCategory && (!selectedCategory.parentId || selectedCategory.parentId === '0');

    return displayCategoryRules.value.filter(item => {
        const itemCategoryId = item.category_id !== null && item.category_id !== undefined
            ? String(item.category_id)
            : '';
        if (!itemCategoryId) {
            return false;
        }
        if (itemCategoryId === filterId) {
            return true;
        }

        const itemCategory = categoryStore.allTransactionCategoriesMap[itemCategoryId];
        return selectedIsPrimary && itemCategory?.parentId === filterId;
    });
});

function clearRuleCategoryFilter(): void {
    ruleCategoryFilterId.value = '';
}
const ruleBuilderModel = computed({
    get: () => ({
        priority: ruleForm.value.priority,
        ruleExpression: ruleForm.value.rule_expression,
        regexEnabled: ruleForm.value.regex_enabled,
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
            rule_expression: value.ruleExpression,
            regex_enabled: value.regexEnabled,
            enabled: value.enabled,
        };
    },
});

function extractPayloadMessage(payload: unknown, depth = 0): string | null {
    if (depth > 2) {
        return null;
    }

    if (typeof payload === 'string' && payload) {
        return payload;
    }

    if (!payload || typeof payload !== 'object') {
        return null;
    }

    const typedPayload = payload as Partial<ErrorResponse> & {
        error?: unknown;
        message?: unknown;
    };

    return extractPayloadMessage(
        typedPayload.errorMessage ?? typedPayload.error ?? typedPayload.message,
        depth + 1
    );
}

function getRequestErrorMessage(error: unknown, fallback: string): string {
    if (axios.isAxiosError(error)) {
        return extractPayloadMessage(error.response?.data) || error.message || fallback;
    }

    if (error instanceof Error && error.message) {
        return error.message;
    }

    return fallback;
}

function requireApiSuccess<T>(response: { data?: ApiResponse<T> }, fallback: string): T {
    if (response.data?.success) {
        return response.data.result;
    }

    throw new Error(fallback);
}

function showSuccessMessage(message: string, options?: Record<string, unknown>): void {
    snackbar.value?.showMessage(message, options);
}

function buildCategoryRulePayload(form: CategoryRuleForm): CategoryRulePayload {
    const categoryId = Number.parseInt(String(form.category_id || ''), 10);
    if (!Number.isFinite(categoryId)) {
        throw new Error(tt('Category is required'));
    }

    const ruleExpression = String(form.rule_expression || '').trim();
    if (!ruleExpression) {
        throw new Error(tt('Expression is required'));
    }

    return {
        category_id: categoryId,
        name: autoRuleName.value,
        priority: form.priority,
        rule_expression: ruleExpression,
        regex_enabled: !!form.regex_enabled,
        enabled: !!form.enabled,
    };
}

function getMigrationCount(value: number | string | null | undefined): number {
    const count = Number(value ?? 0);
    return Number.isFinite(count) ? count : 0;
}

function openCreateDialog() {
    editingRule.value = null;
    ruleForm.value = { category_id: '', priority: 100, rule_expression: '', regex_enabled: false, enabled: true };
    showEditDialog.value = true;
}

function openEditDialog(item: CategoryRuleItem) {
    editingRule.value = item;
    ruleForm.value = {
        category_id: item.category_id ? String(item.category_id) : '',
        priority: item.priority,
        rule_expression: item.rule_expression,
        regex_enabled: !!item.regex_enabled,
        enabled: !!item.enabled,
    };
    showEditDialog.value = true;
}

async function saveRule() {
    saving.value = true;
    error.value = null;
    try {
        const payload = buildCategoryRulePayload(ruleForm.value);
        if (editingRule.value) {
            requireApiSuccess(
                await services.updateCategoryRule(editingRule.value.id, payload),
                tt('Failed to save rule')
            );
            showSuccessMessage('Rule updated');
        } else {
            requireApiSuccess(
                await services.createCategoryRule(payload),
                tt('Failed to save rule')
            );
            showSuccessMessage('Rule created');
        }
        showEditDialog.value = false;
        await fetchCategoryRules();
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, tt('Failed to save rule'));
    } finally {
        saving.value = false;
    }
}

function isRuleToggling(ruleId: number): boolean {
    return togglingRuleIds.value.includes(ruleId);
}

function setRuleToggling(ruleId: number, enabled: boolean): void {
    togglingRuleIds.value = enabled
        ? [...new Set([...togglingRuleIds.value, ruleId])]
        : togglingRuleIds.value.filter(item => item !== ruleId);
}

async function toggleEnabled(item: CategoryRuleItem, nextEnabled: unknown) {
    if (isRuleToggling(item.id)) {
        return;
    }

    const normalizedNextEnabled = !!nextEnabled;
    error.value = null;
    setRuleToggling(item.id, true);
    try {
        requireApiSuccess(
            await services.updateCategoryRule(item.id, { enabled: normalizedNextEnabled }),
            tt('Failed to toggle rule')
        );
        await fetchCategoryRules();
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, tt('Failed to toggle rule'));
    } finally {
        setRuleToggling(item.id, false);
    }
}

// ── Delete ────────
const showDeleteDialog = ref(false);
const deletingRule = ref<CategoryRuleItem | null>(null);

function confirmDelete(item: CategoryRuleItem) {
    deletingRule.value = item;
    showDeleteDialog.value = true;
}

async function doDelete() {
    if (!deletingRule.value) return;
    deleting.value = true;
    error.value = null;
    try {
        requireApiSuccess(
            await services.deleteCategoryRule(deletingRule.value.id),
            tt('Failed to delete rule')
        );
        showDeleteDialog.value = false;
        showSuccessMessage('Rule deleted');
        await fetchCategoryRules();
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, tt('Failed to delete rule'));
    } finally {
        deleting.value = false;
    }
}

// ── Test ────────
const showTestDialog = ref(false);
const testRuleId = ref<number>(0);
const testRuleName = ref('');
const testText = ref('');
const testResult = ref<boolean | null>(null);

function openTestDialog(item: CategoryRuleItem) {
    testRuleId.value = item.id;
    testRuleName.value = item.name;
    testText.value = '';
    testResult.value = null;
    showTestDialog.value = true;
}

async function runTest() {
    if (!testText.value) return;
    testing.value = true;
    error.value = null;
    try {
        const result = requireApiSuccess<CategoryRuleTestResult>(
            await services.testCategoryRule(testRuleId.value, testText.value),
            tt('Test failed')
        );
        testResult.value = !!result?.matched;
    } catch (e: unknown) {
        testResult.value = null;
        error.value = getRequestErrorMessage(e, tt('Test failed'));
    } finally {
        testing.value = false;
    }
}

// ── Migrate ────────
async function migrateKeywords() {
    loading.value = true;
    error.value = null;
    try {
        const result = requireApiSuccess<CategoryKeywordMigrationResult>(
            await services.migrateCategoryKeywords(),
            tt('Migration failed')
        );
        const migrated = getMigrationCount(result?.migrated ?? result?.migrated_count);
        const skipped = getMigrationCount(result?.skipped ?? result?.skipped_count);
        showSuccessMessage('Migration completed: migrated {migrated}, skipped {skipped}', {
            migrated,
            skipped,
        });
        categoryStore.updateTransactionCategoryListInvalidState(true);
        await fetchAll();
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, tt('Migration failed'));
    } finally {
        loading.value = false;
    }
}

// ── Data fetching ────────
async function fetchCategoryRules() {
    try {
        const response = await axios.get<{
            success?: boolean;
            data?: Array<CategoryRuleItem & {
                main_category?: string | null;
                sub_category?: string | null;
            }>;
            error?: string;
        }>('category-rules/', {
            params: {
                enabled_only: false,
            },
        });
        if (!response.data?.success) {
            throw new Error(response.data?.error || tt('Failed to load category rules'));
        }
        const result = response.data.data ?? [];
        categoryRules.value = result.map(item => ({
            ...item,
            category_name: item.category_name ?? item.main_category ?? null,
            sub_category_name: item.sub_category_name ?? item.sub_category ?? null,
        }));
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, tt('Failed to load category rules'));
    }
}

async function fetchOverview() {
    try {
        const result = requireApiSuccess<Overview>(
            await services.getRulesOverview(),
            tt('Failed to load rules overview')
        );
        if (result) {
            overview.value = {
                learningRules: result.learningRules ?? [],
                learningRuleCount: result.learningRuleCount ?? 0,
                categoryRuleCount: result.categoryRuleCount ?? 0,
                recurringRules: result.recurringRules ?? [],
                recurringRuleCount: result.recurringRuleCount ?? 0,
                totalRuleCount: result.totalRuleCount ?? 0,
            };
        }
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, tt('Failed to load rules overview'));
    }
}

async function fetchAll() {
    loading.value = true;
    error.value = null;
    try {
        await Promise.all([
            fetchCategoryRules(),
            fetchOverview(),
            categoryStore.loadAllCategories({ force: false }),
        ]);
    } finally {
        loading.value = false;
    }
}

watch(
    () => [props.initTab, props.tabs] as const,
    ([initTab]) => {
        activeTab.value = normalizeTab(initTab);
    },
    { immediate: true }
);

onMounted(() => fetchAll());
</script>

<style scoped>
.rule-center-category-filter {
    flex: 0 1 280px;
    min-width: 220px;
}
</style>
