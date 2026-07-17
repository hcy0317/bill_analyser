#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const SHA_PATTERN = /^[0-9a-f]{40}$/i;
const ZERO_SHA_PATTERN = /^0{40}$/;

function argumentValue(args, name, fallback = undefined) {
    const index = args.indexOf(name);
    if (index === -1) {
        return fallback;
    }
    if (!args[index + 1]) {
        throw new Error(`Missing value for ${name}`);
    }
    return args[index + 1];
}

function requireSha(value, label) {
    const normalized = String(value ?? '').trim().toLowerCase();
    if (!SHA_PATTERN.test(normalized) || ZERO_SHA_PATTERN.test(normalized)) {
        throw new Error(`${label} must be a non-zero 40-hex commit SHA`);
    }
    return normalized;
}

function runGit(args, { cwd = process.cwd(), allowFailure = false } = {}) {
    const gitArgs = ['-c', 'core.fsmonitor=false', ...args];
    const result = spawnSync('git', gitArgs, {
        cwd,
        encoding: 'utf8',
        env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
    });
    if (!allowFailure && result.status !== 0) {
        const detail = String(result.stderr || result.stdout || '').trim();
        throw new Error(`git ${args.join(' ')} failed${detail ? `: ${detail}` : ''}`);
    }
    return result;
}

function gitOutput(args, options = {}) {
    return runGit(args, options).stdout.trim();
}

function resolveCommit(reference, { cwd = process.cwd() } = {}) {
    const result = runGit(['rev-parse', '--verify', `${reference}^{commit}`], {
        cwd,
        allowFailure: true,
    });
    if (result.status !== 0) {
        throw new Error(`Unable to resolve commit object for ${reference}`);
    }
    return requireSha(result.stdout.trim(), `resolved ${reference}`);
}

function fetchAndResolve(reference, { cwd = process.cwd(), remote = 'origin' } = {}) {
    const fetch = runGit(['-c', 'gc.auto=0', 'fetch', '--no-tags', remote, reference], {
        cwd,
        allowFailure: true,
    });
    if (fetch.status !== 0) {
        const detail = String(fetch.stderr || fetch.stdout || '').trim();
        throw new Error(`Unable to fetch exact ref ${reference} from ${remote}${detail ? `: ${detail}` : ''}`);
    }
    if (SHA_PATTERN.test(reference)) {
        return resolveCommit(requireSha(reference, 'requested ref'), { cwd });
    }
    return resolveCommit('FETCH_HEAD', { cwd });
}

function readEvent(eventPath) {
    if (!eventPath) {
        throw new Error('Event mode requires --event-path');
    }
    let parsed;
    try {
        parsed = JSON.parse(fs.readFileSync(path.resolve(eventPath), 'utf8'));
    } catch (error) {
        throw new Error(`Unable to read Gitea event JSON: ${error.message}`);
    }
    return parsed;
}

function requestedRefs({ eventName, event, defaultBase, githubSha }) {
    if (eventName === 'pull_request') {
        return {
            requestedBase: requireSha(event?.pull_request?.base?.sha, 'pull_request.base.sha'),
            requestedHead: requireSha(event?.pull_request?.head?.sha, 'pull_request.head.sha'),
        };
    }
    if (eventName === 'push') {
        return {
            requestedBase: requireSha(event?.before, 'push.before'),
            requestedHead: requireSha(event?.after, 'push.after'),
        };
    }
    if (eventName === 'workflow_dispatch') {
        const explicitBase = event?.inputs?.base_sha;
        return {
            requestedBase: explicitBase
                ? requireSha(explicitBase, 'workflow_dispatch.inputs.base_sha')
                : defaultBase,
            requestedHead: requireSha(githubSha, 'GITHUB_SHA'),
        };
    }
    throw new Error(`Unsupported Gitea event: ${eventName}`);
}

function atomicWriteJson(outputPath, value) {
    const absolute = path.resolve(outputPath);
    fs.mkdirSync(path.dirname(absolute), { recursive: true });
    const temporary = `${absolute}.${process.pid}.${Date.now()}.tmp`;
    try {
        fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { encoding: 'utf8', flag: 'wx' });
        fs.renameSync(temporary, absolute);
    } finally {
        fs.rmSync(temporary, { force: true });
    }
}

