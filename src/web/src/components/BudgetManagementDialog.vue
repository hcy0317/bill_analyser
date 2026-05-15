<template>
    <v-dialog v-model="showDialog" max-width="1200px" persistent scrollable>
        <v-card class="pa-6 pa-sm-8 pa-md-10">
            <template #title>
                <div class="d-flex align-center justify-space-between">
                    <div class="d-flex align-center">
                        <h4 class="text-h4">预算管理</h4>
                        <v-progress-circular indeterminate size="22" class="ms-2" v-if="loading"></v-progress-circular>
                    </div>
                    <v-btn
                        color="primary"
                        :prepend-icon="mdiPlus"
                        @click="openCreateDialog"
                        :disabled="loading || saving"
                    >
                        新建预算
                    </v-btn>
                </div>
            </template>

            <v-card-text class="pa-0">
                <!-- 预算概览卡片 -->
                <v-row v-if="budgetStatus" class="mb-6">
                    <v-col cols="12" sm="6" md="3">
                        <v-card color="success" variant="tonal">
                            <v-card-text>
                                <div class="text-caption">正常</div>
                                <div class="text-h5">{{ budgetStatus.summary?.normal || 0 }}</div>
                            </v-card-text>
                        </v-card>
                    </v-col>
                    <v-col cols="12" sm="6" md="3">
                        <v-card color="warning" variant="tonal">
                            <v-card-text>
                                <div class="text-caption">预警</div>
                                <div class="text-h5">{{ budgetStatus.summary?.warning || 0 }}</div>
                            </v-card-text>
                        </v-card>
                    </v-col>
                    <v-col cols="12" sm="6" md="3">
                        <v-card color="error" variant="tonal">
                            <v-card-text>
                                <div class="text-caption">临界</div>
                                <div class="text-h5">{{ budgetStatus.summary?.critical || 0 }}</div>
                            </v-card-text>
                        </v-card>
                    </v-col>
                    <v-col cols="12" sm="6" md="3">
                        <v-card color="error" variant="flat">
                            <v-card-text>
                                <div class="text-caption text-white">超支</div>
                                <div class="text-h5 text-white">{{ budgetStatus.summary?.exceeded || 0 }}</div>
                            </v-card-text>
                        </v-card>
                    </v-col>
                </v-row>

                <!-- 预算列表 -->
                <v-data-table
                    :headers="headers"
                    :items="budgets"
                    :loading="loading"
                    density="comfortable"
                    class="elevation-1"
                    :items-per-page="10"
                >
                    <template v-slot:[`item.name`]="{ item }">
                        <div class="font-weight-medium">{{ item.name }}</div>
                    </template>

                    <template v-slot:[`item.category`]="{ item }">
                        <v-chip size="small" variant="outlined">
                            {{ item.category || '全部' }}
                        </v-chip>
                    </template>

                    <template v-slot:[`item.period_type`]="{ item }">
                        <v-chip size="small" :color="getPeriodColor(item.period_type)">
                            {{ getPeriodName(item.period_type) }}
                        </v-chip>
                    </template>

                    <template v-slot:[`item.amount`]="{ item }">
                        <span class="font-weight-bold">{{ formatAmount(item.amount) }}</span>
                    </template>

                    <template v-slot:[`item.used`]="{ item }">
                        <div class="d-flex align-center gap-2">
                            <span :class="getUsedAmountClass(item)">
                                {{ formatAmount(item.used || 0) }}
                            </span>
                            <v-chip
                                size="x-small"
                                :color="getStatusColor(item.status)"
                                variant="flat"
                            >
                                {{ getUsagePercent(item) }}%
                            </v-chip>
                        </div>
                    </template>

                    <template v-slot:[`item.progress`]="{ item }">
                        <v-progress-linear
                            :model-value="getUsagePercent(item)"
                            :color="getStatusColor(item.status)"
                            height="8"
                            rounded
                        ></v-progress-linear>
                    </template>

                    <template v-slot:[`item.enabled`]="{ item }">
                        <v-switch
                            v-model="item.enabled"
                            hide-details
                            density="compact"
                            color="primary"
                            :disabled="loading || saving"
                            @change="toggleBudgetEnabled(item)"
                        ></v-switch>
                    </template>

                    <template v-slot:[`item.actions`]="{ item }">
                        <div class="d-flex gap-1">
                            <v-btn
                                :icon="mdiPencil"
                                size="small"
                                variant="text"
                                :disabled="loading || saving"
                                @click="openEditDialog(item)"
                            ></v-btn>
                            <v-btn
                                :icon="mdiDelete"
                                size="small"
                                variant="text"
                                color="error"
                                :disabled="loading || saving"
                                @click="confirmDelete(item)"
                            ></v-btn>
                        </div>
                    </template>

                    <template v-slot:no-data>
                        <div class="text-center py-8">
                            <v-icon icon="mdi-wallet-outline" size="64" color="grey"></v-icon>
                            <div class="text-h6 mt-4">暂无预算</div>
                            <div class="text-caption text-grey">点击"新建预算"开始管理您的预算</div>
                        </div>
                    </template>
                </v-data-table>

                <v-col cols="12" class="text-center mt-4">
                    <v-btn color="primary" :prepend-icon="mdiPlus" @click="openCreateDialog">
                        {{ tt('Add Budget') }}
                    </v-btn>
                </v-col>
            </v-card-text>

            <v-card-actions>
                <v-spacer></v-spacer>
                <v-btn
                    color="secondary"
                    variant="tonal"
                    :disabled="loading || saving"
                    @click="closeDialog"
                >
                    关闭
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- 创建/编辑预算对话框 -->
    <v-dialog v-model="showEditDialog" max-width="600px" persistent>
        <v-card>
            <v-card-title>
                <span class="text-h5">{{ editingBudget?.id ? '编辑预算' : '新建预算' }}</span>
            </v-card-title>

            <v-card-text>
                <v-form ref="budgetForm" v-model="formValid">
                    <v-row>
                        <v-col cols="12">
                            <v-text-field
                                v-model="editingBudget.name"
                                label="预算名称"
                                placeholder="例如：餐饮预算"
                                :rules="[rules.required]"
                                :disabled="saving"
                            ></v-text-field>
                        </v-col>

                        <v-col cols="12">
                            <v-text-field
                                v-model="editingBudget.category"
                                label="分类"
                                placeholder="留空表示全部分类"
                                :disabled="saving"
                            ></v-text-field>
                        </v-col>

                        <v-col cols="12" sm="6">
                            <v-select
                                v-model="editingBudget.period_type"
                                :items="periodTypes"
                                item-title="name"
                                item-value="value"
                                label="周期类型"
                                :rules="[rules.required]"
                                :disabled="saving"
                            ></v-select>
                        </v-col>

                        <v-col cols="12" sm="6">
                            <v-text-field
                                v-model.number="editingBudget.amount"
                                type="number"
                                label="预算金额"
                                prefix="¥"
                                :rules="[rules.required, rules.positive]"
                                :disabled="saving"
                            ></v-text-field>
                        </v-col>

                        <v-col cols="12" sm="6">
                            <v-text-field
                                v-model="editingBudget.start_date"
                                type="date"
                                label="开始日期"
                                :rules="[rules.required]"
                                :disabled="saving"
                            ></v-text-field>
                        </v-col>

                        <v-col cols="12" sm="6">
                            <v-text-field
                                v-model="editingBudget.end_date"
                                type="date"
                                label="结束日期"
                                :disabled="saving"
                            ></v-text-field>
                        </v-col>

                        <v-col cols="12" sm="6">
                            <v-text-field
                                v-model.number="editingBudget.warning_threshold"
                                type="number"
                                label="预警阈值 (%)"
                                suffix="%"
                                :disabled="saving"
                                hint="默认80%"
                            ></v-text-field>
                        </v-col>

                        <v-col cols="12" sm="6">
                            <v-text-field
                                v-model.number="editingBudget.critical_threshold"
                                type="number"
                                label="临界阈值 (%)"
                                suffix="%"
                                :disabled="saving"
                                hint="默认90%"
                            ></v-text-field>
                        </v-col>

                        <v-col cols="12">
                            <v-textarea
                                v-model="editingBudget.notes"
                                label="备注"
                                rows="2"
                                :disabled="saving"
                            ></v-textarea>
                        </v-col>

                        <v-col cols="12">
                            <v-switch
                                v-model="editingBudget.enabled"
                                label="启用"
                                color="primary"
                                :disabled="saving"
                            ></v-switch>
                        </v-col>
                    </v-row>
                </v-form>
            </v-card-text>

            <v-card-actions>
                <v-spacer></v-spacer>
                <v-btn
                    variant="text"
                    :disabled="saving"
                    @click="closeEditDialog"
                >
                    取消
                </v-btn>
                <v-btn
                    color="primary"
                    :disabled="!formValid || saving"
                    :loading="saving"
                    @click="saveBudget"
                >
                    保存
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- 确认删除对话框 -->
    <v-dialog v-model="showDeleteDialog" max-width="400px">
        <v-card>
            <v-card-title class="text-h5">确认删除</v-card-title>
            <v-card-text>
                确定要删除预算"{{ deletingBudget?.name }}"吗？此操作无法撤销。
            </v-card-text>
            <v-card-actions>
                <v-spacer></v-spacer>
                <v-btn variant="text" @click="showDeleteDialog = false">取消</v-btn>
                <v-btn color="error" :loading="saving" @click="deleteBudget">删除</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <v-snackbar v-model="snackbar.show" :color="snackbar.color" :timeout="3000">
        {{ snackbar.message }}
    </v-snackbar>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { useI18n } from '@/locales/helpers.ts';
