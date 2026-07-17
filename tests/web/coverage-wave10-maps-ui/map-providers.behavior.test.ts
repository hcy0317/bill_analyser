/* eslint-disable @typescript-eslint/no-require-imports, @typescript-eslint/no-explicit-any */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockAsyncLoadAssets = jest.fn<(...args: any[]) => Promise<unknown>>(() => Promise.resolve('loaded'));
const mockLogger = { warn: jest.fn() };
const mockServices = {
    generateMapProxyTileImageUrl: jest.fn(() => 'proxy://tiles'),
    generateMapProxyAnnotationImageUrl: jest.fn(() => 'proxy://annotations'),
    generateBaiduMapJavascriptUrl: jest.fn(() => 'https://baidu.test/maps.js'),
    generateAmapApiInternalProxyUrl: jest.fn(() => 'https://internal.test/amap'),
    generateAmapJavascriptUrl: jest.fn(() => 'https://amap.test/maps.js'),
    generateGoogleMapJavascriptUrl: jest.fn(() => 'https://google.test/maps.js')
};
const mockServerSettings = {
    mapProxy: false,
    customAnnotationProxy: false,
    customTile: 'https://custom.test/{z}/{x}/{y}',
    customAnnotation: '',
    minZoom: 2,
    maxZoom: 18,
    defaultZoom: 11,
    tomtomKey: 'tom-key',
    tiandituKey: 'tian-key',
    amapMethod: 'none',
    amapExternalProxy: 'https://external.test/amap',
    amapSecret: 'amap-secret'
};

const tileSources: Record<string, any> = {
    preset: {
        website: 'https://preset.test',
        tileUrlFormat: 'https://tiles.test/{z}/{x}/{y}',
        tileUrlSubDomains: 'abc',
        annotationUrlFormat: 'https://labels.test/{z}/{x}/{y}?base=1',
        annotationUrlSubDomains: 'def',
        tileUrlExtraParams: [
            { paramName: 'tom', paramValueType: 'tomtom_key' },
            { paramName: 'tian', paramValueType: 'tianditu_key' },
            { paramName: 'lang', paramValueType: 'language' },
            { paramName: 'ignored', paramValueType: 'unknown' }
        ],
        annotationUrlExtraParams: [
            { paramName: 'lang', paramValueType: 'language' }
        ],
        attribution: 'Preset map',
        minZoom: 3,
        maxZoom: 17,
        defaultZoomLevel: 12
    },
    noWebsite: {
        website: undefined,
        tileUrlFormat: 'https://plain.test/{z}/{x}/{y}',
        minZoom: 1,
        maxZoom: 19,
        defaultZoomLevel: 10
    }
};

jest.mock('@/consts/map.ts', () => ({ LEAFLET_TILE_SOURCES: tileSources }));
jest.mock('@/lib/misc.ts', () => ({ asyncLoadAssets: (...args: any[]) => mockAsyncLoadAssets(...args) }));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/lib/server_settings.ts', () => ({
    isMapDataFetchProxyEnabled: () => mockServerSettings.mapProxy,
    getCustomMapTileLayerUrl: () => mockServerSettings.customTile,
    getCustomMapAnnotationLayerUrl: () => mockServerSettings.customAnnotation,
    isCustomMapAnnotationLayerDataFetchProxyEnabled: () => mockServerSettings.customAnnotationProxy,
    getCustomMapMinZoomLevel: () => mockServerSettings.minZoom,
    getCustomMapMaxZoomLevel: () => mockServerSettings.maxZoom,
    getCustomMapDefaultZoomLevel: () => mockServerSettings.defaultZoom,
    getTomTomMapAPIKey: () => mockServerSettings.tomtomKey,
    getTianDiTuMapAPIKey: () => mockServerSettings.tiandituKey,
    getAmapSecurityVerificationMethod: () => mockServerSettings.amapMethod,
    getAmapApiExternalProxyUrl: () => mockServerSettings.amapExternalProxy,
    getAmapApplicationSecret: () => mockServerSettings.amapSecret
}));
jest.mock('leaflet/dist/leaflet.css', () => ({}), { virtual: true });
jest.mock('leaflet/dist/leaflet-src.esm.js', () => ({ mockedLeafletAsset: true }), { virtual: true });