function assertEvidence(value) {
    if (!value || typeof value !== 'object') {
        throw new Error('ci-diff-refs evidence must be a JSON object');
    }
    for (const field of ['event_name', 'requested_base', 'requested_head', 'resolved_base', 'merge_base_sha', 'head_sha']) {
        if (value[field] === undefined || value[field] === null || value[field] === '') {
            throw new Error(`ci-diff-refs evidence is missing ${field}`);
        }
    }
    for (const field of ['resolved_base', 'merge_base_sha', 'head_sha']) {
        requireSha(value[field], field);
    }
    if (SHA_PATTERN.test(String(value.requested_base))) {
        requireSha(value.requested_base, 'requested_base');
    }
    requireSha(value.requested_head, 'requested_head');
    return value;
}

function resolveDiffRefs({
    cwd = process.cwd(),
    eventName,
    eventPath,
    remote = 'origin',
    defaultBase = 'refs/heads/main',
    outputPath,
    localBase,
    localHead,
    environment = process.env,
}) {
    let requestedBase;
    let requestedHead;
    let resolvedBase;
    let headSha;
    let eventRef = environment.GITHUB_REF ?? null;
    let localHeadRef = null;

    if (localBase || localHead) {
        requestedBase = localBase ?? 'main';
        localHeadRef = localHead ?? 'HEAD';
        resolvedBase = resolveCommit(requestedBase, { cwd });
        headSha = resolveCommit(localHeadRef, { cwd });
        requestedHead = headSha;
        eventName = 'local';
        eventRef = null;
    } else {
        const event = readEvent(eventPath);
        const requested = requestedRefs({
            eventName,
            event,
            defaultBase,
            githubSha: environment.GITHUB_SHA,
        });
        requestedBase = requested.requestedBase;
        requestedHead = requested.requestedHead;

        const githubSha = requireSha(environment.GITHUB_SHA, 'GITHUB_SHA');
        if (githubSha !== requestedHead) {
            throw new Error(`Event head ${requestedHead} does not match GITHUB_SHA ${githubSha}`);
        }

        resolvedBase = fetchAndResolve(requestedBase, { cwd, remote });
        headSha = fetchAndResolve(requestedHead, { cwd, remote });
        runGit(['checkout', '--detach', '--force', headSha], { cwd });
        const checkedOutHead = resolveCommit('HEAD', { cwd });
        if (checkedOutHead !== headSha) {
            throw new Error(`Detached checkout mismatch: expected ${headSha}, got ${checkedOutHead}`);
        }
    }

    const mergeBaseResult = runGit(['merge-base', resolvedBase, headSha], {
        cwd,
        allowFailure: true,
    });
    if (mergeBaseResult.status !== 0) {
        throw new Error(`Unable to compute merge base for ${resolvedBase} and ${headSha}`);
    }
    const mergeBaseSha = requireSha(mergeBaseResult.stdout.trim(), 'merge_base_sha');
    const evidence = assertEvidence({
        schema_version: 1,
        event_name: eventName,
        event_ref: eventRef,
        requested_base: requestedBase,
        requested_head: requestedHead,
        requested_head_ref: localHeadRef,
        resolved_base: resolvedBase,
        merge_base_sha: mergeBaseSha,
        head_sha: headSha,
        generated_at: new Date().toISOString(),
    });
    atomicWriteJson(outputPath, evidence);
    return evidence;
}

function readEvidenceField(inputPath, field) {
    let evidence;
    try {
        evidence = assertEvidence(JSON.parse(fs.readFileSync(path.resolve(inputPath), 'utf8')));
    } catch (error) {
        throw new Error(`Unable to read ci-diff-refs evidence: ${error.message}`);
    }
    if (!(field in evidence)) {
        throw new Error(`ci-diff-refs evidence does not contain ${field}`);
    }
    const value = evidence[field];
    if (field.endsWith('_sha') || ['resolved_base', 'head_sha'].includes(field)) {
        return requireSha(value, field);
    }
    if (value === null || typeof value === 'object') {
        throw new Error(`ci-diff-refs field ${field} is not a scalar`);
    }
    return String(value);
}

function writeEvent(directory, name, value) {
    const target = path.join(directory, `${name}.json`);
    fs.writeFileSync(target, JSON.stringify(value), 'utf8');
    return target;
}

