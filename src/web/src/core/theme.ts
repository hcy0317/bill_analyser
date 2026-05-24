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

type ApplicationThemeOverride = Partial<ApplicationVuetifyThemeDefinition> & {
    readonly colors?: Record<string, string>;
    readonly variables?: Record<string, string | number>;
};

interface ApplicationThemeVariant {
    readonly name: ApplicationThemeName;
    readonly displayKey: string;
    readonly family: ApplicationThemeFamily;
    readonly mode: ApplicationThemeMode;
    readonly override?: ApplicationThemeOverride;
}

const lightGreys = {
    'grey': '#8c8c8c',
    'grey-50': '#fafafa',
    'grey-100': '#f0f2f8',
    'grey-200': '#eeeeee',
    'grey-300': '#e0e0e0',
    'grey-400': '#bdbdbd',
    'grey-500': '#9e9e9e',
    'grey-600': '#757575',
    'grey-700': '#616161',
    'grey-800': '#424242',
    'grey-900': '#212121'
};

const darkGreys = {
    'grey': '#4d4c4b',
    'grey-50': '#212121',
    'grey-100': '#424242',
    'grey-200': '#616161',
    'grey-300': '#757575',
    'grey-400': '#909090',
    'grey-500': '#a2a2a2',
    'grey-600': '#b4b4b4',
    'grey-700': '#c6c6c6',
    'grey-800': '#d8d8d8',
    'grey-900': '#eaeaea'
};

const lightVuetifyTheme: ApplicationVuetifyThemeDefinition = {
    dark: false,
    colors: {
        'primary': '#c67e48',
        'primary-darken-1': '#b67443',
        'on-primary': '#ffffff',
        'secondary': '#8c8c8c',
        'secondary-darken-1': '#595754',
        'on-secondary': '#ffffff',
        'success': '#4cd964',
        'success-darken-1': '#40b654',
        'on-success': '#0f2e16',
        'info': '#2196f3',
        'info-darken-1': '#1e85d7',
        'on-info': '#ffffff',
        'warning': '#ff9500',
        'warning-darken-1': '#de8201',
        'on-warning': '#2f1a00',
        'error': '#ff3b30',
        'error-darken-1': '#e1342b',
        'on-error': '#ffffff',
        'teal': '#009688',
        'background': '#faf8f4',
        'on-background': '#413935',
        'surface': '#ffffff',
        'on-surface': '#413935',
        'notification-background': '#ffffff',
        'on-notification-background': '#000000',
        ...lightGreys,
        'perfect-scrollbar-thumb': '#dedcda',
        'skin-bordered-background': '#ffffff',
        'skin-bordered-surface': '#ffffff',
        'expansion-panel-text-custom-bg': '#fafafa',
        'table-row-striped': '#f7f2ec',
        'table-row-hover': '#f1e6db'
    },
    variables: {
        'code-color': '#ff8000',
        'overlay-scrim-background': '#413935',
        'tooltip-background': '#212121',
        'tooltip-color': '#ffffff',
        'overlay-scrim-opacity': 0.5,
        'hover-opacity': 0.04,
        'focus-opacity': 0.1,
        'selected-opacity': 0.08,
        'activated-opacity': 0.16,
        'pressed-opacity': 0.14,
        'dragged-opacity': 0.1,
        'disabled-opacity': 0.4,
        'border-color': '#413f3b',
        'border-opacity': 0.12,
        'table-header-color': '#fdfcf9',
        'high-emphasis-opacity': 0.9,
        'medium-emphasis-opacity': 0.7,
        'shadow-key-umbra-color': '#413935',
        'shadow-xs-opacity': '0.16',
        'shadow-sm-opacity': '0.18',
        'shadow-md-opacity': '0.20',
        'shadow-lg-opacity': '0.22',
        'shadow-xl-opacity': '0.24',
    }
};

