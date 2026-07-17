import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockTemplateRefs = new Map<string, any>();
const mockOpenTextFileContent = jest.fn<(...args: any[]) => Promise<string>>();
const mockStartDownloadFile = jest.fn();
const mockLogger = {
    error: jest.fn()
};

let mockDetectedTimeFormat: string | undefined;
let mockDetectedTimezoneFormat: string | undefined;
let mockDetectedAmountFormat: string | undefined;

const mockTransactionType = {
    ModifyBalance: 0,
    Income: 1,
    Expense: 2,
    Transfer: 3,
    Investment: 4
};

const mockColumnType = {
    TransactionTime: { type: 1, name: 'Transaction Time' },
    TransactionTimezone: { type: 2, name: 'Transaction Timezone' },
    TransactionType: { type: 3, name: 'Transaction Type' },
    Amount: { type: 8, name: 'Amount' },
    GeographicLocation: { type: 12, name: 'Geographic Location' },
    Tags: { type: 13, name: 'Tags' }
};

const mockAmountFormats = [
    {
        type: 'dot',
        format: '1234.56',
        decimalSeparator: { symbol: '.' },
        digitGroupingSymbol: undefined
    },
    {
        type: 'grouped',
        format: '1,234.56',
        decimalSeparator: { symbol: '.' },
        digitGroupingSymbol: { symbol: ',' }
    },
    {
        type: 'comma',
        format: '1234,56',
        decimalSeparator: { symbol: ',' },
        digitGroupingSymbol: undefined
    }
];

class MockImportTransactionDataMapping {
    includeHeader = true;
    dataColumnMapping: Record<number, number> = {};
    transactionTypeMapping: Record<string, number> = {};
    timeFormat = '';
    timezoneFormat = '';
    amountFormat = '';
    geoLocationSeparator = ' ';
    geoLocationOrder = 'lonlat';
    tagSeparator = ';';

    static createEmpty(): MockImportTransactionDataMapping {
        return new MockImportTransactionDataMapping();
    }

    static parseFromJson(content: string): MockImportTransactionDataMapping | null {
        if (content !== 'valid mapping') return null;
        const result = new MockImportTransactionDataMapping();
        result.includeHeader = false;
        result.dataColumnMapping = { 1: 0, 3: 1, 8: 2 };
        result.transactionTypeMapping = { Expense: mockTransactionType.Expense };
        result.timeFormat = 'YYYY-MM-DD';
        result.amountFormat = 'dot';
        return result;
    }

    isColumnMappingSet(column: { type: number }): boolean {
        return Object.prototype.hasOwnProperty.call(this.dataColumnMapping, column.type)
            && typeof this.dataColumnMapping[column.type] === 'number';
    }

    toggleIncludeHeader(): void {
        this.includeHeader = !this.includeHeader;
    }

    toggleDataMappingColumn(columnIndex: number, columnType: number): void {
        if (this.dataColumnMapping[columnType] === columnIndex) {
            delete this.dataColumnMapping[columnType];
            return;
        }
        this.dataColumnMapping[columnType] = columnIndex;
        for (const key of Object.keys(this.dataColumnMapping)) {
            if (Number(key) !== columnType && this.dataColumnMapping[Number(key)] === columnIndex) {
                delete this.dataColumnMapping[Number(key)];
            }
        }
    }

    setGeoLocationFormat(separator: string, order: string): void {
        this.geoLocationSeparator = separator;
        this.geoLocationOrder = order;
    }

    formatGeoLocation(latitude: string, longitude: string): string {
        return this.geoLocationOrder === 'latlon'
            ? `${latitude}${this.geoLocationSeparator}${longitude}`
            : `${longitude}${this.geoLocationSeparator}${latitude}`;
    }

