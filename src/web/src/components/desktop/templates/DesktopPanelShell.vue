<template>
    <v-card>
        <v-card-title class="d-flex align-center flex-wrap ga-2" v-if="hasHeader">
            <div class="flex-grow-1 min-w-0">
                <slot name="title">
                    <span class="text-subtitle-1 font-weight-medium" v-if="title">{{ title }}</span>
                </slot>
                <slot name="subtitle">
                    <div class="text-body-2 text-medium-emphasis" v-if="subtitle">{{ subtitle }}</div>
                </slot>
            </div>
            <div class="d-flex align-center ga-2" v-if="$slots['actions']">
                <slot name="actions" />
            </div>
        </v-card-title>
        <v-divider v-if="hasHeader && divider" />
        <v-card-text :class="contentClass">
            <slot />
        </v-card-text>
        <template v-if="$slots['footer']">
            <v-divider v-if="divider" />
            <v-card-actions class="px-4 py-3">
                <slot name="footer" />
            </v-card-actions>
        </template>
    </v-card>
</template>

<script setup lang="ts">
import { computed, useSlots } from 'vue';

type ClassValue = string | string[] | Record<string, boolean>;

const props = withDefaults(defineProps<{
    title?: string;
    subtitle?: string;
    divider?: boolean;
    contentClass?: ClassValue;
}>(), {
    divider: true,
    contentClass: 'pa-4'
});

const slots = useSlots();

const hasHeader = computed(() => Boolean(
    props.title
    || props.subtitle
    || slots['title']
    || slots['subtitle']
    || slots['actions']
));
</script>
