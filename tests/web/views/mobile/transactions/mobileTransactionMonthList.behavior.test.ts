/* eslint-disable @typescript-eslint/no-explicit-any */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const { computed, ref } = jest.requireActual('vue') as typeof import('@/../node_modules/vue');

const mockGetElementActualHeights = jest.fn<(selector: string) => Record<string, number>>();
const mockGetElementBoundingRect = jest.fn<(selector: string) => DOMRect | null>();

jest.mock('@/lib/ui/mobile.ts', () => ({
    getElementActualHeights: (selector: string) => mockGetElementActualHeights(selector),
    getElementBoundingRect: (selector: string) => mockGetElementBoundingRect(selector)
}));

import { useMobileTransactionMonthList } from '@/views/mobile/transactions/useMobileTransactionMonthList.ts';

function transaction(id: string, date: string): any {
    return { id, gregorianCalendarYearDashMonthDashDay: date };
}

function month(yearDashMonth: string, opened = true): any {
    return {
        year: Number(yearDashMonth.slice(0, 4)),
        month: Number(yearDashMonth.slice(5, 7)),
        yearDashMonth,
        opened,
        items: [],
        totalAmountCents: {},
        dailyTotalAmountsCents: {}
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockGetElementActualHeights.mockReturnValue({});
    mockGetElementBoundingRect.mockReturnValue(null);
    Object.defineProperty(globalThis, 'window', {
        configurable: true,
        value: { innerHeight: 600 }
    });
});

describe('mobile transaction month-list identity and collapsed state', () => {
    test('builds stable DOM ids and resets both cached state maps', async () => {
        const months = ref([month('2026-07'), month('sentinel')]);
        const state = useMobileTransactionMonthList(computed(() => months.value));
        expect(state.getTransactionMonthTitleDomId('2026-07' as any)).toBe('transaction_month_title_2026-07');
        expect(state.getTransactionMonthListDomId('2026-07' as any)).toBe('transaction_month_list_2026-07');
        expect(state.getTransactionDomId(transaction('tx-7', '2026-07-16'))).toBe('transaction_tx-7');

        mockGetElementActualHeights.mockReturnValue({ 'transaction_month_list_2026-07': 144 });
        await state.setTransactionMonthListHeights(false);
        state.transactionInvisibleYearMonths.value['2026-07' as any] = true;
        expect(state.isTransactionMonthListInvisible(months.value[0])).toBe(true);
        state.resetTransactionMonthListState();
        expect(state.transactionInvisibleYearMonths.value).toEqual({});
        expect(state.isTransactionMonthListInvisible(months.value[0])).toBe(false);
        expect(state.getTransactionMonthListHeight(months.value[0])).toBe('auto');
    });

    test('distinguishes missing heights, closed months, viewport-hidden months, and visible months', async () => {
        const openMonth = month('2026-07', true);
        const closedMonth = month('2026-06', false);
        const months = ref([openMonth, closedMonth, month('sentinel')]);
        const state = useMobileTransactionMonthList(computed(() => months.value));
        expect(state.isTransactionMonthListInvisible(openMonth)).toBe(false);

        mockGetElementActualHeights.mockReturnValue({
            'transaction_month_list_2026-07': 120,
            'transaction_month_list_2026-06': 80
        });
        await state.setTransactionMonthListHeights(false);
        expect(state.isTransactionMonthListInvisible(closedMonth)).toBe(true);
        expect(state.getTransactionMonthListHeight(closedMonth)).toBe('80px');
        expect(state.isTransactionMonthListInvisible(openMonth)).toBe(false);
        state.transactionInvisibleYearMonths.value['2026-07' as any] = true;
        expect(state.isTransactionMonthListInvisible(openMonth)).toBe(true);
        expect(state.getTransactionMonthListHeight(openMonth)).toBe('120px');
        delete state.transactionInvisibleYearMonths.value['2026-07' as any];
        expect(state.getTransactionMonthListHeight(openMonth)).toBe('auto');
    });
});

