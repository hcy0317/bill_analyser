/**
 * 预算管理 - 数据模型和请求/响应类型定义
 * @version 6.22.0
 * @description 支持预算CRUD、执行度统计、周期预计、导入导出功能
 */

import {
    DEFAULT_BUDGET_ALERT_THRESHOLD,
    DEFAULT_BUDGET_ENABLED
} from '@/config/budget.ts';

// ============================================================================
// 枚举定义
// ============================================================================

/**
 * 预算周期类型
 */
export enum BudgetPeriodType {
    Daily = 'daily',         // 日度预算
    Weekly = 'weekly',       // 周度预算
    Monthly = 'monthly',     // 月度预算
    Quarterly = 'quarterly', // 季度预算
    Yearly = 'yearly'        // 年度预算
}

/**
 * 预算类型（与交易类型对应）
 */
export enum BudgetType {
    Expense = 3,    // 支出预算
    Investment = 5  // 投资预算
}

// ============================================================================
// 基础数据结构
// ============================================================================

/**
 * 预算分类执行详情
 */
export interface BudgetCategoryExecution {
    readonly budgetId: string;             // 预算ID
    readonly categoryId: string;           // 分类ID
    readonly categoryName: string;         // 分类名称（主分类-子分类格式）
    readonly budgetAmount: number;         // 预算金额（分）
    readonly spentAmount: number;          // 已花费金额（分）
    readonly executionRate: number;        // 执行率（百分比，0-100或更高）
    readonly remainingAmount: number;      // 剩余金额（分）
    readonly isOverBudget: boolean;        // 是否超预算
    readonly alertTriggered: boolean;      // 是否触发预警
}

/**
 * 预算执行详情响应
 */
export interface BudgetExecutionResponse {
    readonly totalBudget: number;          // 总预算金额（分）
    readonly totalSpent: number;           // 总已花费金额（分）
    readonly totalExecutionRate: number;   // 总执行率（百分比）
    readonly categories: BudgetCategoryExecution[];  // 分类执行详情列表
    readonly periodStart: string;          // 统计周期开始（ISO日期）
    readonly periodEnd: string;            // 统计周期结束（ISO日期）
}

/**
 * 周期预计响应项
 */
export interface BudgetForecastItem {
    readonly categoryId: string;           // 分类ID
    readonly categoryName: string;         // 分类名称
    readonly historicalAverage: number;    // 历史平均花费（分）
    readonly currentSpent: number;         // 当前已花费（分）
    readonly projectedTotal: number;       // 预计总花费（分）
    readonly budgetAmount: number;         // 预算金额（分）
    readonly projectedOverBudget: boolean; // 预计是否超预算
    readonly trend: 'up' | 'down' | 'stable'; // 趋势：上升/下降/稳定
    readonly samplePeriods?: number;       // 样本周期数
    readonly strategyExplanation?: string; // 策略说明
    readonly backtestMape?: number | null; // 回测MAPE
    readonly confidence?: 'high' | 'medium' | 'low'; // 置信等级
    readonly periods?: Array<{            // 历史周期明细（用于图表）
        readonly period: string;          // 周期标识（如 YYYY-MM）
        readonly amount: number;          // 该周期金额（分）
    }>;
}

/**
 * 周期预计响应
 */
export interface BudgetForecastResponse {
    readonly forecasts: BudgetForecastItem[];   // 预计列表
    readonly periodStart: string;               // 预计周期开始
    readonly periodEnd: string;                 // 预计周期结束
    readonly daysRemaining: number;             // 剩余天数
    readonly daysElapsed: number;               // 已过天数
    readonly forecastStrategy?: string;         // 预测策略
    readonly historyPeriods?: number;           // 历史周期数
    readonly avgBacktestMape?: number | null;   // 平均回测MAPE
}

export enum BudgetForecastStrategy {
    HistoricalAverage = 'historical_average',
    MovingAverage = 'moving_average'
}

// ============================================================================
// 预算信息响应
// ============================================================================

/**
 * 预算信息响应（对应后端数据库记录）
 */
