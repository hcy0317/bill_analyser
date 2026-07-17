import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import {
    collectHostCallbacks,
    mountWithHostRenderer,
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as any;
const mockMountedCallbacks: Array<() => void> = [];
const mockShowMessage = jest.fn<(...args: any[]) => void>();
const mockShowError = jest.fn<(...args: any[]) => void>();
const mockConfirmOpen = jest.fn<(...args: any[]) => Promise<void>>();
const mockStartDownloadFile = jest.fn<(...args: any[]) => void>();
const mockIsEquals = jest.fn<(...args: any[]) => boolean>();

let mockDataExportingEnabled = true;
let mockLastBase: ReturnType<typeof createDataManagementBase>;

const mockConfirmDialog = { open: mockConfirmOpen };
const mockSnackbar = { showMessage: mockShowMessage, showError: mockShowError };
const mockRootStore = {
    clearAllUserTransactions: jest.fn<(...args: any[]) => Promise<void>>(),
    clearAllUserData: jest.fn<(...args: any[]) => Promise<void>>(),
};
const mockUserStore = {
    getUserDataStatistics: jest.fn<(...args: any[]) => Promise<any>>(),
    getExportedUserData: jest.fn<(...args: any[]) => Promise<Blob>>(),
};

const mockStatistics = {
    totalTransactionCount: '12',
    totalAccountCount: '4',
    totalTransactionCategoryCount: '8',
    totalTransactionTagCount: '3',
    totalTransactionPictureCount: '2',
    totalTransactionTemplateCount: '5',
    totalScheduledTransactionCount: '6',
};
const mockSettingsEntries = [
    {
        sectionKey: 'accounts',
        title: 'tt:Accounts Settings',
        description: 'tt:Manage accounts',
        desktopRoute: '/settings/accounts',
    },
];

function createDataManagementBase(): any {
    const { ref } = jest.requireActual('vue') as any;
    return {
        dataStatistics: ref(null),
        displayDataStatistics: ref(null),
        settingsBundleDataManagementEntries: ref(mockSettingsEntries),
        getExportFileName: jest.fn((extension: string) => `synthetic-user-data.${extension}`),
    };
}

function createImportedStub(name: string): any {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => h(
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

function createExposedStub(name: string, exposed: Record<string, unknown>): any {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, expose, slots }: any) => {
            expose(exposed);
            return () => h(
                'stub',
                attrs,
                Object.values(slots).flatMap((slot: any) => {
                    try {
                        return slot?.({}) ?? [];
                    } catch {
                        return [];
                    }
                }),
            );
        },
    });
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => void) => mockMountedCallbacks.push(callback),
        useTemplateRef: (name: string) => actual.ref(
            name === 'confirmDialog'
                ? mockConfirmDialog
                : name === 'snackbar'
                    ? mockSnackbar
                    : null,
        ),
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/views/base/users/DataManagementPageBase.ts', () => ({
    useDataManagementPageBase: () => {
        mockLastBase = createDataManagementBase();
        return mockLastBase;
    },
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => mockRootStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/lib/common.ts', () => ({ isEquals: (...args: any[]) => mockIsEquals(...args) }));
jest.mock('@/lib/server_settings.ts', () => ({
    isDataExportingEnabled: () => mockDataExportingEnabled,
}));
jest.mock('@/lib/ui/common.ts', () => ({
    startDownloadFile: (...args: any[]) => mockStartDownloadFile(...args),
}));
jest.mock('@/components/desktop/ConfirmDialog.vue', () => ({
    __esModule: true,
    default: createExposedStub('DataManagementConfirmDialogStub', mockConfirmDialog),
}));
jest.mock('@/components/desktop/SettingsJsonImportExportButton.vue', () => ({
    __esModule: true,
    default: createImportedStub('DataManagementSettingsButtonStub'),
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: createExposedStub('DataManagementSnackBarStub', mockSnackbar),
}));

import UserDataManagementSettingTabComponent from '@/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue';

const UserDataManagementSettingTab = UserDataManagementSettingTabComponent as any;

function setup(): any {
    return UserDataManagementSettingTab.setup(
        {},
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() },
    );
}

function deferred<T>() {
    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockMountedCallbacks.length = 0;
    mockDataExportingEnabled = true;
    mockIsEquals.mockReturnValue(false);
    mockConfirmOpen.mockResolvedValue(undefined);
    mockRootStore.clearAllUserTransactions.mockResolvedValue(undefined);
    mockRootStore.clearAllUserData.mockResolvedValue(undefined);
    mockUserStore.getUserDataStatistics.mockResolvedValue(mockStatistics);
    mockUserStore.getExportedUserData.mockResolvedValue(new Blob(['synthetic-export']));
});

