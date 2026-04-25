<template>
    <v-card variant="outlined" rounded="lg" class="category-rule-builder">
        <v-card-text class="pa-4">
            <div class="d-flex flex-column ga-4">
                <div class="category-rule-builder__header">
                    <div class="text-subtitle-1 font-weight-medium">
                        {{ tt(title) }}
                    </div>

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
                </div>

                <rule-expression-input
                    :model-value="draft.ruleExpression"
                    expression-format="composite"
                    :disabled="disabled"
                    :regex-enabled="draft.regexEnabled"
                    :title="tt('Rule Matching Expression')"
                    :add-button-text="tt('Add Expression')"
                    :empty-state-text="tt('No rule expression defined yet')"
                    @update:model-value="updateTextField('ruleExpression', $event)"
                />
            </div>
        </v-card-text>
    </v-card>
</template>

<script setup lang="ts">
import RuleExpressionInput from '@/components/common/KeywordInput.vue';
import { useI18n } from '@/locales/helpers.ts';
import { computed } from 'vue';

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
    title: 'Rule Matching Expression',
    description: '',
    showEnabledToggle: true,
});

const emit = defineEmits<{
    (e: 'update:modelValue', value: CategoryRuleBuilderModel): void;
}>();

const { tt } = useI18n();

const draft = computed(() => props.modelValue);

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
</script>

<style scoped>
.category-rule-builder {
    background: rgba(var(--v-theme-surface), 1);
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
</style>
