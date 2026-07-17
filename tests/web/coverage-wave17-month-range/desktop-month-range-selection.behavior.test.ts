import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import {
    collectHostCallbacks,
    mountWithHostRenderer
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as typeof import('@/../node_modules/vue');
const mockThemeName = mockActualVue.ref('light');
const mockGetFinalMonthRange = jest.fn();
const mockConvertYearMonth = jest.fn((value: string) => (
    value === 'invalid' ? null : { year: Number(value.slice(0, 4)), month0Based: Number(value.slice(5, 7)) - 1 }
));
let mockLastBase: ReturnType<typeof createBase>;

function createBase() {
    return {
        dateRange: mockActualVue.ref<Array<Record<string, number> | null>>([null, null]),
        beginDateTime: mockActualVue.ref(''),
        endDateTime: mockActualVue.ref(''),
        getFinalMonthRange: mockGetFinalMonthRange
    };
}

jest.mock('vuetify', () => ({ useTheme: () => ({ global: { name: mockThemeName } }) }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/components/base/MonthRangeSelectionBase.ts', () => ({
    useMonthRangeSelectionBase: () => {
        mockLastBase = createBase();
        return mockLastBase;
    }
}));
jest.mock('@/core/theme.ts', () => ({
    isDarkApplicationTheme: (name: string) => name === 'dark'
}));
jest.mock('@/lib/datetime.ts', () => ({
    getYear0BasedMonthObjectFromString: (value: string) => mockConvertYearMonth(value)
}));

import MonthRangeSelectionDialogModule from '@/components/desktop/MonthRangeSelectionDialog.vue';

const MonthRangeSelectionDialog = MonthRangeSelectionDialogModule as unknown as {
    setup: (props: object, context: object) => Record<string, unknown>;
};

function setup(overrides: Record<string, unknown> = {}) {
    const props = mockActualVue.reactive({
        show: false,
        title: 'Select months',
        hint: '',
        minTime: '',
        maxTime: '',
        persistent: false,
        ...overrides
    });
    const emit = jest.fn();
    const bindings = MonthRangeSelectionDialog.setup(props, {
        attrs: {}, slots: {}, emit, expose: jest.fn()
    }) as any;
    return { props, emit, bindings };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockThemeName.value = 'light';
    mockGetFinalMonthRange.mockReturnValue({
        minYearMonth: { year: 2025, month0Based: 0 },
        maxYearMonth: { year: 2025, month0Based: 11 }
    });
});

describe('desktop MonthRangeSelectionDialog behavior', () => {
    test('projects theme and show state and emits close updates', () => {
        const light = setup({ show: true });
        expect(light.bindings.isDarkMode.value).toBe(false);
        expect(light.bindings.showState.value).toBe(true);
        light.bindings.showState.value = false;
        expect(light.emit).toHaveBeenCalledWith('update:show', false);

        mockThemeName.value = 'dark';
        const dark = setup();
        expect(dark.bindings.isDarkMode.value).toBe(true);
        dark.bindings.cancel();
        expect(dark.emit).toHaveBeenCalledWith('update:show', false);
    });

    test('confirms a valid range and ignores an absent final range', () => {
        const success = setup();
        success.bindings.confirm();
        expect(success.emit).toHaveBeenCalledWith(
            'dateRange:change',
            { year: 2025, month0Based: 0 },
            { year: 2025, month0Based: 11 }
        );

        mockGetFinalMonthRange.mockReturnValueOnce(null);
        const absent = setup();
        absent.bindings.confirm();
        expect(absent.emit).not.toHaveBeenCalledWith(
            'dateRange:change', expect.anything(), expect.anything()
        );
    });

    test('emits Error messages and safely ignores non-Error throws', () => {
        mockGetFinalMonthRange.mockImplementationOnce(() => {
            throw new Error('month range invalid');
        });
        const error = setup();
        error.bindings.confirm();
        expect(error.emit).toHaveBeenCalledWith('error', 'month range invalid');

        mockGetFinalMonthRange.mockImplementationOnce(() => {
            throw 'raw failure';
        });
        const raw = setup();
        raw.bindings.confirm();
        expect(raw.emit).not.toHaveBeenCalledWith('error', expect.anything());
    });

    test('updates each watched endpoint only for present and parseable month strings', async () => {
        const mounted = setup();
        mounted.props.minTime = '2025-03';
        mounted.props.maxTime = '2026-11';
        await mockActualVue.nextTick();
        expect(mounted.bindings.dateRange.value).toEqual([
            { year: 2025, month0Based: 2 },
            { year: 2026, month0Based: 10 }
        ]);

        mounted.props.minTime = 'invalid';
        mounted.props.maxTime = 'invalid';
        await mockActualVue.nextTick();
        expect(mounted.bindings.dateRange.value).toEqual([
            { year: 2025, month0Based: 2 },
            { year: 2026, month0Based: 10 }
        ]);

        mounted.props.minTime = '';
        mounted.props.maxTime = '';
        await mockActualVue.nextTick();
        expect(mockConvertYearMonth).toHaveBeenCalledTimes(4);
    });

    test('renders hint, date text, persistent and empty states and executes template events', async () => {
        const filled = mountWithHostRenderer(MonthRangeSelectionDialogModule as any, {
            show: true,
            title: 'Select months',
            hint: 'Choose a range',
            minTime: '2025-01',
            maxTime: '2025-12',
            persistent: true
        }, ['VDialog', 'VCard', 'VCardText', 'VRow', 'VCol', 'MonthPicker', 'VBtn']);
        mockLastBase.beginDateTime.value = 'January 2025';
        mockLastBase.endDateTime.value = 'December 2025';
        mockLastBase.dateRange.value = [
            { year: 2025, month0Based: 0 },
            { year: 2025, month0Based: 11 }
        ];
        await mockActualVue.nextTick();
        const callbacks = collectHostCallbacks(filled.root);
        expect(callbacks.length).toBeGreaterThan(0);
        for (const { name, callback } of callbacks) {
            if (name === 'onUpdate:modelValue') callback({ year: 2027, month0Based: 5 });
            else callback();
            await mockActualVue.nextTick();
        }
        filled.app.unmount();

        const empty = mountWithHostRenderer(MonthRangeSelectionDialogModule as any, {
            show: false,
            title: 'Select months',
            hint: '',
            minTime: '',
            maxTime: '',
            persistent: false
        }, ['VDialog', 'VCard', 'VCardText', 'VRow', 'VCol', 'MonthPicker', 'VBtn']);
        empty.app.unmount();
    });
});
