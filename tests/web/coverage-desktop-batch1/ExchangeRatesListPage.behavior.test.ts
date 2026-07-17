import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockTemplateHandlers: Array<{ event: string; handler: (...args: any[]) => unknown }> = [];
const mockGetLatestExchangeRates = jest.fn<(...args: any[]) => Promise<any>>();
const mockDeleteUserCustomExchangeRate = jest.fn<(...args: any[]) => Promise<any>>();
const mockSetSelectedProvider = jest.fn<(provider: string) => void>();
const mockGetConvertedAmount = jest.fn<(...args: any[]) => number | '' | null>();
const mockSetAsBaseline = jest.fn();
const mockFormatRate = jest.fn((value: number) => `rate:${value}`);
const mockLocalizeDigits = jest.fn((value: string) => `localized:${value}`);
const mockLogger = { warn: jest.fn() };
let mockMdAndUp: any;
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

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});
jest.mock('vuetify', () => ({ useDisplay: () => ({ mdAndUp: mockMdAndUp }) }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentNumeralSystemType: () => ({
            digitZero: '٠',
            replaceWesternArabicDigitsToLocalizedDigits: (value: string) => mockLocalizeDigits(value)
        }),
        formatExchangeRateAmountToWesternArabicNumerals: (value: number) => mockFormatRate(value)
    })
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
jest.mock('@/core/numeral.ts', () => ({ NumeralSystem: { WesternArabicNumerals: { digitZero: '0' } } }));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

function importedStub(name: string): any {
    return actualVue.defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => {
            for (const [event, value] of Object.entries(attrs)) {
                if (!event.startsWith('on')) continue;
                for (const handler of Array.isArray(value) ? value : [value]) {
                    if (typeof handler === 'function') {
                        mockTemplateHandlers.push({ event, handler: handler as (...args: any[]) => unknown });
                    }
                }
            }
            return actualVue.h('div', attrs, Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            }));
        }
    });
}

jest.mock('@/components/desktop/ConfirmDialog.vue', () => ({ __esModule: true, default: importedStub('ConfirmDialogStub') }));
jest.mock('@/components/desktop/SnackBar.vue', () => ({ __esModule: true, default: importedStub('SnackBarStub') }));
jest.mock('@/views/desktop/exchangerates/list/dialogs/UpdateDialog.vue', () => ({ __esModule: true, default: importedStub('UpdateDialogStub') }));

const ListPage = require('@/views/desktop/exchangerates/ListPage.vue').default as any;

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
        updateTime: 1_700_000_000,
        ...overrides
    };
}

function resetBase(overrides: Record<string, unknown> = {}): void {
    mockBaseCurrency = actualVue.ref(overrides['baseCurrency'] ?? 'CNY');
    mockBaseAmount = actualVue.ref(overrides['baseAmount'] ?? 12_345);
    mockDefaultCurrency = actualVue.ref(overrides['defaultCurrency'] ?? 'CNY');
    mockExchangeRatesData = actualVue.ref(Object.prototype.hasOwnProperty.call(overrides, 'exchangeRatesData')
        ? overrides['exchangeRatesData']
        : response());
    mockIsUserCustomExchangeRates = actualVue.ref(overrides['isUserCustomExchangeRates'] ?? false);
    mockExchangeRatesDataUpdateTime = actualVue.ref(overrides['exchangeRatesDataUpdateTime'] ?? '2026-07-15 10:00');
    mockAvailableExchangeRates = actualVue.ref(overrides['availableExchangeRates'] ?? [
        rate('CNY', 1, 'Chinese Yuan'),
        rate('USD', 0.14, 'US Dollar'),
        rate('EUR', 0.13, 'Euro')
    ]);
}

function setupPage(): any {
    return ListPage.setup({}, { attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined });
}

