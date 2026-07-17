/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const { proxyRefs, reactive } = actualVue;

const TransactionType = {
    ModifyBalance: 1,
    Income: 2,
    Expense: 3,
    Transfer: 4,
    Investment: 5
} as const;

const TextDirection = { LTR: 'ltr', RTL: 'rtl' } as const;
const TransactionListPageType = {
    List: { type: 0 },
    Calendar: { type: 1 }
} as const;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getWeekdayShortName: (value: any) => `weekday:${value?.type ?? value}`,
        getCalendarDisplayDayOfMonthFromUnixTime: (value: number) => `day:${value}`
    })
}));

jest.mock('@/core/transaction.ts', () => ({ TransactionType }));
jest.mock('@/core/text.ts', () => ({ TextDirection }));
jest.mock('@/views/base/transactions/TransactionListPageBase.ts', () => ({ TransactionListPageType }));

const MobileTransactionMonthBlock = require(
    '@/views/mobile/transactions/components/MobileTransactionMonthBlock.vue'
).default as any;

const componentNames = [
    'f7-block', 'f7-accordion-item', 'f7-block-title', 'f7-accordion-toggle', 'f7-list',
    'f7-list-item', 'f7-icon', 'f7-accordion-content', 'f7-chip', 'f7-swipeout-actions',
    'f7-swipeout-button', 'ItemIcon'
];

function transaction(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'tx-default',
        type: TransactionType.Expense,
        time: 1_721_088_000,
        utcOffset: 480,
        category: { name: 'Food', icon: 'food', color: '#ff0000' },
        sourceAccount: { id: 'cash', name: 'Cash' },
        destinationAccount: null,
        comment: 'Lunch',
        tagIds: ['work', 'missing'],
        editable: true,
        getDisplayDayOfWeekObject: () => ({ type: 2 }),
        ...overrides
    };
}

function transactions(): any[] {
    return [
        transaction(),
        transaction({
            id: 'tx-income',
            type: TransactionType.Income,
            utcOffset: 0,
            category: { name: 'Salary', icon: 'salary', color: '' },
            comment: '',
            tagIds: [],
            editable: false,
            getDisplayDayOfWeekObject: () => null
        }),
        transaction({
            id: 'tx-balance',
            type: TransactionType.ModifyBalance,
            category: null,
            sourceAccount: null,
            destinationAccount: null,
            comment: undefined,
            tagIds: undefined,
            editable: true
        }),
        transaction({
            id: 'tx-transfer',
            type: TransactionType.Transfer,
            category: null,
            sourceAccount: { id: 'cash', name: 'Cash' },
            destinationAccount: { id: 'bank', name: 'Bank' },
            comment: 'Transfer',
            tagIds: ['work']
        }),
        transaction({
            id: 'tx-investment',
            type: TransactionType.Investment,
            category: { name: 'Fund', icon: 'fund', color: '#00ff00' },
            sourceAccount: { id: 'broker', name: 'Broker' },
            destinationAccount: { id: 'broker', name: 'Broker' },
            comment: '',
            tagIds: []
        }),
        transaction({
            id: 'tx-transfer-without-destination',
            type: TransactionType.Transfer,
            category: undefined,
            sourceAccount: { id: 'cash', name: 'Cash' },
            destinationAccount: undefined,
            tagIds: undefined
        })
    ];
}

function props(overrides: Record<string, unknown> = {}): any {
    const monthItems = transactions();
    return {
        monthList: {
            year: 2026,
            month: 7,
            yearDashMonth: '2026-07',
            opened: true,
            items: monthItems,
            totalAmountCents: {
                incomeCents: 500_000,
                incompleteIncome: false,
                expenseCents: 123_456,
                incompleteExpense: true
            },
            dailyTotalAmountsCents: {}
        },
        pageType: TransactionListPageType.List.type,
        showTotalAmount: true,
        showTags: true,
        defaultCurrency: 'CNY',
        currentTimezoneOffsetMinutes: 0,
        textDirection: TextDirection.LTR,
        allTransactionTags: { work: { name: 'Work' }, missing: undefined },
        getDisplayLongYearMonth: jest.fn(() => 'July 2026'),
        getDisplayMonthTotalAmount: jest.fn((amount: number, currency: string, symbol: string, incomplete: boolean) => (
            `${symbol}${currency}:${amount}:${incomplete}`
        )),
        getTransactionMonthTitleDomId: jest.fn((value: string) => `title-${value}`),
        getTransactionMonthListDomId: jest.fn((value: string) => `list-${value}`),
        getTransactionMonthListHeight: jest.fn(() => '333px'),
        isTransactionMonthListInvisible: jest.fn(() => false),
        getTransactionDomId: jest.fn((value: any) => `transaction-${value.id}`),
        getTransactionDateStyle: jest.fn((_value: any, previous: any) => previous ? { color: 'transparent' } : {}),
        getTransactionTypeName: jest.fn((type: number, fallback: string) => `${fallback}:${type}`),
        getDisplayAmount: jest.fn((value: any) => `amount:${value.id}`),
        getDisplayTime: jest.fn((value: any) => `time:${value.id}`),
        getDisplayTimezone: jest.fn((value: any) => `timezone:${value.id}`),
        ...overrides
    };
}