const {
    LeafletMapProvider,
    LeafletMapInstance
} = require('@/lib/map/leaflet.ts') as typeof import('@/lib/map/leaflet.ts');
const {
    BaiduMapProvider,
    BaiduMapInstance
} = require('@/lib/map/baidumap.ts') as typeof import('@/lib/map/baidumap.ts');
const {
    AmapMapProvider,
    AmapMapInstance
} = require('@/lib/map/amap.ts') as typeof import('@/lib/map/amap.ts');
const {
    GoogleMapProvider,
    GoogleMapInstance
} = require('@/lib/map/googlemap.ts') as typeof import('@/lib/map/googlemap.ts');

const center = { latitude: 31.2, longitude: 121.5 };

function options(onClick: ((position: typeof center) => void) | undefined = jest.fn()): any {
    return {
        initCenter: center,
        zoomLevel: 13,
        language: 'zh-CN',
        text: { zoomIn: '放大', zoomOut: '缩小' },
        onClick
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockServerSettings.mapProxy = false;
    mockServerSettings.customAnnotationProxy = false;
    mockServerSettings.customTile = 'https://custom.test/{z}/{x}/{y}';
    mockServerSettings.customAnnotation = '';
    mockServerSettings.amapMethod = 'none';
    LeafletMapProvider.Leaflet = null;
    BaiduMapProvider.BMap = null;
    AmapMapProvider.AMap = null;
    GoogleMapProvider.GoogleMap = null;
    Object.assign(globalThis.window as any, {
        onBMapCallback: undefined,
        BMap: undefined,
        onAMapCallback: undefined,
        AMap: undefined,
        _AMapSecurityConfig: undefined,
        onGoogleMapCallback: undefined,
        google: undefined
    });
});

