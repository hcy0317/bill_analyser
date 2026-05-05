<template>
    <section class="d-flex flex-column ga-3">
        <div class="d-flex align-center flex-wrap ga-2" v-if="hasHeader">
            <div class="flex-grow-1 min-w-0">
                <slot name="title">
                    <h2 class="text-subtitle-1 font-weight-medium mb-0" v-if="title">{{ title }}</h2>
                </slot>
                <slot name="subtitle">
                    <p class="text-body-2 text-medium-emphasis mb-0" v-if="subtitle">{{ subtitle }}</p>
                </slot>
            </div>
            <div class="d-flex align-center ga-2" v-if="$slots['actions']">
                <slot name="actions" />
            </div>
        </div>
        <slot />
    </section>
</template>

<script setup lang="ts">
import { computed, useSlots } from 'vue';

const props = defineProps<{
    title?: string;
    subtitle?: string;
}>();

const slots = useSlots();

const hasHeader = computed(() => Boolean(
    props.title
    || props.subtitle
    || slots['title']
    || slots['subtitle']
    || slots['actions']
));
</script>
