<template>
    <v-select
        persistent-placeholder
        :readonly="readonly"
        :disabled="disabled"
        :label="label"
        :placeholder="placeholder || label"
        :clearable="clearable"
        :menu-props="{ contentClass: 'date-only-select-menu', offset: 0 }"
        v-model="dateValue"
        @click:clear="onClear"
    >
        <template #selection>
            <span class="text-truncate cursor-pointer">{{ displayDate }}</span>
        </template>

        <template #no-data>
            <DateTimePicker :is-dark-mode="isDarkMode"
                            :enable-time-picker="false"
                            :vertical="true"
                            :show-alternate-dates="true"
                            v-model="dateValue">
            </DateTimePicker>
        </template>
    </v-select>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useTheme } from 'vuetify';

import DateTimePicker from '@/components/common/DateTimePicker.vue';

import { isDarkApplicationTheme } from '@/core/theme.ts';

const props = defineProps<{
    modelValue: number;  // 毫秒时间戳
    disabled?: boolean;
    readonly?: boolean;
    label?: string;
    placeholder?: string;
    clearable?: boolean;  // 是否显示清除按钮
}>();

const emit = defineEmits<{
    (e: 'update:modelValue', value: number): void;
}>();

const theme = useTheme();

const isDarkMode = computed<boolean>(() => isDarkApplicationTheme(theme.global.name.value));

const dateValue = computed<Date>({
    get: () => {
        if (!props.modelValue || props.modelValue <= 0) {
            return new Date();
        }
        // modelValue 是毫秒时间戳，直接创建 Date 对象
        return new Date(props.modelValue);
    },
    set: (value: Date) => {
        // 转换回毫秒时间戳
        emit('update:modelValue', value.getTime());
    }
});

const displayDate = computed<string>(() => {
    if (!props.modelValue || props.modelValue <= 0) {
        return '';
    }
    // 使用中文日期格式：XXXX年XX月XX日
    const date = new Date(props.modelValue);
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    return `${year}年${month}月${day}日`;
});

/**
 * 清除日期
 */
function onClear(): void {
    emit('update:modelValue', 0);
}
</script>

<style scoped>
.cursor-pointer {
    cursor: pointer;
}
</style>

<style>
/* 日期选择菜单样式 - 参照DateTimeSelect实现，确保菜单紧贴输入框 */
.date-only-select-menu {
    max-height: inherit !important;
}

/* 移除日历边框 */
.date-only-select-menu .dp__menu {
    border: 0;
}

/* 确保v-list没有额外内边距 */
.date-only-select-menu .v-list {
    padding: 0;
}
</style>