import axios from 'axios';
import { mdiPlus, mdiPencil, mdiDelete } from '@mdi/js';

const { tt } = useI18n();

// 类型
interface Budget {
    id?: number;
    name: string;
    category?: string;
    period_type: string;
    amount: number;
    start_date: string;
    end_date?: string;
    warning_threshold?: number;
    critical_threshold?: number;
    notes?: string;
    enabled: boolean;
    used?: number;
    remaining?: number;
    status?: string;
    usage_ratio?: number;
}

interface BudgetStatus {
    budgets: Budget[];
    summary?: {
        normal: number;
        warning: number;
        critical: number;
        exceeded: number;
    };
}

type BudgetStatusKey = keyof NonNullable<BudgetStatus['summary']>;

// 属性
const props = defineProps<{
    modelValue: boolean;
    persistent?: boolean;
}>();

// 事件
const emit = defineEmits<{
    'update:modelValue': [value: boolean];
    'updated': [];
}>();

// 状态
const showDialog = computed({
    get: () => props.modelValue,
    set: (val: boolean) => emit('update:modelValue', val)
});

const loading = ref(false);
const saving = ref(false);
const budgets = ref<Budget[]>([]);
const budgetStatus = ref<BudgetStatus | null>(null);

const showEditDialog = ref(false);
const showDeleteDialog = ref(false);
const formValid = ref(false);
const editingBudget = ref<Budget>({
    name: '',
    period_type: 'monthly',
    amount: 0,
    start_date: new Date().toISOString().split('T')[0] || '',
    enabled: true
});
const deletingBudget = ref<Budget | null>(null);

