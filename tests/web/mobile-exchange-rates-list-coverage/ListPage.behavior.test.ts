import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockShowAlert = jest.fn();
const mockShowToast = jest.fn();
const mockOpenExternalUrl = jest.fn();
const mockShowLoading = jest.fn();
const mockHideLoading = jest.fn();
const mockOnSwipeoutDeleted = jest.fn();
const mockGetLatestExchangeRates = jest.fn<(...args: any[]) => Promise<any>>();
const mockDeleteUserCustomExchangeRate = jest.fn<(...args: any[]) => Promise<any>>();
const mockSetSelectedProvider = jest.fn<(provider: string) => void>();
const mockGetConvertedAmount = jest.fn<(...args: any[]) => number | '' | null>();
const mockSetAsBaseline = jest.fn();
const mockFormatBaseAmount = jest.fn((value: number, currency: string) => `${currency}:cents:${value}`);
const mockFormatRateAmount = jest.fn((value: number) => `rate:${value}`);
const mockLocalizedDigits = jest.fn((value: string) => `localized:${value}`);

let mockDirection = 'ltr';
let mockNow = 1_000;
let mockBaseCurrency: any;
let mockBaseAmount: any;
let mockDefaultCurrency: any;
let mockExchangeRatesData: any;
let mockIsUserCustomExchangeRates: any;
let mockExchangeRatesDataUpdateTime: any;
let mockAvailableExchangeRates: any;

const mockStore: any = {
    selectedExchangeRateProvider: 'auto',
    latestExchangeRateMap: {},
    setSelectedExchangeRateProvider: mockSetSelectedProvider,
    getLatestExchangeRates: mockGetLatestExchangeRates,
    deleteUserCustomExchangeRate: mockDeleteUserCustomExchangeRate
};

type CapturedAction = {
    component: string;
    event: string;
    label: string;
    handler: (...args: any[]) => unknown;
};

const mockTemplateActions: CapturedAction[] = [];

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageTextDirection: () => mockDirection,
        getCurrentNumeralSystemType: () => ({
            digitZero: '٠',
            replaceWesternArabicDigitsToLocalizedDigits: (value: string) => mockLocalizedDigits(value)
        }),
        getCurrencyName: (currency: string) => `currency:${currency}`,
        formatAmountToLocalizedNumerals: (value: number, currency: string) => mockFormatBaseAmount(value, currency),
        formatExchangeRateAmountToWesternArabicNumerals: (value: number) => mockFormatRateAmount(value)
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showToast: mockShowToast,
        openExternalUrl: mockOpenExternalUrl
    }),
    showLoading: mockShowLoading,
    hideLoading: mockHideLoading,
    onSwipeoutDeleted: mockOnSwipeoutDeleted
}));
jest.mock('@/views/base/ExchangeRatesPageBase.ts', () => ({
    useExchangeRatesPageBase: () => ({
        baseCurrency: mockBaseCurrency,
        baseAmount: mockBaseAmount,
        defaultCurrency: mockDefaultCurrency,
        exchangeRatesData: mockExchangeRatesData,
        isUserCustomExchangeRates: mockIsUserCustomExchangeRates,
        exchangeRatesDataUpdateTime: mockExchangeRatesDataUpdateTime,
        availableExchangeRates: mockAvailableExchangeRates,
        getConvertedAmount: mockGetConvertedAmount,
        setAsBaseline: mockSetAsBaseline
    })
}));
jest.mock('@/stores/exchangeRates.ts', () => ({ useExchangeRatesStore: () => mockStore }));
jest.mock('@/core/text.ts', () => ({ TextDirection: { LTR: 'ltr', RTL: 'rtl' } }));
jest.mock('@/core/numeral.ts', () => ({
    NumeralSystem: { WesternArabicNumerals: { digitZero: '0' } }
}));
jest.mock('@/consts/transaction.ts', () => ({
    TRANSACTION_MIN_AMOUNT: -999_999_999,
    TRANSACTION_MAX_AMOUNT: 999_999_999
}));
jest.mock('@/lib/datetime.ts', () => ({ getCurrentUnixTime: () => mockNow }));

