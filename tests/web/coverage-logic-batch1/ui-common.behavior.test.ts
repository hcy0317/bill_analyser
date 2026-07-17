import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockClipboardCopy = jest.fn();
const mockLogger = { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() };

jest.mock('clipboard', () => ({
    __esModule: true,
    default: { copy: mockClipboardCopy }
}));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

import { ThemeType } from '@/core/theme.ts';
import { PresetAmountColor } from '@/core/color.ts';
import {
    clearBrowserCaches,
    compressJpgImage,
    copyTextToClipboard,
    getExpenseAndIncomeAmountColor,
    getSystemTheme,
    openTextFileContent,
    setExpenseAndIncomeAmountColor,
    startDownloadFile
} from '@/lib/ui/common.ts';

class MockFileReader {
    static instances: MockFileReader[] = [];

    onload: ((event: { target?: { result?: unknown } }) => void) | null = null;
    onerror: ((error: unknown) => void) | null = null;
    readAsText = jest.fn<(_file: File) => void>();
    readAsDataURL = jest.fn<(_file: File) => void>();

    constructor() {
        MockFileReader.instances.push(this);
    }
}

class MockImage {
    static instances: MockImage[] = [];

    width = 0;
    height = 0;
    onload: (() => void) | null = null;
    onerror: ((error: unknown) => void) | null = null;
    private currentSrc = '';

    constructor() {
        MockImage.instances.push(this);
    }

    set src(value: string) {
        this.currentSrc = value;
    }

    get src(): string {
        return this.currentSrc;
    }
}

const inputElements: Array<Record<string, any>> = [];
const anchorElements: Array<Record<string, any>> = [];
const canvasElements: Array<Record<string, any>> = [];
const appendedElements: unknown[] = [];
const htmlClasses = new Set<string>();
const mockCreateObjectURL = jest.fn<(blob: Blob) => string>();

let returnHtmlElement = true;
let nextCanvasContext: { drawImage: jest.Mock } | null;
let nextCanvasBlob: Blob | null;

function classList() {
    return {
        add: jest.fn((name: string) => htmlClasses.add(name)),
        remove: jest.fn((name: string) => htmlClasses.delete(name)),
        contains: jest.fn((name: string) => htmlClasses.has(name))
    };
}

function createInput(): Record<string, any> {
    const input = {
        style: {} as Record<string, string>,
        type: '',
        accept: '',
        onchange: null as ((event: { target: unknown }) => void) | null,
        click: jest.fn()
    };
    inputElements.push(input);
    return input;
}

function createAnchor(): Record<string, any> {
    const anchor = {
        style: {} as Record<string, string>,
        href: '',
        setAttribute: jest.fn(),
        click: jest.fn()
    };
    anchorElements.push(anchor);
    return anchor;
}

function createCanvas(): Record<string, any> {
    const canvas = {
        width: 0,
        height: 0,
        getContext: jest.fn(() => nextCanvasContext),
        toBlob: jest.fn((callback: (blob: Blob | null) => void) => callback(nextCanvasBlob))
    };
    canvasElements.push(canvas);
    return canvas;
}

function installDocument(): void {
    const body = {
        appendChild: jest.fn((element: unknown) => {
            appendedElements.push(element);
            return element;
        })
    };
    const html = { classList: classList() };
    const documentMock = {
        body,
        querySelector: jest.fn(() => returnHtmlElement ? html : null),
        createElement: jest.fn((tag: string) => {
            if (tag === 'input') return createInput();
            if (tag === 'a') return createAnchor();
            if (tag === 'canvas') return createCanvas();
            return { style: {} };
        })
    };
    Object.defineProperty(globalThis, 'document', { configurable: true, writable: true, value: documentMock });
}

beforeEach(() => {
    jest.clearAllMocks();
    MockFileReader.instances.length = 0;
    MockImage.instances.length = 0;
    inputElements.length = 0;
    anchorElements.length = 0;
    canvasElements.length = 0;
    appendedElements.length = 0;
    htmlClasses.clear();
    returnHtmlElement = true;
    nextCanvasContext = { drawImage: jest.fn() };
    nextCanvasBlob = new Blob(['compressed'], { type: 'image/jpeg' });
    mockCreateObjectURL.mockReturnValue('blob:download');

    installDocument();
    Object.defineProperty(globalThis, 'FileReader', { configurable: true, writable: true, value: MockFileReader });
    Object.defineProperty(globalThis, 'Image', { configurable: true, writable: true, value: MockImage });
    Object.defineProperty(globalThis.URL, 'createObjectURL', {
        configurable: true,
        writable: true,
        value: mockCreateObjectURL
    });
    Object.defineProperty(globalThis.window, 'matchMedia', {
        configurable: true,
        writable: true,
        value: jest.fn(() => ({ matches: false }))
    });
    Object.defineProperty(globalThis.window, 'caches', {
        configurable: true,
        writable: true,
        value: undefined
    });
});

