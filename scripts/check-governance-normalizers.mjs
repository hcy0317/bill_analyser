#!/usr/bin/env node
import assert from 'node:assert/strict';

import {
    normalizeForgeEvidence,
    normalizeStructureGateQueue,
    parseLcov,
    parseUnifiedDiffChangedLines,
    isBusinessSourcePath,
    isConservativelyDeclarationOnlyRustSource,
    isTypeOnlyTypescriptSource,
    summarizeChangedLineCoverage,
} from './governance-normalizers.mjs';

function testLcovParser() {
    const records = parseLcov([
        'TN:',
        'SF:src/backend/example.rs',
        'DA:10,1',
        'DA:11,0',
        'DA:12,4',
        'end_of_record',
    ].join('\n'));

    assert.equal(records.size, 1);
    assert.equal(records.get('src/backend/example.rs').executable.get(10), 1);
    assert.equal(records.get('src/backend/example.rs').executable.get(11), 0);
}

function testUnifiedDiffParser() {
    const changed = parseUnifiedDiffChangedLines([
        'diff --git a/src/backend/example.rs b/src/backend/example.rs',
        '--- a/src/backend/example.rs',
        '+++ b/src/backend/example.rs',
        '@@ -8,5 +8,6 @@',
        ' context',
        '+added executable',
        '\\ No newline at end of file',
        '-removed executable',
        ' context two',
        '+another added line',
    ].join('\n'));

    assert.deepEqual(
        [...changed.get('src/backend/example.rs')],
        [9, 11],
    );
}

function testChangedCoverageSummary() {
    const lcovText = [
        'SF:src/backend/example.rs',
        'DA:9,1',
        'DA:11,0',
        'DA:12,1',
        'end_of_record',
    ].join('\n');
    const diffText = [
        'diff --git a/src/backend/example.rs b/src/backend/example.rs',
        '--- a/src/backend/example.rs',
        '+++ b/src/backend/example.rs',
        '@@ -8,4 +8,5 @@',
        ' context',
        '+covered changed line',
        ' context two',
        '+uncovered changed line',
    ].join('\n');

    const summary = summarizeChangedLineCoverage({ lcovText, diffText, threshold: 90 });
    assert.equal(summary.executable_changed_lines, 2);
    assert.equal(summary.covered_changed_lines, 1);
    assert.equal(summary.coverage_percent, 50);
    assert.equal(summary.status, 'failed');
    assert.deepEqual(summary.files[0].uncovered_changed_lines, [11]);
}

