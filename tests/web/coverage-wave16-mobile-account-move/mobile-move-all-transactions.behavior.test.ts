import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockRouteBackOnError = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockFindAccountNameById = jest.fn<(...args: any[]) => string>();

const mockAccountsStore = {
    allAccountsMap: {} as Record<string, any>,
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>()
};
const mockTransactionsStore = {
    moveAllTransactionsBetweenAccounts: jest.fn<(...args: any[]) => Promise<void>>()
};

let mockLastBase: ReturnType<typeof createBase>;

function createBase(): any {
    const { computed, ref } = actualVue;
    const moving = ref(false);
    const fromAccount = ref(undefined as any);
    const toAccountId = ref('');
    const toAccountName = ref('');
    const allAccounts = ref([
        { id: 'source', name: 'Source' },
        { id: 'target', name: 'Target' }
    ]);
    const allVisibleAccounts = ref([...allAccounts.value]);
    const allVisibleCategorizedAccounts = ref([{ category: 'cash', accounts: allVisibleAccounts.value }]);
    const displayToAccountName = computed(() => (
        toAccountId.value ? mockFindAccountNameById(allAccounts.value, toAccountId.value, 'Target Account') : 'Target Account'
    ));
    const isToAccountNameValid = computed(() => (
        !!toAccountId.value
        && !!toAccountName.value
        && mockFindAccountNameById(allAccounts.value, toAccountId.value) === toAccountName.value
    ));

    return {
        moving,
        fromAccount,
        toAccountId,
        toAccountName,
        allAccounts,
        allVisibleAccounts,
        allVisibleCategorizedAccounts,
        displayToAccountName,
        isToAccountNameValid
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => key })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showToast: (...args: any[]) => mockShowToast(...args),
        routeBackOnError: (...args: any[]) => mockRouteBackOnError(...args)
    }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args)
}));
jest.mock('@/views/base/accounts/MoveAllTransactionsPageBase.ts', () => ({
    useMoveAllTransactionsPageBase: () => {
        mockLastBase = createBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/models/account.ts', () => ({
    Account: {
        findAccountNameById: (...args: any[]) => mockFindAccountNameById(...args)
    }
}));

import MoveAllTransactionsPage from '@/views/mobile/accounts/MoveAllTransactionsPage.vue';

function setup(query: Record<string, string> = { fromAccountId: 'source' }): {
    readonly bindings: any;
    readonly router: any;
} {
    const router = { back: jest.fn(), navigate: jest.fn() };
    const bindings = (MoveAllTransactionsPage as any).setup(
        { f7route: { query }, f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() }
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function makeValid(bindings: any): void {
    bindings.fromAccount.value = { id: 'source', name: 'Source' };
    bindings.toAccountId.value = 'target';
    bindings.toAccountName.value = 'Target';
    bindings.password.value = 'current-password';
}

function render(bindings: any): unknown {
    const exposed = actualVue.proxyRefs(bindings);
    return (MoveAllTransactionsPage as any).render(exposed, [], {}, exposed, {}, {});
}

function collectCallbacks(
    value: any,
    callbacks: Array<{ readonly name: string; readonly callback: (...args: any[]) => unknown }>,
    seen = new Set<any>()
): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) return;
    seen.add(value);

    if (Array.isArray(value)) {
        for (const item of value) collectCallbacks(item, callbacks, seen);
        return;
    }

    if (value.props && typeof value.props === 'object') {
        for (const [name, callback] of Object.entries(value.props)) {
            if (name.startsWith('on') && typeof callback === 'function') {
                callbacks.push({ name, callback: callback as (...args: any[]) => unknown });
            }
        }
    }

    if (value.children && typeof value.children === 'object' && !Array.isArray(value.children)) {
        for (const slot of Object.values(value.children)) {
            collectCallbacks(typeof slot === 'function' ? (slot as () => unknown)() : slot, callbacks, seen);
        }
    } else {
        collectCallbacks(value.children, callbacks, seen);
    }
    collectCallbacks(value.dynamicChildren, callbacks, seen);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockAccountsStore.allAccountsMap = {
        source: { id: 'source', name: 'Source' },
        target: { id: 'target', name: 'Target' }
    };
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockTransactionsStore.moveAllTransactionsBetweenAccounts.mockResolvedValue(undefined);
    mockFindAccountNameById.mockImplementation((_accounts: any[], id: string, fallback = '') => (
        id === 'source' ? 'Source' : id === 'target' ? 'Target' : fallback
    ));
    mockShowLoading.mockImplementation((predicate?: () => unknown) => predicate?.());
});