export interface BudgetInfoResponse {
    readonly id: string;                   // 预算ID
    readonly name: string;                 // 预算名称
    readonly category: string;             // 主分类
    readonly subCategory: string;          // 子分类
    readonly categoryId?: string;          // 关联的分类ID
    readonly periodType: BudgetPeriodType; // 周期类型
    readonly amount: number;               // 预算金额（分）
    readonly startDate: string;            // 开始日期（ISO格式）
    readonly endDate: string;              // 结束日期（ISO格式）
    readonly alertThreshold: number;       // 预警阈值（百分比，如80表示80%）
    readonly enabled: boolean;             // 是否启用
    readonly type: BudgetType;             // 预算类型（支出/投资）
    readonly createdAt?: string;           // 创建时间
    readonly updatedAt?: string;           // 更新时间
    // 运行时字段（从预算执行API返回）
    readonly spentAmount?: number;         // 已花费金额（分）
    readonly remainingAmount?: number;     // 剩余金额（分）
    readonly executionRate?: number;       // 执行率（百分比）
    readonly categoryName?: string;        // 分类名称（用于显示）
    readonly categoryIcon?: string;        // 分类图标ID
    readonly categoryColor?: string;       // 分类颜色
}

/**
 * 预算列表响应
 */
export interface BudgetListResponse {
    readonly items: BudgetInfoResponse[];  // 预算列表
    readonly total: number;                // 总数量
}

// ============================================================================
// 请求类型定义
// ============================================================================

/**
 * 获取预算列表请求
 */
export interface BudgetListRequest {
    readonly type?: BudgetType;            // 预算类型筛选
    readonly periodType?: BudgetPeriodType;// 周期类型筛选
    readonly enabled?: boolean;            // 启用状态筛选
    readonly category?: string;            // 分类筛选
    readonly keyword?: string;             // 关键词搜索
}

/**
 * 获取预算执行详情请求
 */
export interface BudgetExecutionRequest {
    readonly type?: BudgetType;            // 预算类型
    readonly periodType?: BudgetPeriodType;// 周期类型
    readonly year?: number;                // 年份
    readonly month?: number;               // 月份（1-12）
    readonly quarter?: number;             // 季度（1-4）
    readonly startDate?: string;           // 自定义开始日期
    readonly endDate?: string;             // 自定义结束日期
}

/**
 * 预算历史查询请求
 */
export interface BudgetHistoryRequest {
    readonly type?: BudgetType;             // 预算类型
    readonly periodType?: BudgetPeriodType; // 周期类型
    readonly year?: number;                 // 年份
    readonly month?: number;                // 月份（1-12）
    readonly quarter?: number;              // 季度（1-4）
    readonly startDate?: string;            // 查询开始日期
    readonly endDate?: string;              // 查询结束日期
    readonly budgetId?: string;             // 预算ID
    readonly categoryId?: string;           // 分类ID
    readonly accountIds?: string[];         // 账户ID列表
    readonly tagIds?: string[];             // 标签ID列表
}

/**
 * 预算历史快照项
 */
export interface BudgetHistoryItem {
    readonly id: string;                    // 快照ID
    readonly budgetId: string;              // 预算ID
    readonly name: string;                  // 预算名称
    readonly category: string;              // 主分类
    readonly subCategory: string;           // 子分类
    readonly periodType: BudgetPeriodType;  // 周期类型
    readonly periodStart: string;           // 周期开始
    readonly periodEnd: string;             // 周期结束
    readonly budgetAmount: number;          // 预算金额（分）
    readonly spentAmount: number;           // 已花费金额（分）
    readonly remainingAmount: number;       // 剩余金额（分）
    readonly executionRate: number;         // 执行率
    readonly status: string;                // 状态
    readonly filterSummary: string;         // 筛选摘要
    readonly calculatedAt: string;          // 计算时间
    readonly alertThreshold: number;        // 预警阈值
    readonly enabled: boolean;              // 是否启用
}

/**
 * 预算历史响应
 */
export interface BudgetHistoryResponse {
    readonly items: BudgetHistoryItem[];    // 历史快照列表
    readonly count: number;                 // 数量
    readonly periodStart: string;           // 查询范围开始
    readonly periodEnd: string;             // 查询范围结束
}

/**
 * 获取周期预计请求
 */
