import {
    ThemeType,
    getApplicationThemeDefinition,
    resolveThemePreference
} from '@/core/theme.ts';

export interface ChartThemePalette {
    readonly primaryRgb: string;
    readonly onPrimary: string;
    readonly surface: string;
    readonly text: string;
    readonly muted: string;
    readonly grid: string;
    readonly border: string;
    readonly tooltipBackground: string;
    readonly tooltipText: string;
    readonly tooltipBorder: string;
    readonly inactive: string;
}

export interface ScrollableLegendThemeOptions {
    readonly mobile?: boolean;
    readonly maxTextWidth?: number;
    readonly fontSize?: number;
}

export function getChartThemePalette(themeName: string | undefined | null): ChartThemePalette {
    const semantic = getApplicationThemeDefinition(themeName).semantic;

    return {
        primaryRgb: semantic.primaryRgb,
        onPrimary: semantic.onPrimary,
        surface: semantic.surface,
        text: semantic.chartText,
        muted: semantic.chartMutedText,
        grid: semantic.chartGrid,
        border: semantic.border,
        tooltipBackground: semantic.tooltipBackground,
        tooltipText: semantic.tooltipText,
        tooltipBorder: semantic.tooltipBorder,
        inactive: semantic.legendInactive
    };
}

export function resolveMobileChartThemePalette(themePreference: string | undefined | null, darkMode: boolean): ChartThemePalette {
    const themeName = resolveThemePreference(themePreference, darkMode ? ThemeType.Dark : ThemeType.Light);
    return getChartThemePalette(themeName);
}

export function truncateChartLabel(value: string, maxLength = 22): string {
    const characters = Array.from(value);
    if (characters.length <= maxLength) {
        return value;
    }
    return characters.slice(0, maxLength).join('') + '…';
}

export function createScrollableLegendTheme(
    palette: ChartThemePalette,
    options: ScrollableLegendThemeOptions = {}
): Record<string, unknown> {
    const mobile = options.mobile ?? false;
    const maxTextWidth = options.maxTextWidth ?? (mobile ? 124 : 180);
    const fontSize = options.fontSize ?? (mobile ? 11 : 12);

    return {
        type: 'scroll',
        left: 4,
        right: 4,
        tooltip: { show: true },
        itemGap: mobile ? 8 : 12,
        textStyle: {
            color: palette.text,
            fontSize,
            width: maxTextWidth,
            overflow: 'truncate',
            ellipsis: '…'
        },
        pageIconColor: palette.muted,
        pageIconInactiveColor: palette.inactive,
        pageTextStyle: {
            color: palette.text,
            fontSize
        }
    };
}
