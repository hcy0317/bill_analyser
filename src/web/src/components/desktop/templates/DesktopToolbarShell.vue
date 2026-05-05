<template>
    <div class="title-and-toolbar d-flex align-center flex-wrap ga-2">
        <div class="flex-grow-1 min-w-0">
            <slot name="title">
                <h1 class="text-h5 mb-0" v-if="title">{{ title }}</h1>
            </slot>
            <slot name="subtitle">
                <div class="text-body-2 text-medium-emphasis" v-if="subtitle">{{ subtitle }}</div>
            </slot>
        </div>
        <div class="d-flex align-center flex-wrap ga-2" v-if="$slots['actions']">
            <slot name="actions" />
        </div>
    </div>
</template>

<script setup lang="ts">
defineProps<{
    title?: string;
    subtitle?: string;
}>();
</script>
