import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockThemeChange = jest.fn<(...args: any[]) => void>();
const mockRegisterServiceWorker = jest.fn<(...args: any[]) => void>();
const mockInitLocale = jest.fn<(...args: any[]) => { locale: string }>(() => ({ locale: 'zh-Hans' }));
const mockInitMapProvider = jest.fn<(...args: any[]) => void>();
const mockSetAmountColors = jest.fn<(...args: any[]) => void>();
const mockSetAppFontSize = jest.fn<(...args: any[]) => void>();
const mockUpdateLocalizedDefaults = jest.fn<(...args: any[]) => void>();
const mockGetLatestExchangeRates = jest.fn<(...args: any[]) => Promise<void>>();
const mockLogger = { info: jest.fn() };
const mockMatchMediaListeners: Array<(event: { matches: boolean }) => void> = [];
const mockF7Handlers = new Map<string, (...args: any[]) => void>();
const mockDocumentListeners = new Map<string, (...args: any[]) => void>();
const mockCssVariables = new Map<string, string>();
const mockTemplateHandlers: Array<(...args: any[]) => unknown> = [];
const mockNotificationCreate = jest.fn();
const mockNotificationClose = jest.fn();
const mockNotificationDestroy = jest.fn();
const mockNotificationOpen = jest.fn();
const mockF7Close = {
    actions: jest.fn(),
    dialog: jest.fn(),
    popover: jest.fn(),
    popup: jest.fn(),
    sheet: jest.fn()
};

let mockProduction = true;
let mockLoggedIn = true;
let mockUnlocked = true;
let mockModalShowing = false;
let mockThemePreference = 'system';
let mockSystemTheme = 'light';
let mockStandalone = false;
let mockLastNotificationOptions: any;
const mockMetaElement: any = {
    content: 'initial',
    setAttribute: jest.fn((name: string, value: string) => {
        if (name === 'content') mockMetaElement.content = value;
    })
};

const mockRootStore = actualVue.reactive({
    currentNotification: null as string | null,
    setNotificationContent: jest.fn((value: string | null) => {
        mockRootStore.currentNotification = value;
    })
});
const mockSettingsStore = actualVue.reactive({
    appSettings: {
        theme: 'system',
        timeZone: 'Asia/Shanghai',
        fontSize: 16,
        applicationLock: false,
        autoUpdateExchangeRatesData: true
    },
    updateLocalizedDefaultSettings: mockUpdateLocalizedDefaults
});
const mockUserStore = actualVue.reactive({
    currentUserLanguage: 'zh-Hans',
    currentUserExpenseAmountColor: 'red',
    currentUserIncomeAmountColor: 'green'
});
const mockEnvironmentStore = actualVue.reactive({ framework7DarkMode: false });
const mockExchangeRatesStore = { getLatestExchangeRates: mockGetLatestExchangeRates };

const mockF7: any = {
    darkMode: false,
    on: jest.fn((event: string, handler: (...args: any[]) => void) => {
        mockF7Handlers.set(event, handler);
    }),
    actions: { close: mockF7Close.actions },
    dialog: { close: mockF7Close.dialog },
    popover: { close: mockF7Close.popover },
    popup: { close: mockF7Close.popup },
    sheet: { close: mockF7Close.sheet },
    notification: {
        create: mockNotificationCreate
    }
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => void) => callback()
    };
});
jest.mock('vuetify', () => ({ useTheme: () => ({ change: mockThemeChange }) }));
jest.mock('register-service-worker', () => ({ register: mockRegisterServiceWorker }));
jest.mock(
    'framework7-vue',
    () => ({ f7ready: (callback: (f7: any) => void) => callback(mockF7) }),
    { virtual: true }
);
jest.mock('@/router/mobile.ts', () => ({ __esModule: true, default: [{ path: '/', component: {} }] }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageInfo: () => ({ alternativeLanguageTag: 'zh-CN' }),
        initLocale: mockInitLocale
    })
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => mockRootStore }));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/environment.ts', () => ({ useEnvironmentsStore: () => mockEnvironmentStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/exchangeRates.ts', () => ({ useExchangeRatesStore: () => mockExchangeRatesStore }));
jest.mock('@/consts/asset.ts', () => ({ APPLICATION_LOGO_PATH: '/logo.png' }));
jest.mock('@/core/theme.ts', () => ({
    SYSTEM_THEME_PREFERENCE: 'system',
    ThemeType: { Light: 'light', Dark: 'dark' },
    getFramework7DarkModePreference: (value: string) => value === 'dark',
    getMobileThemeConfig: (value: string) => ({
        primary: `primary:${value}`,
        cssVariables: { '--shell-theme': value },
        metaThemeColor: {
            default: `default:${value}`,
            backdrop: `backdrop:${value}`,
            pushBackdrop: `push:${value}`
        }
    }),
    normalizeThemePreference: (value: string) => value || 'system',
    resolveThemePreference: (value: string, system: string) => value === 'system' ? system : value
}));
jest.mock('@/lib/version.ts', () => ({ isProduction: () => mockProduction }));
jest.mock('@/lib/settings.ts', () => ({
    getTheme: () => mockThemePreference,
    isEnableSwipeBack: () => true,
    isEnableAnimate: () => false
}));
jest.mock('@/lib/map/index.ts', () => ({ initMapProvider: mockInitMapProvider }));
jest.mock('@/lib/userstate.ts', () => ({
    isUserLogined: () => mockLoggedIn,
    isUserUnlocked: () => mockUnlocked
}));
jest.mock('@/lib/ui/common.ts', () => ({
    getSystemTheme: () => mockSystemTheme,
    setExpenseAndIncomeAmountColor: (...args: unknown[]) => mockSetAmountColors(...args)
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    isModalShowing: () => mockModalShowing,
    setAppFontSize: (...args: unknown[]) => mockSetAppFontSize(...args)
}));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

