#!/usr/bin/env node
import assert from 'node:assert/strict';

import {
    normalizeForgeEvidence,
    normalizeStructureGateQueue,
    parseLcov,
    parseUnifiedDiffChangedLines,
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
testForgeEvidenceNormalizer();
testStructureQueueNormalizer();

console.log('PASS governance normalizer self-tests');