function configureTestRepository(cwd) {
    runGit(['config', 'user.email', 'ci-resolver@example.invalid'], { cwd });
    runGit(['config', 'user.name', 'CI Resolver'], { cwd });
}

function selfTest() {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bill-ci-diff-refs-'));
    try {
        const source = path.join(root, 'source');
        const bare = path.join(root, 'remote.git');
        fs.mkdirSync(source);
        runGit(['init', '--initial-branch=main'], { cwd: source });
        configureTestRepository(source);
        fs.writeFileSync(path.join(source, 'fixture.txt'), 'base\n');
        runGit(['add', 'fixture.txt'], { cwd: source });
        runGit(['commit', '-m', 'base'], { cwd: source });
        const baseSha = resolveCommit('HEAD', { cwd: source });
        runGit(['checkout', '-b', 'feature'], { cwd: source });
        fs.appendFileSync(path.join(source, 'fixture.txt'), 'head\n');
        runGit(['commit', '-am', 'head'], { cwd: source });
        const headSha = resolveCommit('HEAD', { cwd: source });
        runGit(['init', '--bare', bare]);
        runGit(['remote', 'add', 'origin', bare], { cwd: source });
        runGit(['push', 'origin', 'main', 'feature'], { cwd: source });

        const clone = path.join(root, 'clone');
        runGit(['clone', bare, clone]);
        configureTestRepository(clone);
        const output = path.join(clone, 'evidence', 'ci-diff-refs.json');
        const env = { GITHUB_SHA: headSha, GITHUB_REF: 'refs/pull/1/head' };

        const prEvent = writeEvent(root, 'pull_request', {
            pull_request: { base: { sha: baseSha }, head: { sha: headSha } },
        });
        const pr = resolveDiffRefs({
            cwd: clone,
            eventName: 'pull_request',
            eventPath: prEvent,
            remote: 'origin',
            defaultBase: 'refs/heads/main',
            outputPath: output,
            environment: env,
        });
        assert.equal(pr.merge_base_sha, baseSha);
        assert.equal(readEvidenceField(output, 'head_sha'), headSha);

        runGit(['checkout', '--detach', headSha], { cwd: clone });
        const pushEvent = writeEvent(root, 'push', { before: baseSha, after: headSha });
        assert.equal(resolveDiffRefs({
            cwd: clone,
            eventName: 'push',
            eventPath: pushEvent,
            remote: 'origin',
            outputPath: output,
            environment: { GITHUB_SHA: headSha, GITHUB_REF: 'refs/heads/feature' },
        }).resolved_base, baseSha);

        const dispatchExplicit = writeEvent(root, 'dispatch-explicit', { inputs: { base_sha: baseSha } });
        assert.equal(resolveDiffRefs({
            cwd: clone,
            eventName: 'workflow_dispatch',
            eventPath: dispatchExplicit,
            remote: 'origin',
            outputPath: output,
            environment: { GITHUB_SHA: headSha, GITHUB_REF: 'refs/heads/feature' },
        }).requested_base, baseSha);

        const dispatchFallback = writeEvent(root, 'dispatch-fallback', { inputs: {} });
        assert.equal(resolveDiffRefs({
            cwd: clone,
            eventName: 'workflow_dispatch',
            eventPath: dispatchFallback,
            remote: 'origin',
            defaultBase: 'refs/heads/main',
            outputPath: output,
            environment: { GITHUB_SHA: headSha, GITHUB_REF: 'refs/heads/feature' },
        }).resolved_base, baseSha);

        assert.equal(resolveDiffRefs({
            cwd: source,
            localBase: 'main',
            localHead: 'feature',
            outputPath: path.join(source, 'local.json'),
        }).event_name, 'local');

        const zeroEvent = writeEvent(root, 'zero', { before: '0'.repeat(40), after: headSha });
        assert.throws(() => resolveDiffRefs({
            cwd: clone,
            eventName: 'push',
            eventPath: zeroEvent,
            remote: 'origin',
            outputPath: output,
            environment: { GITHUB_SHA: headSha },
        }), /non-zero 40-hex/);

        assert.throws(() => resolveDiffRefs({
            cwd: clone,
            eventName: 'push',
            eventPath: pushEvent,
            remote: 'origin',
            outputPath: output,
            environment: { GITHUB_SHA: baseSha },
        }), /does not match GITHUB_SHA/);

        const missingBase = writeEvent(root, 'missing-base', {
            before: 'f'.repeat(40),
            after: headSha,
        });
        assert.throws(() => resolveDiffRefs({
            cwd: clone,
            eventName: 'push',
            eventPath: missingBase,
            remote: 'origin',
            outputPath: output,
            environment: { GITHUB_SHA: headSha },
        }), /Unable to fetch exact ref/);

        const shallow = path.join(root, 'shallow');
        runGit([
            'clone',
            '--depth', '1',
            '--branch', 'feature',
            pathToFileURL(bare).href,
            shallow,
        ]);
        assert.equal(fs.existsSync(path.join(shallow, '.git', 'shallow')), true);
        assert.throws(() => resolveDiffRefs({
            cwd: shallow,
            eventName: 'pull_request',
            eventPath: prEvent,
            remote: 'origin',
            outputPath: path.join(shallow, 'shallow.json'),
            environment: env,
        }), /Unable to compute merge base/);

        const unrelatedSource = path.join(root, 'unrelated-source');
        fs.mkdirSync(unrelatedSource);
        runGit(['init', '--initial-branch=main'], { cwd: unrelatedSource });
        configureTestRepository(unrelatedSource);
        fs.writeFileSync(path.join(unrelatedSource, 'other.txt'), 'unrelated\n');
        runGit(['add', 'other.txt'], { cwd: unrelatedSource });
        runGit(['commit', '-m', 'unrelated'], { cwd: unrelatedSource });
        const unrelatedSha = resolveCommit('HEAD', { cwd: unrelatedSource });
        runGit(['remote', 'add', 'origin', bare], { cwd: unrelatedSource });
        runGit(['push', '--force', 'origin', 'main:unrelated'], { cwd: unrelatedSource });
        const noMerge = writeEvent(root, 'no-merge-base', { before: unrelatedSha, after: headSha });
        assert.throws(() => resolveDiffRefs({
            cwd: clone,
            eventName: 'push',
            eventPath: noMerge,
            remote: 'origin',
            outputPath: output,
            environment: { GITHUB_SHA: headSha },
        }), /Unable to compute merge base/);

        fs.writeFileSync(output, JSON.stringify({ ...pr, head_sha: 'bad' }), 'utf8');
        assert.throws(() => readEvidenceField(output, 'head_sha'), /40-hex/);
    } finally {
        fs.rmSync(root, { recursive: true, force: true });
    }
    console.log('PASS ci diff ref resolver self-tests');
}

