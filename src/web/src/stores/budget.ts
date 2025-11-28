/**
 * 预算管理 Store
 * @version 6.22.0
 * @description 管理预算列表、执行详情、周期预计等状态
 */

import { ref, computed } from 'vue';
import { defineStore } from 'pinia';

import { isEquals } from '@/lib/common.ts';
import services from '@/lib/services.ts';
import logger from '@/lib/logger.ts';

import {
    Budget,
    BudgetType,
    BudgetPeriodType,
    type BudgetExecutionResponse,
    type BudgetForecastResponse,
    type BudgetCategoryExecution,
    type BudgetInfoResponse
} from '@/models/budget.ts';

export const useBudgetStore = defineStore('budget', () => {
    // ============================================================================
    // 状态定义
    // ============================================================================

    /** 所有预算列表 */
    const allBudgets = ref<Budget[]>([]);

    /** 预算ID映射表 */
    const allBudgetsMap = ref<Record<string, Budget>>({});

    /** 预算列表状态是否无效（需要重新加载） */
    const budgetListStateInvalid = ref<boolean>(true);

    /** 当前执行详情 */
    const currentExecution = ref<BudgetExecutionResponse | null>(null);

    /** 当前周期预计 */
    const currentForecast = ref<BudgetForecastResponse | null>(null);

    /** 执行详情加载状态 */
    const executionLoading = ref<boolean>(false);

    /** 预计加载状态 */
    const forecastLoading = ref<boolean>(false);

    // ============================================================================
    // 计算属性
    // ============================================================================

    /** 支出类型预算 */
    const expenseBudgets = computed<Budget[]>(() => {
        return allBudgets.value.filter(b => b.type === BudgetType.Expense);
    });

    /** 投资类型预算 */
    const investmentBudgets = computed<Budget[]>(() => {
        return allBudgets.value.filter(b => b.type === BudgetType.Investment);
    });

    /** 启用的预算数量 */
    const enabledBudgetsCount = computed<number>(() => {
        return allBudgets.value.filter(b => b.enabled).length;
    });

    /** 月度预算 */
    const monthlyBudgets = computed<Budget[]>(() => {
        return allBudgets.value.filter(b => b.periodType === BudgetPeriodType.Monthly);
    });

    /** 季度预算 */
    const quarterlyBudgets = computed<Budget[]>(() => {
        return allBudgets.value.filter(b => b.periodType === BudgetPeriodType.Quarterly);
    });

    /** 年度预算 */
    const yearlyBudgets = computed<Budget[]>(() => {
        return allBudgets.value.filter(b => b.periodType === BudgetPeriodType.Yearly);
    });

    // ============================================================================
    // 内部方法
    // ============================================================================

    /**
     * 更新预算列表状态
     */
    function setBudgets(budgets: Budget[]): void {
        allBudgets.value = budgets;

        const budgetsMap: Record<string, Budget> = {};
        for (const budget of budgets) {
            budgetsMap[budget.id] = budget;
        }
        allBudgetsMap.value = budgetsMap;

        budgetListStateInvalid.value = false;
    }

    /**
     * 添加预算到列表
     */
    function addBudgetToList(budget: Budget): void {
        allBudgets.value.push(budget);
        allBudgetsMap.value[budget.id] = budget;
    }

    /**
     * 更新列表中的预算
     */
    function updateBudgetInList(budget: Budget): void {
        const index = allBudgets.value.findIndex(b => b.id === budget.id);
        if (index >= 0) {
            allBudgets.value[index] = budget;
        }
        allBudgetsMap.value[budget.id] = budget;
    }

    /**
     * 从列表中移除预算
     */
    function removeBudgetFromList(budgetId: string): void {
        const index = allBudgets.value.findIndex(b => b.id === budgetId);
        if (index >= 0) {
            allBudgets.value.splice(index, 1);
        }
        delete allBudgetsMap.value[budgetId];
    }

    /**
     * 应用执行数据到预算列表
     */
    function applyExecutionData(execution: BudgetExecutionResponse): void {
        if (!execution || !execution.categories) {
            return;
        }

        // 构建分类执行映射
        const executionMap: Record<string, BudgetCategoryExecution> = {};
        for (const cat of execution.categories) {
            executionMap[cat.categoryId] = cat;
        }

        // 更新预算的执行数据
        for (const budget of allBudgets.value) {
            const exec = executionMap[budget.categoryId];
            if (exec) {
                budget.updateExecution(exec);
            } else {
                // 重置执行数据
                budget.spentAmount = 0;
                budget.executionRate = 0;
                budget.remainingAmount = budget.amount;
                budget.isOverBudget = false;
                budget.alertTriggered = false;
            }
        }
    }

    // ============================================================================
    // 异步操作
    // ============================================================================

    /**
     * 加载所有预算
     */
    function loadAllBudgets({ force = false, type, periodType }: {
        force?: boolean,
        type?: BudgetType,
        periodType?: BudgetPeriodType
    } = {}): Promise<Budget[]> {
        return new Promise((resolve, reject) => {
            if (!force && !budgetListStateInvalid.value) {
                resolve(allBudgets.value);
                return;
            }

            const req: any = {};
            if (type !== undefined) {
                req.type = type;
            }
            if (periodType !== undefined) {
                req.periodType = periodType;
            }

            services.getAllBudgets(req).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to get budget list' });
                    return;
                }

                // 后端返回 { items: [...], totalBudget, ... }，需要使用 items 数组
                // 使用类型断言避免TypeScript错误
                const result = data.result as unknown as { items?: BudgetInfoResponse[], [key: string]: unknown };
                const budgetItems = (result.items || []) as BudgetInfoResponse[];
                const budgets = Budget.ofMulti(budgetItems);

                if (force && isEquals(allBudgets.value, budgets)) {
                    reject({ message: 'Budget list is up to date', isUpToDate: true });
                    return;
                }

                setBudgets(budgets);
                logger.info(`[BudgetStore] Loaded ${budgets.length} budgets`);
                resolve(budgets);
            }).catch(error => {
                logger.error('[BudgetStore] Failed to load budgets', error);
                reject(error);
            });
        });
    }

    /**
     * 加载预算执行详情
     */
    function loadBudgetExecution({ type, periodType, year, month, quarter }: {
        type?: BudgetType,
        periodType?: BudgetPeriodType,
        year?: number,
        month?: number,
        quarter?: number
    } = {}): Promise<BudgetExecutionResponse> {
        return new Promise((resolve, reject) => {
            executionLoading.value = true;

            const req: any = {};
            if (type !== undefined) req.type = type;
            if (periodType !== undefined) req.periodType = periodType;
            if (year !== undefined) req.year = year;
            if (month !== undefined) req.month = month;
            if (quarter !== undefined) req.quarter = quarter;

            services.getBudgetExecution(req).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    executionLoading.value = false;
                    reject({ message: 'Unable to get budget execution' });
                    return;
                }

                currentExecution.value = data.result;
                applyExecutionData(data.result);
                executionLoading.value = false;

                logger.info(`[BudgetStore] Loaded execution: ${data.result.categories?.length || 0} categories`);
                resolve(data.result);
            }).catch(error => {
                executionLoading.value = false;
                logger.error('[BudgetStore] Failed to load execution', error);
                reject(error);
            });
        });
    }

    /**
     * 加载周期预计
     */
    function loadBudgetForecast({ type, periodType, monthsHistory }: {
        type?: BudgetType,
        periodType?: BudgetPeriodType,
        monthsHistory?: number
    } = {}): Promise<BudgetForecastResponse> {
        return new Promise((resolve, reject) => {
            forecastLoading.value = true;

            const req: any = {};
            if (type !== undefined) req.type = type;
            if (periodType !== undefined) req.periodType = periodType;
            if (monthsHistory !== undefined) req.monthsHistory = monthsHistory;

            services.getBudgetForecast(req).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    forecastLoading.value = false;
                    reject({ message: 'Unable to get budget forecast' });
                    return;
                }

                currentForecast.value = data.result;
                forecastLoading.value = false;

                logger.info(`[BudgetStore] Loaded forecast: ${data.result.forecasts?.length || 0} items`);
                resolve(data.result);
            }).catch(error => {
                forecastLoading.value = false;
                logger.error('[BudgetStore] Failed to load forecast', error);
                reject(error);
            });
        });
    }

    /**
     * 保存预算（创建或更新）
     */
    function saveBudget({ budget }: { budget: Budget }): Promise<Budget> {
        return new Promise((resolve, reject) => {
            const isNew = !budget.id;

            const promise = isNew
                ? services.addBudget(budget.toCreateRequest())
                : services.modifyBudget(budget.toModifyRequest());

            promise.then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: isNew ? 'Unable to add budget' : 'Unable to update budget' });
                    return;
                }

                const savedBudget = Budget.of(data.result);

                if (isNew) {
                    addBudgetToList(savedBudget);
                } else {
                    updateBudgetInList(savedBudget);
                }

                logger.info(`[BudgetStore] ${isNew ? 'Added' : 'Updated'} budget: ${savedBudget.id}`);
                resolve(savedBudget);
            }).catch(error => {
                logger.error(`[BudgetStore] Failed to ${isNew ? 'add' : 'update'} budget`, error);
                reject(error);
            });
        });
    }

    /**
     * 删除预算
     */
    function deleteBudget({ budgetId, beforeResolve }: {
        budgetId: string,
        beforeResolve?: () => void
    }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.deleteBudget({ id: budgetId }).then(response => {
                const data = response.data;

                if (!data || !data.success) {
                    reject({ message: 'Unable to delete budget' });
                    return;
                }

                if (beforeResolve) {
                    beforeResolve();
                }

                removeBudgetFromList(budgetId);
                logger.info(`[BudgetStore] Deleted budget: ${budgetId}`);
                resolve(true);
            }).catch(error => {
                logger.error('[BudgetStore] Failed to delete budget', error);
                reject(error);
            });
        });
    }

    /**
     * 导出预算
     */
    function exportBudgets(): Promise<any> {
        return new Promise((resolve, reject) => {
            services.exportBudgets().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to export budgets' });
                    return;
                }

                logger.info(`[BudgetStore] Exported ${data.result.budgets?.length || 0} budgets`);
                resolve(data.result);
            }).catch(error => {
                logger.error('[BudgetStore] Failed to export budgets', error);
                reject(error);
            });
        });
    }

    /**
     * 导入预算
     */
    function importBudgets({ budgets, overwriteExisting = false }: {
        budgets: any[],
        overwriteExisting?: boolean
    }): Promise<any> {
        return new Promise((resolve, reject) => {
            services.importBudgets({ budgets, overwriteExisting }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to import budgets' });
                    return;
                }

                // 重新加载预算列表
                budgetListStateInvalid.value = true;

                logger.info(`[BudgetStore] Imported: ${data.result.importedCount} added, ${data.result.updatedCount} updated`);
                resolve(data.result);
            }).catch(error => {
                logger.error('[BudgetStore] Failed to import budgets', error);
                reject(error);
            });
        });
    }

    /**
     * 使预算列表状态无效
     */
    function invalidateBudgetList(): void {
        budgetListStateInvalid.value = true;
    }

    /**
     * 清空所有状态
     */
    function resetBudgetData(): void {
        allBudgets.value = [];
        allBudgetsMap.value = {};
        budgetListStateInvalid.value = true;
        currentExecution.value = null;
        currentForecast.value = null;
        executionLoading.value = false;
        forecastLoading.value = false;
    }

    // ============================================================================
    // 导出
    // ============================================================================

    return {
        // 状态
        allBudgets,
        allBudgetsMap,
        budgetListStateInvalid,
        currentExecution,
        currentForecast,
        executionLoading,
        forecastLoading,

        // 计算属性
        expenseBudgets,
        investmentBudgets,
        enabledBudgetsCount,
        monthlyBudgets,
        quarterlyBudgets,
        yearlyBudgets,

        // 方法
        loadAllBudgets,
        loadBudgetExecution,
        loadBudgetForecast,
        saveBudget,
        deleteBudget,
        exportBudgets,
        importBudgets,
        invalidateBudgetList,
        resetBudgetData
    };
});
