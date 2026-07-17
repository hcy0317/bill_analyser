/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const { computed, nextTick, proxyRefs, reactive, ref } = actualVue;

const mockShowAlert = jest.fn();
const mockOpenExternalUrl = jest.fn();
const mockShowToast = jest.fn();
const mockRouteBackOnError = jest.fn();
const mockShowLoading = jest.fn();
const mockHideLoading = jest.fn();

const mockGetCloudSettings = jest.fn<(...args: any[]) => Promise<any>>();
const mockFullUpdateCloudSettings = jest.fn<(...args: any[]) => Promise<void>>();
const mockDisableCloudSettings = jest.fn<(...args: any[]) => Promise<void>>();

const aboutStates: any[] = [];
const cloudBaseStates: any[] = [];

function createAboutState(): any {
    const state = {
        clientVersion: '1.2.3',
        clientVersionMatchServerVersion: ref(true),
        serverDisplayVersion: ref(''),
        clientBuildTime: ref('2026-07-16 12:00'),
        exchangeRatesData: ref({
            dataSource: 'Exchange Provider',
            referenceUrl: 'https://rates.example.invalid'
        }),
        isUserCustomExchangeRates: ref(false),
        mapProviderName: ref('Map Provider'),
        mapProviderWebsite: ref('https://map.example.invalid'),
        licenseLines: ref(['MIT License', '']),
        thirdPartyLicenses: ref([
            {
                name: 'Complete dependency',
                copyright: 'Copyright holder',
                url: 'https://dependency.example.invalid',
                licenseUrl: 'https://dependency.example.invalid/license'
            },
            { name: 'Minimal dependency' }
        ]),
        refreshBrowserCache: jest.fn(),
        init: jest.fn()
    };
    aboutStates.push(state);
    return state;
}