function main(argv = process.argv.slice(2)) {
    if (argv.includes('--self-test')) {
        selfTest();
        return;
    }
    const [command, ...args] = argv;
    if (command === 'resolve') {
        const outputPath = argumentValue(args, '--out');
        if (!outputPath) {
            throw new Error('resolve requires --out');
        }
        const evidence = resolveDiffRefs({
            eventName: argumentValue(args, '--event-name', process.env.GITHUB_EVENT_NAME),
            eventPath: argumentValue(args, '--event-path', process.env.GITHUB_EVENT_PATH),
            remote: argumentValue(args, '--remote', 'origin'),
            defaultBase: argumentValue(args, '--default-base', 'refs/heads/main'),
            outputPath,
            localBase: argumentValue(args, '--local-base'),
            localHead: argumentValue(args, '--local-head'),
        });
        console.log(JSON.stringify(evidence, null, 2));
        return;
    }
    if (command === 'read') {
        const inputPath = argumentValue(args, '--input');
        const field = argumentValue(args, '--field');
        if (!inputPath || !field) {
            throw new Error('read requires --input and --field');
        }
        console.log(readEvidenceField(inputPath, field));
        return;
    }
    throw new Error([
        'Usage:',
        '  node scripts/resolve-ci-diff-refs.mjs --self-test',
        '  node scripts/resolve-ci-diff-refs.mjs resolve --event-name <name> --event-path <path> --remote origin --default-base refs/heads/main --out <path>',
        '  node scripts/resolve-ci-diff-refs.mjs resolve --local-base main --local-head HEAD --out <path>',
        '  node scripts/resolve-ci-diff-refs.mjs read --input <path> --field <name>',
    ].join('\n'));
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
    assertEvidence,
    readEvidenceField,
    requestedRefs,
    requireSha,
    resolveDiffRefs,
};