describe('theme, amount color and clipboard helpers', () => {
    test('detects dark and light system themes with and without matchMedia', () => {
        (globalThis.window.matchMedia as jest.Mock).mockReturnValueOnce({ matches: true });
        expect(getSystemTheme()).toBe(ThemeType.Dark);
        expect(getSystemTheme()).toBe(ThemeType.Light);

        Object.defineProperty(globalThis.window, 'matchMedia', { configurable: true, value: undefined });
        expect(getSystemTheme()).toBe(ThemeType.Light);
    });

    test('uses configured or default amount colors in light and dark modes', () => {
        expect(getExpenseAndIncomeAmountColor(PresetAmountColor.Yellow.type, PresetAmountColor.BlackOrWhite.type, false)).toEqual({
            expenseAmountColor: PresetAmountColor.Yellow.lightThemeColor,
            incomeAmountColor: PresetAmountColor.BlackOrWhite.lightThemeColor
        });
        expect(getExpenseAndIncomeAmountColor(999, 0, true)).toEqual({
            expenseAmountColor: PresetAmountColor.DefaultExpenseColor.darkThemeColor,
            incomeAmountColor: PresetAmountColor.DefaultIncomeColor.darkThemeColor
        });
    });

    test('updates mutually exclusive html amount-color classes and handles a missing html root', () => {
        htmlClasses.add(PresetAmountColor.Green.expenseClassName);
        htmlClasses.add(PresetAmountColor.Red.incomeClassName);

        setExpenseAndIncomeAmountColor(PresetAmountColor.Yellow.type, PresetAmountColor.BlackOrWhite.type);

        expect(htmlClasses.has(PresetAmountColor.Yellow.expenseClassName)).toBe(true);
        expect(htmlClasses.has(PresetAmountColor.BlackOrWhite.incomeClassName)).toBe(true);
        expect(htmlClasses.has(PresetAmountColor.Green.expenseClassName)).toBe(false);
        expect(htmlClasses.has(PresetAmountColor.Red.incomeClassName)).toBe(false);

        setExpenseAndIncomeAmountColor(0, 999);
        expect(htmlClasses.has(PresetAmountColor.DefaultExpenseColor.expenseClassName)).toBe(true);
        expect(htmlClasses.has(PresetAmountColor.DefaultIncomeColor.incomeClassName)).toBe(true);

        returnHtmlElement = false;
        expect(() => setExpenseAndIncomeAmountColor(1, 2)).not.toThrow();
    });

    test('copies through Clipboard with an explicit or default container', () => {
        const container = { id: 'dialog' } as unknown as Element;
        copyTextToClipboard('explicit', container);
        copyTextToClipboard('default', null);

        expect(mockClipboardCopy).toHaveBeenNthCalledWith(1, 'explicit', { container });
        expect(mockClipboardCopy).toHaveBeenNthCalledWith(2, 'default', { container: globalThis.document.body });
    });
});

describe('text file and download helpers', () => {
    test('opens a selected text file and resolves string content', async () => {
        const file = { name: 'settings.json' } as File;
        const pending = openTextFileContent({ allowedExtensions: '.json,.txt' });
        const input = inputElements[0]!;

        expect(input).toMatchObject({ type: 'file', accept: '.json,.txt' });
        expect(input['style']['display']).toBe('none');
        expect(input['click']).toHaveBeenCalled();
        input['onchange']!({ target: { files: [file] } });
        const reader = MockFileReader.instances[0]!;
        expect(reader.readAsText).toHaveBeenCalledWith(file);
        reader.onload!({ target: { result: 'file-content' } });

        await expect(pending).resolves.toBe('file-content');
    });

    test.each([
        ['non-string result', { target: { result: new ArrayBuffer(1) } }],
        ['missing result', { target: {} }]
    ])('rejects a %s from FileReader', async (_name, event) => {
        const pending = openTextFileContent({ allowedExtensions: '.txt' });
        inputElements[0]!['onchange']!({ target: { files: [{ name: 'x.txt' } as File] } });
        MockFileReader.instances[0]!.onload!(event);
        await expect(pending).rejects.toThrow('file reader result is not string');
    });

    test('ignores empty selections and logs FileReader errors', () => {
        openTextFileContent({ allowedExtensions: '.txt' });
        inputElements[0]!['onchange']!({ target: { files: [] } });
        expect(MockFileReader.instances).toHaveLength(0);

        openTextFileContent({ allowedExtensions: '.txt' });
        inputElements[1]!['onchange']!({ target: { files: [{ name: 'x.txt' } as File] } });
        MockFileReader.instances[0]!.onerror!('read failed');
        expect(mockLogger.error).toHaveBeenCalledWith('failed to load file', 'read failed');
    });

    test('creates and clicks an object-url download anchor', () => {
        const blob = new Blob(['report']);
        startDownloadFile('report.json', blob);
        const anchor = anchorElements[0]!;

        expect(mockCreateObjectURL).toHaveBeenCalledWith(blob);
        expect(anchor).toMatchObject({ href: 'blob:download' });
        expect(anchor['style']['display']).toBe('none');
        expect(anchor['setAttribute']).toHaveBeenCalledWith('download', 'report.json');
        expect(appendedElements).toEqual([anchor]);
        expect(anchor['click']).toHaveBeenCalled();
    });
});

