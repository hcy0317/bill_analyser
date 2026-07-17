/* eslint-disable @typescript-eslint/no-require-imports, @typescript-eslint/no-explicit-any */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockMapFactory = jest.fn<() => any>();
const mockScrollToSelectedItem = jest.fn();
const mockScrollSheetToTop = jest.fn();
const mockShowLoading = jest.fn();
const mockHideLoading = jest.fn();
const mockShowToast = jest.fn();
const mockSaveTag = jest.fn<(...args: any[]) => Promise<any>>();
const mockTagStore = {
    allTransactionTags: [] as any[],
    allVisibleTagsCount: 0,
    saveTag: (...args: any[]) => mockSaveTag(...args)
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, options?: unknown) => options ? `tt:${key}:${JSON.stringify(options)}` : `tt:${key}`,
        tm: (key: string, options?: unknown) => options ? `tm:${key}:${JSON.stringify(options)}` : `tm:${key}`,
        getCurrentLanguageInfo: () => ({ alternativeLanguageTag: 'zh-Hans' })
    })
}));
jest.mock('@/lib/map/index.ts', () => ({ createMapInstance: () => mockMapFactory() }));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast }),
    showLoading: mockShowLoading,
    hideLoading: mockHideLoading,
    scrollToSelectedItem: (...args: any[]) => mockScrollToSelectedItem(...args),
    scrollSheetToTop: (...args: any[]) => mockScrollSheetToTop(...args)
}));
jest.mock('@/models/transaction_tag.ts', () => ({
    TransactionTag: class TransactionTag {
        static createNewTag() { return { id: '', name: '' }; }
    }
}));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));

const MapView = require('@/components/common/MapView.vue').default as any;
const ConfirmDialog = require('@/components/desktop/ConfirmDialog.vue').default as any;
const TransactionTagSelectionSheet = require('@/components/mobile/TransactionTagSelectionSheet.vue').default as any;
const { reactive } = jest.requireActual('vue') as any;
jest.spyOn(console, 'warn').mockImplementation(() => undefined);

function setup(component: any, props: Record<string, unknown>) {
    const emit = jest.fn();
    const exposed: any = {};
    const bindings = component.setup(props, {
        attrs: {},
        slots: {},
        emit,
        expose: (value: Record<string, any>) => Object.assign(exposed, value)
    });
    return { bindings, emit, exposed };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTagStore.allTransactionTags = [];
    mockTagStore.allVisibleTagsCount = 0;
    mockSaveTag.mockResolvedValue({ id: 'saved-id', name: 'Saved' });
});