export interface BudgetForecastRequest {
    readonly type?: BudgetType;            // 预算类型
    readonly periodType?: BudgetPeriodType;// 周期类型
    readonly year?: number;                // 年份
    readonly month?: number;               // 月份（1-12）
    readonly quarter?: number;             // 季度（1-4）
    readonly monthsHistory?: number;       // 历史数据月数（默认6）
    readonly forecastStrategy?: BudgetForecastStrategy; // 预测策略
    readonly startDate?: string;           // 自定义开始日期
    readonly endDate?: string;             // 自定义结束日期
}

/**
 * 创建预算请求
 */
export interface BudgetCreateRequest {
    readonly name: string;                 // 预算名称
    readonly category: string;             // 主分类
    readonly subCategory?: string;         // 子分类（可选）
    readonly categoryId?: string;          // 关联分类ID（可选）
    readonly periodType: BudgetPeriodType; // 周期类型
    readonly amount: number;               // 预算金额（分）
    readonly startDate?: string;           // 开始日期（可选，默认当前周期）
    readonly endDate?: string;             // 结束日期（可选）
    readonly alertThreshold?: number;      // 预警阈值（默认80）
    readonly enabled?: boolean;            // 是否启用（默认true）
    readonly type: BudgetType;             // 预算类型
}

/**
 * 修改预算请求
 */
export interface BudgetModifyRequest {
    readonly id: string;                   // 预算ID
    readonly name?: string;                // 预算名称
    readonly category?: string;            // 主分类
    readonly subCategory?: string;         // 子分类
    readonly categoryId?: string;          // 关联分类ID
    readonly periodType?: BudgetPeriodType;// 周期类型
    readonly amount?: number;              // 预算金额（分）
    readonly startDate?: string;           // 开始日期
    readonly endDate?: string;             // 结束日期
    readonly alertThreshold?: number;      // 预警阈值
    readonly enabled?: boolean;            // 是否启用
    readonly type?: BudgetType;            // 预算类型
}

/**
 * 删除预算请求
 */
export interface BudgetDeleteRequest {
    readonly id: string;                   // 预算ID
}

/**
 * 导入预算请求
 */
export interface BudgetImportRequest {
    readonly budgets: BudgetCreateRequest[];  // 预算列表
    readonly overwriteExisting?: boolean;     // 是否覆盖现有预算
}

/**
 * 导入预算响应
 */
export interface BudgetImportResponse {
    readonly importedCount: number;        // 成功导入数量
    readonly updatedCount: number;         // 更新数量
    readonly failedCount: number;          // 失败数量
    readonly errors: string[];             // 错误信息列表
}

/**
 * 导出预算响应
 */
export interface BudgetExportResponse {
    readonly budgets: BudgetInfoResponse[]; // 预算列表
    readonly exportedAt: string;            // 导出时间
}

// ============================================================================
// 前端模型类
// ============================================================================

/**
 * 预算模型类（前端使用）
 */
export class Budget {
    public id: string = '';
    public name: string = '';
    public category: string = '';
    public subCategory: string = '';
    public categoryId: string = '';
    public periodType: BudgetPeriodType = BudgetPeriodType.Monthly;
    public amount: number = 0;  // 以分为单位
    public startDate: string = '';
    public endDate: string = '';
    public alertThreshold: number = DEFAULT_BUDGET_ALERT_THRESHOLD;
    public enabled: boolean = DEFAULT_BUDGET_ENABLED;
    public type: BudgetType = BudgetType.Expense;
    public createdAt: string = '';
    public updatedAt: string = '';

    // 分类显示字段
    public categoryIcon: string = '';
    public categoryColor: string = '';

    // 运行时计算字段（从执行详情获取）
    public spentAmount: number = 0;
    public executionRate: number = 0;
    public remainingAmount: number = 0;
    public isOverBudget: boolean = false;
    public alertTriggered: boolean = false;

