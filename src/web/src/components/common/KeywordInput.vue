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
                @click="addGroup"
                :disabled="disabled"
            >
                {{ resolvedAddButtonText }}
            </v-btn>
        </div>

        <div v-if="groups.length === 0" class="text-center text-caption text-medium-emphasis py-2">
            {{ resolvedEmptyStateText }}
        </div>

        <div v-for="(group, index) in groups" :key="index" class="d-flex align-start mb-2">
            <div style="width: 120px" class="me-2">
                <v-select
                    v-model="group.type"
                    :items="types"
                    density="compact"
                    hide-details
                    variant="outlined"
                    @update:model-value="serializeKeywords"
                ></v-select>
            </div>
            <div class="flex-grow-1">
                <v-combobox
                    v-model="group.keywords"
                    :label="group.type === 'REGEX' ? tt('Regex Patterns') : tt('Expression Terms')"
                    :placeholder="group.type === 'REGEX' ? tt('Enter regex patterns and press Enter') : tt('Enter terms and press Enter')"
                    chips
                    closable-chips
                    multiple
                    density="compact"
                    hide-details
                    variant="outlined"
                    append-inner-icon=""
                    @update:model-value="serializeKeywords"
                ></v-combobox>
            </div>
            <div class="ms-2">
                <v-btn :icon="mdiDelete" size="small" variant="text" color="error" @click="removeGroup(index)"></v-btn>
            </div>
        </div>

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

import { mdiPlus, mdiDelete } from '@mdi/js';

const { tt } = useI18n();

const props = withDefaults(defineProps<{
    modelValue: string;
    disabled?: boolean;
    expressionFormat?: 'legacy' | 'composite';
    format?: 'legacy' | 'composite';
    title?: string;
    emptyStateText?: string;
    helpText?: string;
    exampleText?: string;
    addButtonText?: string;
}>(), {
    format: 'legacy',
});

const emit = defineEmits(['update:modelValue']);

type KeywordType = 'OR' | 'AND' | 'NOT' | 'REGEX';

interface KeywordGroup {
    type: KeywordType;
    keywords: string[];
}

const resolvedFormat = computed(() => props.expressionFormat ?? props.format);
const resolvedTitle = computed(() => props.title || tt('Rule Expression'));
const resolvedAddButtonText = computed(() => props.addButtonText || tt('Add Clause'));
const resolvedEmptyStateText = computed(() => props.emptyStateText || tt('No rule clauses yet'));
const resolvedHelpText = computed(() => props.helpText || (
    resolvedFormat.value === 'composite'
        ? tt('Build a boolean rule expression with OR / AND / NOT blocks. Regex clauses are also supported in composite mode.')
        : tt('Build a rule expression with OR / AND / NOT blocks. Legacy syntax is still accepted for compatibility.')
));
const resolvedExampleText = computed(() => props.exampleText || (
    resolvedFormat.value === 'composite'
        ? tt('Example: OR={早餐,咖啡}+NOT={退款}')
        : ''
));

const types = computed(() => {
    const base = [
        { title: tt('Match Any (OR)'), value: 'OR' as KeywordType },
        { title: tt('Require All (AND)'), value: 'AND' as KeywordType },
        { title: tt('Exclude (NOT)'), value: 'NOT' as KeywordType },
    ];
    if (resolvedFormat.value === 'composite') {
        base.push({ title: tt('Regex Clause'), value: 'REGEX' as KeywordType });
    }
    return base;
});

const groups = ref<KeywordGroup[]>([]);

watch(() => props.modelValue, (newVal: string) => {
    const currentSerialized = serialize(groups.value);
    if (newVal !== currentSerialized) {
        parseExpression(newVal);
    }
}, { immediate: true });

function detectFormat(str: string): 'composite' | 'legacy' {
    if (str.includes('={')) return 'composite';
    return 'legacy';
}

function parseExpression(str: string) {
    if (!str) {
        groups.value = [];
        return;
    }
    const fmt = detectFormat(str);
    if (fmt === 'composite') {
        parseComposite(str);
    } else {
        parseLegacy(str);
    }
}

function parseLegacy(str: string) {
    const result: KeywordGroup[] = [];
    const parts = str.split('&');
    for (const part of parts) {
        const trimmedPart = part.trim();
        if (!trimmedPart) continue;
        let type: KeywordType = 'OR';
        let content = trimmedPart;
        if (trimmedPart.startsWith('OR:')) {
            type = 'OR';
            content = trimmedPart.substring(3);
        } else if (trimmedPart.startsWith('AND:')) {
            type = 'AND';
            content = trimmedPart.substring(4);
        } else if (trimmedPart.startsWith('NOT:')) {
            type = 'NOT';
            content = trimmedPart.substring(4);
        }
        const keywords = content.split('|').map(k => k.trim()).filter(k => k);
        if (keywords.length > 0) {
            result.push({ type, keywords });
        }
    }
    groups.value = result;
}

function parseComposite(str: string) {
    const result: KeywordGroup[] = [];
    const blocks = str.split('+');
    for (const block of blocks) {
        const trimmed = block.trim();
        if (!trimmed) continue;
        const eqIdx = trimmed.indexOf('={');
        if (eqIdx === -1) {
            result.push({ type: 'OR', keywords: [trimmed] });
            continue;
        }
        const prefix = trimmed.substring(0, eqIdx).toUpperCase() as KeywordType;
        let content = trimmed.substring(eqIdx + 2);
        if (content.endsWith('}')) content = content.slice(0, -1);
        const keywords = content.split(',').map(k => k.trim()).filter(k => k);
        if (keywords.length > 0) {
            const type: KeywordType = ['OR', 'AND', 'NOT', 'REGEX'].includes(prefix) ? prefix : 'OR';
            result.push({ type, keywords });
        }
    }
    groups.value = result;
}

function serialize(currentGroups: KeywordGroup[]): string {
    if (resolvedFormat.value === 'legacy') {
        return serializeLegacy(currentGroups);
    }
    return serializeComposite(currentGroups);
}

function serializeLegacy(currentGroups: KeywordGroup[]): string {
    const parts: string[] = [];
    for (const group of currentGroups) {
        if (group.keywords.length > 0) {
            const typePrefix = `${group.type === 'REGEX' ? 'OR' : group.type}:`;
            parts.push(`${typePrefix}${group.keywords.join('|')}`);
        }
    }
    return parts.join('&');
}

function serializeComposite(currentGroups: KeywordGroup[]): string {
    const parts: string[] = [];
    for (const group of currentGroups) {
        if (group.keywords.length > 0) {
            parts.push(`${group.type}={${group.keywords.join(',')}}`);
        }
    }
    return parts.join('+');
}

function serializeKeywords() {
    const str = serialize(groups.value);
    emit('update:modelValue', str);
}

function addGroup() {
    groups.value.push({ type: 'OR', keywords: [] });
}

function removeGroup(index: number) {
    groups.value.splice(index, 1);
    serializeKeywords();
}
</script>

<style scoped>
.rule-expression-input {
    width: 100%;
}
</style>
