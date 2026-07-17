#!/usr/bin/env node
import assert from 'node:assert/strict';
import process from 'node:process';
import { spawnSync } from 'node:child_process';

const forbiddenExactPaths = new Map([
    ['.python-version', 'removed Python toolchain marker'],
    ['pyproject.toml', 'removed Python project manifest'],
    ['pytest.ini', 'removed pytest configuration'],
    ['tox.ini', 'removed tox configuration'],
    ['uv.lock', 'removed Python lockfile'],
    ['requirements.txt', 'removed Python dependency manifest'],
    ['requirements-dev.txt', 'removed Python development dependency manifest'],
]);

const forbiddenPathPrefixes = new Map([
    ['scripts/hooks/', 'removed legacy hook tree'],
    ['src/backend/python/', 'removed Python backend sidecar tree'],
]);

function normalizeTrackedPath(value) {
    return String(value).replaceAll('\\', '/').replace(/^\.\//, '');
}

function validateTrackedPaths(paths) {
    const findings = [];
    for (const rawPath of paths) {
        const trackedPath = normalizeTrackedPath(rawPath);
        if (!trackedPath) {
            continue;
        }
        if (/\.(?:py|pyi|pyx)$/i.test(trackedPath)) {
            findings.push({ path: trackedPath, reason: 'tracked Python source' });
            continue;
        }
        const exactReason = forbiddenExactPaths.get(trackedPath.toLowerCase());
        if (exactReason) {
            findings.push({ path: trackedPath, reason: exactReason });
            continue;
        }
        for (const [prefix, reason] of forbiddenPathPrefixes) {
            if (trackedPath.toLowerCase().startsWith(prefix)) {
                findings.push({ path: trackedPath, reason });
                break;
            }
        }
    }
    return findings;
}

function trackedPathsFromGit() {
    const result = spawnSync('git', ['ls-files', '-z'], {
        cwd: process.cwd(),
        encoding: 'utf8',
        windowsHide: true,
    });
    if (result.error) {
        throw result.error;
    }
    if (result.status !== 0) {
        throw new Error(`git ls-files failed with exit code ${result.status}: ${result.stderr.trim()}`);
    }
    return result.stdout.split('\0').filter(Boolean);
}

function runSelfTest() {
    const allowed = [
        'src/backend/http/lib.rs',
        'scripts/check-gitea-workflow.mjs',
        'docs/python-history.md',
    ];
    assert.deepEqual(validateTrackedPaths(allowed), []);

    const pythonFinding = validateTrackedPaths(['src/backend/http/server.py']);
    assert.equal(pythonFinding.length, 1);
    assert.match(pythonFinding[0].reason, /Python source/);

    const sidecarFinding = validateTrackedPaths(['scripts/hooks/task_state.mjs']);
    assert.equal(sidecarFinding.length, 1);
    assert.match(sidecarFinding[0].reason, /legacy hook tree/);

    const artifactFinding = validateTrackedPaths(['pyproject.toml']);
    assert.equal(artifactFinding.length, 1);
    assert.match(artifactFinding[0].reason, /Python project manifest/);

    console.log('PASS rust-only source-tree self-test (allowed, Python, legacy sidecar, removed artifact)');
}

function main() {
    if (process.argv.includes('--self-test')) {
        runSelfTest();
        return;
    }
    const trackedPaths = trackedPathsFromGit();
    const findings = validateTrackedPaths(trackedPaths);
    if (findings.length > 0) {
        console.error('Rust-only source-tree gate rejected tracked legacy paths:');
        for (const finding of findings) {
            console.error(`- ${finding.path}: ${finding.reason}`);
        }
        process.exitCode = 1;
        return;
    }
    console.log(`PASS Rust-only source-tree gate (${trackedPaths.length} tracked paths)`);
}

main();
