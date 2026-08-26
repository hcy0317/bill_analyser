/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const { proxyRefs, reactive, ref } = actualVue;

const mockShowToast = jest.fn();
const mockShowConfirm = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn();
const mockHideLoading = jest.fn();
const mockThemeChange = jest.fn();
const mockSetTheme = jest.fn();
const mockSetSwipeBack = jest.fn();
const mockSetAnimate = jest.fn();
const mockSetNavbarButton = jest.fn();
const mockClearAppSettings = jest.fn();
const mockUpdateLocalizedDefaults = jest.fn();
const mockSetAmountColors = jest.fn();
const mockLoadAccounts = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadCategories = jest.fn<(...args: any[]) => Promise<void>>();
const mockLogout = jest.fn<() => Promise<void>>();
const mockSnackbarError = jest.fn();
const mockChooseImportDirectory = jest.fn<() => Promise<{ name: string }>>();
const mockClearImportDirectory = jest.fn<() => Promise<void>>();
const mockSetImportDirectoryName = jest.fn();

const mockBase = {
    loadingAccounts: ref(false),
    loadingTransactionCategories: ref(false),
    allThemes: ref([{ value: 'light', name: 'Light' }, { value: 'dark', name: 'Dark' }]),
    allTimezones: ref([
        { name: 'UTC', displayNameWithUtcOffset: 'UTC +00:00' },
        { name: 'Asia/Shanghai', displayNameWithUtcOffset: 'Shanghai +08:00' },
    ]),
    allTimezoneTypesUsedForStatistics: ref([{ type: 1, displayName: 'Transaction Time' }]),
    allCurrencySortingTypes: ref([{ type: 2, displayName: 'Code' }]),
    allAutoSaveTransactionDraftTypes: ref([{ value: 'enabled', name: 'Enabled' }]),
    hasAnyAccount: ref(true),
    hasAnyVisibleAccount: ref(true),
    hasAnyTransactionCategory: ref(true),
    timeZone: ref('UTC'),
    isAutoUpdateExchangeRatesData: ref(false),
    showAccountBalance: ref(true),
    showAmountInHomePage: ref(true),
    itemsCountInTransactionListPage: ref(20),
    timezoneUsedForStatisticsInHomePage: ref(1),
    showTotalAmountInTransactionListPage: ref(true),
    showTagInTransactionListPage: ref(false),
    autoSaveTransactionDraft: ref('enabled'),
    isAutoGetCurrentGeoLocation: ref(false),
    currencySortByInExchangeRatesPage: ref(2),
    accountsIncludedInHomePageOverviewDisplayContent: ref('All accounts'),
    accountsIncludedInTotalDisplayContent: ref('Visible accounts'),
    transactionCategoriesIncludedInHomePageOverviewDisplayContent: ref('Income, Expense'),
};

const mockSettingsStore = reactive({
    appSettings: {
        theme: 'light',
        swipeBack: true,
        animate: true,
        applicationLock: false,
        showAddTransactionButtonInDesktopNavbar: true,
        timeZone: 'UTC',
        billImportDefaultDirectoryName: '',
    },
    setTheme(value: string) {
        mockSetTheme(value);
        this.appSettings.theme = value;
    },
    setEnableSwipeBack(value: boolean) {
        mockSetSwipeBack(value);
        this.appSettings.swipeBack = value;
    },
    setEnableAnimate(value: boolean) {
        mockSetAnimate(value);
        this.appSettings.animate = value;
    },
    setShowAddTransactionButtonInDesktopNavbar(value: boolean) {
        mockSetNavbarButton(value);
        this.appSettings.showAddTransactionButtonInDesktopNavbar = value;
    },
    setBillImportDefaultDirectoryName(value: string) {
        mockSetImportDirectoryName(value);
        this.appSettings.billImportDefaultDirectoryName = value;
    },
    clearAppSettings: mockClearAppSettings,
    updateLocalizedDefaultSettings: mockUpdateLocalizedDefaults,
});

const mockUserStore = reactive({
    currentUserNickname: 'Alice',
    currentUserLanguage: 'zh-Hans',
    currentUserExpenseAmountColor: 'red',
    currentUserIncomeAmountColor: 'green',
});
const mockExchangeRatesStore = reactive({ exchangeRatesLastUpdateTime: 1_700_000_000 });