describe('MapView production-loaded behavior', () => {
    function mapInstance(overrides: Record<string, unknown> = {}) {
        return {
            dependencyLoaded: true,
            inited: false,
            defaultZoomLevel: 14,
            minZoomLevel: 2,
            initMapInstance: jest.fn(function(this: any, _container: unknown, options: any) {
                this.inited = true;
                this.lastOptions = options;
            }),
            setMapCenterTo: jest.fn(),
            setMapCenterMarker: jest.fn(),
            removeMapCenterMarker: jest.fn(),
            ...overrides
        };
    }

    test('collapses unsupported and dependency-missing maps without mutating caller styles', () => {
        mockMapFactory.mockReturnValueOnce(null);
        const callerStyle = { width: '75%', background: 'red' };
        const unsupported = setup(MapView, reactive({ height: '240px', mapClass: 'map', mapStyle: callerStyle }));
        expect(unsupported.bindings.mapSupported.value).toBe(false);
        expect(unsupported.bindings.mapDependencyLoaded.value).toBe(false);
        expect(unsupported.bindings.finalMapStyle.value).toEqual({ width: '75%', background: 'red', height: '0' });
        expect(callerStyle).toEqual({ width: '75%', background: 'red' });
        expect(() => unsupported.exposed.initMapView()).not.toThrow();
        expect(() => unsupported.exposed.setMarkerPosition({ latitude: 1, longitude: 2 })).not.toThrow();

        const unloadedMap = mapInstance({ dependencyLoaded: false });
        mockMapFactory.mockReturnValueOnce(unloadedMap);
        const unloaded = setup(MapView, reactive({ mapStyle: undefined }));
        expect(unloaded.bindings.finalMapStyle.value).toEqual({ height: '0' });
        unloaded.exposed.initMapView();
        expect(unloadedMap.initMapInstance).not.toHaveBeenCalled();
    });

    test('initializes at a real location, emits clicks and avoids redundant recentering', () => {
        const instance = mapInstance();
        mockMapFactory.mockReturnValue(instance);
        const props = reactive({
            height: '300px',
            mapClass: 'transaction-map',
            mapStyle: { width: '100%' },
            geoLocation: { latitude: 31.2, longitude: 121.5 }
        });
        const mounted = setup(MapView, props);

        mounted.exposed.initMapView();
        expect(instance.initMapInstance).toHaveBeenCalledWith(null, expect.objectContaining({
            language: 'zh-Hans',
            initCenter: { latitude: 31.2, longitude: 121.5 },
            zoomLevel: 14,
            text: { zoomIn: 'tt:Zoom in', zoomOut: 'tt:Zoom out' }
        }));
        expect(instance.setMapCenterTo).toHaveBeenCalledWith({ latitude: 31.2, longitude: 121.5 }, 14);
        expect(instance.setMapCenterMarker).toHaveBeenCalledWith({ latitude: 31.2, longitude: 121.5 });
        (instance as any).lastOptions.onClick({ latitude: 30, longitude: 120 });
        expect(mounted.emit).toHaveBeenCalledWith('click', { latitude: 30, longitude: 120 });

        jest.clearAllMocks();
        mounted.exposed.initMapView();
        expect(instance.setMapCenterTo).not.toHaveBeenCalled();
        expect(instance.setMapCenterMarker).not.toHaveBeenCalled();
        mounted.exposed.setMarkerPosition({ latitude: 32, longitude: 122 });
        mounted.exposed.setMarkerPosition(undefined);
        expect(instance.setMapCenterMarker).toHaveBeenCalledTimes(1);
    });

    test('moves between valid and empty locations and removes the marker at minimum zoom', () => {
        const instance = mapInstance({ inited: true });
        mockMapFactory.mockReturnValue(instance);
        const props = reactive({ geoLocation: { latitude: 1, longitude: 2 } as any });
        const mounted = setup(MapView, props);
        mounted.exposed.initMapView();
        expect(instance.setMapCenterMarker).toHaveBeenCalled();

        props.geoLocation = { latitude: 0, longitude: 0 };
        mounted.exposed.initMapView();
        expect(instance.setMapCenterTo).toHaveBeenLastCalledWith({ latitude: 0, longitude: 0 }, 2);
        expect(instance.removeMapCenterMarker).toHaveBeenCalled();

        props.geoLocation = undefined;
        mounted.exposed.initMapView();
        expect(instance.setMapCenterTo).toHaveBeenCalledTimes(2);
    });

    test('does not recenter when an adapter refuses to initialize', () => {
        const instance = mapInstance({
            initMapInstance: jest.fn(),
            inited: false
        });
        mockMapFactory.mockReturnValue(instance);
        const mounted = setup(MapView, reactive({ geoLocation: undefined }));
        mounted.exposed.initMapView();
        expect(instance.initMapInstance).toHaveBeenCalled();
        expect(instance.setMapCenterTo).not.toHaveBeenCalled();
    });
});

