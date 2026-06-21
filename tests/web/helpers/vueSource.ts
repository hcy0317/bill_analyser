import fs from 'node:fs';
import path from 'node:path';

function readText(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8').replace(/\r\n/g, '\n');
}

function readExternalBlockSource(vueSource: string, vuePath: string): string[] {
    const vueDir = path.dirname(path.resolve(process.cwd(), vuePath));
    const externalPaths: string[] = [];
    const srcPattern = /<(template|style)\b[^>]*\bsrc="([^"]+)"[^>]*>/gu;

    for (const match of vueSource.matchAll(srcPattern)) {
        const src = match[2];
        if (!src || !src.startsWith('./')) {
            continue;
        }

        externalPaths.push(path.resolve(vueDir, src));
    }

    return externalPaths.map(filePath => fs.readFileSync(filePath, 'utf-8').replace(/\r\n/g, '\n'));
}

export function readVueSourceWithExternalBlocks(relativePath: string): string {
    const source = readText(relativePath);
    return [source, ...readExternalBlockSource(source, relativePath)].join('\n');
}

export function readSource(relativePath: string): string {
    return readText(relativePath);
}
