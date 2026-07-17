/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockMapApi = {
    initMapView: jest.fn(),
    setMarkerPosition: jest.fn()
};
const mockSupportMapClick = jest.fn<() => boolean>();
const mockScrollToSelectedItem = jest.fn();
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

jest.mock('vue', () => ({
    ...actualVue,
    useTemplateRef: (name: string) => actualVue.ref(name === 'map' ? mockMapApi : null)
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentNumeralSystemType: () => ({
            replaceWesternArabicDigitsToLocalizedDigits: (value: string) => `n:${value}`
        })
    })
}));

jest.mock('@/components/common/KeywordInput.vue', () => ({ __esModule: true, default: {} }));
jest.mock('@/components/common/MapView.vue', () => ({ __esModule: true, default: {} }));
jest.mock('@mdi/js', () => ({ mdiPlus: 'plus' }));

jest.mock('@/core/base.ts', () => ({
    keys: (value: Record<string, unknown>) => Object.keys(value),
    itemAndIndex: (items: unknown[]) => items.map((item, index) => [item, index])
}));
jest.mock('@/core/numeral.ts', () => ({
    NumeralSystem: { WesternArabicNumerals: { digitZero: '0' } }
}));
jest.mock('@/lib/map/index.ts', () => ({
    isSupportGetGeoLocationByClick: () => mockSupportMapClick()
}));
jest.mock('@/components/base/ScheduleFrequencySelectionBase.ts', () => ({
    useScheduleFrequencySelectionBase: () => ({
        allTransactionScheduledFrequencyTypes: [
            { type: 0, displayName: 'Disabled' },
            { type: 1, displayName: 'Weekly' },
            { type: 2, displayName: 'Monthly' }
        ],
        allWeekDays: [
            { type: 1, displayName: 'Monday' },
            { type: 2, displayName: 'Tuesday' }
        ],
        allAvailableMonthDays: [
            { day: 1, displayName: '1' },
            { day: 15, displayName: '15' }
        ],
        getFrequencyValues: (value: string) => value
            ? value.split(',').map(item => Number.parseInt(item, 10))
            : []
    })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({ currentUserFirstDayOfWeek: 2 })
}));
jest.mock('@/core/template.ts', () => ({
    ScheduledTemplateFrequencyType: {
        Disabled: { type: 0 },
        Weekly: { type: 1 },
        Monthly: { type: 2 }
    }
}));
jest.mock('@/lib/common.ts', () => ({
    sortNumbersArray: (values: number[]) => [...values].sort((left, right) => left - right)
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    scrollToSelectedItem: (...args: unknown[]) => mockScrollToSelectedItem(...args)
}));

const CategoryRuleBuilderFields = require('@/components/common/CategoryRuleBuilderFields.vue').default as any;
const PaginationButtons = require('@/components/desktop/PaginationButtons.vue').default as any;
const StepsBar = require('@/components/desktop/StepsBar.vue').default as any;
const PinCodeInputSheet = require('@/components/mobile/PinCodeInputSheet.vue').default as any;
const MapSheet = require('@/components/mobile/MapSheet.vue').default as any;
const ScheduleFrequencySheet = require('@/components/mobile/ScheduleFrequencySheet.vue').default as any;

interface Runtime {
    props: Record<string, any>;
    bindings: any;
    emit: jest.Mock;
}

function setup(component: any, input: Record<string, unknown>): Runtime {
    const props = actualVue.reactive(input) as Record<string, any>;
    const emit = jest.fn();
    const bindings = component.setup(props, {
        attrs: {},
        slots: {},
        emit,
        expose: jest.fn()
    });
    return { props, bindings, emit };
}

function render(component: any, runtime: Runtime): any {
    return component.render(
        {}, [], runtime.props, actualVue.proxyRefs(runtime.bindings), {}, {}
    );
}

