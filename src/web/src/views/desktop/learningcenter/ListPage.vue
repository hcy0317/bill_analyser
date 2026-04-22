<template>
    <div class="legacy-center-redirect" />
</template>

<script lang="ts" setup>
import { watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';

const route = useRoute();
const router = useRouter();

function redirectToPairingCenter(): void {
    const query: Record<string, string> = {};

    for (const [key, value] of Object.entries(route.query)) {
        if (typeof value === 'string' && key !== 'view') {
            query[key] = value;
        }
    }

    query['view'] = 'learning';

    if (!query['tab']) {
        query['tab'] = 'suggestions';
    }

    void router.replace({ path: '/pairing/list', query });
}

watch(
    () => route.fullPath,
    () => redirectToPairingCenter(),
    { immediate: true }
);
</script>