const darkVuetifyTheme: ApplicationVuetifyThemeDefinition = {
    dark: true,
    colors: {
        'primary': '#c67e48',
        'primary-darken-1': '#b67443',
        'on-primary': '#ffffff',
        'secondary': '#9d9b99',
        'secondary-darken-1': '#3e3d3c',
        'on-secondary': '#151312',
        'success': '#4cd964',
        'success-darken-1': '#40b654',
        'on-success': '#08240f',
        'info': '#2196f3',
        'info-darken-1': '#1e85d7',
        'on-info': '#ffffff',
        'warning': '#ff9500',
        'warning-darken-1': '#de8201',
        'on-warning': '#2f1a00',
        'error': '#ff3b30',
        'error-darken-1': '#e1342b',
        'on-error': '#ffffff',
        'teal': '#009688',
        'background': '#060504',
        'on-background': '#fcf0e3',
        'surface': '#1a1a1a',
        'on-surface': '#fcf0e3',
        'notification-background': '#1e1e1e',
        'on-notification-background': '#ffffff',
        ...darkGreys,
        'perfect-scrollbar-thumb': '#725b4a',
        'skin-bordered-background': '#4b3b2d',
        'skin-bordered-surface': '#4b3b2d',
        'expansion-panel-text-custom-bg': '#503f33',
        'table-row-striped': '#242322',
        'table-row-hover': '#2c241e'
    },
    variables: {
        'code-color': '#ff8000',
        'overlay-scrim-background': '#1a1a1a',
        'tooltip-background': '#333333',
        'tooltip-color': '#eeeeee',
        'overlay-scrim-opacity': 0.6,
        'hover-opacity': 0.04,
        'focus-opacity': 0.1,
        'selected-opacity': 0.08,
        'activated-opacity': 0.16,
        'pressed-opacity': 0.14,
        'disabled-opacity': 0.4,
        'dragged-opacity': 0.1,
        'border-color': '#edece9',
        'border-opacity': 0.12,
        'table-header-color': '#242322',
        'high-emphasis-opacity': 0.9,
        'medium-emphasis-opacity': 0.7,
        'shadow-key-umbra-color': '#383736',
        'shadow-xs-opacity': '0.20',
        'shadow-sm-opacity': '0.22',
        'shadow-md-opacity': '0.24',
        'shadow-lg-opacity': '0.26',
        'shadow-xl-opacity': '0.28',
    }
};

const lightTheme = (colors: Record<string, string>, variables: Record<string, string | number> = {}): ApplicationThemeOverride => ({
    dark: false,
    colors,
    variables
});

const darkTheme = (colors: Record<string, string>, variables: Record<string, string | number> = {}): ApplicationThemeOverride => ({
    dark: true,
    colors,
    variables
});

