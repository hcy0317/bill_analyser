import { describe, expect, test } from '@jest/globals';

import {
    arrangeArrayWithNewStartIndex,
    arrayBufferToString,
    arrayContainsFieldValue,
    arrayItemToObjectField,
    base64decode,
    base64encode,
    categorizedArrayToPlainArray,
    countSplitItems,
    findDisplayNameByType,
    findNameByType,
    findNameByValue,
    getFirstVisibleItem,
    getItemByKeyValue,
    getNameByKeyValue,
    getNumberValue,
    getObjectOwnFieldCount,
    getPrimaryValueBySecondaryValue,
    isArray,
    isArray1SubsetOfArray2,
    isBoolean,
    isDefined,
    isEquals,
    isFunction,
    isInteger,
    isNumber,
    isObject,
    isObjectEmpty,
    isPrimaryItemHasSecondaryValue,
    isString,
    isYearMonth,
    isYearMonthEquals,
    limitText,
    objectFieldToArrayItem,
    objectFieldWithValueToArrayItem,
    ofObject,
    removeAll,
    replaceAll,
    selectAll,
    selectAllVisible,
    selectInvert,
    selectNone,
    sortNumbersArray,
    splitItemsToMap,
    stringToArrayBuffer
} from '@/lib/common.ts';

describe('common type guards and comparisons', () => {
    test.each([
        ['function', isFunction, () => undefined, true],
        ['non-function', isFunction, 1, false],
        ['defined zero', isDefined, 0, true],
        ['null', isDefined, null, false],
        ['undefined', isDefined, undefined, false],
        ['plain object', isObject, {}, true],
        ['array is not object', isObject, [], false],
        ['null is not object', isObject, null, false],
        ['array', isArray, [], true],
        ['string', isString, 'value', true],
        ['non-string', isString, 2, false],
        ['number', isNumber, Number.NaN, true],
        ['non-number', isNumber, '2', false],
        ['integer', isInteger, -2, true],
        ['fraction', isInteger, 1.5, false],
        ['boolean', isBoolean, false, true],
        ['non-boolean', isBoolean, 0, false]
    ])('%s is classified correctly', (_name, predicate, value, expected) => {
        expect(predicate(value)).toBe(expected);
    });

    test('isArray uses its object-tag fallback when Array.isArray is unavailable', () => {
        const originalIsArray = Array.isArray;

        try {
            Object.defineProperty(Array, 'isArray', { configurable: true, value: undefined });
            expect(isArray([])).toBe(true);
            expect(isArray({ length: 0 })).toBe(false);
        } finally {
            Object.defineProperty(Array, 'isArray', { configurable: true, value: originalIsArray });
        }
    });

    test.each([
        ['2024-07', true],
        ['2024-7', true],
        ['2024-07-extra', false],
        ['year-07', false],
        ['2024-0', false],
        [202407, false]
    ])('isYearMonth(%p) returns %p', (value, expected) => {
        expect(isYearMonth(value)).toBe(expected);
    });

    test('isEquals deeply compares arrays, objects, primitives, and mismatched shapes', () => {
        expect(isEquals('same', 'same')).toBe(true);
        expect(isEquals([1, { nested: ['x'] }], [1, { nested: ['x'] }])).toBe(true);
        expect(isEquals([1], [1, 2])).toBe(false);
        expect(isEquals([1, 2], [1, 3])).toBe(false);
        expect(isEquals({ a: 1 }, { a: 1, b: 2 })).toBe(false);
        expect(isEquals({ a: 1 }, { b: 1 })).toBe(false);
        expect(isEquals({ a: 1 }, { a: 2 })).toBe(false);
        expect(isEquals({ a: 1 }, [1] as unknown as { a: number })).toBe(false);
        expect(isEquals(1, 2)).toBe(false);
    });

    test.each([
        ['2024-07', '2024-7', true],
        ['2024', '2024-07', false],
        ['2024-07', '2024', false],
        ['0-07', '0-07', false],
        ['2024-0', '2024-0', false],
        ['2024-07', '2025-07', false],
        ['2024-07', '2024-08', false]
    ])('isYearMonthEquals(%s, %s) returns %p', (left, right, expected) => {
        expect(isYearMonthEquals(left, right)).toBe(expected);
    });

    test('subset and empty-object checks honor values and own enumerable fields', () => {
        expect(isArray1SubsetOfArray2([1, 2], [2, 1, 3])).toBe(true);
        expect(isArray1SubsetOfArray2([1, 2, 3], [1, 2])).toBe(false);
        expect(isArray1SubsetOfArray2([1, 4], [1, 2, 3])).toBe(false);
        expect(isObjectEmpty({})).toBe(true);
        expect(isObjectEmpty({ own: true })).toBe(false);
        expect(isObjectEmpty(null as unknown as object)).toBe(true);
        expect(isObjectEmpty(Object.create({ inherited: true }) as object)).toBe(true);
    });
});

