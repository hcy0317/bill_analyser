<template>
    <f7-page>
        <f7-navbar :title="tt('Budget Management')" back-link="Back"></f7-navbar>

        <f7-block strong inset class="text-align-center margin-top">
            <p>{{ tt('Budget Management') }}</p>
        </f7-block>
    </f7-page>
</template>

<script setup lang="ts">
import { useI18n } from '@/locales/helpers.ts';

const { tt } = useI18n();
</script>
