import { createHash } from 'crypto';
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'fs';
import path from 'path';

import { expect, test } from '@playwright/test';

import { getE2EEnvironment } from '../helpers/env';
import { openSignalFilterMenu } from '../helpers/importSignalSystem';
import {
    assertEvidencePrivacy,
    buildCorpusEvidence,
    parseServerTimingEvidence,
    resolveC0EvidenceDirectory,
    toPreviewRequestEvidence,
    type PreviewRequestEvidence,
    type Real64CorpusFile
} from '../helpers/real64Evidence';
import { desktopRoute } from '../helpers/routes';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';

const REPO_ROOT = path.resolve(__dirname, '../../../..');
const REAL64_DIR = path.join(REPO_ROOT, 'bills');
const LEGACY_BINARY_XLS_FILE_NAME = '交易明细_9316_20230827_20250827.xls';
const EXPECTED_CORPUS_MANIFEST_SHA256 = '0a3b595c268413d7f404d823c6a0d0456d0e81b655e47c3c3f92e412b2c4c838';

test.describe('Cyanflow C0 real 64-file import preview', () => {
    test.describe.configure({ mode: 'serial', retries: 0, timeout: 240_000 });

    test('parses the binary XLS file and continues into dedup', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        const manifest = buildReal64Manifest();
        let dedupRequestCount = 0;

        page.on('request', requestValue => {
            if (requestValue.method() === 'POST' && requestValue.url().includes('/api/bills/import/v2/dedup')) {
                dedupRequestCount += 1;
            }
        });

        try {
            const env = getE2EEnvironment();
            await page.goto(desktopRoute('/transaction/list?pageType=0&dateType=7', env), {
                waitUntil: 'domcontentloaded'
            });
            await page.getByTestId('desktop.transactions.action.import').click();
            await expect(page.getByTestId('desktop.import.dialog')).toBeVisible();
            await page.getByTestId('desktop.import.file-input').setInputFiles(manifest.files.map(file => file.path));

            const parseResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().includes('/api/bills/import/v2/parse')
            ));
            const dedupResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().includes('/api/bills/import/v2/dedup')
            ));
            await page.getByTestId('desktop.import.action.next').click();
            const parseResponse = await parseResponsePromise;
            expect(parseResponse.ok(), 'real64 parse request').toBe(true);
            const parseData = unwrapData(await parseResponse.json() as Record<string, unknown>);
            const unmatchedFiles = Array.isArray(parseData['unmatched_files'])
                ? parseData['unmatched_files'] as Array<Record<string, unknown>>
                : [];

            expect(unmatchedFiles.map(file => String(file['original_name'] || '')))
                .not.toContain(LEGACY_BINARY_XLS_FILE_NAME);
            expect((await dedupResponsePromise).ok(), 'real64 dedup request').toBe(true);
            expect(dedupRequestCount).toBeGreaterThan(0);

            const evidenceDir = evidenceDirectory();
            const evidence = {
                schemaVersion: 1,
                generatedAt: new Date().toISOString(),
                corpus: buildCorpusEvidence(manifest.files),
                parse: {
                    parsedCount: parseData['parsed_count'],
                    unmatchedFileCount: unmatchedFiles.length,
                    legacyBinaryXlsMatched: !unmatchedFiles.some(file => (
                        String(file['original_name'] || '') === LEGACY_BINARY_XLS_FILE_NAME
                    ))
                },
                dedup: { requestObserved: dedupRequestCount > 0 }
            };
            assertEvidencePrivacy(evidence, manifest.files.map(file => file.name));
            mkdirSync(evidenceDir, { recursive: true });
            writeFileSync(
                path.join(evidenceDir, 'c0-real64-binary-xls.json'),
                `${JSON.stringify(evidence, null, 2)}\n`,
                'utf8'
            );
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('imports the compatible real corpus and records canonical filter evidence', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        const manifest = buildReal64Manifest();
        const compatibleFiles = manifest.files;
        expect(compatibleFiles).toHaveLength(64);
        const previewRequests: PreviewRequestEvidence[] = [];
        const requestStartedAt = new Map<string, number>();
        let legacyFilterIndexRequestCount = 0;
        let parseStartedAt = 0;
        let parseElapsedMs = 0;
        let dedupElapsedMs = 0;
        let previewOperableElapsedMs = 0;

        page.on('request', requestValue => {
            const url = requestValue.url();
            if (url.includes('/api/bills/import/v2/preview/')) {
                requestStartedAt.set(url, Date.now());
            }
        });
        page.on('response', response => {
            const url = response.url();
            if (!url.includes('/api/bills/import/v2/preview/')) return;
            if (url.includes('filter-index')) legacyFilterIndexRequestCount += 1;
            previewRequests.push(toPreviewRequestEvidence(
                url,
                response.request().method(),
                response.status(),
                Date.now() - (requestStartedAt.get(url) || Date.now()),
                response.headers()['server-timing'] || ''
            ));
        });

        try {
            const env = getE2EEnvironment();
            await page.goto(desktopRoute('/transaction/list?pageType=0&dateType=7', env), {
                waitUntil: 'domcontentloaded'
            });
            await page.getByTestId('desktop.transactions.action.import').click();
            await expect(page.getByTestId('desktop.import.dialog')).toBeVisible();

            const fileInput = page.getByTestId('desktop.import.file-input');
            await fileInput.setInputFiles(compatibleFiles.map(file => file.path));

            const parseResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().includes('/api/bills/import/v2/parse')
            ));
            const dedupResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().includes('/api/bills/import/v2/dedup')
            ));
            parseStartedAt = Date.now();
            await page.getByTestId('desktop.import.action.next').click();
            const parseResponse = await parseResponsePromise;
            parseElapsedMs = Date.now() - parseStartedAt;
            expect(parseResponse.ok(), 'real64 parse request').toBe(true);
            const parseEnvelope = await parseResponse.json() as Record<string, unknown>;
            const parseData = unwrapData(parseEnvelope);
            const sessionId = String(parseData['session_id'] || '');
            expect(sessionId).not.toBe('');
            const stage1Timing = parseServerTimingEvidence(
                parseResponse.headers()['server-timing'] || '',
                ['multipart', 'parser', 'staging', 'total']
            );

            const dedupResponse = await dedupResponsePromise;
            dedupElapsedMs = Date.now() - parseStartedAt;
            expect(dedupResponse.ok(), 'real64 dedup request').toBe(true);
            const dedupEnvelope = await dedupResponse.json() as Record<string, unknown>;
            const dedupData = unwrapData(dedupEnvelope);
            const stage2Timing = parseServerTimingEvidence(
                dedupResponse.headers()['server-timing'] || '',
                ['dedup', 'intelligence', 'preview-insert', 'response', 'total']
            );

            const previewTable = page.getByTestId('desktop.import.preview.table');
            await expect(previewTable).toBeVisible({ timeout: 120_000 });
            previewOperableElapsedMs = Date.now() - parseStartedAt;
            const evidenceDir = evidenceDirectory();
            mkdirSync(evidenceDir, { recursive: true });
            await page.getByTestId('desktop.import.dialog').screenshot({
                path: path.join(evidenceDir, 'c0-real64-preview-redacted.png'),
                mask: [previewTable],
                maskColor: '#202124'
            });

            const requestsBeforeFilter = previewRequests.length;
            const signalMenu = await openSignalFilterMenu(page);
            const parserFilter = signalMenu.getByText('Parser', { exact: true });
            const parserResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'GET'
                && response.url().includes(`/api/bills/import/v2/preview/${encodeURIComponent(sessionId)}`)
                && response.url().includes('signal=parser')
            ));
            await parserFilter.click();
            const parserResponse = await parserResponsePromise;
            expect(parserResponse.ok(), 'real64 parser filter request').toBe(true);
            await expect(page.getByTestId('desktop.import.preview.table')).toBeVisible();

            const filterRequests = previewRequests.slice(requestsBeforeFilter)
                .filter(item => item.signal === 'parser');
            expect(filterRequests).toHaveLength(1);
            expect(legacyFilterIndexRequestCount).toBe(0);

            const browserEvidence = {
                schemaVersion: 1,
                generatedAt: new Date().toISOString(),
                corpus: buildCorpusEvidence(manifest.files),
                upload: { fileCount: compatibleFiles.length },
                parse: {
                    elapsedMs: parseElapsedMs,
                    parsedCount: parseData['parsed_count'],
                    unmatchedFileCount: Array.isArray(parseData['unmatched_files'])
                        ? parseData['unmatched_files'].length
                        : 0
                },
                dedup: {
                    elapsedMs: dedupElapsedMs,
                    afterDedup: dedupData['after_dedup'],
                    previewCount: dedupData['preview_count']
                },
                profile: {
                    stage1: stage1Timing,
                    stage2: stage2Timing
                },
                previewRequests,
                browser: {
                    previewOperableElapsedMs,
                    parserFilterCanonicalRequestCount: filterRequests.length,
                    legacyFilterIndexRequestCount
                }
            };
            const browserTrace = {
                schemaVersion: 1,
                events: [
                    { name: 'upload-started', elapsedMs: 0 },
                    { name: 'parse-completed', elapsedMs: parseElapsedMs },
                    { name: 'dedup-completed', elapsedMs: dedupElapsedMs },
                    { name: 'preview-operable', elapsedMs: previewOperableElapsedMs },
                    { name: 'parser-filter-applied', elapsedMs: Date.now() - parseStartedAt }
                ]
            };
            const privateFileNames = manifest.files.map(file => file.name);
            assertEvidencePrivacy(browserEvidence, privateFileNames);
            assertEvidencePrivacy(browserTrace, privateFileNames);
            writeFileSync(
                path.join(evidenceDir, 'c0-real64-browser.json'),
                `${JSON.stringify(browserEvidence, null, 2)}\n`,
                'utf8'
            );
            writeFileSync(
                path.join(evidenceDir, 'c0-real64-browser-trace.json'),
                `${JSON.stringify(browserTrace, null, 2)}\n`,
                'utf8'
            );
        } finally {
            await cleanupE2ESession(session);
        }
    });
});

function buildReal64Manifest(): {
    files: Real64CorpusFile[];
} {
    const files = readdirSync(REAL64_DIR)
        .map(name => path.join(REAL64_DIR, name))
        .filter(filePath => statSync(filePath).isFile())
        .sort((left, right) => left.localeCompare(right))
        .map(filePath => {
            const buffer = readFileSync(filePath);
            return {
                path: filePath,
                name: path.basename(filePath),
                bytes: buffer.length,
                sha256: createHash('sha256').update(buffer).digest('hex')
            };
        });
    expect(files).toHaveLength(64);
    expect(buildCorpusEvidence(files).corpusManifestSha256).toBe(EXPECTED_CORPUS_MANIFEST_SHA256);
    return { files };
}

function evidenceDirectory(): string {
    return resolveC0EvidenceDirectory(REPO_ROOT, process.env['E2E_REAL64_EVIDENCE_DIR']);
}

function unwrapData(envelope: Record<string, unknown>): Record<string, unknown> {
    const data = envelope['data'];
    if (!data || typeof data !== 'object') {
        throw new Error('Expected API data envelope');
    }
    return data as Record<string, unknown>;
}