const ListPage = require('@/views/mobile/exchangerates/ListPage.vue').default as any;
const { createSSRApp, defineComponent, h, proxyRefs, ref } = jest.requireActual('vue') as any;
const { renderToString } = jest.requireActual('vue/server-renderer') as any;

function rate(currencyCode: string, value: number, name = currencyCode): any {
    return { currencyCode, currencyDisplayName: name, rate: String(value) };
}

function response(overrides: Record<string, unknown> = {}): any {
    return {
        exchangeRates: [
            { currency: 'CNY', rate: '1' },
            { currency: 'USD', rate: '0.14' },
            { currency: 'EUR', rate: '0.13' }
        ],
        dataSource: 'Bank of China',
        referenceUrl: 'https://example.test/rates',
        fallbackUsed: false,
        requestedProvider: 'boc_cn',
        effectiveProvider: 'boc_cn',
        updateTime: 1_700_000_000,
        ...overrides
    };
}

function resetBase(overrides: Record<string, unknown> = {}): void {
    mockBaseCurrency = ref(overrides['baseCurrency'] ?? 'CNY');
    mockBaseAmount = ref(overrides['baseAmount'] ?? 12_345);
    mockDefaultCurrency = ref(overrides['defaultCurrency'] ?? 'CNY');
    mockExchangeRatesData = ref(Object.prototype.hasOwnProperty.call(overrides, 'exchangeRatesData')
        ? overrides['exchangeRatesData']
        : response());
    mockIsUserCustomExchangeRates = ref(overrides['isUserCustomExchangeRates'] ?? false);
    mockExchangeRatesDataUpdateTime = ref(overrides['exchangeRatesDataUpdateTime'] ?? '2026-07-15 10:00');
    mockAvailableExchangeRates = ref(overrides['availableExchangeRates'] ?? [
        rate('CNY', 1, 'Chinese Yuan'),
        rate('USD', 0.14, 'US Dollar'),
        rate('EUR', 0.13, 'Euro')
    ]);
}

function setupPage(overrides: Record<string, unknown> = {}): { bindings: any; router: { navigate: jest.Mock } } {
    const router = { navigate: jest.fn(), ...overrides['router'] as object };
    const bindings = ListPage.setup({ f7router: router }, {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: () => undefined
    });
    return { bindings, router };
}

async function flushPromises(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
}

function vnodeText(value: unknown): string {
    if (value === null || value === undefined || typeof value === 'boolean') return '';
    if (typeof value === 'string' || typeof value === 'number') return String(value);
    if (Array.isArray(value)) return value.map(vnodeText).join(' ');
    if (typeof value === 'object') {
        const vnode = value as { children?: unknown; props?: Record<string, unknown> };
        return [
            vnodeText(vnode.children),
            vnodeText(vnode.props?.['title']),
            vnodeText(vnode.props?.['text']),
            vnodeText(vnode.props?.['header']),
            vnodeText(vnode.props?.['after'])
        ].filter(Boolean).join(' ');
    }
    return '';
}

function createStub(component: string): any {
    return defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => {
            const children = Object.values(slots).flatMap(slot => (
                typeof slot === 'function' ? (slot as any)({}) : []
            ));
            const label = [
                vnodeText(children),
                vnodeText(attrs.title),
                vnodeText(attrs.text),
                vnodeText(attrs.header),
                vnodeText(attrs.after)
            ].filter(Boolean).join(' ');
            for (const [event, value] of Object.entries(attrs)) {
                if (!event.startsWith('on')) continue;
                for (const handler of Array.isArray(value) ? value : [value]) {
                    if (typeof handler === 'function') {
                        mockTemplateActions.push({ component, event, label, handler: handler as (...args: any[]) => unknown });
                    }
                }
            }
            return h(`${component}-stub`, attrs, children);
        }
    });
}

function registerStubs(app: any): void {
    for (const name of [
        'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
        'f7-list', 'f7-list-item', 'f7-swipeout-actions', 'f7-swipeout-button', 'f7-icon',
        'f7-actions', 'f7-actions-group', 'f7-actions-button', 'f7-actions-label',
        'list-item-selection-popup', 'number-pad-sheet'
    ]) {
        app.component(name, createStub(name));
    }
    app.config.warnHandler = () => undefined;
}

