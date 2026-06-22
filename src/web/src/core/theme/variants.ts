import { ThemeType } from './types.ts';
import type {
    ApplicationThemeFamily,
    ApplicationThemeMode,
    ApplicationThemeName,
    ApplicationThemeVariant
} from './types.ts';
import { darkTheme, lightTheme } from './base.ts';

export const themeVariants: readonly ApplicationThemeVariant[] = [
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

export const themePairByFamily: Readonly<Record<ApplicationThemeFamily, Readonly<Record<ApplicationThemeMode, ApplicationThemeName>>>> = {
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
