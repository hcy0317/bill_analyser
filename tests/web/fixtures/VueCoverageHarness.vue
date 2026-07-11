<template>
    <section data-testid="vue-coverage-harness">{{ (renderedLabel as string).toUpperCase() }}</section>
</template>

<script setup lang="ts">
const props = defineProps<{
    label: string;
}>();

const renderedLabel = props.label.trim();
</script>
