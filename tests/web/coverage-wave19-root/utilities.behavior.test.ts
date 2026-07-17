/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';

class FakeElement {
    readonly attributes = new Map<string, string>();
    readonly listeners = new Map<string, Array<() => void>>();

    constructor(readonly tagName: string) {}

    setAttribute(name: string, value: string): void {
        this.attributes.set(name, value);
    }

    getAttribute(name: string): string | null {
        return this.attributes.get(name) ?? null;
    }

    hasAttribute(name: string): boolean {
        return this.attributes.has(name);
    }

    addEventListener(name: string, listener: () => void): void {
        const listeners = this.listeners.get(name) ?? [];
        listeners.push(listener);
        this.listeners.set(name, listeners);
    }

    dispatchEvent(event: { type: string }): boolean {
        for (const listener of this.listeners.get(event.type) ?? []) listener();
        return true;
    }
}

const mockHeadChildren: FakeElement[] = [];
const mockDocument = {
    head: {
        appendChild(element: FakeElement) {
            mockHeadChildren.push(element);
            return element;
        },
        get innerHTML() {
            return '';
        },
        set innerHTML(_value: string) {
            mockHeadChildren.splice(0);
        }
    },
    createElement: (tagName: string) => new FakeElement(tagName),
    querySelector: (selector: string) => {
        const match = /^(script|link)\[(src|href)="([^"]+)"\]$/u.exec(selector);
        if (!match) return null;
        return mockHeadChildren.find(element => (
            element.tagName === match[1] && element.getAttribute(match[2] as string) === match[3]
        )) ?? null;
    }
};
Object.defineProperty(globalThis, 'document', {
    configurable: true,
    value: mockDocument
});

const mockBase64Encode = jest.fn<(value: ArrayBuffer) => string>();
let mockMapProviderType = '';
const mockMapInstances: Array<Record<string, jest.Mock>> = [];

jest.mock('@/lib/common.ts', () => {
    const actual = jest.requireActual('@/lib/common.ts') as Record<string, unknown>;
    return {
        ...actual,
        base64encode: (value: ArrayBuffer) => mockBase64Encode(value)
    };
});

jest.mock('@/core/base.ts', () => ({
    reversed: <T>(items: T[]) => [...items].reverse()
}));

jest.mock('@/consts/map.ts', () => ({
    LEAFLET_TILE_SOURCES: { openstreetmap: { url: 'tiles' } }
}));
jest.mock('@/lib/server_settings.ts', () => ({
    getMapProvider: () => mockMapProviderType
}));

function createMapProvider(kind: string): Record<string, jest.Mock> {
    const provider = {
        asyncLoadAssets: jest.fn(),
        getWebsite: jest.fn(() => `https://${kind}.example`),
        isSupportGetGeoLocationByClick: jest.fn(() => kind !== 'baidu'),
        createMapInstance: jest.fn(() => ({ kind }))
    };
    mockMapInstances.push(provider);
    return provider;
}

jest.mock('@/lib/map/leaflet.ts', () => ({
    LeafletMapProvider: jest.fn().mockImplementation(() => createMapProvider('leaflet'))
}));
jest.mock('@/lib/map/googlemap.ts', () => ({
    GoogleMapProvider: jest.fn().mockImplementation(() => createMapProvider('google'))
}));
jest.mock('@/lib/map/baidumap.ts', () => ({
    BaiduMapProvider: jest.fn().mockImplementation(() => createMapProvider('baidu'))
}));
jest.mock('@/lib/map/amap.ts', () => ({
    AmapMapProvider: jest.fn().mockImplementation(() => createMapProvider('amap'))
}));