describe('ConfirmDialog promise and keyboard behavior', () => {
    test('opens one-argument and options messages with translated warning, details and color', async () => {
        const mounted = setup(ConfirmDialog, reactive({ show: false, color: undefined, title: undefined, text: undefined }));
        const one = mounted.exposed.open('Delete bill');
        expect(mounted.bindings.titleContent.value).toBe('tt:global.app.title');
        expect(mounted.bindings.textContent.value).toBe('tt:Delete bill');
        mounted.bindings.confirm();
        await expect(one).resolves.toBe(true);

        const options = { warning: 'Permanent warning', details: ['First', 'Second'], color: 'error', id: 7 };
        const withOptions = mounted.exposed.open('Delete account', options);
        expect(mounted.bindings.warningContent.value).toContain('tm:Permanent warning');
        expect(mounted.bindings.detailsContent.value).toHaveLength(2);
        expect(mounted.bindings.finalColor.value).toBe('error');
        mounted.bindings.cancel();
        await expect(withOptions).resolves.toBe(false);
        expect(mounted.emit).toHaveBeenCalledWith('update:show', false);
    });

    test('supports title/text overloads, option formatting and primary fallback color', async () => {
        const mounted = setup(ConfirmDialog, reactive({ show: false, color: 'secondary', title: 'Initial', text: 'Initial text' }));
        const plain = mounted.exposed.open('Title', 'Body');
        expect(mounted.bindings.titleContent.value).toBe('tt:Title');
        expect(mounted.bindings.textContent.value).toBe('tt:Body');
        mounted.bindings.cancel();
        await plain;

        const formatted = mounted.exposed.open('Title {name}', 'Body {name}', {
            name: 'Alice', warning: 'Caution', details: ['Detail'], color: ''
        });
        expect(mounted.bindings.titleContent.value).toContain('Alice');
        expect(mounted.bindings.textContent.value).toContain('Alice');
        expect(mounted.bindings.warningContent.value).toContain('tm:Caution');
        expect(mounted.bindings.finalColor.value).toBe('primary');
        mounted.bindings.confirm();
        await formatted;
    });

    test.each([
        ['Enter', 'primary', true],
        ['Backspace', 'primary', false],
        ['Delete', 'error', true]
    ])('handles %s keyboard confirmation semantics', async (key, color, expected) => {
        const mounted = setup(ConfirmDialog, reactive({ show: false, color, title: undefined, text: undefined }));
        const pending = mounted.exposed.open('Question', { color });
        const event = { key, preventDefault: jest.fn() } as any;
        mounted.bindings.onKeydown(event);
        await expect(pending).resolves.toBe(expected);
        expect(event.preventDefault).toHaveBeenCalled();
    });

    test('ignores unrelated keys and non-error Delete, while confirm/cancel work without an active resolver', () => {
        const mounted = setup(ConfirmDialog, reactive({ show: false, color: 'primary', title: undefined, text: undefined }));
        for (const key of ['Escape', 'Delete']) {
            const event = { key, preventDefault: jest.fn() } as any;
            mounted.bindings.onKeydown(event);
            expect(event.preventDefault).not.toHaveBeenCalled();
        }
        expect(() => mounted.bindings.confirm()).not.toThrow();
        expect(() => mounted.bindings.cancel()).not.toThrow();
    });
});

