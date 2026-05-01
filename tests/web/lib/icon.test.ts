import { describe, expect, test } from '@jest/globals';

import { ALL_CATEGORY_ICONS, DEFAULT_CATEGORY_ICON } from '@/consts/icon.ts';
import {
    isLineAwesomeIconClass,
    LEGACY_MDI_CATEGORY_ICONS,
    normalizeIconId,
    resolveCategoryIcon
} from '@/lib/icon.ts';

const BACKEND_KNOWN_LEGACY_CATEGORY_ICONS = [
    'mdi-account-group',
    'mdi-airplane',
    'mdi-bank-transfer',
    'mdi-basket',
    'mdi-book',
    'mdi-book-open-variant',
    'mdi-briefcase-clock',
    'mdi-bus',
    'mdi-cash-plus',
    'mdi-cellphone',
    'mdi-chart-line',
    'mdi-dots-horizontal',
    'mdi-email-open-outline',
    'mdi-finance',
    'mdi-food',
    'mdi-gamepad-variant',
    'mdi-gas-station',
    'mdi-gift',
    'mdi-gift-outline',
    'mdi-hammer',
    'mdi-home',
    'mdi-home-account',
    'mdi-home-city',
    'mdi-hospital',
    'mdi-laptop',
    'mdi-lipstick',
    'mdi-medical-bag',
    'mdi-parking',
    'mdi-phone',
    'mdi-pill',
    'mdi-presentation',
    'mdi-school',
    'mdi-shopping',
    'mdi-subway',
    'mdi-taxi',
    'mdi-train',
    'mdi-tshirt-crew',
    'mdi-wallet-membership',
    'mdi-water',
    'mdi-wifi'
];

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

    test('accepts existing Line Awesome class inputs directly', () => {
        expect(resolveCategoryIcon('las la-wallet')).toBe('las la-wallet');
        expect(resolveCategoryIcon('lab la-amazon')).toBe('lab la-amazon');
        expect(resolveCategoryIcon('las la-wallet la-fw')).toBe('las la-wallet la-fw');
    });

    test('maps legacy mdi category icon inputs to Line Awesome classes', () => {
        expect(resolveCategoryIcon('mdi-food')).toBe('las la-utensils');
        expect(resolveCategoryIcon('mdi-coffee')).toBe('las la-coffee');
        expect(resolveCategoryIcon('mdi-silverware-fork-knife')).toBe('las la-utensils');
        expect(resolveCategoryIcon('mdi-shape')).toBe('las la-shapes');
    });

    test('covers every backend-known legacy category mdi icon intentionally', () => {
        for (const legacyIconId of BACKEND_KNOWN_LEGACY_CATEGORY_ICONS) {
            const mappedIcon = LEGACY_MDI_CATEGORY_ICONS[legacyIconId]?.icon;

            expect(mappedIcon).toBeTruthy();
            expect(isLineAwesomeIconClass(mappedIcon as string)).toBe(true);
            expect(resolveCategoryIcon(legacyIconId)).toBe(mappedIcon);
        }
    });

    test('falls back before invalid icon classes reach ItemIcon', () => {
        expect(resolveCategoryIcon(null)).toBe(DEFAULT_CATEGORY_ICON.icon);
        expect(resolveCategoryIcon('')).toBe(DEFAULT_CATEGORY_ICON.icon);
        expect(resolveCategoryIcon('mdi-close-circle')).toBe(DEFAULT_CATEGORY_ICON.icon);
        expect(resolveCategoryIcon('las')).toBe(DEFAULT_CATEGORY_ICON.icon);
        expect(resolveCategoryIcon('not-an-icon')).toBe(DEFAULT_CATEGORY_ICON.icon);
    });
});
