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
                    </div>

                    <div class="expression-group">
                        <div class="expression-clause-list">
                            <template v-for="(clause, clauseIndex) in group.clauses" :key="clause.id">
                                <div
                                    v-if="clauseIndex > 0"
                                    class="clause-connector-row"
                                >
                                    <v-btn-toggle
                                        :model-value="getClauseConnector(clause)"
                                        class="clause-connector-toggle"
                                        density="compact"
                                        variant="outlined"
                                        divided
                                        mandatory
                                        :disabled="disabled"
                                        @update:model-value="updateClauseConnector(clause, $event)"
                                    >
                                        <v-btn
                                            v-for="item in connectorItems"
                                            :key="item.value"
                                            :value="item.value"
                                            size="small"
                                            :disabled="disabled"
                                        >
                                            {{ item.title }}
                                        </v-btn>
                                    </v-btn-toggle>
                                </div>

                                <div class="rule-clause-row">
                                    <div
                                        v-if="supportsCompositeGrouping"
                                        class="paren-control paren-control-left"
                                        :class="{ 'paren-control--active': clause.openParens > 0 }"
                                    >
                                        <button
                                            type="button"
                                            class="paren-control__step"
                                            :disabled="disabled || clause.openParens <= 0"
                                            :title="tt('Remove left parenthesis')"
                                            @click="decrementOpenParen(clause)"
                                        >
                                            −
                                        </button>
                                        <button
                                            type="button"
                                            class="paren-control__main"
                                            :disabled="disabled"
                                            :title="tt('Add left parenthesis')"
                                            :aria-pressed="clause.openParens > 0"
                                            @click="incrementOpenParen(clause)"
                                        >
                                            {{ formatParenStack('(', clause.openParens) }}
                                        </button>
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

                                    <div
                                        v-if="supportsCompositeGrouping"
                                        class="paren-control paren-control-right"
                                        :class="{ 'paren-control--active': clause.closeParens > 0 }"
                                    >
                                        <button
                                            type="button"
                                            class="paren-control__main"
                                            :disabled="disabled"
                                            :title="tt('Add right parenthesis')"
                                            :aria-pressed="clause.closeParens > 0"
                                            @click="incrementCloseParen(clause)"
                                        >
                                            {{ formatParenStack(')', clause.closeParens) }}
                                        </button>
                                        <button
                                            type="button"
                                            class="paren-control__step"
                                            :disabled="disabled || clause.closeParens <= 0"
                                            :title="tt('Remove right parenthesis')"
                                            @click="decrementCloseParen(clause)"
                                        >
                                            −
                                        </button>
                                    </div>

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

        <div v-if="resolvedHelpText" class="text-caption text-medium-emphasis mt-2">
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
const resolvedTitle = computed(() => props.title || tt('Rule Matching Expression'));
const resolvedAddButtonText = computed(() => props.addButtonText || tt('Add Rule'));
const resolvedEmptyStateText = computed(() => props.emptyStateText || tt('No rule clauses yet'));
const resolvedHelpText = computed(() => props.helpText ?? '');
const resolvedExampleText = computed(() => props.exampleText ?? '');
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
type ClauseConnector = 'AND' | 'OR' | 'NOT';
const connectorItems = [
    { title: '+', value: 'AND' as ClauseConnector },
    { title: '/', value: 'OR' as ClauseConnector },
    { title: '×', value: 'NOT' as ClauseConnector },
];
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
        const startsNewGroup = !currentGroup || (clause.startsExpression && balance === 0);
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

    if (result.sourceFormat === 'composite' && !result.errorKey) {
        const repairedExpression = getCurrentSerializedExpression();
        if (repairedExpression !== null && repairedExpression !== value.trim()) {
            emit('update:modelValue', repairedExpression);
        }
    }
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
        firstClause.negated = false;
        firstClause.startsExpression = false;
    }
}

function getClauseConnector(clause: RuleClause): ClauseConnector {
    if (clause.negated) {
        return 'NOT';
    }
    if (clause.joiner === 'OR') {
        return 'OR';
    }
    return 'AND';
}

