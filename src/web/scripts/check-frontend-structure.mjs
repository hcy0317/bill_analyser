#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, '..');
const srcRoot = path.resolve(webRoot, 'src');
const baselinePath = path.resolve(scriptDir, 'frontend-structure-baseline.json');
const args = new Set(process.argv.slice(2));

const DEFAULT_THRESHOLDS = Object.freeze({
    vue: {
        healthyMin: 200,
        healthyMax: 400,
        warning: 500,
        hard: 800,
        templateWarning: 250,
        scriptWarning: 500,
        styleWarning: 300
    },
    ts: {
        warning: 600,
        hard: 1000
    },
    js: {
        warning: 600,
        hard: 1000
    },
    style: {
        warning: 600,
        hard: 1000
    }
});

const SCANNED_EXTENSIONS = new Set([
    '.vue',
    '.ts',
    '.tsx',
    '.mts',
    '.js',
    '.jsx',
    '.scss',
    '.css'
]);

const SKIPPED_DIRECTORIES = new Set([
    'node_modules',
    'dist',
    'coverage',
    '.vite'
]);

function printUsage() {
    console.log([
        'Usage: node scripts/check-frontend-structure.mjs [--print-baseline]',
        '',
        'Checks Vue/TypeScript/style file size structure against a committed',
        'baseline. Historical oversized files may shrink, but must not grow.',
        'New files must stay within warning thresholds.'
    ].join('\n'));
}

function countLines(text) {
    if (text.length === 0) {
        return 0;
    }

    const normalized = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
    const trimmedTrailingNewline = normalized.endsWith('\n')
        ? normalized.slice(0, -1)
        : normalized;

    return trimmedTrailingNewline.length === 0
        ? 1
        : trimmedTrailingNewline.split('\n').length;
}

function normalizePath(filePath) {
    return path.relative(webRoot, filePath).split(path.sep).join('/');
}

function getKind(filePath) {
    const extension = path.extname(filePath).toLowerCase();

    if (extension === '.vue') {
        return 'vue';
    }

    if (extension === '.scss' || extension === '.css') {
        return 'style';
    }

    if (extension === '.js' || extension === '.jsx') {
        return 'js';
    }

    return 'ts';
}

function getSectionLines(source, tagName) {
    const matches = source.matchAll(new RegExp(`<${tagName}\\b[^>]*>[\\s\\S]*?<\\/${tagName}>`, 'gi'));
    let lines = 0;

    for (const match of matches) {
        lines += countLines(match[0]);
    }

    return lines;
}

function readFiles(rootDir) {
    const results = [];
    const entries = fs.readdirSync(rootDir, { withFileTypes: true });

    for (const entry of entries) {
        const fullPath = path.resolve(rootDir, entry.name);

        if (entry.isDirectory()) {
            if (!SKIPPED_DIRECTORIES.has(entry.name)) {
                results.push(...readFiles(fullPath));
            }
            continue;
        }

        if (entry.isFile() && SCANNED_EXTENSIONS.has(path.extname(entry.name).toLowerCase())) {
            results.push(fullPath);
        }
    }

    return results;
}

function scanFile(filePath) {
    const source = fs.readFileSync(filePath, 'utf8');
    const kind = getKind(filePath);
    const record = {
        path: normalizePath(filePath),
        kind,
        lines: countLines(source)
    };

    if (kind === 'vue') {
        record.sections = {
            template: getSectionLines(source, 'template'),
            script: getSectionLines(source, 'script'),
            style: getSectionLines(source, 'style')
        };
    }

    return record;
}

function getThresholds(baseline) {
    return {
        ...DEFAULT_THRESHOLDS,
        ...(baseline?.thresholds ?? {})
    };
}

function scanProject() {
    return readFiles(srcRoot)
        .map(scanFile)
        .sort((left, right) => {
            if (right.lines !== left.lines) {
                return right.lines - left.lines;
            }

            return left.path.localeCompare(right.path);
        });
}

function isOversized(record, thresholds) {
    const threshold = thresholds[record.kind];

    if (!threshold) {
        return false;
    }

    if (record.lines > threshold.warning) {
        return true;
    }

    if (record.kind === 'vue') {
        return record.sections.template > threshold.templateWarning
            || record.sections.script > threshold.scriptWarning
            || record.sections.style > threshold.styleWarning;
    }

    return false;
}