    parseFileAllTransactionTypes(fileData: string[][] | undefined): string[] {
        if (!fileData || !this.isColumnMappingSet(mockColumnType.TransactionType)) return [];
        const start = this.includeHeader ? 1 : 0;
        const columnIndex = this.dataColumnMapping[mockColumnType.TransactionType.type]!;
        return [...new Set(fileData.slice(start).map(row => row?.[columnIndex]).filter(Boolean))] as string[];
    }

    parseFileValidMappedTransactionTypes(): Record<string, number> {
        return Object.fromEntries(Object.entries(this.transactionTypeMapping).filter(([, value]) => (
            mockTransactionType.ModifyBalance <= value && value <= mockTransactionType.Investment
        )));
    }

    parseFileAutoDetectedTimeFormat(): string | undefined {
        return mockDetectedTimeFormat;
    }

    parseFileAutoDetectedTimezoneFormat(): string | undefined {
        return mockDetectedTimezoneFormat;
    }

    parseFileAutoDetectedAmountFormat(): string | undefined {
        return mockDetectedAmountFormat;
    }

    reset(): void {
        Object.assign(this, new MockImportTransactionDataMapping());
    }

    toJson(): string {
        return JSON.stringify({ includeHeader: this.includeHeader, dataColumnMapping: this.dataColumnMapping });
    }
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        }
    };
});

jest.mock('@/components/desktop/SnackBar.vue', () => ({ __esModule: true, default: {} }));
jest.mock('@/components/desktop/PaginationButtons.vue', () => ({ __esModule: true, default: {} }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => `n:${value}` }),
        getAllImportTransactionColumnTypes: () => Object.values(mockColumnType).map(value => ({
            type: value.type,
            displayName: `column:${value.name}`
        }))
    })
}));
jest.mock('@/core/import_transaction.ts', () => ({
    ImportTransactionColumnType: mockColumnType,
    ImportTransactionDataMapping: MockImportTransactionDataMapping
}));
jest.mock('@/core/transaction.ts', () => ({ TransactionType: mockTransactionType }));
jest.mock('@/core/numeral.ts', () => ({
    KnownAmountFormat: {
        values: () => mockAmountFormats,
        valueOf: (type: string) => mockAmountFormats.find(format => format.type === type)
    }
}));
jest.mock('@/core/datetime.ts', () => ({
    KnownDateTimeFormat: {
        values: () => [{ format: 'YYYY-MM-DD' }, { format: 'MM/DD/YYYY' }]
    }
}));
jest.mock('@/core/timezone.ts', () => ({
    KnownDateTimezoneFormat: {
        values: () => [{ name: 'Colon offset', value: 'Z' }, { name: 'Compact offset', value: 'ZZ' }],
        valueOf: (value: string) => ({ Z: { name: 'Colon offset', value: 'Z' }, ZZ: { name: 'Compact offset', value: 'ZZ' } } as any)[value]
    }
}));
jest.mock('@/core/file.ts', () => ({
    KnownFileType: {
        JSON: {
            contentType: 'application/json',
            formatFileName: (name: string) => `${name}.json`,
            createBlob: (content: string) => ({ content, type: 'application/json' })
        }
    }
}));
jest.mock('@/core/base.ts', () => ({
    itemAndIndex: (values: unknown[]) => values.map((value, index) => [value, index]),
    entries: (value: Record<string, unknown>) => Object.entries(value)
}));
jest.mock('@/lib/common.ts', () => ({
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value),
    isObjectEmpty: (value: Record<string, unknown> | undefined) => !value || Object.keys(value).length === 0,
    getObjectOwnFieldCount: (value: Record<string, unknown> | undefined) => value ? Object.keys(value).length : 0,
    findDisplayNameByType: (values: Array<{ type: number; displayName: string }>, type: number) => (
        values.find(value => value.type === type)?.displayName
    )
}));
jest.mock('@/lib/ui/common.ts', () => ({
    openTextFileContent: (...args: any[]) => mockOpenTextFileContent(...args),
    startDownloadFile: (...args: any[]) => mockStartDownloadFile(...args)
}));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

