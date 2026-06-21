<template>
    <div class="import-flow-progress cursor-default" aria-live="polite">
        <div class="d-flex align-start justify-space-between gap-4 flex-wrap">
            <div>
                <div class="text-caption text-medium-emphasis">
                    {{ importLabel }} {{ currentIndex + 1 }}/{{ items.length }}
                </div>
                <h5 class="text-subtitle-1 mb-1">{{ currentItem.title }}</h5>
                <div class="text-body-2 text-medium-emphasis">{{ detail }}</div>
            </div>
            <v-chip density="comfortable" color="primary" variant="tonal">
                {{ currentLabel }}
            </v-chip>
        </div>

        <v-progress-linear
            class="mt-4"
            color="primary"
            bg-opacity="0.12"
            rounded
            height="8"
            :model-value="progressValue"
        />

        <div class="import-flow-progress__trail mt-3" role="list" :aria-label="trailLabel">
            <div
                v-for="(item, index) in items"
                :key="item.key"
                role="listitem"
                :aria-current="item.active ? 'step' : undefined"
                :class="[
                    'import-flow-progress__step',
                    {
                        'import-flow-progress__step--active': item.active,
                        'import-flow-progress__step--complete': item.complete
                    }
                ]"
            >
                <span class="import-flow-progress__marker">{{ index + 1 }}</span>
                <span class="import-flow-progress__label">{{ item.title }}</span>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import type { ImportFlowProgressItem } from './types.ts';

defineProps<{
    currentIndex: number;
    currentItem: ImportFlowProgressItem;
    currentLabel: string;
    detail: string;
    importLabel: string;
    items: ImportFlowProgressItem[];
    progressValue: number;
    trailLabel: string;
}>();
</script>

<style scoped src="./ImportFlowProgress.scss"></style>
