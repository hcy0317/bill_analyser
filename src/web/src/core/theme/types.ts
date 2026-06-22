export enum ThemeType {
    Light = 'light',
    Dark = 'dark',
    HalloweenLight = 'halloween-light',
    HalloweenDark = 'halloween-dark',
    ForestLight = 'forest-light',
    ForestDark = 'forest-dark',
    WireframeLight = 'wireframe-light',
    WireframeDark = 'wireframe-dark',
    BlackLight = 'black-light',
    BlackDark = 'black-dark',
    DraculaLight = 'dracula-light',
    DraculaDark = 'dracula-dark',
    BusinessLight = 'business-light',
    BusinessDark = 'business-dark',
    NightLight = 'night-light',
    NightDark = 'night-dark',
    DimLight = 'dim-light',
    DimDark = 'dim-dark'
}

export const SYSTEM_THEME_PREFERENCE = 'auto';

export const APPLICATION_THEME_ORDER = [
    ThemeType.Light,
    ThemeType.Dark,
    ThemeType.HalloweenLight,
    ThemeType.HalloweenDark,
    ThemeType.ForestLight,
    ThemeType.ForestDark,
    ThemeType.WireframeLight,
    ThemeType.WireframeDark,
    ThemeType.BlackLight,
    ThemeType.BlackDark,
    ThemeType.DraculaLight,
    ThemeType.DraculaDark,
    ThemeType.BusinessLight,
    ThemeType.BusinessDark,
    ThemeType.NightLight,
    ThemeType.NightDark,
    ThemeType.DimLight,
    ThemeType.DimDark
] as const;

export const APPLICATION_THEME_FAMILY_OPTION_ORDER = [
    ThemeType.Light,
    ThemeType.HalloweenLight,
    ThemeType.ForestLight,
    ThemeType.WireframeLight,
    ThemeType.BlackLight,
    ThemeType.DraculaLight,
    ThemeType.BusinessLight,
    ThemeType.NightLight,
    ThemeType.DimLight
] as const;

export type ApplicationThemeName = (typeof APPLICATION_THEME_ORDER)[number];
export type ThemePreference = typeof SYSTEM_THEME_PREFERENCE | ApplicationThemeName;
export type ApplicationThemeMode = 'light' | 'dark';
export type ApplicationThemeFamily =
    'classic' | 'halloween' | 'forest' | 'wireframe' | 'black' | 'dracula' | 'business' | 'night' | 'dim';

export interface ApplicationVuetifyThemeDefinition {
    readonly dark: boolean;
    readonly colors: Record<string, string>;
    readonly variables: Record<string, string | number>;
}

export interface ApplicationMobileThemeConfig {
    readonly primary: string;
    readonly cssVariables: Record<string, string>;
    readonly metaThemeColor: {
        readonly default: string;
        readonly backdrop: string;
        readonly pushBackdrop: string;
    };
}

export interface ApplicationThemeDefinition {
    readonly name: ApplicationThemeName;
    readonly displayKey: string;
    readonly dark: boolean;
    readonly family: ApplicationThemeFamily;
    readonly mode: ApplicationThemeMode;
    readonly pairedTheme: ApplicationThemeName;
    readonly vuetify: ApplicationVuetifyThemeDefinition;
    readonly mobile: ApplicationMobileThemeConfig;
}

export interface ThemePreferenceOption {
    readonly name: string;
    readonly value: ThemePreference;
}

export type ApplicationThemeOverride = Partial<ApplicationVuetifyThemeDefinition> & {
    readonly colors?: Record<string, string>;
    readonly variables?: Record<string, string | number>;
};

export interface ApplicationThemeVariant {
    readonly name: ApplicationThemeName;
    readonly displayKey: string;
    readonly family: ApplicationThemeFamily;
    readonly mode: ApplicationThemeMode;
    readonly override?: ApplicationThemeOverride;
}