describe('mobile transaction month-list measurement and viewport pruning', () => {
    test('records finite heights once, excludes the sentinel, and supports reset measurement', async () => {
        const july = month('2026-07');
        const june = month('2026-06');
        const sentinel = month('2026-05');
        const months = ref([july, june, sentinel]);
        const state = useMobileTransactionMonthList(computed(() => months.value));
        mockGetElementActualHeights.mockReturnValue({
            'transaction_month_list_2026-07': 120,
            'transaction_month_list_2026-06': Number.NaN,
            'transaction_month_list_2026-05': 999
        });
        await state.setTransactionMonthListHeights(false);
        expect(state.getTransactionMonthListHeight(july)).toBe('auto');
        july.opened = false;
        expect(state.getTransactionMonthListHeight(july)).toBe('120px');
        expect(state.isTransactionMonthListInvisible(june)).toBe(false);
        expect(state.isTransactionMonthListInvisible(sentinel)).toBe(false);

        mockGetElementActualHeights.mockReturnValue({
            'transaction_month_list_2026-07': 300,
            'transaction_month_list_2026-06': 90
        });
        await state.setTransactionMonthListHeights(false);
        expect(state.getTransactionMonthListHeight(july)).toBe('120px');
        june.opened = false;
        expect(state.getTransactionMonthListHeight(june)).toBe('90px');

        state.transactionInvisibleYearMonths.value['2026-07' as any] = true;
        await state.setTransactionMonthListHeights(true);
        expect(state.transactionInvisibleYearMonths.value).toEqual({});
        expect(state.getTransactionMonthListHeight(july)).toBe('300px');
        expect(mockGetElementActualHeights).toHaveBeenCalledWith('.transaction-month-list');
    });

    test('returns safely for empty or absent month arrays', async () => {
        const emptyState = useMobileTransactionMonthList(computed(() => []));
        await expect(emptyState.setTransactionMonthListHeights(false)).resolves.toBeUndefined();
        expect(() => emptyState.setTransactionInvisibleYearMonthList()).not.toThrow();

        const absentState = useMobileTransactionMonthList(computed(() => undefined as any));
        await expect(absentState.setTransactionMonthListHeights(false)).resolves.toBeUndefined();
        expect(() => absentState.setTransactionInvisibleYearMonthList()).not.toThrow();
    });

    test('marks far-below and far-above months invisible, skips missing rects, and restores visible months', async () => {
        const july = month('2026-07');
        const june = month('2026-06');
        const may = month('2026-05');
        const april = month('2026-04');
        const months = ref([july, june, may, april, month('sentinel')]);
        const state = useMobileTransactionMonthList(computed(() => months.value));
        mockGetElementActualHeights.mockReturnValue({
            'transaction_month_list_2026-07': 100,
            'transaction_month_list_2026-06': 100,
            'transaction_month_list_2026-05': 100
        });
        await state.setTransactionMonthListHeights(false);

        const rects: Record<string, DOMRect | null> = {
            '#transaction_month_title_2026-07': { top: 1_300, height: 20 } as DOMRect,
            '#transaction_month_title_2026-06': { top: -1_400, height: 20 } as DOMRect,
            '#transaction_month_title_2026-05': { top: 100, height: 20 } as DOMRect,
            '#transaction_month_title_2026-04': null
        };
        mockGetElementBoundingRect.mockImplementation(selector => rects[selector] ?? null);
        state.transactionInvisibleYearMonths.value['2026-05' as any] = true;
        state.setTransactionInvisibleYearMonthList();
        expect(state.transactionInvisibleYearMonths.value).toEqual({
            '2026-07': true,
            '2026-06': true
        });
        expect(mockGetElementBoundingRect).toHaveBeenCalledWith('#transaction_month_title_2026-04');
    });
});

describe('mobile transaction month-list date projection', () => {
    test('shows the first date, shows changed dates, and hides repeated full dates', () => {
        const state = useMobileTransactionMonthList(computed(() => []));
        const july16 = transaction('one', '2026-07-16');
        const sameDate = transaction('two', '2026-07-16');
        const june16 = transaction('three', '2026-06-16');
        expect(state.getTransactionDateStyle(july16, undefined)).toEqual({});
        expect(state.getTransactionDateStyle(june16, july16)).toEqual({});
        expect(state.getTransactionDateStyle(sameDate, july16)).toEqual({ color: 'transparent' });
    });
});
