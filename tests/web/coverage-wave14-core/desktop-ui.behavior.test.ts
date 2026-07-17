/* eslint-disable @typescript-eslint/no-explicit-any */
import { afterAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

import {
    getCssValue,
    getNavSideBarOuterHeight,
    getOuterHeight,
    scrollToSelectedItem,
    setChildInputFocus,
} from '@/lib/ui/desktop.ts';

const styles = new WeakMap<object, Record<string, string>>();
const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
const mockGetComputedStyle = jest.fn((target: object) => ({
    getPropertyValue: (name: string) => styles.get(target)?.[name] ?? '0',
}));
Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: { getComputedStyle: mockGetComputedStyle },
});

function element(style: Record<string, string> = {}): any {
    const value: any = {
        children: [],
        scrollTop: 0,
        querySelector: jest.fn(() => null),
        querySelectorAll: jest.fn(() => []),
    };
    styles.set(value, style);
    Object.defineProperty(value, 'offsetTop', { configurable: true, value: 0, writable: true });
    return value;
}

beforeEach(() => {
    mockGetComputedStyle.mockClear();
});

afterAll(() => {
    if (originalWindow) {
        Object.defineProperty(globalThis, 'window', originalWindow);
    } else {
        delete (globalThis as any).window;
    }
});

describe('desktop UI geometry helpers', () => {
    test('returns zero for missing elements and sums the complete outer box', () => {
        expect(getOuterHeight(null)).toBe(0);
        const target = element({
            height: '100px',
            'padding-top': '10px',
            'padding-bottom': '5px',
            'margin-top': '3px',
            'margin-bottom': '2px',
        });
        expect(getOuterHeight(target)).toBe(120);
    });

    test('sums navigation drawer children and handles every empty boundary', () => {
        expect(getNavSideBarOuterHeight(null)).toBe(0);
        const root = element();
        root.querySelectorAll.mockReturnValueOnce([]);
        expect(getNavSideBarOuterHeight(root)).toBe(0);
        const emptyContent = element();
        root.querySelectorAll.mockReturnValueOnce([emptyContent]);
        expect(getNavSideBarOuterHeight(root)).toBe(0);

        const first = element({ height: '10px' });
        const second = element({ height: '20px', 'margin-top': '5px' });
        const content = element();
        content.children = [first, second];
        root.querySelectorAll.mockReturnValueOnce([content]);
        expect(getNavSideBarOuterHeight(root)).toBe(35);
    });

    test('reads CSS values and focuses/selects only existing child inputs', () => {
        expect(getCssValue(null, 'padding-top')).toBe('0');
        const parent = element({ 'padding-top': '12px' });
        expect(getCssValue(parent, 'padding-top')).toBe('12px');
        expect(() => setChildInputFocus(undefined, 'input')).not.toThrow();
        setChildInputFocus(parent, 'input');

        const input = { focus: jest.fn(), select: jest.fn() };
        parent.querySelector.mockReturnValueOnce(input);
        setChildInputFocus(parent, 'input');
        expect(input.focus).toHaveBeenCalled();
        expect(input.select).toHaveBeenCalled();
    });

    test('returns early when parent, requested container, or selected item is absent', () => {
        expect(() => scrollToSelectedItem(null, null, '.selected')).not.toThrow();
        const parent = element();
        parent.querySelectorAll.mockReturnValueOnce([]);
        scrollToSelectedItem(parent, '.list', '.selected');
        expect(parent.scrollTop).toBe(0);

        const container = element();
        parent.querySelectorAll.mockReturnValueOnce([container]);
        container.querySelectorAll.mockReturnValueOnce([]);
        scrollToSelectedItem(parent, '.list', '.selected');
        expect(container.scrollTop).toBe(0);
    });

    test('centers one selected item only when the target position is positive', () => {
        const container = element({ height: '100px', 'padding-top': '10px' });
        const selected = element({ height: '20px' });
        container.querySelectorAll.mockReturnValue([selected]);
        selected.offsetTop = 20;
        scrollToSelectedItem(container, null, '.selected');
        expect(container.scrollTop).toBe(0);

        selected.offsetTop = 100;
        scrollToSelectedItem(container, null, '.selected');
        expect(container.scrollTop).toBe(45);
    });

    test('centers multiple nearby selections and anchors the first when their span exceeds the container', () => {
        const container = element({ height: '100px', 'padding-top': '10px' });
        const first = element({ height: '20px' });
        const last = element({ height: '20px' });
        first.offsetTop = 100;
        last.offsetTop = 140;
        container.querySelectorAll.mockReturnValue([first, last]);
        scrollToSelectedItem(container, null, '.selected');
        expect(container.scrollTop).toBe(65);

        last.offsetTop = 300;
        scrollToSelectedItem(container, null, '.selected');
        expect(container.scrollTop).toBe(90);
    });
});