describe('mobile move-all-transactions initialization', () => {
    test('rejects a missing source-account parameter and routes the captured error on entry', () => {
        const { bindings, router } = setup({});

        expect(mockAccountsStore.loadAllAccounts).not.toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('Parameter Invalid');
        expect(bindings.loadingError.value).toBe('Parameter Invalid');
        bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });

    test('loads and selects the requested source account', async () => {
        const { bindings } = setup();
        expect(bindings.loading.value).toBe(true);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });

        await flush();

        expect(bindings.loading.value).toBe(false);
        expect(bindings.fromAccount.value).toEqual({ id: 'source', name: 'Source' });
        expect(bindings.displayToAccountName.value).toBe('Target Account');
        bindings.toAccountId.value = 'target';
        expect(bindings.displayToAccountName.value).toBe('Target');
    });

    test('rejects an unknown source account after loading', async () => {
        const { bindings } = setup({ fromAccountId: 'missing' });
        await flush();

        expect(bindings.loading.value).toBe(false);
        expect(bindings.fromAccount.value).toBeUndefined();
        expect(bindings.loadingError.value).toBe('Parameter Invalid');
        expect(mockShowToast).toHaveBeenCalledWith('Parameter Invalid');
    });

    test('distinguishes processed, readable, and raw account-loading failures', async () => {
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({
            processed: true,
            message: 'handled account error'
        });
        const processed = setup();
        await flush();
        expect(processed.bindings.loading.value).toBe(false);
        expect(processed.bindings.loadingError.value).toBeNull();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled account error');

        const readableError = { processed: false, message: 'account load failed' };
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce(readableError);
        const readable = setup();
        await flush();
        expect(readable.bindings.loadingError.value).toEqual(readableError);
        expect(mockShowToast).toHaveBeenCalledWith('account load failed');

        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce('raw account load failure');
        const raw = setup();
        await flush();
        expect(raw.bindings.loadingError.value).toBe('raw account load failure');
        expect(mockShowToast).toHaveBeenCalledWith('raw account load failure');
    });
});

describe('mobile move-all-transactions confirmation', () => {
    test('guards every incomplete or unsafe confirmation state', async () => {
        const { bindings } = setup();
        await flush();

        const invalidStates = [
            () => {
                makeValid(bindings);
                bindings.fromAccount.value = undefined;
            },
            () => {
                makeValid(bindings);
                bindings.toAccountId.value = '';
            },
            () => {
                makeValid(bindings);
                bindings.toAccountId.value = 'source';
            },
            () => {
                makeValid(bindings);
                bindings.toAccountName.value = '';
            },
            () => {
                makeValid(bindings);
                bindings.toAccountName.value = 'Wrong Target';
            },
            () => {
                makeValid(bindings);
                bindings.password.value = '';
            }
        ];

        for (const configure of invalidStates) {
            configure();
            bindings.confirm();
        }

        expect(mockTransactionsStore.moveAllTransactionsBetweenAccounts).not.toHaveBeenCalled();
        expect(mockShowLoading).not.toHaveBeenCalled();
    });

    test('moves all transactions, releases loading state, confirms success, and navigates back', async () => {
        const { bindings, router } = setup();
        await flush();
        makeValid(bindings);

        bindings.confirm();

        expect(bindings.moving.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(expect.any(Function));
        expect(mockTransactionsStore.moveAllTransactionsBetweenAccounts).toHaveBeenCalledWith({
            fromAccountId: 'source',
            toAccountId: 'target',
            password: 'current-password'
        });
        await flush();

        expect(bindings.moving.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalledTimes(1);
        expect(mockShowToast).toHaveBeenCalledWith('All transactions in this account has been moved.');
        expect(router.back).toHaveBeenCalledTimes(1);
    });

    test('always releases loading state and reports only unprocessed failures', async () => {
        const { bindings, router } = setup();
        await flush();
        makeValid(bindings);

        mockTransactionsStore.moveAllTransactionsBetweenAccounts.mockRejectedValueOnce({
            processed: true,
            message: 'handled move failure'
        });
        bindings.confirm();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled move failure');

        mockTransactionsStore.moveAllTransactionsBetweenAccounts.mockRejectedValueOnce({
            processed: false,
            message: 'move failed'
        });
        bindings.confirm();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('move failed');

        mockTransactionsStore.moveAllTransactionsBetweenAccounts.mockRejectedValueOnce('raw move failure');
        bindings.confirm();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw move failure');
        expect(bindings.moving.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalledTimes(3);
        expect(router.back).not.toHaveBeenCalled();
    });
});

describe('mobile move-all-transactions production template', () => {
    test('renders loading and editable states and executes Framework7 bindings', async () => {
        const { bindings } = setup();
        const loadingTree = render(bindings);
        expect(loadingTree).toBeTruthy();
        await flush();

        makeValid(bindings);
        bindings.loading.value = false;
        bindings.showAccountSheet.value = true;
        const editableTree = render(bindings);
        const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
        collectCallbacks(editableTree, callbacks);
        expect(callbacks.map(item => item.name)).toEqual(expect.arrayContaining([
            'onPage:afterin',
            'onClick',
            'onUpdate:show',
            'onUpdate:modelValue'
        ]));

        for (const { name, callback } of callbacks) {
            if (name === 'onUpdate:show') callback(false);
            else if (name === 'onUpdate:modelValue') callback('target');
            else if (name === 'onClick') callback();
            else callback();
            await flush(1);
        }

        mockLastBase.allVisibleAccounts.value = [];
        bindings.moving.value = true;
        expect(render(bindings)).toBeTruthy();
    });
});
