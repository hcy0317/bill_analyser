import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import {
    Budget,
    BudgetForecastStrategy,
    BudgetPeriodType,
    BudgetType,
    type BudgetInfoResponse
} from '@/models/budget.ts';

type ApiResponse<T> = Promise<{ data: { success: boolean; result?: T } }>;
type ServiceMock = jest.Mock<(...args: any[]) => ApiResponse<any>>;

const serviceMocks = {
    getAllBudgets: jest.fn(),
    getBudgetExecution: jest.fn(),
    createBudgetHistorySnapshot: jest.fn(),
    getBudgetHistory: jest.fn(),
    getBudgetForecast: jest.fn(),
    addBudget: jest.fn(),
    modifyBudget: jest.fn(),
    deleteBudget: jest.fn(),
    exportBudgets: jest.fn(),
    importBudgets: jest.fn()
} satisfies Record<string, ServiceMock>;

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: serviceMocks
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: jest.fn(),
        error: jest.fn()
    }
}));

import { useBudgetStore } from '@/stores/budget.ts';

function ok<T>(result: T): ApiResponse<T> {
    return Promise.resolve({ data: { success: true, result } });
}

function invalid(): ApiResponse<never> {
    return Promise.resolve({ data: { success: false } });
}

function budgetInfo(
    id: string,
    type: BudgetType = BudgetType.Expense,
    periodType: BudgetPeriodType = BudgetPeriodType.Monthly,
    categoryId = `category-${id}`,
    enabled = true
): BudgetInfoResponse {
    return {
        id,
        name: `budget-${id}`,
        category: '生活',
        subCategory: '餐饮',
        categoryId,
        periodType,
        amountCents: 10_000,
        startDate: '2026-01-01',
        endDate: '2026-12-31',
        alertThreshold: 80,
        enabled,
        type
    };
}

function deferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (reason: unknown) => void;
} {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, resolve, reject };
}

