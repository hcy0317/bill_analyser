#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const backendRoot = path.resolve(repoRoot, 'src', 'backend');
const baselinePath = path.resolve(scriptDir, 'rust-backend-structure-baseline.json');
const args = new Set(process.argv.slice(2));

const DEFAULT_THRESHOLDS = Object.freeze({
    rust: {
        warning: 600,
        hard: 1000
    }
});

function printUsage() {
    console.log([
        'Usage: node scripts/check-rust-backend-structure.mjs [--print-baseline]',
        '',
        'Checks tracked Rust backend file sizes against a committed calibrated baseline.',
        'Counts non-comment, non-observability Rust lines so documentation/logging-only edits do not raise the ratchet.',
        'Historical oversized files may shrink, but must not grow after calibration.',
        'New tracked Rust backend files must stay within warning thresholds.'
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

function countRustCodeLines(text) {
    if (text.length === 0) {
        return 0;
    }

    const normalized = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
    let blockDepth = 0;
    const visibleLines = [];

    for (const rawLine of normalized.split('\n')) {
        let visible = '';

        for (let index = 0; index < rawLine.length; index += 1) {
            const pair = rawLine.slice(index, index + 2);

            if (blockDepth > 0) {
                if (pair === '/*') {
                    blockDepth += 1;
                    index += 1;
                } else if (pair === '*/') {
                    blockDepth -= 1;
                    index += 1;
                }
                continue;
            }

            if (pair === '/*') {
                blockDepth += 1;
                index += 1;
                continue;
            }

            if (pair === '//') {
                break;
            }

            visible += rawLine[index];
        }

        visibleLines.push(visible.trim());
    }

    let codeLines = 0;
    let deferredCoverageCfg = false;
    let skippingTracingMacro = false;

    for (const visible of visibleLines) {
        if (visible.length === 0) {
            continue;
        }

        if (skippingTracingMacro) {
            if (visible.includes(');')) {
                skippingTracingMacro = false;
            }
            continue;
        }

        if (visible === '#[cfg(not(coverage))]') {
            deferredCoverageCfg = true;
            continue;
        }

        const isTracingMacro = /^tracing::(?:trace|debug|info|warn|error)!\s*\(/.test(visible);

        if (deferredCoverageCfg) {
            if (isTracingMacro) {
                skippingTracingMacro = !visible.includes(');');
                deferredCoverageCfg = false;
                continue;
            }

            codeLines += 1;
            deferredCoverageCfg = false;
        }

        if (/^#\[tracing::instrument\b/.test(visible)) {
            continue;
        }

        if (isTracingMacro) {
            skippingTracingMacro = !visible.includes(');');
            continue;
        }

        codeLines += 1;
    }

    if (deferredCoverageCfg) {
        codeLines += 1;
    }

    return codeLines;
}

function normalizePath(filePath) {
    return path.relative(repoRoot, filePath).split(path.sep).join('/');
}

function resolveRepoPath(repoRelativePath) {
    return path.resolve(repoRoot, ...repoRelativePath.split('/'));
}

function getBackendArea(repoRelativePath) {
    const relativeToBackend = path.relative(backendRoot, resolveRepoPath(repoRelativePath));
    const [area] = relativeToBackend.split(path.sep);
    return area || 'backend';
}

function listTrackedRustBackendFiles() {
    let rawOutput;

    try {
        rawOutput = execFileSync('git', ['ls-files', '-z', '--', 'src/backend'], {
            cwd: repoRoot,
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'pipe']
        });
    } catch (error) {
        const stderr = error?.stderr ? String(error.stderr).trim() : '';
        throw new Error(`Unable to list tracked Rust backend files with git ls-files${stderr ? `: ${stderr}` : ''}`);
    }

    return rawOutput
        .split('\0')
        .filter(Boolean)
        .filter(repoRelativePath => repoRelativePath.endsWith('.rs'))
        .sort((left, right) => left.localeCompare(right));
}

function scanFile(repoRelativePath) {
    const fullPath = resolveRepoPath(repoRelativePath);
    const source = fs.readFileSync(fullPath, 'utf8');

    return {
        path: normalizePath(fullPath),
        kind: 'rust',
        area: getBackendArea(repoRelativePath),
        lines: countRustCodeLines(source),
        physicalLines: countLines(source)
    };
}

function getThresholds(baseline) {
    return {
        ...DEFAULT_THRESHOLDS,
        ...(baseline?.thresholds ?? {})
    };
}

function scanProject() {
    return listTrackedRustBackendFiles()
        .map(scanFile)
        .sort((left, right) => {
            if (right.lines !== left.lines) {
                return right.lines - left.lines;
            }

            return left.path.localeCompare(right.path);
        });
}

function getLineLimit(record, thresholds) {
    return thresholds[record.kind]?.warning ?? Number.POSITIVE_INFINITY;
}

function isOversized(record, thresholds) {
    return record.lines > getLineLimit(record, thresholds);
}

function buildBaseline(records, thresholds) {
    return {
        version: 1,
        description: 'Rust backend non-comment, non-observability line baseline for legacy oversized files. Ratchet after calibration: entries may shrink but must not grow.',
        thresholds,
        generatedBy: 'scripts/check-rust-backend-structure.mjs --print-baseline',
        countMode: 'non-comment-non-observability-rust-lines',
        files: records
            .filter(record => isOversized(record, thresholds))
            .map(record => ({
                ...record,
                reason: 'legacy oversized Rust backend file tracked by the structure ratchet'
            }))
    };
}

function readBaseline() {
    if (!fs.existsSync(baselinePath)) {
        throw new Error(`Missing baseline file: ${normalizePath(baselinePath)}`);
    }

    return JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
}

function checkRecordAgainstBaseline(record, baselineEntry, thresholds) {
    const failures = [];
    const warnings = [];

    if (!baselineEntry) {
        const warningLimit = getLineLimit(record, thresholds);

        if (record.lines > warningLimit) {
            failures.push(`${record.path}: ${record.lines} lines exceeds new-file limit ${warningLimit}`);
        }

        return { failures, warnings };
    }

    if (record.lines > baselineEntry.lines) {
        failures.push(`${record.path}: ${record.lines} lines grew beyond baseline ${baselineEntry.lines}`);
    } else if (record.lines < baselineEntry.lines) {
        warnings.push(`${record.path}: ${baselineEntry.lines - record.lines} line reduction from baseline`);
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
    console.log(JSON.stringify(baseline, null, 2));
    process.exit(0);
}

try {
    const result = checkBaseline();

    for (const warning of result.warnings) {
        console.warn(`WARN ${warning}`);
    }

    if (result.failures.length > 0) {
        console.error(`FAIL Rust backend structure gate found ${result.failures.length} issue(s):`);
        for (const failure of result.failures) {
            console.error(`- ${failure}`);
        }
        process.exit(1);
    }

    console.log(`PASS Rust backend structure gate scanned ${result.scanned} files with ${result.baselineEntries} baseline entries`);
} catch (error) {
    console.error(`FAIL ${error instanceof Error ? error.message : String(error)}`);
    process.exit(1);
}
