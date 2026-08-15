import { createHash } from 'crypto';
import path from 'path';

export interface Real64CorpusFile {
    readonly path: string;
    readonly name: string;
    readonly bytes: number;
    readonly sha256: string;
}

export interface Real64CorpusEvidence {
    readonly fileCount: number;
    readonly totalBytes: number;
    readonly extensions: Record<string, { count: number; bytes: number }>;
    readonly corpusManifestSha256: string;
}

export interface PreviewRequestEvidence {
    readonly route: 'preview';
    readonly signal: string | null;
    readonly method: string;
    readonly status: number;
    readonly elapsedMs: number;
    readonly serverTiming: string;
}

const FORBIDDEN_KEY_PATTERN = /(?:session.?id|original.?name|file.?name|description|directory|(?:^|_)path$|url)/iu;
const ABSOLUTE_WINDOWS_PATH_PATTERN = /^(?:[A-Za-z]:[\\/]|\\\\)/u;
const URL_PATTERN = /^[A-Za-z][A-Za-z0-9+.-]*:\/\//u;
const CORPUS_NAME_COLLATOR = new Intl.Collator('zh-CN', {
    usage: 'sort',
    sensitivity: 'variant',
    numeric: false
});

export function buildCorpusEvidence(files: readonly Real64CorpusFile[]): Real64CorpusEvidence {
    const sorted = [...files].sort((left, right) => compareNames(left.name, right.name));
    const extensions: Record<string, { count: number; bytes: number }> = {};
    const records: string[] = [];
    let totalBytes = 0;

    for (const file of sorted) {
        if (!file.name || !Number.isSafeInteger(file.bytes) || file.bytes < 0) {
            throw new Error('Real64 corpus entries require a name and non-negative integer byte count.');
        }
        if (!/^[0-9a-f]{64}$/u.test(file.sha256)) {
            throw new Error('Real64 corpus entries require a lowercase SHA-256 digest.');
        }
        const extension = path.extname(file.name).toLowerCase();
        const current = extensions[extension] ?? { count: 0, bytes: 0 };
        extensions[extension] = {
            count: current.count + 1,
            bytes: current.bytes + file.bytes
        };
        totalBytes += file.bytes;
        records.push(`${file.name}\t${file.bytes}\t${file.sha256}`);
    }

    return {
        fileCount: sorted.length,
        totalBytes,
        extensions,
        corpusManifestSha256: createHash('sha256').update(records.join('\n')).digest('hex')
    };
}

export function toPreviewRequestEvidence(
    rawURL: string,
    method: string,
    status: number,
    elapsedMs: number,
    serverTiming: string
): PreviewRequestEvidence {
    const parsed = new URL(rawURL);
    if (!parsed.pathname.includes('/api/bills/import/v2/preview/')) {
        throw new Error('Preview evidence requires the canonical preview route.');
    }
    return {
        route: 'preview',
        signal: parsed.searchParams.get('signal'),
        method,
        status,
        elapsedMs,
        serverTiming
    };
}

export function assertEvidencePrivacy(value: unknown, privateValues: readonly string[] = []): void {
    const seen = new WeakSet<object>();

    const visit = (candidate: unknown): void => {
        if (typeof candidate === 'string') {
            if (ABSOLUTE_WINDOWS_PATH_PATTERN.test(candidate) || candidate.startsWith('/')) {
                throw new Error('Evidence contains an absolute path.');
            }
            if (URL_PATTERN.test(candidate)) {
                throw new Error('Evidence contains a URL.');
            }
            if (privateValues.some(privateValue => privateValue && candidate.includes(privateValue))) {
                throw new Error('Evidence contains a private value.');
            }
            return;
        }
        if (!candidate || typeof candidate !== 'object') {
            return;
        }
        if (seen.has(candidate)) {
            return;
        }
        seen.add(candidate);
        if (Array.isArray(candidate)) {
            candidate.forEach(visit);
            return;
        }
        for (const [key, child] of Object.entries(candidate)) {
            if (FORBIDDEN_KEY_PATTERN.test(key)) {
                throw new Error(`Evidence contains forbidden key ${key}.`);
            }
            visit(child);
        }
    };

    visit(value);
}

export function resolveC0EvidenceDirectory(repoRoot: string, configured: string | undefined): string {
    if (!configured?.trim()) {
        throw new Error('E2E_REAL64_EVIDENCE_DIR is required for the C0 real64 run.');
    }
    const evidenceRoot = path.resolve(repoRoot, '.cyaness', 'evidence', 'c0-real64');
    const resolved = path.resolve(configured);
    const relative = path.relative(evidenceRoot, resolved);
    if (!relative) {
        throw new Error('E2E_REAL64_EVIDENCE_DIR must be a run-specific child directory.');
    }
    if (relative.startsWith('..') || path.isAbsolute(relative)) {
        throw new Error('E2E_REAL64_EVIDENCE_DIR must stay under .cyaness/evidence/c0-real64.');
    }
    return resolved;
}

function compareNames(left: string, right: string): number {
    const normalizedLeft = left.replace(/\\/gu, '/').toLowerCase();
    const normalizedRight = right.replace(/\\/gu, '/').toLowerCase();
    return CORPUS_NAME_COLLATOR.compare(normalizedLeft, normalizedRight)
        || CORPUS_NAME_COLLATOR.compare(left, right);
}
