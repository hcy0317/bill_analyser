import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockTemplateRefs = new Map<string, any>();
const mockMountedCallbacks: Array<() => void> = [];
const mockUnmountedCallbacks: Array<() => void> = [];
const mockOpenTextFileContent = jest.fn<(...args: any[]) => Promise<string>>();
const mockStartDownloadFile = jest.fn();
const mockGetBrowserTimezoneOffsetMinutes = jest.fn(() => 480);
const mockLogger = { error: jest.fn() };
const mockWindowAddEventListener = jest.fn();
const mockWindowRemoveEventListener = jest.fn();

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => void) => mockMountedCallbacks.push(callback),
        onUnmounted: (callback: () => void) => mockUnmountedCallbacks.push(callback),
        useTemplateRef: (name: string) => {
            const target = actual.shallowRef(null);
            mockTemplateRefs.set(name, target);
            return target;
        }
    };
});

jest.mock('@/components/desktop/SnackBar.vue', () => ({ __esModule: true, default: {} }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, args?: Record<string, unknown>) => args
            ? `tt:${key}:${JSON.stringify(args)}`
            : `tt:${key}`,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => `n:${value}` })
    })
}));
jest.mock('@/core/file.ts', () => ({
    KnownFileType: {
        JS: {
            contentType: 'text/javascript',
            formatFileName: (name: string) => `${name}.js`,
            createBlob: (content: string) => ({ content, type: 'text/javascript' })
        }
    }
}));
jest.mock('@/lib/common.ts', () => ({
    isDefined: (value: unknown) => value !== undefined && value !== null
}));
jest.mock('@/lib/datetime.ts', () => ({
    getBrowserTimezoneOffsetMinutes: () => mockGetBrowserTimezoneOffsetMinutes()
}));
jest.mock('@/lib/ui/common.ts', () => ({
    openTextFileContent: (...args: any[]) => mockOpenTextFileContent(...args),
    startDownloadFile: (...args: any[]) => mockStartDownloadFile(...args)
}));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

const ExecuteCustomScriptTab = require(
    '@/views/desktop/transactions/import/tabs/ImportTransactionExecuteCustomScriptTab.vue'
).default as any;

const parsedFileData = [
    ['Time', 'Type', 'Amount'],
    ['2026-07-15 10:30', 'Expense', '12345'],
    ['2026-07-16 11:45', 'Income', '67890']
];

