/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

class MemoryStorage {
    readonly values = new Map<string, string>();

    getItem(key: string): string | null {
        return this.values.get(key) ?? null;
    }

    setItem(key: string, value: string): void {
        this.values.set(key, value);
    }

    removeItem(key: string): void {
        this.values.delete(key);
    }

    clear(): void {
        this.values.clear();
    }
}

const mockLocalStorage = new MemoryStorage();
const mockSessionStorage = new MemoryStorage();
const mockDateRangeCalls = {
    months: jest.fn((start: unknown, end: unknown) => [{ kind: 'months', start, end }]),
    quarters: jest.fn((start: unknown, end: unknown) => [{ kind: 'quarters', start, end }]),
    years: jest.fn((start: unknown, end: unknown) => [{ kind: 'years', start, end }]),
    fiscal: jest.fn((start: unknown, end: unknown, fiscal: number) => [{ kind: 'fiscal', start, end, fiscal }])
};

Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: mockLocalStorage });
Object.defineProperty(globalThis, 'sessionStorage', { configurable: true, value: mockSessionStorage });
Object.defineProperty(globalThis, '__bill_analyser_VERSION__', { configurable: true, value: '1.0.0' });
Object.defineProperty(globalThis, '__bill_analyser_BUILD_COMMIT_HASH__', { configurable: true, value: 'abcdef123' });
Object.defineProperty(globalThis, '__bill_analyser_BUILD_UNIX_TIME__', { configurable: true, value: '1' });
Object.defineProperty(globalThis, '__bill_analyser_IS_PRODUCTION__', { configurable: true, value: false });

jest.mock('@/core/base.ts', () => ({ keys: (value: object) => Object.keys(value) }));
jest.mock('@/core/statistics.ts', () => ({
    ChartDataType: {
        Default: { type: 1 }
    },
    ChartSortingType: {
        DisplayOrder: { type: 1 },
        Name: { type: 2 },
        Amount: { type: 3 },
        Default: { type: 3 }
    },
    CategoricalChartType: {
        Default: { type: 0 }
    },
    TrendChartType: {
        Default: { type: 1 }
    },
    ChartDateAggregationType: {
        Year: { type: 1 },
        FiscalYear: { type: 2 },
        Quarter: { type: 3 },
        Month: { type: 4 }
    },
    DEFAULT_CATEGORICAL_CHART_DATA_RANGE: { type: 0 },
    DEFAULT_TREND_CHART_DATA_RANGE: { type: 1 },
    DEFAULT_ASSET_TRENDS_CHART_DATA_RANGE: { type: 1 }
}));
jest.mock('@/lib/datetime.ts', () => ({
    getAllMonthsStartAndEndUnixTimes: (...args: unknown[]) => mockDateRangeCalls.months(...args as [unknown, unknown]),
    getAllQuartersStartAndEndUnixTimes: (...args: unknown[]) => mockDateRangeCalls.quarters(...args as [unknown, unknown]),
    getAllYearsStartAndEndUnixTimes: (...args: unknown[]) => mockDateRangeCalls.years(...args as [unknown, unknown]),
    getAllFiscalYearsStartAndEndUnixTimes: (...args: unknown[]) => mockDateRangeCalls.fiscal(...args as [unknown, unknown, number])
}));
jest.mock('@/lib/web.ts', () => ({ getBasePath: () => '/base' }));

const settings = require('@/lib/settings.ts') as typeof import('@/lib/settings.ts');
const statistics = require('@/lib/statistics.ts') as typeof import('@/lib/statistics.ts');
const version = require('@/lib/version.ts') as typeof import('@/lib/version.ts');

beforeEach(() => {
    jest.clearAllMocks();
    mockLocalStorage.clear();
    mockSessionStorage.clear();
});

describe('application settings persistence', () => {
    test('merges defaults, nested values, invalid JSON, and exposes convenience getters', () => {
        expect(settings.getApplicationSettings()).toEqual(expect.objectContaining({
            theme: 'auto', debug: false, swipeBack: true, animate: true, applicationLock: false
        }));
        mockLocalStorage.setItem('ebk_app_settings', '{bad json');
        expect(settings.getApplicationSettings().theme).toBe('auto');

        mockLocalStorage.setItem('ebk_app_settings', JSON.stringify({
            theme: 'dark',
            debug: true,
            swipeBack: false,
            animate: false,
            applicationLock: true,
            statistics: { defaultSortingType: 99 }
        }));
        const merged = settings.getApplicationSettings();
        expect(merged.theme).toBe('dark');
        expect(merged.statistics.defaultSortingType).toBe(99);
        expect(merged.statistics.defaultTimezoneType).toBeDefined();
        expect(settings.isEnableDebug()).toBe(true);
        expect(settings.getTheme()).toBe('dark');
        expect(settings.isEnableApplicationLock()).toBe(true);
        expect(settings.isEnableSwipeBack()).toBe(false);
        expect(settings.isEnableAnimate()).toBe(false);
        expect(settings.getLocaleDefaultSettings()).toEqual(expect.objectContaining({ currency: expect.any(String) }));
    });

    test('updates valid top-level and nested keys while rejecting unknown keys', () => {
        settings.updateApplicationSettingsValue('missing', true);
        expect(mockLocalStorage.getItem('ebk_app_settings')).toBeNull();
        settings.updateApplicationSettingsValue('theme', 'light');
        expect(JSON.parse(mockLocalStorage.getItem('ebk_app_settings') as string).theme).toBe('light');

        settings.updateApplicationSettingsSubValue('missing', 'anything', true);
        settings.updateApplicationSettingsSubValue('statistics', 'missing', true);
        const before = mockLocalStorage.getItem('ebk_app_settings');
        settings.updateApplicationSettingsSubValue('statistics', 'defaultSortingType', 7);
        const updated = JSON.parse(mockLocalStorage.getItem('ebk_app_settings') as string);
        expect(updated.statistics.defaultSortingType).toBe(7);
        expect(mockLocalStorage.getItem('ebk_app_settings')).not.toBe(before);

        mockLocalStorage.setItem('ebk_app_settings', JSON.stringify({ statistics: null }));
        settings.updateApplicationSettingsSubValue('statistics', 'defaultSortingType', 8);
        expect(JSON.parse(mockLocalStorage.getItem('ebk_app_settings') as string).statistics.defaultSortingType).toBe(8);
        settings.clearSettings();
        expect(mockLocalStorage.getItem('ebk_app_settings')).toBeNull();
    });

    test('stores, reads, and clears the session language key', () => {
        expect(settings.getSessionCurrentLanguageKey()).toBe('');
        settings.setSessionCurrentLanguageKey('zh-Hans');
        expect(settings.getSessionCurrentLanguageKey()).toBe('zh-Hans');
        settings.setSessionCurrentLanguageKey('');
        expect(settings.getSessionCurrentLanguageKey()).toBe('');
    });
});

