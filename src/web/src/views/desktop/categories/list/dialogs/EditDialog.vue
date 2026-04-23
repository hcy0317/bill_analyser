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
                        <v-col cols="12" md="12" v-if="category.parentId && category.parentId !== '0'">
                            <div class="d-flex flex-column flex-sm-row align-sm-center ga-2 mb-3">
                                <div class="text-caption text-medium-emphasis flex-grow-1">
                                    {{ tt('Legacy keyword migration copies existing category keywords into canonical category rules. Repeat runs skip rules that already exist.') }}
                                </div>
                                <v-btn
                                    size="small"
                                    variant="outlined"
                                    color="primary"
                                    :prepend-icon="mdiDatabaseImportOutline"
                                    :loading="migratingKeywords"
                                    :disabled="loading || submitting || migratingKeywords"
                                    @click="migrateLegacyKeywords"
                                >
                                    {{ tt('Import Rules from Legacy Keywords') }}
                                </v-btn>
                            </div>
                            <rule-expression-input
                                :model-value="category.ruleExpression || ''"
                                expression-format="composite"
                                :title="tt('Category Matching Rule Expression')"
                                :add-button-text="tt('Add Rule Block')"
                                :empty-state-text="tt('No rule expression defined yet')"
                                :help-text="tt('This secondary category is matched by a boolean rule expression instead of a legacy keyword list. Use OR / AND / NOT blocks to describe when transactions should be assigned here. Parentheses can group rule blocks, for example (OR={早餐}+AND={咖啡})+NOT={退款}. Saving this form stores the expression as the category matching rule.')"
                                :example-text="tt('Example: (OR={早餐}+AND={咖啡})+NOT={退款}')"
                                @update:model-value="category.ruleExpression = $event"
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
import ColorSelect from '@/components/desktop/ColorSelect.vue';
import IconSelect from '@/components/desktop/IconSelect.vue';
import RuleExpressionInput from '@/components/common/KeywordInput.vue';
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';

import { ref, computed, useTemplateRef, onMounted, onUnmounted } from 'vue';
import { mdiDatabaseImportOutline } from '@mdi/js';

import { useI18n } from '@/locales/helpers.ts';
import { useCategoryEditPageBase } from '@/views/base/categories/CategoryEditPageBase.ts';

import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';

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

interface CategoryKeywordMigrationResult {
    migrated?: number | string | null;
    migrated_count?: number | string | null;
    skipped?: number | string | null;
    skipped_count?: number | string | null;
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
const migratingKeywords = ref<boolean>(false);

let resolveFunc: ((value: TransactionCategoryEditResponse) => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

const isCategoryModified = computed<boolean>(() => {
    if (!editCategoryId.value) { // Add
        return !category.value.equals(TransactionCategory.createNewCategory(category.value.type, category.value.parentId));
    } else { // Edit
        return true;
    }
});

function open(options: { id?: string; parentId?: string; type?: CategoryType; currentCategory?: TransactionCategory, color?: ColorValue, icon?: string }): Promise<TransactionCategoryEditResponse> {
    showState.value = true;
    loading.value = true;
    submitting.value = false;

    const newTransactionCategory = TransactionCategory.createNewCategory();
    category.value.fillFrom(newTransactionCategory);

    if (options.id) {
        if (options.currentCategory) {
            category.value.fillFrom(options.currentCategory);
        }

        editCategoryId.value = options.id;
        transactionCategoriesStore.getCategory({
            categoryId: editCategoryId.value
        }).then(response => {
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

function save(): void {
    const problemMessage = inputEmptyProblemMessage.value;

    if (problemMessage) {
        snackbar.value?.showMessage(problemMessage);
        return;
    }

    submitting.value = true;

    transactionCategoriesStore.saveCategory({
        category: category.value,
        isEdit: !!editCategoryId.value,
        clientSessionId: clientSessionId.value
    }).then((savedCategory) => {
        submitting.value = false;

        let message = 'You have saved this category';

        if (!editCategoryId.value) {
            message = 'You have added a new category';
        }

        resolveFunc?.({ message, id: savedCategory.id, category: savedCategory });
        showState.value = false;
    }).catch(error => {
        submitting.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function getMigrationCount(value: number | string | null | undefined): number {
    const count = Number(value ?? 0);
    return Number.isFinite(count) ? count : 0;
}

async function refreshCurrentCategoryAfterMigration(): Promise<void> {
    transactionCategoriesStore.updateTransactionCategoryListInvalidState(true);
    await transactionCategoriesStore.loadAllCategories({ force: false });

    if (!editCategoryId.value) {
        return;
    }

    const refreshedCategory = await transactionCategoriesStore.getCategory({
        categoryId: editCategoryId.value
    });

    category.value.ruleExpression = refreshedCategory.ruleExpression;
    category.value.categoryRules = refreshedCategory.categoryRules;
}

async function migrateLegacyKeywords(): Promise<void> {
    migratingKeywords.value = true;

    try {
        const response = await services.migrateCategoryKeywords();

        if (!response.data?.success) {
            throw new Error('Migration failed');
        }

        const result = (response.data.result ?? {}) as CategoryKeywordMigrationResult;
        const migrated = getMigrationCount(result.migrated ?? result.migrated_count);
        const skipped = getMigrationCount(result.skipped ?? result.skipped_count);

        await refreshCurrentCategoryAfterMigration();

        snackbar.value?.showMessage('Migration completed: migrated {migrated}, skipped {skipped}', {
            migrated,
            skipped
        });
    } catch {
        snackbar.value?.showMessage('Migration failed');
    } finally {
        migratingKeywords.value = false;
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
