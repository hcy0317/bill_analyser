import { beforeEach, describe, expect, jest, test } from '@jest/globals';

let mockUserNickname = 'Alice';
const mockCopyTextToClipboard = jest.fn();
const mockStartDownloadFile = jest.fn();
const mockShowMessage = jest.fn();

const mockRecurringStore = {
    loading: false,
    error: null as string | null,
    suggestions: [] as any[],
    suggestionsTotal: 0,
    detectPatterns: jest.fn<(...args: any[]) => Promise<any>>(),
    loadSuggestions: jest.fn<(...args: any[]) => Promise<void>>(),
    acceptSuggestion: jest.fn<(...args: any[]) => Promise<void>>(),
    rejectSuggestion: jest.fn<(...args: any[]) => Promise<void>>()
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null),
        onMounted: (callback: () => void) => callback()
    };
});
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: { name: 'Wave11ExportSnackBarStub', template: '<div />' }
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: { nickname?: string }) => values?.nickname
            ? `tt:${key}:${values.nickname}`
            : `tt:${key}`
    })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({
        get currentUserNickname() {
            return mockUserNickname;
        }
    })
}));
jest.mock('@/lib/common.ts', () => ({
    replaceAll: (value: string, search: string, replacement: string) => value.split(search).join(replacement)
}));
jest.mock('@/lib/ui/common.ts', () => ({
    copyTextToClipboard: (...args: unknown[]) => mockCopyTextToClipboard(...args),
    startDownloadFile: (...args: unknown[]) => mockStartDownloadFile(...args)
}));
jest.mock('vue-i18n', () => ({
    useI18n: () => ({ t: (key: string) => `tt:${key}` })
}));
jest.mock('@/stores/recurring.ts', () => ({
    useRecurringStore: () => {
        const { reactive } = jest.requireActual('vue') as any;
        return reactive(mockRecurringStore);
    }
}));
jest.mock('@/models/recurring_suggestion.ts', () => ({
    getFrequencyLabel: (frequency: string) => `frequency:${frequency}`,
    getConfidenceColor: (score: number) => score >= 0.8 ? 'strong' : 'weak'
}));

import { KnownFileType } from '@/core/file.ts';
import ExportDialog from '@/views/desktop/statistics/transaction/dialogs/ExportDialog.vue';
import DiscoverPage from '@/views/desktop/recurring/DiscoverPage.vue';

function setupExportDialog(): { bindings: any; exposed: Record<string, unknown> } {
    const exposed: Record<string, unknown> = {};
    const bindings = (ExportDialog as any).setup({}, {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value)
    });
    bindings.buttonContainer.value = { id: 'button-container' };
    bindings.snackbar.value = { showMessage: mockShowMessage };
    return { bindings, exposed };
}

function setupDiscoverPage(): any {
    return (DiscoverPage as any).setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    });
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await (jest.requireActual('vue') as any).nextTick();
    await new Promise(resolve => setImmediate(resolve));
}

beforeEach(() => {
    jest.clearAllMocks();
    mockUserNickname = 'Alice';
    Object.assign(mockRecurringStore, {
        loading: false,
        error: null,
        suggestions: [],
        suggestionsTotal: 0
    });
    mockRecurringStore.detectPatterns.mockResolvedValue(null);
    mockRecurringStore.loadSuggestions.mockResolvedValue(undefined);
    mockRecurringStore.acceptSuggestion.mockResolvedValue(undefined);
    mockRecurringStore.rejectSuggestion.mockResolvedValue(undefined);
});

