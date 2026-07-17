import { afterAll, beforeEach, describe, expect, jest, test } from '@jest/globals';
import {
    collectHostCallbacks,
    type HostNode,
    mountWithHostRenderer
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as any;
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockRouteBackOnError = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockCreateObjectURL = jest.fn((_data: Blob) => 'blob:synthetic-user-data');
const mockGetExportFileName = jest.fn((extension: string) => `synthetic-user-data.${extension}`);
const mockOriginalCreateObjectURL = URL.createObjectURL;

let mockDataExportingEnabled = true;
let mockLastBase: ReturnType<typeof createDataManagementBase>;

const mockRootStore = {
    clearAllUserTransactions: jest.fn<(...args: any[]) => Promise<void>>(),
    clearAllUserData: jest.fn<(...args: any[]) => Promise<void>>()
};
const mockUserStore = {
    getUserDataStatistics: jest.fn<(...args: any[]) => Promise<any>>(),
    getExportedUserData: jest.fn<(...args: any[]) => Promise<Blob>>()
};

const mockStatistics = {
    totalTransactionCount: '12',
    totalAccountCount: '4',
    totalTransactionCategoryCount: '8',
    totalTransactionTagCount: '3',
    totalTransactionPictureCount: '2',
    totalTransactionTemplateCount: '5',
    totalScheduledTransactionCount: '6'
};
const mockDisplayStatistics = {
    totalTransactionCount: '12',
    totalAccountCount: '4',
    totalTransactionCategoryCount: '8',
    totalTransactionTagCount: '3',
    totalTransactionPictureCount: '2',
    totalTransactionTemplateCount: '5',
    totalScheduledTransactionCount: '6'
};
const mockSettingsEntries = [
    {
        sectionKey: 'accounts',
        title: 'tt:Accounts Settings',
        description: 'tt:Manage accounts',
        mobileRoute: '/settings/accounts'
    },
    {
        sectionKey: 'legacy',
        title: 'tt:Legacy Settings',
        description: 'tt:Unavailable on mobile',
        mobileRoute: null
    }
];

function createDataManagementBase(): any {
    const { ref } = jest.requireActual('vue') as any;
    return {
        dataStatistics: ref(null),
        displayDataStatistics: ref(null),
        settingsBundleDataManagementEntries: ref(mockSettingsEntries),
        getExportFileName: mockGetExportFileName
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showToast: mockShowToast,
        routeBackOnError: mockRouteBackOnError
    }),
    showLoading: (predicate?: () => unknown) => {
        mockShowLoading(predicate);
        predicate?.();
    },
    hideLoading: () => mockHideLoading()
}));
jest.mock('@/views/base/users/DataManagementPageBase.ts', () => ({
    useDataManagementPageBase: () => {
        mockLastBase = createDataManagementBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => mockRootStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/lib/server_settings.ts', () => ({
    isDataExportingEnabled: () => mockDataExportingEnabled
}));

import DataManagementPageModule from '@/views/mobile/users/DataManagementPage.vue';

const DataManagementPage = DataManagementPageModule as any;

function createRouter(): any {
    return {
        back: jest.fn(),
        navigate: jest.fn(),
        refreshPage: jest.fn()
    };
}

function setup(): { bindings: any; router: any } {
    const router = createRouter();
    const bindings = DataManagementPage.setup(
        { f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() }
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
}

function findNodes(node: HostNode, predicate: (candidate: HostNode) => boolean): HostNode[] {
    const matches = predicate(node) ? [node] : [];
    for (const child of node.children) matches.push(...findNodes(child, predicate));
    return matches;
}

async function invokeTemplateCallbacks(root: HostNode): Promise<void> {
    for (const { name, callback } of collectHostCallbacks(root)) {
        if (name === 'onUpdate:show') {
            callback(true);
        } else if (name === 'onUpdate:modelValue') {
            callback('<template-password>');
        } else if (name === 'onPassword:confirm') {
            callback('<template-password>');
        } else {
            callback();
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockDataExportingEnabled = true;
    mockGetExportFileName.mockImplementation(extension => `synthetic-user-data.${extension}`);
    mockUserStore.getUserDataStatistics.mockResolvedValue(mockStatistics);
    mockUserStore.getExportedUserData.mockResolvedValue(new Blob(['synthetic-export']));
    mockRootStore.clearAllUserTransactions.mockResolvedValue(undefined);
    mockRootStore.clearAllUserData.mockResolvedValue(undefined);
    Object.defineProperty(URL, 'createObjectURL', {
        configurable: true,
        value: mockCreateObjectURL
    });
});

afterAll(() => {
    if (mockOriginalCreateObjectURL) {
        Object.defineProperty(URL, 'createObjectURL', {
            configurable: true,
            value: mockOriginalCreateObjectURL
        });
    } else {
        delete (URL as any).createObjectURL;
    }
});

describe('mobile DataManagementPage statistics and navigation', () => {
    test('loads statistics on setup, projects export names, and routes load errors on page entry', async () => {
        const { bindings, router } = setup();
        expect(bindings.loading.value).toBe(true);
        expect(mockUserStore.getUserDataStatistics).toHaveBeenCalledTimes(1);
        await flush();

        expect(bindings.dataStatistics.value).toEqual(mockStatistics);
        expect(bindings.loading.value).toBe(false);
        expect(bindings.exportFileName.value).toBe('synthetic-user-data.csv');
        bindings.exportFileType.value = 'tsv';
        expect(bindings.exportFileName.value).toBe('synthetic-user-data.tsv');

        bindings.loadingError.value = { message: 'synthetic loading error' };
        bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });

    test('distinguishes processed, readable, and raw statistics failures', async () => {
        mockUserStore.getUserDataStatistics.mockRejectedValueOnce({
            processed: true,
            message: 'handled statistics failure'
        });
        const processed = setup();
        await flush();
        expect(processed.bindings.loading.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalledWith('handled statistics failure');

        const readableError = { processed: false, message: 'statistics failed' };
        mockUserStore.getUserDataStatistics.mockRejectedValueOnce(readableError);
        processed.bindings.reloadUserDataStatistics();
        await flush();
        expect(processed.bindings.loading.value).toBe(true);
        expect(processed.bindings.loadingError.value).toEqual(readableError);
        expect(mockShowToast).toHaveBeenCalledWith('statistics failed');

        mockUserStore.getUserDataStatistics.mockRejectedValueOnce('raw statistics failure');
        processed.bindings.reloadUserDataStatistics();
        await flush();
        expect(processed.bindings.loadingError.value).toBe('raw statistics failure');
        expect(mockShowToast).toHaveBeenCalledWith('raw statistics failure');
    });
});

describe('mobile DataManagementPage export', () => {
    test('exports the selected format to an object URL and releases loading', async () => {
        const data = new Blob(['id\tamount']);
        mockUserStore.getExportedUserData.mockResolvedValueOnce(data);
        const { bindings } = setup();
        await flush();
        bindings.exportFileType.value = 'tsv';

        bindings.exportData();
        expect(bindings.exportingData.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(undefined);
        expect(mockUserStore.getExportedUserData).toHaveBeenCalledWith('tsv');
        await flush();

        expect(mockCreateObjectURL).toHaveBeenCalledWith(data);
        expect(bindings.exportedData.value).toBe('blob:synthetic-user-data');
        expect(bindings.exportingData.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
    });

    test('clears stale exports and reports only unprocessed export failures', async () => {
        const { bindings } = setup();
        await flush();
        bindings.exportedData.value = 'blob:stale';

        const processed = { processed: true, message: 'handled export failure' };
        mockUserStore.getExportedUserData.mockRejectedValueOnce(processed);
        bindings.exportData();
        await flush();
        expect(bindings.exportedData.value).toBeNull();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled export failure');

        mockUserStore.getExportedUserData.mockRejectedValueOnce({
            processed: false,
            message: 'export failed'
        });
        bindings.exportData();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('export failed');

        mockUserStore.getExportedUserData.mockRejectedValueOnce('raw export failure');
        bindings.exportData();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw export failure');
        expect(bindings.exportingData.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalledTimes(3);
    });
});

describe('mobile DataManagementPage destructive actions', () => {
    test('opens the correct password sheet before either destructive action', async () => {
        const { bindings } = setup();
        await flush();
        bindings.currentPasswordForClearData.value = '<stale-password>';
        bindings.clearAllTransactions(null);
        expect(bindings.currentPasswordForClearData.value).toBe('');
        expect(bindings.showInputPasswordSheetForClearAllTransactions.value).toBe(true);
        expect(mockRootStore.clearAllUserTransactions).not.toHaveBeenCalled();

        bindings.currentPasswordForClearData.value = '<second-stale-password>';
        bindings.clearAllData(null);
        expect(bindings.currentPasswordForClearData.value).toBe('');
        expect(bindings.showInputPasswordSheetForClearAllData.value).toBe(true);
        expect(mockRootStore.clearAllUserData).not.toHaveBeenCalled();
    });

    test('clears all transactions, closes the sheet, and reloads statistics', async () => {
        const { bindings } = setup();
        await flush();
        bindings.currentPasswordForClearData.value = '<current-password>';
        bindings.showInputPasswordSheetForClearAllTransactions.value = true;

        bindings.clearAllTransactions('<current-password>');
        expect(bindings.clearingData.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(expect.any(Function));
        expect(mockRootStore.clearAllUserTransactions).toHaveBeenCalledWith({
            password: '<current-password>'
        });
        await flush();

        expect(bindings.clearingData.value).toBe(false);
        expect(bindings.currentPasswordForClearData.value).toBe('');
        expect(bindings.showInputPasswordSheetForClearAllTransactions.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('All transactions has been cleared');
        expect(mockUserStore.getUserDataStatistics).toHaveBeenCalledTimes(2);
    });

    test('handles processed, readable, and raw clear-transactions failures', async () => {
        const { bindings } = setup();
        await flush();
        const failures = [
            { value: { processed: true, message: 'handled transaction clear' }, toast: null },
            { value: { processed: false, message: 'transaction clear failed' }, toast: 'transaction clear failed' },
            { value: 'raw transaction clear failure', toast: 'raw transaction clear failure' }
        ];

        for (const failure of failures) {
            mockRootStore.clearAllUserTransactions.mockRejectedValueOnce(failure.value);
            bindings.clearAllTransactions('<current-password>');
            await flush();
            expect(bindings.clearingData.value).toBe(false);
            if (failure.toast) expect(mockShowToast).toHaveBeenCalledWith(failure.toast);
            else expect(mockShowToast).not.toHaveBeenCalledWith('handled transaction clear');
        }
    });

    test('clears all user data, closes the sheet, and reloads statistics', async () => {
        const { bindings } = setup();
        await flush();
        bindings.currentPasswordForClearData.value = '<current-password>';
        bindings.showInputPasswordSheetForClearAllData.value = true;

        bindings.clearAllData('<current-password>');
        expect(bindings.clearingData.value).toBe(true);
        expect(mockRootStore.clearAllUserData).toHaveBeenCalledWith({
            password: '<current-password>'
        });
        await flush();

        expect(bindings.clearingData.value).toBe(false);
        expect(bindings.currentPasswordForClearData.value).toBe('');
        expect(bindings.showInputPasswordSheetForClearAllData.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('All user data has been cleared');
        expect(mockUserStore.getUserDataStatistics).toHaveBeenCalledTimes(2);
    });

    test('handles processed, readable, and raw clear-data failures', async () => {
        const { bindings } = setup();
        await flush();
        const failures = [
            { value: { processed: true, message: 'handled data clear' }, toast: null },
            { value: { processed: false, message: 'data clear failed' }, toast: 'data clear failed' },
            { value: 'raw data clear failure', toast: 'raw data clear failure' }
        ];

        for (const failure of failures) {
            mockRootStore.clearAllUserData.mockRejectedValueOnce(failure.value);
            bindings.clearAllData('<current-password>');
            await flush();
            expect(bindings.clearingData.value).toBe(false);
            if (failure.toast) expect(mockShowToast).toHaveBeenCalledWith(failure.toast);
            else expect(mockShowToast).not.toHaveBeenCalledWith('handled data clear');
        }
    });
});

describe('mobile DataManagementPage production template', () => {
    test('renders and executes loading, loaded, export, password, and restricted states', async () => {
        const router = createRouter();
        const mounted = mountWithHostRenderer(DataManagementPage, { f7router: router }, [
            'f7-page', 'f7-navbar', 'f7-list', 'f7-list-item', 'f7-list-button',
            'f7-block-title', 'f7-sheet', 'f7-page-content', 'f7-button', 'f7-link',
            'password-input-sheet'
        ]);
        try {
            expect(findNodes(mounted.root, node => node.props['data-testid'] === 'mobile.data-management.page')).toHaveLength(1);
            expect(findNodes(mounted.root, node => node.props['title'] === 'Transactions')).toHaveLength(1);
            await flush();

            mounted.state.dataStatistics = mockStatistics;
            mounted.state.displayDataStatistics = mockDisplayStatistics;
            mounted.state.settingsBundleDataManagementEntries = mockSettingsEntries;
            mounted.state.currentPasswordForClearData = '<template-password>';
            mounted.state.showExportDataSheet = true;
            mounted.state.showInputPasswordSheetForClearAllTransactions = true;
            mounted.state.showInputPasswordSheetForClearAllData = true;
            await flush();

            expect(findNodes(mounted.root, node => node.props['title'] === 'tt:Transactions' && node.props['after'] === '12')).toHaveLength(1);
            expect(findNodes(mounted.root, node => node.props['link'] === '/settings/accounts')).toHaveLength(1);
            expect(findNodes(mounted.root, node => node.props['link'] === null)).toHaveLength(1);
            const loadedCallbacks = collectHostCallbacks(mounted.root);
            expect(loadedCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onPage:afterin',
                'onClick',
                'onChange',
                'onSheet:closed',
                'onUpdate:show',
                'onUpdate:modelValue',
                'onPassword:confirm'
            ]));
            await invokeTemplateCallbacks(mounted.root);
            expect(mockRouteBackOnError).toHaveBeenCalledWith(router, expect.anything());
            expect(mockRootStore.clearAllUserTransactions).toHaveBeenCalled();
            expect(mockRootStore.clearAllUserData).toHaveBeenCalled();

            mounted.state.exportedData = 'blob:synthetic-user-data';
            mounted.state.exportFileType = 'tsv';
            mounted.state.exportingData = true;
            mounted.state.clearingData = true;
            await flush();
            expect(findNodes(mounted.root, node => node.props['href'] === 'blob:synthetic-user-data')).toHaveLength(1);
            expect(findNodes(mounted.root, node => node.props['download'] === 'synthetic-user-data.tsv')).toHaveLength(1);

            mounted.state.exportingData = false;
            mounted.state.clearingData = false;
            mounted.state.exportedData = null;
            mounted.state.dataStatistics = { ...mockStatistics, totalTransactionCount: '0' };
            mounted.state.displayDataStatistics = null;
            await flush();
            expect(findNodes(mounted.root, node => node.props['title'] === 'tt:Transactions' && node.props['after'] === '-')).toHaveLength(1);

            mounted.state.dataStatistics = null;
            mockDataExportingEnabled = false;
            mounted.state.loading = true;
            await flush();
            expect(findNodes(mounted.root, node => node.props['title'] === 'Transactions')).toHaveLength(1);
            mounted.state.loading = false;
            await flush();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
