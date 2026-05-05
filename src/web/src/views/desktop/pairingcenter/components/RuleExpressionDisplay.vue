<template>
    <div class="rule-expression-stack">
        <div
            v-for="(expressionGroup, expressionIndex) in groups"
            :key="expressionIndex"
            class="rule-expression-line"
        >
            <template
                v-for="(clause, clauseIndex) in expressionGroup.clauses"
                :key="`${expressionIndex}-${clauseIndex}`"
            >
                <span
                    v-if="clause.connectorLabel"
                    class="rule-expression-connector"
                >
                    {{ clause.connectorLabel }}
                </span>
                <span
                    v-if="clause.openParenLabel"
                    class="rule-expression-paren rule-expression-paren--open"
                >
                    {{ clause.openParenLabel }}
                </span>
                <v-chip
                    class="rule-expression-operator"
                    size="x-small"
                    label
                    variant="tonal"
                    :color="getRuleExpressionDisplayOperatorColor(clause)"
                >
                    {{ clause.label }}
                </v-chip>
                <v-chip
                    v-for="term in getVisibleExpressionTerms(expressionIndex, clauseIndex, clause.terms)"
                    :key="term"
                    size="x-small"
                    variant="tonal"
                    color="primary"
                    class="rule-expression-term"
                    :title="term"
                >
                    {{ term }}
                </v-chip>
                <v-chip
                    v-if="getHiddenExpressionTermCount(expressionIndex, clauseIndex, clause.terms) > 0"
                    size="x-small"
                    label
                    variant="outlined"
                    class="rule-expression-more"
                    @click="toggleExpressionClauseExpanded(expressionIndex, clauseIndex)"
                >
                    +{{ getHiddenExpressionTermCount(expressionIndex, clauseIndex, clause.terms) }}
                </v-chip>
                <span
                    v-if="clause.closeParenLabel"
                    class="rule-expression-paren rule-expression-paren--close"
                >
                    {{ clause.closeParenLabel }}
                </span>
            </template>
        </div>
    </div>
</template>

<script setup lang="ts">
import { getRuleExpressionDisplayOperatorColor } from '@/components/common/keywordExpression.ts';
import type { RuleExpressionDisplayGroup } from './ruleExpressionDisplay.ts';

const props = withDefaults(defineProps<{
    ruleId: number;
    groups: RuleExpressionDisplayGroup[];
    expandedClauseKeys: string[];
    maxCollapsedTerms?: number;
}>(), {
    maxCollapsedTerms: 4,
});

const emit = defineEmits<{
    'update:expandedClauseKeys': [value: string[]];
}>();

function makeExpressionClauseKey(expressionIndex: number, clauseIndex: number): string {
    return `${props.ruleId}:${expressionIndex}:${clauseIndex}`;
}

function isExpressionClauseExpanded(expressionIndex: number, clauseIndex: number): boolean {
    return props.expandedClauseKeys.includes(makeExpressionClauseKey(expressionIndex, clauseIndex));
}

function toggleExpressionClauseExpanded(expressionIndex: number, clauseIndex: number): void {
    const key = makeExpressionClauseKey(expressionIndex, clauseIndex);
    const nextKeys = props.expandedClauseKeys.includes(key)
        ? props.expandedClauseKeys.filter(item => item !== key)
        : [...props.expandedClauseKeys, key];
    emit('update:expandedClauseKeys', nextKeys);
}

function getVisibleExpressionTerms(
    expressionIndex: number,
    clauseIndex: number,
    terms: string[]
): string[] {
    if (isExpressionClauseExpanded(expressionIndex, clauseIndex)) {
        return terms;
    }

    return terms.slice(0, props.maxCollapsedTerms);
}

function getHiddenExpressionTermCount(
    expressionIndex: number,
    clauseIndex: number,
    terms: string[]
): number {
    if (isExpressionClauseExpanded(expressionIndex, clauseIndex)) {
        return 0;
    }

    return Math.max(0, terms.length - props.maxCollapsedTerms);
}
</script>

<style scoped>
.rule-expression-stack {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-block: 6px;
}

.rule-expression-line {
    display: flex;
    align-items: center;
    gap: 6px;
    width: fit-content;
    max-width: 100%;
    padding: 4px 6px;
    border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
    border-radius: 8px;
    background: rgba(var(--v-theme-surface), 1);
    overflow: hidden;
}

.rule-expression-connector,
.rule-expression-paren {
    flex: 0 0 auto;
    font-family: ui-monospace, SFMono-Regular, Consolas, 'Liberation Mono', monospace;
    font-size: 0.78rem;
    font-weight: 700;
    line-height: 1;
    color: rgba(var(--v-theme-primary), 0.86);
}

.rule-expression-connector {
    min-width: 12px;
    text-align: center;
}

.rule-expression-paren {
    letter-spacing: 1px;
}

.rule-expression-operator {
    flex: 0 0 auto;
}

.rule-expression-term {
    flex: 0 1 auto;
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border: 1px solid rgba(var(--v-theme-primary), 0.18);
}

.rule-expression-term :deep(.v-chip__content) {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.rule-expression-more {
    cursor: pointer;
    flex: 0 0 auto;
}
</style>
