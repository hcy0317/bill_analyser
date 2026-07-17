import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import {
    collectHostCallbacks,
    mountWithHostRenderer
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as typeof import('@/../node_modules/vue');
const mockTemplateRefs = new Map<string, ReturnType<typeof mockActualVue.ref>>();
const mockShowError = jest.fn();
const mockLoadAllAccounts = jest.fn<() => Promise<void>>();
const mockMoveAllTransactions = jest.fn<() => Promise<void>>();
const mockFindAccountNameById = jest.fn((..._args: unknown[]) => 'Selected account');
let mockLastBase: ReturnType<typeof createBase>;

function createBase() {
    const fromAccount = mockActualVue.ref<{ id: string; name: string } | null>(null);
    const toAccountId = mockActualVue.ref('');
    const toAccountName = mockActualVue.ref('');
    const moving = mockActualVue.ref(false);
    return {
        moving,
        fromAccount,
        toAccountId,
        toAccountName,
        allAccounts: mockActualVue.ref([{ id: 'source', name: 'Source' }, { id: 'target', name: 'Target' }]),
        allVisibleAccounts: mockActualVue.ref([{ id: 'target', name: 'Target' }]),
        allVisibleCategorizedAccounts: mockActualVue.ref([
            { category: 'cash', accounts: [{ id: 'target', name: 'Target' }] }
        ]),
        displayToAccountName: mockActualVue.computed(() => (
            toAccountId.value === 'target' ? 'Target' : 'Unspecified'
        )),
        isToAccountNameValid: mockActualVue.computed(() => toAccountName.value === 'Target')
    };
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as typeof import('@/../node_modules/vue');
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        }
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, params?: Record<string, unknown>) => (
            params ? `${key}:${JSON.stringify(params)}` : `tt:${key}`
        )
    })
}));
jest.mock('@/views/base/accounts/MoveAllTransactionsPageBase.ts', () => ({
    useMoveAllTransactionsPageBase: () => {
        mockLastBase = createBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({ loadAllAccounts: () => mockLoadAllAccounts() })
}));
jest.mock('@/stores/transaction.ts', () => ({
    useTransactionsStore: () => ({
        moveAllTransactionsBetweenAccounts: () => mockMoveAllTransactions()
    })
}));
jest.mock('@/models/account.ts', () => ({
    Account: {
        findAccountNameById: (...args: unknown[]) => mockFindAccountNameById(...args)
    }
}));

const mockSnackBarComponent = {
    __esModule: true,
    default: mockActualVue.defineComponent({
        name: 'MoveAllTransactionsSnackBar',
        setup: (_props, { expose }) => {
            expose({ showError: mockShowError });
            return () => mockActualVue.h('move-all-transactions-snackbar-stub');
        }
    })
};
jest.mock('@/components/desktop/SnackBar.vue', () => mockSnackBarComponent);

import MoveAllTransactionsDialogModule from '@/views/desktop/accounts/list/dialogs/MoveAllTransactionsDialog.vue';

const MoveAllTransactionsDialog = MoveAllTransactionsDialogModule as unknown as {
    setup: (props: object, context: object) => Record<string, unknown>;
};

function setup() {
    const exposed: Record<string, unknown> = {};
    const bindings = MoveAllTransactionsDialog.setup({}, {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value)
    }) as any;
    bindings.snackbar.value = { showError: mockShowError };
    return { bindings, exposed };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    mockLoadAllAccounts.mockResolvedValue(undefined);
    mockMoveAllTransactions.mockResolvedValue(undefined);
});