describe('budget store behavior coverage', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        for (const mock of Object.values(serviceMocks)) {
            mock.mockReset();
        }
    });

    test('loads, filters, caches and force-refreshes budget lists', async () => {
        const rows = [
            budgetInfo('expense-month'),
            budgetInfo('investment-quarter', BudgetType.Investment, BudgetPeriodType.Quarterly, 'investment', false),
            budgetInfo('expense-year', BudgetType.Expense, BudgetPeriodType.Yearly)
        ];
        serviceMocks.getAllBudgets.mockReturnValue(ok({ items: rows }));
        const store = useBudgetStore();

        await expect(store.loadAllBudgets()).resolves.toHaveLength(3);
        await expect(store.loadAllBudgets()).resolves.toBe(store.allBudgets);
        expect(serviceMocks.getAllBudgets).toHaveBeenCalledTimes(1);
        expect(store.expenseBudgets.map(item => item.id)).toEqual(['expense-month', 'expense-year']);
        expect(store.investmentBudgets.map(item => item.id)).toEqual(['investment-quarter']);
        expect(store.enabledBudgetsCount).toBe(2);
        expect(store.monthlyBudgets).toHaveLength(1);
        expect(store.quarterlyBudgets).toHaveLength(1);
        expect(store.yearlyBudgets).toHaveLength(1);
        expect(store.allBudgetsMap['expense-month']).toBe(store.allBudgets[0]);

        serviceMocks.getAllBudgets.mockReturnValueOnce(ok({ items: [rows[0]] }));
        await store.loadAllBudgets({ type: BudgetType.Expense, periodType: BudgetPeriodType.Monthly });
        expect(serviceMocks.getAllBudgets).toHaveBeenLastCalledWith({
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly
        });

        serviceMocks.getAllBudgets.mockReturnValueOnce(ok({ items: [rows[0]] }));
        const retained = store.allBudgets;
        await expect(store.loadAllBudgets({
            force: true,
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly
        })).resolves.toBe(retained);

        store.invalidateBudgetList();
        expect(store.budgetListStateInvalid).toBe(true);
        serviceMocks.getAllBudgets.mockReturnValueOnce(ok({}));
        await expect(store.loadAllBudgets()).resolves.toEqual([]);
    });

    test('rejects malformed and failed budget list requests', async () => {
        const store = useBudgetStore();
        serviceMocks.getAllBudgets.mockReturnValueOnce(invalid());
        await expect(store.loadAllBudgets()).rejects.toEqual({ message: 'Unable to get budget list' });

        const failure = new Error('offline');
        serviceMocks.getAllBudgets.mockRejectedValueOnce(failure);
        await expect(store.loadAllBudgets()).rejects.toBe(failure);
    });

    test('applies execution by budget and category and resets unmatched budgets', async () => {
        serviceMocks.getAllBudgets.mockReturnValue(ok({
            items: [
                budgetInfo('direct', BudgetType.Expense, BudgetPeriodType.Monthly, 'direct-category'),
                budgetInfo('category', BudgetType.Expense, BudgetPeriodType.Monthly, 'shared'),
                budgetInfo('unmatched', BudgetType.Expense, BudgetPeriodType.Monthly, 'none')
            ]
        }));
        const store = useBudgetStore();
        await store.loadAllBudgets();
        store.allBudgetsMap['unmatched']!.spentAmountCents = 500;
        serviceMocks.getBudgetExecution.mockReturnValue(ok({
            totalBudgetCents: 30_000,
            totalSpentCents: 12_000,
            totalExecutionRate: 40,
            periodStart: '2026-01-01',
            periodEnd: '2026-01-31',
            categories: [
                {
                    budgetId: 'direct', categoryId: 'direct-category', categoryName: 'direct',
                    budgetAmountCents: 10_000, spentAmountCents: 8_000,
                    executionRate: 80, remainingAmountCents: 2_000,
                    isOverBudget: false, alertTriggered: true
                },
                {
                    budgetId: '', categoryId: 'shared', categoryName: 'category',
                    budgetAmountCents: 10_000, spentAmountCents: 4_000,
                    executionRate: 40, remainingAmountCents: 6_000,
                    isOverBudget: false, alertTriggered: false
                }
            ]
        }));

        const pending = store.loadBudgetExecution({
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly,
            year: 2026,
            month: 1,
            quarter: 1,
            startDate: '2026-01-01',
            endDate: '2026-01-31'
        });
        expect(store.executionLoading).toBe(true);
        const execution = await pending;
        expect(store.currentExecution).toStrictEqual(execution);
        expect(serviceMocks.getBudgetExecution).toHaveBeenCalledWith({
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly,
            year: 2026,
            month: 1,
            quarter: 1,
            startDate: '2026-01-01',
            endDate: '2026-01-31'
        });
        expect(store.allBudgetsMap['direct']!.spentAmountCents).toBe(8_000);
        expect(store.allBudgetsMap['category']!.spentAmountCents).toBe(4_000);
        expect(store.allBudgetsMap['unmatched']).toMatchObject({
            spentAmountCents: 0,
            remainingAmountCents: 10_000,
            executionRate: 0,
            isOverBudget: false,
            alertTriggered: false
        });
        expect(store.executionLoading).toBe(false);

        serviceMocks.getBudgetExecution.mockReturnValueOnce(ok({ categories: undefined }));
        await store.loadBudgetExecution();
        expect(store.allBudgetsMap['direct']!.spentAmountCents).toBe(8_000);
    });

    test('clears loading flags on invalid and rejected execution and forecast responses', async () => {
        const store = useBudgetStore();
        serviceMocks.getBudgetExecution.mockReturnValueOnce(invalid());
        await expect(store.loadBudgetExecution()).rejects.toEqual({ message: 'Unable to get budget execution' });
        expect(store.executionLoading).toBe(false);
        serviceMocks.getBudgetExecution.mockRejectedValueOnce(new Error('execution failed'));
        await expect(store.loadBudgetExecution()).rejects.toThrow('execution failed');

        serviceMocks.getBudgetForecast.mockReturnValueOnce(invalid());
        await expect(store.loadBudgetForecast()).rejects.toEqual({ message: 'Unable to get budget forecast' });
        expect(store.forecastLoading).toBe(false);
        serviceMocks.getBudgetForecast.mockRejectedValueOnce(new Error('forecast failed'));
        await expect(store.loadBudgetForecast()).rejects.toThrow('forecast failed');
        expect(store.forecastLoading).toBe(false);
    });

    test('loads forecast with every supported filter', async () => {
        const forecast = {
            forecasts: [], periodStart: '2026-01-01', periodEnd: '2026-01-31',
            daysRemaining: 5, daysElapsed: 26
        };
        serviceMocks.getBudgetForecast.mockReturnValue(ok(forecast));
        const store = useBudgetStore();
        const pending = store.loadBudgetForecast({
            type: BudgetType.Investment,
            periodType: BudgetPeriodType.Quarterly,
            year: 2026,
            month: 3,
            quarter: 1,
            monthsHistory: 12,
            forecastStrategy: BudgetForecastStrategy.MovingAverage,
            startDate: '2026-01-01',
            endDate: '2026-03-31'
        });
        expect(store.forecastLoading).toBe(true);
        await expect(pending).resolves.toBe(forecast);
        expect(serviceMocks.getBudgetForecast).toHaveBeenCalledWith({
            type: BudgetType.Investment,
            periodType: BudgetPeriodType.Quarterly,
            year: 2026,
            month: 3,
            quarter: 1,
            monthsHistory: 12,
            forecastStrategy: BudgetForecastStrategy.MovingAverage,
            startDate: '2026-01-01',
            endDate: '2026-03-31'
        });
        expect(store.currentForecast).toStrictEqual(forecast);
    });

    test('keeps only the newest history response authoritative and supports reset invalidation', async () => {
        const first = deferred<{ data: { success: boolean; result: any } }>();
        const second = deferred<{ data: { success: boolean; result: any } }>();
        serviceMocks.getBudgetHistory
            .mockReturnValueOnce(first.promise)
            .mockReturnValueOnce(second.promise);
        const store = useBudgetStore();
        const firstRequest = store.loadBudgetHistory({ year: 2025, accountIds: ['a'], tagIds: ['t'] });
        const secondRequest = store.loadBudgetHistory({
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly,
            year: 2026,
            month: 2,
            quarter: 1,
            startDate: '2026-02-01',
            endDate: '2026-02-28',
            categoryId: 'food',
            accountIds: ['wallet'],
            tagIds: ['daily']
        });
        const newest = { items: [], count: 0, periodStart: '2026-02-01', periodEnd: '2026-02-28' };
        second.resolve({ data: { success: true, result: newest } });
        await expect(secondRequest).resolves.toBe(newest);
        const signature = JSON.parse(store.currentHistoryRequestSignature);
        expect(signature).toMatchObject({ year: 2026, month: 2, categoryId: 'food', accountIds: ['wallet'] });
        first.resolve({ data: { success: true, result: { ...newest, periodStart: 'stale' } } });
        await firstRequest;
        expect(store.currentHistory).toStrictEqual(newest);

        const afterReset = deferred<{ data: { success: boolean; result: any } }>();
        serviceMocks.getBudgetHistory.mockReturnValueOnce(afterReset.promise);
        const resetRequest = store.loadBudgetHistory();
        store.resetBudgetData();
        afterReset.resolve({ data: { success: true, result: { ...newest, periodStart: 'reset-stale' } } });
        await resetRequest;
        expect(store.currentHistory).toBeNull();
        expect(store.historyLoading).toBe(false);
    });

    test('handles history snapshot/history invalid data and transport failures', async () => {
        const store = useBudgetStore();
        serviceMocks.createBudgetHistorySnapshot.mockReturnValueOnce(ok({ id: 'snapshot' }));
        await expect(store.createBudgetHistorySnapshot({ budgetId: 'b' })).resolves.toEqual({ id: 'snapshot' });
        serviceMocks.createBudgetHistorySnapshot.mockReturnValueOnce(invalid());
        await expect(store.createBudgetHistorySnapshot()).rejects.toEqual({ message: 'Unable to create budget history snapshot' });
        serviceMocks.createBudgetHistorySnapshot.mockRejectedValueOnce(new Error('snapshot failed'));
        await expect(store.createBudgetHistorySnapshot()).rejects.toThrow('snapshot failed');

        serviceMocks.getBudgetHistory.mockReturnValueOnce(invalid());
        await expect(store.loadBudgetHistory()).rejects.toEqual({ message: 'Unable to get budget history' });
        expect(store.historyLoading).toBe(false);
        serviceMocks.getBudgetHistory.mockRejectedValueOnce(new Error('history failed'));
        await expect(store.loadBudgetHistory()).rejects.toThrow('history failed');
        expect(store.historyLoading).toBe(false);
    });

    test('creates and updates budgets while preserving list and map behavior', async () => {
        const store = useBudgetStore();
        serviceMocks.getAllBudgets.mockReturnValueOnce(ok({ items: [budgetInfo('existing')] }));
        await store.loadAllBudgets();

        const newBudget = Budget.createNew(BudgetType.Investment);
        newBudget.name = 'new';
        serviceMocks.addBudget.mockReturnValueOnce(ok(budgetInfo('created', BudgetType.Investment)));
        await expect(store.saveBudget({ budget: newBudget })).resolves.toMatchObject({ id: 'created' });
        expect(store.allBudgetsMap['created']).toBeDefined();

        const edited = store.allBudgetsMap['existing']!;
        edited.name = 'edited';
        serviceMocks.modifyBudget.mockReturnValueOnce(ok({ ...budgetInfo('existing'), name: 'saved' }));
        await store.saveBudget({ budget: edited });
        expect(store.allBudgets[0]!.name).toBe('saved');

        const absent = Budget.of(budgetInfo('absent'));
        serviceMocks.modifyBudget.mockReturnValueOnce(ok({ ...budgetInfo('absent'), name: 'map-only' }));
        await store.saveBudget({ budget: absent });
        expect(store.allBudgetsMap['absent']!.name).toBe('map-only');
        expect(store.allBudgets.some(item => item.id === 'absent')).toBe(false);
    });

    test.each([
        ['new invalid', Budget.createNew(), 'addBudget', invalid(), 'Unable to add budget'],
        ['edit invalid', Budget.of(budgetInfo('edit')), 'modifyBudget', invalid(), 'Unable to update budget'],
        ['new transport', Budget.createNew(), 'addBudget', Promise.reject(new Error('add failed')), 'add failed'],
        ['edit transport', Budget.of(budgetInfo('edit')), 'modifyBudget', Promise.reject(new Error('edit failed')), 'edit failed']
    ])('rejects %s save responses', async (_name, budget, method, response, message) => {
        const store = useBudgetStore();
        serviceMocks[method as keyof typeof serviceMocks].mockReturnValueOnce(response as ApiResponse<any>);
        await expect(store.saveBudget({ budget: budget as Budget })).rejects.toMatchObject({ message });
    });

    test('deletes with and without callback and covers missing list entries', async () => {
        const store = useBudgetStore();
        serviceMocks.getAllBudgets.mockReturnValueOnce(ok({ items: [budgetInfo('delete')] }));
        await store.loadAllBudgets();
        serviceMocks.deleteBudget.mockReturnValue(ok(true));

        const beforeResolve = jest.fn();
        await expect(store.deleteBudget({ budgetId: 'delete', beforeResolve })).resolves.toBe(true);
        expect(beforeResolve).toHaveBeenCalledTimes(1);
        expect(store.allBudgetsMap['delete']).toBeUndefined();
        await expect(store.deleteBudget({ budgetId: 'missing' })).resolves.toBe(true);

        serviceMocks.deleteBudget.mockReturnValueOnce(invalid());
        await expect(store.deleteBudget({ budgetId: 'bad' })).rejects.toEqual({ message: 'Unable to delete budget' });
        serviceMocks.deleteBudget.mockRejectedValueOnce(new Error('delete failed'));
        await expect(store.deleteBudget({ budgetId: 'bad' })).rejects.toThrow('delete failed');
    });

    test('exports/imports budgets and resets all state', async () => {
        const store = useBudgetStore();
        serviceMocks.exportBudgets.mockReturnValueOnce(ok({ budgets: [budgetInfo('export')], exportedAt: 'now' }));
        await expect(store.exportBudgets()).resolves.toMatchObject({ budgets: [expect.objectContaining({ id: 'export' })] });
        serviceMocks.exportBudgets.mockReturnValueOnce(ok({ exportedAt: 'now' }));
        await expect(store.exportBudgets()).resolves.toEqual({ exportedAt: 'now' });
        serviceMocks.exportBudgets.mockReturnValueOnce(invalid());
        await expect(store.exportBudgets()).rejects.toEqual({ message: 'Unable to export budgets' });
        serviceMocks.exportBudgets.mockRejectedValueOnce(new Error('export failed'));
        await expect(store.exportBudgets()).rejects.toThrow('export failed');

        const imported = { importedCount: 1, updatedCount: 2, failedCount: 0, errors: [] };
        serviceMocks.importBudgets.mockReturnValue(ok(imported));
        await expect(store.importBudgets({ budgets: [{ name: 'a' }] })).resolves.toBe(imported);
        expect(serviceMocks.importBudgets).toHaveBeenLastCalledWith({ budgets: [{ name: 'a' }], overwriteExisting: false });
        await store.importBudgets({ budgets: [], overwriteExisting: true });
        expect(store.budgetListStateInvalid).toBe(true);
        serviceMocks.importBudgets.mockReturnValueOnce(invalid());
        await expect(store.importBudgets({ budgets: [] })).rejects.toEqual({ message: 'Unable to import budgets' });
        serviceMocks.importBudgets.mockRejectedValueOnce(new Error('import failed'));
        await expect(store.importBudgets({ budgets: [] })).rejects.toThrow('import failed');

        store.resetBudgetData();
        expect(store.allBudgets).toEqual([]);
        expect(store.allBudgetsMap).toEqual({});
        expect(store.currentExecution).toBeNull();
        expect(store.currentForecast).toBeNull();
        expect(store.currentHistory).toBeNull();
        expect(store.currentHistoryRequestSignature).toBe('');
        expect(store.executionLoading).toBe(false);
        expect(store.forecastLoading).toBe(false);
        expect(store.historyLoading).toBe(false);
    });
});
