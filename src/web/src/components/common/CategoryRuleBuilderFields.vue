<template>
    <v-card variant="outlined" rounded="lg" class="category-rule-builder">
        <v-card-text class="pa-4">
            <div class="d-flex flex-column ga-4">
                <div class="d-flex flex-column flex-md-row align-md-center ga-3">
                    <div class="text-subtitle-1 font-weight-medium">
                        {{ tt(title) }}
                    </div>
                    <v-spacer />
                    <v-chip color="primary" variant="tonal" size="small">
                        {{ tt('Auto Rule Name') }} · {{ autoRuleName }}
                    </v-chip>
                </div>

                <v-alert
                    v-if="description"
                    type="info"
                    variant="tonal"
                    density="compact"
                >
                    {{ tt(description) }}
                </v-alert>

                <v-row>
                    <v-col cols="12" md="4">
                        <v-text-field
                            :model-value="draft.priority"
                            type="number"
                            density="comfortable"
                            variant="outlined"
                            :disabled="disabled"
                            :label="tt('Rule Priority')"
                            :hint="tt('Lower number means higher priority')"
                            persistent-hint
                            @update:model-value="updateNumberField('priority', $event)"
                        />
                    </v-col>

                    <v-col cols="12" md="4">
                        <v-switch
                            :model-value="draft.regexEnabled"
                            color="primary"
                            hide-details
                            inset
                            :disabled="disabled"
                            :label="tt('Enable Regex')"
                            @update:model-value="updateBooleanField('regexEnabled', $event)"
                        />
                    </v-col>

                    <v-col v-if="showEnabledToggle" cols="12" md="4">
                        <v-switch
                            :model-value="draft.enabled"
                            color="success"
                            hide-details
                            inset
                            :disabled="disabled"
                            :label="tt('Enabled')"
                            @update:model-value="updateBooleanField('enabled', $event)"
                        />
                    </v-col>
                </v-row>

                <v-text-field
                    :model-value="autoRuleName"
                    density="comfortable"
                    variant="outlined"
                    readonly
                    :label="tt('Rule Name')"
                    :hint="tt('The rule name is generated automatically from category and priority')"
                    persistent-hint
                />

                <rule-expression-input
                    :model-value="draft.ruleExpression"
                    expression-format="composite"
                    :disabled="disabled"
                    :title="tt('Rule Expression Builder')"
                    :add-button-text="tt('Add Rule Block')"
                    :empty-state-text="tt('No rule expression defined yet')"
                    :help-text="tt('Choose OR / AND / NOT blocks and enter only the keywords you want to match. Parentheses and separators are generated automatically when the rule is saved.')"
                    :example-text="tt('Example: (OR={早餐,早饭}+OR={咖啡,奶茶})+NOT={退款}')"
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
    title: 'Rule Builder',
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

function updateNumberField(key: 'priority', value: unknown): void {
    const nextValue = Number(value ?? 0);
    updateField(key, Number.isFinite(nextValue) ? nextValue : 0);
}

function updateBooleanField(key: 'regexEnabled' | 'enabled', value: unknown): void {
    updateField(key, !!value);
}
</script>

<style scoped>
.category-rule-builder {
    background: rgba(var(--v-theme-surface), 1);
}
</style>