describe('desktop UserDataManagementSettingTab statistics', () => {
    test('loads on demand and through mounted lifecycle without force messaging', async () => {
        const bindings = setup();
        const pending = deferred<any>();
        mockUserStore.getUserDataStatistics.mockReturnValueOnce(pending.promise);

        bindings.reloadUserDataStatistics(false);
        expect(bindings.loadingDataStatistics.value).toBe(true);
        pending.resolve(mockStatistics);
        await flush();
        expect(bindings.loadingDataStatistics.value).toBe(false);
        expect(bindings.dataStatistics.value).toStrictEqual(mockStatistics);
        expect(mockShowMessage).not.toHaveBeenCalled();

        expect(mockMountedCallbacks).toHaveLength(1);
        mockMountedCallbacks[0]?.();
        await flush();
        expect(mockUserStore.getUserDataStatistics).toHaveBeenCalledTimes(2);
    });

    test('distinguishes unchanged and updated forced reloads', async () => {
        const bindings = setup();
        bindings.dataStatistics.value = mockStatistics;

        mockIsEquals.mockReturnValueOnce(true);
        bindings.reloadUserDataStatistics(true);
        await flush();
        expect(mockShowMessage).toHaveBeenCalledWith('Data is up to date');

        mockIsEquals.mockReturnValueOnce(false);
        mockUserStore.getUserDataStatistics.mockResolvedValueOnce({
            ...mockStatistics,
            totalTransactionCount: '13',
        });
        bindings.reloadUserDataStatistics(true);
        await flush();
        expect(mockShowMessage).toHaveBeenCalledWith('Data has been updated');
        expect(bindings.dataStatistics.value.totalTransactionCount).toBe('13');
    });

    test('suppresses processed reload errors and reports readable and raw failures', async () => {
        const bindings = setup();
        for (const failure of [
            { processed: true, message: 'handled' },
            { processed: false, message: 'reload failed' },
            'raw reload failure',
        ]) {
            mockUserStore.getUserDataStatistics.mockRejectedValueOnce(failure);
            bindings.reloadUserDataStatistics(false);
            await flush();
        }

        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ processed: true }));
        expect(mockShowError).toHaveBeenCalledWith({ processed: false, message: 'reload failed' });
        expect(mockShowError).toHaveBeenCalledWith('raw reload failure');
        expect(bindings.loadingDataStatistics.value).toBe(false);
    });
});

describe('desktop UserDataManagementSettingTab export', () => {
    test('downloads the selected format and keeps concurrent exports single-flight', async () => {
        const bindings = setup();
        const pending = deferred<Blob>();
        mockUserStore.getExportedUserData.mockReturnValueOnce(pending.promise);

        bindings.exportData('tsv');
        bindings.exportData('csv');
        expect(bindings.exportingData.value).toBe(true);
        expect(mockUserStore.getExportedUserData).toHaveBeenCalledTimes(1);
        pending.resolve(new Blob(['tsv-export']));
        await flush();

        expect(mockStartDownloadFile).toHaveBeenCalledWith(
            'synthetic-user-data.tsv',
            expect.any(Blob),
        );
        expect(bindings.exportingData.value).toBe(false);
    });

    test('suppresses processed export failures and reports readable and raw failures', async () => {
        const bindings = setup();
        for (const failure of [
            { processed: true, message: 'handled' },
            { processed: false, message: 'export failed' },
            'raw export failure',
        ]) {
            mockUserStore.getExportedUserData.mockRejectedValueOnce(failure);
            bindings.exportData('csv');
            await flush();
        }

        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ processed: true }));
        expect(mockShowError).toHaveBeenCalledWith({ processed: false, message: 'export failed' });
        expect(mockShowError).toHaveBeenCalledWith('raw export failure');
        expect(bindings.exportingData.value).toBe(false);
    });
});

