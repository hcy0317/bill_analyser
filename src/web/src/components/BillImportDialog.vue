<template>
    <v-dialog v-model="showDialog" max-width="900px" persistent>
        <v-card class="pa-6 pa-sm-8 pa-md-10">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center">
                        <h4 class="text-h4">导入账单</h4>
                        <v-progress-circular indeterminate size="22" class="ms-2" v-if="previewing || importing"></v-progress-circular>
                    </div>
                </div>
            </template>

            <div class="mt-4 cursor-default">
                <v-stepper v-model="currentStep" :items="stepItems" alt-labels class="elevation-0">
                    <!-- 步骤1: 选择文件 -->
                    <template v-slot:item.1>
                        <v-row class="mt-6">
                            <v-col cols="12">
                                <v-select
                                    v-model="selectedParser"
                                    :items="availableParsers"
                                    item-title="name"
                                    item-value="id"
                                    :disabled="uploading || previewing || importing"
                                    label="文件类型"
                                    placeholder="选择账单来源类型"
                                    persistent-placeholder
                                >
                                    <template v-slot:item="{ props, item }">
                                        <v-list-item v-bind="props">
                                            <template v-slot:subtitle>
                                                <div class="text-caption">{{ item.raw.description }}</div>
                                                <div class="text-caption text-grey">
                                                    支持格式: {{ item.raw.supported_formats.join(', ') }}
                                                </div>
                                            </template>
                                        </v-list-item>
                                    </template>
                                </v-select>
                            </v-col>

                            <v-col cols="12">
                                <v-file-input
                                    v-model="selectedFile"
                                    accept=".csv,.xlsx,.xls,.txt"
                                    label="数据文件"
                                    placeholder="点击选择账单文件"
                                    prepend-icon="mdi-file-upload"
                                    show-size
                                    persistent-placeholder
                                    :disabled="uploading || previewing || importing"
                                    :loading="uploading"
                                    @update:model-value="onFileChange"
                                />
                            </v-col>

                            <v-col cols="12" class="mb-0 pb-0" v-if="uploadError">
                                <v-alert type="error" density="compact">
                                    {{ uploadError }}
                                </v-alert>
                            </v-col>
                        </v-row>
                    </template>

                    <!-- 步骤2: 预览数据 -->
                    <template v-slot:item.2>
                        <div class="mt-6">
                            <div class="d-flex justify-space-between align-center mb-4">
                                <div>
                                    <h6 class="text-h6">预览数据</h6>
                                    <p class="text-caption text-grey ma-0">
                                        共解析到 {{ previewTotal }} 条账单，显示前10条
                                    </p>
                                </div>
                            </div>

                            <v-data-table
                                :headers="previewHeaders"
                                :items="previewItems"
                                :loading="previewing"
                                density="compact"
                                class="elevation-1"
                                :items-per-page="10"
                                hide-default-footer
                            >
                                <template v-slot:[`item.amount`]="{ item }">
                                    <span :class="getAmountClass(item)">
                                        {{ formatAmount(item.amount) }}
                                    </span>
                                </template>
                            </v-data-table>
                        </div>
                    </template>

                    <!-- 步骤3: 确认导入 -->
                    <template v-slot:item.3>
                        <div class="mt-6">
                            <v-alert type="info" class="mb-4" v-if="!importResult">
                                <h6 class="text-subtitle-1 font-weight-bold mb-2">导入摘要</h6>
                                <div class="text-body-2">
                                    <div>文件: {{ fileName }}</div>
                                    <div>解析器: {{ getParserName(selectedParser) }}</div>
                                    <div>总记录数: {{ previewTotal }}</div>
                                </div>
                            </v-alert>

                            <div v-if="importResult" class="mt-4">
                                <v-alert :type="importResult.success ? 'success' : 'error'" class="mb-4">
                                    <h6 class="text-subtitle-1 font-weight-bold mb-2">导入完成</h6>
                                    <div class="text-body-2">
                                        <div>总计: {{ importResult.total || 0 }}</div>
                                        <div>成功: {{ importResult.inserted || 0 }}</div>
                                        <div>重复: {{ importResult.duplicates || 0 }}</div>
                                        <div v-if="hasErrors">
                                            错误: {{ errorCount }}
                                        </div>
                                    </div>
                                </v-alert>

                                <v-expansion-panels v-if="hasErrors" class="mt-2">
                                    <v-expansion-panel>
                                        <v-expansion-panel-title>查看错误详情</v-expansion-panel-title>
                                        <v-expansion-panel-text>
                                            <v-list density="compact">
                                                <v-list-item v-for="(error, index) in importErrors" :key="index">
                                                    <v-list-item-title class="text-caption">
                                                        {{ formatError(error) }}
                                                    </v-list-item-title>
                                                </v-list-item>
                                            </v-list>
                                        </v-expansion-panel-text>
                                    </v-expansion-panel>
                                </v-expansion-panels>
                            </div>
                        </div>
                    </template>
                </v-stepper>
            </div>

            <div class="d-flex justify-sm-space-between gap-4 flex-wrap justify-center mt-10">
                <v-btn 
                    v-if="currentStep > 1 && currentStep < 3"
                    color="secondary" 
                    variant="tonal"
                    :disabled="previewing || importing"
                    @click="currentStep--"
                >
                    上一步
                </v-btn>
                <v-btn 
                    v-if="currentStep < 3"
                    color="secondary" 
                    variant="tonal" 
                    :disabled="previewing || importing"
                    @click="closeDialog"
                >
                    取消
                </v-btn>
                <v-btn
                    v-if="currentStep < 3"
                    class="button-icon-with-direction"
                    color="primary"
                    :disabled="!canProceed"
                    :append-icon="!previewing ? 'mdi-arrow-right' : undefined"
                    @click="nextStep"
                >
                    下一步
                    <v-progress-circular indeterminate size="22" class="ms-2" v-if="previewing"></v-progress-circular>
                </v-btn>
                <v-btn
                    v-if="currentStep === 3 && !importResult"
                    class="button-icon-with-direction"
                    color="teal"
                    :disabled="importing"
                    :append-icon="!importing ? 'mdi-arrow-right' : undefined"
                    @click="importBills"
                >
                    导入
                    <v-progress-circular indeterminate size="22" class="ms-2" v-if="importing"></v-progress-circular>
                </v-btn>
                <v-btn
                    v-if="currentStep === 3 && importResult"
                    color="secondary"
                    variant="tonal"
                    append-icon="mdi-check"
                    @click="closeDialog"
                >
                    关闭
                </v-btn>
            </div>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import axios from 'axios';

