<template>
    <f7-sheet
        class="budget-edit-sheet"
        swipe-to-close
        swipe-handler=".swipe-handler"
        style="height:auto"
        :opened="show"
        @sheet:closed="onSheetClosed"
    >
        <div class="swipe-handler" style="z-index: 10"></div>

        <f7-page-content class="margin-top no-padding-top">
            <div class="display-flex padding justify-content-space-between align-items-center">
                <div class="ebk-sheet-title">
                    <b>{{ isNew ? tt('Add Budget') : tt('Edit Budget') }}</b>
                </div>
            </div>

            <f7-list strong inset dividers no-hairlines-md class="margin-bottom">
                <!-- Type toggle: Expense / Investment.
                     Locked when editing an existing budget so the user does not
                     change a budget's type after the categoryId is bound. -->
                <f7-list-item :title="tt('Budget Type')">
                    <template #after>
                        <f7-segmented strong tag="div">
                            <f7-button
                                :text="tt('Expense')"
                                :active="form.type === BudgetType.Expense"
                                :disabled="!isNew || saving"
                                @click="setType(BudgetType.Expense)"
                            ></f7-button>
                            <f7-button
                                :text="tt('Investment')"
                                :active="form.type === BudgetType.Investment"
                                :disabled="!isNew || saving"
                                @click="setType(BudgetType.Investment)"
                            ></f7-button>
                        </f7-segmented>
                    </template>
                </f7-list-item>

                <!-- Period type. Mirror desktop EditDialog options. -->
                <f7-list-item
                    link="#"
                    no-chevron
                    :header="tt('Period Type')"
                    :title="periodTypeLabel"
                    @click="showPeriodSheet = true"
                >
                    <list-item-selection-sheet
                        value-type="item"
                        key-field="value"
                        value-field="value"
                        title-field="text"
                        :items="periodTypeOptions"
                        v-model:show="showPeriodSheet"
                        v-model="form.periodType"
                    ></list-item-selection-sheet>
                </f7-list-item>

                <!-- Category picker: bound to CategoryType.Expense when type is
                     Expense and CategoryType.Investment when type is Investment.
                     Re-uses the same tree-view-selection-sheet that the mobile
                     transaction edit page uses (S2). -->
                <f7-list-item
                    link="#"
                    no-chevron
                    :header="tt('Category')"
                    :title="selectedCategoryDisplay"
                    @click="showCategorySheet = true"
                >
                    <tree-view-selection-sheet
                        primary-key-field="id"
                        primary-title-field="name"
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
                        :enable-filter="true"
                        :filter-placeholder="tt('Find category')"
                        :filter-no-items-text="tt('No available category')"
                        :items="availableCategories"
                        v-model:show="showCategorySheet"
                        v-model="form.categoryId"
                    ></tree-view-selection-sheet>
                </f7-list-item>

                <!-- Yuan-denominated amount. We display the existing
                     Budget.amountInYuan getter for round-trip and convert back
                     to cents on save (× 100) so the wire format stays in cents
                     exactly like desktop saves. No new helpers introduced. -->
                <f7-list-input
                    type="number"
                    inputmode="decimal"
                    step="0.01"
                    min="0"
                    :label="tt('Budget Amount') + ' (¥)'"
                    :placeholder="tt('Budget Amount')"
                    :value="amountInYuanInput"
                    :disabled="saving"
                    @input="onAmountInput"
                ></f7-list-input>

                <!-- Optional remark / name. -->
                <f7-list-input
                    type="text"
                    :label="tt('Budget Name')"
                    :placeholder="tt('Optional')"
                    :value="form.name"
                    :disabled="saving"
                    @input="(e: Event) => form.name = ((e.target as HTMLInputElement).value || '')"
                ></f7-list-input>
            </f7-list>

            <div class="padding-horizontal padding-bottom">
                <f7-button
                    large
                    fill
                    :class="{ 'disabled': saving || !canSave }"
                    @click="onSave"
                >{{ tt('Save') }}</f7-button>

                <f7-button
                    v-if="!isNew"
                    large
                    color="red"
                    class="margin-top"
                    :class="{ 'disabled': saving }"
                    @click="onDeleteRequested"
                >{{ tt('Delete') }}</f7-button>

                <div class="margin-top text-align-center">
                    <f7-link :class="{ 'disabled': saving }" @click="closeSheet" :text="tt('Cancel')"></f7-link>
                </div>
            </div>
        </f7-page-content>
    </f7-sheet>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';

import { Budget, BudgetType, BudgetPeriodType } from '@/models/budget.ts';
import { CategoryType } from '@/core/category.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';

import logger from '@/lib/logger.ts';

// ============================================================================
// Props / events
// ============================================================================

const props = defineProps<{
    show: boolean;
    budget: Budget | null; // null/empty.id ⇒ create mode
    defaultType?: BudgetType;
}>();

const emit = defineEmits<{
    (e: 'update:show', v: boolean): void;
    (e: 'save', budget: Budget): void;
    (e: 'delete:request', budget: Budget): void;
}>();

// ============================================================================
// State
// ============================================================================

