import { describe, expect, test } from '@jest/globals';

import {
    APPLICATION_THEME_ORDER,
    APPLICATION_THEMES,
    SYSTEM_THEME_PREFERENCE,
    ThemeType,
    getFramework7DarkModePreference,
    getThemeFamilyOptionValue,
    getNextQuickThemePreference,
    getPairedApplicationThemeName,
    getThemePreferenceOptions,
    getVuetifyThemes,
    isDarkApplicationTheme,
    normalizeThemePreference,
    resolveThemePreference
} from '@/core/theme.ts';

const requiredColorTokens = [
    'primary',
    'primary-darken-1',
    'on-primary',
    'secondary',
    'secondary-darken-1',
    'on-secondary',
    'success',
    'success-darken-1',
    'on-success',
    'info',
    'info-darken-1',
    'on-info',
    'warning',
    'warning-darken-1',
    'on-warning',
    'error',
    'error-darken-1',
    'on-error',
    'background',
    'on-background',
    'surface',
    'on-surface',
    'notification-background',
    'on-notification-background',
    'grey',
    'grey-50',
    'grey-100',
    'grey-200',
    'grey-300',
    'grey-400',
    'grey-500',
    'grey-600',
    'grey-700',
    'grey-800',
    'grey-900',
    'perfect-scrollbar-thumb',
    'skin-bordered-background',
    'skin-bordered-surface',
    'expansion-panel-text-custom-bg',
    'table-row-striped',
    'table-row-hover'
] as const;

const requiredVariableTokens = [
    'code-color',
    'overlay-scrim-background',
    'tooltip-background',
    'tooltip-color',
    'hover-opacity',
    'focus-opacity',
    'selected-opacity',
    'activated-opacity',
    'pressed-opacity',
    'disabled-opacity',
    'border-color',
    'border-opacity',
    'table-header-color',
    'high-emphasis-opacity',
    'medium-emphasis-opacity'
] as const;

