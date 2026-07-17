import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import {
    collectHostCallbacks,
    type HostNode,
    mountWithHostRenderer,
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as any;
const mockRouter = { navigate: jest.fn() };
const mockShowToast = jest.fn();
const mockShowAmountInHomePage = mockActualVue.ref(true);
const mockDisplayDateRange = mockActualVue.ref(undefined as any);
const mockTransactionOverview = mockActualVue.ref(undefined as any);
const mockGetDisplayIncomeAmount = jest.fn((value: any) => `income:${value.incomeAmountCents}`);
const mockGetDisplayExpenseAmount = jest.fn((value: any) => `expense:${value.expenseAmountCents}`);
const mockLoadAllAccounts = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadAllCategories = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadAllTemplates = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadTransactionOverview = jest.fn<(...args: any[]) => Promise<void>>();
const mockGetTransactionListPageParams = jest.fn((params: Record<string, unknown>) => (
    new URLSearchParams(Object.entries(params).map(([key, value]) => [key, String(value)])).toString()
));
const mockIsUserLogined = jest.fn(() => true);
const mockIsUserUnlocked = jest.fn(() => true);
const mockIsReceiptRecognitionEnabled = jest.fn(() => false);

const mockAccountsStore = { loadAllAccounts: mockLoadAllAccounts };
const mockCategoriesStore = { loadAllCategories: mockLoadAllCategories };
const mockTemplatesStore = mockActualVue.reactive({
    allVisibleTemplates: {} as Record<number, any[] | undefined>,
    loadAllTemplates: mockLoadAllTemplates,
});
const mockOverviewStore = {
    loadTransactionOverview: mockLoadTransactionOverview,
    getTransactionListPageParams: mockGetTransactionListPageParams,
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast }),
}));
jest.mock('@/views/base/HomePageBase.ts', () => ({
    useHomePageBase: () => ({
        showAmountInHomePage: mockShowAmountInHomePage,
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
jest.mock('@/stores/transactionTemplate.ts', () => ({
    useTransactionTemplatesStore: () => mockTemplatesStore,
}));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockOverviewStore }));
jest.mock('@/core/datetime.ts', () => ({
    DateRange: {
        ThisWeek: { type: 'this-week' },
        ThisMonth: { type: 'this-month' },
        ThisYear: { type: 'this-year' },
    },
}));
jest.mock('@/core/template.ts', () => ({ TemplateType: { Normal: { type: 1 } } }));
jest.mock('@/models/transaction_template.ts', () => ({ TransactionTemplate: class {} }));
jest.mock('@/lib/userstate.ts', () => ({
    isUserLogined: () => mockIsUserLogined(),
    isUserUnlocked: () => mockIsUserUnlocked(),
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isTransactionFromAIImageRecognitionEnabled: () => mockIsReceiptRecognitionEnabled(),
}));

// Keep SFC loading after every hoisted dependency mock is registered.
// eslint-disable-next-line @typescript-eslint/no-require-imports
const HomePage = require('@/views/mobile/HomePage.vue').default as any;

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
        today: createAmount({ incomeAmountCents: 101, expenseAmountCents: 99 }),
        thisWeek: createAmount({ incomeAmountCents: 70_001, expenseAmountCents: 40_001 }),
        thisMonth: createAmount(),
        thisYear: createAmount({ incomeAmountCents: 9_000_001, expenseAmountCents: 7_000_001 }),
        ...overrides,
    };
}

function setupPage(router = mockRouter): any {
    return HomePage.setup({ f7router: router }, {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: jest.fn(),
    });
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
    'f7-page', 'f7-navbar', 'f7-nav-title', 'f7-card', 'f7-card-header', 'f7-link', 'f7-icon',
    'f7-list', 'f7-list-item', 'f7-toolbar', 'f7-popover', 'a-i-image-recognition-sheet',
];