function collectHandlers(node: any, handlers: Array<(event?: any) => unknown> = []): Array<(event?: any) => unknown> {
    if (!node) return handlers;
    if (Array.isArray(node)) {
        for (const child of node) collectHandlers(child, handlers);
        return handlers;
    }
    if (typeof node !== 'object') return handlers;

    for (const [name, candidate] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        for (const handler of Array.isArray(candidate) ? candidate : [candidate]) {
            if (typeof handler === 'function') handlers.push(handler as (event?: any) => unknown);
        }
    }
    if (Array.isArray(node.children)) {
        collectHandlers(node.children, handlers);
    }
    return handlers;
}

beforeAll(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    consoleWarnSpy.mockRestore();
});

beforeEach(() => {
    jest.clearAllMocks();
    mockSupportMapClick.mockReturnValue(true);
});

describe('CategoryRuleBuilderFields production behavior', () => {
    test('updates text and boolean fields and delegates expression creation', () => {
        const runtime = setup(CategoryRuleBuilderFields, {
            modelValue: {
                priority: 1,
                ruleExpression: 'coffee',
                regexEnabled: false,
                enabled: true
            },
            autoRuleName: 'Coffee',
            disabled: false,
            showEnabledToggle: true,
            title: 'Category Matching',
            description: ''
        });

        expect(runtime.bindings.draft.value.ruleExpression).toBe('coffee');
        expect(runtime.bindings.canAddExpression.value).toBe(true);
        runtime.bindings.updateTextField('ruleExpression', 'tea');
        runtime.bindings.updateBooleanField('regexEnabled', 1);
        runtime.bindings.updateBooleanField('enabled', 0);
        expect(runtime.emit.mock.calls).toEqual([
            ['update:modelValue', expect.objectContaining({ ruleExpression: 'tea' })],
            ['update:modelValue', expect.objectContaining({ regexEnabled: true })],
            ['update:modelValue', expect.objectContaining({ enabled: false })]
        ]);

        const addExpression = jest.fn();
        runtime.bindings.expressionInput.value = {
            addExpression,
            canAddExpression: () => false
        };
        expect(runtime.bindings.canAddExpression.value).toBe(false);
        runtime.bindings.addExpression();
        expect(addExpression).toHaveBeenCalledTimes(1);
        expect(render(CategoryRuleBuilderFields, runtime)).toBeDefined();

        runtime.props['showEnabledToggle'] = false;
        runtime.props['disabled'] = true;
        expect(render(CategoryRuleBuilderFields, runtime)).toBeDefined();
    });
});

describe('PaginationButtons production behavior', () => {
    test('localizes pages, validates model changes, closes menus, and renders item variants', () => {
        const runtime = setup(PaginationButtons, {
            density: 'compact',
            disabled: false,
            totalPageCount: 3,
            totalVisible: undefined,
            modelValue: 1
        });

        expect(runtime.bindings.getDisplayPage(2)).toBe('n:2');
        expect(runtime.bindings.allPages.value).toEqual([
            { value: 1, name: 'n:1' },
            { value: 2, name: 'n:2' },
            { value: 3, name: 'n:3' }
        ]);
        runtime.bindings.showMenus.value = { left: true, right: true };
        runtime.bindings.currentPage.value = 0;
        runtime.bindings.currentPage.value = 4;
        expect(runtime.emit).not.toHaveBeenCalled();
        runtime.bindings.currentPage.value = 2;
        expect(runtime.emit).toHaveBeenCalledWith('update:modelValue', 2);
        expect(runtime.bindings.showMenus.value).toEqual({ left: false, right: false });

        const vnode = render(PaginationButtons, runtime);
        expect(vnode.children.item({ key: 'page', page: '2', isActive: true })).toBeDefined();
        expect(vnode.children.item({ key: 'dots', page: '...', isActive: false })).toBeDefined();
        runtime.props['totalVisible'] = 5;
        runtime.props['disabled'] = true;
        expect(render(PaginationButtons, runtime)).toBeDefined();
    });
});

