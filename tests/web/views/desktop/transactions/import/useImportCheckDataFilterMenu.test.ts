import { describe, expect, jest, test } from '@jest/globals';

const { createSSRApp, h, nextTick, ref } = require('vue');
const { renderToString } = require('vue/server-renderer');

import type { ImportTransactionCheckDataFilterMenuGroup } from '@/views/desktop/transactions/import/import-dialog/types.ts';

const filterMenuModule = require('@/views/desktop/transactions/import/import-dialog/useImportCheckDataFilterMenu.ts') as typeof import('@/views/desktop/transactions/import/import-dialog/useImportCheckDataFilterMenu.ts');
const mockUseImportCheckDataFilterMenu = jest.spyOn(filterMenuModule, 'useImportCheckDataFilterMenu');
const { useImportCheckDataFilterMenu } = filterMenuModule;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatNumberToLocalizedNumerals: (value: number) => String(value)
    })
}));

jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({
        allVisiblePlainAccounts: [],
        loadAllAccounts: jest.fn(),
        updateAccountListInvalidState: jest.fn()
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: {},
        allTransactionCategoriesMap: {},
        loadAllCategories: jest.fn()
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({
        loadAllTags: jest.fn()
    })
}));
jest.mock('@/stores/transaction.ts', () => ({
    useTransactionsStore: () => ({ updateTransactionListInvalidState: jest.fn() })
}));
jest.mock('@/stores/overview.ts', () => ({
    useOverviewStore: () => ({ updateTransactionOverviewInvalidState: jest.fn() })
}));
jest.mock('@/stores/statistics.ts', () => ({
    useStatisticsStore: () => ({ updateTransactionStatisticsInvalidState: jest.fn() })
}));
jest.mock('@/stores/setting.ts', () => ({
    useSettingsStore: () => ({ appSettings: { timeZone: 'Asia/Shanghai' } })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({ currentUserCashTransferCategoryId: '' })
}));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => '' }));
jest.mock('@/lib/importFileDialog.ts', () => ({ openImportFileDialog: jest.fn() }));
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: {} }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

for (const componentPath of [
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionDefineColumnTab.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionExecuteCustomScriptTab.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue',
    '@/views/desktop/transactions/import/import-dialog/ImportFlowProgress.vue',
    '@/views/desktop/transactions/import/import-dialog/ImportCheckDataFilterButton.vue'
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: { name: 'ImportDialogRuntimeStub', render: () => null }
    }));
}

describe('useImportCheckDataFilterMenu', () => {
    test('opens active groups, retains valid groups, falls back, and closes outside check data', async () => {
        const currentStep = ref('checkData');
        const openedGroups = ref([] as string[]);
        const showMenu = ref(false);
        const groups = ref([
            { title: 'Type', summary: 'All' },
            { title: 'Signals', summary: 'Learning' }
        ] as ImportTransactionCheckDataFilterMenuGroup[]);
        const menu = useImportCheckDataFilterMenu({
            currentStep,
            openedGroups,
            showMenu,
            getFilterMenus: () => groups.value,
            translate: key => key
        });

        expect(menu.isActiveCheckDataFilterGroup('All')).toBe(false);
        expect(menu.isActiveCheckDataFilterGroup('Learning')).toBe(true);
        showMenu.value = true;
        await nextTick();
        expect(openedGroups.value).toEqual(['Signals']);

        openedGroups.value = ['Type'];
        groups.value = [{ title: 'Type', summary: 'All' }, { title: 'Date', summary: 'All' }];
        await nextTick();
        expect(openedGroups.value).toEqual(['Type']);

        openedGroups.value = ['Removed'];
        groups.value = [{ title: 'Date', summary: 'All' }];
        await nextTick();
        expect(openedGroups.value).toEqual(['Date']);

        currentStep.value = 'uploadFile';
        await nextTick();
        expect(showMenu.value).toBe(false);
    });

    test('clears opened groups when the visible menu becomes empty', async () => {
        const groups = ref([{ title: 'Type' }] as ImportTransactionCheckDataFilterMenuGroup[]);
        const openedGroups = ref(['Type']);
        const showMenu = ref(true);
        useImportCheckDataFilterMenu({
            currentStep: ref('checkData'),
            openedGroups,
            showMenu,
            getFilterMenus: () => groups.value,
            translate: key => key
        });

        groups.value = [];
        await nextTick();
        expect(openedGroups.value).toEqual([]);
    });

    test('initializes the filter menu through the real ImportDialog SFC setup', async () => {
        mockUseImportCheckDataFilterMenu.mockClear();
        const ImportDialog = require('@/views/desktop/transactions/import/ImportDialog.vue').default;
        const RuntimeImportDialog = {
            ...ImportDialog,
            render: () => h('div', { 'data-testid': 'import-dialog-runtime' })
        };

        await renderToString(createSSRApp(RuntimeImportDialog, { persistent: true }));

        expect(mockUseImportCheckDataFilterMenu).toHaveBeenCalledTimes(1);
        const options = mockUseImportCheckDataFilterMenu.mock.calls[0]?.[0];
        expect(options?.getFilterMenus()).toEqual([]);
        expect(options?.translate('All')).toBe('All');
    });
});
