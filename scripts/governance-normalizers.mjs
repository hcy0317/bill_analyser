#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { createRequire } from 'node:module';
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

function parseUnifiedDiffAddedText(diffText) {
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
                files.set(currentPath, new Map());
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
            files.get(currentPath).set(newLine, rawLine.slice(1));
            newLine += 1;
            continue;
        }
        if (rawLine.startsWith('-') && !rawLine.startsWith('---')) {
            continue;
        }
        if (!rawLine.startsWith('\\')) {
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

function intrinsicSourceExclusionReason(filePath) {
    const normalized = normalizePath(filePath).toLowerCase();
    if (/^src\/backend\/.+\.rs$/.test(normalized)) {
        const basename = path.posix.basename(normalized);
        if (basename === 'build.rs') {
            return 'rust_build_script';
        }
        if (
            /\/(?:tests?|test_[^/]*|tests_[^/]*)\//.test(normalized)
            || /^(?:tests?(?:_[^.]+)?|.+_tests?)\.rs$/.test(basename)
        ) {
            return 'test_source';
        }
        return null;
    }
    if (/^src\/web\/src\/.+\.(?:[cm]?[jt]sx?|vue)$/.test(normalized)) {
        if (/^src\/web\/src\/contracts\/.+\.generated\.ts$/.test(normalized)) {
            return 'generated_source';
        }
        if (normalized.endsWith('.d.ts')) {
            return 'type_declaration_source';
        }
        if (/\.(?:test|spec)\.[cm]?[jt]sx?$/.test(normalized)) {
            return 'test_source';
        }
        return null;
    }
    return 'not_business_source';
}

function isBusinessSourcePath(filePath) {
    return intrinsicSourceExclusionReason(filePath) === null;
}

let typescriptModule;

function loadTypescript() {
    if (typescriptModule !== undefined) {
        return typescriptModule;
    }
    try {
        const webRequire = createRequire(path.join(repoRoot, 'src', 'web', 'package.json'));
        typescriptModule = webRequire('typescript');
    } catch {
        typescriptModule = null;
    }
    return typescriptModule;
}

function isConservativelyTypeOnlyTypescriptSource(sourceText) {
    if (typeof sourceText !== 'string' || sourceText.includes('/*')) {
        return false;
    }
    let braceDepth = 0;
    let sawDeclaration = false;
    const allowedStart = /^(?:export\s+)?(?:type|interface|declare)\b|^import\s+type\b|^export\s*\{\s*type\b/;
    for (const rawLine of sourceText.split(/\r?\n/)) {
        const trimmed = rawLine.trim();
        if (trimmed === '' || trimmed.startsWith('//')) {
            continue;
        }
        if (braceDepth === 0 && !trimmed.startsWith('}')) {
            if (!allowedStart.test(trimmed)) {
                return false;
            }
            sawDeclaration = true;
        }
        const opens = (rawLine.match(/\{/g) ?? []).length;
        const closes = (rawLine.match(/\}/g) ?? []).length;
        braceDepth += opens - closes;
        if (braceDepth < 0) {
            return false;
        }
    }
    return sawDeclaration && braceDepth === 0;
}

function isTypeOnlyTypescriptSource(filePath, sourceText) {
    if (!/\.[cm]?tsx?$/i.test(filePath) || filePath.toLowerCase().endsWith('.d.ts')) {
        return filePath.toLowerCase().endsWith('.d.ts');
    }
    const ts = loadTypescript();
    if (sourceText === null || sourceText === undefined) {
        return false;
    }
    if (!ts) {
        return isConservativelyTypeOnlyTypescriptSource(sourceText);
    }
    const source = ts.createSourceFile(
        filePath,
        sourceText,
        ts.ScriptTarget.Latest,
        true,
        /tsx$/i.test(filePath) ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
    );
    const hasDeclareModifier = statement => (
        ts.canHaveModifiers(statement)
        && (ts.getModifiers(statement) ?? []).some(modifier => modifier.kind === ts.SyntaxKind.DeclareKeyword)
    );
    const isTypeOnlyStatement = statement => {
        if (
            ts.isInterfaceDeclaration(statement)
            || ts.isTypeAliasDeclaration(statement)
            || ts.isImportEqualsDeclaration(statement) && statement.isTypeOnly
        ) {
            return true;
        }
        if (ts.isImportDeclaration(statement)) {
            const clause = statement.importClause;
            if (!clause) {
                return false;
            }
            if (clause.isTypeOnly) {
                return true;
            }
            return (
                !clause.name
                && clause.namedBindings
                && ts.isNamedImports(clause.namedBindings)
                && clause.namedBindings.elements.every(element => element.isTypeOnly)
            );
        }
        if (ts.isExportDeclaration(statement)) {
            if (statement.isTypeOnly) {
                return true;
            }
            return (
                statement.exportClause
                && ts.isNamedExports(statement.exportClause)
                && statement.exportClause.elements.every(element => element.isTypeOnly)
            );
        }
        return hasDeclareModifier(statement) || ts.isEmptyStatement(statement);
    };
    return source.statements.length === 0 || source.statements.every(isTypeOnlyStatement);
}

function addedLinesAreCommentOnly(addedLines) {
    if (!addedLines || addedLines.size === 0) {
        return false;
    }
    return [...addedLines.values()].every(line => {
        const trimmed = line.trim();
        return (
            trimmed === ''
            || trimmed.startsWith('//')
        );
    });
}

function isConservativelyDeclarationOnlyRustSource(filePath, sourceText) {
    if (!filePath.toLowerCase().endsWith('.rs') || typeof sourceText !== 'string' || sourceText.includes('/*')) {
        return false;
    }
    let statement = '';
    let sawDeclaration = false;
    let macroBraceDepth = 0;
    const allowed = [
        /^(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*;$/,
        /^(?:pub(?:\([^)]*\))?\s+)?use\s+.+;$/,
        /^(?:pub(?:\([^)]*\))?\s+)?const\s+[A-Za-z_][A-Za-z0-9_]*\s*:\s*.+\s*=\s*.+;$/,
        /^include!\s*\(.+\)\s*;$/,
        /^extern\s+crate\s+.+;$/,
    ];
    for (const rawLine of sourceText.split(/\r?\n/)) {
        const trimmed = rawLine.trim();
        if (trimmed === '' || trimmed.startsWith('//') || /^#!?\[.*\]$/.test(trimmed)) {
            continue;
        }
        if (macroBraceDepth > 0) {
            macroBraceDepth += (rawLine.match(/\{/g) ?? []).length;
            macroBraceDepth -= (rawLine.match(/\}/g) ?? []).length;
            if (macroBraceDepth < 0) {
                return false;
            }
            continue;
        }
        if (/^macro_rules!\s*[A-Za-z_][A-Za-z0-9_]*\s*\{/.test(trimmed)) {
            macroBraceDepth = (rawLine.match(/\{/g) ?? []).length
                - (rawLine.match(/\}/g) ?? []).length;
            if (macroBraceDepth < 0) {
                return false;
            }
            sawDeclaration = true;
            continue;
        }
        statement = `${statement} ${trimmed}`.trim();
        if (!trimmed.endsWith(';')) {
            continue;
        }
        if (!allowed.some(pattern => pattern.test(statement))) {
            return false;
        }
        sawDeclaration = true;
        statement = '';
    }
    return sawDeclaration && statement === '' && macroBraceDepth === 0;
}

function sourceTextForPath(filePath, sourceTextByPath) {
    if (sourceTextByPath instanceof Map && sourceTextByPath.has(filePath)) {
        return sourceTextByPath.get(filePath);
    }
    if (sourceTextByPath && Object.hasOwn(sourceTextByPath, filePath)) {
        return sourceTextByPath[filePath];
    }
    try {
        return fs.readFileSync(path.resolve(repoRoot, filePath), 'utf8');
    } catch {
        return null;
    }
}

function summarizeChangedLineCoverage({
    lcovText,
    diffText,
    threshold = 90,
    requireMatchedFiles = false,
    requireExecutableLines = false,
    sourceTextByPath = null,
}) {
    const lcovFiles = parseLcov(lcovText);
    const changedLinesByFile = parseUnifiedDiffChangedLines(diffText);
    const addedTextByFile = parseUnifiedDiffAddedText(diffText);
    const files = [];
    let matchedFileCount = 0;
    let businessFileCount = 0;
    let coverageEligibleFileCount = 0;
    let businessChangedLines = 0;
    let executableChangedLines = 0;
    let coveredChangedLines = 0;

    for (const [filePath, changedLines] of changedLinesByFile.entries()) {
        const intrinsicExclusion = intrinsicSourceExclusionReason(filePath);
        const businessCandidate = intrinsicExclusion === null;
        const record = businessCandidate ? findLcovRecord(lcovFiles, filePath) : null;
        let exclusionReason = intrinsicExclusion;
        if (businessCandidate) {
            const sourceText = sourceTextForPath(filePath, sourceTextByPath);
            if (isTypeOnlyTypescriptSource(filePath, sourceText)) {
                exclusionReason = 'type_only_source';
            } else if (isConservativelyDeclarationOnlyRustSource(filePath, sourceText)) {
                exclusionReason = 'declaration_only_source';
            } else if (addedLinesAreCommentOnly(addedTextByFile.get(filePath))) {
                exclusionReason = 'comment_only_change';
            }
        }
        if (businessCandidate) {
            businessFileCount += 1;
            businessChangedLines += changedLines.size;
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

        if (businessCandidate && exclusionReason === null && record && executable.length === 0) {
            exclusionReason = 'non_executable_change';
        }
        const scopeIncluded = businessCandidate && exclusionReason === null;
        if (scopeIncluded) {
            coverageEligibleFileCount += 1;
        }
        if (scopeIncluded && record) {
            matchedFileCount += 1;
        }

        if (scopeIncluded) {
            executableChangedLines += executable.length;
            coveredChangedLines += covered.length;
        }
        files.push({
            path: filePath,
            business_candidate: businessCandidate,
            scope_included: scopeIncluded,
            exclusion_reason: exclusionReason,
            matched_lcov_record: scopeIncluded && record !== null,
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
    const changedVueFiles = files.filter(file => (
        file.scope_included && file.path.toLowerCase().endsWith('.vue')
    ));
    const missingLcovFiles = files.filter(file => (
        file.scope_included && !file.matched_lcov_record
    ));
    const vueSfcPassed = changedVueFiles.every(file => (
        file.matched_lcov_record
        && file.executable_changed_lines.length > 0
        && file.coverage_percent > threshold
    ));
    const coveragePassed = coveragePercent !== null && coveragePercent > threshold;
    const matchedFilesPassed = !requireMatchedFiles || (
        businessFileCount > 0
        && matchedFileCount === coverageEligibleFileCount
        && missingLcovFiles.length === 0
    );
    const executableLinesPassed = !requireExecutableLines || executableChangedLines > 0;
    return {
        threshold,
        changed_file_count: changedLinesByFile.size,
        business_file_count: businessFileCount,
        coverage_eligible_file_count: coverageEligibleFileCount,
        matched_file_count: matchedFileCount,
        missing_lcov_file_count: missingLcovFiles.length,
        missing_lcov_files: missingLcovFiles.map(file => file.path),
        changed_vue_file_count: changedVueFiles.length,
        business_changed_lines: businessChangedLines,
        included_executable_lines: executableChangedLines,
        excluded_non_executable_lines: businessChangedLines - executableChangedLines,
        executable_changed_lines: executableChangedLines,
        covered_changed_lines: coveredChangedLines,
        uncovered_changed_lines: executableChangedLines - coveredChangedLines,
        coverage_percent: coveragePercent,
        coverage_disposition: executableChangedLines === 0 ? 'failed_no_executable_lines' : 'measured',
        requirements: {
            coverage_strictly_greater_than_threshold: coveragePassed,
            all_business_files_matched: matchedFilesPassed,
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
    isBusinessSourcePath,
    isConservativelyTypeOnlyTypescriptSource,
    isConservativelyDeclarationOnlyRustSource,
    isTypeOnlyTypescriptSource,
    summarizeChangedLineCoverage,
};
