import fs from 'fs';
import path from 'path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('desktop table theme styling', () => {
    test('striped rows use theme tokens instead of hard-coded light and dark bars', () => {
        const source = readSource('src/styles/desktop/global.scss');
        const themeSource = readSource('src/core/theme/base.ts');

        expect(source).toContain('rgb(var(--v-theme-table-row-striped))');
        expect(source).not.toContain('background: #fcfcfc');
        expect(source).not.toContain('background: #161616');
        expect(source).not.toContain('--v-table-header-background');
        expect(themeSource).toContain("'table-row-striped': '#242322'");
    });

    test('default icon color follows theme text color for custom dark themes', () => {
        const source = readSource('src/styles/desktop/global.scss');

        expect(source).toContain('--default-icon-color: rgb(var(--v-theme-on-surface))');
        expect(source).not.toContain('--default-icon-color: #000');
    });
});
