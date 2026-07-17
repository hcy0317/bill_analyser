import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import {
    collectHostCallbacks,
    type HostNode,
    mountWithHostRenderer,
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as any;
const mockMountedCallbacks: Array<() => void> = [];
const mockTemplateRefs = new Map<string, any>();
const mockRouter = { push: jest.fn() };
const mockThemeName = mockActualVue.ref('light');
const mockShowAmountInHomePage = mockActualVue.ref(true);
const mockAllAccounts = mockActualVue.ref([] as any[] | null);
const mockDisplayDateRange = mockActualVue.ref(undefined as any);
const mockTransactionOverview = mockActualVue.ref(undefined as any);
const mockFormatNumber = jest.fn((value: number) => `count:${value}`);
const mockGetDisplayIncomeAmount = jest.fn((value: any) => `income:${value.incomeAmountCents}`);
const mockGetDisplayExpenseAmount = jest.fn((value: any) => `expense:${value.expenseAmountCents}`);
const mockLoadAllAccounts = jest.fn<(...args: any[]) => Promise<void>>();
const mockSyncAllAccountBalances = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadAllCategories = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadTransactionOverview = jest.fn<(...args: any[]) => Promise<void>>();
const mockGetTransactionListPageParams = jest.fn((params: Record<string, unknown>) => (
    new URLSearchParams(Object.entries(params).map(([key, value]) => [key, String(value)])).toString()
));
const mockIsUserLogined = jest.fn(() => true);
const mockIsUserUnlocked = jest.fn(() => true);
const mockGetUnixTimeAfterUnixTime = jest.fn<(...args: any[]) => number>(() => 2_000_000);
const mockGetUnixTimeBeforeUnixTime = jest.fn<(...args: any[]) => number>(() => 1_999_999);
const mockIsDarkApplicationTheme = jest.fn((name: string) => name === 'dark');
const mockLogger = { error: jest.fn() };

const mockAccountsStore = {
    loadAllAccounts: mockLoadAllAccounts,
    syncAllAccountBalances: mockSyncAllAccountBalances,
};
const mockCategoriesStore = { loadAllCategories: mockLoadAllCategories };
const mockOverviewStore = mockActualVue.reactive({
    transactionDataRange: {} as Record<string, { startTime: number } | undefined>,
    loadTransactionOverview: mockLoadTransactionOverview,
    getTransactionListPageParams: mockGetTransactionListPageParams,
});

function mockCreateImportedStub(name: string): any {
    return mockActualVue.defineComponent({
        name,
        inheritAttrs: false,
        methods: {
            showMessage() {
                return undefined;
            },
            showError() {
                return undefined;
            },
        },
        setup: (_props: unknown, { attrs, slots }: any) => () => mockActualVue.h(
            'stub',
            attrs,
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            }),
        ),
    });
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => void) => mockMountedCallbacks.push(callback),
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
    };
});
jest.mock('vue-router', () => ({ useRouter: () => mockRouter }));
jest.mock('vuetify', () => ({
    useTheme: () => ({ global: { name: mockThemeName } }),
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: mockCreateImportedStub('DesktopHomeSnackBarStub'),
}));
jest.mock('@/views/desktop/overview/cards/IncomeExpenseOverviewCard.vue', () => ({
    __esModule: true,
    default: mockCreateImportedStub('IncomeExpenseOverviewCardStub'),
}));
jest.mock('@/views/desktop/overview/cards/MonthlyIncomeAndExpenseCard.vue', () => ({
    __esModule: true,
    default: mockCreateImportedStub('MonthlyIncomeAndExpenseCardStub'),
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => (
            values ? `tt:${key}:${JSON.stringify(values)}` : `tt:${key}`
        ),
        getCurrentNumeralSystemType: () => ({
            digitZero: 'zero',
            formatNumber: mockFormatNumber,
        }),
    }),
}));
jest.mock('@/views/base/HomePageBase.ts', () => ({
    useHomePageBase: () => ({
        showAmountInHomePage: mockShowAmountInHomePage,
        allAccounts: mockAllAccounts,
        netAssets: mockActualVue.computed(() => 'net:300'),
        totalAssets: mockActualVue.computed(() => 'assets:500'),
        totalLiabilities: mockActualVue.computed(() => 'liabilities:200'),
        displayDateRange: mockDisplayDateRange,
        transactionOverview: mockTransactionOverview,
        getDisplayIncomeAmount: mockGetDisplayIncomeAmount,
        getDisplayExpenseAmount: mockGetDisplayExpenseAmount,
    }),
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoriesStore,
}));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockOverviewStore }));
jest.mock('@/core/datetime.ts', () => ({
    DateRange: {
        ThisMonth: { type: 'this-month' },
        ThisWeek: { type: 'this-week' },
        ThisYear: { type: 'this-year' },
        Custom: { type: 'custom' },
    },
}));
jest.mock('@/core/theme.ts', () => ({
    isDarkApplicationTheme: (name: string) => mockIsDarkApplicationTheme(name),
}));
jest.mock('@/models/transaction.ts', () => ({
    LATEST_12MONTHS_TRANSACTION_AMOUNTS_REQUEST_TYPES: ['monthA', 'monthB', 'monthC'],
}));
jest.mock('@/lib/datetime.ts', () => ({
    getUnixTimeAfterUnixTime: (...args: unknown[]) => mockGetUnixTimeAfterUnixTime(...args),
    getUnixTimeBeforeUnixTime: (...args: unknown[]) => mockGetUnixTimeBeforeUnixTime(...args),
}));
jest.mock('@/lib/userstate.ts', () => ({
    isUserLogined: () => mockIsUserLogined(),
    isUserUnlocked: () => mockIsUserUnlocked(),
}));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

