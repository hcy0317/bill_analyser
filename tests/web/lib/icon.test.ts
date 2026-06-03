import { describe, expect, test } from '@jest/globals';

import { ALL_CATEGORY_ICONS, DEFAULT_CATEGORY_ICON } from '@/consts/icon.ts';
import {
    isLineAwesomeIconClass,
    normalizeIconId,
    resolveCategoryIcon
} from '@/lib/icon.ts';

describe('category icon resolver', () => {
    test('covers every preset category icon id with a valid Line Awesome class', () => {
        expect(ALL_CATEGORY_ICONS['830']?.icon).toBe('las la-chart-pie');

        for (const iconInfo of Object.values(ALL_CATEGORY_ICONS)) {
            expect(isLineAwesomeIconClass(iconInfo.icon)).toBe(true);
        }
    });

    test('normalizes numeric category icon ids before resolving presets', () => {
        expect(normalizeIconId(830)).toBe('830');
        expect(resolveCategoryIcon(830)).toBe('las la-chart-pie');
        expect(resolveCategoryIcon(' 830 ')).toBe('las la-chart-pie');
    });

    test('accepts Line Awesome class inputs directly', () => {
        expect(resolveCategoryIcon('las la-wallet')).toBe('las la-wallet');
        expect(resolveCategoryIcon('lab la-amazon')).toBe('lab la-amazon');
        expect(resolveCategoryIcon('las la-wallet la-fw')).toBe('las la-wallet la-fw');
    });

    test('falls back before invalid icon classes reach ItemIcon', () => {
        expect(resolveCategoryIcon(null)).toBe(DEFAULT_CATEGORY_ICON.icon);
        expect(resolveCategoryIcon('')).toBe(DEFAULT_CATEGORY_ICON.icon);
        expect(resolveCategoryIcon('mdi-close-circle')).toBe(DEFAULT_CATEGORY_ICON.icon);
        expect(resolveCategoryIcon('las')).toBe(DEFAULT_CATEGORY_ICON.icon);
        expect(resolveCategoryIcon('not-an-icon')).toBe(DEFAULT_CATEGORY_ICON.icon);
    });
});
