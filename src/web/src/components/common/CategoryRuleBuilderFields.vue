<template>
    <div class="category-rule-builder">
        <div class="category-rule-builder__legend">
            {{ tt(title) }}
        </div>

        <div class="category-rule-builder__content">
            <div class="category-rule-builder__header">
                <div class="category-rule-builder__toggles">
                    <v-switch
                        :model-value="draft.regexEnabled"
                        class="category-rule-builder__switch"
                        color="primary"
                        density="compact"
                        hide-details
                        inset
                        :disabled="disabled"
                        :label="tt('Enable Regex')"
                        @update:model-value="updateBooleanField('regexEnabled', $event)"
                    />

                    <v-switch
                        v-if="showEnabledToggle"
                        :model-value="draft.enabled"
                        class="category-rule-builder__switch"
                        color="success"
                        density="compact"
                        hide-details
                        inset
                        :disabled="disabled"
                        :label="tt('Enabled')"
                        @update:model-value="updateBooleanField('enabled', $event)"
                    />
                </div>

                <v-btn
                    class="category-rule-builder__action"
                    size="small"
                    variant="text"
                    color="primary"
                    :prepend-icon="mdiPlus"
                    :disabled="disabled || !canAddExpression"
                    @click="addExpression"
                >
                    {{ tt('Add Matching Expression') }}
                </v-btn>
            </div>

            <div class="category-rule-builder__divider" />

            <rule-expression-input
                ref="expressionInput"
                :model-value="draft.ruleExpression"
                expression-format="composite"
                :disabled="disabled"
                :regex-enabled="draft.regexEnabled"
                :show-header="false"
                :empty-state-text="tt('No rule expression defined yet')"
                @update:model-value="updateTextField('ruleExpression', $event)"
            />
        </div>
    </div>
</template>

<script setup lang="ts">
import { mdiPlus } from '@mdi/js';
import RuleExpressionInput from '@/components/common/KeywordInput.vue';
import { useI18n } from '@/locales/helpers.ts';
import { computed, ref } from 'vue';

interface CategoryRuleBuilderModel {
    priority: number;
    ruleExpression: string;
    regexEnabled: boolean;
    enabled: boolean;
}

const props = withDefaults(defineProps<{
    modelValue: CategoryRuleBuilderModel;
    autoRuleName: string;
    disabled?: boolean;
    showEnabledToggle?: boolean;
    title?: string;
    description?: string;
}>(), {
    title: 'Category Matching',
    description: '',
    showEnabledToggle: true,
});

const emit = defineEmits<{
    (e: 'update:modelValue', value: CategoryRuleBuilderModel): void;
}>();

const { tt } = useI18n();

const draft = computed(() => props.modelValue);
const expressionInput = ref<{
    addExpression: () => void;
    canAddExpression: () => boolean;
} | null>(null);
const canAddExpression = computed(() => expressionInput.value?.canAddExpression() ?? true);

function updateField<K extends keyof CategoryRuleBuilderModel>(key: K, value: CategoryRuleBuilderModel[K]): void {
    emit('update:modelValue', {
        ...props.modelValue,
        [key]: value,
    });
}

function updateTextField(key: 'ruleExpression', value: string): void {
    updateField(key, value);
}

function updateBooleanField(key: 'regexEnabled' | 'enabled', value: unknown): void {
    updateField(key, !!value);
}

function addExpression(): void {
    expressionInput.value?.addExpression();
}
</script>

<style scoped>
.category-rule-builder {
    position: relative;
    border: 1px solid rgba(var(--v-theme-on-surface), 0.2);
    border-radius: 12px;
    background: rgba(var(--v-theme-surface), 1);
    padding: 18px 16px 16px;
}

.category-rule-builder__legend {
    position: absolute;
    top: 0;
    left: 12px;
    transform: translateY(-50%);
    padding: 0 6px;
    background: rgba(var(--v-theme-surface), 1);
    color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
    font-size: 12px;
    line-height: 1.2;
}

.category-rule-builder__content {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.category-rule-builder__header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 16px;
}

.category-rule-builder__toggles {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 16px;
}

.category-rule-builder__switch {
    flex: 0 0 auto;
}

.category-rule-builder__action {
    margin-left: auto;
}

.category-rule-builder__divider {
    border-top: 1px dashed rgba(var(--v-theme-on-surface), 0.18);
}
</style>
