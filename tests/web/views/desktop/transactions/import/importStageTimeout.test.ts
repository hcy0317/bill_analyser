import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('import stage timeout policy', () => {
    test('stage 1 parse uses the import parse timeout instead of generic upload timeout', () => {
        const apiSource = readSource('src/consts/api.ts');
        const dialogSource = readSource('src/views/desktop/transactions/import/ImportDialog.vue');

        expect(apiSource).toContain('DEFAULT_IMPORT_PARSE_API_TIMEOUT');
        expect(dialogSource).toContain('DEFAULT_IMPORT_PARSE_API_TIMEOUT');
        expect(dialogSource).toContain('timeoutMs = DEFAULT_UPLOAD_API_TIMEOUT');
        expect(dialogSource).toContain("}, '阶段1解析', DEFAULT_IMPORT_PARSE_API_TIMEOUT);");
    });

    test('abort errors include client-timeout context and cleanup remains in finally', () => {
        const dialogSource = readSource('src/views/desktop/transactions/import/ImportDialog.vue');

        expect(dialogSource).toContain("error.name === 'AbortError'");
        expect(dialogSource).toContain('客户端等待超时');
        expect(dialogSource).toContain('后端 parser/import 日志');
        expect(dialogSource).toMatch(/finally\s*\{\s*submitting\.value = false;\s*\}/);
    });
});