const DesktopApp = require('@/DesktopApp.vue').default as any;
const MobileApp = require('@/MobileApp.vue').default as any;

function setup(component: any): any {
    return component.setup({}, { attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined });
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

async function render(component: any, names: string[]): Promise<string> {
    const { createSSRApp, defineComponent, h } = actualVue;
    const { renderToString } = jest.requireActual('vue/server-renderer') as any;
    const Stub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => {
            for (const value of Object.values(attrs)) {
                for (const handler of Array.isArray(value) ? value : [value]) {
                    if (typeof handler === 'function') mockTemplateHandlers.push(handler);
                }
            }
            return h(
                'div',
                attrs,
                Object.values(slots).flatMap((slot: any) => slot?.({}) ?? [])
            );
        }
    });
    const app = createSSRApp(component);
    for (const name of names) app.component(name, Stub);
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

function installBrowserState(options: { platform?: string; appVersion?: string; hash?: string } = {}): void {
    Object.defineProperty(globalThis.navigator, 'platform', {
        configurable: true,
        value: options.platform ?? 'Win32'
    });
    Object.defineProperty(globalThis.navigator, 'appVersion', {
        configurable: true,
        value: options.appVersion ?? 'Chrome'
    });
    const matchMedia = jest.fn(() => ({
        matches: mockStandalone,
        addEventListener: jest.fn((_event: string, listener: (event: { matches: boolean }) => void) => {
            mockMatchMediaListeners.push(listener);
        })
    }));
    Object.defineProperty(globalThis, 'window', {
        configurable: true,
        writable: true,
        value: { location: { hash: options.hash ?? '' }, matchMedia }
    });
    Object.defineProperty(globalThis, 'document', {
        configurable: true,
        writable: true,
        value: {
            documentElement: {
                style: {
                    setProperty: jest.fn((name: string, value: string) => mockCssVariables.set(name, value)),
                    getPropertyValue: jest.fn((name: string) => mockCssVariables.get(name) ?? '')
                }
            },
            querySelector: jest.fn(() => mockMetaElement),
            addEventListener: jest.fn((name: string, listener: (...args: any[]) => void) => {
                mockDocumentListeners.set(name, listener);
            }),
            dispatchEvent: jest.fn((event: Event) => {
                mockDocumentListeners.get(event.type)?.(event);
                return true;
            })
        }
    });
    mockMetaElement.content = 'initial';
}

