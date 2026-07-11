#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const repoRoot = path.resolve(scriptDir, '..');

const CI_STATUS = new Set(['passed', 'running', 'failed', 'skipped', 'blocked']);
const MERGE_STATUS = new Set(['merged', 'skipped', 'blocked']);

function normalizePath(input) {
    return String(input ?? '')
        .replace(/\\/g, '/')
        .replace(/^\.\//, '')
        .replace(/^b\//, '')
        .replace(/^a\//, '');
}

function normalizeStructurePath(input, prefix = '') {
    const normalized = normalizePath(input);
    if (!prefix) {
        return normalized;
    }
    const normalizedPrefix = normalizePath(prefix).replace(/\/$/, '');
    if (normalized === normalizedPrefix || normalized.startsWith(`${normalizedPrefix}/`)) {
        return normalized;
    }
    return `${normalizedPrefix}/${normalized}`;
}

function pathCandidates(filePath) {
    const normalized = normalizePath(filePath);
    const candidates = new Set([normalized]);
    const repoRelative = normalizePath(path.relative(repoRoot, path.resolve(filePath)));
    if (repoRelative && !repoRelative.startsWith('..')) {
        candidates.add(repoRelative);
    }
    return candidates;
}

function parseLcov(lcovText) {
    const files = new Map();
    let current = null;

    for (const rawLine of String(lcovText ?? '').split(/\r?\n/)) {
        const line = rawLine.trim();
        if (line.startsWith('SF:')) {
            current = {
                path: normalizePath(line.slice(3)),
                executable: new Map(),
            };
            continue;
        }
        if (line === 'end_of_record') {
            if (current) {
                files.set(current.path, current);
                current = null;
            }
            continue;
        }
        if (!current || !line.startsWith('DA:')) {
            continue;
        }
        const [lineNumberRaw, hitsRaw] = line.slice(3).split(',');
        const lineNumber = Number.parseInt(lineNumberRaw, 10);
        const hits = Number.parseInt(hitsRaw, 10);
        if (Number.isInteger(lineNumber) && Number.isFinite(hits)) {
            current.executable.set(lineNumber, Math.max(0, hits));
        }
    }

    if (current) {
        files.set(current.path, current);
    }
    return files;
}

function parseUnifiedDiffChangedLines(diffText) {
    const files = new Map();
    let currentPath = null;
    let newLine = 0;

    for (const rawLine of String(diffText ?? '').split(/\r?\n/)) {
        if (rawLine.startsWith('diff --git ')) {
            currentPath = null;
            newLine = 0;
            continue;
        }
        if (rawLine.startsWith('+++ ')) {
            const nextPath = rawLine.slice(4).trim();
            currentPath = nextPath === '/dev/null' ? null : normalizePath(nextPath);
            if (currentPath && !files.has(currentPath)) {
                files.set(currentPath, new Set());
            }
            continue;
        }
        if (!currentPath) {
            continue;
        }
        const hunk = rawLine.match(/^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
        if (hunk) {
            newLine = Number.parseInt(hunk[1], 10);
            continue;
        }
        if (rawLine.startsWith('+') && !rawLine.startsWith('+++')) {
            files.get(currentPath).add(newLine);
            newLine += 1;
            continue;
        }
        if (rawLine.startsWith('-') && !rawLine.startsWith('---')) {
            continue;
        }
        if (rawLine.startsWith('\\')) {
            continue;
        }
        if (rawLine.length > 0 || rawLine === '') {
            newLine += 1;
        }
    }
    return files;
}

function findLcovRecord(lcovFiles, diffPath) {
    const candidates = pathCandidates(diffPath);
    for (const candidate of candidates) {
        if (lcovFiles.has(candidate)) {
            return lcovFiles.get(candidate);
        }
    }
    for (const record of lcovFiles.values()) {
        const normalized = normalizePath(record.path);
        for (const candidate of candidates) {
            if (normalized.endsWith(candidate) || candidate.endsWith(normalized)) {
                return record;
            }
        }
    }
    return null;
}

function summarizeChangedLineCoverage({
    lcovText,
    diffText,
    threshold = 90,
    requireMatchedFiles = false,
    requireExecutableLines = false,
}) {
    const lcovFiles = parseLcov(lcovText);
    const changedLinesByFile = parseUnifiedDiffChangedLines(diffText);
    const files = [];
    let matchedFileCount = 0;
    let executableChangedLines = 0;
    let coveredChangedLines = 0;

    for (const [filePath, changedLines] of changedLinesByFile.entries()) {
        const record = findLcovRecord(lcovFiles, filePath);
        if (record) {
            matchedFileCount += 1;
        }
        const executable = [];
        const covered = [];
        const uncovered = [];

        for (const line of [...changedLines].sort((left, right) => left - right)) {
            const hits = record?.executable.get(line);
            if (hits === undefined) {
                continue;
            }
            executable.push(line);
            if (hits > 0) {
                covered.push(line);
            } else {
                uncovered.push(line);
            }
        }

        executableChangedLines += executable.length;
        coveredChangedLines += covered.length;
        files.push({
            path: filePath,
            matched_lcov_record: record !== null,
            lcov_path: record?.path ?? null,
            changed_lines: [...changedLines].sort((left, right) => left - right),
            executable_changed_lines: executable,
            covered_changed_lines: covered,
            uncovered_changed_lines: uncovered,
            coverage_percent: executable.length === 0
                ? null
                : Number(((covered.length / executable.length) * 100).toFixed(2)),
        });
    }

    const coveragePercent = executableChangedLines === 0
        ? null
        : Number(((coveredChangedLines / executableChangedLines) * 100).toFixed(2));
    const changedVueFiles = files.filter(file => file.path.toLowerCase().endsWith('.vue'));
    const vueSfcPassed = changedVueFiles.every(file => (
        file.matched_lcov_record
        && file.executable_changed_lines.length > 0
        && file.coverage_percent > threshold
    ));
    const coveragePassed = coveragePercent !== null && coveragePercent > threshold;
    const matchedFilesPassed = !requireMatchedFiles || matchedFileCount > 0;
    const executableLinesPassed = !requireExecutableLines || executableChangedLines > 0;
    return {
        threshold,
        changed_file_count: changedLinesByFile.size,
        matched_file_count: matchedFileCount,
        changed_vue_file_count: changedVueFiles.length,
        executable_changed_lines: executableChangedLines,
        covered_changed_lines: coveredChangedLines,
        uncovered_changed_lines: executableChangedLines - coveredChangedLines,
        coverage_percent: coveragePercent,
        requirements: {
            coverage_strictly_greater_than_threshold: coveragePassed,
            matched_files: matchedFilesPassed,
            executable_lines: executableLinesPassed,
            changed_vue_sfc_files: vueSfcPassed,
        },
        status: coveragePassed && matchedFilesPassed && executableLinesPassed && vueSfcPassed
            ? 'passed'
            : 'failed',
        files,
    };
}

function normalizeCiStatus(raw) {
    const status = String(raw?.status ?? '').toLowerCase();
    const conclusion = String(raw?.conclusion ?? '').toLowerCase();
    if (CI_STATUS.has(status)) {
        return status;
    }
    if (CI_STATUS.has(conclusion)) {
        return conclusion;
    }
    if (status === 'completed' && conclusion === 'success') {
        return 'passed';
    }
    if (!status && conclusion === 'success') {
        return 'passed';
    }
    if (['queued', 'waiting', 'requested', 'in_progress', 'running'].includes(status)) {
        return 'running';
    }
    if (status === 'skipped' || conclusion === 'skipped') {
        return 'skipped';
    }
    if (status === 'blocked' || conclusion === 'blocked' || raw?.blocked_reason) {
        return 'blocked';
    }
    if (status || conclusion) {
        return 'failed';
    }
    return 'skipped';
}

function normalizeMergeStatus(raw) {
    if (raw?.merged === true || raw?.status === 'merged') {
        return 'merged';
    }
    if (raw?.blocked_reason || raw?.status === 'blocked') {
        return 'blocked';
    }
    return 'skipped';
}

function normalizeForgeEvidence(input) {
    const normalized = {
        schema_version: 1,
        target_forge: input.target_forge ?? input.targetForge ?? 'unknown',
        remote: input.remote ?? 'unknown',
        base_ref: input.base_ref ?? input.baseRef ?? null,
        head_ref: input.head_ref ?? input.headRef ?? null,
        pr: {
            number: input.pr?.number ?? null,
            url: input.pr?.url ?? input.pr?.html_url ?? input.pr_url ?? null,
            state: input.pr?.state ?? null,
        },
        ci: {
            status: normalizeCiStatus(input.ci ?? input.run ?? {}),
            run_id: input.ci?.run_id ?? input.ci?.id ?? input.run?.id ?? null,
            conclusion: input.ci?.conclusion ?? input.run?.conclusion ?? null,
        },
        merge: {
            status: normalizeMergeStatus(input.merge ?? {}),
            method: input.merge?.method ?? input.merge?.Do ?? null,
            title: input.merge?.title ?? input.merge?.MergeTitleField ?? null,
            commit: input.merge?.commit ?? input.merge?.merge_commit_sha ?? null,
        },
        divergence: {
            status: input.divergence?.status ?? 'not_checked',
            notes: input.divergence?.notes ?? [],
        },
        generated_at: new Date().toISOString(),
    };

    if (!CI_STATUS.has(normalized.ci.status)) {
        throw new Error(`Invalid normalized CI status: ${normalized.ci.status}`);
    }
    if (!MERGE_STATUS.has(normalized.merge.status)) {
        throw new Error(`Invalid normalized merge status: ${normalized.merge.status}`);
    }
    return normalized;
}

function parseStructureGateOutput(text, gate, pathPrefix = '') {
    const items = [];
    for (const rawLine of String(text ?? '').split(/\r?\n/)) {
        const line = rawLine.trim();
        const failure = line.match(/^-\s+(.+?):\s+(.+)$/);
        const warning = line.match(/^WARN\s+(.+?):\s+(.+)$/);
        if (failure) {
            items.push({
                gate,
                severity: 'fail',
                path: normalizeStructurePath(failure[1], pathPrefix),
                finding: failure[2],
            });
        } else if (warning) {
            items.push({
                gate,
                severity: 'warn',
                path: normalizeStructurePath(warning[1], pathPrefix),
                finding: warning[2],
            });
        }
    }
    return items;
}

function normalizeStructureGateQueue({ rustOutput = '', frontendOutput = '' }) {
    const items = [
        ...parseStructureGateOutput(rustOutput, 'rust-backend-structure'),
        ...parseStructureGateOutput(frontendOutput, 'frontend-structure', 'src/web'),
    ];
    return {
        schema_version: 1,
        generated_at: new Date().toISOString(),
        policy: 'structure failures are backlog input; do not loosen baselines',
        items,
    };
}

function readFileArgument(args, name) {
    const index = args.indexOf(name);
    if (index === -1 || !args[index + 1]) {
        throw new Error(`Missing ${name} <path>`);
    }
    return fs.readFileSync(path.resolve(args[index + 1]), 'utf8');
}

function numberArgument(args, name, fallback) {
    const index = args.indexOf(name);
    if (index === -1 || !args[index + 1]) {
        return fallback;
    }
    const value = Number(args[index + 1]);
    if (!Number.isFinite(value)) {
        throw new Error(`Invalid ${name}: ${args[index + 1]}`);
    }
    return value;
}

function main(argv = process.argv.slice(2)) {
    const [command, ...args] = argv;
    if (command === 'changed-coverage') {
        const lcovText = readFileArgument(args, '--lcov');
        const diffText = readFileArgument(args, '--diff');
        const threshold = numberArgument(args, '--threshold', 90);
        const summary = summarizeChangedLineCoverage({
            lcovText,
            diffText,
            threshold,
            requireMatchedFiles: args.includes('--require-matched-files'),
            requireExecutableLines: args.includes('--require-executable-lines'),
        });
        console.log(JSON.stringify(summary, null, 2));
        if (summary.status !== 'passed') {
            process.exitCode = 1;
        }
        return;
    }
    if (command === 'forge-evidence') {
        const input = JSON.parse(readFileArgument(args, '--input'));
        console.log(JSON.stringify(normalizeForgeEvidence(input), null, 2));
        return;
    }
    if (command === 'structure-queue') {
        const rustOutput = args.includes('--rust-output')
            ? readFileArgument(args, '--rust-output')
            : '';
        const frontendOutput = args.includes('--frontend-output')
            ? readFileArgument(args, '--frontend-output')
            : '';
        console.log(JSON.stringify(normalizeStructureGateQueue({ rustOutput, frontendOutput }), null, 2));
        return;
    }
    console.error([
        'Usage:',
        '  node scripts/governance-normalizers.mjs changed-coverage --lcov <file> --diff <file> [--threshold 90] [--require-matched-files] [--require-executable-lines]',
        '  node scripts/governance-normalizers.mjs forge-evidence --input <json>',
        '  node scripts/governance-normalizers.mjs structure-queue [--rust-output <log>] [--frontend-output <log>]',
    ].join('\n'));
    process.exitCode = 2;
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
    main();
}

export {
    normalizeForgeEvidence,
    normalizePath,
    normalizeStructureGateQueue,
    parseLcov,
    parseUnifiedDiffChangedLines,
    summarizeChangedLineCoverage,
};