async function renderPage(configure?: (bindings: any) => void): Promise<{ bindings: any; html: string; router: { navigate: jest.Mock } }> {
    mockTemplateActions.length = 0;
    const router = { navigate: jest.fn() };
    let bindings: any;
    const pageProps = { f7router: router };
    const Harness = defineComponent({
        setup: () => {
            bindings = ListPage.setup(pageProps, { emit: jest.fn(), expose: () => undefined });
            configure?.(bindings);
            const renderBindings = proxyRefs(bindings);
            return () => ListPage.render(renderBindings, [], pageProps, renderBindings, {}, {});
        }
    });
    const app = createSSRApp(Harness);
    registerStubs(app);
    const html = await renderToString(app);
    await flushPromises();
    return { bindings, html, router };
}

function action(options: { component?: string; event?: string; label?: string; handlerText?: string; occurrence?: number }): CapturedAction {
    const matches = mockTemplateActions.filter(candidate => (
        (!options.component || candidate.component === options.component)
        && (!options.event || candidate.event === options.event)
        && (!options.label || candidate.label.includes(options.label))
        && (!options.handlerText || String(candidate.handler).includes(options.handlerText))
    ));
    const occurrence = options.occurrence ?? 0;
    expect(matches.length).toBeGreaterThan(occurrence);
    return matches[occurrence]!;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockDirection = 'ltr';
    mockNow = 1_000;
    mockStore.selectedExchangeRateProvider = 'auto';
    mockStore.latestExchangeRateMap = {
        CNY: { currency: 'CNY', rate: '1' },
        USD: { currency: 'USD', rate: '0.14' },
        EUR: { currency: 'EUR', rate: '0.13' }
    };
    mockSetSelectedProvider.mockImplementation(provider => {
        mockStore.selectedExchangeRateProvider = provider;
    });
    mockGetLatestExchangeRates.mockResolvedValue(response());
    mockDeleteUserCustomExchangeRate.mockResolvedValue(true);
    mockGetConvertedAmount.mockImplementation((amount: number, from: any, to: any) => (
        from && to ? amount * Number(to.rate) / Number(from.rate) : ''
    ));
    resetBase();
});

