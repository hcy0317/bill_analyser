import { describe, expect, test } from '@jest/globals';

import { ThemeType } from '@/core/theme.ts';
import {
    createScrollableLegendTheme,
    getChartThemePalette,
    resolveMobileChartThemePalette,
    truncateChartLabel
} from '@/lib/chartTheme.ts';

describe('chart theme contract', () => {
    test('resolves every explicit and system mobile theme through the application registry', () => {
        expect(getChartThemePalette(ThemeType.DraculaDark)).toMatchObject({
            text: '#f8f8f2',
            surface: '#343746',
            primaryRgb: '189, 147, 249'
        });
        expect(resolveMobileChartThemePalette(ThemeType.ForestLight, true).text).toBe('#122217');
        expect(resolveMobileChartThemePalette('auto', true).text).toBe('#fcf0e3');
        expect(resolveMobileChartThemePalette('auto', false).text).toBe('#413935');
    });

    test('builds bounded scroll legends with theme-aware paging and tooltip', () => {
        const palette = getChartThemePalette(ThemeType.BusinessDark);
        const legend = createScrollableLegendTheme(palette, { mobile: true, maxTextWidth: 124, fontSize: 11 });

        expect(legend).toMatchObject({
            type: 'scroll',
            left: 4,
            right: 4,
            tooltip: { show: true },
            textStyle: {
                color: palette.text,
                width: 124,
                overflow: 'truncate',
                ellipsis: '…'
            },
            pageIconColor: palette.muted,
            pageTextStyle: { color: palette.text }
        });
    });

    test('truncates by Unicode code points while retaining short labels', () => {
        expect(truncateChartLabel('日常支出', 8)).toBe('日常支出');
        expect(truncateChartLabel('非常非常非常长的分类图例名称', 8)).toBe('非常非常非常长的…');
        expect(truncateChartLabel('emoji-😀-legend', 8)).toBe('emoji-😀-…');
    });
});
