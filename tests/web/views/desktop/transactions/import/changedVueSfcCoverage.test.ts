import { jest } from '@jest/globals';

const mockFetchImportStage = jest.fn<(...args: unknown[]) => Promise<any>>();
const mockUseExternalTemplateBindings = jest.fn();
const mockAddPrimaryCategory = jest.fn();
const mockAddSecondaryCategory = jest.fn();

jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: (...args: unknown[]) => mockUseExternalTemplateBindings(...args)
}));

jest.mock('@/views/desktop/categories/list/useCategoryListPage.ts', () => ({
    useDesktopCategoryListPage: () => new Proxy({}, {
        get: (_target, key) => {
            if (key === 'addPrimaryCategory') return mockAddPrimaryCategory;
            if (key === 'addSecondaryCategory') return mockAddSecondaryCategory;
            if (key === 'tt') return (value: string) => value;
            return jest.fn();
        }
    })
}));

jest.mock('@/views/desktop/transactions/import/importDialogApi.ts', () => ({
    extractApiErrorMessage: jest.fn(),
    fetchImportStage: (...args: unknown[]) => mockFetchImportStage(...args),
    isAbortError: () => false
}));

jest.mock('@/views/desktop/transactions/import/import-dialog/useImportSourceSelection.ts', () => ({
    useImportSourceSelection: () => ({
        allFileSubTypes: { value: [] },
        exportFileGuideDocumentLanguageName: { value: '' },
        exportFileGuideDocumentUrl: { value: '' },
        fileType: { value: 'auto' },
        isImportDataFromTextbox: { value: false },
        showHandlingMethodSelector: { value: false },
        supportedImportFileExtensions: { value: '.csv' }
    })
}));

jest.mock('@/views/desktop/transactions/import/import-dialog/useImportFlowProgress.ts', () => ({
    useImportFlowProgress: () => ({
        currentFlowProgressDetail: { value: '' },
        currentFlowProgressIndex: { value: 0 },
        currentFlowProgressItem: { value: null },
        currentFlowProgressValue: { value: 0 },
        importFlowProgressItems: { value: [] }
    })
}));

jest.mock('@/views/desktop/transactions/import/import-dialog/importFlowProfiler.ts', () => ({
    createImportFlowMilestoneLogger: () => jest.fn()
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (value: string) => value,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatNumberToLocalizedNumerals: (value: number) => String(value)
    })
}));

const mockEmptyStore = {
    loadAllAccounts: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    loadAllCategories: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    loadAllTags: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    allTransactionCategoriesMap: {},
    allTransactionCategories: {},
    allVisiblePlainAccounts: [],
    updateAccountListInvalidState: jest.fn(),
    updateTransactionListInvalidState: jest.fn(),
    updateTransactionOverviewInvalidState: jest.fn(),
    updateTransactionStatisticsInvalidState: jest.fn(),
    appSettings: { timeZone: 'Asia/Shanghai' },
    currentUserCashTransferCategoryId: null
};

jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockEmptyStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockEmptyStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockEmptyStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockEmptyStore }));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockEmptyStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockEmptyStore }));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockEmptyStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockEmptyStore }));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => '' }));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: new Proxy({}, {
        get: () => jest.fn<() => Promise<any>>().mockResolvedValue({ data: { result: {} } })
    })
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

for (const componentPath of [
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
    '@/components/desktop/SettingsJsonImportExportButton.vue',
    '@/views/desktop/categories/list/dialogs/EditDialog.vue',
    '@/views/desktop/categories/list/dialogs/PresetDialog.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionDefineColumnTab.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionExecuteCustomScriptTab.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue',
    '@/views/desktop/transactions/import/import-dialog/ImportFlowProgress.vue',
    '@/views/desktop/transactions/import/import-dialog/ImportCheckDataFilterButton.vue'
]) {
    jest.mock(componentPath, () => ({ __esModule: true, default: { name: 'CoverageStub' } }));
}

import DesktopCategoryListPage from '@/views/desktop/categories/ListPage.vue';
import ImportDialog from '@/views/desktop/transactions/import/ImportDialog.vue';

describe('changed Vue SFC coverage', () => {
    test('desktop category facade binds distinct primary and secondary actions', () => {
        (DesktopCategoryListPage as any).setup({}, { expose: jest.fn() });

        const bindingArguments = mockUseExternalTemplateBindings.mock.calls[0] ?? [];
        expect(bindingArguments).toContain(mockAddPrimaryCategory);
        expect(bindingArguments).toContain(mockAddSecondaryCategory);
    });

    test('ImportDialog replaces an active canonical request only when requested', async () => {
        const bindings = (ImportDialog as any).setup({ persistent: false }, {
            expose: jest.fn()
        });
        bindings.serverSessionId.value = 'coverage-session';
        const firstResponse = deferred<any>();
        mockFetchImportStage
            .mockReturnValueOnce(firstResponse.promise)
            .mockResolvedValueOnce(successfulPreviewResponse());

        const first = bindings.fetchPreviewPage(1, 10, { signal: 'learning' });
        await bindings.onCheckDataPageRequested(1, 10, {
            filters: { signal: 'learning' },
            replaceActive: true
        });
        firstResponse.resolve(successfulPreviewResponse());
        await first;

        expect(mockFetchImportStage).toHaveBeenCalledTimes(2);
        expect(bindings.previewTotalCount.value).toBe(0);
    });
});

function successfulPreviewResponse(): Record<string, unknown> {
    return {
        ok: true,
        json: async () => ({
            success: true,
            data: { preview: [], total: 0, metadata: null }
        })
    };
}

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>(resolveValue => {
        resolve = resolveValue;
    });
    return { promise, resolve };
}
