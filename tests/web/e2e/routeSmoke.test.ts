import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { expect, test } from '@jest/globals';

const routeSmokeSpecs = [
    'src/web/e2e/tests/desktop.route-smoke.spec.ts',
    'src/web/e2e/tests/mobile.route-smoke.spec.ts'
];

test.each(routeSmokeSpecs)('%s waits for the response commit and a page anchor', relativePath => {
    const source = readFileSync(resolve(__dirname, '../../..', relativePath), 'utf8');

    expect(source).toContain("waitUntil: 'commit'");
    expect(source).not.toContain("waitUntil: 'domcontentloaded'");
    expect(source).toContain(
        'await expectPageAnchor(page, target.testId, ROUTE_SMOKE_PAGE_ANCHOR_TIMEOUT_MS);'
    );
});

test('route smoke keeps the normal anchor budget locally and allows a bounded CI cold-start budget', () => {
    const routesSource = readFileSync(
        resolve(__dirname, '../../../src/web/e2e/helpers/routes.ts'),
        'utf8'
    );
    const assertionsSource = readFileSync(
        resolve(__dirname, '../../../src/web/e2e/helpers/assertions.ts'),
        'utf8'
    );

    expect(routesSource).toContain(
        "export const ROUTE_SMOKE_PAGE_ANCHOR_TIMEOUT_MS = process.env['CI'] ? 45_000 : 15_000;"
    );
    expect(routesSource).toContain(
        "export const ROUTE_SMOKE_TEST_TIMEOUT_MS = process.env['CI'] ? 90_000 : 30_000;"
    );
    expect(assertionsSource).toContain('timeoutMs = 15_000');
    expect(assertionsSource).toContain('timeout: timeoutMs');

    for (const relativePath of routeSmokeSpecs) {
        const source = readFileSync(resolve(__dirname, '../../..', relativePath), 'utf8');
        expect(source).toContain('test.setTimeout(ROUTE_SMOKE_TEST_TIMEOUT_MS);');
    }
});
