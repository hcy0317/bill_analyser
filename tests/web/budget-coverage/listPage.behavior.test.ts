import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockMountedCallbacks: Array<() => void> = [];
const mockRouterPush = jest.fn();
const mockGetDateRangeByDateType = jest.fn((dateType: number) => dateType === -1
    ? null
    : { dateType, minTime: 1_704_067_200, maxTime: 1_735_603_199 });
const mockGetShiftedDateRangeAndDateType = jest.fn((min: number, max: number, scale: number) => ({
    dateType: 255,
    minTime: min + scale * 86_400,
    maxTime: max + scale * 86_400
}));
const mockLogger = {
    debug: jest.fn(),
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn()
};

const mockBudgetStore: any = {
    allBudgets: [],
    currentExecution: null,
    currentForecast: null,
    currentHistory: null,
    currentHistoryRequestSignature: '',
    forecastLoading: false,
    loadAllBudgets: jest.fn(async () => undefined),
    loadBudgetExecution: jest.fn(async () => undefined),
    loadBudgetHistory: jest.fn(async () => undefined),
    createBudgetHistorySnapshot: jest.fn(async () => undefined),
    loadBudgetForecast: jest.fn(async () => undefined),
    deleteBudget: jest.fn(async () => undefined),
    exportBudgets: jest.fn(async () => ({ budgets: [] })),
    importBudgets: jest.fn(async () => ({ importedCount: 1, updatedCount: 2 }))
};

const mockPrimaryCategory: any = {
    id: 'food',
    parentId: '',
    name: 'Food',
    type: 3,
    icon: 'food-icon',
    color: '112233',
    displayOrder: 1,
    subCategories: [
        {
            id: 'cafe',
            parentId: 'food',
            name: 'Cafe',
            type: 3,
            icon: 'cafe-icon',
            color: '445566',
            displayOrder: 2,
            subCategories: []
        }
    ]
};

const mockInvestmentCategory: any = {
    id: 'fund',
    parentId: '',
    name: 'Fund',
    type: 5,
    icon: 'fund-icon',
    color: '778899',
    displayOrder: 1,
    subCategories: []
};

const mockCategoryStore: any = {
    allTransactionCategories: {
        3: [mockPrimaryCategory],
        5: [mockInvestmentCategory]
    },
    loadAllCategories: jest.fn(async () => undefined)
};

const mockAccountsStore: any = {
    allAccounts: [
        { id: 'wallet', name: 'Wallet' },
        { id: 'bank', name: 'Bank' }
    ]
};

const mockTagsStore: any = {
    allTransactionTags: [
        { id: 'daily', name: 'Daily' },
        { id: 'work', name: 'Work' }
    ]
};

const mockSettingsStore: any = { appSettings: { currency: 'CNY' } };
const mockUserStore: any = {
    currentUserFirstDayOfWeek: 1,
    currentUserFiscalYearStart: 0x0401
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => void) => mockMountedCallbacks.push(callback),
        useTemplateRef: () => actual.ref(null)
    };
});

jest.mock('vuetify', () => {
    const { ref } = jest.requireActual('vue') as any;
    return {
        useDisplay: () => ({ mdAndUp: ref(true) }),
        useTheme: () => ({ global: { name: ref('light') } })
    };
});

jest.mock('vue-router', () => ({
    useRouter: () => ({ push: mockRouterPush })
}));

jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: (..._bindings: unknown[]) => undefined
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => values
            ? `${key}:${JSON.stringify(values)}`
            : key,
        getAllDateRanges: () => [{ type: 101, name: 'Recent 12 months' }],
        formatDateRange: (type: number, min: number, max: number) => `${type}:${min}-${max}`,
        formatAmountToLocalizedNumeralsWithCurrency: (cents: number, currency: string) => `${currency} ${cents}`
    })
}));

jest.mock('@/stores/budget.ts', () => ({ useBudgetStore: () => mockBudgetStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoryStore
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagsStore }));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));

jest.mock('@/lib/datetime.ts', () => {
    const actual = jest.requireActual('@/lib/datetime.ts') as any;
    return {
        ...actual,
        getCurrentUnixTime: () => 1_800_000_000,
        getTodayFirstUnixTime: () => 1_799_971_200,
        getDateRangeByDateType: mockGetDateRangeByDateType,
        getShiftedDateRangeAndDateType: mockGetShiftedDateRangeAndDateType,
        getDateTypeByDateRange: () => 255
    };
});

jest.mock('@/core/theme.ts', () => ({
    isDarkApplicationTheme: (name: string) => name === 'dark',
    getApplicationThemeDefinition: () => ({ semantic: {
        surface: '#ffffff',
        chartText: '#413935',
        chartMutedText: 'rgba(65, 57, 53, 0.7)',
        chartGrid: 'rgba(65, 57, 53, 0.12)',
        tooltipBackground: '#ffffff',
        tooltipText: '#413935',
        tooltipBorder: 'rgba(65, 57, 53, 0.14)'
    } })
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: mockLogger
}));

for (const componentPath of [
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
    '@/components/desktop/BtnHorizontalGroup.vue',
    '@/components/desktop/BtnVerticalGroup.vue',
    '@/components/desktop/AmountInput.vue',
    '@/components/desktop/DateRangeSelectionDialog.vue',
    '@/views/desktop/common/cards/AccountFilterSettingsCard.vue',
    '@/views/desktop/common/cards/TransactionTagFilterSettingsCard.vue',
    '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue',
    '@/views/desktop/budgets/list/dialogs/EditDialog.vue',
    '@/views/desktop/budgets/components/BudgetForecastPanel.vue',
    '@/views/desktop/budgets/components/BudgetForecastSettingsDialog.vue',
    '@/views/desktop/budgets/components/BudgetHistoryPanel.vue',
    '@/views/desktop/budgets/components/BudgetListTable.vue'
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: { name: 'BudgetCoverageStub' }
    }));
}

import ListPage from '@/views/desktop/budgets/ListPage.vue';
import {
    Budget,
    BudgetForecastStrategy,
    BudgetPeriodType,
    BudgetType
} from '@/models/budget.ts';
import { AmountFilterType } from '@/core/numeral.ts';

function createBudget(overrides: Partial<Budget> = {}): Budget {
    const now = new Date();
    const currentMonthStart = dateOnly(new Date(now.getFullYear(), now.getMonth(), 1));
    const currentMonthEnd = dateOnly(new Date(now.getFullYear(), now.getMonth() + 1, 0));

    return Object.assign(Budget.createNew(BudgetType.Expense), {
        id: 'budget-1',
        name: 'Food Budget',
        category: 'Food',
        subCategory: '',
        categoryId: 'food',
        periodType: BudgetPeriodType.Monthly,
        amountCents: 10_000,
        spentAmountCents: 4_000,
        executionRate: 40,
        startDate: currentMonthStart,
        endDate: currentMonthEnd,
        categoryIcon: 'budget-icon',
        categoryColor: '112233'
    }, overrides);
}

function setupPage(props: Record<string, unknown> = {}): any {
    return (ListPage as any).setup(props, { expose: jest.fn() });
}