function setup(overrides: Record<string, unknown> = {}): {
    bindings: any;
    exposed: any;
} {
    const exposed: any = {};
    const bindings = ExecuteCustomScriptTab.setup({
        parsedFileData,
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

function installSandbox(bindings: any): {
    frame: any;
    postMessage: jest.Mock;
} {
    const postMessage = jest.fn();
    const frame = {
        src: '',
        srcdoc: '',
        onload: undefined as (() => void) | undefined,
        contentWindow: { postMessage }
    };
    mockTemplateRefs.get('sandbox')!.value = frame;
    expect(bindings.sandbox).toBe(mockTemplateRefs.get('sandbox'));
    return { frame, postMessage };
}

function installSnackbar(bindings: any): { showError: jest.Mock } {
    const snackbar = { showError: jest.fn() };
    mockTemplateRefs.get('snackbar')!.value = snackbar;
    expect(bindings.snackbar).toBe(mockTemplateRefs.get('snackbar'));
    return snackbar;
}

async function flushAsync(): Promise<void> {
    await Promise.resolve();
    await new Promise(resolve => setImmediate(resolve));
}

function visitVNode(
    node: any,
    handlers: Array<(...args: any[]) => unknown>,
    slotProps: Record<string, unknown> = {}
): void {
    if (!node) return;
    if (Array.isArray(node)) {
        for (const child of node) visitVNode(child, handlers, slotProps);
        return;
    }
    if (typeof node !== 'object') return;
    for (const [name, value] of Object.entries(node.props || {})) {
        if (!name.startsWith('on')) continue;
        for (const handler of Array.isArray(value) ? value : [value]) {
            if (typeof handler === 'function') handlers.push(handler as (...args: any[]) => unknown);
        }
    }
    if (Array.isArray(node.children)) {
        visitVNode(node.children, handlers, slotProps);
    } else if (node.children && typeof node.children === 'object') {
        for (const child of Object.values(node.children)) {
            if (typeof child === 'function') {
                try {
                    visitVNode((child as (value?: unknown) => unknown)(slotProps), handlers, slotProps);
                } catch {
                    // Vuetify-generated slots accept heterogeneous framework-owned arguments.
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
    mockMountedCallbacks.length = 0;
    mockUnmountedCallbacks.length = 0;
    mockOpenTextFileContent.mockResolvedValue('function parse() { return null; }');
    (window as any).addEventListener = mockWindowAddEventListener;
    (window as any).removeEventListener = mockWindowRemoveEventListener;
});

describe('ImportTransactionExecuteCustomScriptTab production-loaded state', () => {
    test('initializes the localized sample, sandbox protocol, lifecycle listeners, and exposed contract', () => {
        const { bindings, exposed } = setup();
        bindings.reloadSandbox();
        expect(bindings.sandboxLoaded.value).toBe(false);
        const { frame } = installSandbox(bindings);

        expect(Object.keys(exposed).sort()).toStrictEqual(['generateResult', 'menus', 'reset']);
        expect(bindings.customScript.value).toBe('');
        expect(mockMountedCallbacks).toHaveLength(1);
        expect(mockUnmountedCallbacks).toHaveLength(1);

        mockMountedCallbacks[0]!();
        expect(bindings.customScript.value).toContain('// tt:sample.importTransactionCustomScript.headerComment');
        expect(bindings.customScript.value).toContain("utcOffset: '480'");
        expect(bindings.customScript.value).toContain('sourceAmountCents: row[7]');
        expect(mockGetBrowserTimezoneOffsetMinutes).toHaveBeenCalled();
        expect(frame.src).toBe('about:blank');
        expect(frame.srcdoc).toContain("window.TransactionType = {");
        expect(frame.srcdoc).toContain("window.addEventListener('message'");
        expect(frame.srcdoc).toContain('eval(data.code)');
        expect(frame.srcdoc).toContain("knownError: 'No parse function defined'");
        expect(frame.srcdoc).toContain("window.parent.postMessage({ error: error.message }, '*')");
        expect(bindings.sandboxLoaded.value).toBe(false);
        frame.onload?.();
        expect(bindings.sandboxLoaded.value).toBe(true);
        expect(mockWindowAddEventListener).toHaveBeenCalledWith('message', bindings.onMessage);

        mockUnmountedCallbacks[0]!();
        expect(mockWindowRemoveEventListener).toHaveBeenCalledWith('message', bindings.onMessage);
    });

    test('builds preview options and selects each display state deterministically', () => {
        const { bindings } = setup();
        expect(bindings.numeralSystem.value.formatNumber(12)).toBe('n:12');
        expect(bindings.getDisplayCount(50)).toBe('n:50');
        expect(bindings.getTablePageOptions()).toStrictEqual([{ value: -1, name: 'tt:All' }]);
        expect(bindings.getTablePageOptions(0)).toStrictEqual([{ value: -1, name: 'tt:All' }]);
        expect(bindings.getTablePageOptions(9)).toStrictEqual([{ value: -1, name: 'tt:All' }]);
        expect(bindings.getTablePageOptions(10)).toStrictEqual([
            { value: 10, name: 'n:10' },
            { value: -1, name: 'tt:All' }
        ]);
        expect(bindings.getTablePageOptions(75).map((item: any) => item.value)).toStrictEqual([10, 50, -1]);
        expect(bindings.getTablePageOptions(100).map((item: any) => item.value)).toStrictEqual([10, 50, 100, -1]);

        expect(bindings.displayPreviewResult.value).toBe('tt:No Preview Result');
        bindings.previewResult.value = [
            { time: 'first', utcOffset: '480', type: 'Expense', sourceAmountCents: '12345' },
            { time: 'second', utcOffset: '480', type: 'Income', sourceAmountCents: '67890' }
        ];
        bindings.previewCount.value = 1;
        expect(JSON.parse(bindings.displayPreviewResult.value)).toStrictEqual([
            { time: 'first', utcOffset: '480', type: 'Expense', sourceAmountCents: '12345' }
        ]);
        bindings.executionError.value = 'syntax failed';
        expect(bindings.displayPreviewResult.value).toBe('syntax failed');
        bindings.executingScript.value = true;
        expect(bindings.displayPreviewResult.value).toBe('tt:Executing Script...');
    });

    test('shows every preview row when the selected count is All', () => {
        const { bindings } = setup();
        const previewRows = [
            { time: 'first', utcOffset: '480', type: 'Expense', sourceAmountCents: '12345' },
            { time: 'second', utcOffset: '480', type: 'Income', sourceAmountCents: '67890' }
        ];
        bindings.previewResult.value = previewRows;
        bindings.previewCount.value = -1;

        expect(JSON.parse(bindings.displayPreviewResult.value)).toStrictEqual(previewRows);
    });

    test('sends only the sandbox request and applies all execution guards', () => {
        const missing = setup();
        missing.bindings.customScript.value = 'function parse() { return {}; }';
        missing.bindings.executeCustomScript();

        const disabled = setup({ disabled: true });
        const disabledSandbox = installSandbox(disabled.bindings);
        disabled.bindings.executeCustomScript();
        expect(disabledSandbox.postMessage).not.toHaveBeenCalled();

        const busy = setup();
        const busySandbox = installSandbox(busy.bindings);
        busy.bindings.executingScript.value = true;
        busy.bindings.executeCustomScript();
        expect(busySandbox.postMessage).not.toHaveBeenCalled();

        const noWindow = setup();
        noWindow.bindings.sandbox.value = { contentWindow: null };
        noWindow.bindings.executeCustomScript();
        expect(noWindow.bindings.executingScript.value).toBe(true);

        const active = setup();
        const activeSandbox = installSandbox(active.bindings);
        active.bindings.customScript.value = 'function parse(row) { return { time: row[0] }; }';
        active.bindings.executeCustomScript();
        expect(active.bindings.executingScript.value).toBe(true);
        expect(activeSandbox.postMessage).toHaveBeenCalledTimes(1);
        const [serialized, targetOrigin] = activeSandbox.postMessage.mock.calls[0]!;
        expect(targetOrigin).toBe('*');
        expect(JSON.parse(String(serialized))).toStrictEqual({
            parsedFileData,
            code: expect.stringContaining("if (typeof parse !== 'undefined') { window.parse = parse; }")
        });

        const empty = setup({ parsedFileData: undefined });
        const emptySandbox = installSandbox(empty.bindings);
        empty.bindings.executeCustomScript();
        expect(JSON.parse(String(emptySandbox.postMessage.mock.calls[0]![0])).parsedFileData).toStrictEqual([]);
    });

    test('projects successful sandbox results into the canonical request without changing cents', () => {
        const { bindings, exposed } = setup();
        const { frame } = installSandbox(bindings);
        const snackbar = installSnackbar(bindings);
        bindings.executingScript.value = true;

        const originalResult = [
            {
                time: '2026-07-15 10:30',
                utcOffset: 480,
                type: 'Expense',
                categoryName: 'Food',
                sourceAccountName: 'Wallet',
                destinationAccountName: 'Merchant',
                sourceAmountCents: -12345,
                destinationAmountCents: 12345,
                geoLocation: '31.2,121.5',
                tagNames: 'food;work',
                description: 'Lunch'
            },
            {
                time: 0,
                utcOffset: '',
                type: 0,
                categoryName: '',
                sourceAccountName: '',
                destinationAccountName: '',
                sourceAmountCents: 0,
                destinationAmountCents: '',
                geoLocation: 0,
                tagNames: '',
                description: false
            },
            {}
        ];
        bindings.onMessage({
            source: frame.contentWindow,
            data: { result: JSON.stringify(originalResult) }
        } as any);

        expect(bindings.executingScript.value).toBe(false);
        expect(bindings.executionError.value).toBe('');
        expect(bindings.previewResult.value).toStrictEqual([
            {
                time: '2026-07-15 10:30',
                utcOffset: '480',
                type: 'Expense',
                categoryName: 'Food',
                sourceAccountName: 'Wallet',
                destinationAccountName: 'Merchant',
                sourceAmountCents: '-12345',
                destinationAmountCents: '12345',
                geoLocation: '31.2,121.5',
                tagNames: 'food;work',
                comment: 'Lunch'
            },
            {
                time: '0',
                utcOffset: '',
                type: '0',
                categoryName: undefined,
                sourceAccountName: undefined,
                destinationAccountName: undefined,
                sourceAmountCents: '0',
                destinationAmountCents: undefined,
                geoLocation: undefined,
                tagNames: undefined,
                comment: undefined
            },
            {
                time: '',
                utcOffset: '',
                type: '',
                categoryName: undefined,
                sourceAccountName: undefined,
                destinationAccountName: undefined,
                sourceAmountCents: '',
                destinationAmountCents: undefined,
                geoLocation: undefined,
                tagNames: undefined,
                comment: undefined
            }
        ]);
        expect(snackbar.showError).not.toHaveBeenCalled();
        expect(exposed.generateResult()).toBe(JSON.stringify({
            transactions: bindings.previewResult.value
        }));
        expect(frame.srcdoc).toContain('eval(data.code)');
    });

    test('contains malformed sandbox result payloads as execution failures', () => {
        const { bindings } = setup();
        const { frame } = installSandbox(bindings);
        const snackbar = installSnackbar(bindings);
        bindings.executingScript.value = true;
        bindings.previewResult.value = [{
            time: 'old', utcOffset: '480', type: 'Expense', sourceAmountCents: '12345'
        }];

        expect(() => bindings.onMessage({
            source: frame.contentWindow,
            data: { result: 'not-json' }
        } as any)).not.toThrow();
        expect(bindings.executingScript.value).toBe(false);
        expect(bindings.previewResult.value).toBeUndefined();
        expect(bindings.executionError.value).not.toBe('');
        expect(mockLogger.error).toHaveBeenCalled();
        expect(snackbar.showError).toHaveBeenCalledWith('Failed to execute custom script');
    });

    test('rejects foreign messages and reports known, runtime, and empty responses', () => {
        const foreign = setup();
        installSandbox(foreign.bindings);
        foreign.bindings.executingScript.value = true;
        foreign.bindings.onMessage({ source: {}, data: { knownError: 'foreign' } } as any);
        expect(foreign.bindings.executingScript.value).toBe(true);

        const known = setup();
        const knownSandbox = installSandbox(known.bindings);
        const knownSnackbar = installSnackbar(known.bindings);
        known.bindings.previewResult.value = [{ time: 'old' }];
        known.bindings.executingScript.value = true;
        known.bindings.onMessage({
            source: knownSandbox.frame.contentWindow,
            data: { knownError: 'No parse function defined' }
        } as any);
        expect(known.bindings.executingScript.value).toBe(false);
        expect(known.bindings.previewResult.value).toBeUndefined();
        expect(known.bindings.executionError.value).toBe('tt:No parse function defined');
        expect(knownSnackbar.showError).toHaveBeenCalledWith('No parse function defined');

        const runtime = setup();
        const runtimeSandbox = installSandbox(runtime.bindings);
        const runtimeSnackbar = installSnackbar(runtime.bindings);
        runtime.bindings.previewResult.value = [{ time: 'old' }];
        runtime.bindings.onMessage({
            source: runtimeSandbox.frame.contentWindow,
            data: { error: 'row 2 failed' }
        } as any);
        expect(runtime.bindings.previewResult.value).toBeUndefined();
        expect(runtime.bindings.executionError.value).toBe('row 2 failed');
        expect(mockLogger.error).toHaveBeenCalledWith('Failed to execute custom script: row 2 failed');
        expect(runtimeSnackbar.showError).toHaveBeenCalledWith('Failed to execute custom script');

        const empty = setup();
        const emptySandbox = installSandbox(empty.bindings);
        empty.bindings.executingScript.value = true;
        empty.bindings.onMessage({ source: emptySandbox.frame.contentWindow, data: {} } as any);
        expect(empty.bindings.executingScript.value).toBe(false);
        expect(empty.bindings.previewResult.value).toBeUndefined();
        expect(empty.bindings.executionError.value).toBe('');
    });

    test('generates only completed results and resets transient state', () => {
        const { bindings, exposed } = setup();
        const snackbar = installSnackbar(bindings);
        expect(exposed.generateResult()).toBeUndefined();
        expect(snackbar.showError).toHaveBeenCalledWith('Please execute the custom script first');

        bindings.customScript.value = 'changed';
        bindings.previewResult.value = [{
            time: '2026-07-15',
            utcOffset: '480',
            type: 'Expense',
            sourceAmountCents: '-12345'
        }];
        bindings.executionError.value = 'old error';
        bindings.executingScript.value = true;
        bindings.previewCount.value = -1;
        exposed.reset();
        expect(bindings.customScript.value).toContain('function parse(row, index)');
        expect(bindings.previewResult.value).toBeUndefined();
        expect(bindings.executionError.value).toBe('');
        expect(bindings.executingScript.value).toBe(false);
        expect(bindings.previewCount.value).toBe(10);
    });
});

describe('ImportTransactionExecuteCustomScriptTab file and template actions', () => {
    test('loads and saves JavaScript text while keeping failures inside the UI boundary', async () => {
        const { bindings, exposed } = setup();
        const snackbar = installSnackbar(bindings);
        expect(exposed.menus.value.map((menu: any) => menu.title)).toStrictEqual([
            'tt:Load Script File',
            'tt:Save Script File'
        ]);

        mockOpenTextFileContent.mockResolvedValueOnce('function parse() { return null; }');
        exposed.menus.value[0].onClick();
        await flushAsync();
        expect(mockOpenTextFileContent).toHaveBeenCalledWith({ allowedExtensions: 'text/javascript' });
        expect(bindings.customScript.value).toBe('function parse() { return null; }');

        bindings.customScript.value = 'function parse(row) { return row; }';
        exposed.menus.value[1].onClick();
        expect(mockStartDownloadFile).toHaveBeenCalledWith(
            'tt:dataExport.defaultImportHandlingScript.js',
            { content: 'function parse(row) { return row; }', type: 'text/javascript' }
        );

        mockOpenTextFileContent.mockRejectedValueOnce(new Error('file read failed'));
        bindings.loadScriptFile();
        await flushAsync();
        expect(mockLogger.error).toHaveBeenCalledWith('Failed to load script file', expect.any(Error));
        expect(snackbar.showError).toHaveBeenCalledWith('Cannot load script file');
    });

    test('executes generated template branches and handlers without evaluating script text', async () => {
        const { proxyRefs } = actualVue;
        const scenarios: Array<{
            disabled: boolean;
            sandboxLoaded: boolean;
            executingScript: boolean;
            previewResult?: any[];
            previewCount: number;
            executionError?: string;
        }> = [
            {
                disabled: true,
                sandboxLoaded: false,
                executingScript: false,
                previewCount: 10,
                executionError: 'template error'
            },
            { disabled: false, sandboxLoaded: false, executingScript: false, previewCount: 10 },
            { disabled: false, sandboxLoaded: true, executingScript: true, previewCount: 10 },
            { disabled: false, sandboxLoaded: true, executingScript: false, previewCount: 10 },
            {
                disabled: false,
                sandboxLoaded: true,
                executingScript: false,
                previewResult: Array.from({ length: 55 }, (_, index) => ({
                    time: String(index), utcOffset: '480', type: 'Expense', sourceAmountCents: String(index)
                })),
                previewCount: -1
            }
        ];

        for (const scenario of scenarios) {
            const { bindings } = setup({ disabled: scenario.disabled });
            const { postMessage } = installSandbox(bindings);
            bindings.sandboxLoaded.value = scenario.sandboxLoaded;
            bindings.executingScript.value = scenario.executingScript;
            bindings.previewResult.value = scenario.previewResult;
            bindings.previewCount.value = scenario.previewCount;
            bindings.executionError.value = scenario.executionError ?? '';
            const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
            try {
                const vnode = ExecuteCustomScriptTab.render(
                    {}, [], { parsedFileData, disabled: scenario.disabled }, proxyRefs(bindings), {}, {}
                );
                const handlers: Array<(...args: any[]) => unknown> = [];
                visitVNode(vnode, handlers);
                expect(handlers.length).toBeGreaterThan(0);
                for (const handler of handlers) {
                    try {
                        await handler('synthetic-value');
                    } catch {
                        // Generated v-model handlers receive heterogeneous control values.
                    }
                }
            } finally {
                warnSpy.mockRestore();
            }
            expect(postMessage).toHaveBeenCalledTimes(
                scenario.disabled || scenario.executingScript ? 0 : 1
            );
        }
    });
});
