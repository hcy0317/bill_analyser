import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8').replace(/\r\n/g, '\n');
}

function extractRouteBlock(source: string, routePath: string): string {
    const pathIndex = source.indexOf(`path: '${routePath}'`);
    expect(pathIndex).toBeGreaterThanOrEqual(0);
    const nextRouteIndex = source.indexOf('\n                {', pathIndex + 1);
    return source.slice(pathIndex, nextRouteIndex > pathIndex ? nextRouteIndex : source.length);
}

describe('router shared runtime shell contract', () => {
    test('desktop guarded routes keep login, unlock and not-login redirect contracts', () => {
        const source = readSource('src/router/desktop.ts');

        expect(source).toContain('function checkLogin(): NavigationGuardReturn {');
        expect(source).toContain("path: '/login',\n            replace: true");
        expect(source).toContain("path: '/unlock',\n            replace: true");
        expect(source).toContain('function checkLocked(): NavigationGuardReturn {');
        expect(source).toContain("path: '/',\n            replace: true");
        expect(source).toContain('function checkNotLogin(): NavigationGuardReturn {');
    });

    test('desktop transaction and statistics routes keep query-to-prop names stable', () => {
        const source = readSource('src/router/desktop.ts');
        const transactionRoute = extractRouteBlock(source, '/transaction/list');
        const statisticsRoute = extractRouteBlock(source, '/statistics/transaction');

        for (const queryName of [
            'pageType',
            'dateType',
            'maxTime',
            'minTime',
            'type',
            'categoryIds',
            'accountIds',
            'tagIds',
            'tagFilterType',
            'amountFilterCents',
            'keyword'
        ]) {
            expect(transactionRoute).toContain(`route.query['${queryName}']`);
        }

        for (const queryName of [
            'analysisType',
            'chartDataType',
            'chartType',
            'chartDateType',
            'startTime',
            'endTime',
            'filterAccountIds',
            'filterCategoryIds',
            'tagIds',
            'tagFilterType',
            'keyword',
            'sortingType',
            'trendDateAggregationType',
            'assetTrendsDateAggregationType'
        ]) {
            expect(statisticsRoute).toContain(`route.query['${queryName}']`);
        }
    });

    test('mobile guard callbacks reject before redirecting and preserve history options', () => {
        const source = readSource('src/router/mobile.ts');

        for (const targetPath of ['/login', '/unlock', '/']) {
            expect(source).toContain(`router.navigate('${targetPath}', {\n            clearPreviousHistory: true,\n            browserHistory: false\n        });`);
        }

        expect(source).toContain('function checkLogin({ router, resolve, reject }');
        expect(source).toContain('function checkLocked({ router, resolve, reject }');
        expect(source).toContain('function checkNotLogin({ router, resolve, reject }');
        expect(source.match(/reject\(\);\n        router\.navigate/g)).toHaveLength(6);
    });

    test('mobile route registry keeps import preview, account rules and fallback shell entries', () => {
        const source = readSource('src/router/mobile.ts');

        for (const routePath of [
            '/transaction/import/preview',
            '/account/rules',
            '/settings/sync',
            '/user/2fa',
            '/budgets'
        ]) {
            expect(extractRouteBlock(source, routePath)).toContain('beforeEnter: [checkLogin]');
        }

        expect(source).toContain("path: '(.*)',\n        redirect: '/'");
    });
});
