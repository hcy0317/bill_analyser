/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const { proxyRefs, reactive, ref } = actualVue;

const mockMobileScroll = jest.fn();
const mockDesktopScroll = jest.fn();
const mockSearchbarClear = jest.fn();
const mockDropdownParent = { id: 'frequency-menu-parent' };
const mockDropdownMenu = { parentElement: mockDropdownParent };

const frequencyTypes = {
    Disabled: { type: 0 },
    Weekly: { type: 1 },
    Monthly: { type: 2 },
};

jest.mock('vue', () => ({
    ...actualVue,
    useTemplateRef: (name: string) => {
        if (name === 'searchbar') return actualVue.ref({ clear: mockSearchbarClear });
        if (name === 'dropdownMenu') return actualVue.ref(mockDropdownMenu);
        return actualVue.ref(null);
    },
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, params?: Record<string, unknown>) => params ? `${key}:${JSON.stringify(params)}` : `tt:${key}`,
        ti: (value: string, enabled: boolean) => enabled ? `i18n:${value}` : String(value),
        getMultiMonthdayShortNames: (values: number[]) => `month-days:${values.join('|')}`,
        getMultiWeekdayLongNames: (values: number[], firstDay: number) => `week-days:${firstDay}:${values.join('|')}`,
    }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    scrollToSelectedItem: (...args: unknown[]) => mockMobileScroll(...args),
}));
jest.mock('@/lib/ui/desktop.ts', () => ({
    scrollToSelectedItem: (...args: unknown[]) => mockDesktopScroll(...args),
}));
jest.mock('@/components/base/ScheduleFrequencySelectionBase.ts', () => ({
    useScheduleFrequencySelectionBase: () => ({
        allTransactionScheduledFrequencyTypes: ref([
            { type: 0, displayName: 'Disabled' },
            { type: 1, displayName: 'Weekly' },
            { type: 2, displayName: 'Monthly' },
        ]),
        allWeekDays: ref([
            { type: 1, displayName: 'Monday' },
            { type: 2, displayName: 'Tuesday' },
        ]),
        allAvailableMonthDays: ref([
            { day: 1, displayName: '1st' },
            { day: 15, displayName: '15th' },
        ]),
        getFrequencyValues: (value: string) => value
            ? value.split(',').map(item => Number.parseInt(item, 10)).filter(Number.isFinite)
            : [],
    }),
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({ currentUserFirstDayOfWeek: 1 }),
}));
jest.mock('@/core/template.ts', () => ({ ScheduledTemplateFrequencyType: frequencyTypes }));
jest.mock('@/lib/common.ts', () => ({
    sortNumbersArray: (values: number[]) => [...values].sort((left, right) => left - right),
}));

const ListItemSelectionPopup = require('@/components/mobile/ListItemSelectionPopup.vue').default as any;
const ScheduleFrequencySelect = require('@/components/desktop/ScheduleFrequencySelect.vue').default as any;

function setup(component: any, props: Record<string, unknown>): { bindings: any; emit: jest.Mock } {
    const emit = jest.fn();
    const bindings = component.setup(reactive(props), {
        attrs: {}, slots: {}, emit, expose: jest.fn(),
    });
    return { bindings, emit };
}

function render(component: any, bindings: any): unknown {
    return component.render({}, [], {}, proxyRefs(bindings), {}, {});
}

function listProps(overrides: Record<string, unknown> = {}): any {
    return {
        modelValue: 'b',
        title: 'Pick one',
        valueType: 'item',
        keyField: 'id',
        valueField: 'id',
        titleField: 'title',
        titleI18n: false,
        afterField: 'after',
        afterI18n: false,
        iconField: 'icon',
        iconType: 'account',
        colorField: 'color',
        hiddenField: 'hidden',
        enableFilter: true,
        filterPlaceholder: 'Search',
        filterNoItemsText: 'Nothing found',
        items: [
            { id: 'a', title: 'Alpha', after: 'First', icon: '1', color: '#111', hidden: false },
            { id: 'b', title: 'Beta', after: 'Second', icon: '2', color: '#222', hidden: false },
            { id: 'c', title: 'Hidden', after: 'Secret', icon: '3', color: '#333', hidden: true },
        ],
        show: true,
        ...overrides,
    };
}

beforeEach(() => {
    jest.clearAllMocks();
});

