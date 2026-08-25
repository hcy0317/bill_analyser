#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import {
    changedPathsFromGit,
    classifyE2ePaths,
    readDiffRefs,
} from './classify-e2e-scope.mjs';

const relevantPullRequestCases = [
    'src/backend/http/bin/bill_http_server.rs',
    'src/web/src/features/action-center/InsightsPage.tsx',
    'tests/backend/db/postgres_embedded_migrations.rs',
    'tests/fixtures/import/sample.csv',
    'tests/scripts/workflow-contract.test.mjs',
    'tests/real_sample_manifest.json',
    'tests/web/action-center.test.tsx',
    '.gitea/workflows/ci.yml',
    'scripts/run-e2e-ci.mjs',
    'Cargo.toml',
    'Cargo.lock',
    '一键启动.ps1',
    '停止服务器.bat',
];

for (const changedPath of relevantPullRequestCases) {
    const result = classifyE2ePaths('pull_request', [changedPath]);
    assert.equal(result.required, true, `${changedPath} must require E2E`);
    assert.deepEqual(result.matched_paths, [changedPath.replaceAll('\\', '/')]);
}

const configOnly = classifyE2ePaths('pull_request', [
    '.mcp.json',
    '.claude/settings.local.json',
    'docs/AI_WORKFLOW.md',
]);
assert.equal(configOnly.required, false);
assert.deepEqual(configOnly.matched_paths, []);
assert.equal(configOnly.reason, 'no_e2e_relevant_paths');

const mixed = classifyE2ePaths('pull_request', [
    'docs/AI_WORKFLOW.md',
    'src\\web\\src\\App.tsx',
]);
assert.equal(mixed.required, true);
assert.deepEqual(mixed.matched_paths, ['src/web/src/App.tsx']);

const mainPush = classifyE2ePaths('push', ['docs/AI_WORKFLOW.md']);
assert.equal(mainPush.required, true);
assert.equal(mainPush.reason, 'event_requires_full_e2e:push');

const manual = classifyE2ePaths('workflow_dispatch', []);
assert.equal(manual.required, true);
assert.equal(manual.reason, 'event_requires_full_e2e:workflow_dispatch');

function runGit(repoRoot, args) {
    const result = spawnSync('git', args, {
        cwd: repoRoot,
        encoding: 'utf8',
        windowsHide: true,
    });
    if (result.status !== 0) {
        throw new Error(`git ${args.join(' ')} failed: ${result.stderr.trim()}`);
    }
    return result.stdout.trim();
}

const gitFixture = fs.mkdtempSync(path.join(os.tmpdir(), 'bill-e2e-scope-'));
try {
    runGit(gitFixture, ['init']);
    runGit(gitFixture, ['config', 'user.name', 'E2E Scope Test']);
    runGit(gitFixture, ['config', 'user.email', 'e2e-scope@example.test']);
    fs.writeFileSync(path.join(gitFixture, 'README.md'), 'base\n', 'utf8');
    runGit(gitFixture, ['add', 'README.md']);
    runGit(gitFixture, ['commit', '-m', 'base']);
    const baseSha = runGit(gitFixture, ['rev-parse', 'HEAD']);

    fs.mkdirSync(path.join(gitFixture, 'tests', 'scripts'), { recursive: true });
    fs.writeFileSync(path.join(gitFixture, 'tests', 'scripts', 'gate.test.mjs'), 'export {};\n', 'utf8');
    fs.writeFileSync(path.join(gitFixture, '一键启动.ps1'), 'Write-Host "fixture"\n', 'utf8');
    runGit(gitFixture, ['add', '--', 'tests/scripts/gate.test.mjs', '一键启动.ps1']);
    runGit(gitFixture, ['commit', '-m', 'unicode paths']);
    const headSha = runGit(gitFixture, ['rev-parse', 'HEAD']);

    assert.deepEqual(
        changedPathsFromGit(baseSha, headSha, gitFixture).sort(),
        ['tests/scripts/gate.test.mjs', '一键启动.ps1'].sort(),
    );
    assert.throws(
        () => changedPathsFromGit('0'.repeat(40), headSha, gitFixture),
        /git diff --name-only failed/,
    );

    const invalidRefs = path.join(gitFixture, 'invalid-refs.json');
    fs.writeFileSync(invalidRefs, '{"merge_base_sha":"bad","head_sha":"also-bad"}\n', 'utf8');
    assert.throws(() => readDiffRefs(invalidRefs), /40-character/);
} finally {
    fs.rmSync(gitFixture, { recursive: true, force: true });
}

console.log('PASS E2E scope classifier tests');
