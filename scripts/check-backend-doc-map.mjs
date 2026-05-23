#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const markdownPath = path.resolve(repoRoot, 'docs', 'backend-map.md');
const htmlPath = path.resolve(repoRoot, 'docs', 'backend-map.html');

const requiredAnchors = [
    'backend-layering',
    'api-request-lifecycle',
    'import-pipeline',
    'repository-data-flow',
    'development-recipes',
    'verification-matrix'
];

const requiredPaths = [
    'src/backend/http',
    'src/backend/core',
    'src/backend/db',
    'src/backend/parsers',
    'tests/backend/http',
    'tests/backend/core',
    'tests/backend/db',
    'tests/backend/parsers'
];

function readRequired(filePath) {
    if (!fs.existsSync(filePath)) {
        throw new Error(`Missing required file: ${path.relative(repoRoot, filePath)}`);
    }

    return fs.readFileSync(filePath, 'utf8');
}

function requireContains(source, needle, label) {
    if (!source.includes(needle)) {
        throw new Error(`Missing ${label}: ${needle}`);
    }
}

function requireNotContains(source, needle, label) {
    if (source.includes(needle)) {
        throw new Error(`Unexpected ${label}: ${needle}`);
    }
}

function checkNoExternalRuntime(html) {
    const externalPatterns = [
        /https?:\/\//i,
        /<script\b/i,
        /cdn/i
    ];

    for (const pattern of externalPatterns) {
        if (pattern.test(html)) {
            throw new Error(`HTML must stay static and dependency-free; matched ${pattern}`);
        }
    }
}

try {
    const markdown = readRequired(markdownPath);
    const html = readRequired(htmlPath);

    requireContains(markdown, 'Canonical source: docs/backend-map.md', 'Markdown canonical marker');
    requireContains(html, 'Canonical source: docs/backend-map.md', 'HTML canonical marker');
    requireNotContains(markdown, '.tmp/', 'ignored temporary artifact path in Markdown');
    requireNotContains(html, '.tmp/', 'ignored temporary artifact path in HTML');

    for (const anchor of requiredAnchors) {
        requireContains(markdown, `id="${anchor}"`, `Markdown anchor ${anchor}`);
        requireContains(html, `id="${anchor}"`, `HTML anchor ${anchor}`);
    }

    for (const sourcePath of requiredPaths) {
        requireContains(markdown, sourcePath, `Markdown source path ${sourcePath}`);
    }

    for (const flowWord of ['后端分层', 'API 请求生命周期', '导入管线', 'Repository 数据流', '验证矩阵']) {
        requireContains(markdown, flowWord, `Markdown flow block ${flowWord}`);
        requireContains(html, flowWord, `HTML flow block ${flowWord}`);
    }

    checkNoExternalRuntime(html);

    console.log('PASS backend doc map is synchronized with required static markers');
} catch (error) {
    console.error(`FAIL ${error instanceof Error ? error.message : String(error)}`);
    process.exit(1);
}