jest.mock('vue', () => ({
    ...actualVue,
    useTemplateRef: () => actualVue.ref({ showError: mockSnackbarError }),
}));
jest.mock('vuetify', () => ({ useTheme: () => ({ change: mockThemeChange }) }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getAllEnableDisableOptions: () => [{ value: true, displayName: 'Enabled' }],
        formatUnixTimeToLongDate: (value: number) => `date:${value}`,
        initLocale: (language: string, timeZone: string) => ({ language, timeZone }),
    }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast, showConfirm: mockShowConfirm }),
    showLoading: (predicate: () => boolean) => mockShowLoading(predicate()),
    hideLoading: () => mockHideLoading(),
}));
jest.mock('@/views/base/settings/AppSettingsPageBase.ts', () => ({
    useAppSettingPageBase: () => mockBase,
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => ({ logout: mockLogout }) }));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/exchangeRates.ts', () => ({ useExchangeRatesStore: () => mockExchangeRatesStore }));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({ loadAllAccounts: mockLoadAccounts }),
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({ loadAllCategories: mockLoadCategories }),
}));
jest.mock('@/core/theme.ts', () => ({
    getThemeFamilyOptionValue: (value: string) => `family:${value}`,
    resolveThemePreference: (value: string, system: string) => `${value}:${system}`,
}));
jest.mock('@/core/category.ts', () => ({ CategoryType: { Income: 1, Expense: 2 } }));
jest.mock('@/lib/common.ts', () => ({
    findNameByValue: (items: any[], value: string) => items.find(item => item.value === value)?.name ?? '',
}));
jest.mock('@/lib/version.ts', () => ({
    getClientDisplayVersion: () => '1.2.3',
    getDesktopVersionPath: () => '/desktop',
}));
jest.mock('@/lib/server_settings.ts', () => ({ isUserScheduledTransactionEnabled: () => true }));
jest.mock('@/lib/ui/common.ts', () => ({
    setExpenseAndIncomeAmountColor: (...args: any[]) => mockSetAmountColors(...args),
    getSystemTheme: () => 'system-dark',
}));
jest.mock('@/lib/importDirectoryPreference.ts', () => ({
    chooseDefaultImportDirectory: () => mockChooseImportDirectory(),
    clearDefaultImportDirectory: () => mockClearImportDirectory(),
}));
for (const path of [
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/common/cards/AccountFilterSettingsCard.vue',
    '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue',
]) {
    jest.mock(path, () => ({ __esModule: true, default: { name: 'SettingsShellStub' } }));
}

const MobileSettingsPage = require('@/views/mobile/SettingsPage.vue').default as any;
const AppBasicSettingTab = require('@/views/desktop/app/settings/tabs/AppBasicSettingTab.vue').default as any;
const DefaultImportDirectorySettingsCard = require(
    '@/views/desktop/app/settings/tabs/DefaultImportDirectorySettingsCard.vue'
).default as any;

const originalLocation = Object.getOwnPropertyDescriptor(globalThis, 'location');
const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
const mockReload = jest.fn();
const mockReplace = jest.fn();
const mockBrowserLocation = { reload: mockReload, replace: mockReplace };
Object.defineProperty(globalThis, 'location', {
    configurable: true,
    value: mockBrowserLocation,
});
Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: { location: mockBrowserLocation },
});
const consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);

