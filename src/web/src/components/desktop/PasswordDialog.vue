<template>
    <v-dialog persistent min-width="400" max-width="500" v-model="showState">
        <v-card @keydown="onKeydown">
            <v-toolbar color="error">
                <v-toolbar-title>{{ titleContent }}</v-toolbar-title>
            </v-toolbar>
            <v-card-text class="pa-4 pb-6">
                <p v-if="textContent" class="mb-4">{{ textContent }}</p>

                <!-- 警告信息 -->
                <v-alert
                    v-if="warningContent"
                    type="warning"
                    variant="tonal"
                    class="mb-4"
                    density="compact"
                >
                    <template #text>
                        <div v-html="warningContent"></div>
                    </template>
                </v-alert>

                <!-- 密码输入框 -->
                <v-text-field
                    v-model="password"
                    :type="showPassword ? 'text' : 'password'"
                    :append-inner-icon="showPassword ? 'mdi-eye-off' : 'mdi-eye'"
                    :label="labelContent"
                    :placeholder="placeholderContent"
                    :error-messages="validationMessage"
                    variant="outlined"
                    density="comfortable"
                    autofocus
                    @click:append-inner="showPassword = !showPassword"
                    @keyup.enter="confirm"
                    @update:model-value="validationMessage = ''"
                ></v-text-field>

                <!-- 提示信息 -->
                <div v-if="hintContent" class="text-caption text-grey">
                    <v-icon size="small" class="mr-1">mdi-information-outline</v-icon>
                    <span v-html="hintContent"></span>
                </div>
            </v-card-text>

            <v-card-actions class="px-4 pb-4">
                <v-spacer></v-spacer>
                <v-btn color="gray" @click="cancel">{{ tt('Cancel') }}</v-btn>
                <v-btn color="error" :disabled="!password" @click="confirm">
                    {{ tt('Confirm') }}
                </v-btn>
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
    title?: string;
    text?: string;
    warning?: string;
}>();

const emit = defineEmits<{
    (e: 'update:show', value: boolean): void;
}>();

const { tt } = useI18n();

const showState = ref<boolean>(false);
const password = ref<string>('');
const showPassword = ref<boolean>(false);
const validationMessage = ref<string>('');
const titleContent = ref<string>(props.title || tt('Verify Operation Password'));
const textContent = ref<string>(props.text || '');
const warningContent = ref<string>(props.warning || '');
const labelContent = ref<string>(tt('Operation Password'));
const placeholderContent = ref<string>(tt('Please enter operation password'));
const hintContent = ref<string>(
    `${tt('Password can be configured via environment variable')}: `
    + '<code class="text-pink">BILL_ANALYSER_OPERATION_PASSWORD</code>'
);

let resolveFunc: ((value?: string) => void) | null = null;
let rejectFunc: ((reason?: unknown) => void) | null = null;

function open(
    titleOrText: string,
    textOrOptions?: string | Record<string, unknown>,
    options?: Record<string, unknown>
): Promise<string | undefined> {
    showState.value = true;
    password.value = '';
    validationMessage.value = '';
    showPassword.value = false;
    warningContent.value = '';
    labelContent.value = tt('Operation Password');
    placeholderContent.value = tt('Please enter operation password');
    hintContent.value = `${tt('Password can be configured via environment variable')}: `
        + '<code class="text-pink">BILL_ANALYSER_OPERATION_PASSWORD</code>';

    // 解析参数
    if (!textOrOptions || isObject(textOrOptions)) {
        // 只有一个参数或第二个参数是options
        titleContent.value = tt('Verify Operation Password');

        if (!textOrOptions) {
            textContent.value = tt(titleOrText);
        } else {
            const actualOptions = textOrOptions as Record<string, unknown>;
            textContent.value = tt(titleOrText, actualOptions);

            if (isString(actualOptions['warning'])) {
                warningContent.value = tt(actualOptions['warning'] as string, actualOptions);
            }
            if (isString(actualOptions['label'])) {
                labelContent.value = tt(actualOptions['label'] as string, actualOptions);
            }
            if (isString(actualOptions['placeholder'])) {
                placeholderContent.value = tt(actualOptions['placeholder'] as string, actualOptions);
            }
            if (isString(actualOptions['hint'])) {
                hintContent.value = tt(actualOptions['hint'] as string, actualOptions);
            }
        }
    } else if (isString(textOrOptions)) {
        // 第二个参数是text
        if (!options) {
            titleContent.value = tt(titleOrText);
            textContent.value = tt(textOrOptions);
        } else {
            titleContent.value = tt(titleOrText, options);
            textContent.value = tt(textOrOptions, options);

            if (isString(options['warning'])) {
                warningContent.value = tt(options['warning'] as string, options);
            }
            if (isString(options['label'])) {
                labelContent.value = tt(options['label'] as string, options);
            }
            if (isString(options['placeholder'])) {
                placeholderContent.value = tt(options['placeholder'] as string, options);
            }
            if (isString(options['hint'])) {
                hintContent.value = tt(options['hint'] as string, options);
            }
        }
    }

    return new Promise((resolve, reject) => {
        resolveFunc = resolve;
        rejectFunc = reject;
    });
}

function confirm(): void {
    if (!password.value) {
        validationMessage.value = tt('Password cannot be empty');
        return;
    }

    if (resolveFunc) {
        resolveFunc(password.value);
    }

    showState.value = false;
    emit('update:show', false);
}

function cancel(): void {
    if (rejectFunc) {
        rejectFunc();
    }

    showState.value = false;
    emit('update:show', false);
}

function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter' && password.value) {
        e.preventDefault();
        confirm();
    } else if (e.key === 'Escape' || e.key === 'Backspace') {
        // Backspace仅在非输入框焦点时触发取消
        const target = e.target as HTMLElement;
        if (e.key === 'Escape' || (e.key === 'Backspace' && target.tagName !== 'INPUT')) {
            e.preventDefault();
            cancel();
        }
    }
}

watch(() => props.show, (val) => {
    showState.value = !!val;
    if (val) {
        password.value = '';
        validationMessage.value = '';
        showPassword.value = false;
    }
});

defineExpose({
    open
});
</script>

<style scoped>
code {
    background-color: rgba(0, 0, 0, 0.05);
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 0.875rem;
}
</style>