const snackbar = ref({
    show: false,
    message: '',
    color: 'success'
});

// 表头
const headers = [
    { title: '名称', key: 'name', sortable: true },
    { title: '分类', key: 'category', sortable: true },
    { title: '周期', key: 'period_type', sortable: true },
    { title: '预算金额', key: 'amount', sortable: true },
    { title: '已使用', key: 'used', sortable: true },
    { title: '进度', key: 'progress', sortable: false },
    { title: '启用', key: 'enabled', sortable: false },
    { title: '操作', key: 'actions', sortable: false }
];

// 周期类型
const periodTypes = [
    { name: '每日', value: 'daily' },
    { name: '每周', value: 'weekly' },
    { name: '每月', value: 'monthly' },
    { name: '每季度', value: 'quarterly' },
    { name: '每年', value: 'yearly' },
    { name: '自定义', value: 'custom' }
];

// 校验规则
const rules = {
    required: (v: any) => !!v || '此项为必填',
    positive: (v: number) => v > 0 || '金额必须大于0'
};

// 方法
const formatAmount = (amount: number): string => {
    return new Intl.NumberFormat('zh-CN', {
        style: 'currency',
        currency: 'CNY'
    }).format(amount);
};

const getPeriodName = (type: string): string => {
    const period = periodTypes.find(p => p.value === type);
    return period ? period.name : type;
};