describe('Leaflet map provider and instance behavior', () => {
    function createLeafletRuntime() {
        const clickHandlers: Record<string, (...args: any[]) => void> = {};
        const map = {
            addEventListener: jest.fn((name: string, handler: (...args: any[]) => void) => {
                clickHandlers[name] = handler;
            }),
            setView: jest.fn()
        };
        const tileLayers: any[] = [];
        const zoomControl = { addTo: jest.fn() };
        const attribution = { addAttribution: jest.fn(), addTo: jest.fn() };
        const markers: any[] = [];
        const runtime = {
            map: jest.fn(() => map),
            tileLayer: jest.fn((url: string, config: unknown) => {
                const layer = { url, config, addTo: jest.fn() };
                tileLayers.push(layer);
                return layer;
            }),
            control: {
                zoom: jest.fn(() => zoomControl),
                attribution: jest.fn(() => attribution)
            },
            icon: jest.fn((config: unknown) => ({ config })),
            marker: jest.fn((position: unknown, config: unknown) => {
                const marker = { position, config, addTo: jest.fn(), setLatLng: jest.fn(), remove: jest.fn() };
                markers.push(marker);
                return marker;
            })
        };
        return { runtime, map, clickHandlers, tileLayers, zoomControl, attribution, markers };
    }

    test('reports provider capability, website and invalid-provider behavior', () => {
        expect(new LeafletMapProvider('preset').getWebsite()).toBe('https://preset.test');
        expect(new LeafletMapProvider('noWebsite').getWebsite()).toBe('');
        expect(new LeafletMapProvider('custom').getWebsite()).toBe('');
        expect(new LeafletMapProvider('missing').getWebsite()).toBe('');
        expect(new LeafletMapProvider('preset').isSupportGetGeoLocationByClick()).toBe(true);
        expect(new LeafletMapProvider('missing').createMapInstance()).toBeNull();
        expect(new LeafletMapProvider('custom').createMapInstance()).toBeInstanceOf(LeafletMapInstance);
    });

    test('loads the real Leaflet adapter asset boundary', async () => {
        const loaded = await new LeafletMapProvider('preset').asyncLoadAssets('zh-CN') as any[];
        expect(loaded).toHaveLength(2);
        expect(LeafletMapProvider.Leaflet).toMatchObject({ mockedLeafletAsset: true });
    });

    test('initializes preset tiles, annotation, controls, click and marker lifecycle', () => {
        const state = createLeafletRuntime();
        LeafletMapProvider.Leaflet = state.runtime;
        const instance = new LeafletMapInstance('preset', tileSources['preset']);
        const onClick = jest.fn();

        instance.initMapInstance({ id: 'map' } as HTMLElement, options(onClick));

        expect(instance).toMatchObject({ dependencyLoaded: true, inited: true, defaultZoomLevel: 12, minZoomLevel: 3 });
        expect(state.runtime.map as any).toHaveBeenCalledWith(expect.anything(), expect.objectContaining({
            center: [31.2, 121.5], zoom: 13, attributionControl: false, zoomControl: false
        }));
        expect(state.tileLayers[0].url).toBe('https://tiles.test/{z}/{x}/{y}?tom=tom-key&tian=tian-key&lang=zh-CN');
        expect(state.tileLayers[1].url).toBe('https://labels.test/{z}/{x}/{y}?base=1&lang=zh-CN');
        expect(state.attribution.addAttribution).toHaveBeenCalledWith('Preset map');
        state.clickHandlers['click']!({ latlng: { lat: 30, lng: 120 } });
        expect(onClick).toHaveBeenCalledWith({ latitude: 30, longitude: 120 });

        instance.setMapCenterTo({ latitude: 32, longitude: 122 }, 15);
        expect(state.map.setView).toHaveBeenCalledWith([32, 122], 15);
        instance.setMapCenterMarker(center);
        instance.setMapCenterMarker({ latitude: 32, longitude: 122 });
        expect(state.markers[0].addTo).toHaveBeenCalledWith(state.map);
        expect(state.markers[0].setLatLng).toHaveBeenCalledWith([32, 122]);
        instance.removeMapCenterMarker();
        expect(state.markers[0].remove).toHaveBeenCalled();
        expect(() => instance.removeMapCenterMarker()).not.toThrow();
    });

    test('uses proxy URLs and supports custom layers with fallback zooms', () => {
        const proxied = createLeafletRuntime();
        LeafletMapProvider.Leaflet = proxied.runtime;
        mockServerSettings.mapProxy = true;
        const preset = new LeafletMapInstance('preset', tileSources['preset']);
        preset.initMapInstance({} as HTMLElement, options(undefined));
        expect(proxied.tileLayers.map(layer => layer.url)).toEqual(['proxy://tiles', 'proxy://annotations']);

        const customRuntime = createLeafletRuntime();
        LeafletMapProvider.Leaflet = customRuntime.runtime;
        mockServerSettings.mapProxy = false;
        mockServerSettings.customAnnotationProxy = true;
        const custom = new LeafletMapInstance('custom', undefined as any);
        expect(custom).toMatchObject({ defaultZoomLevel: 11, minZoomLevel: 2 });
        custom.initMapInstance({} as HTMLElement, options());
        expect(customRuntime.tileLayers.map(layer => layer.url)).toEqual([
            'https://custom.test/{z}/{x}/{y}',
            ''
        ]);
    });

    test('guards instance actions before dependencies and initialization', () => {
        const instance = new LeafletMapInstance('custom', undefined as any);
        expect(() => instance.initMapInstance({} as HTMLElement, options())).not.toThrow();
        expect(() => instance.setMapCenterTo(center, 10)).not.toThrow();
        expect(() => instance.setMapCenterMarker(center)).not.toThrow();
        expect(() => instance.removeMapCenterMarker()).not.toThrow();
    });
});

