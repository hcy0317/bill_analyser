import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockRouter = { replace: jest.fn() };
const mockThemeChange = jest.fn();
const mockInitLocale = jest.fn<(...args: any[]) => { locale: string }>(() => ({ locale: 'zh-Hans' }));
const mockSetExpenseAndIncomeAmountColor = jest.fn();
const mockLock = jest.fn();
const mockLogout = jest.fn<() => Promise<void>>();
const mockClearAppSettings = jest.fn();
const mockUpdateLocalizedDefaultSettings = jest.fn();
const mockSetTheme = jest.fn<(value: string) => void>();
const mockShowAddDialog = jest.fn();
let mockMdAndDown: any;
let mockScheduledEnabled = true;
const mockTemplateHandlers: Array<{ name: string; handler: (...args: any[]) => unknown }> = [];
const mockNativeClickHandlers: Array<(...args: any[]) => unknown> = [];

const mockSettingsStore = actualVue.reactive({
    appSettings: {
        theme: 'system',
        timeZone: 'Asia/Shanghai',
        showAddTransactionButtonInDesktopNavbar: true,
        applicationLock: true
    },
    clearAppSettings: mockClearAppSettings,
    updateLocalizedDefaultSettings: mockUpdateLocalizedDefaultSettings,
    setTheme: mockSetTheme
});
const mockUserStore = actualVue.reactive({
    currentUserNickname: 'Alice',
    currentUserBasicInfo: { avatar: 'avatar.png' },
    currentUserLanguage: 'zh-Hans',
    currentUserExpenseAmountColor: 'red',
    currentUserIncomeAmountColor: 'green',
    getUserAvatarUrl: jest.fn(() => 'https://example.test/avatar.png')
});
const mockRootStore = { lock: mockLock, logout: mockLogout };
const mockDesktopStore = { setShowAddTransactionDialogInTransactionList: mockShowAddDialog };

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});
jest.mock('vuetify', () => ({
    useDisplay: () => ({ mdAndDown: mockMdAndDown }),
    useTheme: () => ({ change: mockThemeChange })
}));
jest.mock('vue-router', () => ({ useRouter: () => mockRouter }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}`, initLocale: mockInitLocale })
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => mockRootStore }));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/desktopPage.ts', () => ({ useDesktopPageStore: () => mockDesktopStore }));
jest.mock('@/consts/asset.ts', () => ({ APPLICATION_LOGO_PATH: '/logo.png' }));
jest.mock('@/core/theme.ts', () => ({
    SYSTEM_THEME_PREFERENCE: 'system',
    getNextQuickThemePreference: (value: string) => value === 'system' ? 'light' : value === 'light' ? 'dark' : 'system',
    isDarkApplicationTheme: (value: string) => value === 'dark',
    normalizeThemePreference: (value: string) => value || 'system',
    resolveThemePreference: (value: string, system: string) => value === 'system' ? system : `resolved:${value}`
}));
jest.mock('@/lib/server_settings.ts', () => ({ isUserScheduledTransactionEnabled: () => mockScheduledEnabled }));
jest.mock('@/lib/ui/common.ts', () => ({
    getSystemTheme: () => 'dark',
    setExpenseAndIncomeAmountColor: (...args: unknown[]) => mockSetExpenseAndIncomeAmountColor(...args)
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: actualVue.defineComponent({ name: 'SnackBarStub', setup: () => () => actualVue.h('div', { class: 'snackbar-stub' }) })
}));

const MainLayout = require('@/views/desktop/MainLayout.vue').default as any;

function setupLayout(): any {
    return MainLayout.setup({}, { attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined });
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function collectNativeClickHandlers(node: any, handlers: Array<(...args: any[]) => unknown>): void {
    if (!node) return;
    if (Array.isArray(node)) {
        for (const child of node) collectNativeClickHandlers(child, handlers);
        return;
    }
    if (typeof node !== 'object') return;
    if ((node.type === 'a' || String(node.props?.class ?? '').includes('layout-overlay'))
        && typeof node.props?.onClick === 'function') {
        handlers.push(node.props.onClick);
    }
    collectNativeClickHandlers(node.children, handlers);
}

async function renderLayout(configure?: (bindings: any) => void): Promise<string> {
    const { createSSRApp, defineComponent, h } = actualVue;
    const { renderToString } = jest.requireActual('vue/server-renderer') as any;
    const RuntimeLayout = {
        ...MainLayout,
        setup(_props: unknown, context: any) {
            const bindings = MainLayout.setup({}, context);
            configure?.(bindings);
            return bindings;
        },
        render(_ctx: any, _cache: any[], $props: any, $setup: any, $data: any, $options: any) {
            const vnode = MainLayout.render(_ctx, _cache, $props, $setup, $data, $options);
            collectNativeClickHandlers(vnode, mockNativeClickHandlers);
            return vnode;
        }
    };
    const Stub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => {
            for (const [name, value] of Object.entries(attrs)) {
                if (!name.startsWith('on')) continue;
                for (const handler of Array.isArray(value) ? value : [value]) {
                    if (typeof handler === 'function') {
                        mockTemplateHandlers.push({ name, handler: handler as (...args: any[]) => unknown });
                    }
                }
            }
            return h(
                'div',
                attrs,
                Object.values(slots).flatMap((slot: any) => {
                    try {
                        return slot?.({}) ?? [];
                    } catch {
                        return [];
                    }
                })
            );
        }
    });
    const app = createSSRApp(RuntimeLayout);
    for (const name of [
        'router-link', 'router-view', 'perfect-scrollbar', 'switch-to-mobile-dialog',
        'v-icon', 'v-btn', 'v-tooltip', 'v-spacer', 'v-avatar', 'v-img', 'v-menu',
        'v-list', 'v-list-item', 'v-list-item-action', 'v-list-item-title', 'v-divider',
        'v-overlay', 'v-progress-circular'
    ]) app.component(name, Stub);
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockMdAndDown = actualVue.ref(false);
    mockScheduledEnabled = true;
    mockSettingsStore.appSettings.theme = 'system';
    mockSettingsStore.appSettings.timeZone = 'Asia/Shanghai';
    mockSettingsStore.appSettings.showAddTransactionButtonInDesktopNavbar = true;
    mockSettingsStore.appSettings.applicationLock = true;
    mockUserStore.currentUserNickname = 'Alice';
    mockUserStore.currentUserBasicInfo = { avatar: 'avatar.png' };
    mockUserStore.currentUserLanguage = 'zh-Hans';
    mockUserStore.currentUserExpenseAmountColor = 'red';
    mockUserStore.currentUserIncomeAmountColor = 'green';
    mockUserStore.getUserAvatarUrl.mockReturnValue('https://example.test/avatar.png');
    mockLogout.mockResolvedValue(undefined);
    mockTemplateHandlers.length = 0;
    mockNativeClickHandlers.length = 0;
    mockSetTheme.mockImplementation(value => {
        mockSettingsStore.appSettings.theme = value;
    });
});

describe('desktop MainLayout production-loaded behavior', () => {
    test('derives navigation, user, avatar, lock, and all quick-theme states', () => {
        const bindings = setupLayout();
        expect(bindings.mdAndDown.value).toBe(false);
        expect(bindings.currentNickName.value).toBe('Alice');
        expect(bindings.currentUserAvatar.value).toBe('https://example.test/avatar.png');
        expect(bindings.currentTheme.value).toBe('system');
        expect(bindings.currentThemeIcon.value).toBeTruthy();
        expect(bindings.showAddTransactionButtonInDesktopNavbar.value).toBe(true);
        expect(bindings.isEnableApplicationLock.value).toBe(true);

        bindings.currentTheme.value = 'system';
        expect(mockSetTheme).not.toHaveBeenCalled();
        bindings.currentTheme.value = 'light';
        expect(mockSetTheme).toHaveBeenCalledWith('light');
        expect(mockThemeChange).toHaveBeenCalledWith('resolved:light');
        expect(bindings.currentThemeIcon.value).toBeTruthy();
        bindings.currentTheme.value = 'dark';
        expect(bindings.currentThemeIcon.value).toBeTruthy();

        mockUserStore.currentUserNickname = '';
        mockUserStore.getUserAvatarUrl.mockReturnValue(null);
        expect(setupLayout().currentNickName.value).toBe('tt:User');
        expect(setupLayout().currentUserAvatar.value).toBeNull();
    });

    test('tracks scroll and overlay state, locks, and opens the transaction dialog', () => {
        const bindings = setupLayout();
        bindings.handleNavScroll({ target: { scrollTop: 10 } });
        expect(bindings.isVerticalNavScrolled.value).toBe(true);
        bindings.handleNavScroll({ target: { scrollTop: 0 } });
        expect(bindings.isVerticalNavScrolled.value).toBe(false);

        bindings.lock();
        expect(mockLock).toHaveBeenCalledTimes(1);
        expect(mockRouter.replace).toHaveBeenCalledWith('/unlock');
        bindings.showAddDialogInTransactionListPage();
        expect(mockShowAddDialog).toHaveBeenCalledTimes(1);
    });

    test('logout success clears loading, resets localized settings and routes to login', async () => {
        const bindings = setupLayout();
        bindings.logout();
        expect(bindings.logouting.value).toBe(true);
        expect(bindings.showLoading.value).toBe(true);
        await flush();

        expect(bindings.logouting.value).toBe(false);
        expect(bindings.showLoading.value).toBe(false);
        expect(mockClearAppSettings).toHaveBeenCalledTimes(1);
        expect(mockInitLocale).toHaveBeenCalledWith('zh-Hans', 'Asia/Shanghai');
        expect(mockUpdateLocalizedDefaultSettings).toHaveBeenCalledWith({ locale: 'zh-Hans' });
        expect(mockSetExpenseAndIncomeAmountColor).toHaveBeenCalledWith('red', 'green');
        expect(mockRouter.replace).toHaveBeenCalledWith('/login');
    });

    test('logout exposes only unprocessed errors and always clears busy state', async () => {
        for (const error of [{ processed: false }, { processed: true }]) {
            mockLogout.mockRejectedValueOnce(error);
            const bindings = setupLayout();
            const snackbar = { showError: jest.fn() };
            bindings.snackbar.value = snackbar;
            bindings.logout();
            await flush();
            expect(bindings.logouting.value).toBe(false);
            expect(bindings.showLoading.value).toBe(false);
            if (error.processed) expect(snackbar.showError).not.toHaveBeenCalled();
            else expect(snackbar.showError).toHaveBeenCalledWith(error);
        }
    });

    test('renders desktop and mobile template branches with optional navigation and account controls', async () => {
        const desktop = await renderLayout();
        expect(desktop).toContain('data-testid="desktop.layout.root"');
        expect(desktop).toContain('data-testid="desktop.nav.schedules"');
        expect(desktop).toContain('tt:Lock Application');
        expect(desktop).toContain('https://example.test/avatar.png');

        mockMdAndDown.value = true;
        mockScheduledEnabled = false;
        mockSettingsStore.appSettings.showAddTransactionButtonInDesktopNavbar = false;
        mockSettingsStore.appSettings.applicationLock = false;
        mockUserStore.currentUserNickname = '';
        mockUserStore.getUserAvatarUrl.mockReturnValue(null);
        const mobile = await renderLayout(bindings => {
            bindings.showVerticalOverlayMenu.value = true;
            bindings.isVerticalNavScrolled.value = true;
            bindings.showLoading.value = true;
            bindings.showMobileQrCode.value = true;
        });
        expect(mobile).toContain('layout-overlay-nav');
        expect(mobile).toContain('overlay-nav');
        expect(mobile).not.toContain('data-testid="desktop.nav.schedules"');
        expect(mobile).not.toContain('tt:Lock Application');
        expect(mobile).toContain('tt:User');

        expect(mockNativeClickHandlers.length).toBeGreaterThanOrEqual(2);
        for (const handler of mockNativeClickHandlers) handler();

        for (const { name, handler } of mockTemplateHandlers) {
            if (name === 'onPsScrollY') handler({ target: { scrollTop: 4 } });
            else if (name.startsWith('onUpdate:')) handler(false);
            else handler({ target: { scrollTop: 0 } });
            await flush(2);
        }
    });
});
