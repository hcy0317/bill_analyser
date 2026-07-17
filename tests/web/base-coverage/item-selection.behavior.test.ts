import { beforeEach, describe, expect, jest, test } from '@jest/globals';
const { reactive, ref } = jest.requireActual('vue') as typeof import('@/../node_modules/vue');

import { DEFAULT_ACCOUNT_ICON } from '@/consts/icon.ts';
import {
    DEFAULT_ACCOUNT_COLOR,
    DEFAULT_CATEGORY_COLOR,
    DEFAULT_COLOR_STYLE_VARIABLE,
    DEFAULT_ICON_COLOR
} from '@/consts/color.ts';
import { useItemIconBase, type CommonIconProps } from '@/components/base/ItemIconBase.ts';
import {
    useTwoLevelItemSelectionBase,
    type TwoLevelItemSelectionBaseProps
} from '@/components/base/TwoLevelItemSelectionBase.ts';
import {
    useTwoColumnListItemSelectionBase,
    type CommonTwoColumnListItemSelectionProps
} from '@/components/base/TwoColumnListItemSelectionBase.ts';

const mockResolveCategoryIcon = jest.fn((value: unknown) => `category:${String(value)}`);

jest.mock('@/lib/icon.ts', () => ({
    __esModule: true,
    resolveCategoryIcon: (value: unknown) => mockResolveCategoryIcon(value)
}));

jest.mock('@/locales/helpers.ts', () => ({
    __esModule: true,
    useI18n: () => ({
        ti: (value: string, translate: boolean) => translate ? `translated ${value}` : value
    })
}));

describe('item icon behavior', () => {
    beforeEach(() => {
        mockResolveCategoryIcon.mockClear();
    });

    test('resolves account and category icons with safe fallbacks', () => {
        const base = useItemIconBase({ iconType: 'account', iconId: '1' });

        expect(base.getAccountIcon('1')).toBe('las la-wallet');
        expect(base.getAccountIcon(10)).toBe('las la-coins');
        expect(base.getAccountIcon(null)).toBe(DEFAULT_ACCOUNT_ICON.icon);
        expect(base.getAccountIcon(undefined)).toBe(DEFAULT_ACCOUNT_ICON.icon);
        expect(base.getAccountIcon('missing')).toBe(DEFAULT_ACCOUNT_ICON.icon);
        expect(base.getCategoryIcon(7)).toBe('category:7');
        expect(mockResolveCategoryIcon).toHaveBeenCalledWith(7);
    });

    test('computes account styles for defaults, custom colors, attributes, and sizes', () => {
        const props = reactive<CommonIconProps>({
            iconType: 'account',
            iconId: '1',
            color: DEFAULT_ACCOUNT_COLOR,
            additionalColorAttr: 'data-color',
            size: 18
        });
        const base = useItemIconBase(props);

        expect(base.style.value).toEqual({
            color: DEFAULT_COLOR_STYLE_VARIABLE,
            'data-color': DEFAULT_ACCOUNT_COLOR,
            'font-size': 18
        });

        props.color = '123456';
        props.defaultColor = '#ff0000';
        expect(base.style.value).toEqual({ color: '#123456', 'data-color': '123456', 'font-size': 18 });

        props.additionalColorAttr = undefined;
        props.size = undefined;
        expect(base.style.value).toEqual({ color: '#123456' });
    });

    test('computes category and fixed styles against their own sentinel colors', () => {
        const props = reactive<CommonIconProps>({ iconType: 'category', iconId: 'food', color: DEFAULT_CATEGORY_COLOR });
        const base = useItemIconBase(props);

        expect(base.style.value).toEqual({ color: DEFAULT_COLOR_STYLE_VARIABLE });
        props.color = 'abcdef';
        props.defaultColor = '#0000ff';
        props.additionalColorAttr = 'fill';
        props.size = '2rem';
        expect(base.style.value).toEqual({ color: '#abcdef', fill: 'abcdef', 'font-size': '2rem' });

        props.iconType = 'fixed';
        props.color = DEFAULT_ICON_COLOR;
        expect(base.style.value).toEqual({ color: '#0000ff', fill: DEFAULT_ICON_COLOR, 'font-size': '2rem' });
        props.color = 'fedcba';
        expect(base.style.value['color']).toBe('#fedcba');

        props.color = undefined;
        props.additionalColorAttr = undefined;
        props.size = undefined;
        expect(base.style.value).toEqual({ color: '#0000ff' });
    });
});

function selectionProps(overrides: Partial<TwoLevelItemSelectionBaseProps> = {}): TwoLevelItemSelectionBaseProps {
    return {
        modelValue: 'a1',
        primaryTitleField: 'title',
        primaryTitleI18n: true,
        primaryHiddenField: 'hidden',
        primarySubItemsField: 'children',
        secondaryValueField: 'id',
        secondaryTitleField: 'title',
        secondaryTitleI18n: false,
        secondaryHiddenField: 'hidden',
        enableFilter: true,
        items: [
            {
                id: 'a',
                title: 'Accounts',
                children: [
                    { id: 'a1', title: 'Cash' },
                    { id: 'a2', title: 'Savings', hidden: true },
                    { id: 'a3', title: 'Card' }
                ]
            },
            {
                id: 'b',
                title: 'Budgets',
                children: [{ id: 'b1', title: 'Food' }]
            },
            {
                id: 'c',
                title: 'Hidden',
                hidden: true,
                children: [{ id: 'c1', title: 'Secret' }]
            }
        ],
        ...overrides
    };
}