describe('JPEG compression', () => {
    const file = { name: 'photo.jpg' } as File;

    function provideReaderResult(pending: Promise<Blob>, result: unknown = 'data:image/jpeg;base64,x') {
        const reader = MockFileReader.instances.at(-1)!;
        reader.onload!({ target: { result } });
        return { pending, reader, image: MockImage.instances.at(-1)! };
    }

    test('scales large images, draws them and resolves the compressed JPEG', async () => {
        const pending = compressJpgImage(file, 100, 100, 0.8);
        const { reader, image } = provideReaderResult(pending);
        image.width = 400;
        image.height = 200;
        image.onload!();

        const canvas = canvasElements[0]!;
        expect(reader.readAsDataURL).toHaveBeenCalledWith(file);
        expect(image.src).toBe('data:image/jpeg;base64,x');
        expect(canvas).toMatchObject({ width: 100, height: 50 });
        expect(nextCanvasContext!.drawImage).toHaveBeenCalledWith(image, 0, 0, 100, 50);
        expect(canvas['toBlob']).toHaveBeenCalledWith(expect.any(Function), 'image/jpeg', 0.8);
        await expect(pending).resolves.toBe(nextCanvasBlob);
    });

    test('keeps small dimensions without scaling', async () => {
        const pending = compressJpgImage(file, 100, 100, 1);
        const { image } = provideReaderResult(pending);
        image.width = 80;
        image.height = 60;
        image.onload!();

        expect(canvasElements[0]).toMatchObject({ width: 80, height: 60 });
        await expect(pending).resolves.toBe(nextCanvasBlob);
    });

    test('rejects missing canvas contexts and null compressed blobs', async () => {
        nextCanvasContext = null;
        const noContext = compressJpgImage(file, 100, 100, 1);
        let runtime = provideReaderResult(noContext);
        runtime.image.width = 80;
        runtime.image.height = 60;
        runtime.image.onload!();
        await expect(noContext).rejects.toThrow('failed to get canvas context');

        nextCanvasContext = { drawImage: jest.fn() };
        nextCanvasBlob = null;
        const noBlob = compressJpgImage(file, 100, 100, 1);
        runtime = provideReaderResult(noBlob);
        runtime.image.width = 80;
        runtime.image.height = 60;
        runtime.image.onload!();
        await expect(noBlob).rejects.toThrow('failed to compress image');
    });

    test('rejects image, missing reader-result and reader errors', async () => {
        const imageFailure = compressJpgImage(file, 100, 100, 1);
        const runtime = provideReaderResult(imageFailure);
        runtime.image.onerror!('invalid image');
        await expect(imageFailure).rejects.toBe('invalid image');

        const missingResult = compressJpgImage(file, 100, 100, 1);
        MockFileReader.instances.at(-1)!.onload!({ target: {} });
        await expect(missingResult).rejects.toThrow('failed to read file');

        const readerFailure = compressJpgImage(file, 100, 100, 1);
        MockFileReader.instances.at(-1)!.onerror!('reader failed');
        await expect(readerFailure).rejects.toBe('reader failed');
    });
});

describe('browser cache clearing', () => {
    test('rejects when Cache API is unavailable', async () => {
        await expect(clearBrowserCaches()).rejects.toBeUndefined();
        expect(mockLogger.error).toHaveBeenCalledWith('caches API is not supported in this browser');
    });

    test('deletes every cache and resolves when all deletions succeed', async () => {
        const deleteCache = jest.fn<(name: string) => Promise<boolean>>().mockResolvedValue(true);
        Object.defineProperty(globalThis.window, 'caches', {
            configurable: true,
            value: { keys: jest.fn(() => Promise.resolve(['app', 'images'])), delete: deleteCache }
        });

        await expect(clearBrowserCaches()).resolves.toBeUndefined();
        expect(deleteCache.mock.calls).toEqual([['app'], ['images']]);
        expect(mockLogger.info).toHaveBeenCalledWith('cache "app" cleared successfully');
        expect(mockLogger.info).toHaveBeenCalledWith('all caches cleared successfully');
    });

    test('still resolves when a cache deletion reports false or rejects', async () => {
        const deleteCache = jest.fn<(name: string) => Promise<boolean>>()
            .mockResolvedValueOnce(false)
            .mockRejectedValueOnce(new Error('delete failed'));
        Object.defineProperty(globalThis.window, 'caches', {
            configurable: true,
            value: { keys: jest.fn(() => Promise.resolve(['stale', 'broken'])), delete: deleteCache }
        });

        await expect(clearBrowserCaches()).resolves.toBeUndefined();
        expect(mockLogger.warn).toHaveBeenCalledWith('failed to clear cache "stale"');
    });

    test('rejects when cache key enumeration fails', async () => {
        const error = new Error('keys failed');
        Object.defineProperty(globalThis.window, 'caches', {
            configurable: true,
            value: { keys: jest.fn(() => Promise.reject(error)), delete: jest.fn() }
        });

        await expect(clearBrowserCaches()).rejects.toBe(error);
        expect(mockLogger.warn).toHaveBeenCalledWith('failed to clear cache', error);
    });
});
