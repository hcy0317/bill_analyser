<template>
    <v-dialog width="800" :persistent="isCategoryModified" v-model="showState">
        <v-card class="pa-2 pa-sm-4 pa-md-8">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <h4 class="text-h4">{{ tt(title) }}</h4>
                    <v-progress-circular indeterminate size="22" class="ms-2" v-if="loading"></v-progress-circular>
                </div>
            </template>
            <v-card-text class="pt-0">
                <v-form class="mt-md-6">
                    <v-row>
                        <v-col cols="12" md="12">
                            <v-text-field
                                type="text"
                                persistent-placeholder
                                :disabled="loading || submitting"
                                :label="tt('Category Name')"
                                :placeholder="tt('Category Name')"
                                v-model="category.name"
                            />
                        </v-col>
                        <v-col cols="12" md="12" v-if="editCategoryId && category.parentId && category.parentId !== '0'">
                            <v-select
                                item-title="name"
                                item-value="id"
                                persistent-placeholder
                                :disabled="loading || submitting"
                                :label="tt('Primary Category')"
                                :placeholder="tt('Primary Category')"
                                :items="allAvailableCategories"
                                :no-data-text="tt('No available primary category')"
                                v-model="category.parentId"
                            >
                                <template #item="{ props, item }">
                                    <v-list-item v-bind="props">
                                        <template #prepend>
                                            <ItemIcon class="me-2" icon-type="category"
                                                      :icon-id="item.raw.icon" :color="item.raw.color"></ItemIcon>
                                        </template>
                                        <template #title>
                                            <div class="text-truncate">{{ item.raw.name }}</div>
                                        </template>
                                    </v-list-item>
                                </template>
                            </v-select>
                        </v-col>
                        <v-col cols="12" md="6">
                            <icon-select icon-type="category"
                                         :all-icon-infos="ALL_CATEGORY_ICONS"
                                          :label="tt('Category Icon')"
                                          :color="category.color"
                                          :disabled="loading || submitting"
                                          v-model="category.icon" />
                        </v-col>
                        <v-col cols="12" md="6">
                            <color-select :all-color-infos="ALL_CATEGORY_COLORS"
                                         :label="tt('Category Color')"
                                         :disabled="loading || submitting"
                                         v-model="category.color" />
                        </v-col>
                        <v-col cols="12" md="12">
                            <v-text-field
                                type="number"
                                persistent-placeholder
                                :disabled="loading || submitting"
                                :label="tt('Priority')"
                                :placeholder="tt('Priority (smaller number means higher priority)')"
                                v-model.number="category.displayOrder"
                            />
                        </v-col>
                        <v-col cols="12" md="12" v-if="isSecondaryCategory">
                            <v-progress-linear v-if="ruleLoading" indeterminate color="primary" class="mb-3" />
                            <category-rule-builder-fields
                                v-model="categoryRuleBuilderModel"
                                :auto-rule-name="autoPrimaryRuleName"
                                :disabled="loading || submitting || ruleLoading"
                                title="Category Matching"
                            />
                        </v-col>
                        <v-col cols="12" md="12">
                            <v-textarea
                                type="text"
                                persistent-placeholder
                                rows="3"
                                :disabled="loading || submitting"
                                :label="tt('Description')"
                                :placeholder="tt('Your category description (optional)')"
                                v-model="category.comment"
                            />
                        </v-col>
                        <v-col class="py-0" cols="12" md="12" v-if="editCategoryId">
                            <v-switch :disabled="loading || submitting"
                                      :label="tt('Visible')" v-model="category.visible"/>
                        </v-col>
                    </v-row>
                </v-form>
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

    <snack-bar ref="snackbar" />
</template>

<script setup lang="ts">
import axios from 'axios';
import ColorSelect from '@/components/desktop/ColorSelect.vue';
import IconSelect from '@/components/desktop/IconSelect.vue';
import CategoryRuleBuilderFields from '@/components/common/CategoryRuleBuilderFields.vue';
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';

import { ref, computed, useTemplateRef, onMounted, onUnmounted } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { useCategoryEditPageBase } from '@/views/base/categories/CategoryEditPageBase.ts';

