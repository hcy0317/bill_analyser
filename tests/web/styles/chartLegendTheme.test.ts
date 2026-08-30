import fs from 'fs';
import path from 'path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('chart legend theme styling', () => {
    test('budget history legend truncates visually while retaining full labels', () => {
        const source = readSource('src/views/desktop/budgets/components/BudgetHistoryLegend.vue');

        expect(source).toContain(':title="group.primaryLabel"');
        expect(source).toContain(':aria-label="group.primaryLabel"');
        expect(source).toContain(':title="item.label"');
        expect(source).toContain('text-overflow: ellipsis');
        expect(source).toContain('white-space: nowrap');
    });

    test('mobile progress tracks follow each application theme', () => {
        const source = readSource('src/styles/mobile/global.scss');

        expect(source).toContain('--f7-progressbar-bg-color: var(--ebk-progress-track-color)');
        expect(source).not.toContain('--f7-progressbar-bg-color: #f8f8f8');
        expect(source).not.toContain('--f7-progressbar-bg-color: #444444');
    });
});
