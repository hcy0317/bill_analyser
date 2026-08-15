import { readFileSync } from 'fs';
import path from 'path';

import {
    assertEvidencePrivacy,
    buildCorpusEvidence,
    resolveC0EvidenceDirectory,
    toPreviewRequestEvidence
} from '../../../src/web/e2e/helpers/real64Evidence';

describe('C0 real64 evidence contract', () => {
    test('corpus evidence keeps aggregates and digest without private file identity', () => {
        const evidence = buildCorpusEvidence([
            {
                path: '/private/bills/private-b.xls',
                name: 'private-b.xls',
                bytes: 5,
                sha256: 'b'.repeat(64)
            },
            {
                path: '/private/bills/private-a.csv',
                name: 'private-a.csv',
                bytes: 3,
                sha256: 'a'.repeat(64)
            }
        ]);

        expect(evidence).toEqual({
            fileCount: 2,
            totalBytes: 8,
            extensions: {
                '.csv': { count: 1, bytes: 3 },
                '.xls': { count: 1, bytes: 5 }
            },
            corpusManifestSha256: '89142ec83e0f0142fea07056f2bf355e24cf18b6f9ebeea272be35686090f19a'
        });
        expect(JSON.stringify(evidence)).not.toContain('private-a.csv');
        expect(JSON.stringify(evidence)).not.toContain('/private/bills');
    });

    test('corpus digest uses explicit zh-CN collation instead of the host locale', () => {
        const evidence = buildCorpusEvidence([
            { path: '/fixture/second.csv', name: '账单乙.csv', bytes: 2, sha256: 'b'.repeat(64) },
            { path: '/fixture/first.csv', name: '账单甲.csv', bytes: 1, sha256: 'a'.repeat(64) }
        ]);

        expect(evidence.corpusManifestSha256).toBe(
            '72b79d1db1619ac8fdf5972dadad52265ef94591d5a2cdceb3237e2d427c9662'
        );
    });

    test('preview request evidence removes host and session identity', () => {
        const evidence = toPreviewRequestEvidence(
            'http://127.0.0.1:5000/api/bills/import/v2/preview/session-secret?page=1&signal=parser',
            'GET',
            200,
            42,
            'app;dur=12'
        );

        expect(evidence).toEqual({
            route: 'preview',
            signal: 'parser',
            method: 'GET',
            status: 200,
            elapsedMs: 42,
            serverTiming: 'app;dur=12'
        });
        expect(JSON.stringify(evidence)).not.toContain('session-secret');
        expect(JSON.stringify(evidence)).not.toContain('127.0.0.1');
    });

    test('privacy assertion rejects identifiers, paths, URLs, and known file names', () => {
        expect(() => assertEvidencePrivacy({ sessionId: 'secret' })).toThrow(/forbidden key/i);
        expect(() => assertEvidencePrivacy({ source: 'C:\\private\\bill.csv' })).toThrow(/absolute path/i);
        expect(() => assertEvidencePrivacy({ source: 'https://example.test/private' })).toThrow(/URL/i);
        expect(() => assertEvidencePrivacy({ note: 'private-a.csv' }, ['private-a.csv']))
            .toThrow(/private value/i);
    });

    test('evidence directory must be a run child under the Cyaness C0 root', () => {
        const repoRoot = path.resolve('/repo');
        const allowed = path.join(repoRoot, '.cyaness', 'evidence', 'c0-real64', 'run-1');

        expect(resolveC0EvidenceDirectory(repoRoot, allowed)).toBe(allowed);
        expect(() => resolveC0EvidenceDirectory(repoRoot, ''))
            .toThrow(/E2E_REAL64_EVIDENCE_DIR/);
        expect(() => resolveC0EvidenceDirectory(repoRoot, path.join(repoRoot, '.omx', 'context')))
            .toThrow(/\.cyaness/);
        expect(() => resolveC0EvidenceDirectory(repoRoot, path.join(repoRoot, '.cyaness', 'evidence', 'c0-real64')))
            .toThrow(/run-specific child/);
    });

    test('real browser spec persists evidence only through the Cyaness privacy contract', () => {
        const source = readFileSync(
            path.resolve(__dirname, '../../../src/web/e2e/tests/import-preview-fixes.real64.desktop.spec.ts'),
            'utf8'
        );

        expect(source).not.toContain("'.omx'");
        expect(source).toContain('resolveC0EvidenceDirectory');
        expect(source).toContain('toPreviewRequestEvidence');
        expect((source.match(/assertEvidencePrivacy\(/gu) ?? []).length).toBeGreaterThanOrEqual(2);
    });

    test('baseline runner owns isolated database cleanup and Cyaness evidence indexing', () => {
        const source = readFileSync(
            path.resolve(__dirname, '../../../scripts/run-c0-real64-baseline.mjs'),
            'utf8'
        );
        const packageManifest = JSON.parse(readFileSync(
            path.resolve(__dirname, '../../../src/web/package.json'),
            'utf8'
        )) as { scripts?: Record<string, string> };
        const gitignore = readFileSync(path.resolve(__dirname, '../../../.gitignore'), 'utf8');

        expect(source).not.toContain('.omx');
        expect(source).toContain("'.cyaness', 'evidence', 'c0-real64'");
        expect(source).toContain('runSupervisor');
        expect(source).toContain('DROP DATABASE IF EXISTS');
        expect(source).toContain('WITH (FORCE)');
        expect(source).toContain('writeArtifactIndex');
        expect(source).toMatch(
            /await adapters\.build\(signal\);\s+writeRuntimeProvenance\(config, databaseName\);/u
        );
        expect(packageManifest.scripts?.['e2e:c0:real64']).toBe(
            'node ../../scripts/run-c0-real64-baseline.mjs'
        );
        expect(gitignore.split(/\r?\n/gu)).toContain('.cyaness/evidence/');
    });
});