function setup(componentProps = props()): { bindings: any; emit: jest.Mock; props: any } {
    const emit = jest.fn();
    const reactiveProps = reactive(componentProps);
    const bindings = MobileTransactionMonthBlock.setup(reactiveProps, {
        attrs: {}, slots: {}, emit, expose: jest.fn()
    });
    return { bindings, emit, props: reactiveProps };
}

function render(page: { bindings: any; props: any }): any {
    return MobileTransactionMonthBlock.render(
        { ...page.props, ...proxyRefs(page.bindings) },
        [],
        page.props,
        proxyRefs(page.bindings),
        {},
        {}
    );
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
});

describe('MobileTransactionMonthBlock production template', () => {
    test('renders the complete list matrix and emits every accordion/swipe action', () => {
        const collapseMonth = jest.fn();
        const collapseStateChanged = jest.fn();
        const duplicate = jest.fn();
        const edit = jest.fn();
        const remove = jest.fn();
        const componentProps = props({
            onCollapseMonth: collapseMonth,
            onCollapseStateChanged: collapseStateChanged,
            onDuplicate: duplicate,
            onEdit: edit,
            onRemove: remove
        });
        const mounted = mountWithHostRenderer(MobileTransactionMonthBlock, componentProps, componentNames);
        try {
            expect(mounted.root.children.length).toBeGreaterThan(0);
            const callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.length).toBeGreaterThan(10);
            for (const { callback } of callbacks) callback({});

            const month = componentProps.monthList;
            expect(collapseMonth).toHaveBeenCalledWith(month, false);
            expect(collapseMonth).toHaveBeenCalledWith(month, true);
            expect(collapseStateChanged).toHaveBeenCalled();
            expect(duplicate).toHaveBeenCalledWith(month.items[0]);
            expect(edit).toHaveBeenCalledWith(month.items[0]);
            expect(remove).toHaveBeenCalledWith(month.items[0], false);

            expect(componentProps.getDisplayLongYearMonth).toHaveBeenCalledWith(month);
            expect(componentProps.getDisplayMonthTotalAmount).toHaveBeenCalledTimes(2);
            expect(componentProps.getTransactionMonthTitleDomId).toHaveBeenCalledWith('2026-07');
            expect(componentProps.getTransactionMonthListDomId).toHaveBeenCalledWith('2026-07');
            expect(componentProps.getTransactionDomId).toHaveBeenCalledTimes(month.items.length);
            expect(componentProps.getTransactionDateStyle).toHaveBeenNthCalledWith(1, month.items[0], undefined);
            expect(componentProps.getTransactionDateStyle).toHaveBeenNthCalledWith(2, month.items[1], month.items[0]);
            expect(componentProps.getTransactionTypeName).toHaveBeenCalled();
            expect(componentProps.getDisplayAmount).toHaveBeenCalled();
            expect(componentProps.getDisplayTime).toHaveBeenCalledTimes(month.items.length);
            expect(componentProps.getDisplayTimezone).toHaveBeenCalled();
        } finally {
            mounted.app.unmount();
        }
    });

    test('renders collapsed calendar, invisible placeholder, RTL, and empty item states', () => {
        const hiddenProps = props({
            pageType: TransactionListPageType.Calendar.type,
            showTotalAmount: false,
            showTags: false,
            textDirection: TextDirection.RTL,
            monthList: {
                year: 2026,
                month: 6,
                yearDashMonth: '2026-06',
                opened: false,
                items: [],
                totalAmountCents: null,
                dailyTotalAmountsCents: {}
            },
            isTransactionMonthListInvisible: jest.fn(() => true)
        });
        const mounted = mountWithHostRenderer(MobileTransactionMonthBlock, hiddenProps, componentNames);
        try {
            expect(mounted.root.children.length).toBeGreaterThan(0);
            expect(hiddenProps.getTransactionMonthListHeight).toHaveBeenCalledWith(hiddenProps.monthList);
            expect(hiddenProps.getDisplayLongYearMonth).not.toHaveBeenCalled();
            for (const { callback } of collectHostCallbacks(mounted.root)) callback({});
        } finally {
            mounted.app.unmount();
        }
    });

    test('renders list title without totals and direct-render state transitions', () => {
        const page = setup(props({
            showTotalAmount: true,
            monthList: {
                year: 2026,
                month: 5,
                yearDashMonth: '2026-05',
                opened: false,
                items: [],
                totalAmountCents: null,
                dailyTotalAmountsCents: {}
            }
        }));
        expect(render(page)).toBeDefined();
        page.props['pageType'] = TransactionListPageType.Calendar.type;
        page.props['showTags'] = false;
        expect(render(page)).toBeDefined();
    });
});