// Keep SFC loading after every hoisted dependency mock is registered.
// eslint-disable-next-line @typescript-eslint/no-require-imports
const HomePage = require('@/views/desktop/HomePage.vue').default as any;

function createAmount(overrides: Record<string, unknown> = {}): any {
    return {
        valid: true,
        incomeAmountCents: 12_345,
        expenseAmountCents: 6_789,
        incompleteIncomeAmount: false,
        incompleteExpenseAmount: false,
        ...overrides,
    };
}

function createOverview(overrides: Record<string, unknown> = {}): any {
    return {
        today: createAmount({ incomeAmountCents: 100, expenseAmountCents: 50 }),
        thisWeek: createAmount({ incomeAmountCents: 700, expenseAmountCents: 400 }),
        thisMonth: createAmount(),
        thisYear: createAmount({ incomeAmountCents: 90_000, expenseAmountCents: 70_000 }),
        monthA: createAmount({ incomeAmountCents: 1_000, expenseAmountCents: 500 }),
        monthB: undefined,
        ...overrides,
    };
}

function setupPage(): any {
    return HomePage.setup({}, {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: jest.fn(),
    });
}

function installSnackbar(bindings: any): { showMessage: jest.Mock; showError: jest.Mock } {
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    bindings.snackbar.value = snackbar;
    return snackbar;
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
    await new Promise(resolve => setImmediate(resolve));
}

function deferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (reason: unknown) => void;
} {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

function findHostNodes(node: HostNode, predicate: (candidate: HostNode) => boolean): HostNode[] {
    const matches: HostNode[] = [];
    const visit = (candidate: HostNode): void => {
        if (predicate(candidate)) matches.push(candidate);
        for (const child of candidate.children) visit(child);
    };
    visit(node);
    return matches;
}

function collectHostText(node: HostNode): string {
    return [node.text, ...node.children.map(collectHostText)].join(' ');
}

const HOST_COMPONENTS = [
    'v-row', 'v-col', 'v-card', 'v-card-text', 'v-btn', 'v-progress-circular', 'v-icon',
    'v-tooltip', 'v-skeleton-loader', 'v-img', 'v-avatar', 'v-list-item', 'v-list-item-title',
];

beforeEach(() => {
    jest.clearAllMocks();
    mockMountedCallbacks.length = 0;
    mockTemplateRefs.clear();
    mockThemeName.value = 'light';
    mockShowAmountInHomePage.value = true;
    mockAllAccounts.value = [{ id: 'account-1' }, { id: 'account-2' }];
    mockDisplayDateRange.value = {
        today: { displayTime: 'today' },
        thisWeek: { startTime: 'week-start', endTime: 'week-end' },
        thisMonth: { startTime: 'month-start', endTime: 'month-end', displayTime: 'July' },
        thisYear: { displayTime: '2026' },
    };
    mockTransactionOverview.value = createOverview();
    mockOverviewStore.transactionDataRange = {
        monthA: { startTime: 100 },
        monthB: { startTime: 200 },
        monthC: undefined,
    };
    mockLoadAllAccounts.mockResolvedValue(undefined);
    mockSyncAllAccountBalances.mockResolvedValue(undefined);
    mockLoadAllCategories.mockResolvedValue(undefined);
    mockLoadTransactionOverview.mockResolvedValue(undefined);
    mockIsUserLogined.mockReturnValue(true);
    mockIsUserUnlocked.mockReturnValue(true);
    mockGetUnixTimeAfterUnixTime.mockReturnValue(2_000_000);
    mockGetUnixTimeBeforeUnixTime.mockReturnValue(1_999_999);
});

describe('desktop HomePage production-loaded setup and projections', () => {
    test('derives theme, account count, and complete monthly chart data without changing cents', () => {
        const bindings = setupPage();

        expect(bindings.isDarkMode.value).toBe(false);
        mockThemeName.value = 'dark';
        expect(bindings.isDarkMode.value).toBe(true);
        expect(bindings.displayAccountCount.value).toBe('count:2');
        expect(mockFormatNumber).toHaveBeenCalledWith(2);

        expect(bindings.monthlyIncomeAndExpenseData.value).toEqual([
            {
                monthStartTime: 100,
                incomeAmountCents: 1_000,
                expenseAmountCents: 500,
                incompleteIncomeAmount: false,
                incompleteExpenseAmount: false,
            },
            {
                monthStartTime: 200,
                incomeAmountCents: 0,
                expenseAmountCents: 0,
                incompleteIncomeAmount: true,
                incompleteExpenseAmount: true,
            },
        ]);

        mockAllAccounts.value = null;
        expect(bindings.displayAccountCount.value).toBe('zero');
    });

    test('returns no chart data for every invalid overview boundary and skips missing ranges', () => {
        const bindings = setupPage();

        for (const overview of [
            null,
            {},
            { thisMonth: null },
            { thisMonth: { valid: false } },
        ]) {
            mockTransactionOverview.value = overview;
            expect(bindings.monthlyIncomeAndExpenseData.value).toEqual([]);
        }

        mockTransactionOverview.value = createOverview({
            monthA: createAmount({
                incomeAmountCents: 0,
                expenseAmountCents: 0,
                incompleteIncomeAmount: true,
                incompleteExpenseAmount: true,
            }),
        });
        mockOverviewStore.transactionDataRange.monthA = undefined;
        expect(bindings.monthlyIncomeAndExpenseData.value).toEqual([
            {
                monthStartTime: 200,
                incomeAmountCents: 0,
                expenseAmountCents: 0,
                incompleteIncomeAmount: true,
                incompleteExpenseAmount: true,
            },
        ]);
    });

    test('routes a clicked month through the custom range contract', () => {
        const bindings = setupPage();

        bindings.clickMonthlyIncomeOrExpense({
            monthStartTime: 1_000_000,
            transactionType: 'expense',
        });

        expect(mockGetUnixTimeAfterUnixTime).toHaveBeenCalledWith(1_000_000, 1, 'months');
        expect(mockGetUnixTimeBeforeUnixTime).toHaveBeenCalledWith(2_000_000, 1, 'seconds');
        expect(mockGetTransactionListPageParams).toHaveBeenCalledWith({
            type: 'expense',
            dateType: 'custom',
            minTime: 1_000_000,
            maxTime: 1_999_999,
        });
        expect(mockRouter.push).toHaveBeenCalledWith(
            '/transaction/list?type=expense&dateType=custom&minTime=1000000&maxTime=1999999',
        );
    });
});

describe('desktop HomePage production-loaded lifecycle and reload flow', () => {
    test('loads only for a logged-in and unlocked mounted user', async () => {
        const first = setupPage();
        expect(mockMountedCallbacks).toHaveLength(1);
        mockMountedCallbacks[0]!();
        await flush();
        expect(mockLoadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockLoadTransactionOverview).toHaveBeenCalledWith({ force: false, loadLast11Months: true });
        expect(first.loadingOverview.value).toBe(false);

        mockLoadAllAccounts.mockClear();
        mockLoadTransactionOverview.mockClear();
        mockIsUserUnlocked.mockClear();
        mockMountedCallbacks.length = 0;
        mockIsUserLogined.mockReturnValue(false);
        setupPage();
        mockMountedCallbacks[0]!();
        expect(mockIsUserUnlocked).not.toHaveBeenCalled();
        expect(mockLoadAllAccounts).not.toHaveBeenCalled();

        mockMountedCallbacks.length = 0;
        mockIsUserLogined.mockReturnValue(true);
        mockIsUserUnlocked.mockReturnValue(false);
        setupPage();
        mockMountedCallbacks[0]!();
        expect(mockLoadAllAccounts).not.toHaveBeenCalled();
    });

    test('runs normal and forced success flows, including background category failure ownership', async () => {
        mockLoadAllCategories.mockRejectedValueOnce(new Error('category background failed'));
        const bindings = setupPage();
        const snackbar = installSnackbar(bindings);

        bindings.reload(false);
        await flush();
        expect(bindings.loadingOverview.value).toBe(false);
        expect(mockSyncAllAccountBalances).not.toHaveBeenCalled();
        expect(mockLoadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(mockLogger.error).toHaveBeenCalledWith(
            'failed to load category list in background',
            expect.objectContaining({ message: 'category background failed' }),
        );
        expect(snackbar.showMessage).not.toHaveBeenCalled();

        const sync = deferred<void>();
        mockSyncAllAccountBalances.mockReturnValueOnce(sync.promise);
        mockLoadAllAccounts.mockClear();
        bindings.reload(true);
        expect(bindings.loadingOverview.value).toBe(true);
        expect(mockSyncAllAccountBalances).toHaveBeenCalledWith({ refreshAccounts: false });
        expect(mockLoadAllAccounts).not.toHaveBeenCalled();

        sync.resolve(undefined);
        await flush();
        expect(mockLoadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockLoadTransactionOverview).toHaveBeenLastCalledWith({ force: true, loadLast11Months: true });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Data has been updated');
    });

    test('reports only current unprocessed failures and suppresses processed failures', async () => {
        const bindings = setupPage();
        const snackbar = installSnackbar(bindings);

        mockLoadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'load failed' });
        bindings.reload(false);
        await flush();
        expect(bindings.loadingOverview.value).toBe(false);
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'load failed' }));

        mockLoadAllAccounts.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.reload(false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);

        mockSyncAllAccountBalances.mockRejectedValueOnce({ processed: false, message: 'sync failed' });
        bindings.reload(true);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'sync failed' }));
        expect(bindings.loadingOverview.value).toBe(false);
    });

    test('lets only the newest overlapping request mutate loading and success state', async () => {
        const first = deferred<void>();
        const second = deferred<void>();
        mockLoadAllAccounts.mockReset()
            .mockReturnValueOnce(first.promise)
            .mockReturnValueOnce(second.promise);
        const bindings = setupPage();
        const snackbar = installSnackbar(bindings);

        bindings.reload(false);
        bindings.reload(true);
        second.resolve(undefined);
        await flush();
        expect(bindings.loadingOverview.value).toBe(false);
        expect(mockLoadAllCategories).toHaveBeenCalledTimes(1);
        expect(snackbar.showMessage).toHaveBeenCalledTimes(1);

        first.resolve(undefined);
        await flush();
        expect(mockLoadAllCategories).toHaveBeenCalledTimes(1);
        expect(snackbar.showMessage).toHaveBeenCalledTimes(1);
    });

    test('ignores a stale rejection after a newer request succeeds', async () => {
        const first = deferred<void>();
        const second = deferred<void>();
        mockLoadAllAccounts.mockReset()
            .mockReturnValueOnce(first.promise)
            .mockReturnValueOnce(second.promise);
        const bindings = setupPage();
        const snackbar = installSnackbar(bindings);

        bindings.reload(false);
        bindings.reload(false);
        second.resolve(undefined);
        await flush();
        first.reject({ processed: false, message: 'stale failure' });
        await flush();

        expect(bindings.loadingOverview.value).toBe(false);
        expect(snackbar.showError).not.toHaveBeenCalled();
        expect(mockLoadAllCategories).toHaveBeenCalledTimes(1);
    });
});