const { tt } = useI18n();
const categoryStore = useTransactionCategoriesStore();

const saving = ref<boolean>(false);

// Local edit state. We never mutate the Budget passed in via props directly —
// the parent re-passes the (possibly updated) budget after successful save.
const form = ref<Budget>(new Budget());
const amountInYuanInput = ref<string>('');

const showPeriodSheet = ref<boolean>(false);
const showCategorySheet = ref<boolean>(false);

// ============================================================================
// Computed
// ============================================================================

const isNew = computed<boolean>(() => !form.value.id);

const periodTypeOptions = computed(() => [
    { value: BudgetPeriodType.Monthly, text: tt('Monthly') },
    { value: BudgetPeriodType.Quarterly, text: tt('Quarterly') },
    { value: BudgetPeriodType.Yearly, text: tt('Yearly') }
]);

const periodTypeLabel = computed<string>(() => {
    const found = periodTypeOptions.value.find(opt => opt.value === form.value.periodType);
    return found ? found.text : '';
});

// Resolve the category list for the current type via the existing categories
// store (which already partitions by CategoryType.Expense / Investment).
const availableCategories = computed<TransactionCategory[]>(() => {
    const catType = form.value.type === BudgetType.Investment
        ? CategoryType.Investment
        : CategoryType.Expense;
    return categoryStore.allTransactionCategories[catType] || [];
});

const selectedCategoryDisplay = computed<string>(() => {
    if (!form.value.categoryId) {
        return tt('Select Category');
    }
    for (const primary of availableCategories.value) {
        if (primary.id === form.value.categoryId) {
            return primary.name;
        }
        for (const secondary of primary.subCategories || []) {
            if (secondary.id === form.value.categoryId) {
                return `${primary.name} / ${secondary.name}`;
            }
        }
    }
    return tt('Select Category');
});

// `canSave` mirrors the desktop EditDialog invariants: a category is required
// and amount cents must be positive.
const canSave = computed<boolean>(() => {
    if (form.value.amountCents <= 0) {
        return false;
    }
    return !!(form.value.categoryId || form.value.category);
});

// ============================================================================
// Methods
// ============================================================================

function setType(type: BudgetType): void {
    if (!isNew.value || saving.value) {
        return; // type is locked once persisted
    }
    if (form.value.type === type) {
        return;
    }
    form.value.type = type;
    // Clear category fields — the picker lists will switch via availableCategories.
    form.value.categoryId = '';
    form.value.category = '';
    form.value.subCategory = '';
}

function onAmountInput(event: Event): void {
    const raw = (event.target as HTMLInputElement).value || '';
    amountInYuanInput.value = raw;
    const parsed = parseFloat(raw);
    if (!Number.isFinite(parsed) || parsed < 0) {
        form.value.amountCents = 0;
        return;
    }
    // Yuan input boundary -> cents using the same ratio as Budget.amountInYuan.
    // We round to avoid floating-point dust.
    form.value.amountCents = Math.round(parsed * 100);
}

function resetForm(source: Budget | null, defaultType: BudgetType): void {
    const next = source ? Object.assign(new Budget(), source) : Budget.createNew(defaultType);
    form.value = next;
    // Display the yuan amount via the existing model getter.
    amountInYuanInput.value = next.amountCents > 0 ? next.amountInYuan.toFixed(2) : '';
}

function closeSheet(): void {
    emit('update:show', false);
}

function onSheetClosed(): void {
    if (props.show) {
        emit('update:show', false);
    }
}

function syncCategoryNamesFromId(): void {
    if (!form.value.categoryId) {
        return;
    }
    for (const primary of availableCategories.value) {
        if (primary.id === form.value.categoryId) {
            form.value.category = primary.name;
            form.value.subCategory = '';
            return;
        }
        for (const secondary of primary.subCategories || []) {
            if (secondary.id === form.value.categoryId) {
                form.value.category = primary.name;
                form.value.subCategory = secondary.name;
                return;
            }
        }
    }
}

function onSave(): void {
    if (!canSave.value || saving.value) {
        return;
    }
    syncCategoryNamesFromId();
    if (!form.value.category) {
        logger.warn('[MobileBudgetEditSheet] Category is required');
        return;
    }
    saving.value = true;
    emit('save', form.value);
}

function onDeleteRequested(): void {
    if (saving.value || !form.value.id) {
        return;
    }
    emit('delete:request', form.value);
}

// ============================================================================
// Watchers
// ============================================================================

// Re-initialise the form whenever the sheet is re-opened with a new budget.
watch(
    () => [props.show, props.budget?.id, props.defaultType],
    () => {
        if (props.show) {
            saving.value = false;
            resetForm(props.budget, props.defaultType ?? BudgetType.Expense);
            void categoryStore.loadAllCategories({ force: false });
        }
    },
    { immediate: true }
);

// Allow parent to mark sheet ready again after save resolves.
defineExpose({
    setSaving(value: boolean): void {
        saving.value = value;
    }
});
</script>

<style>
.budget-edit-sheet .ebk-sheet-title {
    font-size: 16px;
}

.budget-edit-sheet .item-after .segmented {
    width: 180px;
}
</style>