beforeEach(() => {
    jest.clearAllMocks();
    mockF7Handlers.clear();
    mockDocumentListeners.clear();
    mockCssVariables.clear();
    mockTemplateHandlers.length = 0;
    mockMatchMediaListeners.length = 0;
    mockProduction = true;
    mockLoggedIn = true;
    mockUnlocked = true;
    mockModalShowing = false;
    mockThemePreference = 'system';
    mockSystemTheme = 'light';
    mockStandalone = false;
    mockLastNotificationOptions = null;
    mockRootStore.currentNotification = null;
    mockSettingsStore.appSettings.theme = 'system';
    mockSettingsStore.appSettings.timeZone = 'Asia/Shanghai';
    mockSettingsStore.appSettings.fontSize = 16;
    mockSettingsStore.appSettings.applicationLock = false;
    mockSettingsStore.appSettings.autoUpdateExchangeRatesData = true;
    mockEnvironmentStore.framework7DarkMode = false;
    mockF7.darkMode = false;
    mockGetLatestExchangeRates.mockResolvedValue(undefined);
    mockNotificationOpen.mockImplementation(function (this: any) { return this; });
    mockNotificationCreate.mockImplementation((options: any) => {
        mockLastNotificationOptions = options;
        return {
            close: mockNotificationClose,
            destroy: mockNotificationDestroy,
            open: mockNotificationOpen
        };
    });
    installBrowserState();
});

describe('DesktopApp production-loaded shell behavior', () => {
    test('initializes locale, theme, amount colors, exchange rates and service worker', () => {
        installBrowserState({ hash: '#/statistics?tab=month' });
        const bindings = setup(DesktopApp);

        expect(bindings.initialRoutePath).toBe('/statistics');
        expect(mockThemeChange).toHaveBeenCalledWith('light');
        expect(mockInitLocale).toHaveBeenCalledWith('zh-Hans', 'Asia/Shanghai');
        expect(mockUpdateLocalizedDefaults).toHaveBeenCalledWith({ locale: 'zh-Hans' });
        expect(mockSetAmountColors).toHaveBeenCalledWith('red', 'green');
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: true, force: false });
        expect(mockRegisterServiceWorker).toHaveBeenCalledWith('./sw.js', {
            registrationOptions: { scope: './' }
        });
        expect(mockLogger.info).toHaveBeenCalled();

        document.dispatchEvent(new Event('DOMContentLoaded'));
        expect(mockInitMapProvider).toHaveBeenCalledWith('zh-CN');

        mockMatchMediaListeners[0]?.({ matches: true });
        mockMatchMediaListeners[0]?.({ matches: false });
        expect(mockThemeChange).toHaveBeenNthCalledWith(2, 'dark');
        expect(mockThemeChange).toHaveBeenNthCalledWith(3, 'light');
    });

    test('honors route and lock guards, watches notifications and renders the shell', async () => {
        installBrowserState({ hash: '#/verify_email?token=synthetic' });
        mockSettingsStore.appSettings.applicationLock = true;
        mockUnlocked = false;
        mockProduction = false;
        const bindings = setup(DesktopApp);
        expect(bindings.initialRoutePath).toBe('/verify_email');
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();
        expect(mockRegisterServiceWorker).not.toHaveBeenCalled();

        mockRootStore.currentNotification = 'Fresh notification';
        await flush();
        expect(bindings.currentNotificationContent.value).toBe('Fresh notification');
        expect(bindings.showNotification.value).toBe(true);

        mockSettingsStore.appSettings.theme = 'light';
        mockMatchMediaListeners[0]?.({ matches: true });
        expect(mockThemeChange).not.toHaveBeenCalledWith('dark');

        const html = await render(DesktopApp, [
            'v-app', 'router-view', 'v-snackbar', 'v-tooltip'
        ]);
        expect(html).toContain('tt:global.app.title');
        expect(html).toContain('/logo.png');
        for (const handler of mockTemplateHandlers) handler(false);
        await flush(2);
    });

    test('normalizes absent and malformed hashes without starting a locked session', () => {
        installBrowserState({ hash: '#/reports' });
        mockSettingsStore.appSettings.applicationLock = true;
        mockUnlocked = false;
        expect(setup(DesktopApp).initialRoutePath).toBe('/reports');
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();

        mockUnlocked = true;
        mockSettingsStore.appSettings.autoUpdateExchangeRatesData = false;
        setup(DesktopApp);
        expect(mockLogger.info).toHaveBeenCalled();
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();

        mockLoggedIn = false;
        installBrowserState({ hash: '#not-a-route' });
        expect(setup(DesktopApp).initialRoutePath).toBe('/');
        window.location.hash = '';
        expect(setup(DesktopApp).initialRoutePath).toBe('/');
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();
    });
});

