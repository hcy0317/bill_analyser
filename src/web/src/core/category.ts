import type { ColorValue } from '@/core/color.ts';

export enum CategoryType {
    Expense = 3,
    Income = 2,
    Transfer = 4,
    Investment = 5
}

export const ALL_CATEGORY_TYPES: CategoryType[] = [
    CategoryType.Expense,
    CategoryType.Income,
    CategoryType.Transfer,
    CategoryType.Investment
];

export interface PresetCategory {
    readonly name: string;
    readonly categoryIconId: string;
    readonly color: ColorValue;
    readonly keywords?: string;
    readonly subCategories: PresetSubCategory[];
}

export interface PresetSubCategory {
    readonly name: string;
    readonly categoryIconId: string;
    readonly color: ColorValue;
    readonly keywords?: string;
}

export interface LocalizedPresetCategory {
    readonly name: string;
    readonly type: CategoryType;
    readonly icon: string;
    readonly color: ColorValue;
    readonly keywords?: string;
    readonly subCategories: LocalizedPresetSubCategory[];
}

export interface LocalizedPresetSubCategory {
    readonly name: string;
    readonly type: CategoryType;
    readonly icon: string;
    readonly color: ColorValue;
    readonly keywords?: string;
}