describe('desktop UserDataManagementSettingTab destructive actions', () => {
    test('requires a password and prevents duplicate transaction-clear requests', async () => {
        const bindings = setup();
        bindings.clearAllTransactions();
        expect(mockShowMessage).toHaveBeenCalledWith('Current password cannot be blank');
        expect(mockConfirmOpen).not.toHaveBeenCalled();

        bindings.currentPasswordForClearData.value = 'synthetic-current-password';
        bindings.clearingData.value = true;
        bindings.clearAllTransactions();
        expect(mockConfirmOpen).not.toHaveBeenCalled();
    });

    test('confirms and clears all transactions before reloading statistics', async () => {
        const bindings = setup();
        const pending = deferred<void>();
        mockRootStore.clearAllUserTransactions.mockReturnValueOnce(pending.promise);
        bindings.currentPasswordForClearData.value = 'synthetic-current-password';

        bindings.clearAllTransactions();
        await flush(2);
        expect(mockConfirmOpen).toHaveBeenCalledWith(
            'Are you sure you want to clear all transactions?',
            { color: 'error' },
        );
        expect(bindings.clearingData.value).toBe(true);
        expect(mockRootStore.clearAllUserTransactions).toHaveBeenCalledWith({
            password: 'synthetic-current-password',
        });
        pending.resolve();
        await flush();

        expect(bindings.clearingData.value).toBe(false);
        expect(bindings.currentPasswordForClearData.value).toBe('');
        expect(mockShowMessage).toHaveBeenCalledWith('All transactions has been cleared');
        expect(mockUserStore.getUserDataStatistics).toHaveBeenCalled();
    });

    test('maps processed, readable, and raw transaction-clear failures', async () => {
        const bindings = setup();
        bindings.currentPasswordForClearData.value = 'synthetic-current-password';
        for (const failure of [
            { processed: true, message: 'handled' },
            { processed: false, message: 'clear transactions failed' },
            'raw clear transactions failure',
        ]) {
            mockRootStore.clearAllUserTransactions.mockRejectedValueOnce(failure);
            bindings.clearAllTransactions();
            await flush();
        }

        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ processed: true }));
        expect(mockShowError).toHaveBeenCalledWith({
            processed: false,
            message: 'clear transactions failed',
        });
        expect(mockShowError).toHaveBeenCalledWith('raw clear transactions failure');
        expect(bindings.clearingData.value).toBe(false);
    });

    test('requires a password and prevents duplicate full-data clear requests', () => {
        const bindings = setup();
        bindings.clearAllData();
        expect(mockShowMessage).toHaveBeenCalledWith('Current password cannot be blank');

        bindings.currentPasswordForClearData.value = 'synthetic-current-password';
        bindings.clearingData.value = true;
        bindings.clearAllData();
        expect(mockConfirmOpen).not.toHaveBeenCalled();
    });

    test('confirms and clears all user data before reloading statistics', async () => {
        const bindings = setup();
        const pending = deferred<void>();
        mockRootStore.clearAllUserData.mockReturnValueOnce(pending.promise);
        bindings.currentPasswordForClearData.value = 'synthetic-current-password';

        bindings.clearAllData();
        await flush(2);
        expect(mockConfirmOpen).toHaveBeenCalledWith(
            'Are you sure you want to clear all data?',
            { color: 'error' },
        );
        expect(bindings.clearingData.value).toBe(true);
        expect(mockRootStore.clearAllUserData).toHaveBeenCalledWith({
            password: 'synthetic-current-password',
        });
        pending.resolve();
        await flush();

        expect(bindings.currentPasswordForClearData.value).toBe('');
        expect(mockShowMessage).toHaveBeenCalledWith('All user data has been cleared');
        expect(mockUserStore.getUserDataStatistics).toHaveBeenCalled();
    });

    test('maps processed, readable, and raw full-data clear failures', async () => {
        const bindings = setup();
        bindings.currentPasswordForClearData.value = 'synthetic-current-password';
        for (const failure of [
            { processed: true, message: 'handled' },
            { processed: false, message: 'clear data failed' },
            'raw clear data failure',
        ]) {
            mockRootStore.clearAllUserData.mockRejectedValueOnce(failure);
            bindings.clearAllData();
            await flush();
        }

        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ processed: true }));
        expect(mockShowError).toHaveBeenCalledWith({ processed: false, message: 'clear data failed' });
        expect(mockShowError).toHaveBeenCalledWith('raw clear data failure');
        expect(bindings.clearingData.value).toBe(false);
    });
});

describe('desktop UserDataManagementSettingTab production template', () => {
    test('renders loading, populated, exporting, destructive, and disabled-export states', async () => {
        const mounted = mountWithHostRenderer(
            UserDataManagementSettingTab,
            {},
            [
                'v-row', 'v-col', 'v-card', 'v-card-text', 'v-btn', 'v-progress-circular',
                'v-icon', 'v-tooltip', 'v-avatar', 'v-skeleton-loader', 'v-btn-group', 'v-menu',
                'v-list', 'v-list-item', 'v-list-item-title', 'v-divider', 'v-text-field',
            ],
        );
        try {
            await flush();
            mounted.state.loadingDataStatistics = false;
            mounted.state.dataStatistics = mockStatistics;
            mounted.state.displayDataStatistics = mockStatistics;
            mounted.state.currentPasswordForClearData = 'synthetic-current-password';
            mounted.state.exportingData = false;
            mounted.state.clearingData = false;
            await mockActualVue.nextTick();

            const callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onClick', 'onImported', 'onUpdate:modelValue',
            ]));
            for (const { name, callback } of callbacks) {
                if (name === 'onUpdate:modelValue') callback('updated-password');
                else callback();
                await flush(1);
            }

            mounted.state.loadingDataStatistics = true;
            mounted.state.exportingData = true;
            mounted.state.clearingData = true;
            mounted.state.displayDataStatistics = null;
            mounted.state.dataStatistics = { ...mockStatistics, totalTransactionCount: '0' };
            mockDataExportingEnabled = false;
            await mockActualVue.nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