function dateOnly(date: Date): string {
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

function createUiHarness(bindings: any, confirmResult = true): {
    snackbar: { showMessage: jest.Mock; showError: jest.Mock };
    confirm: { open: jest.Mock<(...args: unknown[]) => Promise<boolean>> };
    edit: { open: jest.Mock };
    fileInput: { click: jest.Mock };
} {
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const confirm = {
        open: jest.fn<(...args: unknown[]) => Promise<boolean>>(async () => confirmResult)
    };
    const edit = { open: jest.fn() };
    const fileInput = { click: jest.fn() };
    bindings.snackbar.value = snackbar;
    bindings.confirmDialog.value = confirm;
    bindings.editDialog.value = edit;
    bindings.fileInput.value = fileInput;
    return { snackbar, confirm, edit, fileInput };
}

async function flushPromises(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockMountedCallbacks.length = 0;
    mockBudgetStore.allBudgets = [];
    mockBudgetStore.currentExecution = null;
    mockBudgetStore.currentForecast = null;
    mockBudgetStore.currentHistory = null;
    mockBudgetStore.currentHistoryRequestSignature = '';
    mockBudgetStore.forecastLoading = false;
    mockBudgetStore.loadAllBudgets.mockResolvedValue(undefined);
    mockBudgetStore.loadBudgetExecution.mockResolvedValue(undefined);
    mockBudgetStore.loadBudgetHistory.mockResolvedValue(undefined);
    mockBudgetStore.createBudgetHistorySnapshot.mockResolvedValue(undefined);
    mockBudgetStore.loadBudgetForecast.mockResolvedValue(undefined);
    mockBudgetStore.deleteBudget.mockResolvedValue(undefined);
    mockBudgetStore.exportBudgets.mockResolvedValue({ budgets: [] });
    mockBudgetStore.importBudgets.mockResolvedValue({ importedCount: 1, updatedCount: 2 });
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockGetDateRangeByDateType.mockImplementation((dateType: number) => dateType === -1
        ? null
        : { dateType, minTime: 1_704_067_200, maxTime: 1_735_603_199 });
    mockGetShiftedDateRangeAndDateType.mockImplementation((min: number, max: number, scale: number) => ({
        dateType: 255,
        minTime: min + scale * 86_400,
        maxTime: max + scale * 86_400
    }));

    const storage = new Map<string, string>();
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: (key: string) => storage.get(key) ?? null,
            setItem: (key: string, value: string) => storage.set(key, value),
            removeItem: (key: string) => storage.delete(key),
            clear: () => storage.clear()
        }
    });

    const link = { href: '', download: '', click: jest.fn() };
    Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: { createElement: jest.fn(() => link) }
    });
    Object.defineProperty(URL, 'createObjectURL', {
        configurable: true,
        value: jest.fn(() => 'blob:budget-export')
    });
    Object.defineProperty(URL, 'revokeObjectURL', {
        configurable: true,
        value: jest.fn()
    });
    Object.defineProperty(globalThis.window, 'requestAnimationFrame', {
        configurable: true,
        value: (callback: FrameRequestCallback) => {
            callback(0);
            return 1;
        }
    });
});