// Types
interface ParserInfo {
    id: string;
    name: string;
    description: string;
    supported_formats: string[];
}

interface PreviewBill {
    date: string;
    type: string;
    amount: number;
    counterparty: string;
    description: string;
    channel: string;
}

interface PreviewData {
    preview?: PreviewBill[];
    total?: number;
}

interface ImportResult {
    success: boolean;
    total?: number;
    inserted?: number;
    duplicates?: number;
    errors?: string[];
    error?: string;
}

// Props
const props = defineProps<{
    modelValue: boolean;
    persistent?: boolean;
}>();

// Emits
const emit = defineEmits<{
    'update:modelValue': [value: boolean];
    'imported': [];
}>();

// State
const showDialog = computed({
    get: () => props.modelValue,
    set: (val: boolean) => emit('update:modelValue', val)
});

const currentStep = ref(1);
const stepItems = [
    { title: '上传文件', value: 1 },
    { title: '预览数据', value: 2 },
    { title: '完成', value: 3 }
];

const selectedParser = ref('auto');
const selectedFile = ref<File[] | null>(null);
const availableParsers = ref<ParserInfo[]>([]);

const uploading = ref(false);
const previewing = ref(false);
const importing = ref(false);

const uploadError = ref('');
const previewData = ref<PreviewData>({});
const importResult = ref<ImportResult | null>(null);

// Headers for preview table
const previewHeaders = [
    { title: '日期', key: 'date', sortable: false },
    { title: '类型', key: 'type', sortable: false },
    { title: '金额', key: 'amount', sortable: false },
    { title: '对方', key: 'counterparty', sortable: false },
    { title: '描述', key: 'description', sortable: false },
    { title: '渠道', key: 'channel', sortable: false }
];

// Computed
const fileName = computed(() => {
    if (selectedFile.value && selectedFile.value.length > 0) {
        return selectedFile.value[0]?.name || '';
    }
    return '';
});