import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';

import type { ApiResponse, ErrorResponse } from '@/core/api.ts';
import type { ColorValue } from '@/core/color.ts';
import { CategoryType } from '@/core/category.ts';
import { ALL_CATEGORY_ICONS } from '@/consts/icon.ts';
import { ALL_CATEGORY_COLORS } from '@/consts/color.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';

import { generateRandomUUID } from '@/lib/misc.ts';
import services from '@/lib/services.ts';

interface TransactionCategoryEditResponse {
    message: string;
    id?: string;
    category?: TransactionCategory;
}

interface CategoryRuleBuilderModel {
    priority: number;
    ruleExpression: string;
    regexEnabled: boolean;
    enabled: boolean;
}

interface CategoryRuleItem {
    id: number;
    category_id: number;
    name: string;
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

type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();
const {
    editCategoryId,
    clientSessionId,
    loading,
    submitting,
    category,
    allAvailableCategories,
    title,
    saveButtonTitle,
    inputEmptyProblemMessage,
    inputIsEmpty
} = useCategoryEditPageBase();

const transactionCategoriesStore = useTransactionCategoriesStore();

const snackbar = useTemplateRef<SnackBarType>('snackbar');

const showState = ref<boolean>(false);
const ruleLoading = ref<boolean>(false);
const ruleLoadFailed = ref<boolean>(false);
const primaryCategoryRuleId = ref<number | null>(null);
const additionalCategoryRulesCount = ref<number>(0);
const categoryRuleDraft = ref<CategoryRuleBuilderModel>(createEmptyCategoryRuleDraft());

let resolveFunc: ((value: TransactionCategoryEditResponse) => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const isSecondaryCategory = computed<boolean>(() => !!category.value.parentId && category.value.parentId !== '0');
const isCategoryModified = computed<boolean>(() => {
    if (!editCategoryId.value) { // Add
        return !category.value.equals(TransactionCategory.createNewCategory(category.value.type, category.value.parentId));
    } else { // Edit
        return true;
    }
});
const autoPrimaryRuleName = computed<string>(() => {
    const primaryName = resolvePrimaryCategoryName(category.value.parentId);
    const secondaryName = (category.value.name || '').trim() || tt('Secondary Category');
    const categoryPath = primaryName ? `${primaryName} / ${secondaryName}` : secondaryName;
    return `${categoryPath} · P${categoryRuleDraft.value.priority}`;
});
const categoryRuleBuilderModel = computed<CategoryRuleBuilderModel>({
    get: () => categoryRuleDraft.value,
    set: (value) => {
        categoryRuleDraft.value = normalizeCategoryRuleDraft(value);
    }
});

function createEmptyCategoryRuleDraft(): CategoryRuleBuilderModel {
    return {
        priority: 100,
        ruleExpression: '',
        regexEnabled: false,
        enabled: true
    };
}

function normalizeCategoryRuleDraft(value?: Partial<CategoryRuleBuilderModel> | null): CategoryRuleBuilderModel {
    const parsedPriority = Number(value?.priority ?? 100);

    return {
        priority: Number.isFinite(parsedPriority) ? parsedPriority : 100,
        ruleExpression: String(value?.ruleExpression ?? ''),
        regexEnabled: !!value?.regexEnabled,
        enabled: value?.enabled !== false
    };
}

function resetCategoryRuleEditor(): void {
    ruleLoading.value = false;
    ruleLoadFailed.value = false;
    primaryCategoryRuleId.value = null;
    additionalCategoryRulesCount.value = 0;
    categoryRuleDraft.value = createEmptyCategoryRuleDraft();
}

function resolvePrimaryCategoryName(parentId: string): string {
    if (!parentId || parentId === '0') {
        return '';
    }

    if (String(parentId).startsWith('virtual_')) {
        return String(parentId).replace('virtual_', '');
    }

    return allAvailableCategories.value.find(item => item.id === parentId)?.name ?? '';
}

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
        typedPayload.error ?? typedPayload.message,
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

    if (error && typeof error === 'object') {
        if ('message' in error && typeof error.message === 'string' && error.message) {
            return error.message;
        }
        if ('error' in error) {
            return extractPayloadMessage(error.error) || fallback;
        }
    }

    return fallback;
}

function showDialogError(error: unknown, fallback: string): void {
    snackbar.value?.showError({
        message: getRequestErrorMessage(error, fallback)
    });
}

function requireApiSuccess<T>(response: { data?: ApiResponse<T> }, fallback: string): T {
    if (response.data?.success) {
        return response.data.result;
    }

    throw new Error(fallback);
}

function getSortedCategoryRules(rules: CategoryRuleItem[]): CategoryRuleItem[] {
    return [...rules].sort((firstRule, secondRule) => (
        firstRule.priority - secondRule.priority
        || firstRule.id - secondRule.id
    ));
}

async function loadPrimaryCategoryRule(
    categoryId?: string | null,
    options: { rethrowOnError?: boolean; showError?: boolean } = {}
): Promise<void> {
    if (!categoryId || !isSecondaryCategory.value) {
        resetCategoryRuleEditor();
        return;
    }

    const numericCategoryId = Number.parseInt(categoryId, 10);
    if (!Number.isFinite(numericCategoryId)) {
        resetCategoryRuleEditor();
        return;
    }

    ruleLoading.value = true;
    ruleLoadFailed.value = false;

    try {
        const response = await axios.get<{
            success?: boolean;
            data?: CategoryRuleItem[];
            error?: string;
        }>('category-rules/', {
            params: {
                category_id: numericCategoryId,
                enabled_only: false
            }
        });

        if (!response.data?.success) {
            throw new Error(response.data?.error || 'Failed to load category rules');
        }

        const rules = getSortedCategoryRules(response.data.data ?? []);
        const primaryRule = rules[0] ?? null;

        primaryCategoryRuleId.value = primaryRule?.id ?? null;
        additionalCategoryRulesCount.value = Math.max(0, rules.length - 1);
        categoryRuleDraft.value = primaryRule ? normalizeCategoryRuleDraft({
            priority: primaryRule.priority,
            ruleExpression: primaryRule.rule_expression,
            regexEnabled: primaryRule.regex_enabled,
            enabled: primaryRule.enabled
        }) : createEmptyCategoryRuleDraft();
        category.value.categoryRules = rules.map(rule => ({
            id: rule.id,
            name: rule.name,
            ruleExpression: rule.rule_expression,
            priority: rule.priority,
            enabled: rule.enabled,
            regexEnabled: rule.regex_enabled
        }));
    } catch (error) {
        resetCategoryRuleEditor();
        ruleLoadFailed.value = true;
        if (options.showError !== false) {
            showDialogError(error, 'Failed to load canonical category rule');
        }
        if (options.rethrowOnError) {
            throw error;
        }
    } finally {
        ruleLoading.value = false;
    }
}

function getPrimaryRuleExpressionFromEditorState(): string {
    return categoryRuleDraft.value.ruleExpression.trim();
}

function buildPrimaryCategoryRulePayload(categoryId: string): CategoryRulePayload | null {
    const numericCategoryId = Number.parseInt(categoryId, 10);
    const ruleExpression = categoryRuleDraft.value.ruleExpression.trim();

    if (!Number.isFinite(numericCategoryId) || !ruleExpression) {
        return null;
    }

    return {
        category_id: numericCategoryId,
        name: autoPrimaryRuleName.value,
        priority: categoryRuleDraft.value.priority,
        rule_expression: ruleExpression,
        regex_enabled: categoryRuleDraft.value.regexEnabled,
        enabled: categoryRuleDraft.value.enabled
    };
}

async function syncPrimaryCategoryRule(categoryId: string): Promise<void> {
    const payload = buildPrimaryCategoryRulePayload(categoryId);

    if (!payload) {
        if (primaryCategoryRuleId.value !== null) {
            requireApiSuccess(
                await services.deleteCategoryRule(primaryCategoryRuleId.value),
                'Failed to delete category rule'
            );
            primaryCategoryRuleId.value = null;
        }
        return;
    }

    if (primaryCategoryRuleId.value !== null) {
        requireApiSuccess(
            await services.updateCategoryRule(primaryCategoryRuleId.value, payload),
            'Failed to save category rule'
        );
        return;
    }

    const createdRule = requireApiSuccess<Partial<CategoryRuleItem>>(
        await services.createCategoryRule(payload),
        'Failed to save category rule'
    );

    if (createdRule.id !== undefined && createdRule.id !== null) {
        primaryCategoryRuleId.value = Number(createdRule.id);
    }
}

function open(options: { id?: string; parentId?: string; type?: CategoryType; currentCategory?: TransactionCategory, color?: ColorValue, icon?: string }): Promise<TransactionCategoryEditResponse> {
    showState.value = true;
    loading.value = true;
    submitting.value = false;
    resetCategoryRuleEditor();

    const newTransactionCategory = TransactionCategory.createNewCategory();
    category.value.fillFrom(newTransactionCategory);

    if (options.id) {
        if (options.currentCategory) {
            category.value.fillFrom(options.currentCategory);
        }

        editCategoryId.value = options.id;
        transactionCategoriesStore.getCategory({
            categoryId: editCategoryId.value
        }).then(async response => {
            // Only overwrite icon and color if the response has valid values
            // This prevents overwriting existing values with empty strings if the backend returns incomplete data
            const currentIcon = category.value.icon;
            const currentColor = category.value.color;

            category.value.fillFrom(response);

            if (!category.value.icon && currentIcon) {
                category.value.icon = currentIcon;
            }

            if (!category.value.color && currentColor) {
                category.value.color = currentColor;
            }

            if (category.value.parentId && category.value.parentId !== '0') {
                await loadPrimaryCategoryRule(editCategoryId.value);
            }

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
    } else if (options.parentId) {
        editCategoryId.value = null;

        const categoryType = options.type;

        if (categoryType !== CategoryType.Income &&
            categoryType !== CategoryType.Expense &&
            categoryType !== CategoryType.Transfer &&
            categoryType !== CategoryType.Investment) {
            loading.value = false;
            showState.value = false;

            return Promise.reject('Parameter Invalid');
        }

        category.value.type = categoryType;
        category.value.parentId = options.parentId;

        if (options.color) {
            category.value.color = options.color;
        }

        if (options.icon) {
            category.value.icon = options.icon;
        }

        clientSessionId.value = generateRandomUUID();
        loading.value = false;
    }

    return new Promise((resolve, reject) => {
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

    const wasEdit = !!editCategoryId.value;
    const canSyncSecondaryRule = isSecondaryCategory.value && !ruleLoadFailed.value;
    let savedCategory: TransactionCategory | null = null;

    try {
        savedCategory = await transactionCategoriesStore.saveCategory({
            category: category.value,
            isEdit: wasEdit,
            clientSessionId: clientSessionId.value
        });

        category.value.fillFrom(savedCategory);
        editCategoryId.value = savedCategory.id;

        if (canSyncSecondaryRule && savedCategory.id) {
            await syncPrimaryCategoryRule(savedCategory.id);
            await loadPrimaryCategoryRule(savedCategory.id, {
                rethrowOnError: true,
                showError: false
            });

            const currentPrimaryRuleExpression = getPrimaryRuleExpressionFromEditorState();
            savedCategory.ruleExpression = currentPrimaryRuleExpression;
            category.value.ruleExpression = currentPrimaryRuleExpression;
        }

        const message = wasEdit
            ? 'You have saved this category'
            : 'You have added a new category';

        resolveFunc?.({ message, id: savedCategory.id, category: savedCategory });
        showState.value = false;
    } catch (error) {
        if (savedCategory) {
            category.value.fillFrom(savedCategory);
            editCategoryId.value = savedCategory.id;
        }

        showDialogError(error, 'Unable to save category');
    } finally {
        submitting.value = false;
    }
}

function cancel(): void {
    rejectFunc?.();
    showState.value = false;
}

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