describe('mobile exchange-rates ListPage production-loaded behavior', () => {
    test('mount loads cached rates and reports only a missing base currency', async () => {
        setupPage();
        await flushPromises();
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: true, force: false });
        expect(mockShowToast).not.toHaveBeenCalledWith('There is no exchange rates data for your default currency');

        jest.clearAllMocks();
        mockGetLatestExchangeRates.mockResolvedValue(response());
        resetBase({ exchangeRatesData: response({ exchangeRates: [{ currency: 'USD', rate: '0.14' }] }) });
        setupPage();
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('There is no exchange rates data for your default currency');

        jest.clearAllMocks();
        mockGetLatestExchangeRates.mockResolvedValue(response());
        resetBase({ exchangeRatesData: undefined });
        setupPage();
        await flushPromises();
        expect(mockShowToast).not.toHaveBeenCalled();
    });

    test('computed values cover directions, provider lookup, selection setter, and amount font thresholds', () => {
        const { bindings } = setupPage();
        expect(bindings.textDirection.value).toBe('ltr');
        mockDirection = 'rtl';
        expect(setupPage().bindings.textDirection.value).toBe('rtl');
        expect(bindings.displayBaseAmount.value).toBe('CNY:cents:12345');
        expect(mockFormatBaseAmount).toHaveBeenCalledWith(12_345, 'CNY');

        for (const [amountCents, cssClass] of [
            [100_000_000, 'ebk-small-amount'],
            [-100_000_000, 'ebk-small-amount'],
            [1_000_000, 'ebk-normal-amount'],
            [-1_000_000, 'ebk-normal-amount'],
            [999_999, 'ebk-large-amount']
        ] as const) {
            mockBaseAmount.value = amountCents;
            expect(bindings.baseAmountFontSizeClass.value).toBe(cssClass);
        }

        expect(bindings.selectedProvider.value).toBe('auto');
        bindings.selectedProvider.value = 'ecb';
        expect(mockSetSelectedProvider).toHaveBeenCalledWith('ecb');
        expect(bindings.exchangeRateProviderOptions.value).toHaveLength(5);
        expect(bindings.getProviderDisplayName('boc_cn')).toBe('tt:Bank of China (Domestic)');
        expect(bindings.getProviderDisplayName('private')).toBe('private');
        expect(bindings.getExchangeRateDomId(rate('USD', 0.14))).toBe('exchangeRate_USD');
    });

    test('reload covers busy, success, callback, loading UI, provider change, and all error forms', async () => {
        const { bindings } = setupPage();
        await flushPromises();
        jest.clearAllMocks();

        const busyDone = jest.fn();
        bindings.loading.value = true;
        bindings.reload(busyDone);
        expect(busyDone).toHaveBeenCalledTimes(1);
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();

        bindings.loading.value = false;
        const successDone = jest.fn();
        mockGetLatestExchangeRates.mockResolvedValueOnce(response());
        bindings.reload(successDone);
        await flushPromises();
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: false, force: true });
        expect(successDone).toHaveBeenCalledTimes(1);
        expect(mockShowLoading).not.toHaveBeenCalled();
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('Exchange rates data has been updated');
        expect(bindings.loading.value).toBe(false);

        jest.clearAllMocks();
        mockGetLatestExchangeRates.mockResolvedValueOnce(response());
        bindings.reload();
        await flushPromises();
        expect(mockShowLoading).toHaveBeenCalledTimes(1);

        jest.clearAllMocks();
        mockGetLatestExchangeRates.mockRejectedValueOnce({ processed: false, message: 'provider unavailable' });
        bindings.reload();
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('provider unavailable');

        jest.clearAllMocks();
        const rawError = { processed: false };
        mockGetLatestExchangeRates.mockRejectedValueOnce(rawError);
        bindings.reload(jest.fn());
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith(rawError);

        jest.clearAllMocks();
        mockGetLatestExchangeRates.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.reload();
        await flushPromises();
        expect(mockShowToast).not.toHaveBeenCalled();

        jest.clearAllMocks();
        mockGetLatestExchangeRates.mockResolvedValueOnce(response());
        bindings.changeProvider('rba');
        await flushPromises();
        expect(mockSetSelectedProvider).toHaveBeenCalledWith('rba');
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: false, force: true });
    });

    test('converted display keeps rates unscaled and converts cents only at the explicit presentation boundary', () => {
        const { bindings } = setupPage();
        mockBaseAmount.value = 12_345;
        mockGetConvertedAmount.mockReturnValueOnce(17.283);
        expect(bindings.getFinalConvertedAmount(rate('USD', 0.14), false)).toBe('rate:17.283');
        expect(mockGetConvertedAmount).toHaveBeenCalledWith(
            123.45,
            { currency: 'CNY', rate: '1' },
            { currencyCode: 'USD', currencyDisplayName: 'USD', rate: '0.14' }
        );
        expect(mockFormatRateAmount).toHaveBeenCalledWith(17.283);

        mockGetConvertedAmount.mockReturnValueOnce(17.283);
        expect(bindings.getFinalConvertedAmount(rate('USD', 0.14), true)).toBe('localized:rate:17.283');
        mockGetConvertedAmount.mockReturnValueOnce(0);
        expect(bindings.getFinalConvertedAmount(rate('USD', 0.14), true)).toBe('٠');
        mockGetConvertedAmount.mockReturnValueOnce(null);
        expect(bindings.getFinalConvertedAmount(rate('USD', 0.14), false)).toBe('0');

        mockBaseCurrency.value = 'MISSING';
        mockGetConvertedAmount.mockReturnValueOnce('');
        expect(bindings.getFinalConvertedAmount(rate('EUR', 0.13), false)).toBe('0');
        expect(mockGetConvertedAmount).toHaveBeenLastCalledWith(123.45, undefined, expect.objectContaining({ rate: '0.13' }));
        expect(mockStore.latestExchangeRateMap.USD.rate).toBe('0.14');
        expect(mockStore.latestExchangeRateMap.EUR.rate).toBe('0.13');
    });

    test('update navigation and swipeout close preserve page state', () => {
        const { bindings, router } = setupPage();
        bindings.update();
        expect(router.navigate).toHaveBeenCalledWith('/exchange_rates/update');

        bindings.settingBaseLine.value = true;
        mockNow = 2_000;
        bindings.onExchangeRateSwipeoutClosed();
        expect(bindings.baseCurrencyChangedTime.value).toBe(2_000);
        expect(bindings.settingBaseLine.value).toBe(false);
    });

    test('delete flow covers missing, confirm, real payload, swipeout callback, base reset, and failures', async () => {
        const { bindings } = setupPage();
        await flushPromises();
        jest.clearAllMocks();

        bindings.remove(null, false);
        expect(mockShowAlert).toHaveBeenCalledWith('An error occurred');

        const usd = rate('USD', 0.14, 'US Dollar');
        bindings.remove(usd, false);
        expect(bindings.customExchangeRateToDelete.value).toStrictEqual(usd);
        expect(bindings.showDeleteActionSheet.value).toBe(true);

        mockBaseCurrency.value = 'USD';
        mockDeleteUserCustomExchangeRate.mockResolvedValueOnce(true);
        bindings.remove(usd, true);
        expect(mockShowLoading).toHaveBeenCalledTimes(1);
        const payload = mockDeleteUserCustomExchangeRate.mock.calls[0]![0];
        expect(payload).toEqual({ currency: 'USD', beforeResolve: expect.any(Function) });
        const beforeResolveDone = jest.fn();
        payload.beforeResolve(beforeResolveDone);
        expect(mockOnSwipeoutDeleted).toHaveBeenCalledWith('exchangeRate_USD', beforeResolveDone);
        await flushPromises();
        expect(mockBaseCurrency.value).toBe('CNY');
        expect(mockHideLoading).toHaveBeenCalled();

        jest.clearAllMocks();
        mockBaseCurrency.value = 'CNY';
        mockDeleteUserCustomExchangeRate.mockResolvedValueOnce(true);
        bindings.remove(rate('EUR', 0.13), true);
        await flushPromises();
        expect(mockBaseCurrency.value).toBe('CNY');

        jest.clearAllMocks();
        mockDeleteUserCustomExchangeRate.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove(usd, true);
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith('delete failed');

        jest.clearAllMocks();
        const rawError = { processed: false };
        mockDeleteUserCustomExchangeRate.mockRejectedValueOnce(rawError);
        bindings.remove(usd, true);
        await flushPromises();
        expect(mockShowToast).toHaveBeenCalledWith(rawError);

        jest.clearAllMocks();
        mockDeleteUserCustomExchangeRate.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.remove(usd, true);
        await flushPromises();
        expect(mockShowToast).not.toHaveBeenCalled();
    });

    test('SSR covers loading-empty, provider fallback, reference, custom data, direction, and template handlers', async () => {
        resetBase({ exchangeRatesData: undefined, availableExchangeRates: [] });
        const empty = await renderPage();
        expect(empty.html).toContain('tt:No exchange rates data');

        resetBase({
            exchangeRatesData: response({ fallbackUsed: true }),
            availableExchangeRates: [rate('CNY', 1, 'Chinese Yuan'), rate('USD', 0.14, 'US Dollar')]
        });
        const regular = await renderPage(bindings => {
            bindings.loading.value = true;
        });
        expect(regular.html).toContain('tt:Preferred source');
        expect(regular.html).toContain('Bank of China');
        expect(regular.html).toContain('tt:Fallback');
        expect(regular.html).toContain('Selected source is unavailable');
        await action({ component: 'f7-link', event: 'onClick', label: 'Bank of China' }).handler();
        expect(mockOpenExternalUrl).toHaveBeenCalledWith('https://example.test/rates');

        mockDirection = 'rtl';
        resetBase({
            isUserCustomExchangeRates: true,
            exchangeRatesData: response({ dataSource: 'User', referenceUrl: '', fallbackUsed: false }),
            availableExchangeRates: [rate('CNY', 1, 'Chinese Yuan'), rate('USD', 0.14, 'US Dollar')]
        });
        const custom = await renderPage();
        expect(custom.html).toContain('tt:User Custom');
        expect(custom.html).toContain('tt:Update');
        expect(custom.html).toContain('tt:Delete');
        expect(custom.html).toContain('left="true"');

        await action({ component: 'f7-link', event: 'onClick', occurrence: 0 }).handler();
        expect(custom.bindings.showMoreActionSheet.value).toBe(true);
        await action({ component: 'f7-list-item', event: 'onClick', label: 'tt:Preferred source' }).handler();
        await action({ component: 'f7-list-item', event: 'onClick', label: 'tt:Base Currency' }).handler();
        await action({ component: 'f7-list-item', event: 'onClick', label: 'tt:Base Amount' }).handler();
        expect(custom.bindings.showProviderPopup.value).toBe(true);
        expect(custom.bindings.showBaseCurrencyPopup.value).toBe(true);
        expect(custom.bindings.showBaseAmountSheet.value).toBe(true);

        await action({ component: 'list-item-selection-popup', event: 'onUpdate:show', occurrence: 0 }).handler(false);
        await action({ component: 'list-item-selection-popup', event: 'onUpdate:show', occurrence: 1 }).handler(false);
        expect(custom.bindings.showProviderPopup.value).toBe(false);
        expect(custom.bindings.showBaseCurrencyPopup.value).toBe(false);

        const providerModel = action({ component: 'list-item-selection-popup', event: 'onUpdate:modelValue', occurrence: 0 });
        await providerModel.handler('ecb');
        expect(mockSetSelectedProvider).toHaveBeenCalledWith('ecb');
        await action({
            component: 'list-item-selection-popup',
            event: 'onUpdate:modelValue',
            label: 'tt:Base Currency'
        }).handler('USD');
        expect(mockBaseCurrency.value).toBe('USD');
        await action({ component: 'number-pad-sheet', event: 'onUpdate:show' }).handler(false);
        await action({ component: 'number-pad-sheet', event: 'onUpdate:modelValue' }).handler(54_321);
        expect(custom.bindings.showBaseAmountSheet.value).toBe(false);
        expect(mockBaseAmount.value).toBe(54_321);
        const providerChange = mockTemplateActions.find(candidate => (
            candidate.component === 'list-item-selection-popup'
            && candidate.event === 'onUpdate:modelValue'
            && String(candidate.handler).includes('changeProvider')
        ));
        expect(providerChange).toBeDefined();
        mockGetLatestExchangeRates.mockResolvedValueOnce(response());
        await providerChange!.handler('ecb');
        await flushPromises();

        const setBase = action({ component: 'f7-swipeout-button', event: 'onClick', label: 'tt:Set as Base' });
        mockGetConvertedAmount.mockReturnValueOnce(14);
        await setBase.handler();
        expect(mockSetAsBaseline).toHaveBeenCalledWith('USD', 'rate:14');
        expect(custom.bindings.settingBaseLine.value).toBe(true);

        await action({ component: 'f7-list-item', event: 'onSwipeout:closed' }).handler();
        expect(custom.bindings.settingBaseLine.value).toBe(false);
        await action({ component: 'f7-swipeout-button', event: 'onClick', handlerText: 'remove' }).handler();
        expect(custom.bindings.showDeleteActionSheet.value).toBe(true);
        mockDeleteUserCustomExchangeRate.mockResolvedValueOnce(true);
        await action({ component: 'f7-actions-button', event: 'onClick', label: 'tt:Delete' }).handler();
        await flushPromises();
        expect(mockDeleteUserCustomExchangeRate).toHaveBeenCalledWith(expect.objectContaining({ currency: 'USD' }));

        await action({ component: 'f7-actions-button', event: 'onClick', label: 'tt:Update' }).handler();
        expect(custom.router.navigate).toHaveBeenCalledWith('/exchange_rates/update');
        mockGetLatestExchangeRates.mockResolvedValueOnce(response());
        await action({ component: 'f7-actions-button', event: 'onClick', label: 'tt:Refresh' }).handler();
        await flushPromises();

        await action({ component: 'f7-actions', event: 'onActions:closed', occurrence: 0 }).handler();
        await action({ component: 'f7-actions', event: 'onActions:closed', occurrence: 1 }).handler();
        expect(custom.bindings.showMoreActionSheet.value).toBe(false);
        expect(custom.bindings.showDeleteActionSheet.value).toBe(false);
    });
});