function setup(component: any, props: Record<string, unknown> = {}): any {
    return component.setup(reactive(props), {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn(),
    });
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function collectCallbacks(node: any): Array<{ name: string; callback: (value?: any) => unknown }> {
    const result: Array<{ name: string; callback: (value?: any) => unknown }> = [];
    const seen = new Set<any>();
    const visit = (candidate: any): void => {
        if (candidate == null) return;
        if (Array.isArray(candidate)) return candidate.forEach(visit);
        if (typeof candidate !== 'object' || seen.has(candidate)) return;
        seen.add(candidate);
        for (const [name, value] of Object.entries(candidate.props ?? {})) {
            if (name.startsWith('on') && typeof value === 'function') {
                result.push({ name, callback: value as (value?: any) => unknown });
            }
        }
        visit(candidate.children);
        if (candidate.children && typeof candidate.children === 'object' && !Array.isArray(candidate.children)) {
            for (const slot of Object.values(candidate.children)) {
                if (typeof slot === 'function') visit((slot as () => unknown)());
            }
        }
    };
    visit(node);
    return result;
}

function render(component: any, bindings: any) {
    return collectCallbacks(component.render({}, [], {}, proxyRefs(bindings), {}, {}));
}

function invokeLastConfirm(): void {
    const callback = mockShowConfirm.mock.calls.at(-1)?.[1] as (() => void) | undefined;
    callback?.();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockLoadAccounts.mockResolvedValue(undefined);
    mockLoadCategories.mockResolvedValue(undefined);
    mockLogout.mockResolvedValue(undefined);
    mockSettingsStore.appSettings.theme = 'light';
    mockSettingsStore.appSettings.swipeBack = true;
    mockSettingsStore.appSettings.animate = true;
    mockSettingsStore.appSettings.applicationLock = false;
    mockSettingsStore.appSettings.showAddTransactionButtonInDesktopNavbar = true;
    mockSettingsStore.appSettings.billImportDefaultDirectoryName = '';
    mockChooseImportDirectory.mockResolvedValue({ name: '账单目录' });
    mockClearImportDirectory.mockResolvedValue(undefined);
    mockUserStore.currentUserNickname = 'Alice';
    mockExchangeRatesStore.exchangeRatesLastUpdateTime = 1_700_000_000;
    mockBase.timeZone.value = 'UTC';
    mockBase.loadingAccounts.value = false;
    mockBase.loadingTransactionCategories.value = false;
});

afterAll(() => {
    if (originalLocation) Object.defineProperty(globalThis, 'location', originalLocation);
    if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow);
    consoleWarnSpy.mockRestore();
});