describe('ListItemSelectionPopup production behavior', () => {
    test('filters item values by title and after text while excluding hidden items', () => {
        const { bindings } = setup(ListItemSelectionPopup, listProps());

        expect(bindings.filteredItems.value.map((item: any) => item.id)).toEqual(['a', 'b']);
        bindings.filterContent.value = 'beta';
        expect(bindings.filteredItems.value.map((item: any) => item.id)).toEqual(['b']);
        bindings.filterContent.value = 'first';
        expect(bindings.filteredItems.value.map((item: any) => item.id)).toEqual(['a']);
        bindings.filterContent.value = 'missing';
        expect(bindings.filteredItems.value).toEqual([]);

        expect(bindings.isSelected(listProps().items[1], 1)).toBe(true);
        expect(bindings.isSelected(listProps().items[0], 0)).toBe(false);
        expect(bindings.getItemValue(listProps().items[0], 0, 'id', 'item')).toBe('a');
        expect(bindings.getItemValue('raw', 0, undefined, 'item')).toBe('raw');
        expect(bindings.getItemAfterText(listProps().items[0])).toBe('First');
        expect(render(ListItemSelectionPopup, bindings)).toBeDefined();
    });

    test('supports index values with and without filtering', () => {
        const props = listProps({
            modelValue: 1,
            valueType: 'index',
            titleField: '',
            keyField: undefined,
            valueField: undefined,
            afterField: undefined,
            hiddenField: undefined,
            iconField: undefined,
            enableFilter: true,
            items: ['Alpha', 'Beta', null],
        });
        const { bindings } = setup(ListItemSelectionPopup, props);

        bindings.filterContent.value = 'bet';
        expect(bindings.filteredItems.value).toEqual(['Beta']);
        expect(bindings.isSelected('Beta', 1)).toBe(true);
        expect(bindings.getItemValue('Beta', 1, undefined, 'index')).toBe(1);
        expect(bindings.getItemAfterText('Beta')).toBe('');
        bindings.filterContent.value = '';
        expect(bindings.filteredItems.value).toEqual(['Alpha', 'Beta', null]);
        expect(render(ListItemSelectionPopup, bindings)).toBeDefined();
    });

    test('emits selected item/index and closes the popup', () => {
        const itemSetup = setup(ListItemSelectionPopup, listProps());
        const alpha = listProps().items[0];
        itemSetup.bindings.onItemClicked(alpha, 0);
        expect(itemSetup.emit.mock.calls).toEqual([
            ['update:modelValue', 'a'],
            ['update:show', false],
        ]);

        const raw = { id: 'raw', title: 'Raw' };
        const rawSetup = setup(ListItemSelectionPopup, listProps({
            modelValue: null,
            valueField: undefined,
            items: [raw],
        }));
        rawSetup.bindings.onItemClicked(raw, 0);
        expect(rawSetup.emit).toHaveBeenCalledWith('update:modelValue', raw);

        const indexSetup = setup(ListItemSelectionPopup, listProps({
            modelValue: 0,
            valueType: 'index',
            items: ['Alpha', 'Beta'],
        }));
        indexSetup.bindings.onItemClicked('Beta', 1);
        expect(indexSetup.emit).toHaveBeenCalledWith('update:modelValue', 1);
    });

    test('restores selection on open and clears filter/searchbar on close', () => {
        const props = listProps({ modelValue: 'a' });
        const { bindings, emit } = setup(ListItemSelectionPopup, props);
        bindings.filterContent.value = 'alpha';
        const popupElement = { id: 'popup' };

        bindings.onPopupOpen({ $el: popupElement });
        expect(bindings.currentValue.value).toBe('a');
        expect(mockMobileScroll).toHaveBeenCalledWith(
            popupElement,
            '.page-content',
            'li.list-item-selected',
        );

        bindings.onPopupClosed();
        expect(bindings.filterContent.value).toBe('');
        expect(mockSearchbarClear).toHaveBeenCalledTimes(1);
        expect(emit).toHaveBeenCalledWith('update:show', false);
    });

    test('uses i18n title/after projections and filter-disabled fast path', () => {
        const props = listProps({ titleI18n: true, afterI18n: true, enableFilter: false });
        const { bindings } = setup(ListItemSelectionPopup, props);
        bindings.filterContent.value = 'does-not-matter';
        expect(bindings.filteredItems.value.map((item: any) => item.id)).toEqual(['a', 'b']);
        expect(bindings.getItemAfterText(listProps().items[0])).toBe('i18n:First');
    });
});

