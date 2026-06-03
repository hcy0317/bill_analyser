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

    if (isLineAwesomeIconClass(normalizedIconId)) {
        return normalizedIconId;
    }

    return DEFAULT_CATEGORY_ICON.icon;
}
