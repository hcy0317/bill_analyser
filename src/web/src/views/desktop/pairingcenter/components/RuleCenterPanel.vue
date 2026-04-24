<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-title v-if="!props.hideHeader" class="d-flex align-center">
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
                        <div class="rule-center-table-toolbar d-flex flex-column flex-xl-row align-xl-center pa-4 ga-3">
                            <div class="rule-center-primary-actions d-flex align-center flex-wrap ga-2">
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
                            </div>
                            <v-spacer />
                            <div class="rule-center-filter-toolbar d-flex align-center flex-wrap ga-2">
                                <v-chip size="small" variant="tonal">
                                    {{ rulePaginationLabel }}
                                </v-chip>
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
                                        :label="tt('Filter Category')"
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
                                <div class="rule-center-page-size">
                                    <v-select
                                        v-model="ruleItemsPerPage"
                                        :items="rulePageSizeOptions"
                                        density="compact"
                                        variant="outlined"
                                        hide-details
                                        :label="tt('Rows')"
                                    />
                                </div>
                            </div>
                        </div>
                        <v-table class="rule-center-rules-table" density="compact" hover>
                            <thead>
                            <tr>
                                <th class="rule-center-column-category">{{ tt('Category') }}</th>
                                <th class="rule-center-column-expression">{{ tt('Expression') }}</th>
                                <th class="rule-center-column-regex text-no-wrap">{{ tt('Regex') }}</th>
                                <th class="rule-center-column-enabled text-no-wrap">{{ tt('Enabled') }}</th>
                                <th class="rule-center-column-applied text-no-wrap">{{ tt('Applied') }}</th>
                                <th class="rule-center-column-actions text-no-wrap">{{ tt('Actions') }}</th>
                            </tr>
                            </thead>
                            <tbody v-if="groupedCategoryRules.length > 0">
                            <template v-for="group in groupedCategoryRules" :key="group.key">
                                <tr class="rule-center-group-row">
                                    <td colspan="6">
                                        <div class="d-flex align-center ga-2">
                                            <ItemIcon
                                                v-if="group.icon && group.color"
                                                icon-type="category"
                                                size="22px"
                                                :icon-id="group.icon"
                                                :color="group.color"
                                            />
                                            <v-icon v-else size="22" :icon="mdiCloseCircle" color="grey" />
                                            <span class="font-weight-medium">{{ group.title }}</span>
                                            <v-chip size="x-small" variant="tonal">{{ group.items.length }}</v-chip>
                                        </div>
                                    </td>
                                </tr>
                                <tr v-for="item in group.items" :key="item.id">
                                    <td class="rule-center-column-category">
                                        <div class="d-flex align-center" :title="item.category_full_name">
                                            <ItemIcon
                                                v-if="item.category_icon && item.category_color"
                                                icon-type="category"
                                                size="24px"
                                                :icon-id="item.category_icon"
                                                :color="item.category_color"
                                            />
                                            <v-icon v-else size="24" :icon="mdiCloseCircle" color="grey" />
                                            <span class="ms-2 text-truncate">{{ item.category_display_name }}</span>
                                        </div>
                                    </td>
                                    <td class="rule-center-column-expression">
                                        <div class="rule-expression-stack">
                                            <div
                                                v-for="(expressionGroup, expressionIndex) in getRuleExpressionGroups(item)"
                                                :key="`${item.id}-${expressionIndex}`"
                                                class="rule-expression-line"
                                            >
                                                <template
                                                    v-for="(clause, clauseIndex) in expressionGroup.clauses"
                                                    :key="`${item.id}-${expressionIndex}-${clauseIndex}`"
                                                >
                                                    <v-chip
                                                        class="rule-expression-operator"
                                                        size="x-small"
                                                        label
                                                        variant="tonal"
                                                        :color="clause.operator === 'NOT' ? 'warning' : clause.operator === 'REGEX' ? 'info' : 'primary'"
                                                    >
                                                        {{ clause.label }}
                                                    </v-chip>
                                                    <v-chip
                                                        v-for="term in getVisibleExpressionTerms(item.id, expressionIndex, clauseIndex, clause.terms)"
                                                        :key="term"
                                                        size="x-small"
                                                        variant="tonal"
                                                        color="primary"
                                                        class="rule-expression-term"
                                                        :title="term"
                                                    >
                                                        {{ term }}
                                                    </v-chip>
                                                    <v-chip
                                                        v-if="getHiddenExpressionTermCount(item.id, expressionIndex, clauseIndex, clause.terms) > 0"
                                                        size="x-small"
                                                        label
                                                        variant="outlined"
                                                        class="rule-expression-more"
                                                        @click="toggleExpressionClauseExpanded(item.id, expressionIndex, clauseIndex)"
                                                    >
                                                        +{{ getHiddenExpressionTermCount(item.id, expressionIndex, clauseIndex, clause.terms) }}
                                                    </v-chip>
                                                </template>
                                            </div>
                                        </div>
                                    </td>
                                    <td class="rule-center-column-regex text-no-wrap">
                                        <v-icon
                                            :icon="item.regex_enabled ? mdiCheckCircle : mdiCloseCircle"
                                            :color="item.regex_enabled ? 'info' : 'grey'"
                                            size="small"
                                        />
                                    </td>
                                    <td class="rule-center-column-enabled">
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
                                    <td class="rule-center-column-applied">{{ item.applied_count }}</td>
                                    <td class="rule-center-column-actions">
                                        <div class="rule-center-actions-row">
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
                            </template>
                            </tbody>
                            <tbody v-else>
                            <tr>
                                <td colspan="6" class="text-center text-medium-emphasis py-8">
                                    {{ tt('No category rules') }}
                                </td>
                            </tr>
                            </tbody>
                        </v-table>
                        <div
                            v-if="filteredDisplayCategoryRules.length > 0"
                            class="rule-center-pagination d-flex flex-column flex-sm-row align-sm-center justify-space-between ga-3 px-4 py-3"
                        >
                            <div class="text-caption text-medium-emphasis">
                                {{ rulePaginationLabel }}
                            </div>
                            <v-pagination
                                v-model="rulePage"
                                :length="rulePageCount"
                                :total-visible="5"
                                density="comfortable"
                                size="small"
                            />
                        </div>
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
                        <v-card-text>
                            <div class="d-flex flex-column flex-md-row align-md-center ga-3">
                                <div class="text-body-2 text-medium-emphasis">
                                    {{ tt('Recurring matching uses scheduled templates. Manage the templates in Scheduled Templates, or review newly detected recurring bills in discovery.') }}
                                </div>
                                <v-spacer />
                                <v-btn
                                    variant="outlined"
                                    :prepend-icon="mdiTextBoxEditOutline"
                                    :to="{ path: '/schedule/list' }"
                                >
                                    {{ tt('Manage Scheduled Templates') }}
                                </v-btn>
                                <v-btn
                                    variant="tonal"
                                    :prepend-icon="mdiCalendarSearch"
                                    :to="{ path: '/recurring/discover' }"
                                >
                                    {{ tt('Review Recurring Suggestions') }}
                                </v-btn>
                            </div>
                            <v-alert
                                class="mt-4"
                                variant="tonal"
                                type="info"
                                density="compact"
                            >
                                {{ tt('This panel does not edit scheduled-template matches directly. Current scheduled templates') }}:
                                {{ overview.recurringRuleCount }}
                            </v-alert>
                        </v-card-text>
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
    mdiTestTube, mdiDatabaseImportOutline, mdiCalendarSearch, mdiTextBoxEditOutline,
} from '@mdi/js';
import type { ApiResponse, ErrorResponse } from '@/core/api.ts';
import services from '@/lib/services.ts';
import { useI18n } from '@/locales/helpers.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { CategoryType } from '@/core/category.ts';
import { parseExpression } from '@/components/common/keywordExpression.ts';
import type { RuleClause, RuleOperator } from '@/components/common/keywordExpression.ts';
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
    hideHeader?: boolean;
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
    category_group_key: string;
    category_group_name: string;
    category_group_icon: string;
    category_group_color: string;
}