function buildBaseline(records, thresholds) {
    return {
        version: 1,
        description: 'Frontend file-size baseline for legacy oversized files. Ratchet only: entries may shrink but must not grow.',
        thresholds,
        generatedBy: 'scripts/check-frontend-structure.mjs --print-baseline',
        files: records
            .filter(record => isOversized(record, thresholds))
            .map(record => ({
                ...record,
                reason: 'legacy oversized file tracked by the frontend structure ratchet'
            }))
    };
}

function readBaseline() {
    if (!fs.existsSync(baselinePath)) {
        throw new Error(`Missing baseline file: ${normalizePath(baselinePath)}`);
    }

    return JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
}

function getLineLimit(record, thresholds) {
    return thresholds[record.kind]?.warning ?? Number.POSITIVE_INFINITY;
}

function checkRecordAgainstBaseline(record, baselineEntry, thresholds) {
    const failures = [];
    const warnings = [];
    const threshold = thresholds[record.kind];

    if (!baselineEntry) {
        const warningLimit = getLineLimit(record, thresholds);

        if (record.lines > warningLimit) {
            failures.push(`${record.path}: ${record.lines} lines exceeds new-file limit ${warningLimit}`);
        }

        if (record.kind === 'vue') {
            if (record.sections.template > threshold.templateWarning) {
                failures.push(`${record.path}: template section ${record.sections.template} lines exceeds new-file limit ${threshold.templateWarning}`);
            }
            if (record.sections.script > threshold.scriptWarning) {
                failures.push(`${record.path}: script section ${record.sections.script} lines exceeds new-file limit ${threshold.scriptWarning}`);
            }
            if (record.sections.style > threshold.styleWarning) {
                failures.push(`${record.path}: style section ${record.sections.style} lines exceeds new-file limit ${threshold.styleWarning}`);
            }
        }

        return { failures, warnings };
    }

    if (record.lines > baselineEntry.lines) {
        failures.push(`${record.path}: ${record.lines} lines grew beyond baseline ${baselineEntry.lines}`);
    } else if (record.lines < baselineEntry.lines) {
        warnings.push(`${record.path}: ${baselineEntry.lines - record.lines} line reduction from baseline`);
    }

    if (record.kind === 'vue' && baselineEntry.sections) {
        for (const sectionName of ['template', 'script', 'style']) {
            const sectionLines = record.sections[sectionName] ?? 0;
            const baselineLines = baselineEntry.sections[sectionName] ?? 0;

            if (sectionLines > baselineLines) {
                failures.push(`${record.path}: ${sectionName} section ${sectionLines} lines grew beyond baseline ${baselineLines}`);
            }
        }
    }

    return { failures, warnings };
}

function checkBaseline() {
    const baseline = readBaseline();
    const thresholds = getThresholds(baseline);
    const baselineByPath = new Map((baseline.files ?? []).map(record => [record.path, record]));
    const currentRecords = scanProject();
    const currentPaths = new Set(currentRecords.map(record => record.path));
    const failures = [];
    const warnings = [];

    for (const record of currentRecords) {
        const result = checkRecordAgainstBaseline(record, baselineByPath.get(record.path), thresholds);
        failures.push(...result.failures);
        warnings.push(...result.warnings);
    }

    for (const baselineEntry of baseline.files ?? []) {
        if (!currentPaths.has(baselineEntry.path)) {
            warnings.push(`${baselineEntry.path}: baseline entry no longer exists and can be removed`);
        }
    }

    return {
        scanned: currentRecords.length,
        baselineEntries: baselineByPath.size,
        failures,
        warnings
    };
}

if (args.has('--help') || args.has('-h')) {
    printUsage();
    process.exit(0);
}

if (args.has('--print-baseline')) {
    const baseline = buildBaseline(scanProject(), DEFAULT_THRESHOLDS);
    console.log(`${JSON.stringify(baseline, null, 2)}\n`);
    process.exit(0);
}

try {
    const result = checkBaseline();

    for (const warning of result.warnings) {
        console.warn(`WARN ${warning}`);
    }

    if (result.failures.length > 0) {
        console.error(`FAIL frontend structure gate found ${result.failures.length} issue(s):`);
        for (const failure of result.failures) {
            console.error(`- ${failure}`);
        }
        process.exit(1);
    }

    console.log(`PASS frontend structure gate scanned ${result.scanned} files with ${result.baselineEntries} baseline entries`);
} catch (error) {
    console.error(`FAIL ${error instanceof Error ? error.message : String(error)}`);
    process.exit(1);
}