describe('common conversions and text helpers', () => {
    test('identity, number conversion, sorting, and own-field count preserve their contracts', () => {
        const value = { id: 'same-reference' };
        expect(ofObject(value)).toBe(value);
        expect(getNumberValue('42px', 7)).toBe(42);
        expect(getNumberValue(4.5, 7)).toBe(4.5);
        expect(getNumberValue(false, 7)).toBe(7);
        expect(sortNumbersArray([10, -1, 2])).toEqual([-1, 2, 10]);
        expect(getObjectOwnFieldCount({ a: 1, b: 2 })).toBe(2);
        expect(getObjectOwnFieldCount([])).toBe(0);
        expect(getObjectOwnFieldCount(null as unknown as object)).toBe(0);
        expect(getObjectOwnFieldCount(Object.assign(Object.create({ inherited: 1 }) as object, { own: 1 }))).toBe(1);
    });

    test('replace and remove treat regular-expression characters literally', () => {
        expect(replaceAll('a.b.a.b', 'a.b', 'z')).toBe('z.z');
        expect(replaceAll('x+y+x+y', 'x+y', 'z')).toBe('z+z');
        expect(removeAll('cost$cost$', '$')).toBe('costcost');
    });

    test.each([
        ['short', 10, 'short'],
        ['abcdef', 3, 'abcdef'],
        ['abcdef', 5, 'ab...'],
        ['中文ab', 5, '中文...'],
        ['ｦab12', 5, 'ｦab12']
    ])('limitText(%s, %i) returns %s', (value, maxLength, expected) => {
        expect(limitText(value, maxLength)).toBe(expected);
    });

    test('binary and base64 helpers round-trip byte-oriented strings', () => {
        const buffer = stringToArrayBuffer('A\u0000\u00ff');

        expect(arrayBufferToString(buffer)).toBe('A\u0000\u00ff');
        expect(base64encode(buffer)).toBe('QQD/');
        expect(base64decode('QQD/')).toBe('A\u0000\u00ff');
        expect(base64decode('')).toBe('');
    });
});