function item(name: string, totalAmountCents: number, displayOrders: number[]): any {
    return { name, totalAmountCents, displayOrders };
}

describe('statistics sorting and date ranges', () => {
    test('sorts display order, name, amount, and all tie-break paths', () => {
        const displayOrder = [item('B', 1, [1, 2]), item('A', 2, [1, 1]), item('C', 3, [2])];
        statistics.sortStatisticsItems(displayOrder, 1);
        expect(displayOrder.map(value => value.name)).toEqual(['A', 'B', 'C']);

        const displayTie = [item('B2', 1, [1]), item('B10', 1, [1])];
        statistics.sortStatisticsItems(displayTie, 1);
        expect(displayTie.map(value => value.name)).toEqual(['B2', 'B10']);

        const byName = [item('Z', 1, []), item('A', 2, [])];
        statistics.sortStatisticsItems(byName, 2);
        expect(byName.map(value => value.name)).toEqual(['A', 'Z']);

        const byAmount = [item('B', 1, []), item('A', 2, []), item('A2', 1, [])];
        statistics.sortStatisticsItems(byAmount, 3);
        expect(byAmount.map(value => value.name)).toEqual(['A', 'A2', 'B']);
    });

    test('derives missing range endpoints from nested items and dispatches every aggregation', () => {
        const items = [
            { items: [{ year: 2026, month1base: 7 }, { year: 2025, month1base: 12 }] },
            { items: [{ year: 2027, month1base: 1 }, { year: 2026, month1base: 1 }] }
        ] as any;
        expect(statistics.getAllDateRangesFromItems(items, '', '', 4, 4)).toEqual([
            expect.objectContaining({ kind: 'months', start: '2025-12', end: '2027-1' })
        ]);
        expect(statistics.getAllDateRangesFromItems([], '', '', 4, 4)).toEqual([]);
        expect(statistics.getAllDateRangesFromItems(items, '2026-1' as any, '2026-2' as any, 4, 4))
            .toEqual([expect.objectContaining({ kind: 'months' })]);

        expect(statistics.getAllDateRangesByYearMonthRange('', '', 4, 1)).toEqual([]);
        expect(statistics.getAllDateRangesByYearMonthRange('2026-1' as any, '2026-2' as any, 4, 1)[0])
            .toEqual(expect.objectContaining({ kind: 'years' }));
        expect(statistics.getAllDateRangesByYearMonthRange('2026-1' as any, '2026-2' as any, 4, 2)[0])
            .toEqual(expect.objectContaining({ kind: 'fiscal', fiscal: 4 }));
        expect(statistics.getAllDateRangesByYearMonthRange('2026-1' as any, '2026-2' as any, 4, 3)[0])
            .toEqual(expect.objectContaining({ kind: 'quarters' }));
        expect(statistics.getAllDateRangesByYearMonthRange('2026-1' as any, '2026-2' as any, 4, 99)[0])
            .toEqual(expect.objectContaining({ kind: 'months' }));
    });
});

describe('version display and runtime paths', () => {
    test('formats missing, development, release, short, and long hashes', () => {
        expect(version.formatDisplayVersion({ version: '', commitHash: '', buildTime: '' })).toBe('unknown');
        expect(version.formatDisplayVersion({ version: '1.2.3', commitHash: 'abc', buildTime: '1' }))
            .toBe('v1.2.3-dev (abc)');
        expect(version.formatDisplayVersion({ version: '1.2.3', commitHash: '123456789', buildTime: '' }))
            .toBe('v1.2.3-dev (1234567)');
        expect(version.getClientVersionInfo()).toEqual(expect.objectContaining({
            version: expect.any(String), commitHash: expect.any(String)
        }));
        expect(version.getClientDisplayVersion()).toEqual(expect.any(String));
        expect(version.getClientBuildTime()).toEqual(expect.any(String));
        expect(version.getMobileVersionPath()).toMatch(/^\/base\/mobile(?:\.html)?#\/$/u);
        expect(version.getDesktopVersionPath()).toMatch(/^\/base\/desktop(?:\.html)?#\/$/u);
        expect(typeof version.isProduction()).toBe('boolean');
    });
});