const themeVariants: readonly ApplicationThemeVariant[] = [
    { name: ThemeType.Light, displayKey: 'Light', family: 'classic', mode: 'light' },
    { name: ThemeType.Dark, displayKey: 'Dark', family: 'classic', mode: 'dark' },
    {
        name: ThemeType.HalloweenLight,
        displayKey: 'Halloween',
        family: 'halloween',
        mode: 'light',
        override: lightTheme({
            'primary': '#f97316', 'primary-darken-1': '#ea580c', 'on-primary': '#251000',
            'secondary': '#7c3aed', 'secondary-darken-1': '#6d28d9', 'on-secondary': '#ffffff',
            'warning': '#ea580c', 'warning-darken-1': '#c2410c', 'on-warning': '#ffffff',
            'background': '#fff7ed', 'on-background': '#2b1604',
            'surface': '#fffdf8', 'on-surface': '#2b1604',
            'notification-background': '#fff9f0', 'on-notification-background': '#2b1604',
            'perfect-scrollbar-thumb': '#f4b36d',
            'skin-bordered-background': '#fffaf2', 'skin-bordered-surface': '#fffaf2',
            'expansion-panel-text-custom-bg': '#ffedd5',
            'table-row-striped': '#fff0dc', 'table-row-hover': '#ffe2bd',
            'grey': '#8a6f5a'
        }, { 'code-color': '#c2410c', 'table-header-color': '#fff2df' })
    },
    {
        name: ThemeType.HalloweenDark,
        displayKey: 'Halloween',
        family: 'halloween',
        mode: 'dark',
        override: darkTheme({
            'primary': '#ff8c00', 'primary-darken-1': '#d97000', 'on-primary': '#1f1200',
            'secondary': '#8b5cf6', 'secondary-darken-1': '#6d28d9', 'on-secondary': '#ffffff',
            'warning': '#f97316', 'warning-darken-1': '#c2410c', 'on-warning': '#1f1200',
            'background': '#120b16', 'on-background': '#fff3e6',
            'surface': '#1d1326', 'on-surface': '#fff3e6',
            'notification-background': '#271733',
            'perfect-scrollbar-thumb': '#8b5a2b',
            'skin-bordered-background': '#24172d', 'skin-bordered-surface': '#24172d',
            'expansion-panel-text-custom-bg': '#2d1a39',
            'table-row-striped': '#25162f', 'table-row-hover': '#321b40',
            'grey': '#6f6175'
        }, { 'code-color': '#ffb86c', 'table-header-color': '#1a1021' })
    },
    {
        name: ThemeType.ForestLight,
        displayKey: 'Forest',
        family: 'forest',
        mode: 'light',
        override: lightTheme({
            'primary': '#15803d', 'primary-darken-1': '#166534', 'on-primary': '#ffffff',
            'secondary': '#73805f', 'secondary-darken-1': '#566146', 'on-secondary': '#ffffff',
            'success': '#16a34a', 'success-darken-1': '#15803d', 'on-success': '#ffffff',
            'background': '#f3fbf2', 'on-background': '#122217',
            'surface': '#ffffff', 'on-surface': '#122217',
            'notification-background': '#fbfff9', 'on-notification-background': '#122217',
            'perfect-scrollbar-thumb': '#9bc8a5',
            'skin-bordered-background': '#ffffff', 'skin-bordered-surface': '#ffffff',
            'expansion-panel-text-custom-bg': '#e8f4e7',
            'table-row-striped': '#edf8ec', 'table-row-hover': '#dff0dc',
            'grey': '#657568'
        }, { 'code-color': '#3f6212', 'table-header-color': '#f2fbf1' })
    },
    {
        name: ThemeType.ForestDark,
        displayKey: 'Forest',
        family: 'forest',
        mode: 'dark',
        override: darkTheme({
            'primary': '#4ade80', 'primary-darken-1': '#22c55e', 'on-primary': '#052e16',
            'secondary': '#a3a380', 'secondary-darken-1': '#7c7c5e', 'on-secondary': '#1f2418',
            'success': '#86efac', 'success-darken-1': '#4ade80', 'on-success': '#052e16',
            'background': '#07150f', 'on-background': '#ecfdf5',
            'surface': '#10251b', 'on-surface': '#ecfdf5',
            'notification-background': '#143522',
            'perfect-scrollbar-thumb': '#315c43',
            'skin-bordered-background': '#183322', 'skin-bordered-surface': '#183322',
            'expansion-panel-text-custom-bg': '#1d3d2a',
            'table-row-striped': '#142c20', 'table-row-hover': '#1b3a2a',
            'grey': '#607568'
        }, { 'code-color': '#a3e635', 'table-header-color': '#0d1f16' })
    },
    {
        name: ThemeType.WireframeLight,
        displayKey: 'Wireframe',
        family: 'wireframe',
        mode: 'light',
        override: lightTheme({
            'primary': '#111827', 'primary-darken-1': '#000000', 'on-primary': '#ffffff',
            'secondary': '#6b7280', 'secondary-darken-1': '#374151', 'on-secondary': '#ffffff',
            'success': '#047857', 'success-darken-1': '#065f46', 'on-success': '#ffffff',
            'info': '#2563eb', 'info-darken-1': '#1d4ed8',
            'warning': '#b45309', 'warning-darken-1': '#92400e', 'on-warning': '#ffffff',
            'error': '#b91c1c', 'error-darken-1': '#991b1b',
            'background': '#f8fafc', 'on-background': '#111827',
            'surface': '#ffffff', 'on-surface': '#111827',
            'notification-background': '#ffffff', 'on-notification-background': '#111827',
            'perfect-scrollbar-thumb': '#cbd5e1',
            'skin-bordered-background': '#ffffff', 'skin-bordered-surface': '#ffffff',
            'expansion-panel-text-custom-bg': '#f1f5f9',
            'table-row-striped': '#f1f5f9', 'table-row-hover': '#e2e8f0',
            'grey': '#6b7280'
        }, {
            'code-color': '#111827',
            'border-color': '#111827',
            'border-opacity': 0.24,
            'table-header-color': '#f8fafc',
            'shadow-xs-opacity': '0.08',
            'shadow-sm-opacity': '0.10',
            'shadow-md-opacity': '0.12',
            'shadow-lg-opacity': '0.14',
            'shadow-xl-opacity': '0.16'
        })
    },
    {
        name: ThemeType.WireframeDark,
        displayKey: 'Wireframe',
        family: 'wireframe',
        mode: 'dark',
        override: darkTheme({
            'primary': '#f8fafc', 'primary-darken-1': '#e2e8f0', 'on-primary': '#111827',
            'secondary': '#94a3b8', 'secondary-darken-1': '#64748b', 'on-secondary': '#111827',
            'background': '#0b1120', 'on-background': '#f8fafc',
            'surface': '#111827', 'on-surface': '#f8fafc',
            'notification-background': '#151d2f',
            'skin-bordered-background': '#1f2937', 'skin-bordered-surface': '#1f2937',
            'expansion-panel-text-custom-bg': '#1f2937',
            'perfect-scrollbar-thumb': '#475569',
            'table-row-striped': '#182235', 'table-row-hover': '#243047',
            'grey': '#94a3b8'
        }, {
            'code-color': '#e2e8f0',
            'border-color': '#f8fafc',
            'border-opacity': 0.26,
            'table-header-color': '#111827',
            'shadow-xs-opacity': '0.24',
            'shadow-sm-opacity': '0.26',
            'shadow-md-opacity': '0.28',
            'shadow-lg-opacity': '0.30',
            'shadow-xl-opacity': '0.32'
        })
    },
    {
        name: ThemeType.BlackLight,
        displayKey: 'Black',
        family: 'black',
        mode: 'light',
        override: lightTheme({
            'primary': '#111111', 'primary-darken-1': '#000000', 'on-primary': '#ffffff',
            'secondary': '#4b5563', 'secondary-darken-1': '#1f2937', 'on-secondary': '#ffffff',
            'background': '#f5f5f5', 'on-background': '#111111',
            'surface': '#ffffff', 'on-surface': '#111111',
            'notification-background': '#ffffff', 'on-notification-background': '#111111',
            'perfect-scrollbar-thumb': '#b8b8b8',
            'skin-bordered-background': '#ffffff', 'skin-bordered-surface': '#ffffff',
            'expansion-panel-text-custom-bg': '#eeeeee',
            'table-row-striped': '#eeeeee', 'table-row-hover': '#e2e2e2',
            'grey': '#666666'
        }, { 'code-color': '#111111', 'table-header-color': '#f5f5f5' })
    },
    {
        name: ThemeType.BlackDark,
        displayKey: 'Black',
        family: 'black',
        mode: 'dark',
        override: darkTheme({
            'primary': '#e5e7eb', 'primary-darken-1': '#cbd5e1', 'on-primary': '#000000',
            'secondary': '#94a3b8', 'secondary-darken-1': '#64748b', 'on-secondary': '#000000',
            'background': '#000000', 'on-background': '#f8fafc',
            'surface': '#070707', 'on-surface': '#f8fafc',
            'notification-background': '#0a0a0a',
            'perfect-scrollbar-thumb': '#333333',
            'skin-bordered-background': '#0f0f0f', 'skin-bordered-surface': '#0f0f0f',
            'expansion-panel-text-custom-bg': '#101010',
            'table-row-striped': '#111111', 'table-row-hover': '#191919',
            'grey': '#666666'
        }, {
            'code-color': '#facc15',
            'table-header-color': '#080808',
            'shadow-xs-opacity': '0.30',
            'shadow-sm-opacity': '0.32',
            'shadow-md-opacity': '0.34',
            'shadow-lg-opacity': '0.36',
            'shadow-xl-opacity': '0.38'
        })
    },
    {
        name: ThemeType.DraculaLight,
        displayKey: 'Dracula',
        family: 'dracula',
        mode: 'light',
        override: lightTheme({
            'primary': '#7c3aed', 'primary-darken-1': '#6d28d9', 'on-primary': '#ffffff',
            'secondary': '#c026d3', 'secondary-darken-1': '#a21caf', 'on-secondary': '#ffffff',
            'success': '#16a34a', 'success-darken-1': '#15803d', 'on-success': '#ffffff',
            'info': '#0284c7', 'info-darken-1': '#0369a1',
            'warning': '#ca8a04', 'warning-darken-1': '#a16207', 'on-warning': '#1f1200',
            'error': '#dc2626', 'error-darken-1': '#b91c1c',
            'background': '#fbf8ff', 'on-background': '#2b2435',
            'surface': '#ffffff', 'on-surface': '#2b2435',
            'notification-background': '#ffffff', 'on-notification-background': '#2b2435',
            'perfect-scrollbar-thumb': '#c4b5fd',
            'skin-bordered-background': '#ffffff', 'skin-bordered-surface': '#ffffff',
            'expansion-panel-text-custom-bg': '#f1ecff',
            'table-row-striped': '#f3edff', 'table-row-hover': '#e9ddff',
            'grey': '#7d748b'
        }, { 'code-color': '#c026d3', 'table-header-color': '#faf7ff' })
    },
    {
        name: ThemeType.DraculaDark,
        displayKey: 'Dracula',
        family: 'dracula',
        mode: 'dark',
        override: darkTheme({
            'primary': '#bd93f9', 'primary-darken-1': '#9d6ff2', 'on-primary': '#282a36',
            'secondary': '#ff79c6', 'secondary-darken-1': '#df5aa9', 'on-secondary': '#282a36',
            'success': '#50fa7b', 'success-darken-1': '#2ed85b', 'on-success': '#173b22',
            'info': '#8be9fd', 'info-darken-1': '#5ad8f4', 'on-info': '#102830',
            'warning': '#f1fa8c', 'warning-darken-1': '#d7e65c', 'on-warning': '#282a36',
            'error': '#ff5555', 'error-darken-1': '#e03e3e',
            'background': '#282a36', 'on-background': '#f8f8f2',
            'surface': '#343746', 'on-surface': '#f8f8f2',
            'notification-background': '#44475a',
            'perfect-scrollbar-thumb': '#6272a4',
            'skin-bordered-background': '#343746', 'skin-bordered-surface': '#343746',
            'expansion-panel-text-custom-bg': '#3e4255',
            'table-row-striped': '#3b3f51', 'table-row-hover': '#454a60',
            'grey': '#6272a4'
        }, { 'code-color': '#ff79c6', 'table-header-color': '#2f3140' })
    },
    {
        name: ThemeType.BusinessLight,
        displayKey: 'Business',
        family: 'business',
        mode: 'light',
        override: lightTheme({
            'primary': '#2563eb', 'primary-darken-1': '#1d4ed8', 'on-primary': '#ffffff',
            'secondary': '#64748b', 'secondary-darken-1': '#475569', 'on-secondary': '#ffffff',
            'info': '#0284c7', 'info-darken-1': '#0369a1',
            'background': '#f8fafc', 'on-background': '#0f172a',
            'surface': '#ffffff', 'on-surface': '#0f172a',
            'notification-background': '#ffffff', 'on-notification-background': '#0f172a',
            'perfect-scrollbar-thumb': '#cbd5e1',
            'skin-bordered-background': '#ffffff', 'skin-bordered-surface': '#ffffff',
            'expansion-panel-text-custom-bg': '#eef4fb',
            'table-row-striped': '#f1f5f9', 'table-row-hover': '#e5edf7',
            'grey': '#64748b'
        }, { 'code-color': '#2563eb', 'table-header-color': '#f8fafc' })
    },
    {
        name: ThemeType.BusinessDark,
        displayKey: 'Business',
        family: 'business',
        mode: 'dark',
        override: darkTheme({
            'primary': '#60a5fa', 'primary-darken-1': '#3b82f6', 'on-primary': '#0f172a',
            'secondary': '#94a3b8', 'secondary-darken-1': '#64748b', 'on-secondary': '#0f172a',
            'info': '#38bdf8', 'info-darken-1': '#0ea5e9', 'on-info': '#082f49',
            'background': '#0f172a', 'on-background': '#e2e8f0',
            'surface': '#1e293b', 'on-surface': '#e2e8f0',
            'notification-background': '#162033',
            'perfect-scrollbar-thumb': '#475569',
            'skin-bordered-background': '#1e293b', 'skin-bordered-surface': '#1e293b',
            'expansion-panel-text-custom-bg': '#243449',
            'table-row-striped': '#24324a', 'table-row-hover': '#2b3b57',
            'grey': '#64748b'
        }, { 'code-color': '#93c5fd', 'table-header-color': '#162033' })
    },
    {
        name: ThemeType.NightLight,
        displayKey: 'Night',
        family: 'night',
        mode: 'light',
        override: lightTheme({
            'primary': '#0369a1', 'primary-darken-1': '#075985', 'on-primary': '#ffffff',
            'secondary': '#4f46e5', 'secondary-darken-1': '#4338ca', 'on-secondary': '#ffffff',
            'background': '#f0f9ff', 'on-background': '#0f172a',
            'surface': '#ffffff', 'on-surface': '#0f172a',
            'notification-background': '#ffffff', 'on-notification-background': '#0f172a',
            'perfect-scrollbar-thumb': '#bae6fd',
            'skin-bordered-background': '#ffffff', 'skin-bordered-surface': '#ffffff',
            'expansion-panel-text-custom-bg': '#e0f2fe',
            'table-row-striped': '#e8f6ff', 'table-row-hover': '#d7efff',
            'grey': '#5b728a'
        }, { 'code-color': '#0369a1', 'table-header-color': '#f1faff' })
    },
    {
        name: ThemeType.NightDark,
        displayKey: 'Night',
        family: 'night',
        mode: 'dark',
        override: darkTheme({
            'primary': '#38bdf8', 'primary-darken-1': '#0ea5e9', 'on-primary': '#03111f',
            'secondary': '#818cf8', 'secondary-darken-1': '#6366f1', 'on-secondary': '#07111f',
            'background': '#07111f', 'on-background': '#e0f2fe',
            'surface': '#0f1d33', 'on-surface': '#e0f2fe',
            'notification-background': '#10233d',
            'perfect-scrollbar-thumb': '#1f4f78',
            'skin-bordered-background': '#11253f', 'skin-bordered-surface': '#11253f',
            'expansion-panel-text-custom-bg': '#14304f',
            'table-row-striped': '#13263f', 'table-row-hover': '#193251',
            'grey': '#5b728a'
        }, { 'code-color': '#7dd3fc', 'table-header-color': '#0b1828' })
    },
    {
        name: ThemeType.DimLight,
        displayKey: 'Dim',
        family: 'dim',
        mode: 'light',
        override: lightTheme({
            'primary': '#a16207', 'primary-darken-1': '#854d0e', 'on-primary': '#ffffff',
            'secondary': '#6b7280', 'secondary-darken-1': '#4b5563', 'on-secondary': '#ffffff',
            'background': '#f5f1eb', 'on-background': '#3f3832',
            'surface': '#fffaf5', 'on-surface': '#3f3832',
            'notification-background': '#fffaf5', 'on-notification-background': '#3f3832',
            'perfect-scrollbar-thumb': '#c8b8a8',
            'skin-bordered-background': '#fffaf5', 'skin-bordered-surface': '#fffaf5',
            'expansion-panel-text-custom-bg': '#eee4da',
            'table-row-striped': '#efe7dd', 'table-row-hover': '#e5d8ca',
            'grey': '#77716d'
        }, {
            'code-color': '#a16207',
            'table-header-color': '#f6eee5',
            'high-emphasis-opacity': 0.88,
            'medium-emphasis-opacity': 0.68
        })
    },
    {
        name: ThemeType.DimDark,
        displayKey: 'Dim',
        family: 'dim',
        mode: 'dark',
        override: darkTheme({
            'primary': '#d59a6f', 'primary-darken-1': '#b9825b', 'on-primary': '#1f1f23',
            'secondary': '#9ca3af', 'secondary-darken-1': '#6b7280', 'on-secondary': '#1f1f23',
            'background': '#1f1f23', 'on-background': '#e4dfd8',
            'surface': '#2a2a2f', 'on-surface': '#e4dfd8',
            'notification-background': '#303036',
            'perfect-scrollbar-thumb': '#5a514c',
            'skin-bordered-background': '#333036', 'skin-bordered-surface': '#333036',
            'expansion-panel-text-custom-bg': '#37333a',
            'table-row-striped': '#303036', 'table-row-hover': '#393940',
            'grey': '#77716d'
        }, {
            'code-color': '#f3b37c',
            'table-header-color': '#25252a',
            'high-emphasis-opacity': 0.86,
            'medium-emphasis-opacity': 0.66
        })
    }
];

