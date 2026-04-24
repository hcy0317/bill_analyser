<template>
    <div class="rule-expression-input">
        <div class="d-flex align-center mb-2">
            <span class="text-subtitle-2">{{ resolvedTitle }}</span>
            <v-spacer />
            <v-btn
                size="small"
                variant="text"
                color="primary"
                :prepend-icon="mdiPlus"
                @click="addClause"
                :disabled="disabled || isRawMode"
            >
                {{ resolvedAddButtonText }}
            </v-btn>
        </div>

        <v-alert
            v-if="parseErrorKey"
            type="warning"
            variant="tonal"
            density="compact"
            class="mb-2"
        >
            {{ tt(parseErrorKey) }}
        </v-alert>

        <v-textarea
            v-if="isRawMode"
            :model-value="rawExpression"
            :label="tt('Original rule expression')"
            rows="2"
            readonly
            density="compact"
            variant="outlined"
            hide-details
            class="mb-2"
        />

        <template v-else>
            <div v-if="clauses.length === 0" class="text-center text-caption text-medium-emphasis py-2">
                {{ resolvedEmptyStateText }}
            </div>

            <div v-for="clause in clauses" :key="clause.id" class="rule-clause-row mb-2">
                <div v-if="showGroupingControls" class="paren-control" :title="tt('Left parentheses')">
                    <v-btn
                        size="x-small"
                        variant="text"
                        :disabled="disabled"
                        @click="increaseOpenParen(clause)"
                    >
                        {{ tt('Add left parenthesis') }}
                    </v-btn>
                    <span class="paren-display">{{ repeatParen('(', clause.openParens) }}</span>
                    <v-btn
                        size="x-small"
                        variant="text"
                        :disabled="disabled || clause.openParens === 0"
                        @click="decreaseOpenParen(clause)"
                    >
                        {{ tt('Remove left parenthesis') }}
                    </v-btn>
                </div>

                <div class="clause-operator">
                    <v-select
                        v-model="clause.operator"
                        :items="types"
                        density="compact"
                        hide-details
                        variant="outlined"
                        :disabled="disabled"
                        @update:model-value="serializeKeywords"
                    />
                </div>

                <div class="clause-terms">
                    <v-combobox
                        v-model="clause.terms"
                        v-model:search="draftTerms[clause.id]"
                        :label="tt('Expression Terms')"
                        :placeholder="tt('Enter terms and press Enter')"
                        chips
                        closable-chips
                        multiple
                        density="compact"
                        hide-details
                        variant="outlined"
                        append-inner-icon=""
                        :disabled="disabled"
                        @keydown.enter.prevent="commitPendingTerm(clause)"
                        @update:model-value="onTermsUpdated(clause)"
                    />
                </div>

                <div v-if="showGroupingControls" class="paren-control" :title="tt('Right parentheses')">
                    <v-btn
                        size="x-small"
                        variant="text"
                        :disabled="disabled"
                        @click="increaseCloseParen(clause)"
                    >
                        {{ tt('Add right parenthesis') }}
                    </v-btn>
                    <span class="paren-display">{{ repeatParen(')', clause.closeParens) }}</span>
                    <v-btn
                        size="x-small"
                        variant="text"
                        :disabled="disabled || clause.closeParens === 0"
                        @click="decreaseCloseParen(clause)"
                    >
                        {{ tt('Remove right parenthesis') }}
                    </v-btn>
                </div>

                <v-btn
                    :icon="mdiPlus"
                    size="small"
                    variant="text"
                    color="primary"
                    :title="tt('Insert clause after this row')"
                    :disabled="disabled"
                    @click="insertClauseAfterRow(clause)"
                />
                <v-btn
                    :icon="mdiDelete"
                    size="small"
                    variant="text"
                    color="error"
                    :disabled="disabled"
                    @click="removeClause(clause.id)"
                />
            </div>

            <v-alert
                v-if="validationErrorKey"
                type="error"
                variant="tonal"
                density="compact"
                class="mb-2"
            >
                {{ tt(validationErrorKey) }}
            </v-alert>
        </template>

        <div class="text-caption text-medium-emphasis mt-2">
            {{ resolvedHelpText }}
        </div>

        <div v-if="resolvedExampleText" class="text-caption text-medium-emphasis mt-1">
            {{ resolvedExampleText }}
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { useI18n } from '@/locales/helpers.ts';
import {
    addTermToClause,
    createRuleClause,
    insertClauseAfter,
    normalizeRuleTerms,
    parseExpression,
    serializeForFormat,
    type ExpressionFormat,
    type RuleClause,
    type RuleOperator
} from '@/components/common/keywordExpression.ts';

import { mdiPlus, mdiDelete } from '@mdi/js';

const { tt } = useI18n();

const props = withDefaults(defineProps<{
    modelValue: string;
    disabled?: boolean;
    expressionFormat?: ExpressionFormat;
    format?: ExpressionFormat;
    title?: string;
    emptyStateText?: string;
    helpText?: string;
    exampleText?: string;
    addButtonText?: string;
}>(), {
    format: 'legacy',
});

const emit = defineEmits<{
    'update:modelValue': [value: string];
}>();

const resolvedFormat = computed<ExpressionFormat>(() => props.expressionFormat ?? props.format);
const resolvedTitle = computed(() => props.title || tt('Rule Expression'));
const resolvedAddButtonText = computed(() => props.addButtonText || tt('Add Clause'));
const resolvedEmptyStateText = computed(() => props.emptyStateText || tt('No rule clauses yet'));
const resolvedHelpText = computed(() => props.helpText || (
    resolvedFormat.value === 'composite'
        ? tt('Add keyword chips, then use AND / NOT and parentheses only when needed.')
        : tt('Add keyword chips with OR / AND / NOT blocks.')
));
const resolvedExampleText = computed(() => props.exampleText || (
    resolvedFormat.value === 'composite'
        ? tt('Example: OR={早餐,咖啡}+NOT={退款}')
        : ''
));

