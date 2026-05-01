import { entries } from '@/core/base.ts';
import type { IconInfo, IconInfoWithId } from '@/core/icon.ts';
import { ALL_CATEGORY_ICONS, DEFAULT_CATEGORY_ICON } from '@/consts/icon.ts';

type IconIdValue = string | number | null | undefined;

const LINE_AWESOME_FAMILY_CLASSES = new Set(['la', 'las', 'lar', 'lab', 'lal', 'lad']);
const LINE_AWESOME_MODIFIER_CLASSES = new Set([
    'la-fw',
    'la-lg',
    'la-xs',
    'la-sm',
    'la-1x',
    'la-2x',
    'la-3x',
    'la-4x',
    'la-5x',
    'la-6x',
    'la-7x',
    'la-8x',
    'la-9x',
    'la-10x',
    'la-spin',
    'la-pulse',
    'la-border',
    'la-pull-left',
    'la-pull-right',
    'la-rotate-90',
    'la-rotate-180',
    'la-rotate-270',
    'la-flip-horizontal',
    'la-flip-vertical',
    'la-flip-both',
    'la-stack',
    'la-stack-1x',
    'la-stack-2x',
    'la-inverse'
]);
const LINE_AWESOME_CLASS_PATTERN = /^la(?:[bdrs]?|l)?$|^la-[a-z0-9-]+$/;

export const LEGACY_MDI_CATEGORY_ICONS: Record<string, IconInfo> = {
    'mdi-account-group': { icon: 'las la-users' },
    'mdi-airplane': { icon: 'las la-plane' },
    'mdi-bank': { icon: 'las la-landmark' },
    'mdi-bank-transfer': { icon: 'las la-exchange-alt' },
    'mdi-basket': { icon: 'las la-shopping-basket' },
    'mdi-book': { icon: 'las la-book' },
    'mdi-book-open-variant': { icon: 'las la-book-open' },
    'mdi-briefcase-clock': { icon: 'las la-briefcase' },
    'mdi-bus': { icon: 'las la-bus' },
    'mdi-car': { icon: 'las la-car' },
    'mdi-cart': { icon: 'las la-shopping-cart' },
    'mdi-cash': { icon: 'las la-money-bill-wave-alt' },
    'mdi-cash-plus': { icon: 'las la-plus-circle' },
    'mdi-cellphone': { icon: 'las la-mobile' },
    'mdi-chart-line': { icon: 'las la-chart-line' },
    'mdi-coffee': { icon: 'las la-coffee' },
    'mdi-credit-card': { icon: 'las la-credit-card' },
    'mdi-dots-horizontal': { icon: 'las la-ellipsis-h' },
    'mdi-email-open-outline': { icon: 'las la-envelope' },
    'mdi-finance': { icon: 'las la-chart-line' },
    'mdi-food': { icon: 'las la-utensils' },
    'mdi-gamepad': { icon: 'las la-gamepad' },
    'mdi-gamepad-variant': { icon: 'las la-gamepad' },
    'mdi-gas-station': { icon: 'las la-gas-pump' },
    'mdi-gift': { icon: 'las la-gift' },
    'mdi-gift-outline': { icon: 'las la-gift' },
    'mdi-hammer': { icon: 'las la-tools' },
    'mdi-home': { icon: 'las la-home' },
    'mdi-home-account': { icon: 'las la-home' },
    'mdi-home-city': { icon: 'las la-building' },
    'mdi-hospital': { icon: 'las la-hospital' },
    'mdi-laptop': { icon: 'las la-laptop' },
    'mdi-lipstick': { icon: 'las la-spray-can' },
    'mdi-medical-bag': { icon: 'las la-briefcase-medical' },
    'mdi-movie': { icon: 'las la-film' },
    'mdi-music': { icon: 'las la-music' },
    'mdi-parking': { icon: 'las la-car' },
    'mdi-phone': { icon: 'las la-phone-volume' },
    'mdi-pill': { icon: 'las la-pills' },
    'mdi-presentation': { icon: 'las la-chalkboard-teacher' },
    'mdi-school': { icon: 'las la-graduation-cap' },
    'mdi-shape': { icon: 'las la-shapes' },
    'mdi-shopping': { icon: 'las la-shopping-bag' },
    'mdi-silverware-fork-knife': { icon: 'las la-utensils' },
    'mdi-subway': { icon: 'las la-subway' },
    'mdi-tag': { icon: 'las la-tag' },
    'mdi-taxi': { icon: 'las la-taxi' },
    'mdi-train': { icon: 'las la-train' },
    'mdi-tshirt-crew': { icon: 'las la-tshirt' },
    'mdi-wallet': { icon: 'las la-wallet' },
    'mdi-wallet-membership': { icon: 'las la-wallet' },
    'mdi-water': { icon: 'las la-tint' },
    'mdi-wifi': { icon: 'las la-wifi' }
};

export function getIconsInRows(allIconInfos: Record<string, IconInfo>, itemPerRow: number): IconInfoWithId[][] {
    const ret: IconInfoWithId[][] = [];
    let rowCount = 0;

    for (const [iconInfoId, iconInfo] of entries(allIconInfos)) {
        if (!ret[rowCount]) {
            ret[rowCount] = [];
        } else if (ret[rowCount] && ret[rowCount]!.length >= itemPerRow) {
            rowCount++;
            ret[rowCount] = [];
        }

        ret[rowCount]!.push({
            id: iconInfoId,
            icon: iconInfo.icon
        });
    }

    return ret;
}

export function normalizeIconId(iconId: IconIdValue): string | null {
    if (iconId === null || iconId === undefined) {
        return null;
    }

    const normalizedIconId = String(iconId).trim();

    return normalizedIconId || null;
}

export function isLineAwesomeIconClass(iconClass: string): boolean {
    const tokens = iconClass.trim().split(/\s+/).filter(Boolean);

    if (tokens.length < 2) {
        return false;
    }

    let hasFamilyClass = false;
    let hasIconClass = false;

    for (const token of tokens) {
        if (!LINE_AWESOME_CLASS_PATTERN.test(token)) {
            return false;
        }

        if (LINE_AWESOME_FAMILY_CLASSES.has(token)) {
            hasFamilyClass = true;
        } else if (!LINE_AWESOME_MODIFIER_CLASSES.has(token)) {
            hasIconClass = true;
        }
    }

    return hasFamilyClass && hasIconClass;
}

export function resolveCategoryIcon(iconId: IconIdValue): string {
    const normalizedIconId = normalizeIconId(iconId);

    if (!normalizedIconId) {
        return DEFAULT_CATEGORY_ICON.icon;
    }

    const presetIcon = ALL_CATEGORY_ICONS[normalizedIconId];

    if (presetIcon) {
        return presetIcon.icon;
    }

    const legacyMdiIcon = LEGACY_MDI_CATEGORY_ICONS[normalizedIconId];

    if (legacyMdiIcon) {
        return legacyMdiIcon.icon;
    }

    if (isLineAwesomeIconClass(normalizedIconId)) {
        return normalizedIconId;
    }

    return DEFAULT_CATEGORY_ICON.icon;
}