describe('common collection lookup and conversion helpers', () => {
    const arrayItems = [
        { id: 'hidden', name: 'Hidden', hidden: true },
        { id: 'visible', name: 'Visible', hidden: false }
    ];
    const objectItems = { first: arrayItems[0]!, second: arrayItems[1]! };

    test('visible-item lookup supports arrays, maps, omitted hidden fields, and empty inputs', () => {
        expect(getFirstVisibleItem(arrayItems, 'hidden')).toBe(arrayItems[1]);
        expect(getFirstVisibleItem(objectItems, 'hidden')).toBe(arrayItems[1]);
        expect(getFirstVisibleItem(arrayItems, '')).toBe(arrayItems[0]);
        expect(getFirstVisibleItem([], 'hidden')).toBeNull();
        expect(getFirstVisibleItem(3 as unknown as typeof arrayItems, 'hidden')).toBeNull();
    });

    test('key-value lookup searches arrays and maps and returns null when absent', () => {
        expect(getItemByKeyValue(arrayItems, 'visible', 'id')).toBe(arrayItems[1]);
        expect(getItemByKeyValue(objectItems, 'hidden', 'id')).toBe(arrayItems[0]);
        expect(getItemByKeyValue(arrayItems, 'missing', 'id')).toBeNull();
        expect(getItemByKeyValue('invalid' as unknown as typeof arrayItems, 'x', 'id')).toBeNull();
    });

    test('name lookup helpers return matching labels or null', () => {
        expect(findNameByValue([{ name: 'One', value: '1' }], '1')).toBe('One');
        expect(findNameByValue([{ name: 'One', value: '1' }], '2')).toBeNull();
        expect(findNameByType([{ name: 'Two', type: 2 }], 2)).toBe('Two');
        expect(findNameByType([{ name: 'Two', type: 2 }], 3)).toBeNull();
        expect(findDisplayNameByType([{ displayName: 'Three', type: 3 }], 3)).toBe('Three');
        expect(findDisplayNameByType([{ displayName: 'Three', type: 3 }], 4)).toBeNull();
    });

    test('getNameByKeyValue supports keyed and positional arrays plus keyed objects', () => {
        const keyedItems: Record<string, string>[] = [
            { id: 'hidden', name: 'Hidden' },
            { id: 'visible', name: 'Visible' }
        ];
        const positionalItems: Record<string, string | number>[] = [
            { index: 0, name: 'Hidden' },
            { index: 1, name: 'Visible' }
        ];
        const keyedObject: Record<string, Record<string, string>> = {
            first: keyedItems[0]!,
            second: keyedItems[1]!
        };

        expect(getNameByKeyValue<string, string>(keyedItems, 'visible', 'id', 'name', 'Fallback')).toBe('Visible');
        expect(getNameByKeyValue<number, string>(positionalItems, 1, null, 'name', 'Fallback')).toBe('Visible');
        expect(getNameByKeyValue<number, string>(positionalItems, 9, null, 'name', 'Fallback')).toBe('Fallback');
        expect(getNameByKeyValue<string, string>(keyedItems, 'visible', null, 'name', 'Fallback')).toBe('Fallback');
        expect(getNameByKeyValue<string, string>(keyedObject, 'hidden', 'id', 'name', 'Fallback')).toBe('Hidden');
        expect(getNameByKeyValue<string, string>(keyedObject, 'second', null, 'name', 'Fallback')).toBe('Visible');
        expect(getNameByKeyValue<string, string>(keyedObject, 'missing', null, 'name', 'Fallback')).toBe('Fallback');
        expect(getNameByKeyValue<number, string>(keyedObject, 1, null, 'name', 'Fallback')).toBe('Fallback');
    });

    test('array membership validates its input before matching a field', () => {
        expect(arrayContainsFieldValue(arrayItems, 'id', 'visible')).toBe(true);
        expect(arrayContainsFieldValue(arrayItems, 'id', 'missing')).toBe(false);
        expect(arrayContainsFieldValue(arrayItems, 'id', '')).toBe(false);
        expect(arrayContainsFieldValue(null as unknown as typeof arrayItems, 'id', 'visible')).toBe(false);
        expect(arrayContainsFieldValue([], 'id', 'visible')).toBe(false);
    });

    test('object and split conversion helpers preserve keys and skip empty tokens', () => {
        expect(objectFieldToArrayItem({ a: 1, b: 2 })).toEqual(['a', 'b']);
        expect(objectFieldWithValueToArrayItem({ a: true, b: false, c: true }, true)).toEqual(['a', 'c']);
        expect(arrayItemToObjectField(['a', 'b'], 7)).toEqual({ a: 7, b: 7 });
        expect(splitItemsToMap('a,,b,a', ',')).toEqual({ a: true, b: true });
        expect(splitItemsToMap(null, ',')).toEqual({});
        expect(countSplitItems('a,,b,a', ',')).toBe(3);
        expect(countSplitItems(undefined, ',')).toBe(0);
        expect(categorizedArrayToPlainArray({ first: [1, 2], second: [3] })).toEqual([1, 2, 3]);
    });
});