function createCloudBaseState(): any {
    const settings = ref({
        mobileSetting: true,
        desktopSetting: false,
        sharedSetting: true
    } as Record<string, boolean>);
    const categories = [
        {
            categoryName: 'Basic Settings',
            items: [
                { settingKey: 'mobileSetting', settingName: 'Mobile setting', mobile: true, desktop: false },
                { settingKey: 'sharedSetting', settingName: 'Shared setting', mobile: true, desktop: true }
            ]
        },
        {
            categoryName: 'Statistics Settings',
            categorySubName: 'Common Settings',
            items: [
                { settingKey: 'desktopSetting', settingName: 'Desktop setting', mobile: false, desktop: true }
            ]
        }
    ];
    const state = {
        ALL_APPLICATION_CLOUD_SETTINGS: categories,
        loading: ref(false),
        enabling: ref(false),
        disabling: ref(false),
        enabledApplicationCloudSettings: settings,
        isEnableCloudSync: ref(false),
        hasEnabledApplicationCloudSettings: computed(() => Object.values(settings.value).some(Boolean)),
        enabledApplicationCloudSettingKeys: computed(() => Object.keys(settings.value).filter(key => settings.value[key])),
        isAllSettingsSelected: jest.fn((category: any) => category.items.every((item: any) => settings.value[item.settingKey])),
        hasSettingSelectedButNotAllChecked: jest.fn((category: any) => {
            const selected = category.items.filter((item: any) => settings.value[item.settingKey]).length;
            return selected > 0 && selected < category.items.length;
        }),
        updateSettingsSelected: jest.fn((category: any, selected: boolean) => {
            for (const item of category.items) settings.value[item.settingKey] = selected;
        }),
        selectAllSettings: jest.fn(() => {
            for (const category of categories) {
                for (const item of category.items) settings.value[item.settingKey] = true;
            }
        }),
        selectNoneSettings: jest.fn(() => {
            for (const key of Object.keys(settings.value)) settings.value[key] = false;
        }),
        selectInvertSettings: jest.fn(() => {
            for (const key of Object.keys(settings.value)) settings.value[key] = !settings.value[key];
        }),
        setUserApplicationCloudSettings: jest.fn((response: any) => {
            settings.value = response && response.length
                ? Object.fromEntries(response.map((item: any) => [item.settingKey, true]))
                : {};
        })
    };
    cloudBaseStates.push(state);
    return state;
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`
    })
}));

jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: (...args: unknown[]) => mockShowAlert(...args),
        openExternalUrl: (...args: unknown[]) => mockOpenExternalUrl(...args),
        showToast: (...args: unknown[]) => mockShowToast(...args),
        routeBackOnError: (...args: unknown[]) => mockRouteBackOnError(...args)
    }),
    showLoading: (...args: unknown[]) => mockShowLoading(...args),
    hideLoading: (...args: unknown[]) => mockHideLoading(...args)
}));

jest.mock('@/views/base/AboutPageBase.ts', () => ({
    useAboutPageBase: () => createAboutState()
}));

jest.mock('@/views/base/settings/AppCloudSyncPageBase.ts', () => ({
    useAppCloudSyncBase: () => createCloudBaseState()
}));

jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({
        getUserApplicationCloudSettings: (...args: any[]) => mockGetCloudSettings(...args),
        fullUpdateUserApplicationCloudSettings: (...args: any[]) => mockFullUpdateCloudSettings(...args),
        disableUserApplicationCloudSettings: (...args: any[]) => mockDisableCloudSettings(...args)
    })
}));

const AboutPage = require('@/views/mobile/AboutPage.vue').default as any;
const ApplicationCloudSyncSettingsPage = require(
    '@/views/mobile/settings/ApplicationCloudSyncSettingsPage.vue'
).default as any;

const aboutComponentNames = [
    'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
    'f7-block-title', 'f7-list', 'f7-list-item', 'f7-popup', 'f7-subnavbar', 'f7-block',
    'f7-actions', 'f7-actions-group', 'f7-actions-button'
];

const cloudComponentNames = [
    'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
    'f7-list', 'f7-list-item', 'f7-list-button', 'f7-icon', 'f7-actions',
    'f7-actions-group', 'f7-actions-button'
];

function setup(component: any, componentProps: Record<string, unknown> = {}): {
    props: Record<string, unknown>;
    bindings: any;
    emit: jest.Mock;
} {
    const props = reactive(componentProps);
    const emit = jest.fn();
    const bindings = component.setup(props, {
        attrs: {}, slots: {}, emit, expose: jest.fn()
    });
    return { props, bindings, emit };
}

function render(component: any, props: Record<string, unknown>, bindings: any): any {
    return component.render({ ...props, ...proxyRefs(bindings) }, [], props, proxyRefs(bindings), {}, {});
}

async function renderAboutToHtml(): Promise<string> {
    const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
    const { renderToString } = jest.requireActual('vue/server-renderer') as any;
    const Stub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => h(
            'div',
            attrs,
            Object.values(slots).flatMap((slot: any) => slot?.({}) ?? [])
        )
    });
    const app = createSSRApp(AboutPage);
    for (const name of aboutComponentNames) app.component(name, Stub);
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

async function flushPromises(times = 8): Promise<void> {
    for (let index = 0; index < times; index += 1) await Promise.resolve();
}

let warnSpy: jest.SpiedFunction<typeof console.warn>;

beforeAll(() => {
    warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    warnSpy.mockRestore();
});

beforeEach(() => {
    jest.clearAllMocks();
    aboutStates.length = 0;
    cloudBaseStates.length = 0;
    mockGetCloudSettings.mockResolvedValue(false);
    mockFullUpdateCloudSettings.mockResolvedValue(undefined);
    mockDisableCloudSettings.mockResolvedValue(undefined);
});

describe('mobile AboutPage production behavior', () => {
    test('counts version clicks and alerts only for known backend mismatches', () => {
        const page = setup(AboutPage);
        const state = aboutStates[0];
        expect(state.init).toHaveBeenCalledTimes(1);
        expect(page.bindings.forceShowRefreshBrowserCacheMenu.value).toBe(false);

        page.bindings.showVersion();
        expect(mockShowAlert).not.toHaveBeenCalled();
        for (let index = 0; index < 4; index += 1) page.bindings.showVersion();
        expect(page.bindings.forceShowRefreshBrowserCacheMenu.value).toBe(true);

        state.serverDisplayVersion.value = 'unknown';
        page.bindings.showVersion();
        state.serverDisplayVersion.value = state.clientVersion;
        page.bindings.showVersion();
        expect(mockShowAlert).not.toHaveBeenCalled();

        state.serverDisplayVersion.value = '9.9.9';
        page.bindings.showVersion();
        expect(mockShowAlert).toHaveBeenCalledWith(
            'tt:Frontend Version: 1.2.3<br/>tt:Backend Version: 9.9.9'
        );
    });

    test('renders rich and empty provider/license states and executes template events', async () => {
        const mounted = mountWithHostRenderer(AboutPage, {}, aboutComponentNames);
        try {
            const state = aboutStates[0];
            expect(mounted.root.children.length).toBeGreaterThan(0);
            const callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.length).toBeGreaterThan(7);
            for (const { callback } of callbacks) callback({});
            expect(mockOpenExternalUrl.mock.calls.flat()).toEqual(expect.arrayContaining([
                'https://github.com/mayswind/bill_analyser',
                'https://github.com/mayswind/bill_analyser/issues',
                'https://bill_analyser.mayswind.net',
                'https://rates.example.invalid',
                'https://map.example.invalid'
            ]));
            expect(state.refreshBrowserCache).toHaveBeenCalledTimes(1);

            state.clientVersionMatchServerVersion.value = false;
            state.clientBuildTime.value = '';
            state.exchangeRatesData.value = { dataSource: 'Offline Provider', referenceUrl: '' };
            state.mapProviderWebsite.value = '';
            await nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
            for (const { callback } of collectHostCallbacks(mounted.root)) callback({});
            expect(mounted.state.showRefreshBrowserCacheSheet).toBe(false);

            state.isUserCustomExchangeRates.value = true;
            state.mapProviderName.value = '';
            state.licenseLines.value = [''];
            state.thirdPartyLicenses.value = [];
            await nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);

            state.exchangeRatesData.value = undefined;
            state.clientVersionMatchServerVersion.value = true;
            mounted.state.versionClickCount = 5;
            await nextTick();
            expect(mounted.state.forceShowRefreshBrowserCacheMenu).toBe(true);
        } finally {
            mounted.app.unmount();
        }
    });

    test('direct render keeps version/build/provider branches executable', () => {
        const page = setup(AboutPage);
        expect(render(AboutPage, page.props, page.bindings)).toBeDefined();
        const state = aboutStates[0];
        state.clientBuildTime.value = '';
        state.exchangeRatesData.value = undefined;
        state.mapProviderName.value = '';
        expect(render(AboutPage, page.props, page.bindings)).toBeDefined();
    });

    test('SSR executes the matching-version navigation placeholder slot', async () => {
        expect(await renderAboutToHtml()).toContain('mobile.about.page');
        expect(aboutStates.at(-1).clientVersionMatchServerVersion.value).toBe(true);
    });
});

describe('mobile ApplicationCloudSyncSettingsPage actions', () => {
    test('initializes success, empty, processed, visible, and raw failure states', async () => {
        mockGetCloudSettings.mockResolvedValueOnce([{ settingKey: 'serverSetting' }]);
        const success = setup(ApplicationCloudSyncSettingsPage, { f7router: { back: jest.fn() } });
        await flushPromises();
        expect(cloudBaseStates[0].setUserApplicationCloudSettings).toHaveBeenCalledWith([
            { settingKey: 'serverSetting' }
        ]);
        expect(success.bindings.loading.value).toBe(false);

        mockGetCloudSettings.mockResolvedValueOnce(false);
        const empty = setup(ApplicationCloudSyncSettingsPage, { f7router: { back: jest.fn() } });
        await flushPromises();
        expect(cloudBaseStates[1].setUserApplicationCloudSettings).toHaveBeenCalledWith(false);
        expect(empty.bindings.loading.value).toBe(false);

        mockGetCloudSettings.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processed = setup(ApplicationCloudSyncSettingsPage, { f7router: { back: jest.fn() } });
        await flushPromises();
        expect(processed.bindings.loading.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalledWith('handled');

        mockGetCloudSettings.mockRejectedValueOnce({ processed: false, message: 'load failed' });
        const visible = setup(ApplicationCloudSyncSettingsPage, { f7router: { back: jest.fn() } });
        await flushPromises();
        visible.bindings.onPageAfterIn();
        expect((mockRouteBackOnError.mock.calls.at(-1)?.[1] as { value: unknown }).value).toEqual({
            processed: false,
            message: 'load failed'
        });
        expect(mockShowToast).toHaveBeenCalledWith('load failed');

        mockGetCloudSettings.mockRejectedValueOnce('raw load failure');
        setup(ApplicationCloudSyncSettingsPage, { f7router: { back: jest.fn() } });
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('raw load failure');
    });

    test('enables, updates, disables, and reports processed or visible action failures', async () => {
        const f7router = { back: jest.fn() };
        const page = setup(ApplicationCloudSyncSettingsPage, { f7router });
        await flushPromises();
        const state = cloudBaseStates[0];
        state.enabledApplicationCloudSettings.value = { mobileSetting: true, sharedSetting: true };

        page.bindings.enable(false);
        expect(page.bindings.enabling.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(expect.any(Function));
        expect((mockShowLoading.mock.calls.at(-1)?.[0] as () => boolean)()).toBe(true);
        await flushPromises();
        expect(mockFullUpdateCloudSettings).toHaveBeenCalledWith(['mobileSetting', 'sharedSetting']);
        expect(page.bindings.enabling.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('Settings sync has been enabled');

        page.bindings.enable(true);
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('Synchronized settings have been updated');

        mockFullUpdateCloudSettings.mockRejectedValueOnce({ processed: true, message: 'handled update' });
        page.bindings.enable(false);
        await flushPromises();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled update');
        mockFullUpdateCloudSettings.mockRejectedValueOnce({ processed: false, message: 'update failed' });
        page.bindings.enable(false);
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('update failed');
        mockFullUpdateCloudSettings.mockRejectedValueOnce('raw update failure');
        page.bindings.enable(false);
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('raw update failure');

        page.bindings.disable();
        expect(page.bindings.disabling.value).toBe(true);
        await flushPromises();
        expect(state.enabledApplicationCloudSettings.value).toEqual({});
        expect(mockShowToast).toHaveBeenCalledWith('Settings sync has been disabled');

        mockDisableCloudSettings.mockRejectedValueOnce({ processed: true, message: 'handled disable' });
        page.bindings.disable();
        await flushPromises();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled disable');
        mockDisableCloudSettings.mockRejectedValueOnce({ processed: false, message: 'disable failed' });
        page.bindings.disable();
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('disable failed');
        mockDisableCloudSettings.mockRejectedValueOnce('raw disable failure');
        page.bindings.disable();
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('raw disable failure');

        page.bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(f7router, expect.any(Object));
        expect((mockRouteBackOnError.mock.calls.at(-1)?.[1] as { value: unknown }).value).toBeNull();
    });
});

describe('mobile ApplicationCloudSyncSettingsPage template behavior', () => {
    test('renders loading, enabled, disabled, busy, empty-selection, and template event states', async () => {
        mockGetCloudSettings.mockImplementationOnce(() => new Promise(() => undefined));
        const loadingMount = mountWithHostRenderer(
            ApplicationCloudSyncSettingsPage,
            { f7router: { back: jest.fn() } },
            cloudComponentNames
        );
        expect(loadingMount.state.loading).toBe(true);
        expect(loadingMount.root.children.length).toBeGreaterThan(0);
        loadingMount.app.unmount();

        const mounted = mountWithHostRenderer(
            ApplicationCloudSyncSettingsPage,
            { f7router: { back: jest.fn() } },
            cloudComponentNames
        );
        try {
            await flushPromises();
            const state = cloudBaseStates[1];
            expect(mounted.state.loading).toBe(false);
            let callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.length).toBeGreaterThan(9);
            for (const { callback } of callbacks) callback({ target: { checked: false } });
            await flushPromises();
            expect(state.updateSettingsSelected).toHaveBeenCalled();
            expect(state.selectAllSettings).toHaveBeenCalled();
            expect(state.selectNoneSettings).toHaveBeenCalled();
            expect(state.selectInvertSettings).toHaveBeenCalled();

            state.isEnableCloudSync.value = true;
            state.enabledApplicationCloudSettings.value = { mobileSetting: true };
            mounted.state.showMoreActionSheet = true;
            await nextTick();
            callbacks = collectHostCallbacks(mounted.root);
            for (const { callback } of callbacks) callback({ target: { checked: true } });
            await flushPromises();
            expect(mockDisableCloudSettings).toHaveBeenCalled();

            state.enabledApplicationCloudSettings.value = {};
            state.enabling.value = true;
            state.disabling.value = true;
            await nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });

    test('direct render exposes synchronized selection bindings', async () => {
        const page = setup(ApplicationCloudSyncSettingsPage, { f7router: { back: jest.fn() } });
        await flushPromises();
        expect(render(ApplicationCloudSyncSettingsPage, page.props, page.bindings)).toBeDefined();
        cloudBaseStates[0].loading.value = true;
        expect(render(ApplicationCloudSyncSettingsPage, page.props, page.bindings)).toBeDefined();
    });
});
