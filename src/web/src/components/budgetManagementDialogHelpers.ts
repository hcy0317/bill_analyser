export interface Budget {
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

export interface BudgetStatus {
    budgets: Budget[];
    summary?: {
        normal: number;
        warning: number;
        critical: number;
        exceeded: number;
    };
}

export type BudgetStatusKey = keyof NonNullable<BudgetStatus['summary']>;

interface BudgetExecutionSource {
    id?: number;
    name?: string;
    category?: string;
    sub_category?: string;
    period_type?: string;
    budget_amount?: number;
    amount?: number;
    spent_amount?: number;
    used?: number;
    remaining_amount?: number;
    alert_threshold?: number;
    critical_threshold?: number;
    start_date?: string;
    end_date?: string;
    notes?: string;
    enabled?: unknown;
}

export const headers = [
    { title: '名称', key: 'name', sortable: true },
    { title: '分类', key: 'category', sortable: true },
    { title: '周期', key: 'period_type', sortable: true },
    { title: '预算金额', key: 'amount', sortable: true },
    { title: '已使用', key: 'used', sortable: true },
    { title: '进度', key: 'progress', sortable: false },
    { title: '启用', key: 'enabled', sortable: false },
    { title: '操作', key: 'actions', sortable: false }
];

export const periodTypes = [
    { name: '每日', value: 'daily' },
    { name: '每周', value: 'weekly' },
    { name: '每月', value: 'monthly' },
    { name: '每季度', value: 'quarterly' },
    { name: '每年', value: 'yearly' },
    { name: '自定义', value: 'custom' }
];

export const rules = {
    required: (value: unknown): true | string => !!value || '此项为必填',
    positive: (value: number): true | string => value > 0 || '金额必须大于0'
};

export function formatAmount(amount: number): string {
    return new Intl.NumberFormat('zh-CN', {
        style: 'currency',
        currency: 'CNY'
    }).format(amount);
}

export function getPeriodName(type: string): string {
    const period = periodTypes.find(item => item.value === type);
    return period ? period.name : type;
}

export function getPeriodColor(type: string): string {
    const colors: Record<string, string> = {
        daily: 'blue',
        weekly: 'green',
        monthly: 'orange',
        quarterly: 'purple',
        yearly: 'red',
        custom: 'grey'
    };
    return colors[type] || 'grey';
}

export function getStatusColor(status?: string): string {
    if (!status) {
        return 'grey';
    }

    const colors: Record<string, string> = {
        normal: 'success',
        warning: 'warning',
        critical: 'error',
        exceeded: 'error'
    };
    return colors[status] || 'grey';
}

export function getUsedAmountClass(budget: Budget): string {
    const status = budget.status || 'normal';
    if (status === 'exceeded' || status === 'critical') {
        return 'text-error font-weight-bold';
    } else if (status === 'warning') {
        return 'text-warning font-weight-bold';
    }
    return 'text-success';
}

export function getUsagePercent(budget: Budget): number {
    if (!budget.amount) {
        return 0;
    }

    const ratio = budget.usage_ratio || ((budget.used || 0) / budget.amount);
    return Math.round(ratio * 100);
}

export function normalizeEnabled(value: unknown): boolean {
    if (value === false || value === 0 || value === '0') {
        return false;
    }
    return true;
}

export function normalizeExecutionBudget(item: BudgetExecutionSource): Budget {
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
}

export function summarizeBudgetStatuses(budgets: Budget[]): NonNullable<BudgetStatus['summary']> {
    return budgets.reduce<NonNullable<BudgetStatus['summary']>>((counts, budget) => {
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
}
