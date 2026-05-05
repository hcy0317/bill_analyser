<template>
    <f7-page :ptr="ptr" @ptr:refresh="onPtrRefresh">
        <f7-navbar v-if="hasNavbar">
            <f7-nav-left :back-link="backLinkText" v-if="backLinkText"></f7-nav-left>
            <f7-nav-title :title="title" v-if="title"></f7-nav-title>
            <f7-nav-right class="navbar-compact-icons" v-if="$slots['actions']">
                <slot name="actions" />
            </f7-nav-right>
            <slot name="navbar" />
        </f7-navbar>
        <slot name="before" />
        <slot />
        <slot name="after" />
    </f7-page>
</template>

<script setup lang="ts">
import { computed, useSlots } from 'vue';

const props = withDefaults(defineProps<{
    title?: string;
    backLinkText?: string;
    ptr?: boolean;
}>(), {
    ptr: false
});

const emit = defineEmits<{
    (e: 'ptr:refresh', event: unknown): void;
}>();

const slots = useSlots();

const hasNavbar = computed(() => Boolean(
    props.title
    || props.backLinkText
    || slots['actions']
    || slots['navbar']
));

function onPtrRefresh(event: unknown): void {
    emit('ptr:refresh', event);
}
</script>
