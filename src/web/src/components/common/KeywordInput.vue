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
                @click="addExpression"
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
            <div v-if="expressionGroups.length === 0" class="text-center text-caption text-medium-emphasis py-2">
                {{ resolvedEmptyStateText }}
            </div>

            <div v-else class="rule-expression-groups">
                <template v-for="(group, groupIndex) in expressionGroups" :key="group.id">
                    <div v-if="groupIndex > 0" class="expression-divider">
                        <span>{{ tt('OR') }}</span>
                    </div>

                    <div class="expression-group">
                        <div class="expression-clause-list">
                            <template v-for="(clause, clauseIndex) in group.clauses" :key="clause.id">
                                <div v-if="clauseIndex > 0" class="clause-separator">+</div>

                                <div class="rule-clause-row">
                                    <button
                                        v-if="supportsCompositeGrouping"
                                        type="button"
                                        class="paren-ghost paren-ghost-left"
                                        :class="{ 'paren-ghost--active': clause.openParens > 0 }"
                                        :disabled="disabled"
                                        :title="tt(clause.openParens > 0 ? 'Remove left parenthesis' : 'Add left parenthesis')"
                                        :aria-pressed="clause.openParens > 0"
                                        @click="toggleOpenParen(clause)"
                                    >
                                        (
                                    </button>

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
                                            :label="termsLabel"
                                            :placeholder="termsPlaceholder"
                                            chips
                                            closable-chips
                                            multiple
                                            density="compact"
                                            hide-details
                                            variant="outlined"
                                            append-inner-icon=""
                                            :disabled="disabled"
                                            @update:model-value="onTermsUpdated(clause)"
                                        />
                                    </div>

                                    <button
                                        v-if="supportsCompositeGrouping"
                                        type="button"
                                        class="paren-ghost paren-ghost-right"
                                        :class="{ 'paren-ghost--active': clause.closeParens > 0 }"
                                        :disabled="disabled"
                                        :title="tt(clause.closeParens > 0 ? 'Remove right parenthesis' : 'Add right parenthesis')"
                                        :aria-pressed="clause.closeParens > 0"
                                        @click="toggleCloseParen(clause)"
                                    >
                                        )
                                    </button>

                                    <div class="clause-actions">
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
                                </div>
                            </template>
                        </div>
                    </div>
                </template>
            </div>

            <v-alert
                v-if="validationErrorKey"
                type="error"
                variant="tonal"
                density="compact"
                class="mt-2 mb-2"
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
import { computed, ref, watch } from 'vue';
import { useI18n } from '@/locales/helpers.ts';
import {
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

interface RuleExpressionGroup {
    id: string;
    clauses: RuleClause[];
}

const props = withDefaults(defineProps<{
    modelValue: string;
    disabled?: boolean;
    regexEnabled?: boolean;
    expressionFormat?: ExpressionFormat;
    format?: ExpressionFormat;
    title?: string;
    emptyStateText?: string;
    helpText?: string;
    exampleText?: string;
    addButtonText?: string;
}>(), {
    format: 'legacy',
    regexEnabled: false,
});

const emit = defineEmits<{
    'update:modelValue': [value: string];
}>();

const resolvedFormat = computed<ExpressionFormat>(() => props.expressionFormat ?? props.format);
const resolvedTitle = computed(() => props.title || tt('Rule Expression'));
const resolvedAddButtonText = computed(() => props.addButtonText || tt('Add Expression'));
const resolvedEmptyStateText = computed(() => props.emptyStateText || tt('No rule clauses yet'));
const resolvedHelpText = computed(() => props.helpText || (
    resolvedFormat.value === 'composite'
        ? tt('Add one or more expressions. Each expression contains rule blocks joined by AND; expressions are joined by OR.')
        : tt('Add keyword chips with OR / AND / NOT blocks.')
));
const resolvedExampleText = computed(() => props.exampleText || (
    resolvedFormat.value === 'composite'
        ? tt('Example: OR={早餐,咖啡}+NOT={退款}|OR={午餐}')
        : ''
));
const termsLabel = computed(() => props.regexEnabled ? tt('Regex Patterns') : tt('Expression Terms'));
const termsPlaceholder = computed(() => props.regexEnabled
    ? tt('Enter regex patterns and press Enter')
    : tt('Enter terms and press Enter'));

const clauses = ref<RuleClause[]>([]);
const draftTerms = ref<Record<string, string>>({});
const rawExpression = ref('');
const parseErrorKey = ref('');
const validationErrorKey = ref('');
let localClauseId = 0;

const supportsCompositeGrouping = computed(() => resolvedFormat.value === 'composite');
const isRawMode = computed(() => rawExpression.value.length > 0);
const types = computed(() => {
    return [
        { title: tt('OR'), value: 'OR' as RuleOperator },
        { title: tt('AND'), value: 'AND' as RuleOperator },
        { title: tt('NOT'), value: 'NOT' as RuleOperator },
    ];
});
const expressionGroups = computed<RuleExpressionGroup[]>(() => {
    const groups: RuleExpressionGroup[] = [];
    let currentGroup: RuleExpressionGroup | null = null;
    let balance = 0;

    for (const clause of clauses.value) {
        const startsNewGroup = !currentGroup || (clause.joiner === 'OR' && balance === 0);
        if (startsNewGroup) {
            currentGroup = {
                id: `expression-${clause.id}`,
                clauses: [],
            };
            groups.push(currentGroup);
        }
        currentGroup!.clauses.push(clause);
        balance += Math.max(0, clause.openParens) - Math.max(0, clause.closeParens);
        if (balance < 0) {
            balance = 0;
        }
    }

    return groups;
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
    normalizeClauseJoiners();
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

function normalizeClauseJoiners() {
    if (clauses.value.length === 0) {
        return;
    }
    const firstClause = clauses.value[0];
    if (firstClause) {
        firstClause.joiner = 'AND';
    }
}

function serializeKeywords() {
    if (isRawMode.value) {
        return;
    }
    normalizeClauseJoiners();
    const result = serializeForFormat(clauses.value, resolvedFormat.value);
    validationErrorKey.value = result.errorKey ?? '';
    if (result.errorKey) {
        return;
    }
    emit('update:modelValue', result.expression);
}

function addExpression() {
    rawExpression.value = '';
    parseErrorKey.value = '';
    clauses.value.push(createRuleClause({
        joiner: clauses.value.length > 0 ? 'OR' : 'AND',
        operator: 'OR'
    }, createLocalClauseId));
    syncDraftTerms();
    serializeKeywords();
}

function insertClauseAfterRow(clause: RuleClause) {
    clauses.value = insertClauseAfter(
        clauses.value,
        clause.id,
        createRuleClause({ joiner: 'AND', operator: 'OR' }, createLocalClauseId)
    );
    syncDraftTerms();
    serializeKeywords();
}

function removeClause(clauseId: string) {
    const clauseIndex = clauses.value.findIndex(clause => clause.id === clauseId);
    if (clauseIndex === -1) {
        return;
    }
    const removedClause = clauses.value[clauseIndex];
    const nextClauses = clauses.value.filter(clause => clause.id !== clauseId);
    if (nextClauses.length > 0) {
        if (clauseIndex === 0) {
            nextClauses[0] = { ...nextClauses[0]!, joiner: 'AND' };
        } else if (removedClause?.joiner === 'OR' && clauseIndex < nextClauses.length) {
            nextClauses[clauseIndex] = { ...nextClauses[clauseIndex]!, joiner: 'OR' };
        }
    }
    clauses.value = nextClauses;
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

function toggleOpenParen(clause: RuleClause) {
    replaceClause({
        ...clause,
        openParens: clause.openParens > 0 ? 0 : 1,
    });
    serializeKeywords();
}

function toggleCloseParen(clause: RuleClause) {
    replaceClause({
        ...clause,
        closeParens: clause.closeParens > 0 ? 0 : 1,
    });
    serializeKeywords();
}
</script>

<style scoped>
.rule-expression-input {
    width: 100%;
}

.rule-expression-groups {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.expression-divider {
    display: flex;
    align-items: center;
    gap: 8px;
    color: rgba(var(--v-theme-on-surface), 0.58);
    font-size: 12px;
    font-weight: 600;
}

.expression-divider::before,
.expression-divider::after {
    content: '';
    flex: 1 1 auto;
    border-top: 1px dashed rgba(var(--v-theme-on-surface), 0.22);
}

.expression-divider span {
    padding: 0 8px;
}

.expression-group {
    border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
    border-radius: 12px;
    padding: 10px;
    background: rgba(var(--v-theme-surface), 0.72);
}

.expression-clause-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.clause-separator {
    align-self: center;
    color: rgba(var(--v-theme-on-surface), 0.72);
    font-size: 20px;
    font-weight: 700;
    line-height: 1;
    margin-block: -2px;
}

.rule-clause-row {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
    padding-inline: 18px;
}

.clause-operator {
    flex: 0 0 82px;
    max-width: 82px;
}

.clause-terms {
    flex: 1 1 210px;
    min-width: 180px;
}

.clause-actions {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
}

.paren-ghost {
    position: absolute;
    top: 2px;
    width: 16px;
    height: 38px;
    border: 0;
    padding: 0;
    background: transparent;
    color: rgba(var(--v-theme-on-surface), 0.52);
    cursor: pointer;
    font-family: ui-monospace, SFMono-Regular, Consolas, 'Liberation Mono', monospace;
    font-size: 30px;
    line-height: 34px;
    opacity: 0;
    transition: opacity 0.14s ease, color 0.14s ease;
}

.paren-ghost:disabled {
    cursor: default;
}

.paren-ghost-left {
    left: 0;
}

.paren-ghost-right {
    right: 0;
}

.rule-clause-row:hover .paren-ghost,
.paren-ghost--active {
    opacity: 0.38;
}

.rule-clause-row:hover .paren-ghost:hover,
.rule-clause-row:hover .paren-ghost--active {
    opacity: 0.78;
    color: rgba(var(--v-theme-primary), 0.86);
}

@media (max-width: 720px) {
    .rule-clause-row {
        flex-wrap: wrap;
    }

    .clause-terms {
        flex-basis: 100%;
        order: 2;
    }

    .clause-actions {
        margin-left: auto;
    }
}
</style>