    /**
     * 从API响应创建Budget实例
     */
    public static of(response: BudgetInfoResponse): Budget {
        const budget = new Budget();
        budget.id = response.id || '';
        budget.name = response.name || '';
        budget.category = response.category || '';
        budget.subCategory = response.subCategory || '';
        budget.categoryId = response.categoryId || '';
        budget.periodType = response.periodType || BudgetPeriodType.Monthly;
        budget.amount = response.amount || 0;
        budget.startDate = response.startDate || '';
        budget.endDate = response.endDate || '';
        budget.alertThreshold = response.alertThreshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD;
        budget.enabled = response.enabled ?? DEFAULT_BUDGET_ENABLED;
        budget.type = response.type || BudgetType.Expense;
        budget.createdAt = response.createdAt || '';
        budget.updatedAt = response.updatedAt || '';
        // 分类显示字段
        budget.categoryIcon = response.categoryIcon || '';
        budget.categoryColor = response.categoryColor || '';
        // 运行时字段（从预算执行API返回）
        budget.spentAmount = response.spentAmount || 0;
        budget.remainingAmount = response.remainingAmount || 0;
        budget.executionRate = response.executionRate || 0;
        // 计算是否超预算和是否触发预警
        budget.isOverBudget = budget.spentAmount > budget.amount;
        budget.alertTriggered = budget.executionRate >= budget.alertThreshold;
        return budget;
    }

    /**
     * 从多个API响应创建Budget实例数组
     */
    public static ofMulti(responses: BudgetInfoResponse[]): Budget[] {
        if (!responses || !Array.isArray(responses)) {
            return [];
        }
        return responses.map(r => Budget.of(r));
    }

    /**
     * 创建新预算实例
     */
    public static createNew(type: BudgetType = BudgetType.Expense): Budget {
        const budget = new Budget();
        budget.type = type;
        budget.periodType = BudgetPeriodType.Monthly;
        budget.alertThreshold = DEFAULT_BUDGET_ALERT_THRESHOLD;
        budget.enabled = DEFAULT_BUDGET_ENABLED;
        return budget;
    }

    /**
     * 获取完整分类名称
     */
    public get fullCategoryName(): string {
        if (this.subCategory) {
            return `${this.category}-${this.subCategory}`;
        }
        return this.category;
    }

    /**
     * 获取执行率显示文本
     */
    public get executionRateText(): string {
        return `${this.executionRate.toFixed(1)}%`;
    }

    /**
     * 获取金额显示文本（元）
     */
    public get amountInYuan(): number {
        return this.amount / 100;
    }

    /**
     * 获取已花费金额显示文本（元）
     */
    public get spentAmountInYuan(): number {
        return this.spentAmount / 100;
    }

    /**
     * 获取剩余金额显示文本（元）
     */
    public get remainingAmountInYuan(): number {
        return this.remainingAmount / 100;
    }

    /**
     * 转换为创建请求
     */
    public toCreateRequest(): BudgetCreateRequest {
        return {
            name: this.name,
            category: this.category,
            subCategory: this.subCategory || undefined,
            categoryId: this.categoryId || undefined,
            periodType: this.periodType,
            amount: this.amount,
            startDate: this.startDate || undefined,
            endDate: this.endDate || undefined,
            alertThreshold: this.alertThreshold,
            enabled: this.enabled,
            type: this.type
        };
    }

    /**
     * 转换为修改请求
     */
    public toModifyRequest(): BudgetModifyRequest {
        return {
            id: this.id,
            name: this.name,
            category: this.category,
            subCategory: this.subCategory || undefined,
            categoryId: this.categoryId || undefined,
            periodType: this.periodType,
            amount: this.amount,
            startDate: this.startDate || undefined,
            endDate: this.endDate || undefined,
            alertThreshold: this.alertThreshold,
            enabled: this.enabled,
            type: this.type
        };
    }

    /**
     * 更新执行数据
     */
    public updateExecution(execution: BudgetCategoryExecution): void {
        this.spentAmount = execution.spentAmount;
        this.executionRate = execution.executionRate;
        this.remainingAmount = execution.remainingAmount;
        this.isOverBudget = execution.isOverBudget;
        this.alertTriggered = execution.alertTriggered;
    }
}

/**
 * 预算周期类型显示名称
 */
export const BudgetPeriodTypeNames: Record<BudgetPeriodType, string> = {
    [BudgetPeriodType.Daily]: '日度',
    [BudgetPeriodType.Weekly]: '周度',
    [BudgetPeriodType.Monthly]: '月度',
    [BudgetPeriodType.Quarterly]: '季度',
    [BudgetPeriodType.Yearly]: '年度'
};

/**
 * 预算类型显示名称
 */
export const BudgetTypeNames: Record<BudgetType, string> = {
    [BudgetType.Expense]: '支出',
    [BudgetType.Investment]: '投资'
};