describe('mobile SettingsPage production behavior', () => {
    const router = { navigate: jest.fn(), back: jest.fn() };

    test('projects user, theme, timezone, lock, exchange date, and version state', () => {
        const bindings = setup(MobileSettingsPage, { f7router: router });
        expect(bindings.currentNickName.value).toBe('Alice');
        mockUserStore.currentUserNickname = '';
        expect(bindings.currentNickName.value).toBe('tt:User');
        expect(bindings.currentTheme.value).toBe('family:light');
        expect(bindings.currentTimezoneName.value).toBe('UTC +00:00');
        mockBase.timeZone.value = 'missing';
        expect(bindings.currentTimezoneName.value).toBe('');
        expect(bindings.isEnableApplicationLock.value).toBe(false);
        expect(bindings.exchangeRatesLastUpdateDate.value).toBe('date:1700000000');
        mockExchangeRatesStore.exchangeRatesLastUpdateTime = 0;
        expect(bindings.exchangeRatesLastUpdateDate.value).toBe('');
        expect(bindings.version).toBe('1.2.3');
    });

    test('updates changed theme and interaction preferences but ignores equal values', () => {
        const bindings = setup(MobileSettingsPage, { f7router: router });
        bindings.currentTheme.value = 'light';
        expect(mockSetTheme).not.toHaveBeenCalled();
        expect(mockReload).not.toHaveBeenCalled();
        bindings.currentTheme.value = 'dark';
        expect(mockSetTheme).toHaveBeenCalledWith('dark');
        expect(mockReload).toHaveBeenCalledTimes(1);

        bindings.isEnableSwipeBack.value = false;
        bindings.isEnableSwipeBack.value = false;
        expect(mockSetSwipeBack).toHaveBeenCalledTimes(1);
        bindings.isEnableAnimate.value = false;
        bindings.isEnableAnimate.value = false;
        expect(mockSetAnimate).toHaveBeenCalledTimes(1);

        const callbacks = render(MobileSettingsPage, bindings);
        callbacks.filter(item => ['onUpdate:show', 'onToggle:change'].includes(item.name))
            .forEach((item, index) => item.callback(index % 2 === 0));
        expect(callbacks.length).toBeGreaterThan(0);
    });

    test('switches shell after confirmation', () => {
        const bindings = setup(MobileSettingsPage, { f7router: router });
        bindings.switchToDesktopVersion();
        expect(mockShowConfirm).toHaveBeenCalledWith(expect.any(String), expect.any(Function));
        invokeLastConfirm();
        expect(mockReplace).toHaveBeenCalledWith('/desktop');
    });

    test('logs out, resets localized settings, colors, and navigation', async () => {
        const bindings = setup(MobileSettingsPage, { f7router: router });
        bindings.logout();
        invokeLastConfirm();
        expect(bindings.logouting.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(true);
        await flush();
        expect(bindings.logouting.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockClearAppSettings).toHaveBeenCalled();
        expect(mockUpdateLocalizedDefaults).toHaveBeenCalledWith({ language: 'zh-Hans', timeZone: 'UTC' });
        expect(mockSetAmountColors).toHaveBeenCalledWith('red', 'green');
        expect(router.navigate).toHaveBeenCalledWith('/');
    });

    test('maps only unprocessed logout failures to toast message and raw fallbacks', async () => {
        const bindings = setup(MobileSettingsPage, { f7router: router });
        mockLogout.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.logout();
        invokeLastConfirm();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalled();

        mockLogout.mockRejectedValueOnce({ processed: false, message: 'failed' });
        bindings.logout();
        invokeLastConfirm();
        await flush();
        expect(mockShowToast).toHaveBeenLastCalledWith('failed');

        const raw = { processed: false, message: '' };
        mockLogout.mockRejectedValueOnce(raw);
        bindings.logout();
        invokeLastConfirm();
        await flush();
        expect(mockShowToast).toHaveBeenLastCalledWith(raw);
        expect(bindings.logouting.value).toBe(false);
    });
});

describe('desktop AppBasicSettingTab production behavior', () => {
    test('loads accounts and categories and projects settings controls', async () => {
        const bindings = setup(AppBasicSettingTab);
        expect(mockBase.loadingAccounts.value).toBe(true);
        expect(mockLoadAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockLoadCategories).toHaveBeenCalledWith({ force: false });
        await flush();
        expect(mockBase.loadingAccounts.value).toBe(false);
        expect(mockBase.loadingTransactionCategories.value).toBe(false);
        expect(bindings.enableDisableOptions.value).toEqual([{ value: true, displayName: 'Enabled' }]);
        expect(bindings.currentTheme.value).toBe('family:light');
        bindings.currentTheme.value = 'dark';
        expect(mockSetTheme).toHaveBeenCalledWith('dark');
        expect(mockThemeChange).toHaveBeenCalledWith('dark:system-dark');
        bindings.currentTheme.value = 'dark';
        expect(mockThemeChange).toHaveBeenCalledTimes(1);
        bindings.showAddTransactionButtonInDesktopNavbar.value = false;
        expect(mockSetNavbarButton).toHaveBeenCalledWith(false);
    });

    test('maps unprocessed dependency failures to snackbar and suppresses processed failures', async () => {
        mockLoadAccounts.mockRejectedValueOnce({ processed: false, message: 'accounts failed' });
        mockLoadCategories.mockRejectedValueOnce({ processed: true, message: 'handled categories' });
        setup(AppBasicSettingTab);
        await flush();
        expect(mockSnackbarError).toHaveBeenCalledWith({ processed: false, message: 'accounts failed' });
        expect(mockSnackbarError).not.toHaveBeenCalledWith({ processed: true, message: 'handled categories' });
        expect(mockBase.loadingAccounts.value).toBe(false);
        expect(mockBase.loadingTransactionCategories.value).toBe(false);
    });

    test('stores and resets the local-only default import directory', async () => {
        const bindings = setup(DefaultImportDirectorySettingsCard);
        await bindings.chooseDirectory();
        expect(mockSetImportDirectoryName).toHaveBeenLastCalledWith('账单目录');
        expect(bindings.displayName.value).toBe('账单目录');

        await bindings.resetDirectory();
        expect(mockSetImportDirectoryName).toHaveBeenLastCalledWith('');
        expect(mockClearImportDirectory).toHaveBeenCalledTimes(1);

        mockChooseImportDirectory.mockRejectedValueOnce(new Error('picker failed'));
        await bindings.chooseDirectory();
        expect(mockSnackbarError).toHaveBeenLastCalledWith('picker failed');
    });

    test('executes production template update, click, and settings-change handlers', async () => {
        const bindings = setup(AppBasicSettingTab);
        await flush();
        const callbacks = render(AppBasicSettingTab, bindings);
        callbacks.filter(item => item.name === 'onUpdate:modelValue').forEach(item => item.callback(true));
        callbacks.filter(item => item.name === 'onClick').forEach(item => item.callback());
        callbacks.filter(item => item.name === 'onSettings:change').forEach(item => item.callback());
        expect(bindings.showAccountsIncludedInHomePageOverviewDialog.value).toBe(false);
        expect(bindings.showTransactionCategoriesIncludedInHomePageOverviewDialog.value).toBe(false);
        expect(bindings.showAccountsIncludedInTotalDialog.value).toBe(false);
        expect(callbacks.length).toBeGreaterThan(10);
    });
});
