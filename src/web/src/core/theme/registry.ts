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

/**
 * 中文说明：合并基础 Vuetify 主题与品牌覆盖项，保证新增主题只覆盖差异 token。
 */
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

/**
 * 中文说明：把 Vuetify 主题 token 投影到 Framework7/mobile CSS variables，保持桌面和移动端配色同步。
 */
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

/**
 * 中文说明：由主题变体生成完整应用主题定义，集中补齐 paired theme、Vuetify 和移动端配置。
 */
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

/**
 * 中文说明：判断外部偏好值是否是已注册应用主题名，供设置读取和路由恢复时做白名单校验。
 */
export function isApplicationThemeName(value: string | undefined | null): value is ApplicationThemeName {
    return !!value && Object.prototype.hasOwnProperty.call(APPLICATION_THEMES, value);
}

/**
 * 中文说明：规范化主题偏好，非法值统一回落到 system，避免本地存储旧值破坏主题初始化。
 */
export function normalizeThemePreference(value: string | undefined | null): ThemePreference {
    if (value === SYSTEM_THEME_PREFERENCE || isApplicationThemeName(value)) {
        return value;
    }

    return SYSTEM_THEME_PREFERENCE;
}

/**
 * 中文说明：把当前偏好折叠到主题家族选项值，system 保持独立，其余值回到该家族 light 入口。
 */
export function getThemeFamilyOptionValue(preference: string | undefined | null): ThemePreference {
    const normalizedPreference = normalizeThemePreference(preference);

    if (normalizedPreference === SYSTEM_THEME_PREFERENCE) {
        return SYSTEM_THEME_PREFERENCE;
    }

    const definition = APPLICATION_THEMES[normalizedPreference];
    return themePairByFamily[definition.family].light;
}

/**
 * 中文说明：解析实际应用主题；system 偏好根据系统明暗模式落到默认 light/dark 主题。
 */
export function resolveThemePreference(preference: string | undefined | null, systemTheme: string = ThemeType.Light): ApplicationThemeName {
    const normalizedPreference = normalizeThemePreference(preference);

    if (normalizedPreference !== SYSTEM_THEME_PREFERENCE) {
        return normalizedPreference;
    }

    return systemTheme === ThemeType.Dark ? ThemeType.Dark : ThemeType.Light;
}

/**
 * 中文说明：判断主题是否为暗色模式，未知主题按浅色处理以避免移动端误切 dark class。
 */
export function isDarkApplicationTheme(themeName: string | undefined | null): boolean {
    return isApplicationThemeName(themeName) ? APPLICATION_THEMES[themeName].dark : false;
}

/**
 * 中文说明：读取完整主题定义，未知主题回退默认浅色定义，保护调用方不处理 undefined。
 */
export function getApplicationThemeDefinition(themeName: string | undefined | null): ApplicationThemeDefinition {
    const normalizedThemeName = isApplicationThemeName(themeName) ? themeName : ThemeType.Light;
    return APPLICATION_THEMES[normalizedThemeName];
}

/**
 * 中文说明：在同一主题家族内切换明暗配对主题，支撑快速切换和显式 targetMode。
 */
export function getPairedApplicationThemeName(themeName: string | undefined | null, targetMode?: ApplicationThemeMode): ApplicationThemeName {
    const normalizedThemeName = normalizeThemePreference(themeName);
    const definition = getApplicationThemeDefinition(normalizedThemeName);
    const nextMode = targetMode ?? (definition.mode === 'dark' ? 'light' : 'dark');

    return themePairByFamily[definition.family][nextMode];
}

/**
 * 中文说明：生成 Vuetify themes 注册表，保持注册顺序和主题定义集中来自 APPLICATION_THEMES。
 */
export function getVuetifyThemes(): Record<ApplicationThemeName, ApplicationVuetifyThemeDefinition> {
    return APPLICATION_THEME_ORDER.reduce((themes, themeName) => {
        themes[themeName] = APPLICATION_THEMES[themeName].vuetify;
        return themes;
    }, {} as Record<ApplicationThemeName, ApplicationVuetifyThemeDefinition>);
}

/**
 * 中文说明：读取移动端主题配置，先规范化偏好再返回 Framework7 需要的 CSS variables。
 */
export function getMobileThemeConfig(themeName: string | undefined | null): ApplicationMobileThemeConfig {
    return getApplicationThemeDefinition(normalizeThemePreference(themeName)).mobile;
}

/**
 * 中文说明：生成设置页主题家族选项，显示文案由调用方翻译函数提供。
 */
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

/**
 * 中文说明：把应用主题偏好转换为 Framework7 darkMode 配置，system 保持 auto。
 */
export function getFramework7DarkModePreference(preference: string | undefined | null): boolean | 'auto' {
    const normalizedPreference = normalizeThemePreference(preference);

    if (normalizedPreference === SYSTEM_THEME_PREFERENCE) {
        return SYSTEM_THEME_PREFERENCE;
    }

    return isDarkApplicationTheme(normalizedPreference);
}

/**
 * 中文说明：计算快速切换按钮的下一主题偏好，system 先落到默认浅色再按配对主题切换。
 */
export function getNextQuickThemePreference(preference: string | undefined | null): ThemePreference {
    const normalizedPreference = normalizeThemePreference(preference);

    if (normalizedPreference === SYSTEM_THEME_PREFERENCE) {
        return ThemeType.Light;
    }

    return getPairedApplicationThemeName(normalizedPreference);
}