function testChangedCoverageThresholdIsStrict() {
    const summary = summarizeChangedLineCoverage({
        lcovText: [
            'SF:src/backend/example.rs',
            'DA:9,1',
            'DA:10,1',
            'DA:11,1',
            'DA:12,1',
            'DA:13,1',
            'DA:14,1',
            'DA:15,1',
            'DA:16,1',
            'DA:17,1',
            'DA:18,0',
            'end_of_record',
        ].join('\n'),
        diffText: [
            'diff --git a/src/backend/example.rs b/src/backend/example.rs',
            '--- a/src/backend/example.rs',
            '+++ b/src/backend/example.rs',
            '@@ -8,0 +9,10 @@',
            ...Array.from({ length: 10 }, (_, index) => `+line ${index + 1}`),
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.coverage_percent, 90);
    assert.equal(summary.status, 'failed');
    assert.equal(summary.requirements.coverage_strictly_greater_than_threshold, false);
}

function testChangedCoverageRejectsEmptyDiff() {
    const summary = summarizeChangedLineCoverage({
        lcovText: 'SF:src/backend/example.rs\nDA:1,1\nend_of_record',
        diffText: '',
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.changed_file_count, 0);
    assert.equal(summary.matched_file_count, 0);
    assert.equal(summary.executable_changed_lines, 0);
    assert.equal(summary.status, 'failed');
}

function testChangedCoverageRejectsZeroMatchedFiles() {
    const summary = summarizeChangedLineCoverage({
        lcovText: 'SF:src/backend/other.rs\nDA:9,1\nend_of_record',
        diffText: [
            'diff --git a/src/backend/example.rs b/src/backend/example.rs',
            '--- a/src/backend/example.rs',
            '+++ b/src/backend/example.rs',
            '@@ -8,0 +9 @@',
            '+changed line',
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.matched_file_count, 0);
    assert.equal(summary.status, 'failed');
    assert.equal(summary.requirements.matched_files, false);
}

function testChangedCoverageRejectsZeroExecutableChangedLines() {
    const summary = summarizeChangedLineCoverage({
        lcovText: 'SF:src/backend/example.rs\nDA:20,1\nend_of_record',
        diffText: [
            'diff --git a/src/backend/example.rs b/src/backend/example.rs',
            '--- a/src/backend/example.rs',
            '+++ b/src/backend/example.rs',
            '@@ -8,0 +9 @@',
            '+non executable changed line',
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.matched_file_count, 0);
    assert.equal(summary.executable_changed_lines, 0);
    assert.equal(summary.files[0].exclusion_reason, 'non_executable_change');
    assert.equal(summary.status, 'failed');
    assert.equal(summary.coverage_disposition, 'failed_no_executable_lines');
    assert.equal(summary.requirements.executable_lines, false);
}

function testChangedCoverageMatchesRustAndVuePaths() {
    const summary = summarizeChangedLineCoverage({
        lcovText: [
            'SF:src/backend/example.rs',
            'DA:9,1',
            'end_of_record',
            'SF:src/views/ExampleView.vue',
            'DA:4,1',
            'end_of_record',
        ].join('\n'),
        diffText: [
            'diff --git a/src/backend/example.rs b/src/backend/example.rs',
            '--- a/src/backend/example.rs',
            '+++ b/src/backend/example.rs',
            '@@ -8,0 +9 @@',
            '+covered Rust line',
            'diff --git a/src/web/src/views/ExampleView.vue b/src/web/src/views/ExampleView.vue',
            '--- a/src/web/src/views/ExampleView.vue',
            '+++ b/src/web/src/views/ExampleView.vue',
            '@@ -3,0 +4 @@',
            '+covered Vue line',
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.matched_file_count, 2);
    assert.equal(summary.executable_changed_lines, 2);
    assert.equal(summary.coverage_percent, 100);
    assert.equal(summary.status, 'passed');
    assert.deepEqual(summary.files.map(file => file.matched_lcov_record), [true, true]);
}

function testChangedCoverageRejectsMissingVueSfcRecord() {
    const summary = summarizeChangedLineCoverage({
        lcovText: 'SF:src/views/OtherView.vue\nDA:4,1\nend_of_record',
        diffText: [
            'diff --git a/src/web/src/views/ExampleView.vue b/src/web/src/views/ExampleView.vue',
            '--- a/src/web/src/views/ExampleView.vue',
            '+++ b/src/web/src/views/ExampleView.vue',
            '@@ -3,0 +4 @@',
            '+changed Vue line',
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.changed_vue_file_count, 1);
    assert.equal(summary.requirements.changed_vue_sfc_files, false);
    assert.equal(summary.status, 'failed');
}

function testChangedCoverageRejectsAnyMissingBusinessFile() {
    const summary = summarizeChangedLineCoverage({
        lcovText: [
            'SF:src/web/src/covered.ts',
            'DA:1,1',
            'end_of_record',
        ].join('\n'),
        diffText: [
            'diff --git a/src/web/src/covered.ts b/src/web/src/covered.ts',
            '--- a/src/web/src/covered.ts',
            '+++ b/src/web/src/covered.ts',
            '@@ -0,0 +1 @@',
            '+covered',
            'diff --git a/src/web/src/missing.ts b/src/web/src/missing.ts',
            '--- a/src/web/src/missing.ts',
            '+++ b/src/web/src/missing.ts',
            '@@ -0,0 +1 @@',
            '+silently omitted before this regression test',
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.business_file_count, 2);
    assert.equal(summary.matched_file_count, 1);
    assert.deepEqual(summary.missing_lcov_files, ['src/web/src/missing.ts']);
    assert.equal(summary.requirements.all_business_files_matched, false);
    assert.equal(summary.status, 'failed');
}

function testChangedCoverageReportsIncludedAndExcludedLines() {
    const summary = summarizeChangedLineCoverage({
        lcovText: [
            'SF:src/backend/example.rs',
            'DA:1,1',
            'end_of_record',
        ].join('\n'),
        diffText: [
            'diff --git a/src/backend/example.rs b/src/backend/example.rs',
            '--- a/src/backend/example.rs',
            '+++ b/src/backend/example.rs',
            '@@ -0,0 +1,2 @@',
            '+covered executable',
            '+comment-only line',
            'diff --git a/tests/backend/example.rs b/tests/backend/example.rs',
            '--- a/tests/backend/example.rs',
            '+++ b/tests/backend/example.rs',
            '@@ -0,0 +1 @@',
            '+test line',
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.business_changed_lines, 2);
    assert.equal(summary.included_executable_lines, 1);
    assert.equal(summary.excluded_non_executable_lines, 1);
    assert.equal(summary.files.find(file => file.path.startsWith('tests/')).scope_included, false);
    assert.equal(summary.status, 'passed');
}

function testBusinessSourceClassification() {
    assert.equal(isBusinessSourcePath('src/backend/http/lib.rs'), true);
    assert.equal(isBusinessSourcePath('src/web/src/stores/user.ts'), true);
    assert.equal(isBusinessSourcePath('src/web/src/App.vue'), true);
    assert.equal(isBusinessSourcePath('src/web/src/contracts/rustRouteOwnership.generated.ts'), false);
    assert.equal(isBusinessSourcePath('src/web/src/types/generated.d.ts'), false);
    assert.equal(isBusinessSourcePath('tests/web/store.test.ts'), false);
    assert.equal(isBusinessSourcePath('src/web/package.json'), false);
    assert.equal(isBusinessSourcePath('src/backend/db/build.rs'), false);
    assert.equal(isBusinessSourcePath('src/backend/http/routes/tests.rs'), false);
    assert.equal(isBusinessSourcePath('src/backend/db/import_staging/tests_filters/core.rs'), false);
    assert.equal(isBusinessSourcePath('src/backend/http/matching_routes_contract_tests.rs'), false);
    assert.equal(isBusinessSourcePath('src/backend/http/router_test.rs'), false);
    assert.equal(isBusinessSourcePath('src/backend/http/latest.rs'), true);
}

function testChangedCoverageExcludesPureTypeSourceWithReason() {
    const filePath = 'src/web/src/lib/services/contracts.ts';
    const sourceText = [
        "import type { AxiosRequestConfig } from 'axios';",
        'export interface RequestContract {',
        '  config: AxiosRequestConfig;',
        '}',
        'export type RequestId = string;',
    ].join('\n');
    const summary = summarizeChangedLineCoverage({
        lcovText: `SF:${filePath}\nend_of_record`,
        diffText: [
            `diff --git a/${filePath} b/${filePath}`,
            '--- /dev/null',
            `+++ b/${filePath}`,
            '@@ -0,0 +1,5 @@',
            ...sourceText.split('\n').map(line => `+${line}`),
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
        sourceTextByPath: { [filePath]: sourceText },
    });

    assert.equal(isTypeOnlyTypescriptSource(filePath, sourceText), true);
    assert.equal(summary.files[0].exclusion_reason, 'type_only_source');
    assert.equal(summary.coverage_eligible_file_count, 0);
    assert.equal(summary.coverage_disposition, 'failed_no_executable_lines');
    assert.equal(summary.status, 'failed');
}

function testChangedCoverageExcludesCommentOnlyChangeWithReason() {
    const filePath = 'src/backend/http/example.rs';
    const summary = summarizeChangedLineCoverage({
        lcovText: '',
        diffText: [
            `diff --git a/${filePath} b/${filePath}`,
            `--- a/${filePath}`,
            `+++ b/${filePath}`,
            '@@ -4,0 +5,2 @@',
            '+// Explain the invariant.',
            '+/// Explain the public contract.',
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });

    assert.equal(summary.files[0].exclusion_reason, 'comment_only_change');
    assert.equal(summary.status, 'failed');

    const inlineBlockWithCode = summarizeChangedLineCoverage({
        lcovText: '',
        diffText: [
            `diff --git a/${filePath} b/${filePath}`,
            `--- a/${filePath}`,
            `+++ b/${filePath}`,
            '@@ -4,0 +5 @@',
            '+/* misleading */ execute_runtime();',
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
    });
    assert.equal(inlineBlockWithCode.files[0].exclusion_reason, null);
    assert.deepEqual(inlineBlockWithCode.missing_lcov_files, [filePath]);
}

function testChangedCoverageReportsDeclarationOnlyRustFacade() {
    const filePath = 'src/backend/db/postgres.rs';
    const sourceText = [
        '#![allow(clippy::all)]',
        'pub(crate) mod migration_manifest;',
        'pub use migration_manifest::{embedded_migrations, migration_count};',
        'include!("postgres/runtime.rs");',
    ].join('\n');
    const summary = summarizeChangedLineCoverage({
        lcovText: '',
        diffText: [
            `diff --git a/${filePath} b/${filePath}`,
            `--- a/${filePath}`,
            `+++ b/${filePath}`,
            '@@ -0,0 +1,4 @@',
            ...sourceText.split('\n').map(line => `+${line}`),
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
        sourceTextByPath: { [filePath]: sourceText },
    });

    assert.equal(isConservativelyDeclarationOnlyRustSource(filePath, sourceText), true);
    assert.equal(summary.files[0].exclusion_reason, 'declaration_only_source');
    assert.equal(summary.missing_lcov_file_count, 0);
    assert.equal(summary.status, 'failed');
}

function testChangedCoverageExcludesRustConstDataInventories() {
    const filePath = 'src/backend/core/runtime_governance/ownership/routes.rs';
    const sourceText = [
        'use super::EndpointOwnership;',
        'macro_rules! route {',
        '    ($method:literal) => { EndpointOwnership { method: $method, pattern: "/api/health" } };',
        '}',
        'pub(super) const ROUTES: &[EndpointOwnership] = &[',
        '    route!("GET"),',
        '];',
    ].join('\n');
    const summary = summarizeChangedLineCoverage({
        lcovText: '',
        diffText: [
            `diff --git a/${filePath} b/${filePath}`,
            '--- /dev/null',
            `+++ b/${filePath}`,
            `@@ -0,0 +1,${sourceText.split('\n').length} @@`,
            ...sourceText.split('\n').map(line => `+${line}`),
        ].join('\n'),
        threshold: 90,
        requireMatchedFiles: true,
        requireExecutableLines: true,
        sourceTextByPath: { [filePath]: sourceText },
    });

    assert.equal(isConservativelyDeclarationOnlyRustSource(filePath, sourceText), true);
    assert.equal(summary.files[0].exclusion_reason, 'declaration_only_source');
    assert.equal(summary.missing_lcov_file_count, 0);

    const runtimeSource = `${sourceText}\npub fn routes() -> &'static [EndpointOwnership] { ROUTES }`;
    assert.equal(isConservativelyDeclarationOnlyRustSource(filePath, runtimeSource), false);
}

function testForgeEvidenceNormalizer() {
    const evidence = normalizeForgeEvidence({
        target_forge: 'gitea',
        remote: 'bill_analyser',
        base_ref: 'main',
        head_ref: 'codex/example',
        pr: { number: 178, html_url: 'https://git.hehome.xyz/pulls/178', state: 'closed' },
        run: { id: 14058, status: 'completed', conclusion: 'success' },
        merge: {
            merged: true,
            method: 'squash',
            title: 'test(scope): title (#178)',
            merge_commit_sha: 'abc123',
        },
        divergence: { status: 'none', notes: ['same target forge'] },
    });

    assert.equal(evidence.ci.status, 'passed');
    assert.equal(evidence.ci.run_id, 14058);
    assert.equal(evidence.merge.status, 'merged');
    assert.equal(evidence.merge.commit, 'abc123');

    const normalizedAgain = normalizeForgeEvidence({
        ci: { status: 'passed' },
        merge: { status: 'merged' },
    });

    assert.equal(normalizedAgain.ci.status, 'passed');
    assert.equal(normalizedAgain.merge.status, 'merged');
}

function testStructureQueueNormalizer() {
    const queue = normalizeStructureGateQueue({
        rustOutput: [
            'WARN src/backend/db/taxonomy/postgres_reads.rs: 32 line reduction from baseline',
            'FAIL Rust backend structure gate found 2 issue(s):',
            '- src/backend/http/import_routes/stage_handlers.rs: 2998 lines grew beyond baseline 2950',
        ].join('\n'),
        frontendOutput: [
            'WARN src/locales/helpers.ts: 9 line reduction from baseline',
            'FAIL frontend structure gate found 1 issue(s):',
            '- src/web/src/lib/services.ts: 2342 lines grew beyond baseline 2214',
        ].join('\n'),
    });

    assert.equal(queue.items.length, 4);
    assert.deepEqual(
        queue.items.map(item => `${item.gate}:${item.severity}:${item.path}`),
        [
            'rust-backend-structure:warn:src/backend/db/taxonomy/postgres_reads.rs',
            'rust-backend-structure:fail:src/backend/http/import_routes/stage_handlers.rs',
            'frontend-structure:warn:src/web/src/locales/helpers.ts',
            'frontend-structure:fail:src/web/src/lib/services.ts',
        ],
    );
}

testLcovParser();
testUnifiedDiffParser();
testChangedCoverageSummary();
testChangedCoverageThresholdIsStrict();
testChangedCoverageRejectsEmptyDiff();
testChangedCoverageRejectsZeroMatchedFiles();
testChangedCoverageRejectsZeroExecutableChangedLines();
testChangedCoverageMatchesRustAndVuePaths();
testChangedCoverageRejectsMissingVueSfcRecord();
testChangedCoverageRejectsAnyMissingBusinessFile();
testChangedCoverageReportsIncludedAndExcludedLines();
testBusinessSourceClassification();
testChangedCoverageExcludesPureTypeSourceWithReason();
testChangedCoverageExcludesCommentOnlyChangeWithReason();
testChangedCoverageReportsDeclarationOnlyRustFacade();
testChangedCoverageExcludesRustConstDataInventories();
testForgeEvidenceNormalizer();
testStructureQueueNormalizer();

console.log('PASS governance normalizer self-tests');