const themePairByFamily: Readonly<Record<ApplicationThemeFamily, Readonly<Record<ApplicationThemeMode, ApplicationThemeName>>>> = {
    classic: { light: ThemeType.Light, dark: ThemeType.Dark },
    halloween: { light: ThemeType.HalloweenLight, dark: ThemeType.HalloweenDark },
    forest: { light: ThemeType.ForestLight, dark: ThemeType.ForestDark },
    wireframe: { light: ThemeType.WireframeLight, dark: ThemeType.WireframeDark },
    black: { light: ThemeType.BlackLight, dark: ThemeType.BlackDark },
    dracula: { light: ThemeType.DraculaLight, dark: ThemeType.DraculaDark },
    business: { light: ThemeType.BusinessLight, dark: ThemeType.BusinessDark },
    night: { light: ThemeType.NightLight, dark: ThemeType.NightDark },
    dim: { light: ThemeType.DimLight, dark: ThemeType.DimDark }
};

const legacyThemeAliases: Readonly<Record<string, ApplicationThemeName>> = {
    halloween: ThemeType.HalloweenDark,
    forest: ThemeType.ForestDark,
    wireframe: ThemeType.WireframeLight,
    black: ThemeType.BlackDark,
    dracula: ThemeType.DraculaDark,
    business: ThemeType.BusinessDark,
    night: ThemeType.NightDark,
    dim: ThemeType.DimDark
};

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

    if (value && legacyThemeAliases[value]) {
        return legacyThemeAliases[value];
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
