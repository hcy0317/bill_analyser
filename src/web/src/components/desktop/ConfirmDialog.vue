<template>
    <v-dialog persistent min-width="400" max-width="600" v-model="showState">
        <v-card @keydown="onKeydown">
            <v-toolbar :color="finalColor">
                <v-toolbar-title>{{ titleContent }}</v-toolbar-title>
            </v-toolbar>
            <v-card-text class="pa-4 pb-6">
                <p v-if="textContent">{{ textContent }}</p>

                <!-- 警告信息 -->
                <v-alert
                    v-if="warningContent"
                    type="warning"
                    variant="tonal"
                    class="mt-3"
                    density="compact"
                >
                    <template #text>
                        <div v-html="warningContent"></div>
                    </template>
                </v-alert>

                <!-- 详细信息列表 -->
                <v-list v-if="detailsContent && detailsContent.length > 0" dense class="mt-3">
                    <v-list-item v-for="(detail, index) in detailsContent" :key="index" class="px-0">
                        <template #prepend>
                            <v-icon size="small">mdi-circle-small</v-icon>
                        </template>
                        <v-list-item-title class="text-body-2">{{ detail }}</v-list-item-title>
                    </v-list-item>
                </v-list>
            </v-card-text>
            <v-card-actions class="px-4 pb-4">
                <v-spacer></v-spacer>
                <v-btn color="gray" @click="cancel">{{ tt('Cancel') }}</v-btn>
                <v-btn :color="finalColor" @click="confirm">{{ tt('OK') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';

import { useI18n } from '@/locales/helpers.ts';

import { isString, isObject } from '@/lib/common.ts';

const props = defineProps<{
    show?: boolean;
    color?: string;
    title?: string;
    text?: string;
}>();

const emit = defineEmits<{
    (e: 'update:show', value: boolean): void;
}>();

const { tt } = useI18n();

const showState = ref<boolean>(false);
const titleContent = ref<string>(props.title || tt('global.app.title'));
const textContent = ref<string>(props.text || '');
const warningContent = ref<string>('');
const detailsContent = ref<string[]>([]);
const finalColor = ref<string>(props.color || 'primary');

let resolveFunc: ((value?: unknown) => void) | null = null;

function open(titleOrText: string, textOrOptions?: string | Record<string, unknown>, options?: Record<string, unknown>): Promise<unknown> {
    showState.value = true;
    warningContent.value = '';
    detailsContent.value = [];

    if (!textOrOptions || isObject(textOrOptions)) { // only one parameter or second parameter is options
        titleContent.value = tt('global.app.title');

        if (!textOrOptions) {
            textContent.value = tt(titleOrText);
        } else {
            const actualOptions = textOrOptions as Record<string, unknown>;
            textContent.value = tt(titleOrText, actualOptions);

            // 提取警告和详情
            if (isString(actualOptions['warning'])) {
                warningContent.value = tt(actualOptions['warning'] as string, actualOptions);
            }
            if (Array.isArray(actualOptions['details'])) {
                detailsContent.value = (actualOptions['details'] as string[]).map(d => tt(d, actualOptions));
            }
        }
    } else if (isString(textOrOptions)) { // second parameter is text
        if (!options) {
            titleContent.value = tt(titleOrText);
            textContent.value = tt(textOrOptions);
        } else {
            titleContent.value = tt(titleOrText, options);
            textContent.value = tt(textOrOptions, options);

            // 提取警告和详情
            if (isString(options['warning'])) {
                warningContent.value = tt(options['warning'] as string, options);
            }
            if (Array.isArray(options['details'])) {
                detailsContent.value = (options['details'] as string[]).map(d => tt(d, options));
            }
        }
    }

    if (textOrOptions && isObject(textOrOptions) && isString(textOrOptions['color'])){
        finalColor.value = (textOrOptions['color'] as string) || 'primary';
    } else if (options && isString(options['color'])) {
        finalColor.value = (options['color'] as string) || 'primary';
    }

    return new Promise((resolve, _reject) => {
        resolveFunc = resolve;
        // reject用于Promise构造函数的必需参数，但当前取消操作通过resolve(false)处理
        void _reject;  // 标记为已使用
    });
}

function confirm(): void {
    if (resolveFunc) {
        resolveFunc(true);
    }

    showState.value = false;
    emit('update:show', false);
}

function cancel(): void {
    if (resolveFunc) {
        resolveFunc(false);
    }

    showState.value = false;
    emit('update:show', false);
}

function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
        e.preventDefault();
        confirm();
    } else if (e.key === 'Backspace') {
        e.preventDefault();
        cancel();
    } else if (e.key === 'Delete') {
        if (finalColor.value === 'error') {
            e.preventDefault();
            confirm();
        }
    }
}

watch(showState, newValue => {
    emit('update:show', newValue);
});

defineExpose({
    open
});
</script>