describe('TransactionTagSelectionSheet user-visible selection behavior', () => {
    function sheetProps(overrides: Record<string, unknown> = {}) {
        return reactive({ modelValue: ['hidden-selected'], allowAddNewTag: true, enableFilter: true, show: true, ...overrides });
    }

    test('filters case-insensitively, preserves selected hidden tags and reports availability', () => {
        mockTagStore.allTransactionTags = [
            { id: 'food', name: 'Food', hidden: false },
            { id: 'hidden-selected', name: 'Secret', hidden: true },
            { id: 'hidden-other', name: 'Other', hidden: true }
        ];
        mockTagStore.allVisibleTagsCount = 9;
        const mounted = setup(TransactionTagSelectionSheet, sheetProps());
        expect(mounted.bindings.heightClass.value).toBe('tag-selection-huge-sheet');
        expect(mounted.bindings.allTags.value.map((tag: any) => tag.id)).toEqual(['food', 'hidden-selected']);
        expect(mounted.bindings.noAvailableTag.value).toBe(false);

        mounted.bindings.filterContent.value = 'FO';
        expect(mounted.bindings.allTags.value.map((tag: any) => tag.id)).toEqual(['food']);
        mounted.bindings.filterContent.value = 'missing';
        expect(mounted.bindings.allTags.value).toEqual([]);

        mockTagStore.allTransactionTags = [{ id: 'hidden', name: 'Hidden', hidden: true }];
        const hiddenOnly = setup(TransactionTagSelectionSheet, sheetProps());
        expect(hiddenOnly.bindings.noAvailableTag.value).toBe(true);
    });

    test.each([[5, 'tag-selection-large-sheet'], [4, 'tag-selection-default-sheet']])(
        'chooses height for %i visible tags',
        (count, expected) => {
            mockTagStore.allTransactionTags = [{ id: 'tag', name: 'Tag', hidden: false }];
            mockTagStore.allVisibleTagsCount = count;
            expect(setup(TransactionTagSelectionSheet, sheetProps()).bindings.heightClass.value).toBe(expected);
        }
    );

    test('adds and removes checked tags without duplicates, then saves the selection', () => {
        const mounted = setup(TransactionTagSelectionSheet, sheetProps({ modelValue: ['a'] }));
        mounted.bindings.changeTagSelection({ target: { value: 'b', checked: true } });
        mounted.bindings.changeTagSelection({ target: { value: 'b', checked: true } });
        mounted.bindings.changeTagSelection({ target: { value: 'a', checked: false } });
        mounted.bindings.changeTagSelection({ target: { value: 'missing', checked: false } });
        expect(mounted.bindings.selectedItemIds.value).toEqual(['b']);
        expect(mounted.bindings.isChecked('b')).toBe(true);
        expect(mounted.bindings.isChecked('a')).toBe(false);
        mounted.bindings.save();
        expect(mounted.emit).toHaveBeenNthCalledWith(1, 'update:modelValue', ['b']);
        expect(mounted.emit).toHaveBeenNthCalledWith(2, 'update:show', false);
    });

    test('creates and saves a new tag, selects its returned id and supports cancel', async () => {
        const mounted = setup(TransactionTagSelectionSheet, sheetProps({ modelValue: [] }));
        expect(() => mounted.bindings.saveNewTag()).not.toThrow();
        mounted.bindings.addNewTag();
        mounted.bindings.newTag.value.name = 'Travel';
        mounted.bindings.saveNewTag();
        expect(mockShowLoading).toHaveBeenCalled();
        await Promise.resolve();
        await Promise.resolve();
        expect(mockSaveTag).toHaveBeenCalledWith({ tag: expect.objectContaining({ name: 'Travel' }) });
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mounted.bindings.selectedItemIds.value).toEqual(['saved-id']);
        expect(mounted.bindings.newTag.value).toBeNull();

        mounted.bindings.addNewTag();
        mounted.bindings.cancelSaveNewTag();
        expect(mounted.bindings.newTag.value).toBeNull();
    });

    test.each([
        [{ processed: true, message: 'handled' }, false, 'handled error'],
        [{ processed: false, message: 'Save failed' }, true, 'message error'],
        [{ processed: false }, true, 'object fallback']
    ])('handles %s save rejection (%s)', async (error, shouldToast, _name) => {
        mockSaveTag.mockRejectedValueOnce(error);
        const mounted = setup(TransactionTagSelectionSheet, sheetProps({ modelValue: [] }));
        mounted.bindings.addNewTag();
        mounted.bindings.saveNewTag();
        await Promise.resolve();
        await Promise.resolve();
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledTimes(shouldToast ? 1 : 0);
    });

    test('handles focus/open/closed lifecycle and resets draft selection and filter', () => {
        const props = sheetProps({ modelValue: ['fresh'] });
        const mounted = setup(TransactionTagSelectionSheet, props);
        mounted.bindings.selectedItemIds.value = ['stale'];
        mounted.bindings.filterContent.value = 'query';
        mounted.bindings.addNewTag();
        mounted.bindings.onSearchBarFocus();
        expect(mockScrollSheetToTop).toHaveBeenCalledWith(undefined, window.innerHeight);

        const element = { id: 'sheet' };
        mounted.bindings.onSheetOpen({ $el: element });
        expect(mounted.bindings.selectedItemIds.value).toEqual(['fresh']);
        expect(mounted.bindings.newTag.value).toBeNull();
        expect(mockScrollToSelectedItem).toHaveBeenCalledWith(element, '.page-content', 'li.list-item-selected');

        mounted.bindings.onSheetClosed();
        expect(mounted.bindings.filterContent.value).toBe('');
        expect(mounted.emit).toHaveBeenCalledWith('update:show', false);
    });
});
