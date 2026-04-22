<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-title class="d-flex align-center">
                    <v-icon :icon="mdiBookCogOutline" class="me-2" />
                    <span>{{ tt('Rule Center') }}</span>
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

                <v-alert v-if="successMsg" type="success" closable class="ma-4" @click:close="successMsg = null">
                    {{ successMsg }}
                </v-alert>

                <v-sheet border rounded="lg" class="mx-4 mb-4 pa-4">
                    <div class="d-flex flex-column flex-lg-row align-lg-center ga-3">
                        <div class="min-w-0">
                            <div class="d-flex flex-wrap align-center ga-2">
                                <v-icon :icon="mdiLinkVariant" size="small" />
                                <span class="text-subtitle-1 font-weight-medium">
                                    {{ tt('Temporary compatibility entry') }}
                                </span>
                                <v-chip size="small" color="warning" variant="tonal">
                                    {{ tt('Legacy access point') }}
                                </v-chip>
                            </div>
                            <div class="text-body-2 text-medium-emphasis mt-2">
                                {{ tt('Rule Center stays available for existing deep links during migration. Use Pairing Center as the main home for pairing tools.') }}
                            </div>
                        </div>
                        <v-spacer />
                        <v-btn color="primary" variant="tonal" to="/pairing/list">
                            <v-icon start :icon="mdiOpenInNew" />
                            {{ tt('Open Pairing Center') }}
                        </v-btn>
                    </div>
                </v-sheet>

                <v-tabs v-model="activeTab" class="px-4">
                    <v-tab value="rules">
                        <v-icon start :icon="mdiBookCogOutline" />
                        {{ tt('Category Rules') }} ({{ categoryRules.length }})
                    </v-tab>
                    <v-tab value="learning">
                        <v-icon start :icon="mdiBrain" />
                        {{ tt('Learning Rules') }} ({{ overview.learningRuleCount }})
                    </v-tab>
                    <v-tab value="investment">
                        <v-icon start :icon="mdiFinance" />
                        {{ tt('Investment Settings') }}
                    </v-tab>
                    <v-tab value="recurring">
                        <v-icon start :icon="mdiCalendarSync" />
                        {{ tt('Recurring Rules') }} ({{ overview.recurringRuleCount }})
                    </v-tab>
                </v-tabs>

                <v-tabs-window v-model="activeTab">
                    <!-- Category Rules -->
                    <v-tabs-window-item value="rules">
                        <div class="d-flex align-center pa-4 ga-2">
                            <v-btn color="primary" :prepend-icon="mdiPlus" @click="openCreateDialog">
                                {{ tt('Add Rule') }}
                            </v-btn>
                            <v-btn variant="outlined" :prepend-icon="mdiDatabaseImportOutline" @click="migrateKeywords">
                                {{ tt('Migrate from Keywords') }}
                            </v-btn>
                        </div>
                        <v-data-table
                            :headers="ruleHeaders"
                            :items="categoryRules"
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
                            <template #item.category_name="{ item }">
                                <span>{{ item.category_name || '-' }}</span>
                                <span v-if="item.sub_category_name" class="text-grey ms-1">
                                    / {{ item.sub_category_name }}
                                </span>
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
                                    @update:model-value="toggleEnabled(item)"
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
                    <v-tabs-window-item value="learning">
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

                    <!-- Investment Settings -->
                    <v-tabs-window-item value="investment">
                        <v-card-text>
                            <div class="d-flex align-center mb-4">
                                <span class="text-subtitle-1 font-weight-medium">{{ tt('Investment Recognition Settings') }}</span>
                                <v-spacer />
                                <v-btn variant="outlined" size="small" @click="goToInvestmentSettings">
                                    <v-icon start :icon="mdiPencilOutline" />
                                    {{ tt('Manage') }}
                                </v-btn>
                            </div>

                            <v-row>
                                <v-col cols="12" md="4">
                                    <v-card variant="outlined">
                                        <v-card-subtitle class="pt-3">{{ tt('Platform Keywords') }}</v-card-subtitle>
                                        <v-card-text>
                                            <v-chip v-for="kw in investmentSettingsSummary.platform" :key="kw"
                                                    size="small" class="ma-1" color="green" variant="tonal">
                                                {{ kw }}
                                            </v-chip>
                                            <span v-if="investmentSettingsSummary.platform.length === 0" class="text-grey text-caption">
                                                {{ tt('Using defaults') }}
                                            </span>
                                        </v-card-text>
                                    </v-card>
                                </v-col>
                                <v-col cols="12" md="4">
                                    <v-card variant="outlined">
                                        <v-card-subtitle class="pt-3">{{ tt('Product Keywords') }}</v-card-subtitle>
                                        <v-card-text>
                                            <v-chip v-for="kw in investmentSettingsSummary.product" :key="kw"
                                                    size="small" class="ma-1" color="blue" variant="tonal">
                                                {{ kw }}
                                            </v-chip>
                                            <span v-if="investmentSettingsSummary.product.length === 0" class="text-grey text-caption">
                                                {{ tt('Using defaults') }}
                                            </span>
                                        </v-card-text>
                                    </v-card>
                                </v-col>
                                <v-col cols="12" md="4">
                                    <v-card variant="outlined">
                                        <v-card-subtitle class="pt-3">{{ tt('Exclude Keywords') }}</v-card-subtitle>
                                        <v-card-text>
                                            <v-chip v-for="kw in investmentSettingsSummary.exclude" :key="kw"
                                                    size="small" class="ma-1" color="red" variant="tonal">
                                                {{ kw }}
                                            </v-chip>
                                            <span v-if="investmentSettingsSummary.exclude.length === 0" class="text-grey text-caption">
                                                {{ tt('Using defaults') }}
                                            </span>
                                        </v-card-text>
                                    </v-card>
                                </v-col>
                            </v-row>

                            <v-alert type="info" variant="tonal" class="mt-4" density="compact">
                                {{ tt('Rule Center keeps a read-only compatibility summary here. Manage the canonical investment recognition settings in Pairing Center.') }}
                            </v-alert>
                        </v-card-text>
                    </v-tabs-window-item>

                    <!-- Recurring Rules -->
                    <v-tabs-window-item value="recurring">
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
                    <v-text-field
                        v-model="ruleForm.name"
                        :label="tt('Name')"
                        :rules="[v => !!v || tt('Name is required')]"
                        class="mb-2"
                    />
                    <v-autocomplete
                        v-model="ruleForm.category_id"
                        :label="tt('Category')"
                        :items="categoryOptions"
                        item-title="text"
                        item-value="value"
                        :rules="[v => !!v || tt('Category is required')]"
                        class="mb-2"
                    />
                    <v-text-field
                        v-model.number="ruleForm.priority"
                        :label="tt('Priority')"
                        type="number"
                        :hint="tt('Lower number = higher priority')"
                        persistent-hint
                        class="mb-2"
                    />
                    <v-textarea
                        v-model="ruleForm.rule_expression"
                        :label="tt('Rule Expression')"
                        hint="OR={k1,k2}+AND={k3}+NOT={k4}"
                        persistent-hint
                        rows="3"
                        :rules="[v => !!v || tt('Expression is required')]"
                        class="mb-2"
                    />
                    <v-checkbox v-model="ruleForm.regex_enabled" :label="tt('Enable Regex')" hide-details />
                    <v-checkbox v-model="ruleForm.enabled" :label="tt('Enabled')" hide-details />
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
                    placeholder="Enter description or counterparty text..."
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
</template>