const previewItems = computed(() => previewData.value.preview || []);
const previewTotal = computed(() => previewData.value.total || 0);

const hasErrors = computed(() => {
    return importResult.value?.errors && importResult.value.errors.length > 0;
});

const errorCount = computed(() => {
    return importResult.value?.errors?.length || 0;
});

const importErrors = computed(() => {
    return importResult.value?.errors || [];
});

const canProceed = computed(() => {
    if (currentStep.value === 1) {
        return selectedFile.value && selectedFile.value.length > 0 && selectedParser.value;
    }
    if (currentStep.value === 2) {
        return previewTotal.value > 0;
    }
    return true;
});

// Methods
const getAmountClass = (item: PreviewBill): string => {
    return item.type === '支出' ? 'text-red' : 'text-green';
};

const formatError = (error: string | unknown): string => {
    return typeof error === 'string' ? error : JSON.stringify(error);
};

const loadParsers = async (): Promise<void> => {
    try {
        const response = await axios.get('/api/bills/import/parsers');
        if (response.data.success) {
            availableParsers.value = response.data.data;
        }
    } catch (error) {
        console.error('Failed to load parsers:', error);
    }
};

const onFileChange = (): void => {
    uploadError.value = '';
    previewData.value = {};
    importResult.value = null;
};

const uploadAndPreview = async (): Promise<boolean> => {
    if (!selectedFile.value || selectedFile.value.length === 0 || !selectedFile.value[0]) {
        uploadError.value = '请选择文件';
        return false;
    }

    previewing.value = true;
    uploadError.value = '';

    try {
        const formData = new FormData();
        const file = selectedFile.value[0];
        formData.append('file', file);
        formData.append('parser_type', selectedParser.value);
        formData.append('preview_only', 'true');

        const response = await axios.post('/api/bills/import/upload', formData, {
            headers: {
                'Content-Type': 'multipart/form-data'
            }
        });

        if (response.data.success) {
            previewData.value = response.data.data;
            return true;
        } else {
            uploadError.value = response.data.error || '预览失败';
            return false;
        }
    } catch (error: any) {
        uploadError.value = error.response?.data?.error || error.message || '预览失败';
        return false;
    } finally {
        previewing.value = false;
    }
};

const importBills = async (): Promise<void> => {
    if (!selectedFile.value || selectedFile.value.length === 0 || !selectedFile.value[0]) {
        return;
    }

    importing.value = true;

    try {
        const formData = new FormData();
        const file = selectedFile.value[0];
        formData.append('file', file);
        formData.append('parser_type', selectedParser.value);
        formData.append('preview_only', 'false');

        const response = await axios.post('/api/bills/import/upload', formData, {
            headers: {
                'Content-Type': 'multipart/form-data'
            }
        });

        importResult.value = response.data.data;

        if (response.data.success) {
            emit('imported');
        }
    } catch (error: any) {
        importResult.value = {
            success: false,
            error: error.response?.data?.error || error.message || '导入失败'
        };
    } finally {
        importing.value = false;
    }
};

const nextStep = async (): Promise<void> => {
    if (currentStep.value === 1) {
        const success = await uploadAndPreview();
        if (success) {
            currentStep.value++;
        }
    } else {
        currentStep.value++;
    }
};

const closeDialog = (): void => {
    currentStep.value = 1;
    selectedFile.value = null;
    previewData.value = {};
    importResult.value = null;
    uploadError.value = '';
    showDialog.value = false;
};

const getParserName = (parserId: string): string => {
    const parser = availableParsers.value.find((p: ParserInfo) => p.id === parserId);
    return parser ? parser.name : parserId;
};

const formatAmount = (amount: number): string => {
    return new Intl.NumberFormat('zh-CN', {
        style: 'currency',
        currency: 'CNY'
    }).format(amount);
};

// Lifecycle
onMounted(() => {
    loadParsers();
});

// Expose
defineExpose({
    open: () => {
        showDialog.value = true;
    }
});
</script>

<style scoped>
.text-red {
    color: rgb(var(--v-theme-error));
}

.text-green {
    color: rgb(var(--v-theme-success));
}

.button-icon-with-direction {
    flex-direction: row-reverse;
}

.cursor-default {
    cursor: default;
}
</style>
