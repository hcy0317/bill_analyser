import childProcess from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

import {
    RUST_ROUTE_OWNERSHIP,
    RUST_ROUTE_OWNERSHIP_GENERATED_FROM,
} from '@/contracts/rustRouteOwnership.generated';

const REPO_ROOT = path.resolve(process.cwd(), '..', '..');
const FRONTEND_ROOT = path.resolve(REPO_ROOT, 'src', 'web', 'src');
const FRONTEND_SOURCE_EXTENSIONS = new Set(['.ts', '.vue']);
const AXIOS_VERBS = new Map([
    ['get', 'GET'],
    ['post', 'POST'],
    ['postForm', 'POST'],
    ['put', 'PUT'],
    ['delete', 'DELETE'],
]);
const RETIRED_IMPORT_RUNTIME_ROUTES = new Set([
    'GET /api/bills/import/parsers',
    'POST /api/bills/import/upload',
]);

interface FrontendRouteUse {
    readonly method: string;
    readonly path: string;
    readonly source: string;
}

function uniqueMethods(methods: string[]): string[] {
    return [...new Set(methods)];
}

function walkFrontendFiles(dir: string): string[] {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    const files: string[] = [];
    for (const entry of entries) {
        const absolutePath = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            files.push(...walkFrontendFiles(absolutePath));
        } else if (FRONTEND_SOURCE_EXTENSIONS.has(path.extname(entry.name))) {
            files.push(absolutePath);
        }
    }
    return files;
}

function normalizePath(rawPath: string, sourcePath: string): string | null {
    const trimmed = rawPath
        .replace(/\$\{BASE_API_URL_PATH\}/g, '/api')
        .replace(/\$\{[^}]+\}/g, '{param}')
        .replace(/`/g, '')
        .trim();

    if (/^https?:\/\//.test(trimmed)) {
        return null;
    }

    const withoutQuery = trimmed.split('?')[0] ?? trimmed;
    if (withoutQuery.startsWith('/api/')) {
        return withoutQuery;
    }
    if (sourcePath.replaceAll('\\', '/').endsWith('src/web/src/lib/services.ts') && withoutQuery.length > 0) {
        return `/api/${withoutQuery.replace(/^\/+/, '')}`;
    }
    return null;
}

function collectFrontendRouteUses(): FrontendRouteUse[] {
    const uses: FrontendRouteUse[] = [];
    for (const absolutePath of walkFrontendFiles(FRONTEND_ROOT)) {
        const source = fs.readFileSync(absolutePath, 'utf8');
        const relativePath = path.relative(REPO_ROOT, absolutePath).replaceAll('\\', '/');

        for (const match of source.matchAll(/axios\.(get|postForm|post|put|delete)(?:<[\s\S]{0,220}?>)?\s*\(\s*(['"`])([^'"`]*)\2/g)) {
            const method = AXIOS_VERBS.get(match[1] ?? '');
            const routePath = method ? normalizePath(match[3] ?? '', relativePath) : null;
            if (method && routePath) {
                uses.push({ method, path: routePath, source: relativePath });
            }
        }

        for (const match of source.matchAll(/fetch\s*\(\s*(['"`])([^'"`]*)\1\s*,\s*\{([\s\S]*?)\}/g)) {
            const options = match[3] ?? '';
            const methodMatch = options.match(/method:\s*['"`]([A-Z]+)['"`]/);
            const methods = methodMatch?.[1]
                ? [methodMatch[1]]
                : uniqueMethods([...options.matchAll(/['"`](GET|POST|PUT|DELETE|PATCH)['"`]/g)]
                    .map(optionMatch => optionMatch[1] ?? ''))
                    .filter(Boolean);
            const effectiveMethods = methods.length > 0 ? methods : ['GET'];
            const routePath = normalizePath(match[2] ?? '', relativePath);
            if (routePath) {
                for (const method of effectiveMethods) {
                    uses.push({ method, path: routePath, source: relativePath });
                }
            }
        }

        for (const match of source.matchAll(/fetch\s*\(\s*(['"`])([^'"`]*)\1\s*\)/g)) {
            const routePath = normalizePath(match[2] ?? '', relativePath);
            if (routePath) {
                uses.push({ method: 'GET', path: routePath, source: relativePath });
            }
        }
    }
    return uses;
}

function routePatternToRegex(pattern: string): RegExp {
    const normalizedPattern = pattern.replace(/\/$/, '');
    const escaped = normalizedPattern
        .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
        .replace(/\\\{\\\*[^}]+\\\}/g, '.+')
        .replace(/\\\{[^}]+\\\}/g, '[^/]+');
    return new RegExp(`^${escaped}/?$`);
}

function frontendPathToRegex(frontendPath: string): RegExp {
    const normalizedPath = frontendPath.replace(/\/$/, '');
    const escaped = normalizedPath
        .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
        .replace(/\\\{param\\\}/g, '[^/]+');
    return new RegExp(`^${escaped}/?$`);
}

const runtimeRoutes = RUST_ROUTE_OWNERSHIP
    .filter(route => route.state === 'rust_owned_verified')
    .map(route => ({
        ...route,
        patternMatcher: routePatternToRegex(route.pattern),
    }));

function findRuntimeRoute(use: FrontendRouteUse): string | null {
    const frontendMatcher = frontendPathToRegex(use.path);
    const candidatePaths = use.path.endsWith('/')
        ? [use.path, `${use.path}{param}`]
        : [use.path];
    const match = runtimeRoutes.find(route => {
        if (route.method !== use.method) {
            return false;
        }
        return candidatePaths.some(candidatePath => route.patternMatcher.test(candidatePath))
            || frontendMatcher.test(route.pattern);
    });
    return match ? `${match.method} ${match.pattern} [${match.state}]` : null;
}

describe('frontend Rust route contract', () => {
    test('keeps the generated Rust route fixture in sync with runtime governance', () => {
        expect(RUST_ROUTE_OWNERSHIP_GENERATED_FROM)
            .toBe('bill_runtime_manifest::governance_manifest_snapshot.routes');
        childProcess.execFileSync(
            process.execPath,
            ['scripts/generate-rust-route-fixture.mjs', '--check'],
            { cwd: path.resolve(REPO_ROOT, 'src', 'web'), stdio: 'pipe' },
        );
    });

    test('all Vue service/fetch API calls resolve to Rust runtime-owned routes', () => {
        const missing = collectFrontendRouteUses()
            .map(use => ({ use, route: findRuntimeRoute(use) }))
            .filter(({ route }) => route === null)
            .map(({ use }) => `${use.method} ${use.path} from ${use.source}`);

        expect(missing).toEqual([]);
    });

    test('frontend code does not point runtime calls at the retired v1 API namespace', () => {
        const offenders = walkFrontendFiles(FRONTEND_ROOT)
            .flatMap(absolutePath => {
                const source = fs.readFileSync(absolutePath, 'utf8');
                const relativePath = path.relative(REPO_ROOT, absolutePath).replaceAll('\\', '/');
                return [...source.matchAll(/(['"`])\/api\/v1\b/g)].map(() => relativePath);
            });

        expect([...new Set(offenders)]).toEqual([]);
    });

    test('frontend code does not revive retired one-shot import endpoints', () => {
        const offenders = collectFrontendRouteUses()
            .filter(use => RETIRED_IMPORT_RUNTIME_ROUTES.has(`${use.method} ${use.path}`))
            .map(use => `${use.method} ${use.path} from ${use.source}`);

        expect(offenders).toEqual([]);
    });
});