function installRefs(bindings: any, overrides: Record<string, unknown> = {}): {
    snackbar: { showMessage: jest.Mock; showError: jest.Mock };
    confirm: { open: jest.Mock<(...args: any[]) => Promise<void>> };
    update: { open: jest.Mock<() => Promise<any>> };
} {
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const confirm = {
        open: jest.fn<(...args: any[]) => Promise<void>>().mockImplementation(() => (
            overrides['confirm'] as Promise<void> | undefined ?? Promise.resolve()
        ))
    };
    const update = {
        open: jest.fn<() => Promise<any>>().mockImplementation(() => (
            overrides['update'] as Promise<any> | undefined ?? Promise.resolve({ message: 'Rates saved' })
        ))
    };
    bindings.snackbar.value = snackbar;
    bindings.confirmDialog.value = confirm;
    bindings.updateDialog.value = update;
    return { snackbar, confirm, update };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

async function renderPage(configure?: (bindings: any) => void): Promise<string> {
    const { createSSRApp, defineComponent, h } = actualVue;
    const { renderToString } = jest.requireActual('vue/server-renderer') as any;
    const RuntimePage = {
        ...ListPage,
        setup(_props: unknown, context: any) {
            const bindings = ListPage.setup({}, context);
            configure?.(bindings);
            return bindings;
        }
    };
    const Stub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => {
            for (const [event, value] of Object.entries(attrs)) {
                if (!event.startsWith('on')) continue;
                for (const handler of Array.isArray(value) ? value : [value]) {
                    if (typeof handler === 'function') {
                        mockTemplateHandlers.push({ event, handler: handler as (...args: any[]) => unknown });
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
    const app = createSSRApp(RuntimePage);
    for (const name of [
        'v-row', 'v-col', 'v-card', 'v-layout', 'v-navigation-drawer', 'v-skeleton-loader',
        'v-select', 'v-divider', 'amount-input', 'v-tabs', 'v-tab', 'v-main', 'v-window',
        'v-window-item', 'v-btn', 'v-icon', 'v-tooltip', 'v-progress-circular', 'v-table', 'v-spacer'
    ]) app.component(name, Stub);
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateHandlers.length = 0;
    mockMdAndUp = actualVue.ref(true);
    mockStore.selectedExchangeRateProvider = 'auto';
    mockStore.latestExchangeRateMap = {
        CNY: { currency: 'CNY', rate: '1' },
        USD: { currency: 'USD', rate: '0.14' },
        EUR: { currency: 'EUR', rate: '0.13' }
    };
    mockSetSelectedProvider.mockImplementation(provider => { mockStore.selectedExchangeRateProvider = provider; });
    mockGetLatestExchangeRates.mockResolvedValue(response());
    mockDeleteUserCustomExchangeRate.mockResolvedValue(true);
    mockGetConvertedAmount.mockImplementation((amount: number, from: any, to: any) => (
        from && to ? amount * Number(to.rate) / Number(from.rate) : ''
    ));
    resetBase();
});

describe('desktop exchange-rates ListPage production-loaded behavior', () => {
    test('initial load and computed provider fields preserve store and numeral contracts', async () => {
        const bindings = setupPage();
        const { snackbar } = installRefs(bindings);
        expect(bindings.loading.value).toBe(true);
        await flush();
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: false, force: false });
        expect(bindings.loading.value).toBe(false);
        expect(snackbar.showMessage).not.toHaveBeenCalled();
        expect(bindings.numeralSystem.value.digitZero).toBe('٠');
        expect(bindings.selectedProvider.value).toBe('auto');
        bindings.selectedProvider.value = 'ecb';
        expect(mockSetSelectedProvider).toHaveBeenCalledWith('ecb');
        expect(bindings.exchangeRateProviderOptions.value).toEqual([
            { title: 'tt:Automatic (Recommended)', value: 'auto' },
            { title: 'tt:Bank of China (Domestic)', value: 'boc_cn' },
            { title: 'tt:China Merchants Bank (Domestic)', value: 'cmb_cn' },
            { title: 'tt:European Central Bank (International)', value: 'ecb' },
            { title: 'tt:Reserve Bank of Australia (International)', value: 'rba' }
        ]);
        expect(bindings.alwaysShowNav.value).toBe(true);
        expect(bindings.showNav.value).toBe(true);
    });

    test('reload covers forced success, missing default, absent data, and processed errors', async () => {
        const bindings = setupPage();
        const { snackbar } = installRefs(bindings);
        await flush();
        jest.clearAllMocks();

        bindings.reload(true);
        await flush();
        expect(snackbar.showMessage).toHaveBeenCalledWith('Exchange rates data has been updated');

        jest.clearAllMocks();
        mockExchangeRatesData.value = response({ exchangeRates: [{ currency: 'USD', rate: '0.14' }] });
        bindings.reload(false);
        await flush();
        expect(snackbar.showMessage).toHaveBeenCalledWith('There is no exchange rates data for your default currency');

        jest.clearAllMocks();
        mockExchangeRatesData.value = undefined;
        bindings.reload(false);
        await flush();
        expect(snackbar.showMessage).not.toHaveBeenCalled();

        for (const error of [{ processed: false }, { processed: true }]) {
            mockGetLatestExchangeRates.mockRejectedValueOnce(error);
            bindings.reload(false);
            await flush();
            expect(bindings.loading.value).toBe(false);
            if (error.processed) expect(snackbar.showError).not.toHaveBeenLastCalledWith(error);
            else expect(snackbar.showError).toHaveBeenCalledWith(error);
        }
    });

    test('provider changes persist once and force a refresh', async () => {
        const bindings = setupPage();
        installRefs(bindings);
        await flush();
        jest.clearAllMocks();
        bindings.changeProvider('rba');
        await flush();
        expect(mockSetSelectedProvider).toHaveBeenCalledWith('rba');
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ silent: false, force: true });
    });

    test('update dialog reports result messages and truthy errors only', async () => {
        let bindings = setupPage();
        let refs = installRefs(bindings);
        bindings.update();
        await flush();
        expect(refs.update.open).toHaveBeenCalledTimes(1);
        expect(refs.snackbar.showMessage).toHaveBeenCalledWith('Rates saved');

        bindings = setupPage();
        refs = installRefs(bindings, { update: Promise.resolve({}) });
        bindings.update();
        await flush();
        expect(refs.snackbar.showMessage).not.toHaveBeenCalled();

        const error = new Error('update failed');
        bindings = setupPage();
        refs = installRefs(bindings, { update: Promise.reject(error) });
        bindings.update();
        await flush();
        expect(refs.snackbar.showError).toHaveBeenCalledWith(error);

        bindings = setupPage();
        refs = installRefs(bindings, { update: Promise.reject(null) });
        bindings.update();
        await flush();
        expect(refs.snackbar.showError).not.toHaveBeenCalled();
    });

    test('delete handles base reset, other currency, and processed error disposition', async () => {
        const bindings = setupPage();
        const { snackbar, confirm } = installRefs(bindings);
        await flush();
        mockBaseCurrency.value = 'USD';
        bindings.remove('USD');
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to delete this user custom exchange rate?');
        await flush();
        expect(mockDeleteUserCustomExchangeRate).toHaveBeenCalledWith({ currency: 'USD' });
        expect(mockBaseCurrency.value).toBe('CNY');
        expect(bindings.customExchangeRateRemoving.value.USD).toBe(false);

        mockBaseCurrency.value = 'CNY';
        bindings.remove('EUR');
        await flush();
        expect(mockBaseCurrency.value).toBe('CNY');

        for (const error of [{ processed: false }, { processed: true }]) {
            mockDeleteUserCustomExchangeRate.mockRejectedValueOnce(error);
            bindings.remove('USD');
            await flush();
            expect(bindings.updating.value).toBe(false);
            if (error.processed) expect(snackbar.showError).not.toHaveBeenLastCalledWith(error);
            else expect(snackbar.showError).toHaveBeenCalledWith(error);
        }
    });

    test('converted amounts divide cents once and cover localized, zero, missing-base, and exception paths', () => {
        const bindings = setupPage();
        mockGetConvertedAmount.mockReturnValueOnce(17.283);
        expect(bindings.getFinalConvertedAmount(rate('USD', 0.14), false)).toBe('rate:17.283');
        expect(mockGetConvertedAmount).toHaveBeenCalledWith(123.45, mockStore.latestExchangeRateMap.CNY, rate('USD', 0.14));
        mockGetConvertedAmount.mockReturnValueOnce(17.283);
        expect(bindings.getFinalConvertedAmount(rate('USD', 0.14), true)).toBe('localized:rate:17.283');

        mockGetConvertedAmount.mockReturnValueOnce(0);
        expect(bindings.getFinalConvertedAmount(rate('USD', 0.14), true)).toBe('٠');
        mockGetConvertedAmount.mockReturnValueOnce(null);
        expect(bindings.getFinalConvertedAmount(rate('USD', 0.14), false)).toBe('0');

        mockBaseCurrency.value = '';
        expect(bindings.getFinalConvertedAmount(rate('EUR', 0.13), true)).toBe('٠');
        expect(bindings.getFinalConvertedAmount(rate('EUR', 0.13), false)).toBe('0');

        mockBaseCurrency.value = 'CNY';
        mockGetConvertedAmount.mockImplementationOnce(() => { throw new Error('bad rate'); });
        expect(bindings.getFinalConvertedAmount(rate('EUR', 0.13), false)).toBe('0');
        expect(mockLogger.warn).toHaveBeenCalledWith(
            'failed to convert amount by exchange rates, original base amount is 12345',
            expect.any(Error)
        );
    });

    test('responsive navigation follows breakpoint only when the drawer is closed', async () => {
        const bindings = setupPage();
        installRefs(bindings);
        await flush();
        mockMdAndUp.value = false;
        await actualVue.nextTick();
        expect(bindings.alwaysShowNav.value).toBe(false);
        expect(bindings.showNav.value).toBe(true);

        bindings.showNav.value = false;
        mockMdAndUp.value = true;
        await actualVue.nextTick();
        expect(bindings.alwaysShowNav.value).toBe(true);
        expect(bindings.showNav.value).toBe(true);
    });

    test('renders loading, empty, reference, source-only, fallback, and custom table states', async () => {
        resetBase({ exchangeRatesData: undefined, availableExchangeRates: [] });
        const loading = await renderPage(bindings => { bindings.loading.value = true; });
        expect(loading).toContain('tt:Exchange Rates Data');

        const empty = await renderPage(bindings => { bindings.loading.value = false; });
        expect(empty).toContain('tt:No exchange rates data');

        resetBase({ exchangeRatesData: response(), availableExchangeRates: [rate('CNY', 1, 'Chinese Yuan'), rate('USD', 0.14, 'US Dollar')] });
        const reference = await renderPage(bindings => { bindings.loading.value = false; });
        expect(reference).toContain('https://example.test/rates');
        expect(reference).toContain('US Dollar');

        resetBase({ exchangeRatesData: response({ referenceUrl: '', fallbackUsed: true }) });
        const fallback = await renderPage(bindings => { bindings.loading.value = false; });
        expect(fallback).toContain('Bank of China');
        expect(fallback).toContain('Selected source is unavailable');

        resetBase({ isUserCustomExchangeRates: true, exchangeRatesData: response({ referenceUrl: '' }) });
        const custom = await renderPage(bindings => { bindings.loading.value = false; });
        expect(custom).toContain('tt:User Custom');
        expect(custom).toContain('tt:Update');
        expect(custom).toContain('tt:Delete');

        expect(mockTemplateHandlers.length).toBeGreaterThan(5);
        for (const { event, handler } of mockTemplateHandlers) {
            if (event.startsWith('onUpdate:')) handler(event.includes('modelValue') ? 'USD' : false);
            else handler();
            await flush(2);
        }
    });
});