describe('desktop MoveAllTransactionsDialog behavior', () => {
    test('loads accounts and distinguishes processed, readable, and raw failures', async () => {
        const success = setup();
        expect(success.bindings.loading.value).toBe(false);
        await flush();
        expect(mockLoadAllAccounts).toHaveBeenCalled();
        expect(mockShowError).not.toHaveBeenCalled();

        mockLoadAllAccounts.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processed = setup();
        await flush();
        expect(processed.bindings.loading.value).toBe(false);
        expect(mockShowError).not.toHaveBeenCalled();

        const readable = { processed: false, message: 'account load failed' };
        mockLoadAllAccounts.mockRejectedValueOnce(readable);
        const failed = setup();
        await flush();
        expect(failed.bindings.loading.value).toBe(false);
        expect(mockShowError).toHaveBeenCalledWith(readable);

        mockLoadAllAccounts.mockRejectedValueOnce('raw failure');
        setup();
        await flush();
        expect(mockShowError).toHaveBeenLastCalledWith('raw failure');
    });

    test('opens with a clean form and exposes the public open contract', () => {
        const { bindings, exposed } = setup();
        bindings.moving.value = true;
        bindings.toAccountId.value = 'old-target';
        bindings.toAccountName.value = 'Old target';
        bindings.password.value = 'old-password';

        const pending = bindings.open({ id: 'source', name: 'Source' });
        expect(pending).toBeInstanceOf(Promise);
        expect(bindings.showState.value).toBe(true);
        expect(bindings.moving.value).toBe(false);
        expect(bindings.fromAccount.value).toEqual({ id: 'source', name: 'Source' });
        expect(bindings.toAccountId.value).toBe('');
        expect(bindings.toAccountName.value).toBe('');
        expect(bindings.password.value).toBe('');
        expect(exposed['open']).toBe(bindings.open);
    });

    test('guards every incomplete or unsafe confirmation state', () => {
        const { bindings } = setup();
        const source = { id: 'source', name: 'Source' };

        bindings.confirm();
        bindings.fromAccount.value = source;
        bindings.confirm();
        bindings.toAccountId.value = 'source';
        bindings.confirm();
        bindings.toAccountId.value = 'target';
        bindings.confirm();
        bindings.toAccountName.value = 'Wrong';
        bindings.confirm();
        bindings.toAccountName.value = 'Target';
        bindings.confirm();

        expect(mockMoveAllTransactions).not.toHaveBeenCalled();
    });

    test('moves transactions, resolves the caller, and closes the dialog', async () => {
        const { bindings } = setup();
        const completion = bindings.open({ id: 'source', name: 'Source' });
        bindings.toAccountId.value = 'target';
        bindings.toAccountName.value = 'Target';
        bindings.password.value = 'password';

        bindings.confirm();
        expect(bindings.moving.value).toBe(true);
        await flush();
        await expect(completion).resolves.toBeUndefined();
        expect(mockMoveAllTransactions).toHaveBeenCalled();
        expect(bindings.moving.value).toBe(false);
        expect(bindings.showState.value).toBe(false);
    });

    test('releases moving state and reports only unprocessed move failures', async () => {
        mockMoveAllTransactions.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processed = setup().bindings;
        processed.open({ id: 'source', name: 'Source' });
        processed.toAccountId.value = 'target';
        processed.toAccountName.value = 'Target';
        processed.password.value = 'password';
        processed.confirm();
        await flush();
        expect(processed.moving.value).toBe(false);
        expect(mockShowError).not.toHaveBeenCalled();

        const error = { processed: false, message: 'move failed' };
        mockMoveAllTransactions.mockRejectedValueOnce(error);
        const failed = setup().bindings;
        failed.open({ id: 'source', name: 'Source' });
        failed.toAccountId.value = 'target';
        failed.toAccountName.value = 'Target';
        failed.password.value = 'password';
        failed.confirm();
        await flush();
        expect(failed.moving.value).toBe(false);
        expect(mockShowError).toHaveBeenCalledWith(error);
    });

    test('rejects the caller on cancel and tolerates cancel before open', async () => {
        const beforeOpen = setup().bindings;
        beforeOpen.cancel();
        expect(beforeOpen.showState.value).toBe(false);

        const bindings = setup().bindings;
        const completion = bindings.open({ id: 'source', name: 'Source' });
        const rejection = completion.catch((error: unknown) => error);
        bindings.cancel();
        await expect(rejection).resolves.toBeUndefined();
        expect(bindings.showState.value).toBe(false);
    });

    test('renders enabled, disabled, loading, moving, and template event paths', async () => {
        const mounted = mountWithHostRenderer(MoveAllTransactionsDialogModule as any, {}, [
            'VDialog', 'VCard', 'VCardText', 'VRow', 'VCol', 'TwoColumnSelect',
            'VTextField', 'VBtn', 'VProgressCircular'
        ]);
        expect(collectHostCallbacks(mounted.root).length).toBeGreaterThan(0);
        mounted.state.showState = true;
        mounted.state.fromAccount = { id: 'source', name: 'Source' };
        mounted.state.toAccountId = 'target';
        mounted.state.toAccountName = 'Target';
        mounted.state.password = 'password';
        mounted.state.loading = true;
        mounted.state.moving = true;
        await mockActualVue.nextTick();

        for (const { name, callback } of collectHostCallbacks(mounted.root)) {
            if (name === 'onUpdate:modelValue') callback('template-value');
        }
        expect(mockFindAccountNameById).toHaveBeenCalled();
        mounted.app.unmount();
    });
});