interface CategoryRuleGroup {
    key: string;
    title: string;
    icon: string;
    color: string;
    items: DisplayCategoryRuleItem[];
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

interface RuleExpressionDisplayClause {
    label: string;
    operator: RuleOperator | 'RAW';
    terms: string[];
}

interface RuleExpressionDisplayGroup {
    clauses: RuleExpressionDisplayClause[];
}

const categoryRules = ref<CategoryRuleItem[]>([]);
const togglingRuleIds = ref<number[]>([]);
const expandedExpressionClauseKeys = ref<string[]>([]);
const maxCollapsedExpressionTerms = 4;
const rulePageSizeOptions = [10, 20, 50, 100];
const ruleItemsPerPage = ref<number>(20);
const rulePage = ref(1);

const learningHeaders = computed(() => [
    { title: tt('Match Type'), key: 'matchType' },
    { title: tt('Match Value'), key: 'matchValue' },
    { title: tt('Learned Type'), key: 'learnedType' },
    { title: tt('Applied'), key: 'appliedCount' },
    { title: tt('Enabled'), key: 'enabled' },
]);

const displayCategoryRules = computed<DisplayCategoryRuleItem[]>(() => categoryRules.value.map(item => {
    const display = resolveCategoryDisplay(item);
    const group = resolveCategoryGroup(item);
    return {
        ...item,
        category_display_name: display.name,
        category_full_name: display.fullName,
        category_icon: display.icon,
        category_color: display.color,
        category_group_key: group.key,
        category_group_name: group.name,
        category_group_icon: group.icon,
        category_group_color: group.color,
    };
}));

const groupedCategoryRules = computed<CategoryRuleGroup[]>(() => {
    const groupsByKey = new Map<string, CategoryRuleGroup>();

    for (const item of paginatedDisplayCategoryRules.value) {
        const group = groupsByKey.get(item.category_group_key) ?? {
            key: item.category_group_key,
            title: item.category_group_name,
            icon: item.category_group_icon,
            color: item.category_group_color,
            items: [],
        };
        group.items.push(item);
        groupsByKey.set(item.category_group_key, group);
    }

    return [...groupsByKey.values()]
        .map(group => ({
            ...group,
            items: [...group.items].sort(compareDisplayCategoryRules),
        }))
        .sort((firstGroup, secondGroup) => firstGroup.title.localeCompare(secondGroup.title, 'zh-Hans'));
});

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

function resolveCategoryGroup(item: CategoryRuleItem): { key: string; name: string; icon: string; color: string } {
    const categoryId = item.category_id !== null && item.category_id !== undefined
        ? String(item.category_id)
        : '';
    const category = categoryId ? categoryStore.allTransactionCategoriesMap[categoryId] : null;
    const primaryCategory = category && category.parentId && category.parentId !== '0'
        ? categoryStore.allTransactionCategoriesMap[category.parentId]
        : category;

    if (primaryCategory) {
        return {
            key: `category:${primaryCategory.id}`,
            name: primaryCategory.name,
            icon: primaryCategory.icon,
            color: String(primaryCategory.color),
        };
    }

    const fallbackName = item.category_name || item.sub_category_name || tt('Unassigned Category');
    return {
        key: fallbackName ? `category-name:${fallbackName}` : 'unassigned',
        name: fallbackName || tt('Unassigned Category'),
        icon: '',
        color: '',
    };
}

function compareDisplayCategoryRules(firstRule: DisplayCategoryRuleItem, secondRule: DisplayCategoryRuleItem): number {
    return firstRule.category_full_name.localeCompare(secondRule.category_full_name, 'zh-Hans')
        || firstRule.name.localeCompare(secondRule.name, 'zh-Hans')
        || firstRule.id - secondRule.id;
}

function compareDisplayCategoryRuleOrder(
    firstRule: DisplayCategoryRuleItem,
    secondRule: DisplayCategoryRuleItem
): number {
    return firstRule.category_group_name.localeCompare(secondRule.category_group_name, 'zh-Hans')
        || firstRule.category_group_key.localeCompare(secondRule.category_group_key, 'zh-Hans')
        || compareDisplayCategoryRules(firstRule, secondRule);
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
    return `${selectionLabel} · ${tt('Category Rule')}`;
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

const orderedFilteredDisplayCategoryRules = computed<DisplayCategoryRuleItem[]>(() => (
    [...filteredDisplayCategoryRules.value].sort(compareDisplayCategoryRuleOrder)
));
const rulePageCount = computed(() => Math.max(
    1,
    Math.ceil(orderedFilteredDisplayCategoryRules.value.length / ruleItemsPerPage.value)
));
const paginatedDisplayCategoryRules = computed<DisplayCategoryRuleItem[]>(() => {
    const startIndex = (rulePage.value - 1) * ruleItemsPerPage.value;
    return orderedFilteredDisplayCategoryRules.value.slice(startIndex, startIndex + ruleItemsPerPage.value);
});
const rulePaginationStart = computed(() => (
    filteredDisplayCategoryRules.value.length === 0
        ? 0
        : (rulePage.value - 1) * ruleItemsPerPage.value + 1
));
const rulePaginationEnd = computed(() => Math.min(
    filteredDisplayCategoryRules.value.length,
    rulePage.value * ruleItemsPerPage.value
));
const rulePaginationLabel = computed(() => {
    if (filteredDisplayCategoryRules.value.length === 0) {
        return `0 / ${categoryRules.value.length}`;
    }

    return `${rulePaginationStart.value}-${rulePaginationEnd.value} / ${filteredDisplayCategoryRules.value.length}`;
});

function clearRuleCategoryFilter(): void {
    ruleCategoryFilterId.value = '';
}

function getRuleExpressionGroups(rule: CategoryRuleItem): RuleExpressionDisplayGroup[] {
    const expression = String(rule.rule_expression || '').trim();
    const parsedExpression = parseExpression(expression, { format: 'composite' });

    if (parsedExpression.clauses.length < 1) {
        return [{
            clauses: [{
                label: parsedExpression.sourceFormat === 'empty' ? tt('Empty') : tt('Raw'),
                operator: 'RAW',
                terms: [parsedExpression.rawExpression || expression || tt('Empty')],
            }],
        }];
    }

    const groups: RuleExpressionDisplayGroup[] = [];
    let currentClauses: RuleExpressionDisplayClause[] = [];
    let parenthesisDepth = 0;

    parsedExpression.clauses.forEach((clause, index) => {
        if (index > 0 && clause.joiner === 'OR' && parenthesisDepth === 0 && currentClauses.length > 0) {
            groups.push({ clauses: currentClauses });
            currentClauses = [];
        }

        currentClauses.push(toExpressionDisplayClause(clause, currentClauses.length === 0));
        parenthesisDepth = Math.max(0, parenthesisDepth + clause.openParens - clause.closeParens);
    });

    if (currentClauses.length > 0) {
        groups.push({ clauses: currentClauses });
    }

    return groups;
}

function toExpressionDisplayClause(clause: RuleClause, isFirstClause: boolean): RuleExpressionDisplayClause {
    const connector = isFirstClause ? '' : clause.joiner === 'OR' ? '/ ' : '+ ';
    return {
        label: `${connector}${clause.operator}`,
        operator: clause.operator,
        terms: clause.terms.length > 0 ? clause.terms : [tt('Empty')],
    };
}

function makeExpressionClauseKey(ruleId: number, expressionIndex: number, clauseIndex: number): string {
    return `${ruleId}:${expressionIndex}:${clauseIndex}`;
}

function isExpressionClauseExpanded(ruleId: number, expressionIndex: number, clauseIndex: number): boolean {
    return expandedExpressionClauseKeys.value.includes(makeExpressionClauseKey(ruleId, expressionIndex, clauseIndex));
}

function toggleExpressionClauseExpanded(ruleId: number, expressionIndex: number, clauseIndex: number): void {
    const key = makeExpressionClauseKey(ruleId, expressionIndex, clauseIndex);
    expandedExpressionClauseKeys.value = expandedExpressionClauseKeys.value.includes(key)
        ? expandedExpressionClauseKeys.value.filter(item => item !== key)
        : [...expandedExpressionClauseKeys.value, key];
}

function getVisibleExpressionTerms(
    ruleId: number,
    expressionIndex: number,
    clauseIndex: number,
    terms: string[]
): string[] {
    if (isExpressionClauseExpanded(ruleId, expressionIndex, clauseIndex)) {
        return terms;
    }

    return terms.slice(0, maxCollapsedExpressionTerms);
}

function getHiddenExpressionTermCount(
    ruleId: number,
    expressionIndex: number,
    clauseIndex: number,
    terms: string[]
): number {
    if (isExpressionClauseExpanded(ruleId, expressionIndex, clauseIndex)) {
        return 0;
    }

    return Math.max(0, terms.length - maxCollapsedExpressionTerms);
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
        await refreshRuleTables();
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
    if (normalizedNextEnabled === item.enabled) {
        return;
    }

    const previousEnabled = item.enabled;
    error.value = null;
    setRuleToggling(item.id, true);
    categoryRules.value = categoryRules.value.map(rule => rule.id === item.id
        ? { ...rule, enabled: normalizedNextEnabled }
        : rule
    );
    try {
        requireApiSuccess(
            await services.updateCategoryRule(item.id, { enabled: normalizedNextEnabled }),
            tt('Failed to toggle rule')
        );
        await refreshRuleTables();
    } catch (e: unknown) {
        categoryRules.value = categoryRules.value.map(rule => rule.id === item.id
            ? { ...rule, enabled: previousEnabled }
            : rule
        );
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
        await refreshRuleTables();
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
            regex_enabled: !!item.regex_enabled,
            enabled: !!item.enabled,
            applied_count: Number(item.applied_count ?? 0),
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

async function refreshRuleTables() {
    await Promise.all([
        fetchCategoryRules(),
        fetchOverview(),
    ]);
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

watch(ruleCategoryFilterId, () => {
    rulePage.value = 1;
});

watch(ruleItemsPerPage, () => {
    rulePage.value = 1;
});

watch(
    () => filteredDisplayCategoryRules.value.length,
    () => {
        if (rulePage.value > rulePageCount.value) {
            rulePage.value = rulePageCount.value;
        }
    }
);

onMounted(() => fetchAll());

defineExpose({
    refresh: fetchAll,
});
</script>

<style scoped>
.rule-center-category-filter {
    flex: 1 1 320px;
    min-width: 240px;
}

.rule-center-primary-actions {
    flex: 0 0 auto;
}

.rule-center-filter-toolbar {
    flex: 1 1 560px;
    justify-content: flex-end;
}

.rule-center-page-size {
    flex: 0 0 112px;
}

.rule-center-rules-table {
    table-layout: fixed;
}

.rule-center-rules-table :deep(th),
.rule-center-rules-table :deep(td) {
    vertical-align: middle;
}

.rule-center-rules-table :deep(th) {
    white-space: nowrap;
}

.rule-center-group-row {
    background: rgba(var(--v-theme-surface-variant), 0.46);
}

.rule-center-group-row td {
    padding-block: 8px;
}

.rule-center-column-category {
    width: 190px;
}

.rule-center-column-expression {
    min-width: 320px;
}

.rule-center-column-regex {
    width: 108px;
    text-align: center;
}

.rule-center-column-enabled {
    width: 104px;
    text-align: center;
}

.rule-center-column-applied {
    width: 84px;
    text-align: center;
}

.rule-center-column-actions {
    width: 132px;
    text-align: center;
}

.rule-center-actions-row {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
    flex-wrap: nowrap;
}

.rule-center-pagination {
    border-top: 1px solid rgba(var(--v-theme-on-surface), 0.08);
}

.rule-expression-stack {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-block: 6px;
}

.rule-expression-line {
    display: flex;
    align-items: center;
    gap: 6px;
    width: fit-content;
    max-width: 100%;
    padding: 4px 6px;
    border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
    border-radius: 8px;
    background: rgba(var(--v-theme-surface), 1);
    overflow: hidden;
}

.rule-expression-operator {
    flex: 0 0 auto;
}

.rule-expression-term {
    flex: 0 1 auto;
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border: 1px solid rgba(var(--v-theme-primary), 0.18);
}

.rule-expression-term :deep(.v-chip__content) {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.rule-expression-more {
    cursor: pointer;
    flex: 0 0 auto;
}

@media (max-width: 960px) {
    .rule-center-filter-toolbar {
        justify-content: flex-start;
        flex-basis: auto;
    }

    .rule-center-rules-table {
        table-layout: auto;
    }
}
</style>