function updateClauseConnector(clause: RuleClause, value: unknown) {
    const connector = value === 'OR' || value === 'NOT' ? value : 'AND';
    replaceClause({
        ...clause,
        joiner: connector === 'OR' ? 'OR' : 'AND',
        negated: connector === 'NOT',
        startsExpression: false,
    });
    serializeKeywords();
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
        operator: 'OR',
        startsExpression: clauses.value.length > 0
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
            nextClauses[0] = { ...nextClauses[0]!, joiner: 'AND', negated: false, startsExpression: false };
        } else if ((removedClause?.joiner === 'OR' || removedClause?.negated) && clauseIndex < nextClauses.length) {
            nextClauses[clauseIndex] = {
                ...nextClauses[clauseIndex]!,
                joiner: removedClause.joiner,
                negated: removedClause.negated,
                startsExpression: removedClause.startsExpression,
            };
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

function incrementOpenParen(clause: RuleClause) {
    replaceClause({
        ...clause,
        openParens: clause.openParens + 1,
    });
    serializeKeywords();
}

function decrementOpenParen(clause: RuleClause) {
    replaceClause({
        ...clause,
        openParens: Math.max(0, clause.openParens - 1),
    });
    serializeKeywords();
}

function incrementCloseParen(clause: RuleClause) {
    replaceClause({
        ...clause,
        closeParens: clause.closeParens + 1,
    });
    serializeKeywords();
}

function decrementCloseParen(clause: RuleClause) {
    replaceClause({
        ...clause,
        closeParens: Math.max(0, clause.closeParens - 1),
    });
    serializeKeywords();
}

function formatParenStack(paren: '(' | ')', count: number): string {
    if (count <= 0) {
        return paren;
    }
    if (count <= 3) {
        return paren.repeat(count);
    }
    return `${paren.repeat(3)}×${count}`;
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
    color: rgba(var(--v-theme-primary), 0.86);
    font-size: 12px;
    font-weight: 600;
    padding-block: 2px;
}

.expression-divider::before,
.expression-divider::after {
    content: '';
    flex: 1 1 auto;
    border-top: 2px dashed rgba(var(--v-theme-primary), 0.42);
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

.clause-connector-row {
    align-self: center;
    margin-block: -1px;
}

.clause-connector-toggle {
    background: rgba(var(--v-theme-surface), 1);
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

.paren-control {
    position: absolute;
    top: 2px;
    display: flex;
    align-items: stretch;
    width: 34px;
    height: 38px;
    opacity: 0;
    transition: opacity 0.14s ease;
}

.paren-control-left {
    left: 0;
}

.paren-control-right {
    right: 0;
}

.paren-control__step,
.paren-control__main {
    border: 0;
    padding: 0;
    background: transparent;
    color: rgba(var(--v-theme-on-surface), 0.52);
    cursor: pointer;
    font-family: ui-monospace, SFMono-Regular, Consolas, 'Liberation Mono', monospace;
    line-height: 1;
}

.paren-control__step:disabled,
.paren-control__main:disabled {
    cursor: default;
    opacity: 0.36;
}

.paren-control__step {
    flex: 0 0 12px;
    font-size: 13px;
}

.paren-control__main {
    flex: 1 1 auto;
    min-width: 0;
    font-size: 24px;
    font-weight: 700;
}

.rule-clause-row:hover .paren-control,
.paren-control--active {
    opacity: 0.38;
}

.paren-control--active {
    border-radius: 8px;
    background: rgba(var(--v-theme-primary), 0.22);
    color: rgb(var(--v-theme-primary));
    opacity: 1;
    box-shadow: inset 0 0 0 1px rgba(var(--v-theme-primary), 0.55);
}

.paren-control--active .paren-control__step,
.paren-control--active .paren-control__main {
    color: rgb(var(--v-theme-primary));
}

.rule-clause-row:hover .paren-control:hover,
.rule-clause-row:hover .paren-control--active {
    opacity: 1;
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