const getPeriodColor = (type: string): string => {
    const colors: Record<string, string> = {
        daily: 'blue',
        weekly: 'green',
        monthly: 'orange',
        quarterly: 'purple',
        yearly: 'red',
        custom: 'grey'
    };
    return colors[type] || 'grey';
};

const getStatusColor = (status?: string): string => {
    if (!status) return 'grey';
    const colors: Record<string, string> = {
        normal: 'success',
        warning: 'warning',
        critical: 'error',
        exceeded: 'error'
    };
    return colors[status] || 'grey';
};

const getUsedAmountClass = (budget: Budget): string => {
    const status = budget.status || 'normal';
    if (status === 'exceeded' || status === 'critical') {
        return 'text-error font-weight-bold';
    } else if (status === 'warning') {
        return 'text-warning font-weight-bold';
    }
    return 'text-success';
};

const getUsagePercent = (budget: Budget): number => {
    if (!budget.amount) return 0;
    const ratio = budget.usage_ratio || ((budget.used || 0) / budget.amount);
    return Math.round(ratio * 100);
};

const normalizeEnabled = (value: unknown): boolean => {
    if (value === false || value === 0 || value === '0') {
        return false;
    }
    return true;
};

const normalizeExecutionBudget = (item: any): Budget => {
    const amount = Number(item?.budget_amount ?? item?.amount ?? 0);
    const used = Number(item?.spent_amount ?? item?.used ?? 0);
    const usageRatio = amount > 0 ? used / amount : 0;
    const alertThreshold = Number(item?.alert_threshold ?? 80) / 100;
    const criticalThreshold = Number(item?.critical_threshold ?? 90) / 100;
    const status = usageRatio >= 1
        ? 'exceeded'
        : usageRatio >= criticalThreshold
            ? 'critical'
            : usageRatio >= alertThreshold
                ? 'warning'
                : 'normal';

    return {
        id: item?.id,
        name: item?.name || '',
        category: item?.sub_category ? `${item.category || ''}/${item.sub_category}` : (item?.category || ''),
        period_type: item?.period_type || 'monthly',
        amount,
        start_date: item?.start_date || '',
        end_date: item?.end_date || '',
        warning_threshold: item?.alert_threshold,
        critical_threshold: item?.critical_threshold,
        notes: item?.notes,
        enabled: normalizeEnabled(item?.enabled),
        used,
        remaining: Number(item?.remaining_amount ?? Math.max(0, amount - used)),
        status,
        usage_ratio: usageRatio
    };
};