<script setup lang="ts">
import axios from 'axios';
import { ref, computed, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import {
    mdiBookCogOutline, mdiRefresh, mdiBrain, mdiCalendarSync,
    mdiCheckCircle, mdiCloseCircle, mdiPlus, mdiPencilOutline, mdiDeleteOutline,
    mdiTestTube, mdiDatabaseImportOutline, mdiFinance, mdiLinkVariant, mdiOpenInNew,
} from '@mdi/js';
import type { ApiResponse, ErrorResponse } from '@/core/api.ts';
import services from '@/lib/services.ts';
import { useI18n } from '@/locales/helpers.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useUserStore } from '@/stores/user.ts';

const router = useRouter();
const categoryStore = useTransactionCategoriesStore();
const userStore = useUserStore();
const { tt } = useI18n();

const loading = ref(false);
const saving = ref(false);
const deleting = ref(false);
const testing = ref(false);
const error = ref<string | null>(null);
const successMsg = ref<string | null>(null);
const activeTab = ref('rules');

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

interface CategoryRuleForm {
    name: string;
    category_id: number | null;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
}

interface CategoryRuleTestResult {
    matched?: boolean | null;
}

interface CategoryKeywordMigrationResult {
    migrated_count?: number | string | null;
}

const categoryRules = ref<CategoryRuleItem[]>([]);

const ruleHeaders = computed(() => [
    { title: tt('Priority'), key: 'priority', sortable: true },
    { title: tt('Name'), key: 'name' },
    { title: tt('Category'), key: 'category_name' },
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

// ── Investment settings summary (read from user profile) ────────
const investmentSettingsSummary = computed(() => {
    const info = userStore.currentUserBasicInfo;
    return {
        platform: info?.investmentPlatformKeywords || [],
        product: info?.investmentProductKeywords || [],
        exclude: info?.investmentExcludeKeywords || [],
    };
});

function goToInvestmentSettings() {
    router.push('/pairing/list?view=investment-settings');
}

// ── Category selector options ────────
const categoryOptions = computed(() => {
    const options: { text: string; value: number }[] = [];
    const catMap = categoryStore.allTransactionCategoriesMap;
    for (const key of Object.keys(catMap)) {
        const cat = catMap[key];
        if (cat) {
            options.push({ text: cat.name, value: Number(cat.id) });
        }
    }
    return options;
});

// ── Edit dialog state ────────
const showEditDialog = ref(false);
const editingRule = ref<CategoryRuleItem | null>(null);
const ruleFormRef = ref<unknown>(null);
const ruleForm = ref<CategoryRuleForm>({
    name: '',
    category_id: null as number | null,
    priority: 100,
    rule_expression: '',
    regex_enabled: false,
    enabled: true,
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

function buildCategoryRuleCreatePayload(form: CategoryRuleForm): CategoryRuleForm & { category_id: number } {
    if (form.category_id == null) {
        throw new Error('Category is required');
    }

    return {
        ...form,
        category_id: form.category_id,
    };
}

function openCreateDialog() {
    editingRule.value = null;
    ruleForm.value = { name: '', category_id: null, priority: 100, rule_expression: '', regex_enabled: false, enabled: true };
    showEditDialog.value = true;
}

function openEditDialog(item: CategoryRuleItem) {
    editingRule.value = item;
    ruleForm.value = {
        name: item.name,
        category_id: item.category_id,
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
        if (editingRule.value) {
            requireApiSuccess(
                await services.updateCategoryRule(editingRule.value.id, ruleForm.value),
                'Failed to save rule'
            );
            successMsg.value = 'Rule updated';
        } else {
            requireApiSuccess(
                await services.createCategoryRule(buildCategoryRuleCreatePayload(ruleForm.value)),
                'Failed to save rule'
            );
            successMsg.value = 'Rule created';
        }
        showEditDialog.value = false;
        await fetchCategoryRules();
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, 'Failed to save rule');
    } finally {
        saving.value = false;
    }
}

async function toggleEnabled(item: CategoryRuleItem) {
    error.value = null;
    try {
        requireApiSuccess(
            await services.updateCategoryRule(item.id, { enabled: !item.enabled }),
            'Failed to toggle rule'
        );
        await fetchCategoryRules();
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, 'Failed to toggle rule');
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
            'Failed to delete rule'
        );
        showDeleteDialog.value = false;
        successMsg.value = 'Rule deleted';
        await fetchCategoryRules();
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, 'Failed to delete rule');
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
            'Test failed'
        );
        testResult.value = !!result?.matched;
    } catch (e: unknown) {
        testResult.value = null;
        error.value = getRequestErrorMessage(e, 'Test failed');
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
            'Migration failed'
        );
        const count = Number(result?.migrated_count ?? 0);
        successMsg.value = `Migrated ${count} keyword(s) to category rules`;
        await fetchAll();
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, 'Migration failed');
    } finally {
        loading.value = false;
    }
}

// ── Data fetching ────────
async function fetchCategoryRules() {
    try {
        const result = requireApiSuccess<CategoryRuleItem[]>(
            await services.getCategoryRules(),
            'Failed to load category rules'
        );
        categoryRules.value = result ?? [];
    } catch (e: unknown) {
        error.value = getRequestErrorMessage(e, 'Failed to load category rules');
    }
}

async function fetchOverview() {
    try {
        const result = requireApiSuccess<Overview>(
            await services.getRulesOverview(),
            'Failed to load rules overview'
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
        error.value = getRequestErrorMessage(e, 'Failed to load rules overview');
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

onMounted(() => fetchAll());
</script>