describe('common selection and hierarchy helpers', () => {
    test('selection helpers update only ids that exist in the item map', () => {
        const allItems = {
            a: { id: 'a' },
            b: { id: 'b', hidden: true },
            c: { id: 'c', hidden: false }
        };

        const all = { a: true, missing: true };
        selectAll(all, allItems);
        expect(all).toEqual({ a: false, missing: true });

        const none = { a: false, missing: false };
        selectNone(none, allItems);
        expect(none).toEqual({ a: true, missing: false });

        const inverted = { a: true, missing: false };
        selectInvert(inverted, allItems);
        expect(inverted).toEqual({ a: false, missing: false });

        const visible = { a: true, b: true, c: true, missing: true };
        selectAllVisible(visible, allItems);
        expect(visible).toEqual({ a: false, b: true, c: false, missing: true });
    });

    test('secondary membership ignores hidden entries and supports fields or object identity', () => {
        const direct = { code: 'direct' };
        const primary = {
            children: [
                { code: 'hidden', hidden: true },
                { code: 'wanted', hidden: false },
                direct
            ]
        };

        expect(isPrimaryItemHasSecondaryValue({}, 'children', 'code', 'hidden', 'wanted')).toBe(false);
        expect(isPrimaryItemHasSecondaryValue({ children: [] }, 'children', 'code', 'hidden', 'wanted')).toBe(false);
        expect(isPrimaryItemHasSecondaryValue(primary, 'children', 'code', 'hidden', 'hidden')).toBe(false);
        expect(isPrimaryItemHasSecondaryValue(primary, 'children', 'code', 'hidden', 'wanted')).toBe(true);
        expect(isPrimaryItemHasSecondaryValue(primary, 'children', undefined, undefined, direct)).toBe(true);
        expect(isPrimaryItemHasSecondaryValue(primary, 'children', undefined, undefined, { code: 'direct' })).toBe(false);
    });

    test('primary lookup supports arrays, maps, hidden primaries, projected values, and whole items', () => {
        const hiddenPrimary = { hidden: true, children: [{ code: 'wanted' }], result: [{ name: 'hidden' }] };
        const visiblePrimary = { hidden: false, children: [{ code: 'wanted' }], result: [{ name: 'visible' }] };
        const array = [hiddenPrimary, visiblePrimary];
        type PrimaryArray = Parameters<typeof getPrimaryValueBySecondaryValue<string>>[0];
        const typedArray = array as unknown as PrimaryArray;
        const typedObject = { hidden: hiddenPrimary, visible: visiblePrimary } as unknown as PrimaryArray;

        expect(getPrimaryValueBySecondaryValue(typedArray, 'children', 'result', 'hidden', 'code', undefined, 'wanted')).toEqual([{ name: 'visible' }]);
        expect(getPrimaryValueBySecondaryValue(typedArray, 'children', undefined, 'hidden', 'code', undefined, 'wanted')).toBe(visiblePrimary);
        expect(getPrimaryValueBySecondaryValue(typedObject, 'children', 'result', 'hidden', 'code', undefined, 'wanted')).toEqual([{ name: 'visible' }]);
        expect(getPrimaryValueBySecondaryValue(typedObject, 'children', undefined, 'hidden', 'code', undefined, 'wanted')).toBe(visiblePrimary);
        expect(getPrimaryValueBySecondaryValue(typedArray, 'children', 'result', undefined, 'code', undefined, 'missing')).toBeNull();
        expect(getPrimaryValueBySecondaryValue(typedArray, undefined, 'result', undefined, 'code', undefined, 'wanted')).toBeNull();
    });

    test.each([
        [[1, 2, 3, 4], 2, [3, 4, 1, 2]],
        [[1, 2, 3], 0, [1, 2, 3]],
        [[1, 2, 3], -1, [1, 2, 3]],
        [[1, 2, 3], 3, [1, 2, 3]]
    ])('arrangeArrayWithNewStartIndex(%p, %i) returns %p', (array, startIndex, expected) => {
        expect(arrangeArrayWithNewStartIndex(array, startIndex)).toEqual(expected);
    });
});