describe('Baidu map provider and converted coordinate behavior', () => {
    function createBaiduRuntime() {
        const clickHandlers: Record<string, (...args: any[]) => void> = {};
        const maps: any[] = [];
        const markers: any[] = [];
        const translate = jest.fn();
        class Point {
            constructor(public lng: number, public lat: number) {}
        }
        class Map {
            enableScrollWheelZoom = jest.fn();
            addControl = jest.fn();
            centerAndZoom = jest.fn();
            addEventListener = jest.fn((name: string, handler: (...args: any[]) => void) => {
                clickHandlers[name] = handler;
            });
            addOverlay = jest.fn();
            removeOverlay = jest.fn();
            constructor(public container: unknown, public config: unknown) { maps.push(this); }
        }
        class NavigationControl { constructor(public config: unknown) {} }
        class Convertor { translate = translate; }
        class Marker {
            setPosition = jest.fn();
            constructor(public point: unknown) { markers.push(this); }
        }
        return { runtime: { Map, Point, NavigationControl, Convertor, Marker }, maps, markers, translate, clickHandlers };
    }

    test('loads once, installs callback and reflects browser runtime constants', async () => {
        const provider = new BaiduMapProvider();
        expect(provider.getWebsite()).toBe('https://map.baidu.com');
        expect(provider.isSupportGetGeoLocationByClick()).toBe(false);
        await expect(provider.asyncLoadAssets()).resolves.toBe('loaded');
        expect(mockAsyncLoadAssets).toHaveBeenCalledWith('js', 'https://baidu.test/maps.js');

        (globalThis.window as any).BMap = { Map: class {} };
        Object.assign(globalThis.window as any, {
            BMAP_NAVIGATION_CONTROL_ZOOM: 8,
            BMAP_ANCHOR_TOP_LEFT: 9,
            COORDINATES_WGS84: 10,
            COORDINATES_BD09: 11
        });
        (globalThis.window as any).onBMapCallback();
        expect(BaiduMapProvider.BMap).toBe((globalThis.window as any).BMap);
        expect(BaiduMapProvider.BMAP_NAVIGATION_CONTROL_ZOOM).toBe(8);
        expect(BaiduMapProvider.BMAP_ANCHOR_TOP_LEFT).toBe(9);
        expect(BaiduMapProvider.COORDINATES_WGS84).toBe(10);
        expect(BaiduMapProvider.COORDINATES_BD09).toBe(11);
        await expect(provider.asyncLoadAssets()).resolves.toBeUndefined();
        expect(provider.createMapInstance()).toBeInstanceOf(BaiduMapInstance);
    });

    test('initializes, converts and caches centers, then creates, updates and removes marker', () => {
        const state = createBaiduRuntime();
        BaiduMapProvider.BMap = state.runtime;
        const instance = new BaiduMapInstance();
        const onClick = jest.fn();
        instance.initMapInstance({} as HTMLElement, options(onClick));
        expect(instance).toMatchObject({ dependencyLoaded: true, inited: true });
        state.clickHandlers['click']!({ point: { lat: 30, lng: 120 } });
        expect(onClick).toHaveBeenCalledWith({ latitude: 30, longitude: 120 });

        instance.setMapCenterTo(center, 16);
        const centerCallback = state.translate.mock.calls[0]![3] as (data: any) => void;
        centerCallback({ status: 0, points: [{ lng: 122, lat: 32 }] });
        expect(state.maps[0].centerAndZoom).toHaveBeenLastCalledWith(expect.objectContaining({ lng: 122, lat: 32 }), 16);
        instance.setMapCenterTo(center, 17);
        expect(state.translate).toHaveBeenCalledTimes(1);

        instance.setMapCenterMarker(center);
        expect(state.markers).toHaveLength(1);
        instance.setMapCenterMarker({ latitude: 33, longitude: 123 });
        const markerCallback = state.translate.mock.calls[1]![3] as (data: any) => void;
        markerCallback({ status: 0, points: [{ lng: 124, lat: 34 }] });
        expect(state.markers[0].setPosition).toHaveBeenCalledWith(expect.objectContaining({ lng: 124, lat: 34 }));
        instance.removeMapCenterMarker();
        expect(state.maps[0].removeOverlay).toHaveBeenCalledWith(state.markers[0]);
    });

    test('falls back on conversion errors, absent converter and guarded states', () => {
        const state = createBaiduRuntime();
        BaiduMapProvider.BMap = state.runtime;
        const instance = new BaiduMapInstance();
        instance.initMapInstance({} as HTMLElement, options(undefined));
        instance.setMapCenterTo(center, 14);
        (state.translate.mock.calls[0]![3] as (data: any) => void)({ status: 1, points: null });
        instance.setMapCenterMarker({ latitude: 1, longitude: 2 });
        (state.translate.mock.calls[1]![3] as (data: any) => void)({ status: 1, points: undefined });
        expect(mockLogger.warn).toHaveBeenCalledTimes(2);

        (instance as any).baiduMapConverter = null;
        instance.setMapCenterTo({ latitude: 3, longitude: 4 }, 10);
        instance.setMapCenterMarker({ latitude: 5, longitude: 6 });
        expect(state.maps[0].centerAndZoom).toHaveBeenCalledWith(expect.objectContaining({ lng: 4, lat: 3 }), 10);

        BaiduMapProvider.BMap = null;
        const unloaded = new BaiduMapInstance();
        expect(() => unloaded.initMapInstance({} as HTMLElement, options())).not.toThrow();
        expect(() => unloaded.setMapCenterTo(center, 1)).not.toThrow();
        expect(() => unloaded.setMapCenterMarker(center)).not.toThrow();
        expect(() => unloaded.removeMapCenterMarker()).not.toThrow();
    });
});