describe('desktop budget ListPage production-loaded behavior', () => {
    test('setup exposes the budget page orchestration surface', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const now = new Date();
            mockBudgetStore.allBudgets = [createBudget({
                startDate: dateOnly(new Date(now.getFullYear(), now.getMonth(), 1)),
                endDate: dateOnly(new Date(now.getFullYear(), now.getMonth() + 1, 0))
            })];
            const bindings = setupPage();
            expect(bindings.reload).toEqual(expect.any(Function));
            expect(bindings.filteredBudgets.value).toHaveLength(1);
            expect(mockMountedCallbacks).toHaveLength(1);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('filters, sorts, groups, and summarizes cents without changing units', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const now = new Date();
        const start = dateOnly(new Date(now.getFullYear(), now.getMonth(), 1));
        const end = dateOnly(new Date(now.getFullYear(), now.getMonth() + 1, 0));
        const budgets = [
            createBudget({
                id: 'primary-a',
                name: 'Primary A',
                startDate: start,
                endDate: end,
                amountCents: 10_000,
                spentAmountCents: 2_500,
                executionRate: 25
            }),
            createBudget({
                id: 'primary-b',
                name: 'Primary B',
                startDate: start,
                endDate: end,
                amountCents: 5_000,
                spentAmountCents: 1_000,
                executionRate: 20,
                categoryIcon: '',
                categoryColor: ''
            }),
            createBudget({
                id: 'cafe',
                name: 'Cafe Budget',
                subCategory: 'Cafe',
                categoryId: 'cafe',
                startDate: start,
                endDate: end,
                amountCents: 3_000,
                spentAmountCents: 1_500,
                executionRate: 50
            }),
            createBudget({
                id: 'fund',
                name: 'Fund Budget',
                category: 'Fund',
                categoryId: 'fund',
                type: BudgetType.Investment,
                startDate: start,
                endDate: end
            }),
            createBudget({
                id: 'housing-child',
                name: 'Rent',
                category: 'Housing',
                subCategory: 'Rent',
                categoryId: 'rent',
                startDate: start,
                endDate: end,
                amountCents: 20_000,
                spentAmountCents: 18_000,
                executionRate: 90,
                categoryColor: 'bad-color'
            })
        ];
        mockBudgetStore.allBudgets = budgets;

        try {
            const bindings = setupPage();
            const ui = createUiHarness(bindings);

            expect(bindings.budgetPeriodTypeButtons.value).toHaveLength(3);
            expect(bindings.visiblePeriodFilters.value).toHaveLength(2);
            expect(bindings.activePeriodFilterIndex.value).toBe(0);
            expect(bindings.filteredBudgets.value).toHaveLength(4);
            expect(bindings.groupedBudgets.value).toHaveLength(2);

            const foodGroup = bindings.groupedBudgets.value.find((group: any) => group.category === 'Food');
            const housingGroup = bindings.groupedBudgets.value.find((group: any) => group.category === 'Housing');
            expect(foodGroup).toMatchObject({
                totalAmountCents: 18_000,
                totalSpentCents: 5_000,
                primaryAmountCents: 15_000,
                subTotalAmountCents: 3_000,
                categoryIcon: 'food-icon',
                categoryColor: '112233'
            });
            expect(housingGroup).toMatchObject({ totalAmountCents: 20_000, totalSpentCents: 18_000 });
            expect(bindings.filteredSummary.value).toMatchObject({
                totalBudgetCents: 38_000,
                totalSpentCents: 23_000
            });
            expect(bindings.getPrimaryBudgetForHeader(foodGroup).id).toBe('primary-a');
            expect(bindings.getExpandedPrimaryBudgets(foodGroup)).toHaveLength(2);
            expect(bindings.getBudgetCategoryIcon(budgets[1], foodGroup)).toBe('food-icon');
            expect(bindings.getBudgetCategoryColor(budgets[1], foodGroup)).toBe('112233');
            expect(bindings.groupHasExpandedRows(foodGroup)).toBe(true);
            expect(bindings.getGroupExecutionRate(foodGroup)).toBeCloseTo(27.7777);
            expect(bindings.getGroupExecutionRateText(foodGroup)).toBe('27.8%');

            bindings.toggleCategoryCollapse('Food');
            expect(bindings.collapsedCategories.value.has('Food')).toBe(true);
            bindings.toggleCategoryCollapse('Food');
            expect(bindings.collapsedCategories.value.has('Food')).toBe(false);
            bindings.toggleAllCategories();
            expect(bindings.isAllExpanded.value).toBe(false);
            bindings.toggleAllCategories();
            expect(bindings.isAllExpanded.value).toBe(true);

            for (const [sortBy, descending] of [
                ['category', false],
                ['executionRate', true],
                ['spent', false],
                ['budget', true],
                ['unknown', false]
            ] as const) {
                bindings.sortBy.value = sortBy;
                bindings.sortDesc.value = descending;
                expect(bindings.sortedBudgets.value).toHaveLength(4);
            }

            bindings.searchKeyword.value = 'primary a';
            expect(bindings.filteredBudgets.value.map((budget: Budget) => budget.id)).toEqual(['primary-a']);
            bindings.searchKeyword.value = 'food';
            expect(bindings.filteredBudgets.value.length).toBeGreaterThan(0);
            bindings.searchKeyword.value = 'cafe';
            expect(bindings.filteredBudgets.value.map((budget: Budget) => budget.id)).toEqual(['cafe']);
            bindings.searchKeyword.value = '';

            bindings.categoryFilter.value = 'food';
            expect(bindings.filteredBudgets.value).toHaveLength(3);
            bindings.categoryFilter.value = 'cafe';
            expect(bindings.filteredBudgets.value.map((budget: Budget) => budget.id)).toEqual(['cafe']);
            bindings.categoryFilter.value = 'missing';
            expect(bindings.filteredBudgets.value).toHaveLength(4);
            bindings.categoryFilter.value = null;

            bindings.executionRateFilter.value = { label: 'range', value: 'range', min: 20, max: 50 };
            expect(bindings.filteredBudgets.value).toHaveLength(3);
            bindings.executionRateFilter.value = { label: 'minimum', value: 'min', min: 80 };
            expect(bindings.filteredBudgets.value.map((budget: Budget) => budget.id)).toEqual(['housing-child']);
            bindings.executionRateFilter.value = { label: 'maximum', value: 'max', max: 30 };
            expect(bindings.filteredBudgets.value).toHaveLength(2);
            bindings.executionRateFilter.value = null;

            bindings.spentAmountFilterCents.value = `${AmountFilterType.GreaterThan.type}:2000`;
            expect(bindings.filteredBudgets.value.map((budget: Budget) => budget.id)).toEqual(['primary-a', 'housing-child']);
            bindings.spentAmountFilterCents.value = 'invalid';
            expect(bindings.filteredBudgets.value).toHaveLength(4);
            bindings.spentAmountFilterCents.value = '';
            bindings.budgetAmountFilterCents.value = `${AmountFilterType.Between.type}:3000:10000`;
            expect(bindings.filteredBudgets.value).toHaveLength(3);
            bindings.budgetAmountFilterCents.value = 'invalid';
            expect(bindings.filteredBudgets.value).toHaveLength(4);
            bindings.budgetAmountFilterCents.value = '';

            expect(bindings.getBudgetProgressColor(budgets[0])).toBe('#112233');
            expect(bindings.getBudgetProgressColor(createBudget({ categoryColor: '' }))).toBeTruthy();
            expect(bindings.getGroupProgressColor(foodGroup)).toBe('#112233');
            expect(bindings.getGroupProgressColor({ ...housingGroup, categoryColor: '' })).toBeTruthy();
            expect(bindings.formatFilterAmountCents(12_345)).toBe('CNY 12345');
            expect(ui.snackbar.showError).not.toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('covers every relative period filter and custom date boundary', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const now = new Date();
        const makePeriodBudget = (
            id: string,
            periodType: BudgetPeriodType,
            startDate: Date,
            endDate: Date
        ) => createBudget({ id, periodType, startDate: dateOnly(startDate), endDate: dateOnly(endDate) });
        const budgets = [
            makePeriodBudget('this-month', BudgetPeriodType.Monthly,
                new Date(now.getFullYear(), now.getMonth(), 1), new Date(now.getFullYear(), now.getMonth() + 1, 0)),
            makePeriodBudget('last-month', BudgetPeriodType.Monthly,
                new Date(now.getFullYear(), now.getMonth() - 1, 1), new Date(now.getFullYear(), now.getMonth(), 0)),
            makePeriodBudget('this-quarter', BudgetPeriodType.Quarterly,
                new Date(now.getFullYear(), Math.floor(now.getMonth() / 3) * 3, 1),
                new Date(now.getFullYear(), Math.floor(now.getMonth() / 3) * 3 + 3, 0)),
            makePeriodBudget('last-quarter', BudgetPeriodType.Quarterly,
                new Date(now.getFullYear(), Math.floor(now.getMonth() / 3) * 3 - 3, 1),
                new Date(now.getFullYear(), Math.floor(now.getMonth() / 3) * 3, 0)),
            makePeriodBudget('this-year', BudgetPeriodType.Yearly,
                new Date(now.getFullYear(), 0, 1), new Date(now.getFullYear(), 11, 31)),
            makePeriodBudget('last-year', BudgetPeriodType.Yearly,
                new Date(now.getFullYear() - 1, 0, 1), new Date(now.getFullYear() - 1, 11, 31)),
            createBudget({ id: 'expired', endDate: '2000-01-01' }),
            createBudget({ id: 'no-end', endDate: '' })
        ];
        try {
            const bindings = setupPage();
            createUiHarness(bindings);
            for (const [filter, expectedId] of [
                ['thisMonth', 'this-month'],
                ['lastMonth', 'last-month'],
                ['thisQuarter', 'this-quarter'],
                ['lastQuarter', 'last-quarter'],
                ['thisYear', 'this-year'],
                ['lastYear', 'last-year']
            ]) {
                bindings.activePeriodFilter.value = filter;
                expect(bindings.filterByPeriod(budgets).map((budget: Budget) => budget.id)).toContain(expectedId);
            }
            bindings.activePeriodFilter.value = 'expired';
            expect(bindings.filterByPeriod(budgets).map((budget: Budget) => budget.id)).toContain('expired');
            bindings.activePeriodFilter.value = 'custom';
            expect(bindings.filterByPeriod(budgets)).toEqual(budgets);
            bindings.customStartDate.value = dateOnly(new Date(now.getFullYear(), now.getMonth(), 1));
            bindings.customEndDate.value = dateOnly(new Date(now.getFullYear(), now.getMonth() + 1, 0));
            expect(bindings.filterByPeriod(budgets).map((budget: Budget) => budget.id)).toContain('this-month');
            bindings.activePeriodFilter.value = 'unsupported';
            expect(bindings.filterByPeriod(budgets)).toEqual(budgets);

            bindings.onCustomDateRangeChange(1_704_067_200, 1_706_745_599);
            expect(bindings.customStartDate.value).toBe('2024-01-01');
            expect(bindings.customEndDate.value).toBe('2024-02-01');
            expect(bindings.activePeriodFilter.value).toBe('custom');

            for (const filter of ['thisMonth', 'lastMonth', 'thisQuarter', 'lastQuarter', 'thisYear', 'lastYear', 'custom', 'unknown']) {
                bindings.activePeriodFilter.value = filter;
                expect(bindings.getCurrentPeriodRequest()).toHaveProperty('periodType');
                expect(bindings.getCurrentPeriodType()).toBeTruthy();
            }
            bindings.customStartDate.value = '';
            bindings.customEndDate.value = '';
            bindings.activePeriodFilter.value = 'custom';
            expect(bindings.getCurrentPeriodRequest()).toMatchObject({ startDate: undefined, endDate: undefined });
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('covers filter controls, presets, labels, and drilldown navigation', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const bindings = setupPage();
            const ui = createUiHarness(bindings);

            expect(bindings.getAccountFilterDisplayName()).toBe('All Accounts');
            bindings.accountFilter.value = ['wallet'];
            expect(bindings.getAccountFilterDisplayName()).toBe('Wallet');
            bindings.accountFilter.value = ['missing'];
            expect(bindings.getAccountFilterDisplayName()).toBe('Selected Account');
            bindings.accountFilter.value = ['wallet', 'bank'];
            expect(bindings.getAccountFilterDisplayName()).toBe('2 Accounts');

            expect(bindings.getTagFilterDisplayName()).toBe('All Tags');
            bindings.tagFilter.value = ['daily'];
            expect(bindings.getTagFilterDisplayName()).toBe('Daily');
            bindings.tagFilter.value = ['missing'];
            expect(bindings.getTagFilterDisplayName()).toBe('Selected Tag');
            bindings.tagFilter.value = ['daily', 'work'];
            expect(bindings.getTagFilterDisplayName()).toBe('2 Tags');

            bindings.showFilterAccountDialog.value = true;
            bindings.setAccountFilter(false);
            expect(bindings.showFilterAccountDialog.value).toBe(false);
            bindings.setAccountFilter(true);
            bindings.showFilterTagDialog.value = true;
            bindings.setTagFilter(false);
            bindings.setTagFilter(true);
            bindings.showFilterCategoryDialog.value = true;
            bindings.onCategoryFilterDialogChange(false);
            bindings.onCategoryFilterDialogChange(true);
            bindings.setKeywordFilter('Food');
            expect(bindings.searchKeyword.value).toBe('Food');

            bindings.onSpentFilterTypeClick('gt');
            expect(bindings.currentSpentFilterType.value).toBe('gt');
            bindings.onSpentFilterTypeClick('gt');
            expect(bindings.currentSpentFilterType.value).toBe('');
            bindings.changeSpentFilter('');
            bindings.changeSpentFilter('unknown');
            bindings.currentSpentFilterValue1.value = 100;
            bindings.changeSpentFilter(AmountFilterType.GreaterThan.type);
            expect(bindings.spentAmountFilterCents.value).toBe('gt:100');
            bindings.currentSpentFilterValue1.value = 200;
            bindings.currentSpentFilterValue2.value = 100;
            bindings.changeSpentFilter(AmountFilterType.Between.type);
            expect(ui.snackbar.showMessage).toHaveBeenCalledWith('Incorrect amount range');
            bindings.currentSpentFilterValue2.value = 300;
            bindings.changeSpentFilter(AmountFilterType.Between.type);
            expect(bindings.getSpentFilterDisplayName()).toContain('CNY 200');
            expect(bindings.getSpentFilterLabel()).toBe('CNY 200~CNY 300');
            bindings.spentAmountFilterCents.value = 'bad';
            expect(bindings.getSpentFilterDisplayName()).toBe('Spent');
            expect(bindings.getSpentFilterLabel()).toBe('');
            bindings.spentAmountFilterCents.value = '';
            expect(bindings.getSpentFilterDisplayName()).toBe('Spent');
            expect(bindings.getSpentFilterLabel()).toBe('');

            bindings.onBudgetFilterTypeClick('lt');
            expect(bindings.currentBudgetFilterType.value).toBe('lt');
            bindings.onBudgetFilterTypeClick('lt');
            bindings.changeBudgetFilter('');
            bindings.changeBudgetFilter('unknown');
            bindings.currentBudgetFilterValue1.value = 500;
            bindings.changeBudgetFilter(AmountFilterType.LessThan.type);
            expect(bindings.budgetAmountFilterCents.value).toBe('lt:500');
            bindings.currentBudgetFilterValue1.value = 600;
            bindings.currentBudgetFilterValue2.value = 500;
            bindings.changeBudgetFilter(AmountFilterType.NotBetween.type);
            bindings.currentBudgetFilterValue2.value = 700;
            bindings.changeBudgetFilter(AmountFilterType.NotBetween.type);
            expect(bindings.getBudgetFilterDisplayName()).toContain('CNY 600');
            expect(bindings.getBudgetFilterLabel()).toBe('CNY 600~CNY 700');
            bindings.budgetAmountFilterCents.value = 'bad';
            expect(bindings.getBudgetFilterDisplayName()).toBe('Budget');
            expect(bindings.getBudgetFilterLabel()).toBe('');
            bindings.budgetAmountFilterCents.value = '';
            expect(bindings.getBudgetFilterDisplayName()).toBe('Budget');
            expect(bindings.getBudgetFilterLabel()).toBe('');

            bindings.presetName.value = '   ';
            bindings.savePreset();
            expect(ui.snackbar.showMessage).toHaveBeenCalledWith('Please enter a preset name');
            bindings.categoryFilter.value = 'cafe';
            bindings.accountFilter.value = ['wallet'];
            bindings.tagFilter.value = ['daily'];
            bindings.executionRateFilter.value = { label: 'range', value: 'range', min: 10, max: 90 };
            bindings.spentAmountFilterCents.value = 'gt:100';
            bindings.budgetAmountFilterCents.value = 'lt:900';
            bindings.presetName.value = ' Daily budget ';
            bindings.savePreset();
            expect(bindings.filterPresets.value).toHaveLength(1);
            const preset = bindings.filterPresets.value[0];
            bindings.clearAllFilters();
            expect(bindings.hasActiveFilters.value).toBe(true);
            bindings.accountFilter.value = [];
            bindings.tagFilter.value = [];
            expect(bindings.hasActiveFilters.value).toBe(false);
            bindings.loadPreset(preset);
            expect(bindings.hasActiveFilters.value).toBe(true);
            bindings.deletePreset(preset.id);
            expect(bindings.filterPresets.value).toEqual([]);

            expect(bindings.getCategoryFilterDisplayName()).toBe('Food > Cafe');
            bindings.categoryFilter.value = 'food';
            expect(bindings.getCategoryFilterDisplayName()).toBe('Food');
            bindings.categoryFilter.value = 'missing';
            expect(bindings.getCategoryFilterDisplayName()).toBe('missing');
            bindings.categoryFilter.value = null;
            expect(bindings.getCategoryFilterDisplayName()).toBe('');
            bindings.setExecutionRateFilter(null);
            bindings.setExecutionRateFilter({ label: 'all', value: 'all' });

            const budget = createBudget();
            bindings.accountFilter.value = ['wallet'];
            bindings.tagFilter.value = ['daily'];
            bindings.navigateToTransactions('Food', budget);
            expect(mockRouterPush).toHaveBeenCalledWith(expect.objectContaining({ path: '/transaction/list' }));
            bindings.activeBudgetType.value = BudgetType.Investment;
            bindings.navigateToTransactions('Fund', null);
            expect(mockRouterPush).toHaveBeenCalledTimes(2);

            bindings.toggleHistoricalPeriodFocus('2026-01');
            expect(bindings.selectedHistoricalPeriodKey.value).toBe('2026-01');
            bindings.toggleHistoricalPeriodFocus('2026-01');
            expect(bindings.selectedHistoricalPeriodKey.value).toBeNull();

            await flushPromises();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('builds monthly, quarterly, yearly, and fiscal historical periods', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const bindings = setupPage();
            createUiHarness(bindings);

            expect(bindings.buildHistoricalAggregationPeriods(
                BudgetPeriodType.Monthly,
                '2024-01-15',
                '2024-03-05'
            ).map((period: any) => period.key)).toEqual(['2024-01', '2024-02', '2024-03']);
            expect(bindings.buildHistoricalAggregationPeriods(
                BudgetPeriodType.Quarterly,
                '2024-02-01',
                '2024-10-01'
            ).map((period: any) => period.key)).toEqual(['2024-Q1', '2024-Q2', '2024-Q3', '2024-Q4']);
            expect(bindings.buildHistoricalAggregationPeriods(
                BudgetPeriodType.Yearly,
                '2023-06-01',
                '2025-01-01'
            ).map((period: any) => period.key)).toEqual(['2023', '2024', '2025']);
            expect(bindings.buildHistoricalAggregationPeriods(
                bindings.HISTORICAL_FISCAL_YEAR,
                '2024-05-01',
                '2026-03-31'
            ).map((period: any) => period.key)).toEqual(['FY2025', 'FY2026']);
            expect(bindings.buildHistoricalAggregationPeriods(BudgetPeriodType.Monthly, 'bad', '2024-01-01')).toEqual([]);
            expect(bindings.buildHistoricalAggregationPeriods(BudgetPeriodType.Monthly, '2024-02-01', '2024-01-01')).toEqual([]);

            const target = new Date(2024, 7, 10);
            expect(bindings.resolveHistoricalPeriodByDate(BudgetPeriodType.Monthly, target).key).toBe('2024-08');
            expect(bindings.resolveHistoricalPeriodByDate(BudgetPeriodType.Quarterly, target).key).toBe('2024-Q3');
            expect(bindings.resolveHistoricalPeriodByDate(BudgetPeriodType.Yearly, target).key).toBe('2024');
            expect(bindings.resolveHistoricalPeriodByDate(bindings.HISTORICAL_FISCAL_YEAR, target).key).toBe('FY2025');
            expect(bindings.buildFiscalYearPeriod(new Date(2024, 1, 1)).key).toBe('FY2024');

            bindings.historicalAggregationType.value = bindings.HISTORICAL_FISCAL_YEAR;
            expect(bindings.activeHistoricalAggregationType.value).toBe(BudgetPeriodType.Yearly);
            expect(bindings.getHistoricalRequestPeriodType()).toBe(BudgetPeriodType.Yearly);
            bindings.activeHistoricalAggregationType.value = BudgetPeriodType.Quarterly;
            expect(bindings.historicalAggregationType.value).toBe(BudgetPeriodType.Quarterly);
            bindings.activeHistoricalAggregationType.value = BudgetPeriodType.Quarterly;
            expect(bindings.historicalAggregationType.value).toBe(BudgetPeriodType.Quarterly);
            expect(bindings.getHistoricalRequestPeriodType()).toBe(BudgetPeriodType.Quarterly);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('projects historical cents into chart and grouped budget contracts', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const historyItems = [
            {
                id: 'h-primary', budgetId: 'primary', name: 'Food', type: BudgetType.Expense,
                category: 'Food', subCategory: '', periodType: BudgetPeriodType.Monthly,
                periodStart: '2024-01-01', periodEnd: '2024-01-31',
                budgetAmountCents: 10_000, spentAmountCents: 0, remainingAmountCents: 10_000,
                executionRate: 0, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            },
            {
                id: 'h-cafe', budgetId: 'cafe', name: 'Cafe', type: BudgetType.Expense,
                category: 'Food', subCategory: 'Cafe', periodType: BudgetPeriodType.Monthly,
                periodStart: '2024-01-01', periodEnd: '2024-01-31',
                budgetAmountCents: 5_000, spentAmountCents: 4_000, remainingAmountCents: 1_000,
                executionRate: 80, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            },
            {
                id: 'h-food-feb', budgetId: 'cafe-feb', name: 'Cafe', type: BudgetType.Expense,
                category: 'Food', subCategory: 'Cafe', periodType: BudgetPeriodType.Monthly,
                periodStart: '2024-02-01', periodEnd: '2024-02-29',
                budgetAmountCents: 2_000, spentAmountCents: 1_000, remainingAmountCents: 1_000,
                executionRate: 50, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            },
            {
                id: 'h-housing', budgetId: 'housing', name: 'Housing', type: BudgetType.Expense,
                category: 'Housing', subCategory: '', periodType: BudgetPeriodType.Monthly,
                periodStart: '2024-01-01', periodEnd: '2024-01-31',
                budgetAmountCents: 8_000, spentAmountCents: 7_000, remainingAmountCents: 1_000,
                executionRate: 87.5, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            },
            {
                id: 'h-zero', budgetId: 'zero', name: 'Zero', type: BudgetType.Expense,
                category: 'Misc', subCategory: 'Zero', periodType: BudgetPeriodType.Monthly,
                periodStart: '2024-01-01', periodEnd: '2024-01-31',
                budgetAmountCents: 0, spentAmountCents: 0, remainingAmountCents: 0,
                executionRate: 0, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            },
            {
                id: 'h-uncategorized', budgetId: 'uncategorized', name: '', type: BudgetType.Expense,
                category: '', subCategory: 'Other', periodType: BudgetPeriodType.Monthly,
                periodStart: '2024-01-01', periodEnd: '2024-01-31',
                budgetAmountCents: 100, spentAmountCents: 200, remainingAmountCents: -100,
                executionRate: 200, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            },
            {
                id: 'h-investment', budgetId: 'fund', name: 'Fund', type: BudgetType.Investment,
                category: 'Fund', subCategory: '', periodType: BudgetPeriodType.Monthly,
                periodStart: '2024-01-01', periodEnd: '2024-01-31',
                budgetAmountCents: 9_000, spentAmountCents: 9_000, remainingAmountCents: 0,
                executionRate: 100, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            },
            {
                id: 'h-invalid', budgetId: 'invalid', name: 'Invalid', type: BudgetType.Expense,
                category: 'Food', subCategory: 'Cafe', periodType: BudgetPeriodType.Monthly,
                periodStart: 'not-a-date', periodEnd: 'not-a-date',
                budgetAmountCents: 100, spentAmountCents: 100, remainingAmountCents: 0,
                executionRate: 100, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            },
            {
                id: 'h-outside', budgetId: 'outside', name: 'Outside', type: BudgetType.Expense,
                category: 'Food', subCategory: 'Cafe', periodType: BudgetPeriodType.Monthly,
                periodStart: '2023-01-01', periodEnd: '2023-01-31',
                budgetAmountCents: 100, spentAmountCents: 100, remainingAmountCents: 0,
                executionRate: 100, status: 'active', filterSummary: '', calculatedAt: '', alertThreshold: 80, enabled: true
            }
        ];

        try {
            const signatureSeed = setupPage();
            const signature = signatureSeed.activeHistoricalHistoryRequestSignature.value;
            mockBudgetStore.currentHistory = {
                items: historyItems,
                count: historyItems.length,
                periodStart: '2024-01-01',
                periodEnd: '2024-12-31'
            };
            mockBudgetStore.currentHistoryRequestSignature = signature;

            const bindings = setupPage();
            createUiHarness(bindings);
            expect(bindings.isHistoricalHistoryReady.value).toBe(true);
            expect(bindings.filteredHistoricalItems.value).toHaveLength(8);
            expect(bindings.historicalPeriods.value).toHaveLength(12);
            expect(bindings.historicalCategoryChartData.value.points.length).toBeGreaterThanOrEqual(3);
            expect(bindings.historicalCategoryChartData.value.points).toEqual(expect.arrayContaining([
                expect.objectContaining({
                    category: 'Cafe',
                    budgetAmountCents: 7_000,
                    spentAmountCents: 5_000,
                    color: '#445566'
                })
            ]));
            expect(bindings.historicalChartModel.value.primaryBands.length).toBeGreaterThan(0);
            expect(bindings.historicalLegendGroups.value.length).toBeGreaterThan(0);
            expect(bindings.canShowHistoricalBudgetPanel.value).toBe(true);
            expect(bindings.historicalChartOptions.value).toHaveProperty('series');
            expect(bindings.historicalCategoryMeta.value.Food.subCategories.Cafe).toMatchObject({
                color: '445566', icon: 'cafe-icon'
            });
            expect(bindings.historicalBudgetGroups.value.length).toBeGreaterThan(0);

            bindings.historicalBudgetLevel.value = 'primary';
            await flushPromises();
            expect(bindings.historicalCategoryChartData.value.points).toEqual(expect.arrayContaining([
                expect.objectContaining({
                    category: 'Food',
                    budgetAmountCents: 12_000,
                    spentAmountCents: 5_000,
                    color: '#112233'
                }),
                expect.objectContaining({ category: 'Housing', budgetAmountCents: 8_000 })
            ]));

            bindings.categoryFilter.value = 'cafe';
            expect(bindings.filteredHistoricalItems.value.every((item: any) => item.subCategory === 'Cafe')).toBe(true);
            bindings.categoryFilter.value = 'food';
            expect(bindings.filteredHistoricalItems.value.every((item: any) => item.category === 'Food')).toBe(true);
            bindings.categoryFilter.value = null;

            bindings.selectedHistoricalPeriodKey.value = '2024-01';
            expect(bindings.periodFilteredHistoricalItems.value.every((item: any) => item.periodStart === '2024-01-01')).toBe(true);
            expect(bindings.visibleHistoricalBudgetGroups.value.every((group: any) => group.key === '2024-01')).toBe(true);
            bindings.selectedHistoricalPeriodKey.value = null;
            expect(bindings.visibleHistoricalBudgetGroups.value).toEqual(bindings.historicalBudgetGroups.value);

            const firstGroup = bindings.historicalBudgetGroups.value[0];
            const firstRow = firstGroup.rows[0];
            expect(bindings.buildHistoricalPrimaryCollapseKey(firstGroup.key, firstRow)).toContain('::');
            expect(bindings.isHistoricalPrimaryCollapsed(firstGroup.key, firstRow)).toBe(false);
            bindings.toggleHistoricalPrimaryCollapse(firstGroup.key, firstRow);
            expect(bindings.isHistoricalPrimaryCollapsed(firstGroup.key, firstRow)).toBe(true);
            bindings.toggleHistoricalPrimaryCollapse(firstGroup.key, firstRow);
            expect(bindings.isHistoricalPrimaryCollapsed(firstGroup.key, firstRow)).toBe(false);

            bindings.toggleHistoricalPrimaryLegend(bindings.historicalLegendGroups.value[0].key);
            bindings.toggleHistoricalSecondaryLegend('Food::Cafe');
            bindings.historicalExpandedGroupKeys.value = [];
            bindings.toggleAllHistoricalGroups();
            expect(bindings.historicalExpandedGroupKeys.value).toHaveLength(bindings.historicalBudgetGroups.value.length);
            bindings.toggleAllHistoricalGroups();
            expect(bindings.historicalExpandedGroupKeys.value).toEqual([]);

            const resize = jest.fn();
            const nestedResize = jest.fn();
            bindings.historicalChartRef.value = { resize, chart: { resize: nestedResize } };
            bindings.scheduleHistoricalChartResize();
            await flushPromises();
            expect(resize).toHaveBeenCalled();
            expect(nestedResize).toHaveBeenCalled();

            mockBudgetStore.currentHistoryRequestSignature = 'stale';
            const staleBindings = setupPage();
            expect(staleBindings.isHistoricalHistoryReady.value).toBe(false);
            expect(staleBindings.filteredHistoricalItems.value).toEqual([]);
            expect(staleBindings.historicalCategoryChartData.value).toEqual({ categories: [], points: [] });
            expect(staleBindings.historicalChartOptions.value).toEqual({});
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('controls historical ranges and preserves stale-request safety', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const bindings = setupPage();
            const ui = createUiHarness(bindings);
            expect(bindings.historicalDateRangeName.value).toBe('Recent 12 months');
            expect(bindings.historicalDateStartText.value).toBe('');
            expect(bindings.historicalDateEndText.value).toBe('');
            expect(bindings.canShiftHistoricalDateRange.value).toBe(false);

            bindings.ensureHistoricalDateRangeInitialized();
            expect(bindings.historicalMinDatetime.value).toBe(1_704_067_200);
            expect(bindings.historicalMaxDatetime.value).toBe(1_735_603_199);
            expect(bindings.historicalDateRangeName.value).toContain('101:');
            expect(bindings.historicalDateStartText.value).toBe('2024-01-01');
            expect(bindings.historicalDateEndText.value).toBe('2024-12-31');
            expect(bindings.canShiftHistoricalDateRange.value).toBe(true);
            bindings.ensureHistoricalDateRangeInitialized();

            expect(bindings.canShowHistoricalCustomDateRange(255)).toBe(false);
            bindings.setHistoricalDateFilter(255);
            expect(bindings.showHistoricalDateDialog.value).toBe(true);
            expect(bindings.canShowHistoricalCustomDateRange(255)).toBe(false);
            bindings.historicalDateType.value = 255;
            expect(bindings.canShowHistoricalCustomDateRange(255)).toBe(true);
            expect(bindings.canShowHistoricalCustomDateRange(101)).toBe(false);

            bindings.setHistoricalDateFilter(101);
            expect(bindings.historicalDateType.value).toBe(101);
            mockGetDateRangeByDateType.mockReturnValueOnce(null);
            bindings.setHistoricalDateFilter(102);
            expect(bindings.historicalDateType.value).toBe(101);

            bindings.onHistoricalDateRangeChange(1_704_067_200, 1_706_745_599);
            expect(bindings.historicalDateType.value).toBe(255);
            expect(bindings.showHistoricalDateDialog.value).toBe(false);
            bindings.shiftHistoricalDateRange(1);
            expect(mockGetShiftedDateRangeAndDateType).toHaveBeenCalled();
            bindings.historicalDateType.value = 0;
            bindings.shiftHistoricalDateRange(-1);

            bindings.onDateRangeError('range error');
            expect(ui.snackbar.showMessage).toHaveBeenCalledWith('range error');

            bindings.historicalMinDatetime.value = Number.NaN;
            bindings.historicalMaxDatetime.value = Number.NaN;
            mockGetDateRangeByDateType.mockReturnValue(null);
            const snapshot = bindings.getHistoricalDateRangeSnapshot();
            expect(snapshot).toEqual({ dateType: 101, minTime: 1_800_000_000, maxTime: 1_800_000_000 });
            bindings.ensureHistoricalDateRangeInitialized();
            expect(Number.isNaN(bindings.historicalMinDatetime.value)).toBe(true);

            mockGetDateRangeByDateType.mockImplementation((dateType: number) => ({
                dateType,
                minTime: 1_704_067_200,
                maxTime: 1_735_603_199
            }));
            mockBudgetStore.createBudgetHistorySnapshot.mockRejectedValueOnce(new Error('snapshot unavailable'));
            bindings.reloadRequestId.value = 9;
            await bindings.loadHistoricalBudgetView(8);
            expect(mockLogger.warn).toHaveBeenCalled();
            expect(mockBudgetStore.loadBudgetHistory).not.toHaveBeenCalled();

            bindings.activeViewMode.value = 'history';
            bindings.reloadRequestId.value = 10;
            await bindings.loadHistoricalBudgetView(10);
            expect(mockBudgetStore.loadBudgetHistory).toHaveBeenCalledWith(expect.objectContaining({
                type: BudgetType.Expense,
                startDate: '2024-01-01',
                endDate: '2024-12-31'
            }));
            await bindings.loadBudgetHistoryForExpired(10);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('orchestrates reload, forecast, view switches, and failure reporting', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const bindings = setupPage();
            const ui = createUiHarness(bindings);

            await bindings.reload(false);
            expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
            expect(mockBudgetStore.loadAllBudgets).toHaveBeenCalledWith({
                force: false,
                type: BudgetType.Expense,
                periodType: BudgetPeriodType.Monthly
            });
            expect(mockBudgetStore.loadBudgetExecution).toHaveBeenCalled();
            expect(bindings.loading.value).toBe(false);

            mockCategoryStore.loadAllCategories.mockRejectedValueOnce({ message: 'category failure' });
            await bindings.reload(true);
            expect(ui.snackbar.showError).toHaveBeenCalledWith('category failure');
            mockCategoryStore.loadAllCategories.mockRejectedValueOnce({ isUpToDate: true, message: 'ignored category' });
            await bindings.reload(false);
            expect(ui.snackbar.showError).not.toHaveBeenCalledWith('ignored category');

            mockBudgetStore.loadAllBudgets.mockRejectedValueOnce({ message: 'budget failure' });
            await bindings.reload(false);
            expect(ui.snackbar.showError).toHaveBeenCalledWith('budget failure');
            mockBudgetStore.loadAllBudgets.mockRejectedValueOnce({ isUpToDate: true, message: 'ignored budget' });
            await bindings.reload(false);
            expect(ui.snackbar.showError).not.toHaveBeenCalledWith('ignored budget');

            mockBudgetStore.loadBudgetExecution.mockImplementationOnce(async () => {
                bindings.reloadRequestId.value += 1;
            });
            await bindings.reload(false);
            expect(bindings.loading.value).toBe(true);
            bindings.loading.value = false;

            bindings.activePeriodFilter.value = 'expired';
            await bindings.reload(false);
            await flushPromises();
            expect(mockBudgetStore.createBudgetHistorySnapshot).toHaveBeenCalled();

            bindings.activeViewMode.value = 'forecast';
            bindings.activePeriodFilter.value = 'thisQuarter';
            await bindings.reload(false);
            await flushPromises();
            expect(mockBudgetStore.loadBudgetForecast).toHaveBeenCalled();

            bindings.forecastMonthsHistory.value = 9;
            bindings.forecastStrategy.value = BudgetForecastStrategy.MovingAverage;
            await bindings.loadForecast();
            expect(mockBudgetStore.loadBudgetForecast).toHaveBeenLastCalledWith(expect.objectContaining({
                monthsHistory: 9,
                forecastStrategy: BudgetForecastStrategy.MovingAverage
            }));
            mockBudgetStore.loadBudgetForecast.mockRejectedValueOnce(new Error('forecast failure'));
            await bindings.loadForecast();
            expect(ui.snackbar.showError).toHaveBeenCalledWith('forecast failure');
            mockBudgetStore.loadBudgetForecast.mockRejectedValueOnce({});
            await bindings.loadForecast();
            expect(ui.snackbar.showError).toHaveBeenCalledWith('Failed to load forecast');

            bindings.activePeriodFilter.value = 'expired';
            bindings.switchViewMode('forecast');
            expect(bindings.activePeriodFilter.value).toBe('thisMonth');
            bindings.switchViewMode('budget');
            bindings.switchViewMode('history');
            bindings.switchBudgetType(BudgetType.Investment);
            expect(bindings.activeBudgetType.value).toBe(BudgetType.Investment);

            bindings.activeViewMode.value = 'history';
            await bindings.reload(false);
            expect(mockBudgetStore.loadBudgetHistory).toHaveBeenCalled();
            bindings.activeHistoricalAggregationType.value = BudgetPeriodType.Yearly;
            await flushPromises();

            bindings.setPeriodFilter('lastYear');
            expect(bindings.activePeriodFilter.value).toBe('lastYear');
            bindings.activeBudgetRelativeScope.value = 'current';
            bindings.activeBudgetRelativeScope.value = 'current';
            bindings.activeBudgetPeriodType.value = BudgetPeriodType.Monthly;
            bindings.activeBudgetPeriodType.value = BudgetPeriodType.Monthly;

            bindings.currentForecast.value;
            bindings.currentExecution.value;
            bindings.forecastLoading.value;
            bindings.forecastRiskSummary.value;
            bindings.displayForecasts.value;
            bindings.currentViewTitle.value;
            bindings.activeViewMode.value = 'forecast';
            expect(bindings.currentViewTitle.value).toBe('Period Forecast');
            bindings.activeViewMode.value = 'history';
            expect(bindings.currentViewTitle.value).toBe('Historical Budget Execution');
            bindings.activeViewMode.value = 'budget';
            expect(bindings.currentViewTitle.value).toBe('Budget Management');
            expect(bindings.viewModeButtons.value).toHaveLength(3);
            expect(bindings.forecastStrategies.value).toHaveLength(2);
            expect(bindings.forecastSortOptions.value).toHaveLength(4);
            expect(bindings.historyPeriodOptions.value).toEqual([3, 6, 9, 12]);
            expect(bindings.executionRateOptions.value).toHaveLength(4);
            expect(bindings.allHistoricalDateRanges.value).toHaveLength(1);
            expect(bindings.defaultCurrency.value).toBe('CNY');
            expect(bindings.alwaysShowNav.value).toBe(true);
            expect(bindings.isDarkMode.value).toBe(false);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('covers budget edit, delete, export, and import outcomes', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const bindings = setupPage();
            const ui = createUiHarness(bindings);
            const budget = createBudget({ id: 'crud-budget', name: '', category: 'Food' });
            const group = {
                category: 'Food', categoryIcon: 'food-icon', categoryColor: '112233',
                primaryBudgets: [], subBudgets: [budget],
                totalAmountCents: 12_345, totalSpentCents: 1_000,
                subTotalAmountCents: 12_345, subTotalSpentCents: 1_000,
                primaryAmountCents: 0, primarySpentCents: 0, isCollapsed: false
            };

            bindings.add();
            expect(ui.edit.open).toHaveBeenCalledWith(expect.objectContaining({
                budget: expect.objectContaining({ type: BudgetType.Expense, periodType: BudgetPeriodType.Monthly })
            }));
            bindings.addPrimaryBudget(group);
            expect(ui.edit.open).toHaveBeenCalledWith(expect.objectContaining({
                budget: expect.objectContaining({
                    category: 'Food', subCategory: '', amountCents: 12_345,
                    categoryIcon: 'food-icon', categoryColor: '112233'
                }),
                usePrimaryCategoryOnly: true
            }));
            bindings.edit(budget);
            expect(ui.edit.open).toHaveBeenLastCalledWith({ budget, type: BudgetType.Expense });

            ui.confirm.open.mockResolvedValueOnce(false);
            bindings.remove(budget);
            await flushPromises();
            expect(mockBudgetStore.deleteBudget).not.toHaveBeenCalled();

            ui.confirm.open.mockResolvedValueOnce(true);
            bindings.remove(budget);
            await flushPromises();
            expect(mockBudgetStore.deleteBudget).toHaveBeenCalledWith({ budgetId: 'crud-budget' });
            expect(ui.snackbar.showMessage).toHaveBeenCalledWith('Budget deleted successfully');
            expect(bindings.budgetRemoving.value['crud-budget']).toBe(false);

            mockBudgetStore.deleteBudget.mockRejectedValueOnce(new Error('delete failure'));
            ui.confirm.open.mockResolvedValueOnce(true);
            bindings.remove(createBudget({ id: 'delete-fail', name: 'Delete fail' }));
            await flushPromises();
            expect(ui.snackbar.showError).toHaveBeenCalledWith('delete failure');
            mockBudgetStore.deleteBudget.mockRejectedValueOnce({});
            ui.confirm.open.mockResolvedValueOnce(true);
            bindings.remove(createBudget({ id: 'delete-fail-default' }));
            await flushPromises();
            expect(ui.snackbar.showError).toHaveBeenCalledWith('Failed to delete budget');

            bindings.onBudgetSaved();
            mockBudgetStore.exportBudgets.mockResolvedValueOnce({ budgets: [{ amountCents: 12_345 }] });
            await bindings.exportBudgets();
            expect(URL.createObjectURL).toHaveBeenCalledWith(expect.any(Blob));
            expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:budget-export');
            expect(ui.snackbar.showMessage).toHaveBeenCalledWith('Budgets exported successfully');
            mockBudgetStore.exportBudgets.mockRejectedValueOnce(new Error('export failure'));
            await bindings.exportBudgets();
            expect(ui.snackbar.showError).toHaveBeenCalledWith('export failure');
            mockBudgetStore.exportBudgets.mockRejectedValueOnce({});
            await bindings.exportBudgets();
            expect(ui.snackbar.showError).toHaveBeenCalledWith('Failed to export budgets');

            bindings.importBudgets();
            expect(ui.fileInput.click).toHaveBeenCalled();
            await bindings.onFileSelected({ target: { files: [], value: 'empty' } } as unknown as Event);

            ui.confirm.open.mockResolvedValueOnce(false);
            const declinedInput = {
                files: [{ text: async () => JSON.stringify([{ amountCents: 100 }]) }],
                value: 'declined.json'
            };
            await bindings.onFileSelected({ target: declinedInput } as unknown as Event);
            await flushPromises();
            expect(declinedInput.value).toBe('');

            ui.confirm.open.mockResolvedValueOnce(true);
            const acceptedInput = {
                files: [{ text: async () => JSON.stringify([{ amountCents: 12_345 }]) }],
                value: 'accepted.json'
            };
            await bindings.onFileSelected({ target: acceptedInput } as unknown as Event);
            await flushPromises();
            expect(mockBudgetStore.importBudgets).toHaveBeenCalledWith({
                budgets: [{ amountCents: 12_345 }], overwriteExisting: true
            });
            expect(ui.snackbar.showMessage).toHaveBeenCalledWith(expect.stringContaining('Imported'));

            mockBudgetStore.importBudgets.mockRejectedValueOnce(new Error('import failure'));
            ui.confirm.open.mockResolvedValueOnce(true);
            await bindings.onFileSelected({
                target: { files: [{ text: async () => '[]' }], value: 'failure.json' }
            } as unknown as Event);
            await flushPromises();
            expect(ui.snackbar.showError).toHaveBeenCalledWith('import failure');

            mockBudgetStore.importBudgets.mockRejectedValueOnce({});
            ui.confirm.open.mockResolvedValueOnce(true);
            await bindings.onFileSelected({
                target: { files: [{ text: async () => '[]' }], value: 'failure-default.json' }
            } as unknown as Event);
            await flushPromises();
            expect(ui.snackbar.showError).toHaveBeenCalledWith('Failed to import budgets');

            for (const text of ['{}', '{invalid']) {
                const input = { files: [{ text: async () => text }], value: 'invalid.json' };
                await bindings.onFileSelected({ target: input } as unknown as Event);
                expect(input.value).toBe('');
            }
            const readFailureInput = {
                files: [{ text: async () => { throw new Error('read failure'); } }],
                value: 'read-failure.json'
            };
            await bindings.onFileSelected({ target: readFailureInput } as unknown as Event);
            expect(ui.snackbar.showError).toHaveBeenCalledWith('Failed to parse file');
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('runs initial props, stored presets, and reactive lifecycle callbacks', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            localStorage.setItem('budgetFilterPresets', JSON.stringify([{
                id: 'stored', name: 'Stored', categoryFilter: null,
                accountFilter: [], tagFilter: [], executionRateFilter: null,
                spentAmountFilterCents: '', budgetAmountFilterCents: ''
            }]));
            const bindings = setupPage({
                initType: String(BudgetType.Investment),
                initPeriodType: BudgetPeriodType.Quarterly,
                initViewMode: 'forecast'
            });
            createUiHarness(bindings);
            const mounted = mockMountedCallbacks[mockMountedCallbacks.length - 1];
            mounted!();
            await flushPromises();
            expect(bindings.activeBudgetType.value).toBe(BudgetType.Investment);
            expect(bindings.activePeriodFilter.value).toBe('thisQuarter');
            expect(bindings.activeViewMode.value).toBe('forecast');
            expect(bindings.filterPresets.value).toHaveLength(1);

            bindings.filterKeyword.value = 'reactive keyword';
            await flushPromises();
            expect(bindings.searchKeyword.value).toBe('reactive keyword');
            bindings.forecastMonthsHistory.value = 12;
            await flushPromises();
            expect(mockBudgetStore.loadBudgetForecast).toHaveBeenCalled();

            localStorage.setItem('budgetFilterPresets', '{broken');
            bindings.loadPresetsFromStorage();
            expect(bindings.filterPresets.value).toEqual([]);
            localStorage.removeItem('budgetFilterPresets');
            bindings.loadPresetsFromStorage();

            const invalidBindings = setupPage({
                initType: String(BudgetType.Expense),
                initPeriodType: 'invalid-period',
                initViewMode: 'invalid-view'
            });
            createUiHarness(invalidBindings);
            mockMountedCallbacks[mockMountedCallbacks.length - 1]!();
            await flushPromises();
            expect(invalidBindings.activePeriodFilter.value).toBe('thisMonth');
            expect(invalidBindings.activeViewMode.value).toBe('budget');

            const defaultBindings = setupPage();
            createUiHarness(defaultBindings);
            mockMountedCallbacks[mockMountedCallbacks.length - 1]!();
            await flushPromises();
            expect(defaultBindings.activeBudgetType.value).toBe(BudgetType.Expense);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('covers fallback-heavy computed and display branches', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const originalCategories = mockCategoryStore.allTransactionCategories;
        const originalCurrency = mockSettingsStore.appSettings.currency;
        const originalFiscalStart = mockUserStore.currentUserFiscalYearStart;
        try {
            const now = new Date();
            const start = dateOnly(new Date(now.getFullYear(), now.getMonth(), 1));
            const end = dateOnly(new Date(now.getFullYear(), now.getMonth() + 1, 0));
            const lonePrimaryWithSpent = createBudget({
                id: 'single-spent', category: 'Single Spent', name: '',
                categoryId: '', categoryIcon: '', categoryColor: '',
                startDate: start, endDate: end,
                amountCents: 1_000, spentAmountCents: 500, executionRate: 50
            });
            const lonePrimaryWithoutSpent = createBudget({
                id: 'single-zero', category: 'Single Zero', name: '',
                categoryId: '', categoryIcon: '', categoryColor: '',
                startDate: start, endDate: end,
                amountCents: 2_000, spentAmountCents: 0, executionRate: 0
            });
            const childOfSingleZero = createBudget({
                id: 'single-zero-child', category: 'Single Zero', subCategory: 'Child',
                categoryId: '', categoryIcon: '', categoryColor: '',
                startDate: start, endDate: end,
                amountCents: 500, spentAmountCents: 400, executionRate: 80
            });
            const nameFallback = createBudget({
                id: 'name-fallback', category: '', name: 'Name Fallback',
                categoryId: '', categoryIcon: '', categoryColor: '',
                startDate: start, endDate: end
            });
            mockBudgetStore.allBudgets = [
                lonePrimaryWithSpent,
                lonePrimaryWithoutSpent,
                childOfSingleZero,
                nameFallback
            ];
            mockSettingsStore.appSettings.currency = '';
            mockUserStore.currentUserFiscalYearStart = 0;
            mockCategoryStore.allTransactionCategories = {
                3: [
                    {
                        id: 'plain', parentId: '', name: 'Plain', type: 3,
                        icon: '', color: '', subCategories: undefined
                    },
                    {
                        id: 'orphan-parent', parentId: '', name: 'Orphan Parent', type: 3,
                        displayOrder: undefined, subCategories: [{
                            id: 'orphan', parentId: 'missing-parent', name: 'Orphan', type: 3,
                            color: '', displayOrder: undefined, subCategories: []
                        }]
                    }
                ]
            };

            const bindings = setupPage();
            createUiHarness(bindings);
            expect(bindings.defaultCurrency.value).toBe('CNY');
            expect(bindings.fiscalYearStartInfo.value).toMatchObject({ month: 1, day: 1 });
            expect(bindings.visiblePeriodFilters.value).toHaveLength(2);
            bindings.activeViewMode.value = 'forecast';
            expect(bindings.visiblePeriodFilters.value).toHaveLength(3);

            const groups = bindings.groupedBudgets.value;
            const spentGroup = groups.find((group: any) => group.category === 'Single Spent');
            const zeroGroup = groups.find((group: any) => group.category === 'Single Zero');
            const fallbackGroup = groups.find((group: any) => group.category === 'Name Fallback');
            expect(spentGroup).toMatchObject({ totalAmountCents: 1_000, totalSpentCents: 500 });
            expect(zeroGroup).toMatchObject({ totalAmountCents: 2_000, totalSpentCents: 400 });
            expect(fallbackGroup).toMatchObject({ categoryIcon: '', totalAmountCents: 10_000 });
            expect(bindings.getExpandedPrimaryBudgets(spentGroup)).toEqual([]);
            expect(bindings.groupHasExpandedRows({ ...spentGroup, primaryBudgets: [], subBudgets: [] })).toBe(false);
            expect(bindings.getBudgetCategoryIcon(lonePrimaryWithSpent, spentGroup)).toBe('');
            expect(bindings.getBudgetCategoryColor(lonePrimaryWithSpent, spentGroup)).toBe('');

            bindings.sortBy.value = 'category';
            expect(bindings.sortedBudgets.value).toHaveLength(4);
            bindings.activeBudgetType.value = BudgetType.Investment;
            expect(bindings.budgetPrimaryCategories.value).toEqual([]);
            expect(bindings.filteredSummary.value).toMatchObject({ totalBudgetCents: 0, totalExecutionRate: 0 });
            expect(bindings.isAllExpanded.value).toBe(true);
            bindings.activeBudgetType.value = BudgetType.Expense;
            expect(bindings.allCategoriesMap.value.orphan).toBeDefined();
            bindings.categoryFilter.value = 'orphan';
            expect(bindings.getCategoryFilterDisplayName()).toBe('Orphan');

            bindings.currentSpentFilterValue1.value = 111;
            bindings.changeSpentFilter(AmountFilterType.EqualTo.type);
            expect(bindings.getSpentFilterDisplayName()).toContain('CNY 111');
            expect(bindings.getSpentFilterLabel()).toContain('CNY 111');
            bindings.currentBudgetFilterValue1.value = 222;
            bindings.changeBudgetFilter(AmountFilterType.NotEqualTo.type);
            expect(bindings.getBudgetFilterDisplayName()).toContain('CNY 222');
            expect(bindings.getBudgetFilterLabel()).toContain('CNY 222');

            const invalidColorBudget = createBudget({ categoryColor: 'not-a-color' });
            expect(bindings.getBudgetProgressColor(invalidColorBudget)).toBeTruthy();
            const validPrimary = createBudget({ categoryColor: 'abcdef', executionRate: 0 });
            const invalidPrimary = createBudget({ categoryColor: 'not-a-color', executionRate: 0 });
            const baseGroup = {
                category: 'Manual', categoryIcon: '', categoryColor: '',
                primaryBudgets: [validPrimary], subBudgets: [],
                totalAmountCents: 1_000, totalSpentCents: 500,
                subTotalAmountCents: 0, subTotalSpentCents: 0,
                primaryAmountCents: 1_000, primarySpentCents: 0, isCollapsed: false
            };
            expect(bindings.getGroupProgressColor(baseGroup)).toBe('#abcdef');
            expect(bindings.getGroupProgressColor({
                ...baseGroup, categoryColor: 'not-a-color', primaryBudgets: [invalidPrimary]
            })).toBeTruthy();

            expect(bindings.getGroupExecutionRate({ ...baseGroup, primaryBudgets: [createBudget({ executionRate: 75 })] })).toBe(75);
            expect(bindings.getGroupExecutionRate({ ...baseGroup, primaryBudgets: [createBudget({ executionRate: 0 })] })).toBe(50);
            expect(bindings.getGroupExecutionRate({
                ...baseGroup, primaryBudgets: [createBudget({ executionRate: 0 })],
                primaryAmountCents: 0, totalAmountCents: 0
            })).toBe(0);
            expect(bindings.getGroupExecutionRate({
                ...baseGroup, primaryBudgets: [], primaryAmountCents: 0,
                totalAmountCents: 2_000, totalSpentCents: 1_000
            })).toBe(50);
            expect(bindings.getGroupExecutionRate({
                ...baseGroup, primaryBudgets: [], primaryAmountCents: 0,
                totalAmountCents: 0, totalSpentCents: 0
            })).toBe(0);

            bindings.activePeriodFilter.value = 'custom';
            bindings.customStartDate.value = '2024-03-01';
            bindings.customEndDate.value = '2024-03-31';
            expect(bindings.getBudgetDrilldownDateRange(null)).toEqual({
                startDate: '2024-03-01', endDate: '2024-03-31'
            });
            for (const filter of ['thisMonth', 'thisQuarter', 'thisYear']) {
                bindings.activePeriodFilter.value = filter;
                expect(bindings.getBudgetDrilldownDateRange(null)).toEqual(expect.objectContaining({
                    startDate: expect.any(String), endDate: expect.any(String)
                }));
            }
            bindings.activePeriodFilter.value = 'custom';
            bindings.customStartDate.value = '';
            bindings.customEndDate.value = '';
            expect(bindings.getBudgetDrilldownDateRange(null)).toBeNull();

            bindings.activePeriodFilter.value = 'thisMonth';
            bindings.switchViewMode('forecast');
            expect(bindings.activePeriodFilter.value).toBe('thisMonth');
            bindings.activeViewMode.value = 'budget';
            bindings.forecastMonthsHistory.value = 3;
            await flushPromises();

            const savedWindow = globalThis.window;
            Reflect.deleteProperty(globalThis, 'window');
            bindings.scheduleHistoricalChartResize();
            Object.defineProperty(globalThis, 'window', { configurable: true, value: savedWindow, writable: true });
        } finally {
            mockCategoryStore.allTransactionCategories = originalCategories;
            mockSettingsStore.appSettings.currency = originalCurrency;
            mockUserStore.currentUserFiscalYearStart = originalFiscalStart;
            warnSpy.mockRestore();
        }
    });
});