describe('desktop HomePage production template behavior', () => {
    test('mounts real loaded template state and keeps refresh, visibility, and chart actions callable', async () => {
        const mounted = mountWithHostRenderer(HomePage, {}, HOST_COMPONENTS);
        try {
            mounted.state.loadingOverview = false;
            await mockActualVue.nextTick();

            const page = findHostNodes(
                mounted.root,
                node => node.props['data-testid'] === 'desktop.home.page',
            );
            expect(page).toHaveLength(1);
            expect(collectHostText(mounted.root)).toContain('expense:6789');
            expect(collectHostText(mounted.root)).toContain('income:12345');

            const refresh = findHostNodes(
                mounted.root,
                node => String(node.props['class'] ?? '').includes('ms-2')
                    && typeof node.props['onClick'] === 'function',
            )[0];
            const visibility = findHostNodes(
                mounted.root,
                node => String(node.props['class'] ?? '').includes('ms-1')
                    && typeof node.props['onClick'] === 'function',
            )[0];
            const chart = findHostNodes(
                mounted.root,
                node => (node.props['enableClickItem'] === true || node.props['enable-click-item'] === true)
                    && typeof node.props['onClick'] === 'function',
            )[0];

            expect(refresh).toBeDefined();
            expect(visibility).toBeDefined();
            expect(chart).toBeDefined();
            (visibility!.props['onClick'] as () => void)();
            expect(mockShowAmountInHomePage.value).toBe(false);
            (refresh!.props['onClick'] as () => void)();
            (chart!.props['onClick'] as (event: unknown) => void)({
                monthStartTime: 1_000_000,
                transactionType: 'income',
            });
            await flush();
            expect(mockSyncAllAccountBalances).toHaveBeenCalledWith({ refreshAccounts: false });
            expect(mockRouter.push).toHaveBeenCalledWith(expect.stringContaining('type=income'));
        } finally {
            mounted.app.unmount();
        }
    });

    test('renders skeleton, missing-value, empty-account, and optional-date branches', async () => {
        mockAllAccounts.value = [];
        mockDisplayDateRange.value = undefined;
        mockTransactionOverview.value = {
            today: null,
            thisWeek: null,
            thisMonth: null,
            thisYear: null,
        };
        const mounted = mountWithHostRenderer(HomePage, {}, HOST_COMPONENTS);
        try {
            expect(findHostNodes(
                mounted.root,
                node => node.props['type'] === 'text' && node.props['loading'] === true,
            ).length).toBeGreaterThanOrEqual(4);
            expect(mounted.state.monthlyIncomeAndExpenseData).toEqual([]);

            mounted.state.loadingOverview = false;
            await mockActualVue.nextTick();
            expect(collectHostText(mounted.root)).toContain('-');
            expect(collectHostText(mounted.root)).toContain('tt:format.misc.youHaveAccounts');
            expect(collectHostCallbacks(mounted.root).length).toBeGreaterThanOrEqual(2);
        } finally {
            mounted.app.unmount();
        }
    });
});