const loadBudgetStatus = async (): Promise<void> => {
    try {
        const response = await axios.get('/api/budgets/execution');
        if (response.data.success) {
            const executionItems: Budget[] = Array.isArray(response.data.result?.items)
                ? response.data.result.items.map(normalizeExecutionBudget)
                : [];
            const summary = executionItems.reduce<NonNullable<BudgetStatus['summary']>>((counts, budget) => {
                const status = (budget.status || 'normal') as BudgetStatusKey;
                if (status === 'warning' || status === 'critical' || status === 'exceeded') {
                    counts[status] += 1;
                } else {
                    counts.normal += 1;
                }
                return counts;
            }, {
                normal: 0,
                warning: 0,
                critical: 0,
                exceeded: 0
            });

            budgetStatus.value = {
                budgets: executionItems,
                summary
            };
            budgets.value = executionItems;
        }
    } catch (error: any) {
        console.error('Failed to load budget status:', error);
    }
};

const openCreateDialog = (): void => {
    editingBudget.value = {
        name: '',
        period_type: 'monthly',
        amount: 0,
        start_date: new Date().toISOString().split('T')[0] || '',
        enabled: true
    };
    showEditDialog.value = true;
};

const openEditDialog = (budget: Budget): void => {
    editingBudget.value = { ...budget };
    showEditDialog.value = true;
};

const closeEditDialog = (): void => {
    showEditDialog.value = false;
    editingBudget.value = {
        name: '',
        period_type: 'monthly',
        amount: 0,
        start_date: new Date().toISOString().split('T')[0] || '',
        enabled: true
    };
};

const saveBudget = async (): Promise<void> => {
    saving.value = true;
    try {
        const isEdit = !!editingBudget.value.id;
        const url = isEdit ? `/api/budgets/${editingBudget.value.id}` : '/api/budgets';
        const method = isEdit ? 'put' : 'post';

        const response = await axios[method](url, editingBudget.value);

        if (response.data.success) {
            showSnackbar(isEdit ? '预算更新成功' : '预算创建成功', 'success');
            closeEditDialog();
            await loadBudgetStatus();
            emit('updated');
        }
    } catch (error: any) {
        showSnackbar(error.response?.data?.error || '保存失败', 'error');
    } finally {
        saving.value = false;
    }
};

const confirmDelete = (budget: Budget): void => {
    deletingBudget.value = budget;
    showDeleteDialog.value = true;
};

const deleteBudget = async (): Promise<void> => {
    if (!deletingBudget.value?.id) return;

    saving.value = true;
    try {
        const response = await axios.delete(`/api/budgets/${deletingBudget.value.id}`);

        if (response.data.success) {
            showSnackbar('预算删除成功', 'success');
            showDeleteDialog.value = false;
            deletingBudget.value = null;
            await loadBudgetStatus();
            emit('updated');
        }
    } catch (error: any) {
        showSnackbar(error.response?.data?.error || '删除失败', 'error');
    } finally {
        saving.value = false;
    }
};

const toggleBudgetEnabled = async (budget: Budget): Promise<void> => {
    if (!budget.id) return;

    saving.value = true;
    try {
        const response = await axios.put(`/api/budgets/${budget.id}`, {
            enabled: budget.enabled
        });

        if (response.data.success) {
            showSnackbar('状态更新成功', 'success');
            emit('updated');
        } else {
            // 回滚状态
            budget.enabled = !budget.enabled;
        }
    } catch (error: any) {
        // 回滚状态
        budget.enabled = !budget.enabled;
        showSnackbar(error.response?.data?.error || '更新失败', 'error');
    } finally {
        saving.value = false;
    }
};

const closeDialog = (): void => {
    showDialog.value = false;
};

const showSnackbar = (message: string, color: string = 'success'): void => {
    snackbar.value = { show: true, message, color };
};

// 生命周期
onMounted(() => {
    if (showDialog.value) {
        loadBudgetStatus();
    }
});

watch(showDialog, (newVal: boolean) => {
    if (newVal) {
        loadBudgetStatus();
    }
});

// 暴露接口
defineExpose({
    open: () => {
        showDialog.value = true;
    },
    refresh: loadBudgetStatus
});
</script>

<style scoped>
.gap-1 {
    gap: 4px;
}

.gap-2 {
    gap: 8px;
}
</style>