const misc = require('@/lib/misc.ts') as typeof import('@/lib/misc.ts');
const file = require('@/lib/file.ts') as typeof import('@/lib/file.ts');
const tag = require('@/lib/tag.ts') as typeof import('@/lib/tag.ts');
const template = require('@/lib/template.ts') as typeof import('@/lib/template.ts');
const map = require('@/lib/map/index.ts') as typeof import('@/lib/map/index.ts');
const learning = require('@/models/import_learning.ts') as typeof import('@/models/import_learning.ts');
const recurring = require('@/views/desktop/transactions/list/dialogs/edit-dialog/recurringCandidateDisplay.ts') as typeof import('@/views/desktop/transactions/list/dialogs/edit-dialog/recurringCandidateDisplay.ts');
const display = require('@/views/mobile/transactions/edit-page/displayHelpers.ts') as typeof import('@/views/mobile/transactions/edit-page/displayHelpers.ts');

beforeEach(() => {
    jest.clearAllMocks();
    mockBase64Encode.mockReturnValue('encoded-randoms');
    document.head.innerHTML = '';
    mockMapProviderType = '';
});

afterEach(() => {
    document.head.innerHTML = '';
});

describe('misc asset and id helpers', () => {
    test('loads new and existing JavaScript and CSS assets', async () => {
        const jsPromise = misc.asyncLoadAssets('js', '/bundle.js');
        const script = document.querySelector('script[src="/bundle.js"]') as HTMLScriptElement;
        expect(script).not.toBeNull();
        expect(script.getAttribute('async')).toBe('true');
        script.dispatchEvent(new Event('load'));
        await expect(jsPromise).resolves.toEqual({ type: 'js', assetUrl: '/bundle.js' });
        expect(script.getAttribute('data-loaded')).toBe('true');
        await expect(misc.asyncLoadAssets('js', '/bundle.js')).resolves.toEqual({
            type: 'js', assetUrl: '/bundle.js'
        });

        const cssPromise = misc.asyncLoadAssets('css', '/theme.css');
        const link = document.querySelector('link[href="/theme.css"]') as HTMLLinkElement;
        expect(link.getAttribute('rel')).toBe('stylesheet');
        link.dispatchEvent(new Event('load'));
        await expect(cssPromise).resolves.toEqual({ type: 'css', assetUrl: '/theme.css' });

        const pending = document.createElement('link');
        pending.setAttribute('href', '/pending.css');
        document.head.appendChild(pending);
        const pendingPromise = misc.asyncLoadAssets('css', '/pending.css');
        pending.dispatchEvent(new Event('load'));
        await expect(pendingPromise).resolves.toEqual({ type: 'css', assetUrl: '/pending.css' });
    });

    test('rejects unsupported, error, abort, and unexpected creation paths', async () => {
        await expect(misc.asyncLoadAssets('font', '/font.woff')).rejects.toEqual({
            type: 'font', assetUrl: '/font.woff', error: 'notsupport'
        });

        const errorPromise = misc.asyncLoadAssets('js', '/error.js');
        document.querySelector('script[src="/error.js"]')?.dispatchEvent(new Event('error'));
        await expect(errorPromise).rejects.toEqual({
            type: 'js', assetUrl: '/error.js', error: 'error'
        });

        const abortPromise = misc.asyncLoadAssets('css', '/abort.css');
        document.querySelector('link[href="/abort.css"]')?.dispatchEvent(new Event('abort'));
        await expect(abortPromise).rejects.toEqual({
            type: 'css', assetUrl: '/abort.css', error: 'abort'
        });

    });

    test('generates hashes and UUID v8 values with crypto and fallback entropy', () => {
        const cryptoDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
        const cryptoValue = {
            getRandomValues: jest.fn((values: Uint8Array) => {
                values.fill(7);
                return values;
            })
        };
        Object.defineProperty(globalThis, 'crypto', { configurable: true, value: cryptoValue });
        const cryptoHash = misc.generateRandomString();
        expect(cryptoHash).toMatch(/^[a-f0-9]{64}$/u);
        expect(mockBase64Encode).toHaveBeenCalled();
        expect(misc.generateRandomUUID()).toMatch(/^[a-f0-9]{8}-[a-f0-9]{4}-8[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/u);

        Object.defineProperty(globalThis, 'crypto', { configurable: true, value: undefined });
        expect(misc.generateRandomString()).toMatch(/^[a-f0-9]{64}$/u);

        if (cryptoDescriptor) Object.defineProperty(globalThis, 'crypto', cryptoDescriptor);
        else delete (globalThis as any).crypto;
    });
});

describe('file extension helpers', () => {
    test('normalizes extensions and resolves supported type contracts', () => {
        expect(file.getFileExtension('')).toBe('');
        expect(file.getFileExtension(null as any)).toBe('');
        expect(file.getFileExtension('archive.tar.gz')).toBe('gz');
        expect(file.findExtensionByType(undefined, 'csv')).toBeUndefined();
        expect(file.findExtensionByType([], 'csv')).toBeUndefined();
        expect(file.findExtensionByType([
            { type: 'csv', extensions: '.csv,.CSV' },
            { type: 'excel', extensions: '.xlsx' }
        ], 'excel')).toBe('.xlsx');
        expect(file.findExtensionByType([{ type: 'csv', extensions: '.csv' }], 'pdf')).toBeUndefined();
        expect(file.isFileExtensionSupported('report.CSV', '.csv,.xlsx')).toBe(true);
        expect(file.isFileExtensionSupported('report.pdf', '.csv,.xlsx')).toBe(false);
        expect(file.isFileExtensionSupported('report.csv', '')).toBe(false);
    });
});

describe('tag and template visibility helpers', () => {
    const visible = { id: 'visible', hidden: false } as any;
    const hidden = { id: 'hidden', hidden: true } as any;

    test.each([
        ['tag', tag],
        ['template', template]
    ])('%s helpers cover visible, hidden, show-all, and empty lists', (_name, helpers: any) => {
        expect(helpers.isNoAvailableTag?.([], false) ?? helpers.isNoAvailableTemplate([], false)).toBe(true);
        expect(helpers.isNoAvailableTag?.([hidden], false) ?? helpers.isNoAvailableTemplate([hidden], false)).toBe(true);
        expect(helpers.isNoAvailableTag?.([hidden], true) ?? helpers.isNoAvailableTemplate([hidden], true)).toBe(false);
        expect(helpers.isNoAvailableTag?.([hidden, visible], false) ?? helpers.isNoAvailableTemplate([hidden, visible], false)).toBe(false);

        const count = helpers.getAvailableTagCount ?? helpers.getAvailableTemplateCount;
        const first = helpers.getFirstShowingId;
        const last = helpers.getLastShowingId;
        expect(count([hidden, visible], false)).toBe(1);
        expect(count([hidden, visible], true)).toBe(2);
        expect(first([hidden, visible], false)).toBe('visible');
        expect(first([hidden], false)).toBeNull();
        expect(last([visible, hidden], false)).toBe('visible');
        expect(last([hidden], false)).toBeNull();
    });
});

describe('map provider facade', () => {
    test('returns neutral values before initialization and selects every provider', () => {
        expect(map.getMapWebsite()).toBe('');
        expect(map.isSupportGetGeoLocationByClick()).toBe(false);
        expect(map.createMapInstance()).toBeNull();

        for (const provider of ['openstreetmap', 'custom', 'googlemap', 'baidumap', 'amap']) {
            mockMapProviderType = provider;
            map.initMapProvider('zh-CN');
            const instance = mockMapInstances.at(-1) as Record<string, jest.Mock>;
            expect(instance['asyncLoadAssets']).toHaveBeenCalledWith('zh-CN');
            expect(map.getMapWebsite()).toContain('https://');
            expect(map.createMapInstance()).toEqual(expect.objectContaining({ kind: expect.any(String) }));
        }
        expect(map.isSupportGetGeoLocationByClick()).toBe(true);
    });
});

function suggestion(overrides: Record<string, unknown> = {}): any {
    return {
        matchType: 'counterparty',
        matchValue: 'Cafe',
        matchFeatures: { parser_id: 'wechat', counterparty: 'Cafe' },
        sampleCount: 2,
        sourcePreviewIds: [3, 2],
        learnedType: 'expense',
        learnedCategoryId: 1,
        learnedCategoryName: 'Food',
        learnedSourceAccountId: 2,
        learnedSourceAccountName: 'Wallet',
        learnedDestinationAccountId: null,
        learnedDestinationAccountName: '',
        summary: 'summary',
        ...overrides
    };
}

describe('import learning model helpers', () => {
    test('prefers stable keys and filters preview ids and feature summaries', () => {
        expect(learning.getImportLearningSuggestionKey(suggestion({ recommendationKey: ' key ' }))).toBe('key');
        expect(learning.getImportLearningSuggestionKey(suggestion({ recommendation_key: ' legacy ' }))).toBe('legacy');
        expect(learning.getImportLearningSuggestionKey(suggestion({ matchValue: 'Shop' })))
            .toBe('counterparty::Shop::3,2');
        expect(learning.collectImportLearningSuggestionPreviewIds([
            suggestion({ sourcePreviewIds: [3, 0, -1, 2] }),
            suggestion({ sourcePreviewIds: [2, 4] })
        ])).toEqual([3, 2, 4]);
        expect(learning.getImportLearningSuggestionFeatureSummary(suggestion({
            matchFeatures: {
                parser_id: 'wechat',
                counterparty: ' Cafe ',
                description: '',
                payment_method: ' ',
                custom: 'value',
                invalid: 4
            }
        }))).toBe('parser: wechat · counterparty:  Cafe  · custom: value');
        expect(learning.getImportLearningSuggestionFeatureSummary(suggestion({ matchFeatures: null }))).toBe('');
    });
});

describe('transaction display helpers', () => {
    test('formats recurring subtitles and primary reasons', () => {
        const format = recurring.createRecurringCandidateSubtitleFormatter(key => `tt:${key}`);
        expect(format({
            id: 1,
            name: 'Rent',
            matchedOccurrenceDate: '2026-07-01',
            matchReasons: ['same amount', 'same day']
        })).toBe('tt:Matched Date: 2026-07-01 · tt:Match Reasons: same amount | same day');
        expect(format({ id: 2, name: 'Empty' })).toBe('');
        expect(recurring.getRecurringCandidatePrimaryReason({ id: 1, name: 'A' })).toBe('');
        expect(recurring.getRecurringCandidatePrimaryReason({ id: 1, name: 'A', matchReasons: [] })).toBe('');
        expect(recurring.getRecurringCandidatePrimaryReason({ id: 1, name: 'A', matchReasons: ['', 'x'] })).toBe('');
        expect(recurring.getRecurringCandidatePrimaryReason({ id: 1, name: 'A', matchReasons: ['x'] })).toBe('x');
    });

    test('parses cents, selects amount classes, and projects picture urls', () => {
        expect(display.parseStrictQueryCents(12)).toBeUndefined();
        expect(display.parseStrictQueryCents('  +123 ')).toBe(123);
        expect(display.parseStrictQueryCents('1.2')).toBeUndefined();
        expect(display.parseStrictQueryCents('9007199254740992')).toBeUndefined();
        expect(display.getFontClassByAmount(100_000_000)).toBe('ebk-small-amount');
        expect(display.getFontClassByAmount(-100_000_000)).toBe('ebk-small-amount');
        expect(display.getFontClassByAmount(1_000_000)).toBe('ebk-normal-amount');
        expect(display.getFontClassByAmount(-1_000_000)).toBe('ebk-normal-amount');
        expect(display.getFontClassByAmount(999)).toBe('ebk-large-amount');
        const pictures = [{ id: 'a' }, { id: 'b' }] as any;
        const url = (picture: any) => picture.id === 'a' ? '/a.png' : undefined;
        expect(display.buildTransactionPictureItems(undefined, url)).toEqual([]);
        expect(display.buildTransactionPictureItems([], url)).toEqual([]);
        expect(display.buildTransactionPictureItems(pictures, url)).toEqual([
            { url: '/a.png' }, { url: undefined }
        ]);
        expect(display.buildTransactionThumbs(undefined, url)).toEqual([]);
        expect(display.buildTransactionThumbs([], url)).toEqual([]);
        expect(display.buildTransactionThumbs(pictures, url)).toEqual(['/a.png', undefined]);
    });
});