describe('Amap provider security configuration and coordinate behavior', () => {
    function createAmapRuntime() {
        const maps: any[] = [];
        const markers: any[] = [];
        const clickHandlers: Record<string, (...args: any[]) => void> = {};
        const convertFrom = jest.fn();
        class LngLat {
            constructor(public lng: number, public lat: number) {}
            getLng() { return this.lng; }
            getLat() { return this.lat; }
        }
        class Map {
            addControl = jest.fn();
            on = jest.fn((name: string, handler: (...args: any[]) => void) => { clickHandlers[name] = handler; });
            setZoomAndCenter = jest.fn();
            add = jest.fn();
            remove = jest.fn();
            constructor(public container: unknown, public config: unknown) { maps.push(this); }
        }
        class ToolBar { constructor(public config: unknown) {} }
        class Marker {
            setPosition = jest.fn();
            constructor(public config: unknown) { markers.push(this); }
        }
        return { runtime: { Map, ToolBar, LngLat, Marker, convertFrom }, maps, markers, clickHandlers, convertFrom, LngLat };
    }

    test.each([
        ['internalproxy', { serviceHost: 'https://internal.test/amap' }],
        ['externalproxy', { serviceHost: 'https://external.test/amap' }],
        ['plaintext', { securityJsCode: 'amap-secret' }],
        ['none', {}]
    ])('configures %s security mode before loading', async (method, expected) => {
        mockServerSettings.amapMethod = method;
        const provider = new AmapMapProvider();
        await expect(provider.asyncLoadAssets()).resolves.toBe('loaded');
        expect((globalThis.window as any)._AMapSecurityConfig).toEqual(expected);
        expect(mockAsyncLoadAssets).toHaveBeenCalledWith('js', 'https://amap.test/maps.js');
        (globalThis.window as any).AMap = { Map: class {} };
        (globalThis.window as any).onAMapCallback();
        expect(AmapMapProvider.AMap).toBe((globalThis.window as any).AMap);
        await expect(provider.asyncLoadAssets()).resolves.toBeUndefined();
        expect(provider.getWebsite()).toBe('https://www.amap.com');
        expect(provider.isSupportGetGeoLocationByClick()).toBe(false);
        expect(provider.createMapInstance()).toBeInstanceOf(AmapMapInstance);
    });

    test('initializes, converts and caches centers, handles markers and click coordinates', () => {
        const state = createAmapRuntime();
        AmapMapProvider.AMap = state.runtime;
        const instance = new AmapMapInstance();
        const onClick = jest.fn();
        instance.initMapInstance({} as HTMLElement, options(onClick));
        expect(instance).toMatchObject({ dependencyLoaded: true, inited: true, defaultZoomLevel: 14, minZoomLevel: 1 });
        state.clickHandlers['click']!({ lnglat: { lat: 30, lng: 120 } });
        expect(onClick).toHaveBeenCalledWith({ latitude: 30, longitude: 120 });

        instance.setMapCenterTo(center, 15);
        const converted = new state.LngLat(122, 32);
        (state.convertFrom.mock.calls[0]![2] as (status: string, result: any) => void)('complete', {
            info: 'ok', locations: [converted]
        });
        expect(state.maps[0].setZoomAndCenter).toHaveBeenLastCalledWith(15, converted);
        instance.setMapCenterTo(center, 16);
        expect(state.convertFrom).toHaveBeenCalledTimes(1);

        instance.setMapCenterMarker(center);
        expect(state.markers).toHaveLength(1);
        instance.setMapCenterMarker({ latitude: 33, longitude: 123 });
        const convertedMarker = new state.LngLat(124, 34);
        (state.convertFrom.mock.calls[1]![2] as (status: string, result: any) => void)('complete', {
            info: 'ok', locations: [convertedMarker]
        });
        expect(state.markers[0].setPosition).toHaveBeenCalledWith(convertedMarker);
        instance.removeMapCenterMarker();
        expect(state.maps[0].remove).toHaveBeenCalledWith(state.markers[0]);
    });

    test('uses original points on conversion failure and guards unloaded actions', () => {
        const state = createAmapRuntime();
        AmapMapProvider.AMap = state.runtime;
        const instance = new AmapMapInstance();
        instance.initMapInstance({} as HTMLElement, options(undefined));
        instance.setMapCenterTo(center, 15);
        (state.convertFrom.mock.calls[0]![2] as (status: string, result: any) => void)('error', {
            info: 'error', locations: null
        });
        instance.setMapCenterMarker({ latitude: 1, longitude: 2 });
        (state.convertFrom.mock.calls[1]![2] as (status: string, result: any) => void)('error', {
            info: 'error', locations: undefined
        });
        expect(mockLogger.warn).toHaveBeenCalledTimes(2);

        AmapMapProvider.AMap = null;
        const unloaded = new AmapMapInstance();
        expect(() => unloaded.initMapInstance({} as HTMLElement, options())).not.toThrow();
        expect(() => unloaded.setMapCenterTo(center, 1)).not.toThrow();
        expect(() => unloaded.setMapCenterMarker(center)).not.toThrow();
        expect(() => unloaded.removeMapCenterMarker()).not.toThrow();
    });
});