describe('ScheduleFrequencySelect production behavior', () => {
    function frequencyProps(overrides: Record<string, unknown> = {}): Record<string, unknown> {
        return {
            type: frequencyTypes.Disabled.type,
            modelValue: '',
            label: 'Frequency',
            readonly: false,
            disabled: false,
            ...overrides,
        };
    }

    test('projects disabled, weekly, monthly, and invalid display text', () => {
        const props = reactive(frequencyProps());
        const { bindings } = setup(ScheduleFrequencySelect, props);
        expect(bindings.displayFrequency.value).toBe('tt:Disabled');

        props.type = frequencyTypes.Weekly.type;
        props.modelValue = '1,2';
        expect(bindings.displayFrequency.value).toContain('week-days:1:1|2');
        props.modelValue = '';
        expect(bindings.displayFrequency.value).toBe('tt:Weekly');

        props.type = frequencyTypes.Monthly.type;
        props.modelValue = '1,15';
        expect(bindings.displayFrequency.value).toContain('month-days:1|15');
        props.modelValue = '';
        expect(bindings.displayFrequency.value).toBe('tt:Monthly');

        props.type = 99;
        expect(bindings.displayFrequency.value).toBe('');
        expect(render(ScheduleFrequencySelect, bindings)).toBeDefined();
    });

    test.each([
        [frequencyTypes.Weekly.type, '1'],
        [frequencyTypes.Monthly.type, '1'],
        [frequencyTypes.Disabled.type, ''],
    ])('initializes values when frequency type changes to %s', (nextType, expectedValue) => {
        const initialType = nextType === frequencyTypes.Disabled.type ? frequencyTypes.Weekly.type : frequencyTypes.Disabled.type;
        const { bindings, emit } = setup(ScheduleFrequencySelect, frequencyProps({
            type: initialType,
            modelValue: '2',
        }));

        bindings.frequencyType.value = nextType;
        expect(emit).toHaveBeenCalledWith('update:type', nextType);
        expect(emit).toHaveBeenCalledWith('update:modelValue', expectedValue);
    });

    test('does not emit when setting the existing frequency type', () => {
        const { bindings, emit } = setup(ScheduleFrequencySelect, frequencyProps({ type: frequencyTypes.Weekly.type }));
        bindings.frequencyType.value = frequencyTypes.Weekly.type;
        expect(emit).not.toHaveBeenCalled();
    });

    test('sorts, adds, removes, and detects selected values', () => {
        const props = frequencyProps({ type: frequencyTypes.Weekly.type, modelValue: '3,1,2' });
        const { bindings, emit } = setup(ScheduleFrequencySelect, props);
        expect(bindings.frequencyValue.value).toEqual([3, 1, 2]);
        expect(bindings.isFrequencyValueSelected(2)).toBe(true);
        expect(bindings.isFrequencyValueSelected(9)).toBe(false);

        bindings.frequencyValue.value = [3, 1];
        expect(emit).toHaveBeenCalledWith('update:modelValue', '1,3');
        bindings.updateFrequencyValue(2, false);
        expect(emit).toHaveBeenCalledWith('update:modelValue', '1,3');
        bindings.updateFrequencyValue(4, true);
        expect(emit).toHaveBeenCalledWith('update:modelValue', '1,2,3,4');
        bindings.updateFrequencyValue(3, null);
        expect(emit).toHaveBeenCalledWith('update:modelValue', '1,2');
    });

    test('scrolls selected frequency into view only when an opened menu has a parent', async () => {
        const { bindings } = setup(ScheduleFrequencySelect, frequencyProps({
            type: frequencyTypes.Weekly.type,
            modelValue: '1',
        }));
        bindings.onMenuStateChanged(false);
        expect(mockDesktopScroll).not.toHaveBeenCalled();

        bindings.onMenuStateChanged(true);
        await actualVue.nextTick();
        expect(mockDesktopScroll).toHaveBeenCalledWith(
            mockDropdownParent,
            '.schedule-frequency-value-container',
            '.frequency-value-selected',
        );
    });
});