describe('statistics ExportDialog', () => {
    test('opens with CSV defaults and projects headers and rows into table bindings', () => {
        const { bindings, exposed } = setupExportDialog();
        const options = {
            headers: ['Name,Label', 'Amount'],
            data: [
                ['Coffee, shop', '12.34'],
                ['Salary', '1000.00']
            ]
        };

        expect(exposed).toEqual({ open: bindings.open });
        (exposed['open'] as (value: typeof options) => void)(options);

        expect(bindings.showState.value).toBe(true);
        expect(bindings.fileFormat.value).toBe(KnownFileType.CSV.extension);
        expect(bindings.showRawData.value).toBe(false);
        expect(bindings.dataTableHeaders.value).toEqual([
            { key: '0', value: 'column0', title: 'Name,Label', sortable: false, nowrap: true },
            { key: '1', value: 'column1', title: 'Amount', sortable: true, nowrap: true }
        ]);
        expect(bindings.dataTableItems.value).toEqual([
            { column0: 'Coffee, shop', column1: '12.34' },
            { column0: 'Salary', column1: '1000.00' }
        ]);
        expect(bindings.exportedData.value).toBe(
            'Name Label,Amount\nCoffee  shop,12.34\nSalary,1000.00'
        );

        bindings.open({ headers: undefined, data: undefined } as any);
        expect(bindings.headers.value).toEqual([]);
        expect(bindings.data.value).toEqual([]);
        expect(bindings.exportedData.value).toBe('');
    });

    test('exports TSV and Markdown while sanitizing format delimiters', () => {
        const { bindings } = setupExportDialog();
        bindings.open({
            headers: ['Name\tLabel', 'Pipe|Value'],
            data: [['A\tB', 'C|D']]
        });

        bindings.fileFormat.value = KnownFileType.TSV.extension;
        expect(bindings.exportedData.value).toBe('Name Label\tPipe|Value\nA B\tC|D');

        bindings.fileFormat.value = KnownFileType.MARKDOWN.extension;
        expect(bindings.exportedData.value).toBe(
            '| Name\tLabel | Pipe Value |\n| --- | --- |\n| A\tB | C D |'
        );

        bindings.fileFormat.value = 'unsupported';
        expect(bindings.exportedData.value).toBe('');
    });

    test('copies, downloads with known/fallback formats, resolves filenames, and cancels', () => {
        const { bindings } = setupExportDialog();
        bindings.open({ headers: ['Name'], data: [['Coffee']] });

        expect(bindings.fileName.value).toBe('tt:dataExport.exportStatisticsFileName:Alice');
        bindings.copy();
        expect(mockCopyTextToClipboard).toHaveBeenCalledWith(
            'Name\nCoffee',
            { id: 'button-container' }
        );
        expect(mockShowMessage).toHaveBeenCalledWith('Data copied');

        bindings.fileFormat.value = KnownFileType.MARKDOWN.extension;
        bindings.save();
        expect(mockStartDownloadFile).toHaveBeenLastCalledWith(
            'tt:dataExport.exportStatisticsFileName:Alice.md',
            expect.objectContaining({ type: 'text/markdown' })
        );

        mockUserNickname = '';
        const fallback = setupExportDialog().bindings;
        fallback.open({ headers: ['Name'], data: [['Coffee']] });
        fallback.fileFormat.value = 'unsupported';
        fallback.save();
        expect(fallback.fileName.value).toBe('tt:dataExport.defaultExportStatisticsFileName');
        expect(mockStartDownloadFile).toHaveBeenLastCalledWith(
            'tt:dataExport.defaultExportStatisticsFileName.csv',
            expect.objectContaining({ type: 'text/csv' })
        );

        bindings.cancel();
        expect(bindings.showState.value).toBe(false);
    });
});

describe('recurring DiscoverPage', () => {
    test('projects store state, filters suggestions, and formats table values', async () => {
        Object.assign(mockRecurringStore, {
            loading: true,
            error: 'load failed',
            suggestionsTotal: 3,
            suggestions: [
                { id: 1, status: 'pending', amountCents: 12_345 },
                { id: 2, status: 'accepted', amountCents: 500 }
            ]
        });
        const bindings = setupDiscoverPage();

        expect(bindings.loading.value).toBe(true);
        expect(bindings.error.value).toBe('load failed');
        bindings.error.value = null;
        expect(mockRecurringStore.error).toBeNull();
        expect(bindings.suggestionsTotal.value).toBe(3);
        expect(bindings.filteredSuggestions.value).toHaveLength(2);
        bindings.statusFilter.value = 'pending';
        await (jest.requireActual('vue') as any).nextTick();
        expect(bindings.filteredSuggestions.value).toEqual([
            expect.objectContaining({ id: 1, status: 'pending' })
        ]);

        expect(bindings.tableHeaders.value).toHaveLength(10);
        expect(bindings.tableHeaders.value[0]).toEqual(expect.objectContaining({
            title: 'tt:Name', key: 'name', sortable: true
        }));
        expect(bindings.formatAmount(12_345)).toBe('123.45');
        expect(bindings.statusColor('accepted')).toBe('success');
        expect(bindings.statusColor('rejected')).toBe('error');
        expect(bindings.statusColor('pending')).toBe('warning');
        expect(bindings.statusLabel('pending')).toBe('tt:Pending');
        expect(bindings.statusLabel('accepted')).toBe('tt:Accepted');
        expect(bindings.statusLabel('rejected')).toBe('tt:Rejected');
        expect(bindings.statusLabel('unknown')).toBe('unknown');
    });

    test('runs discovery, refresh, accept, and reject actions with exact store arguments', async () => {
        const result = { detected: 4, created: 2, updated: 1 };
        mockRecurringStore.detectPatterns
            .mockResolvedValueOnce(result)
            .mockResolvedValueOnce(null);
        const bindings = setupDiscoverPage();

        await bindings.handleDetect();
        expect(bindings.lastDetectResult.value).toEqual(result);
        await bindings.handleDetect();
        expect(bindings.lastDetectResult.value).toEqual(result);

        await bindings.handleRefresh();
        expect(mockRecurringStore.loadSuggestions).toHaveBeenLastCalledWith(undefined);
        bindings.statusFilter.value = 'accepted';
        await flush();
        expect(mockRecurringStore.loadSuggestions).toHaveBeenCalledWith('accepted');
        await bindings.handleRefresh();
        expect(mockRecurringStore.loadSuggestions).toHaveBeenLastCalledWith('accepted');

        await bindings.handleAccept(41);
        await bindings.handleReject(42);
        expect(mockRecurringStore.acceptSuggestion).toHaveBeenCalledWith(41);
        expect(mockRecurringStore.rejectSuggestion).toHaveBeenCalledWith(42);
    });
});