describe('StepsBar production behavior', () => {
    const steps = [
        { name: 'one', title: 'One', subTitle: 'First' },
        { name: 'two', title: 'Two', subTitle: 'Second' },
        { name: 'three', title: 'Three', subTitle: 'Third' }
    ];

    test('derives localized state and respects every clickable form', () => {
        const runtime = setup(StepsBar, {
            steps,
            currentStep: 'two',
            clickable: true,
            minWidth: 320
        });
        expect(runtime.bindings.getDisplayStep(2)).toBe('n:02');
        expect(runtime.bindings.isStepActive(steps[1])).toBe(true);
        expect(runtime.bindings.isStepActive(steps[0])).toBe(false);
        expect(runtime.bindings.isStepCompleted(0)).toBe(true);
        expect(runtime.bindings.isStepCompleted(2)).toBe(false);
        runtime.bindings.changeStep(steps[2]);
        expect(runtime.emit).toHaveBeenCalledWith('step:change', 'three');
        expect(render(StepsBar, runtime)).toBeDefined();

        runtime.props['currentStep'] = 'missing';
        runtime.props['minWidth'] = 0;
        expect(runtime.bindings.isStepCompleted(0)).toBe(false);
        expect(render(StepsBar, runtime)).toBeDefined();

        for (const clickable of [false, 'false']) {
            const disabled = setup(StepsBar, { steps, currentStep: 'one', clickable, minWidth: '' });
            expect(disabled.bindings.isClickable.value).toBe(false);
            disabled.bindings.changeStep(steps[1]);
            expect(disabled.emit).not.toHaveBeenCalled();
        }
    });
});

describe('PinCodeInputSheet production behavior', () => {
    test('guards confirmation, emits valid pins, resets, cancels, and renders', async () => {
        const runtime = setup(PinCodeInputSheet, {
            modelValue: 'old',
            title: 'PIN',
            hint: 'Enter PIN',
            confirmDisabled: false,
            cancelDisabled: false,
            show: true
        });
        expect(runtime.bindings.currentPinCodeValid.value).toBe(false);
        runtime.bindings.confirm();
        expect(runtime.emit).not.toHaveBeenCalled();

        runtime.bindings.currentPinCode.value = '123456';
        expect(runtime.bindings.currentPinCodeValid.value).toBe(true);
        runtime.bindings.confirm();
        expect(runtime.emit.mock.calls).toEqual([
            ['update:modelValue', '123456'],
            ['pincode:confirm', '123456']
        ]);

        runtime.props['confirmDisabled'] = true;
        runtime.bindings.confirm();
        expect(runtime.emit).toHaveBeenCalledTimes(2);
        runtime.bindings.onSheetOpen();
        expect(runtime.bindings.currentPinCode.value).toBe('');
        runtime.bindings.cancel();
        runtime.bindings.onSheetClosed();
        expect(runtime.emit).toHaveBeenLastCalledWith('update:show', false);

        runtime.bindings.currentPinCode.value = '654321';
        runtime.props['modelValue'] = '';
        await actualVue.nextTick();
        expect(runtime.bindings.currentPinCode.value).toBe('');
        expect(render(PinCodeInputSheet, runtime)).toBeDefined();
    });
});

describe('MapSheet production behavior', () => {
    function props(overrides: Record<string, unknown> = {}): Record<string, unknown> {
        return {
            modelValue: { latitude: 10, longitude: 20 },
            readonly: false,
            setGeoLocationByClickMap: true,
            show: true,
            ...overrides
        };
    }

    test('updates supported editable coordinates and covers all close/open actions', () => {
        const runtime = setup(MapSheet, props());
        expect(runtime.bindings.geoLocation.value).toEqual({ latitude: 10, longitude: 20 });
        runtime.bindings.geoLocation.value = { latitude: 11, longitude: 21 };
        expect(runtime.emit).toHaveBeenCalledWith('update:modelValue', { latitude: 11, longitude: 21 });

        runtime.bindings.updateSpecifiedGeoLocation({ latitude: 12, longitude: 22 });
        expect(mockMapApi.setMarkerPosition).toHaveBeenCalledWith({ latitude: 12, longitude: 22 });
        runtime.bindings.switchSetGeoLocationByClickMap(false);
        expect(runtime.emit).toHaveBeenCalledWith('update:setGeoLocationByClickMap', false);
        runtime.bindings.save();
        runtime.bindings.close();
        runtime.bindings.onSheetOpen();
        runtime.bindings.onSheetClosed();
        expect(mockMapApi.initMapView).toHaveBeenCalledTimes(1);
        expect(runtime.emit).toHaveBeenLastCalledWith('update:show', false);

        const vnode = render(MapSheet, runtime);
        const handlers = collectHandlers(vnode);
        expect(handlers.length).toBeGreaterThan(2);
    });

    test('ignores readonly, unsupported, and disabled click-to-set paths and renders toolbar variants', () => {
        for (const [overrides, support] of [
            [{ readonly: true }, true],
            [{ setGeoLocationByClickMap: false }, true],
            [{}, false]
        ] as Array<[Record<string, unknown>, boolean]>) {
            mockSupportMapClick.mockReturnValue(support);
            const runtime = setup(MapSheet, props(overrides));
            runtime.bindings.updateSpecifiedGeoLocation({ latitude: 30, longitude: 40 });
            expect(runtime.emit).not.toHaveBeenCalledWith(
                'update:modelValue', { latitude: 30, longitude: 40 }
            );
            expect(render(MapSheet, runtime)).toBeDefined();
        }
    });
});

