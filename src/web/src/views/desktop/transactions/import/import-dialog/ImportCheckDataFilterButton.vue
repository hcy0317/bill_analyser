<template>
    <v-btn
        density="comfortable"
        color="default"
        variant="text"
        class="ms-2"
        :icon="true"
        :disabled="disabled"
    >
        <v-icon :icon="mdiFilterOutline" />
        <v-menu
            activator="parent"
            max-height="500"
            min-width="360"
            v-model="visibleProxy"
            :close-on-content-click="false"
        >
            <v-list
                density="compact"
                class="py-1 import-check-data-filter-menu"
                v-model:opened="openedProxy"
                open-strategy="multiple"
            >
                <v-list-group v-for="group in filterMenus" :key="group.title" :value="group.title">
                    <template #activator="{ props: groupActivatorProps }">
                        <v-list-item v-bind="groupActivatorProps" class="import-check-data-filter-menu__group">
                            <template #title>
                                <span
                                    :class="{
                                        'import-check-data-filter-menu__group-title': true,
                                        'import-check-data-filter-menu__group-title--active': isActiveSummary(group.summary)
                                    }"
                                >
                                    {{ group.title }}
                                </span>
                            </template>
                        </v-list-item>
                    </template>

                    <template v-for="(menu, index) in group.items" :key="`${group.title}_${index}`">
                        <v-list-group v-if="menu.items?.length" :value="`${group.title}_${menu.title}`">
                            <template #activator="{ props: childGroupActivatorProps }">
                                <v-list-item
                                    v-bind="childGroupActivatorProps"
                                    :prepend-icon="menu.prependIcon"
                                    :title="menu.title"
                                    :subtitle="menu.subTitle"
                                    :disabled="menu.disabled"
                                    class="import-check-data-filter-menu__item"
                                />
                            </template>
                            <v-list-item
                                v-for="(childMenu, childIndex) in menu.items"
                                :key="`${group.title}_${menu.title}_${childIndex}`"
                                :prepend-icon="childMenu.prependIcon"
                                :title="childMenu.title"
                                :subtitle="childMenu.subTitle"
                                :append-icon="childMenu.appendIcon"
                                :disabled="childMenu.disabled"
                                class="import-check-data-filter-menu__item"
                                @click="childMenu.onClick?.()"
                            />
                        </v-list-group>
                        <v-list-item
                            v-else
                            :prepend-icon="menu.prependIcon"
                            :title="menu.title"
                            :subtitle="menu.subTitle"
                            :append-icon="menu.appendIcon"
                            :disabled="menu.disabled"
                            class="import-check-data-filter-menu__item"
                            @click="menu.onClick?.()"
                        />
                    </template>
                </v-list-group>
            </v-list>
        </v-menu>
    </v-btn>
</template>

<script setup lang="ts">
import { computed } from 'vue';

import type { ImportTransactionCheckDataMenuGroup } from '../checkDataTypes.ts';
import { mdiFilterOutline } from '@mdi/js';

const props = defineProps<{
    disabled: boolean;
    filterMenus: ImportTransactionCheckDataMenuGroup[];
    isActiveSummary: (summary?: string) => boolean;
    opened: string[];
    visible: boolean;
}>();

const emit = defineEmits<{
    (event: 'update:opened', value: string[]): void;
    (event: 'update:visible', value: boolean): void;
}>();

// 组件内部只代理 Vuetify 菜单状态，筛选项本身仍由 CheckData tab 生成并执行。
const visibleProxy = computed<boolean>({
    get: () => props.visible,
    set: value => emit('update:visible', value)
});

const openedProxy = computed<string[]>({
    get: () => props.opened,
    set: value => emit('update:opened', value)
});
</script>