const clauses = ref<RuleClause[]>([]);
const draftTerms = ref<Record<string, string>>({});
const rawExpression = ref('');
const parseErrorKey = ref('');
const validationErrorKey = ref('');
let localClauseId = 0;

const supportsCompositeGrouping = computed(() => resolvedFormat.value === 'composite');
const showGroupingControls = computed(() => supportsCompositeGrouping.value);
const isRawMode = computed(() => rawExpression.value.length > 0);
const types = computed(() => {
    return [
        { title: tt('Match Any (OR)'), value: 'OR' as RuleOperator },
        { title: tt('Require All (AND)'), value: 'AND' as RuleOperator },
        { title: tt('Exclude (NOT)'), value: 'NOT' as RuleOperator },
    ];
});

watch(() => [props.modelValue, resolvedFormat.value] as const, ([newVal]) => {
    const currentSerialized = getCurrentSerializedExpression();
    if (!rawExpression.value && currentSerialized !== null && newVal === currentSerialized) {
        return;
    }
    if (rawExpression.value && newVal === rawExpression.value) {
        return;
    }
    parseModelValue(newVal);
}, { immediate: true });

function parseModelValue(value: string) {
    const result = parseExpression(value, {
        format: resolvedFormat.value,
        idFactory: createLocalClauseId
    });
    clauses.value = result.clauses;
    rawExpression.value = result.rawExpression ?? '';
    parseErrorKey.value = result.errorKey ?? '';
    validationErrorKey.value = '';
    syncDraftTerms();
}

function getCurrentSerializedExpression(): string | null {
    const result = serializeForFormat(clauses.value, resolvedFormat.value);
    if (result.errorKey) {
        return null;
    }
    return result.expression;
}

function createLocalClauseId(): string {
    localClauseId += 1;
    return `rule-clause-local-${localClauseId}`;
}

function syncDraftTerms() {
    const nextDraftTerms: Record<string, string> = {};
    for (const clause of clauses.value) {
        nextDraftTerms[clause.id] = draftTerms.value[clause.id] ?? '';
    }
    draftTerms.value = nextDraftTerms;
}

function serializeKeywords() {
    if (isRawMode.value) {
        return;
    }
    const result = serializeForFormat(clauses.value, resolvedFormat.value);
    validationErrorKey.value = result.errorKey ?? '';
    if (result.errorKey) {
        return;
    }
    emit('update:modelValue', result.expression);
}

function addClause() {
    rawExpression.value = '';
    parseErrorKey.value = '';
    clauses.value.push(createRuleClause({ operator: 'OR' }, createLocalClauseId));
    syncDraftTerms();
    serializeKeywords();
}

function insertClauseAfterRow(clause: RuleClause) {
    clauses.value = insertClauseAfter(
        clauses.value,
        clause.id,
        createRuleClause({ operator: 'OR' }, createLocalClauseId)
    );
    syncDraftTerms();
    serializeKeywords();
}

function removeClause(clauseId: string) {
    clauses.value = clauses.value.filter(clause => clause.id !== clauseId);
    syncDraftTerms();
    serializeKeywords();
}

function replaceClause(updatedClause: RuleClause) {
    const clauseIndex = clauses.value.findIndex(clause => clause.id === updatedClause.id);
    if (clauseIndex === -1) {
        return;
    }
    clauses.value.splice(clauseIndex, 1, updatedClause);
}

function onTermsUpdated(clause: RuleClause) {
    clause.terms = normalizeRuleTerms(clause.terms);
    serializeKeywords();
}

function commitPendingTerm(clause: RuleClause) {
    const draftTerm = draftTerms.value[clause.id] ?? '';
    const updatedClause = addTermToClause(clause, draftTerm);
    replaceClause(updatedClause);
    draftTerms.value[clause.id] = '';
    serializeKeywords();
}

function increaseOpenParen(clause: RuleClause) {
    clause.openParens += 1;
    serializeKeywords();
}

function decreaseOpenParen(clause: RuleClause) {
    clause.openParens = Math.max(0, clause.openParens - 1);
    serializeKeywords();
}

function increaseCloseParen(clause: RuleClause) {
    clause.closeParens += 1;
    serializeKeywords();
}

function decreaseCloseParen(clause: RuleClause) {
    clause.closeParens = Math.max(0, clause.closeParens - 1);
    serializeKeywords();
}

function repeatParen(paren: '(' | ')', count: number): string {
    return paren.repeat(count) || '·';
}
</script>

<style scoped>
.rule-expression-input {
    width: 100%;
}

.rule-clause-row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
}

.clause-operator {
    flex: 0 0 148px;
    max-width: 148px;
}

.clause-terms {
    flex: 1 1 210px;
    min-width: 180px;
}

.paren-control {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: 0 0 auto;
    min-height: 40px;
}

.paren-control :deep(.v-btn) {
    min-width: 24px;
    padding-inline: 4px;
    font-family: monospace;
}

.paren-display {
    min-width: 28px;
    text-align: center;
    font-family: monospace;
    color: rgba(var(--v-theme-on-surface), 0.72);
}

@media (max-width: 720px) {
    .rule-clause-row {
        flex-wrap: wrap;
    }

    .clause-terms {
        flex-basis: 100%;
        order: 2;
    }
}
</style>
