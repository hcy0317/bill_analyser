import { createHash } from 'crypto';
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'fs';
import path from 'path';

import { expect, test } from '@playwright/test';

import { getE2EEnvironment } from '../helpers/env';
import { openSignalFilterMenu } from '../helpers/importSignalSystem';
import { desktopRoute } from '../helpers/routes';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';

const REPO_ROOT = path.resolve(__dirname, '../../../..');
const REAL64_DIR = path.join(REPO_ROOT, 'bills');
const EVIDENCE_DIR = path.join(REPO_ROOT, '.omx', 'context');

test.describe('G019 real 64-file import preview', () => {
    test.describe.configure({ mode: 'serial', retries: 0, timeout: 240_000 });

    test('uploads the fixed real corpus and records canonical filter evidence', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        const manifest = buildReal64Manifest();
        const previewRequests: Array<Record<string, unknown>> = [];
        const requestStartedAt = new Map<string, number>();
        let parseStartedAt = 0;
        let parseElapsedMs = 0;
        let dedupElapsedMs = 0;

        page.on('request', requestValue => {
            const url = requestValue.url();
            if (url.includes('/api/bills/import/v2/preview/')) {
                requestStartedAt.set(url, Date.now());
            }
        });
        page.on('response', response => {
            const url = response.url();
            if (!url.includes('/api/bills/import/v2/preview/')) return;
            previewRequests.push({
                method: response.request().method(),
                url,
                status: response.status(),
                elapsedMs: Date.now() - (requestStartedAt.get(url) || Date.now()),
                serverTiming: response.headers()['server-timing'] || ''
            });
        });

        try {
            const env = getE2EEnvironment();
            await page.goto(desktopRoute('/transaction/list?pageType=0&dateType=7', env), {
                waitUntil: 'domcontentloaded'
            });
            await page.getByTestId('desktop.transactions.action.import').click();
            await expect(page.getByTestId('desktop.import.dialog')).toBeVisible();

            const fileInput = page.getByTestId('desktop.import.file-input');
            await fileInput.setInputFiles(manifest.files.map(file => file.path));

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

            const dedupResponse = await dedupResponsePromise;
            dedupElapsedMs = Date.now() - parseStartedAt;
            expect(dedupResponse.ok(), 'real64 dedup request').toBe(true);
            const dedupEnvelope = await dedupResponse.json() as Record<string, unknown>;
            const dedupData = unwrapData(dedupEnvelope);

            await expect(page.getByTestId('desktop.import.preview.table')).toBeVisible({ timeout: 120_000 });
            await page.screenshot({
                path: path.join(EVIDENCE_DIR, 'g019-real64-preview.png'),
                fullPage: true
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
                .filter(item => String(item['url']).includes('signal=parser'));
            expect(filterRequests).toHaveLength(1);
            expect(previewRequests.some(item => String(item['url']).includes('filter-index'))).toBe(false);

            mkdirSync(EVIDENCE_DIR, { recursive: true });
            writeFileSync(path.join(EVIDENCE_DIR, 'g019-real64-browser.json'), JSON.stringify({
                generatedAt: new Date().toISOString(),
                sessionId,
                corpus: {
                    directory: REAL64_DIR,
                    count: manifest.files.length,
                    totalBytes: manifest.totalBytes,
                    extensions: manifest.extensions,
                    files: manifest.files.map(file => ({
                        name: file.name,
                        bytes: file.bytes,
                        sha256: file.sha256
                    }))
                },
                parse: {
                    elapsedMs: parseElapsedMs,
                    parsedCount: parseData['parsed_count'],
                    unmatchedFiles: parseData['unmatched_files'] || []
                },
                dedup: {
                    elapsedMs: dedupElapsedMs,
                    afterDedup: dedupData['after_dedup'],
                    previewCount: dedupData['preview_count']
                },
                previewRequests,
                parserFilterCanonicalRequestCount: filterRequests.length
            }, null, 2));
        } finally {
            await cleanupE2ESession(session);
        }
    });
});

function buildReal64Manifest(): {
    files: Array<{ path: string; name: string; bytes: number; sha256: string }>;
    totalBytes: number;
    extensions: Record<string, number>;
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
    const extensions = files.reduce<Record<string, number>>((counts, file) => {
        const extension = path.extname(file.name).toLowerCase();
        counts[extension] = (counts[extension] || 0) + 1;
        return counts;
    }, {});
    return {
        files,
        totalBytes: files.reduce((sum, file) => sum + file.bytes, 0),
        extensions
    };
}

function unwrapData(envelope: Record<string, unknown>): Record<string, unknown> {
    const data = envelope['data'];
    if (!data || typeof data !== 'object') {
        throw new Error('Expected API data envelope');
    }
    return data as Record<string, unknown>;
}