describe('application theme registry', () => {
    test('theme order exposes compatible defaults and requested paired classic themes', () => {
        expect(APPLICATION_THEME_ORDER).toEqual([
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
        ]);
        expect(new Set(APPLICATION_THEME_ORDER).size).toBe(APPLICATION_THEME_ORDER.length);
    });

    test('theme preference options are generated from the registry', () => {
        const options = getThemePreferenceOptions(key => `t:${key}`);

        expect(options.map(option => option.value)).toEqual([
            SYSTEM_THEME_PREFERENCE,
            ThemeType.Light,
            ThemeType.HalloweenLight,
            ThemeType.ForestLight,
            ThemeType.WireframeLight,
            ThemeType.BlackLight,
            ThemeType.DraculaLight,
            ThemeType.BusinessLight,
            ThemeType.NightLight,
            ThemeType.DimLight
        ]);
        expect(options[0]).toEqual({ name: 't:System Default', value: SYSTEM_THEME_PREFERENCE });
        expect(options).toContainEqual({ name: 't:Halloween', value: ThemeType.HalloweenLight });
        expect(options).not.toContainEqual({ name: 't:Halloween / t:Dark', value: ThemeType.HalloweenDark });
        expect(options.at(-1)).toEqual({ name: 't:Dim', value: ThemeType.DimLight });
    });

    test('theme preference resolver keeps auto compatible and falls back safely', () => {
        expect(resolveThemePreference(SYSTEM_THEME_PREFERENCE, ThemeType.Light)).toBe(ThemeType.Light);
        expect(resolveThemePreference(SYSTEM_THEME_PREFERENCE, ThemeType.Dark)).toBe(ThemeType.Dark);
        expect(resolveThemePreference(ThemeType.DraculaDark, ThemeType.Light)).toBe(ThemeType.DraculaDark);
        expect(normalizeThemePreference('dracula')).toBe(ThemeType.DraculaDark);
        expect(resolveThemePreference('wireframe', ThemeType.Dark)).toBe(ThemeType.WireframeLight);
        expect(normalizeThemePreference('unknown-theme')).toBe(SYSTEM_THEME_PREFERENCE);
        expect(resolveThemePreference('unknown-theme', ThemeType.Dark)).toBe(ThemeType.Dark);
    });

    test('dark-like presets report dark mode for charts and mobile adapters', () => {
        expect(isDarkApplicationTheme(ThemeType.Light)).toBe(false);
        expect(isDarkApplicationTheme(ThemeType.WireframeLight)).toBe(false);

        for (const themeName of [
            ThemeType.Dark,
            ThemeType.HalloweenDark,
            ThemeType.ForestDark,
            ThemeType.WireframeDark,
            ThemeType.BlackDark,
            ThemeType.DraculaDark,
            ThemeType.BusinessDark,
            ThemeType.NightDark,
            ThemeType.DimDark
        ]) {
            expect(isDarkApplicationTheme(themeName)).toBe(true);
            expect(getFramework7DarkModePreference(themeName)).toBe(true);
        }

        expect(getFramework7DarkModePreference(SYSTEM_THEME_PREFERENCE)).toBe(SYSTEM_THEME_PREFERENCE);
    });

    test('quick light dark switch keeps the selected theme family paired', () => {
        expect(getPairedApplicationThemeName(ThemeType.HalloweenLight)).toBe(ThemeType.HalloweenDark);
        expect(getPairedApplicationThemeName(ThemeType.HalloweenDark)).toBe(ThemeType.HalloweenLight);
        expect(getNextQuickThemePreference(ThemeType.ForestLight)).toBe(ThemeType.ForestDark);
        expect(getNextQuickThemePreference(ThemeType.ForestDark)).toBe(ThemeType.ForestLight);
        expect(getNextQuickThemePreference(ThemeType.Light)).toBe(ThemeType.Dark);
        expect(getNextQuickThemePreference(ThemeType.Dark)).toBe(ThemeType.Light);
        expect(getNextQuickThemePreference('halloween')).toBe(ThemeType.HalloweenLight);
    });

    test('theme family option value maps explicit light and dark variants to one setting item', () => {
        expect(getThemeFamilyOptionValue(ThemeType.Dark)).toBe(ThemeType.Light);
        expect(getThemeFamilyOptionValue(ThemeType.HalloweenDark)).toBe(ThemeType.HalloweenLight);
        expect(getThemeFamilyOptionValue(ThemeType.ForestLight)).toBe(ThemeType.ForestLight);
        expect(getThemeFamilyOptionValue(SYSTEM_THEME_PREFERENCE)).toBe(SYSTEM_THEME_PREFERENCE);
    });

    test('vuetify theme adapter emits complete token sets', () => {
        const vuetifyThemes = getVuetifyThemes();

        for (const themeName of APPLICATION_THEME_ORDER) {
            const vuetifyTheme = vuetifyThemes[themeName];

            for (const token of requiredColorTokens) {
                expect(vuetifyTheme.colors[token]).toEqual(expect.any(String));
            }

            for (const token of requiredVariableTokens) {
                expect(vuetifyTheme.variables[token]).toBeDefined();
            }

            expect(APPLICATION_THEMES[themeName].mobile.primary).toBe(vuetifyTheme.colors['primary']);
            expect(APPLICATION_THEMES[themeName].mobile.cssVariables['--f7-theme-color']).toBe(vuetifyTheme.colors['primary']);
        }
    });

    test('light and dark keep existing brand-compatible anchor colors', () => {
        const vuetifyThemes = getVuetifyThemes();

        expect(vuetifyThemes.light.colors['primary']).toBe('#c67e48');
        expect(vuetifyThemes.light.colors['background']).toBe('#faf8f4');
        expect(vuetifyThemes.dark.colors['primary']).toBe('#c67e48');
        expect(vuetifyThemes.dark.colors['background']).toBe('#060504');
        expect(vuetifyThemes.dark.colors['table-row-striped']).toBe('#242322');
    });
});
