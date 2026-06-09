import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

const REPO_ROOT = path.resolve(process.cwd(), '..', '..');

function readRepoFile(relativePath: string): string {
    return fs.readFileSync(path.resolve(REPO_ROOT, relativePath), 'utf8');
}

describe('frontend auth logging redaction contract', () => {
    test('does not log bearer token or authorization header previews', () => {
        const sources = {
            'src/web/src/lib/services.ts': readRepoFile('src/web/src/lib/services.ts'),
            'src/web/src/lib/userstate.ts': readRepoFile('src/web/src/lib/userstate.ts'),
        };

        const forbiddenPatterns: RegExp[] = [
            /\btokenPreview\b/,
            /\bauthPreview\b/,
            /\bauthHeaderValue\b/,
            /Storing token:\s*preview=/,
            /token\.substring\(0,\s*20\)/,
            /String\([^)]*\)\.substring/,
            /Authorization[^\n]*substring/,
            /substring[^\n]*Authorization/,
        ];

        const violations = Object.entries(sources).flatMap(([relativePath, source]) => (
            forbiddenPatterns
                .filter(pattern => pattern.test(source))
                .map(pattern => `${relativePath} matched ${pattern}`)
        ));

        expect(violations).toEqual([]);
    });
});