beforeEach(() => {
    jest.clearAllMocks();
    mockShowAmountInHomePage.value = true;
    mockDisplayDateRange.value = {
        today: { displayTime: '07/15/2026' },
        thisWeek: { startTime: '07/13', endTime: '07/19' },
        thisMonth: { displayTime: 'July 2026', startTime: '07/01', endTime: '07/31' },
        thisYear: { displayTime: '2026' },
    };
    mockTransactionOverview.value = createOverview();
    mockTemplatesStore.allVisibleTemplates = {
        1: [{ id: 'template-1', name: 'Lunch template' }],
    };
    mockLoadAllAccounts.mockReset().mockResolvedValue(undefined);
    mockLoadAllCategories.mockReset().mockResolvedValue(undefined);
    mockLoadAllTemplates.mockReset().mockResolvedValue(undefined);
    mockLoadTransactionOverview.mockReset().mockResolvedValue(undefined);
    mockIsUserLogined.mockReturnValue(true);
    mockIsUserUnlocked.mockReturnValue(true);
    mockIsReceiptRecognitionEnabled.mockReturnValue(false);
});

describe('mobile HomePage production-loaded setup and initialization', () => {
    test('projects normal templates and resolves the immediate four-store initialization', async () => {
        const accountLoad = deferred<void>();
        mockLoadAllAccounts.mockReturnValueOnce(accountLoad.promise);

        const bindings = setupPage();
        expect(bindings.loading.value).toBe(true);
        expect(bindings.allTransactionTemplates.value).toEqual([
            { id: 'template-1', name: 'Lunch template' },
        ]);
        expect(mockLoadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockLoadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(mockLoadAllTemplates).toHaveBeenCalledWith({ templateType: 1, force: false });
        expect(mockLoadTransactionOverview).toHaveBeenCalledWith({ force: false });

        accountLoad.resolve(undefined);
        await flush();
        expect(bindings.loading.value).toBe(false);

        mockTemplatesStore.allVisibleTemplates = {};
        expect(bindings.allTransactionTemplates.value).toEqual([]);
    });

    test('does not initialize when logged out or locked', () => {
        mockIsUserLogined.mockReturnValue(false);
        const loggedOut = setupPage();
        expect(mockIsUserUnlocked).not.toHaveBeenCalled();
        expect(mockLoadAllAccounts).not.toHaveBeenCalled();
        expect(loggedOut.loading.value).toBe(true);

        jest.clearAllMocks();
        mockIsUserLogined.mockReturnValue(true);
        mockIsUserUnlocked.mockReturnValue(false);
        const locked = setupPage();
        expect(mockLoadAllAccounts).not.toHaveBeenCalled();
        expect(locked.loading.value).toBe(true);
    });

    test('reports only unprocessed initialization failures and accepts non-Error reasons', async () => {
        mockLoadAllCategories.mockRejectedValueOnce({ processed: false, message: 'category failed' });
        const failed = setupPage();
        await flush();
        expect(failed.loading.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('category failed');

        mockShowToast.mockClear();
        mockLoadAllAccounts.mockRejectedValueOnce({ processed: true, message: 'already handled' });
        setupPage();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalled();

        mockLoadAllTemplates.mockRejectedValueOnce('plain failure');
        setupPage();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('plain failure');
    });
});

describe('mobile HomePage reload, page lifecycle, and popover behavior', () => {
    test('runs background and pull-to-refresh success contracts', async () => {
        const bindings = setupPage();
        await flush();
        mockLoadTransactionOverview.mockClear();

        bindings.reload();
        await flush();
        expect(mockLoadTransactionOverview).toHaveBeenLastCalledWith({ force: false });
        expect(mockShowToast).not.toHaveBeenCalled();

        const done = jest.fn();
        bindings.reload(done);
        await flush();
        expect(mockLoadTransactionOverview).toHaveBeenLastCalledWith({ force: true });
        expect(done).toHaveBeenCalledTimes(1);
        expect(mockShowToast).toHaveBeenCalledWith('Data has been updated');
    });

    test('finishes every failed refresh and suppresses processed failures', async () => {
        const bindings = setupPage();
        await flush();
        mockShowToast.mockClear();

        const done = jest.fn();
        mockLoadTransactionOverview.mockRejectedValueOnce({ processed: false, message: 'refresh failed' });
        bindings.reload(done);
        await flush();
        expect(done).toHaveBeenCalledTimes(1);
        expect(mockShowToast).toHaveBeenCalledWith('refresh failed');

        mockLoadTransactionOverview.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.reload();
        await flush();
        expect(mockShowToast).toHaveBeenCalledTimes(1);

        mockLoadTransactionOverview.mockRejectedValueOnce('plain refresh failure');
        bindings.reload();
        await flush();
        expect(mockShowToast).toHaveBeenLastCalledWith('plain refresh failure');
    });

    test('refreshes after page re-entry only after initialization is complete', async () => {
        const accountLoad = deferred<void>();
        mockLoadAllAccounts.mockReturnValueOnce(accountLoad.promise);
        const bindings = setupPage();
        mockLoadTransactionOverview.mockClear();

        bindings.onPageAfterIn();
        expect(mockLoadTransactionOverview).not.toHaveBeenCalled();

        accountLoad.resolve(undefined);
        await flush();
        bindings.onPageAfterIn();
        await flush();
        expect(mockLoadTransactionOverview).toHaveBeenCalledWith({ force: false });
    });

    test('opens the transaction popover only when OCR or a normal template is available', () => {
        const bindings = setupPage();
        mockTemplatesStore.allVisibleTemplates = {};

        bindings.openTransactionTemplatePopover();
        expect(bindings.showTransactionTemplatePopover.value).toBe(false);

        mockIsReceiptRecognitionEnabled.mockReturnValue(true);
        bindings.openTransactionTemplatePopover();
        expect(bindings.showTransactionTemplatePopover.value).toBe(true);

        bindings.showTransactionTemplatePopover.value = false;
        mockIsReceiptRecognitionEnabled.mockReturnValue(false);
        mockTemplatesStore.allVisibleTemplates = { 1: [{ id: 'template-2', name: 'Rent' }] };
        bindings.openTransactionTemplatePopover();
        expect(bindings.showTransactionTemplatePopover.value).toBe(true);
    });
});

describe('mobile HomePage receipt recognition navigation', () => {
    test.each([
        [12.34, '1234'],
        [0.005, '1'],
        [-123.45, '-12345'],
    ])('converts the yuan amount %s to the exact cents query %s', (amount, expectedCents) => {
        const router = { navigate: jest.fn() };
        const bindings = setupPage(router);

        bindings.onReceiptRecognitionChanged({
            amount,
            tradeTime: null,
            description: '',
        });

        expect(router.navigate).toHaveBeenCalledWith(
            `/transaction/add?sourceAmountCents=${expectedCents}&noTransactionDraft=true`,
        );
    });

    test('maps valid ISO time and encoded description without changing the cents result', () => {
        const router = { navigate: jest.fn() };
        const bindings = setupPage(router);

        bindings.onReceiptRecognitionChanged({
            amount: 123.45,
            tradeTime: '2026-07-15T08:09:10.999Z',
            description: 'Coffee & tea/早饭',
        });

        expect(router.navigate).toHaveBeenCalledWith(
            '/transaction/add?sourceAmountCents=12345&time=1784102950'
            + '&comment=Coffee%20%26%20tea%2F%E6%97%A9%E9%A5%AD&noTransactionDraft=true',
        );
    });

    test.each([null, Number.NaN, Number.POSITIVE_INFINITY, '12.34']) (
        'omits a non-finite or non-numeric amount boundary: %s',
        (amount) => {
            const router = { navigate: jest.fn() };
            const bindings = setupPage(router);

            bindings.onReceiptRecognitionChanged({
                amount,
                tradeTime: 'not-a-date',
                description: null,
            });

            expect(router.navigate).toHaveBeenCalledWith('/transaction/add?noTransactionDraft=true');
        },
    );
});

describe('mobile HomePage production template behavior', () => {
    test('renders overview cents and drives the real refresh, visibility, template, and OCR events', async () => {
        mockIsReceiptRecognitionEnabled.mockReturnValue(true);
        const mounted = mountWithHostRenderer(HomePage, { f7router: mockRouter }, HOST_COMPONENTS);
        try {
            await flush();
            expect(findHostNodes(
                mounted.root,
                node => node.props['data-testid'] === 'mobile.home.page',
            )).toHaveLength(1);
            expect(collectHostText(mounted.root)).toContain('expense:6789');
            expect(collectHostText(mounted.root)).toContain('income:12345');
            expect(collectHostText(mounted.root)).toContain('income:101');
            expect(collectHostText(mounted.root)).toContain('expense:7000001');
            expect(findHostNodes(
                mounted.root,
                node => node.props['title'] === 'Lunch template',
            )).toHaveLength(1);

            const callbacks = collectHostCallbacks(mounted.root);
            const refresh = callbacks.find(callback => callback.name === 'onPtr:refresh');
            const pageAfterIn = callbacks.find(callback => callback.name === 'onPage:afterin');
            const tapHold = callbacks.find(callback => callback.name === 'onTaphold');
            const recognition = callbacks.find(callback => callback.name === 'onRecognition:change');
            const updatePopover = callbacks.find(callback => callback.name === 'onUpdate:opened');
            const updateRecognitionSheet = callbacks.find(callback => callback.name === 'onUpdate:show');
            const amountToggle = findHostNodes(
                mounted.root,
                node => String(node.props['class'] ?? '').includes('margin-inline-start-half')
                    && typeof node.props['onClick'] === 'function',
            )[0];
            const aiItem = findHostNodes(
                mounted.root,
                node => node.props['title'] === 'tt:AI Image Recognition'
                    && typeof node.props['onClick'] === 'function',
            )[0];

            expect(refresh).toBeDefined();
            expect(pageAfterIn).toBeDefined();
            expect(tapHold).toBeDefined();
            expect(recognition).toBeDefined();
            expect(updatePopover).toBeDefined();
            expect(updateRecognitionSheet).toBeDefined();
            expect(amountToggle).toBeDefined();
            expect(aiItem).toBeDefined();

            (amountToggle!.props['onClick'] as () => void)();
            expect(mockShowAmountInHomePage.value).toBe(false);

            tapHold!.callback();
            expect(mounted.state.showTransactionTemplatePopover).toBe(true);
            (aiItem!.props['onClick'] as () => void)();
            expect(mounted.state.showTransactionTemplatePopover).toBe(false);
            expect(mounted.state.showAIReceiptImageRecognitionSheet).toBe(true);
            updatePopover!.callback(true);
            updateRecognitionSheet!.callback(false);
            expect(mounted.state.showTransactionTemplatePopover).toBe(true);
            expect(mounted.state.showAIReceiptImageRecognitionSheet).toBe(false);

            const done = jest.fn();
            refresh!.callback(done);
            pageAfterIn!.callback();
            recognition!.callback({
                amount: 88.88,
                tradeTime: null,
                description: 'Receipt event',
            });
            await flush();

            expect(done).toHaveBeenCalledTimes(1);
            expect(mockLoadTransactionOverview).toHaveBeenCalledWith({ force: true });
            expect(mockLoadTransactionOverview).toHaveBeenCalledWith({ force: false });
            expect(mockRouter.navigate).toHaveBeenCalledWith(
                '/transaction/add?sourceAmountCents=8888&comment=Receipt%20event&noTransactionDraft=true',
            );
            expect(mockGetTransactionListPageParams.mock.calls.map(call => call[0])).toEqual(
                expect.arrayContaining([
                    { dateType: 'this-week' },
                    { dateType: 'this-month' },
                    { dateType: 'this-year' },
                ]),
            );
        } finally {
            mounted.app.unmount();
        }
    });

    test('renders skeleton and every empty or invalid overview projection branch', async () => {
        const accountLoad = deferred<void>();
        mockLoadAllAccounts.mockReturnValueOnce(accountLoad.promise);
        const mounted = mountWithHostRenderer(HomePage, { f7router: mockRouter }, HOST_COMPONENTS);
        try {
            expect(String(findHostNodes(
                mounted.root,
                node => String(node.props['class'] ?? '').includes('home-summary-card'),
            )[0]?.props['class'])).toContain('skeleton-text');
            expect(collectHostText(mounted.root)).toContain('Monthly income 0.00 USD');

            mockDisplayDateRange.value = undefined;
            mockTransactionOverview.value = {
                today: createAmount({ valid: false }),
                thisWeek: null,
                thisMonth: null,
                thisYear: createAmount({ valid: false }),
            };
            mockTemplatesStore.allVisibleTemplates = {};
            accountLoad.resolve(undefined);
            await flush();

            expect(collectHostText(mounted.root)).toContain('-');
            expect(collectHostText(mounted.root)).toContain('tt:Today');
            expect(collectHostText(mounted.root)).toContain('tt:This Week');
            expect(collectHostText(mounted.root)).toContain('tt:This Month');
            expect(collectHostText(mounted.root)).toContain('tt:This Year');
            expect(findHostNodes(
                mounted.root,
                node => node.props['title'] === 'tt:AI Image Recognition',
            )).toHaveLength(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