describe('ScheduleFrequencySheet production behavior', () => {
    function props(overrides: Record<string, unknown> = {}): Record<string, unknown> {
        return { type: 0, modelValue: '', show: true, ...overrides };
    }

    test('changes every frequency type, toggles values, saves sorted values, and restores on open', () => {
        const runtime = setup(ScheduleFrequencySheet, props({ type: 1, modelValue: '2,1' }));
        expect(runtime.bindings.firstDayOfWeek.value).toBe(2);
        expect(runtime.bindings.isChecked(1)).toBe(true);
        expect(runtime.bindings.isChecked(3)).toBe(false);

        runtime.bindings.changeFrequencyType(1);
        expect(runtime.bindings.currentFrequencyValue.value).toEqual([2, 1]);
        runtime.bindings.changeFrequencyType(2);
        expect(runtime.bindings.currentFrequencyValue.value).toEqual([1]);
        runtime.bindings.changeFrequencyType(1);
        expect(runtime.bindings.currentFrequencyValue.value).toEqual([2]);
        runtime.bindings.changeFrequencyType(0);
        expect(runtime.bindings.currentFrequencyValue.value).toEqual([]);

        runtime.bindings.changeFrequencyValue({ target: { value: '3', checked: true } } as any);
        runtime.bindings.changeFrequencyValue({ target: { value: '3', checked: true } } as any);
        expect(runtime.bindings.currentFrequencyValue.value).toEqual([3]);
        runtime.bindings.changeFrequencyValue({ target: { value: '3', checked: false } } as any);
        runtime.bindings.changeFrequencyValue({ target: { value: '9', checked: false } } as any);
        expect(runtime.bindings.currentFrequencyValue.value).toEqual([]);

        runtime.bindings.currentFrequencyValue.value = [15, 1];
        runtime.bindings.save();
        expect(runtime.emit.mock.calls).toEqual(expect.arrayContaining([
            ['update:type', 0],
            ['update:modelValue', '1,15'],
            ['update:show', false]
        ]));

        runtime.props['type'] = 2;
        runtime.props['modelValue'] = '15,1';
        runtime.bindings.onSheetOpen({ $el: { id: 'sheet' } });
        expect(runtime.bindings.currentFrequencyType.value).toBe(2);
        expect(runtime.bindings.currentFrequencyValue.value).toEqual([15, 1]);
        expect(mockScrollToSelectedItem).toHaveBeenCalledWith(
            { id: 'sheet' }, '.schedule-frequency-value-container', 'li.list-item-selected'
        );
        runtime.bindings.onSheetClosed();
        expect(runtime.emit).toHaveBeenLastCalledWith('update:show', false);
    });

    test('renders disabled, weekly, and monthly template branches', () => {
        const runtime = setup(ScheduleFrequencySheet, props());
        for (const type of [0, 1, 2]) {
            runtime.bindings.currentFrequencyType.value = type;
            expect(render(ScheduleFrequencySheet, runtime)).toBeDefined();
        }
    });
});