describe('two-level item selection behavior', () => {
    test('counts visible primaries and filters by translated primary or secondary titles', () => {
        const base = useTwoLevelItemSelectionBase(selectionProps());

        expect(base.visibleItemsCount.value).toBe(2);
        expect(base.filteredItems.value.map(item => item['id'])).toEqual(['a', 'b']);

        base.filterContent.value = 'translated acc';
        expect(base.filteredItems.value.map(item => item['id'])).toEqual(['a']);
        expect(base.getFilteredSubItems(base.filteredItems.value[0]).map(item => item['id'])).toEqual(['a1', 'a3']);

        base.filterContent.value = 'food';
        expect(base.filteredItems.value.map(item => item['id'])).toEqual(['b']);
        expect(base.getFilteredSubItems(selectionProps().items[1]).map(item => item['id'])).toEqual(['b1']);

        base.filterContent.value = 'missing';
        expect(base.filteredItems.value).toEqual([]);
    });

    test('handles disabled filters, missing primaries, and hidden secondary items', () => {
        const props = selectionProps({ enableFilter: false, primaryTitleField: undefined });
        const base = useTwoLevelItemSelectionBase(props);
        base.filterContent.value = 'does-not-matter';

        expect(base.filteredItems.value.map(item => item['id'])).toEqual(['a', 'b']);
        expect(base.getFilteredSubItems(props.items[0]).map(item => item['id'])).toEqual(['a1', 'a3']);
        expect(base.getFilteredSubItems(null)).toEqual([]);

        const noSubField = selectionProps({ primarySubItemsField: '' });
        const noSubBase = useTwoLevelItemSelectionBase(noSubField);
        expect(noSubBase.getFilteredSubItems(noSubField.items[0])).toEqual([]);
    });

    test('selects and updates secondary values by field or object identity', () => {
        const props = selectionProps();
        const base = useTwoLevelItemSelectionBase(props);
        const primary = props.items[0]!;
        const child = (primary['children'] as Record<string, unknown>[])[0]!;

        expect(base.isSecondaryValueSelected('a1', child)).toBe(true);
        expect(base.isSecondaryValueSelected('other', child)).toBe(false);
        expect(base.getSelectedSecondaryItem('a1', primary)).toBe(child);
        expect(base.getSelectedSecondaryItem('', primary)).toBeNull();
        expect(base.getSelectedSecondaryItem('a1', null)).toBeNull();

        const selected = ref<unknown>('');
        base.updateCurrentSecondaryValue(selected, child);
        expect(selected.value).toBe('a1');

        const objectProps = selectionProps({ secondaryValueField: undefined });
        const objectBase = useTwoLevelItemSelectionBase(objectProps);
        const objectSelected = ref<unknown>(null);
        expect(objectBase.isSecondaryValueSelected(child, child)).toBe(true);
        objectBase.updateCurrentSecondaryValue(objectSelected, child);
        expect(objectSelected.value).toStrictEqual(child);
    });

    test('keeps primary items without title/subitems out of matching filters', () => {
        const props = selectionProps({
            primaryTitleField: undefined,
            primarySubItemsField: 'children',
            items: [{ id: 'empty', children: [] }]
        });
        const base = useTwoLevelItemSelectionBase(props);
        base.filterContent.value = 'x';
        expect(base.filteredItems.value).toEqual([]);

        const noFilter = useTwoLevelItemSelectionBase(selectionProps({ enableFilter: undefined }));
        noFilter.filterContent.value = '';
        expect(noFilter.getFilteredSubItems(noFilter.filteredItems.value[0])).toHaveLength(2);

        const absentItems = useTwoLevelItemSelectionBase(selectionProps({ items: undefined as unknown as Record<string, unknown>[] }));
        expect(absentItems.visibleItemsCount.value).toBe(0);
        expect(absentItems.filteredItems.value).toEqual([]);
    });
});

describe('two-column item selection behavior', () => {
    function columnProps(overrides: Partial<CommonTwoColumnListItemSelectionProps> = {}): CommonTwoColumnListItemSelectionProps {
        return {
            ...selectionProps(),
            primaryValueField: 'id',
            ...overrides
        };
    }

    test('maps secondary values to primaries and updates field-backed selections', () => {
        const props = columnProps();
        const base = useTwoColumnListItemSelectionBase(props);

        expect(base.visibleItemsCount.value).toBe(2);
        expect(base.getCurrentPrimaryValueBySecondaryValue('b1')).toBe('b');
        expect(base.getSelectedPrimaryItem('a')).toBe(props.items[0]);

        const selectedPrimary = ref<unknown>('');
        base.updateCurrentPrimaryValue(selectedPrimary, props.items[1]);
        expect(selectedPrimary.value).toBe('b');

        const selectedSecondary = ref<unknown>('');
        const secondary = (props.items[1]!['children'] as Record<string, unknown>[])[0]!;
        base.updateCurrentSecondaryValue(selectedSecondary, secondary);
        expect(selectedSecondary.value).toBe('b1');
    });

    test('supports object-backed primary and secondary selections', () => {
        const props = columnProps({ primaryValueField: undefined, secondaryValueField: undefined });
        const base = useTwoColumnListItemSelectionBase(props);
        const primary = props.items[0]!;
        const secondary = (primary['children'] as Record<string, unknown>[])[0]!;

        expect(base.getSelectedPrimaryItem(primary)).toBe(primary);
        const selectedPrimary = ref<unknown>(null);
        base.updateCurrentPrimaryValue(selectedPrimary, primary);
        expect(selectedPrimary.value).toStrictEqual(primary);

        expect(base.isSecondaryValueSelected(secondary, secondary)).toBe(true);
        const selectedSecondary = ref<unknown>(null);
        base.updateCurrentSecondaryValue(selectedSecondary, secondary);
        expect(selectedSecondary.value).toStrictEqual(secondary);
    });
});
