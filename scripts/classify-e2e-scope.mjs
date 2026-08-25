#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const E2E_RELEVANT_PREFIXES = [
    '.gitea/',
    'scripts/',
    'src/backend/',
    'src/web/',
    'tests/',
];
const E2E_RELEVANT_ROOT_FILES = new Set(['Cargo.toml', 'Cargo.lock']);

function normalizeChangedPath(value) {
    return String(value).replaceAll('\\', '/').replace(/^\.\//, '');
}

function isE2eRelevantPath(value) {
    const changedPath = normalizeChangedPath(value);
    return E2E_RELEVANT_ROOT_FILES.has(changedPath)
        || E2E_RELEVANT_PREFIXES.some(prefix => changedPath.startsWith(prefix))
        || /^[^/]+\.(?:ps1|bat)$/i.test(changedPath);
}

function classifyE2ePaths(eventName, changedPaths) {
    const normalizedEvent = String(eventName).trim();
    if (normalizedEvent !== 'pull_request') {
        return {
            required: true,
            reason: `event_requires_full_e2e:${normalizedEvent || 'unknown'}`,
            matched_paths: [],
        };
    }

    const matchedPaths = [...new Set(changedPaths
        .map(normalizeChangedPath)
        .filter(Boolean)
        .filter(isE2eRelevantPath))];
    return {
        required: matchedPaths.length > 0,
        reason: matchedPaths.length > 0
            ? `matched_e2e_paths:${matchedPaths.length}`
            : 'no_e2e_relevant_paths',
        matched_paths: matchedPaths,
    };
}

function parseArgs(argv) {
    const values = new Map();
    for (let index = 0; index < argv.length; index += 1) {
        const name = argv[index];
        if (!name.startsWith('--')) {
            throw new Error(`Unexpected argument: ${name}`);
        }
        const value = argv[index + 1];
        if (value === undefined || value.startsWith('--')) {
            throw new Error(`Missing value for ${name}`);
        }
        values.set(name, value);
        index += 1;
    }
    return values;
}

function readDiffRefs(inputPath) {
    const payload = JSON.parse(fs.readFileSync(inputPath, 'utf8'));
    const mergeBaseSha = String(payload.merge_base_sha ?? '').trim();
    const headSha = String(payload.head_sha ?? '').trim();
    if (!/^[0-9a-f]{40}$/i.test(mergeBaseSha) || !/^[0-9a-f]{40}$/i.test(headSha)) {
        throw new Error('Immutable CI diff refs must contain 40-character merge_base_sha and head_sha');
    }
    return { mergeBaseSha, headSha };
}

function changedPathsFromGit(mergeBaseSha, headSha, cwd = process.cwd()) {
    const result = spawnSync('git', [
        '-c',
        'core.quotepath=false',
        '-c',
        'gc.auto=0',
        'diff',
        '-z',
        '--name-only',
        '--no-renames',
        `${mergeBaseSha}...${headSha}`,
        '--',
    ], {
        cwd,
        windowsHide: true,
    });
    if (result.error) {
        throw result.error;
    }
    if (result.status !== 0) {
        throw new Error(`git diff --name-only failed with exit code ${result.status}: ${result.stderr.toString('utf8').trim()}`);
    }
    return result.stdout.toString('utf8').split('\0').map(normalizeChangedPath).filter(Boolean);
}

function writeGithubOutput(outputPath, result) {
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.appendFileSync(
        outputPath,
        `required=${result.required ? 'true' : 'false'}\nreason=${result.reason}\n`,
        'utf8',
    );
}

function main(argv = process.argv.slice(2)) {
    const args = parseArgs(argv);
    const eventName = args.get('--event-name');
    const inputPath = args.get('--input');
    const outputPath = args.get('--out');
    if (!eventName || !inputPath || !outputPath) {
        throw new Error('Usage: classify-e2e-scope.mjs --event-name <event> --input <diff-refs.json> --out <github-output>');
    }

    let changedPaths = [];
    if (eventName === 'pull_request') {
        const { mergeBaseSha, headSha } = readDiffRefs(inputPath);
        changedPaths = changedPathsFromGit(mergeBaseSha, headSha);
    }
    const result = classifyE2ePaths(eventName, changedPaths);
    writeGithubOutput(outputPath, result);
    console.log(JSON.stringify({ event_name: eventName, ...result }));
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
    try {
        main();
    } catch (error) {
        console.error(`FAIL ${error.message}`);
        process.exitCode = 1;
    }
}

export {
    changedPathsFromGit,
    classifyE2ePaths,
    isE2eRelevantPath,
    normalizeChangedPath,
    readDiffRefs,
};