const ImportTransactionDefineColumnTab = require(
    '@/views/desktop/transactions/import/tabs/ImportTransactionDefineColumnTab.vue'
).default as any;

const sampleData = [
    ['Date', 'Kind', 'Amount', 'Timezone', 'Location', 'Tags'],
    ['2026-07-15', 'Expense', '1,234.56', '+08:00', '31.2,121.5', 'food;work'],
    ['2026-07-16', 'Income', '5.00', '+08:00', '30.2,120.1', 'salary']
];

function setup(overrides: Record<string, unknown> = {}): { bindings: any; exposed: Record<string, any> } {
    const exposed: Record<string, any> = {};
    const bindings = ImportTransactionDefineColumnTab.setup({
        parsedFileData: sampleData,
        disabled: false,
        ...overrides
    }, {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: (value: Record<string, any>) => Object.assign(exposed, value)
    });
    return { bindings, exposed };
}

function installSnackbar(): { showError: jest.Mock } {
    const value = { showError: jest.fn() };
    mockTemplateRefs.get('snackbar')!.value = value;
    return value;
}

async function flushAsync(): Promise<void> {
    await Promise.resolve();
    await new Promise(resolve => setImmediate(resolve));
}

function visitVNode(node: any, handlers: Array<(...args: any[]) => unknown>, slotProps: any): void {
    if (!node) return;
    if (Array.isArray(node)) {
        for (const child of node) visitVNode(child, handlers, slotProps);
        return;
    }
    if (typeof node !== 'object') return;
    for (const [name, handler] of Object.entries(node.props || {})) {
        if (!name.startsWith('on')) continue;
        if (typeof handler === 'function') handlers.push(handler as (...args: any[]) => unknown);
        if (Array.isArray(handler)) handlers.push(...handler.filter(value => typeof value === 'function') as Array<(...args: any[]) => unknown>);
    }
    if (Array.isArray(node.children)) {
        visitVNode(node.children, handlers, slotProps);
    } else if (node.children && typeof node.children === 'object') {
        for (const child of Object.values(node.children)) {
            if (typeof child === 'function') {
                try {
                    visitVNode((child as (value?: unknown) => unknown)(slotProps), handlers, slotProps);
                } catch {
                    // Generated slots have heterogeneous framework-owned contracts.
                }
            } else {
                visitVNode(child, handlers, slotProps);
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    mockDetectedTimeFormat = undefined;
    mockDetectedTimezoneFormat = undefined;
    mockDetectedAmountFormat = undefined;
});

describe('ImportTransactionDefineColumnTab production-loaded state', () => {
    test('derives headers, rows, table sizing, menus, separators, and mapping labels', () => {
        const { bindings, exposed } = setup();
        expect(Object.keys(exposed).sort()).toStrictEqual([
            'applyFieldMappings', 'generateResult', 'loadColumnMappingFile', 'menus', 'reset', 'saveColumnMappingFile'
        ]);
        expect(bindings.menus.value.map((menu: any) => menu.title)).toStrictEqual([
            'tt:Load Data Mapping File', 'tt:Save Data Mapping File'
        ]);
        expect(bindings.allSeparators.value.map((item: any) => item.value)).toStrictEqual([' ', ',', ';', '\t', '|']);
        expect(bindings.parsedFileLinesHeaders.value).toEqual(expect.arrayContaining([
            expect.objectContaining({ key: 'index', title: '#' }),
            expect.objectContaining({ key: '0', title: 'Date' }),
            expect.objectContaining({ key: '5', title: 'Tags' })
        ]));
        expect(bindings.parsedFileLines.value).toStrictEqual([
            expect.objectContaining({ index: '1', column1: '2026-07-15', column3: '1,234.56' }),
            expect.objectContaining({ index: '2', column2: 'Income', column6: 'salary' })
        ]);
        expect(bindings.parsedFileLinesTableHeight.value).toBeUndefined();
        expect(bindings.getDisplayCount(25)).toBe('n:25');
        expect(bindings.getTablePageOptions()).toStrictEqual([{ value: -1, name: 'tt:All' }]);
        expect(bindings.getTablePageOptions(4)).toStrictEqual([{ value: -1, name: 'tt:All' }]);
        expect(bindings.getTablePageOptions(5)).toStrictEqual([
            { value: 5, name: 'n:5' }, { value: -1, name: 'tt:All' }
        ]);
        expect(bindings.getTablePageOptions(100).map((item: any) => item.value)).toStrictEqual([5, 10, 15, 20, 25, 30, 50, -1]);
        expect(bindings.getParseDataMappedColumnDisplayName(0)).toBe('tt:Unspecified');
        bindings.parsedFileDataColumnMapping.value.dataColumnMapping = { 1: 0 };
        expect(bindings.getParseDataMappedColumnDisplayName(0)).toBe('column:Transaction Time');
        expect(bindings.getParseDataMappedColumnDisplayName(1)).toBe('tt:Unspecified');
        bindings.parsedFileDataColumnMapping.value.dataColumnMapping = { 99: 0 };
        expect(bindings.getParseDataMappedColumnDisplayName(0)).toBe('tt:Unspecified');
        bindings.parsedFileDataColumnMapping.value.includeHeader = false;
        expect(bindings.parsedFileLines.value).toHaveLength(3);

        const manyRows = setup({ parsedFileData: [sampleData[0], ...Array.from({ length: 12 }, (_, index) => [String(index)])] });
        manyRows.bindings.countPerPage.value = 15;
        expect(manyRows.bindings.parsedFileLinesTableHeight.value).toBe(400);
        manyRows.bindings.countPerPage.value = 10;
        expect(manyRows.bindings.parsedFileLinesTableHeight.value).toBeUndefined();

        const empty = setup({ parsedFileData: undefined });
        expect(empty.bindings.parsedFileLines.value).toBeUndefined();
        expect(empty.bindings.parsedFileLinesHeaders.value).toStrictEqual([
            { key: 'index', value: 'index', title: '#', sortable: true, nowrap: true }
        ]);
    });

    test('validates mappings and emits a canonical result from explicit or detected formats', () => {
        const { bindings } = setup();
        const snackbar = installSnackbar();
        expect(bindings.generateResult()).toBeUndefined();
        expect(snackbar.showError).toHaveBeenLastCalledWith('Missing transaction time, transaction type, or amount column mapping');

        const mapping = bindings.parsedFileDataColumnMapping.value as MockImportTransactionDataMapping;
        mapping.dataColumnMapping = { 1: 0, 3: 1, 8: 2 };
        expect(bindings.generateResult()).toBeUndefined();
        expect(snackbar.showError).toHaveBeenLastCalledWith('Transaction type mapping is not set');

        mapping.transactionTypeMapping = { Expense: mockTransactionType.Expense, Invalid: 99 };
        expect(bindings.generateResult()).toBeUndefined();
        expect(snackbar.showError).toHaveBeenLastCalledWith('Transaction time format is not set');

        mapping.timeFormat = 'YYYY-MM-DD';
        mapping.amountFormat = 'missing';
        expect(bindings.generateResult()).toBeUndefined();
        expect(snackbar.showError).toHaveBeenLastCalledWith('Transaction amount format is not set');

        mapping.amountFormat = 'grouped';
        mapping.timezoneFormat = 'Z';
        mapping.geoLocationSeparator = ',';
        mapping.geoLocationOrder = 'latlon';
        mapping.tagSeparator = '|';
        expect(bindings.generateResult()).toStrictEqual({
            includeHeader: true,
            columnMapping: { 1: 0, 3: 1, 8: 2 },
            transactionTypeMapping: { Expense: mockTransactionType.Expense },
            timeFormat: 'YYYY-MM-DD',
            timezoneFormat: 'Z',
            amountDecimalSeparator: '.',
            amountDigitGroupingSymbol: ',',
            geoLocationSeparator: ',',
            geoLocationOrder: 'latlon',
            tagSeparator: '|'
        });

        mockDetectedTimeFormat = 'MM/DD/YYYY';
        mockDetectedTimezoneFormat = 'ZZ';
        mockDetectedAmountFormat = 'dot';
        const auto = setup().bindings;
        installSnackbar();
        auto.parsedFileDataColumnMapping.value.dataColumnMapping = { 1: 0, 3: 1, 8: 2 };
        auto.parsedFileDataColumnMapping.value.transactionTypeMapping = { Expense: mockTransactionType.Expense };
        expect(auto.generateResult()).toEqual(expect.objectContaining({
            timeFormat: 'MM/DD/YYYY',
            timezoneFormat: 'ZZ',
            amountDecimalSeparator: '.',
            amountDigitGroupingSymbol: undefined
        }));
    });

    test('applies persisted mappings, resolves amount symbols, and resets pagination', () => {
        const { bindings } = setup();
        const original = bindings.parsedFileDataColumnMapping.value;
        bindings.applyFieldMappings(undefined);
        expect(bindings.parsedFileDataColumnMapping.value).toBe(original);

        bindings.applyFieldMappings({
            includeHeader: false,
            columnMapping: { 1: 0, 3: 1, 8: 2 },
            transactionTypeMapping: { Income: mockTransactionType.Income },
            timeFormat: 'YYYY-MM-DD',
            timezoneFormat: 'Z',
            amountDecimalSeparator: '.',
            amountDigitGroupingSymbol: ',',
            geoLocationSeparator: ',',
            geoLocationOrder: 'latlon',
            tagSeparator: '|'
        });
        expect(bindings.parsedFileDataColumnMapping.value).toEqual(expect.objectContaining({
            includeHeader: false,
            amountFormat: 'grouped',
            geoLocationSeparator: ',',
            geoLocationOrder: 'latlon',
            tagSeparator: '|'
        }));

        bindings.applyFieldMappings({ amountDecimalSeparator: '.', amountDigitGroupingSymbol: '!' });
        expect(bindings.parsedFileDataColumnMapping.value.amountFormat).toBe('');
        bindings.applyFieldMappings({ amountDecimalSeparator: '.' });
        expect(bindings.parsedFileDataColumnMapping.value.amountFormat).toBe('dot');
        bindings.applyFieldMappings({ includeHeader: true });
        expect(bindings.parsedFileDataColumnMapping.value).toEqual(expect.objectContaining({
            includeHeader: true,
            dataColumnMapping: {},
            transactionTypeMapping: {},
            timeFormat: '',
            timezoneFormat: ''
        }));

        bindings.currentPage.value = 4;
        bindings.countPerPage.value = 50;
        bindings.parsedFileDataColumnMapping.value.includeHeader = false;
        bindings.reset();
        expect(bindings.currentPage.value).toBe(1);
        expect(bindings.countPerPage.value).toBe(10);
        expect(bindings.parsedFileDataColumnMapping.value.includeHeader).toBe(true);
    });
});

describe('ImportTransactionDefineColumnTab file actions and template', () => {
    test('loads valid files and reports parse or read failures', async () => {
        const { bindings } = setup();
        const snackbar = installSnackbar();
        mockOpenTextFileContent.mockResolvedValueOnce('valid mapping');
        bindings.loadColumnMappingFile();
        await flushAsync();
        expect(mockOpenTextFileContent).toHaveBeenCalledWith({ allowedExtensions: 'application/json' });
        expect(bindings.parsedFileDataColumnMapping.value).toEqual(expect.objectContaining({
            includeHeader: false,
            amountFormat: 'dot'
        }));

        mockOpenTextFileContent.mockResolvedValueOnce('invalid mapping');
        bindings.loadColumnMappingFile();
        await flushAsync();
        expect(mockLogger.error).toHaveBeenCalledWith('Failed to parse data mapping file');
        expect(snackbar.showError).toHaveBeenLastCalledWith('Data mapping file is invalid');

        mockOpenTextFileContent.mockRejectedValueOnce(new Error('read failed'));
        bindings.loadColumnMappingFile();
        await flushAsync();
        expect(mockLogger.error).toHaveBeenCalledWith('Failed to open data mapping file', expect.any(Error));
        expect(snackbar.showError).toHaveBeenLastCalledWith('Data mapping file is invalid');
    });

    test('downloads the current mapping with the localized default name', () => {
        const { bindings } = setup();
        bindings.parsedFileDataColumnMapping.value.includeHeader = false;
        bindings.saveColumnMappingFile();
        expect(mockStartDownloadFile).toHaveBeenCalledWith(
            'tt:dataExport.defaultImportDataMappingFileName.json',
            expect.objectContaining({ content: expect.stringContaining('"includeHeader":false') })
        );
    });

    test('executes generated template slots and handlers across configured and empty states', async () => {
        const { proxyRefs } = jest.requireActual('vue') as any;
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const scenarios: Array<{
                kind: string;
                parsedFileData?: string[][];
                time?: string;
                timezone?: string;
                amount?: string;
            }> = [
                { kind: 'empty', parsedFileData: undefined },
                { kind: 'unconfigured', parsedFileData: sampleData },
                { kind: 'auto', parsedFileData: sampleData, time: 'MM/DD/YYYY', timezone: 'ZZ', amount: 'dot' },
                { kind: 'invalid-auto', parsedFileData: sampleData, time: undefined, timezone: 'invalid', amount: 'invalid' },
                { kind: 'configured', parsedFileData: sampleData }
            ];
            for (const scenario of scenarios) {
                mockDetectedTimeFormat = scenario.time;
                mockDetectedTimezoneFormat = scenario.timezone;
                mockDetectedAmountFormat = scenario.amount;
                const { bindings } = setup({ parsedFileData: scenario.parsedFileData });
                const mapping = bindings.parsedFileDataColumnMapping.value as MockImportTransactionDataMapping;
                if (scenario.kind === 'auto' || scenario.kind === 'invalid-auto') {
                    mapping.includeHeader = false;
                    mapping.dataColumnMapping = { 1: 0, 2: 3, 3: 1, 8: 2 };
                } else if (scenario.kind === 'configured') {
                    mapping.dataColumnMapping = { 1: 0, 2: 3, 3: 1, 8: 2, 12: 4, 13: 5 };
                    mapping.transactionTypeMapping = { Expense: mockTransactionType.Expense };
                    mapping.timeFormat = 'YYYY-MM-DD';
                    mapping.timezoneFormat = 'Z';
                    mapping.amountFormat = 'grouped';
                    mapping.geoLocationSeparator = ',';
                    mapping.geoLocationOrder = 'latlon';
                    mapping.tagSeparator = '|';
                    bindings.countPerPage.value = 15;
                }
                const vnode = ImportTransactionDefineColumnTab.render(
                    {}, [], { parsedFileData: scenario.parsedFileData, disabled: scenario.kind !== 'configured' }, proxyRefs(bindings), {}, {}
                );
                const handlers: Array<(...args: any[]) => unknown> = [];
                visitVNode(vnode, handlers, {
                    columns: [
                        { key: undefined, title: 'No key' },
                        { key: null, title: 'Null key' },
                        ...bindings.parsedFileLinesHeaders.value
                    ]
                });
                expect(handlers.length).toBeGreaterThan(1);
                for (const handler of handlers) {
                    try {
                        await handler({ target: { value: 1 } });
                    } catch {
                        // Generated v-model handlers intentionally accept heterogeneous scalar values.
                    }
                }
            }
        } finally {
            warnSpy.mockRestore();
        }
    });
});
