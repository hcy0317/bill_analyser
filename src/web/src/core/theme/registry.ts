import {
    APPLICATION_THEME_FAMILY_OPTION_ORDER,
    APPLICATION_THEME_ORDER,
    SYSTEM_THEME_PREFERENCE,
    ThemeType
} from './types.ts';
import type {
    ApplicationMobileThemeConfig,
    ApplicationThemeDefinition,
    ApplicationThemeMode,
    ApplicationThemeName,
    ApplicationThemeOverride,
    ApplicationThemeVariant,
    ApplicationVuetifyThemeDefinition,
    ThemePreference,
    ThemePreferenceOption
} from './types.ts';
import { darkVuetifyTheme, lightVuetifyTheme } from './base.ts';
import { themePairByFamily, themeVariants } from './variants.ts';

function mergeVuetifyTheme(baseTheme: ApplicationVuetifyThemeDefinition, override: ApplicationThemeOverride = {}): ApplicationVuetifyThemeDefinition {
    return {
        dark: override.dark ?? baseTheme.dark,
        colors: {
            ...baseTheme.colors,
            ...override.colors
        },
        variables: {
            ...baseTheme.variables,
            ...override.variables
        }
    };
}

function createMobileThemeConfig(vuetifyTheme: ApplicationVuetifyThemeDefinition, override?: Partial<ApplicationMobileThemeConfig>): ApplicationMobileThemeConfig {
    const primary = override?.primary ?? vuetifyTheme.colors['primary']!;
    const background = vuetifyTheme.colors['background']!;
    const surface = vuetifyTheme.colors['surface']!;
    const onBackground = vuetifyTheme.colors['on-background']!;
    const onSurface = vuetifyTheme.colors['on-surface']!;

    return {
        primary,
        cssVariables: {
            '--f7-theme-color': primary,
            '--f7-page-bg-color': background,
            '--f7-text-color': onBackground,
            '--f7-bars-bg-color': surface,
            '--f7-bars-text-color': onSurface,
            '--f7-list-bg-color': surface,
            '--f7-block-strong-bg-color': surface,
            '--f7-card-bg-color': surface,
            '--f7-input-bg-color': surface,
            '--f7-border-color': vuetifyTheme.dark ? 'rgba(255, 255, 255, 0.14)' : 'rgba(65, 57, 53, 0.14)',
            ...override?.cssVariables
        },
        metaThemeColor: {
            default: background,
            backdrop: vuetifyTheme.dark ? '#0b0b0b' : '#949495',
            pushBackdrop: '#000000',
            ...override?.metaThemeColor
        }
    };
}

function createThemeDefinition(variant: ApplicationThemeVariant): ApplicationThemeDefinition {
    const baseTheme = variant.mode === 'dark' ? darkVuetifyTheme : lightVuetifyTheme;
    const vuetify = mergeVuetifyTheme(baseTheme, variant.override);
    const pairedMode: ApplicationThemeMode = variant.mode === 'dark' ? 'light' : 'dark';

    return {
        name: variant.name,
        displayKey: variant.displayKey,
        dark: vuetify.dark,
        family: variant.family,
        mode: variant.mode,
        pairedTheme: themePairByFamily[variant.family][pairedMode],
        vuetify,
        mobile: createMobileThemeConfig(vuetify)
    };
}

export const APPLICATION_THEMES: Readonly<Record<ApplicationThemeName, ApplicationThemeDefinition>> = themeVariants.reduce((themes, variant) => {
    themes[variant.name] = createThemeDefinition(variant);
    return themes;
}, {} as Record<ApplicationThemeName, ApplicationThemeDefinition>);

export function isApplicationThemeName(value: string | undefined | null): value is ApplicationThemeName {
    return !!value && Object.prototype.hasOwnProperty.call(APPLICATION_THEMES, value);
}

export function normalizeThemePreference(value: string | undefined | null): ThemePreference {
    if (value === SYSTEM_THEME_PREFERENCE || isApplicationThemeName(value)) {
        return value;
    }

    return SYSTEM_THEME_PREFERENCE;
}

export function getThemeFamilyOptionValue(preference: string | undefined | null): ThemePreference {
    const normalizedPreference = normalizeThemePreference(preference);

    if (normalizedPreference === SYSTEM_THEME_PREFERENCE) {
        return SYSTEM_THEME_PREFERENCE;
    }

    const definition = APPLICATION_THEMES[normalizedPreference];
    return themePairByFamily[definition.family].light;
}

export function resolveThemePreference(preference: string | undefined | null, systemTheme: string = ThemeType.Light): ApplicationThemeName {
    const normalizedPreference = normalizeThemePreference(preference);

    if (normalizedPreference !== SYSTEM_THEME_PREFERENCE) {
        return normalizedPreference;
    }

    return systemTheme === ThemeType.Dark ? ThemeType.Dark : ThemeType.Light;
}

export function isDarkApplicationTheme(themeName: string | undefined | null): boolean {
    return isApplicationThemeName(themeName) ? APPLICATION_THEMES[themeName].dark : false;
}

export function getApplicationThemeDefinition(themeName: string | undefined | null): ApplicationThemeDefinition {
    const normalizedThemeName = isApplicationThemeName(themeName) ? themeName : ThemeType.Light;
    return APPLICATION_THEMES[normalizedThemeName];
}

export function getPairedApplicationThemeName(themeName: string | undefined | null, targetMode?: ApplicationThemeMode): ApplicationThemeName {
    const normalizedThemeName = normalizeThemePreference(themeName);
    const definition = getApplicationThemeDefinition(normalizedThemeName);
    const nextMode = targetMode ?? (definition.mode === 'dark' ? 'light' : 'dark');

    return themePairByFamily[definition.family][nextMode];
}

export function getVuetifyThemes(): Record<ApplicationThemeName, ApplicationVuetifyThemeDefinition> {
    return APPLICATION_THEME_ORDER.reduce((themes, themeName) => {
        themes[themeName] = APPLICATION_THEMES[themeName].vuetify;
        return themes;
    }, {} as Record<ApplicationThemeName, ApplicationVuetifyThemeDefinition>);
}

export function getMobileThemeConfig(themeName: string | undefined | null): ApplicationMobileThemeConfig {
    return getApplicationThemeDefinition(normalizeThemePreference(themeName)).mobile;
}

export function getThemePreferenceOptions(translate: (key: string) => string): ThemePreferenceOption[] {
    return [
        { name: translate('System Default'), value: SYSTEM_THEME_PREFERENCE },
        ...APPLICATION_THEME_FAMILY_OPTION_ORDER.map(themeName => {
            const definition = APPLICATION_THEMES[themeName];
            const familyName = translate(definition.displayKey);

            return {
                name: definition.family === 'classic' ? translate('Default') : familyName,
                value: themeName
            };
        })
    ];
}

export function getFramework7DarkModePreference(preference: string | undefined | null): boolean | 'auto' {
    const normalizedPreference = normalizeThemePreference(preference);

    if (normalizedPreference === SYSTEM_THEME_PREFERENCE) {
        return SYSTEM_THEME_PREFERENCE;
    }

    return isDarkApplicationTheme(normalizedPreference);
}

export function getNextQuickThemePreference(preference: string | undefined | null): ThemePreference {
    const normalizedPreference = normalizeThemePreference(preference);

    if (normalizedPreference === SYSTEM_THEME_PREFERENCE) {
        return ThemeType.Light;
    }

    return getPairedApplicationThemeName(normalizedPreference);
}
