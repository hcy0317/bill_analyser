<template>
    <div class="keyword-input">
        <div class="d-flex align-center mb-2">
            <span class="text-subtitle-2">{{ tt('Keyword Logic') }}</span>
            <v-spacer />
            <v-btn
                size="small"
                variant="text"
                color="primary"
                :prepend-icon="mdiPlus"
                @click="addGroup"
                :disabled="disabled"
            >
                {{ tt('Add') }}
            </v-btn>
        </div>

        <div v-if="groups.length === 0" class="text-center text-caption text-medium-emphasis py-2">
            {{ tt('No keywords set. Click "Add Condition" to start.') }}
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
                    :label="tt('Keywords')"
                    placeholder="Type and press Enter"
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
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { useI18n } from '@/locales/helpers.ts';

import { mdiPlus, mdiDelete } from '@mdi/js';

const { tt } = useI18n();

const props = defineProps<{
    modelValue: string;
    disabled?: boolean;
}>();

const emit = defineEmits(['update:modelValue']);

type KeywordType = 'OR' | 'AND' | 'NOT';

interface KeywordGroup {
    type: KeywordType;
    keywords: string[];
}

const types = computed(() => [
    { title: 'OR', value: 'OR' },
    { title: 'AND', value: 'AND' },
    { title: 'NOT', value: 'NOT' }
]);

const groups = ref<KeywordGroup[]>([]);

watch(() => props.modelValue, (newVal: string) => {
    const currentSerialized = serializeToString(groups.value);
    if (newVal !== currentSerialized) {
        parseKeywords(newVal);
    }
}, { immediate: true });

function parseKeywords(str: string) {
    if (!str) {
        groups.value = [];
        return;
    }

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

function serializeToString(currentGroups: KeywordGroup[]): string {
    const parts: string[] = [];

    for (const group of currentGroups) {
        if (group.keywords.length > 0) {
            const typePrefix = group.type === 'OR' ? 'OR:' : (group.type === 'AND' ? 'AND:' : 'NOT:');
            parts.push(`${typePrefix}${group.keywords.join('|')}`);
        }
    }

    return parts.join('&');
}

function serializeKeywords() {
    const str = serializeToString(groups.value);
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
.keyword-input {
    width: 100%;
}
</style>