describe('MobileApp production-loaded shell behavior', () => {
    test('initializes Framework7, theme variables, locale and startup exchange rates', () => {
        const bindings = setup(MobileApp);
        expect(bindings.f7params.value.colors.primary).toBe('primary:light');
        expect(bindings.f7params.value.serviceWorker.path).toBe('./sw.js');
        expect(bindings.f7params.value.view.browserHistory).toBe(true);
        expect(mockSetAppFontSize).toHaveBeenCalledWith(16);
        expect(mockEnvironmentStore.framework7DarkMode).toBe(false);
        expect(mockF7.on).toHaveBeenCalled();
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: true, force: false });
        expect(document.documentElement.style.getPropertyValue('--shell-theme')).toBe('light');

        document.dispatchEvent(new Event('DOMContentLoaded'));
        expect(mockInitMapProvider).toHaveBeenCalledWith('zh-CN');
    });

    test('tracks push and regular backdrops, modal cleanup and dark mode changes', () => {
        setup(MobileApp);
        const meta = document.querySelector('meta[name=theme-color]') as HTMLMetaElement;

        mockF7Handlers.get('actionsOpen')?.({ push: true, opened: true });
        expect(meta.content).toBe('push:light');
        mockF7Handlers.get('actionsClose')?.({ push: true, opened: false });
        expect(meta.content).toBe('default:light');
        mockF7Handlers.get('dialogOpen')?.({ opened: true });
        expect(meta.content).toBe('backdrop:light');
        mockF7Handlers.get('dialogClose')?.({ opened: false });
        expect(meta.content).toBe('default:light');
        for (const event of ['popoverOpen', 'popoverClose', 'popupOpen', 'popupClose', 'sheetOpen', 'sheetClose']) {
            mockF7Handlers.get(event)?.({ opened: event.endsWith('Open') });
        }

        mockModalShowing = false;
        mockF7Handlers.get('pageBeforeOut')?.();
        expect(mockF7Close.actions).not.toHaveBeenCalled();
        mockModalShowing = true;
        mockF7Handlers.get('pageBeforeOut')?.();
        expect(mockF7Close.actions).toHaveBeenCalledWith('.actions-modal.modal-in', false);
        expect(mockF7Close.dialog).toHaveBeenCalledWith('.dialog.modal-in', false);
        expect(mockF7Close.popover).toHaveBeenCalledWith('.popover.modal-in', false);
        expect(mockF7Close.popup).toHaveBeenCalledWith('.popup.modal-in', false);
        expect(mockF7Close.sheet).toHaveBeenCalledWith('.sheet-modal.modal-in', false);

        mockF7Handlers.get('darkModeChange')?.(true);
        expect(mockEnvironmentStore.framework7DarkMode).toBe(true);
        expect(meta.content).toBe('default:dark');
    });

    test('opens, replaces and closes notifications through the root store contract', async () => {
        const bindings = setup(MobileApp);
        mockRootStore.currentNotification = 'First notification';
        await flush();
        expect(mockNotificationCreate).toHaveBeenCalled();
        expect(mockNotificationOpen).toHaveBeenCalled();
        expect(mockLastNotificationOptions.text).toBe('First notification');

        mockRootStore.currentNotification = 'Replacement notification';
        await flush();
        expect(mockNotificationClose).toHaveBeenCalled();
        expect(mockNotificationDestroy).toHaveBeenCalled();
        expect(mockLastNotificationOptions.text).toBe('Replacement notification');

        mockLastNotificationOptions.on.close();
        expect(mockRootStore.currentNotification).toBeNull();
        await flush();
        expect(bindings.notification.value).toBeNull();
    });

    test('disables browser history in iOS standalone mode and renders the mobile shell', async () => {
        mockStandalone = true;
        mockProduction = false;
        mockLoggedIn = false;
        installBrowserState({ platform: 'iPhone', appVersion: 'Mobile Safari' });
        const bindings = setup(MobileApp);
        expect(bindings.f7params.value.view.browserHistory).toBe(false);
        expect(bindings.f7params.value.serviceWorker.path).toBeUndefined();
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();

        mockThemePreference = 'dark';
        expect(setup(MobileApp).f7params.value.colors.primary).toBe('primary:dark');

        mockLoggedIn = true;
        mockSettingsStore.appSettings.applicationLock = true;
        mockUnlocked = false;
        setup(MobileApp);
        mockUnlocked = true;
        mockSettingsStore.appSettings.autoUpdateExchangeRatesData = false;
        setup(MobileApp);
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();

        const html = await render(MobileApp, ['f7-app', 'f7-view']);
        expect(html).toContain('id="main-view"');
        expect(html).toContain('safe-areas');
    });
});