describe('Google Maps provider and marker behavior', () => {
    function createGoogleRuntime() {
        const maps: any[] = [];
        const markers: any[] = [];
        const clickHandlers: Record<string, (...args: any[]) => void> = {};
        class Map {
            addListener = jest.fn((name: string, handler: (...args: any[]) => void) => { clickHandlers[name] = handler; });
            setCenter = jest.fn();
            setZoom = jest.fn();
            constructor(public container: unknown, public config: unknown) { maps.push(this); }
        }
        class Marker {
            setPosition = jest.fn();
            setMap = jest.fn();
            constructor(public config: unknown) { markers.push(this); }
        }
        return { runtime: { Map, Marker }, maps, markers, clickHandlers };
    }

    test('loads browser API once and updates resolved control position', async () => {
        const provider = new GoogleMapProvider();
        expect(provider.getWebsite()).toBe('https://maps.google.com');
        expect(provider.isSupportGetGeoLocationByClick()).toBe(true);
        await expect(provider.asyncLoadAssets('fr')).resolves.toBe('loaded');
        expect(mockAsyncLoadAssets).toHaveBeenCalledWith('js', 'https://google.test/maps.js');
        (globalThis.window as any).google = { maps: { ControlPosition: { LEFT_TOP: 42 }, Map: class {} } };
        (globalThis.window as any).onGoogleMapCallback();
        expect(GoogleMapProvider.GoogleMap).toBe((globalThis.window as any).google.maps);
        expect(GoogleMapProvider.ControlPosition.LEFT_TOP).toBe(42);
        await expect(provider.asyncLoadAssets('fr')).resolves.toBeUndefined();
        expect(provider.createMapInstance()).toBeInstanceOf(GoogleMapInstance);
    });

    test('initializes map, emits clicks, moves view, and manages marker lifecycle', () => {
        const state = createGoogleRuntime();
        GoogleMapProvider.GoogleMap = state.runtime;
        GoogleMapProvider.ControlPosition.LEFT_TOP = 7;
        const instance = new GoogleMapInstance();
        const onClick = jest.fn();
        instance.initMapInstance({} as HTMLElement, options(onClick));
        expect(instance).toMatchObject({ dependencyLoaded: true, inited: true, defaultZoomLevel: 14, minZoomLevel: 1 });
        expect(state.maps[0].config).toEqual(expect.objectContaining({
            center: { lat: 31.2, lng: 121.5 }, zoom: 13,
            zoomControlOptions: { position: 7 }, gestureHandling: 'greedy'
        }));
        state.clickHandlers['click']!({ latLng: { lat: () => 30, lng: () => 120 } });
        expect(onClick).toHaveBeenCalledWith({ latitude: 30, longitude: 120 });

        instance.setMapCenterTo({ latitude: 32, longitude: 122 }, 16);
        expect(state.maps[0].setCenter).toHaveBeenCalledWith({ lat: 32, lng: 122 });
        expect(state.maps[0].setZoom).toHaveBeenCalledWith(16);
        instance.setMapCenterMarker(center);
        instance.setMapCenterMarker({ latitude: 32, longitude: 122 });
        expect(state.markers[0].setPosition).toHaveBeenCalledWith({ lat: 32, lng: 122 });
        instance.removeMapCenterMarker();
        expect(state.markers[0].setMap).toHaveBeenCalledWith(null);
        expect(() => instance.removeMapCenterMarker()).not.toThrow();
    });

    test('guards map operations until the dependency and instance exist', () => {
        const instance = new GoogleMapInstance();
        expect(() => instance.initMapInstance({} as HTMLElement, options(undefined))).not.toThrow();
        expect(() => instance.setMapCenterTo(center, 1)).not.toThrow();
        expect(() => instance.setMapCenterMarker(center)).not.toThrow();
        expect(() => instance.removeMapCenterMarker()).not.toThrow();
    });
});
