#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), '..');
const webRoot = path.join(repoRoot, 'src', 'web');
const fixturePath = path.join(repoRoot, 'scripts', 'fixtures', 'e2e-ci-deliberate-failure.mjs');
const defaultEvidenceRoot = path.join(repoRoot, '.omx', 'ultragoal', 'evidence', 'e2e-ci');

const PHASE_NAMES = [
    'dependency-readiness',
    'start-rust',
    'desktop-smoke',
    'mobile-smoke',
    'cleanup',
];
const NEGATIVE_MODES = new Set([
    'missing-postgres',
    'missing-weaviate',
    'readiness-timeout',
    'desktop-spec',
    'mobile-spec',
    'cleanup-failure',
]);
const COMMANDS = {
    build: 'cargo build -p bill-analyser-http --bin bill_http_server',
    rust: 'target/debug/bill_http_server',
    desktop: 'npm --prefix src/web run e2e -- --project=desktop-chromium e2e/tests/desktop.route-smoke.spec.ts',
    mobile: 'npm --prefix src/web run e2e -- --project=mobile-chromium e2e/tests/mobile.route-smoke.spec.ts',
};
const DEFAULT_TIMEOUTS = {
    dependencyMs: 60_000,
    dependencyTotalMs: 180_000,
    rustHealthMs: 60_000,
    commandMs: 15 * 60_000,
    cleanupMs: 15_000,
};
const MAX_LOG_BYTES = 1024 * 1024;
const MAX_ARTIFACT_BYTES = 5 * 1024 * 1024;
const MAX_DIAGNOSTIC_LOG_TAIL_BYTES = 12 * 1024;
const MAX_DIAGNOSTIC_LOG_FILES = 8;
const MAX_DIAGNOSTIC_TEXT_BYTES = 8 * 1024;
const MAX_DIAGNOSTIC_OUTPUT_BYTES = 128 * 1024;
const MAX_ERROR_NODES = 64;
const ARTIFACT_RETENTION_DAYS = 14;
const PROCESS_TREE_GRACE_MS = 1_500;
const PROCESS_TREE_FORCE_MS = 5_000;
const LOCAL_HOSTS = new Set(['127.0.0.1', 'localhost', '::1']);
const CONTROLLED_POSTGRES_HOSTS = new Set([...LOCAL_HOSTS, 'postgres']);
const CONTROLLED_WEAVIATE_HOSTS = new Set([...LOCAL_HOSTS, 'weaviate']);
const DEFAULT_POSTGRES_URL = 'postgresql://bill_analyser_e2e:bill_analyser_e2e@127.0.0.1:5432/bill_analyser_e2e';
const DEFAULT_WEAVIATE_ENDPOINT = 'http://127.0.0.1:8088';
const REQUIRED_NO_PROXY_HOSTS = ['127.0.0.1', 'localhost', '::1', 'postgres', 'weaviate'];
const SENSITIVE_ENV_NAME_PATTERN = /(?:^|_)(?:PASSWORD|PASS|SECRET|TOKEN|API_KEY|PRIVATE_KEY|ACCESS_KEY)(?:_|$)/iu;

class SupervisorError extends Error {
    constructor(message, options = {}) {
        super(message, options);
        this.name = 'SupervisorError';
    }
}

function parseArgs(argv) {
    const result = { selfTest: false, negativeMode: null };
    for (let index = 0; index < argv.length; index += 1) {
        const value = argv[index];
        if (value === '--self-test') {
            result.selfTest = true;
            continue;
        }
        if (value === '--negative') {
            const mode = argv[index + 1];
            if (!NEGATIVE_MODES.has(mode)) {
                throw new SupervisorError(`--negative requires one of: ${[...NEGATIVE_MODES].join(', ')}`);
            }
            result.negativeMode = mode;
            index += 1;
            continue;
        }
        throw new SupervisorError(`Unknown argument: ${value}`);
    }
    if (result.selfTest && result.negativeMode) {
        throw new SupervisorError('--self-test and --negative cannot be combined');
    }
    return result;
}

function sanitizeRunId(value) {
    const sanitized = String(value).trim().replace(/[^A-Za-z0-9._-]+/gu, '-').replace(/^-+|-+$/gu, '');
    if (!sanitized) {
        throw new SupervisorError('E2E_RUN_ID must contain at least one safe identifier character');
    }
    return sanitized.slice(0, 96);
}

function mergeNoProxy(...values) {
    const entries = [];
    const seen = new Set();
    for (const value of [...values, REQUIRED_NO_PROXY_HOSTS]) {
        const candidates = Array.isArray(value) ? value : String(value ?? '').split(',');
        for (const candidate of candidates) {
            const entry = String(candidate).trim();
            if (entry && !seen.has(entry)) {
                seen.add(entry);
                entries.push(entry);
            }
        }
    }
    return entries.join(',');
}

function resolveHeadSha() {
    const fromEnvironment = process.env['GITHUB_SHA']?.trim();
    if (fromEnvironment) {
        return fromEnvironment;
    }
    try {
        return execFileSync('git', ['rev-parse', 'HEAD'], {
            cwd: repoRoot,
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'ignore'],
        }).trim();
    } catch {
        return 'unavailable';
    }
}

function createConfig({
    negativeMode = null,
    evidenceRoot = defaultEvidenceRoot,
    runId = null,
    controlled = false,
    authStoragePath = path.join(webRoot, 'e2e', '.auth', 'e2e-user.json'),
    timeouts = DEFAULT_TIMEOUTS,
} = {}) {
    const baseRunId = sanitizeRunId(runId ?? process.env['E2E_RUN_ID'] ?? `local-${Date.now()}-${process.pid}`);
    const effectiveRunId = negativeMode && !runId
        ? sanitizeRunId(`${baseRunId}-${negativeMode}`)
        : baseRunId;
    const expectedPrefix = `BillAnalyserE2E${effectiveRunId}`;
    const runDirectory = path.join(evidenceRoot, effectiveRunId);
    const postgresUrl = process.env['BILL_ANALYSER_POSTGRES_URL']?.trim() || DEFAULT_POSTGRES_URL;
    const weaviateEndpoint = process.env['BILL_ANALYSER_WEAVIATE_ENDPOINT']?.trim() || DEFAULT_WEAVIATE_ENDPOINT;
    const noProxy = mergeNoProxy(process.env['NO_PROXY'], process.env['no_proxy']);
    const runtimeEnv = {
        ...process.env,
        BILL_ANALYSER_DATABASE_BACKEND: 'postgres',
        BILL_ANALYSER_POSTGRES_URL: postgresUrl,
        BILL_ANALYSER_WEAVIATE_ENABLED: 'true',
        BILL_ANALYSER_WEAVIATE_ENDPOINT: weaviateEndpoint,
        BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX: expectedPrefix,
        NO_PROXY: noProxy,
        no_proxy: noProxy,
        BILL_ANALYSER_HTTP_BIND: '127.0.0.1:5000',
        BILL_ANALYSER_AUTH_JWT_SECRET: process.env['BILL_ANALYSER_AUTH_JWT_SECRET']?.trim() || 'bill-analyser-e2e-ci-secret',
        BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION: 'true',
        BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION: 'false',
        E2E_RUN_ID: effectiveRunId,
        E2E_EXPECTED_WEAVIATE_PREFIX: expectedPrefix,
        E2E_STRICT_CLEANUP: 'true',
        E2E_BASE_URL: 'http://127.0.0.1:8081',
        E2E_API_BASE_URL: 'http://127.0.0.1:5000/api',
        E2E_ACCOUNT_USERNAME: process.env['E2E_ACCOUNT_USERNAME']?.trim() || 'bill-analyser-e2e',
        E2E_ACCOUNT_EMAIL: process.env['E2E_ACCOUNT_EMAIL']?.trim() || 'bill-analyser-e2e@example.test',
        E2E_ACCOUNT_NICKNAME: process.env['E2E_ACCOUNT_NICKNAME']?.trim() || 'Bill Analyser E2E',
        E2E_ACCOUNT_PASSWORD: process.env['E2E_ACCOUNT_PASSWORD']?.trim() || 'BillAnalyserE2E!2026',
        CI: '1',
    };
    return {
        negativeMode,
        evidenceRoot,
        runId: effectiveRunId,
        runDirectory,
        headSha: resolveHeadSha(),
        expectedPrefix,
        runtimeEnv,
        authStoragePath,
        timeouts: { ...DEFAULT_TIMEOUTS, ...timeouts },
        controlled,
    };
}

function redactSensitiveText(value, runtimeEnv = {}) {
    let redacted = String(value ?? '');
    redacted = redacted.replace(
        /\b([a-z][a-z0-9+.-]*:\/\/)([^\s/@]+)@/giu,
        '$1[REDACTED]@',
    );
    redacted = redacted.replace(
        /\b(authorization\s*[:=]\s*bearer\s+)[^\s"',}]+/giu,
        '$1[REDACTED]',
    );
    redacted = redacted.replace(
        /(["']?(?:password|secret|token|api[_-]?key)["']?\s*[:=]\s*["']?)[^\s"',}]+/giu,
        '$1[REDACTED]',
    );
    for (const [name, rawValue] of Object.entries(runtimeEnv)) {
        if (!SENSITIVE_ENV_NAME_PATTERN.test(name)) {
            continue;
        }
        const secret = String(rawValue ?? '');
        if (secret.length >= 4) {
            redacted = redacted.replaceAll(secret, '[REDACTED]');
        }
    }
    return redacted;
}

function truncateUtf8(value, maxBytes = MAX_DIAGNOSTIC_TEXT_BYTES) {
    const buffer = Buffer.from(String(value ?? ''), 'utf8');
    if (buffer.length <= maxBytes) {
        return buffer.toString('utf8');
    }
    return `${buffer.subarray(0, maxBytes).toString('utf8')}\n[truncated after ${maxBytes} bytes]`;
}

function serializeError(error, runtimeEnv = {}, budget = null) {
    const state = budget ?? { seen: new WeakSet(), remaining: MAX_ERROR_NODES };
    if (state.remaining <= 0) {
        return { name: 'Error', message: `[error node budget exhausted at ${MAX_ERROR_NODES}]` };
    }
    state.remaining -= 1;
    if (error instanceof Error) {
        if (state.seen.has(error)) {
            return { name: error.name, message: '[circular error]' };
        }
        state.seen.add(error);
        const result = {
            name: error.name,
            message: truncateUtf8(redactSensitiveText(error.message, runtimeEnv)),
            stack: error.stack
                ? truncateUtf8(redactSensitiveText(error.stack, runtimeEnv))
                : undefined,
            cause: error.cause
                ? serializeError(error.cause, runtimeEnv, state)
                : undefined,
        };
        if (error instanceof AggregateError) {
            result.errors = [];
            for (let index = 0; index < error.errors.length; index += 1) {
                if (state.remaining <= 0) {
                    result.omittedErrors = error.errors.length - index;
                    break;
                }
                result.errors.push(serializeError(error.errors[index], runtimeEnv, state));
            }
        }
        return result;
    }
    return {
        name: 'Error',
        message: truncateUtf8(redactSensitiveText(error, runtimeEnv)),
    };
}

function publicErrorMessage(error, runtimeEnv = {}) {
    const message = error instanceof Error ? error.message : String(error);
    return truncateUtf8(redactSensitiveText(message, runtimeEnv));
}

function atomicWriteJson(target, value) {
    fs.mkdirSync(path.dirname(target), { recursive: true });
    const temporary = `${target}.${process.pid}.tmp`;
    fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
    fs.rmSync(target, { force: true });
    fs.renameSync(temporary, target);
}

function writeArtifactIndex(config) {
    const artifactPath = path.join(config.runDirectory, 'artifact-index.json');
    const artifacts = fs.readdirSync(config.runDirectory, { withFileTypes: true })
        .filter(entry => entry.isFile() && entry.name !== 'artifact-index.json')
        .map(entry => {
            const target = path.join(config.runDirectory, entry.name);
            return { path: entry.name, bytes: fs.statSync(target).size };
        })
        .sort((left, right) => left.path.localeCompare(right.path));
    const totalBytes = artifacts.reduce((sum, item) => sum + item.bytes, 0);
    if (totalBytes > MAX_ARTIFACT_BYTES) {
        throw new SupervisorError(`E2E artifact set exceeds ${MAX_ARTIFACT_BYTES} bytes`);
    }
    const index = {
        schemaVersion: 1,
        runId: config.runId,
        headSha: config.headSha,
        generatedAt: new Date().toISOString(),
        retentionDays: ARTIFACT_RETENTION_DAYS,
        maxArtifactBytes: MAX_ARTIFACT_BYTES,
        totalBytes,
        artifacts,
    };
    atomicWriteJson(artifactPath, index);
    return index;
}

class EvidenceRecorder {
    constructor(config) {
        this.config = config;
        this.statePath = path.join(config.runDirectory, 'supervisor-state.json');
        this.phasePath = path.join(config.runDirectory, 'phase-evidence.json');
        this.pidPath = path.join(config.runDirectory, 'rust.pid');
        this.artifactPath = path.join(config.runDirectory, 'artifact-index.json');
        this.startedAt = new Date().toISOString();
        this.state = {
            schemaVersion: 1,
            runId: config.runId,
            headSha: config.headSha,
            controlled: config.controlled,
            negativeMode: config.negativeMode,
            status: 'running',
            currentPhase: null,
            rustPid: null,
            startedAt: this.startedAt,
            finishedAt: null,
            error: null,
        };
        this.phases = PHASE_NAMES.map(name => ({
            name,
            outcome: 'pending',
            startedAt: null,
            finishedAt: null,
            timeout: null,
            details: {},
            error: null,
        }));
        fs.mkdirSync(config.runDirectory, { recursive: true });
        this.persist();
    }

    persist() {
        atomicWriteJson(this.statePath, this.state);
        atomicWriteJson(this.phasePath, {
            schemaVersion: 1,
            runId: this.config.runId,
            headSha: this.config.headSha,
            phases: this.phases,
        });
    }

    phaseRecord(name) {
        const phase = this.phases.find(item => item.name === name);
        if (!phase) {
            throw new SupervisorError(`Unknown supervisor phase ${name}`);
        }
        return phase;
    }

    async runPhase(name, timeout, details, work) {
        const phase = this.phaseRecord(name);
        if (phase.outcome !== 'pending') {
            throw new SupervisorError(`Supervisor phase ${name} cannot start from ${phase.outcome}`);
        }
        phase.outcome = 'running';
        phase.startedAt = new Date().toISOString();
        phase.timeout = timeout;
        phase.details = { ...details };
        this.state.currentPhase = name;
        this.persist();
        try {
            const result = await work(phase);
            phase.outcome = 'success';
            phase.details = { ...phase.details, ...(result ?? {}) };
            return result;
        } catch (error) {
            phase.outcome = 'failure';
            phase.error = serializeError(error, this.config.runtimeEnv);
            throw error;
        } finally {
            phase.finishedAt = new Date().toISOString();
            this.persist();
        }
    }

    recordRust(pid) {
        if (!Number.isInteger(pid) || pid <= 0) {
            throw new SupervisorError(`Rust child returned invalid PID ${pid}`);
        }
        this.state.rustPid = pid;
        fs.writeFileSync(this.pidPath, `${pid}\n`, 'utf8');
        this.persist();
    }

    skipPending(reason) {
        const timestamp = new Date().toISOString();
        for (const phase of this.phases) {
            if (phase.name !== 'cleanup' && phase.outcome === 'pending') {
                phase.outcome = 'skipped';
                phase.startedAt = timestamp;
                phase.finishedAt = timestamp;
                phase.details = { reason };
            }
        }
        this.persist();
    }

    finalize(status, error = null) {
        this.state.status = status;
        this.state.currentPhase = null;
        this.state.finishedAt = new Date().toISOString();
        this.state.error = error ? serializeError(error, this.config.runtimeEnv) : null;
        this.persist();
        this.writeArtifactIndex();
    }

    writeArtifactIndex() {
        return writeArtifactIndex(this.config);
    }
}

function appendCapped(target, chunks) {
    const joined = chunks.join('');
    const buffer = Buffer.from(joined, 'utf8');
    const output = buffer.length <= MAX_LOG_BYTES
        ? buffer
        : Buffer.concat([
            buffer.subarray(0, MAX_LOG_BYTES),
            Buffer.from(`\n[truncated after ${MAX_LOG_BYTES} bytes]\n`, 'utf8'),
        ]);
    fs.writeFileSync(target, output);
}

function executable(name) {
    return process.platform === 'win32' ? `${name}.cmd` : name;
}

function isProcessGroupAlive(groupId) {
    try {
        process.kill(-groupId, 0);
        return true;
    } catch (error) {
        if (error?.code === 'ESRCH') {
            return false;
        }
        if (error?.code === 'EPERM') {
            return true;
        }
        throw error;
    }
}

async function waitForPosixTreeExit(child, groupId, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        if ((child.exitCode !== null || child.signalCode !== null) && !isProcessGroupAlive(groupId)) {
            return;
        }
        await delay(25);
    }
    throw new SupervisorError(`owned process group ${groupId} did not exit within ${timeoutMs}ms`);
}

function signalPosixTree(groupId, signal) {
    try {
        process.kill(-groupId, signal);
    } catch (error) {
        if (error?.code !== 'ESRCH') {
            throw error;
        }
    }
}

function runTaskkill(pid, force, timeoutMs) {
    return new Promise((resolve, reject) => {
        const args = ['/PID', String(pid), '/T'];
        if (force) {
            args.push('/F');
        }
        const helper = spawn('taskkill.exe', args, {
            shell: false,
            windowsHide: true,
            stdio: ['ignore', 'pipe', 'pipe'],
        });
        const chunks = [];
        let settled = false;
        const finish = error => {
            if (settled) {
                return;
            }
            settled = true;
            clearTimeout(timer);
            if (error) {
                reject(error);
            } else {
                resolve();
            }
        };
        const timer = setTimeout(() => {
            helper.kill('SIGKILL');
            finish(new SupervisorError(`taskkill for owned PID ${pid} timed out after ${timeoutMs}ms`));
        }, timeoutMs);
        helper.stdout?.on('data', chunk => chunks.push(chunk.toString()));
        helper.stderr?.on('data', chunk => chunks.push(chunk.toString()));
        helper.once('error', error => finish(new SupervisorError(`taskkill for owned PID ${pid} failed to start`, { cause: error })));
        helper.once('close', code => {
            if (code === 0) {
                finish();
                return;
            }
            finish(new SupervisorError(
                `taskkill for owned PID ${pid} exited with code ${code}: ${chunks.join('').trim()}`,
            ));
        });
    });
}

async function terminateOwnedProcessTree(child, {
    label,
    graceMs = PROCESS_TREE_GRACE_MS,
    forceMs = PROCESS_TREE_FORCE_MS,
} = {}) {
    const pid = child.pid;
    if (!Number.isInteger(pid) || pid <= 0) {
        throw new SupervisorError(`${label} has no owned PID to terminate`);
    }
    if (process.platform === 'win32') {
        let gracefulError = null;
        try {
            await runTaskkill(pid, false, graceMs);
            await waitForChildExit(child, graceMs);
            return { pid, platform: 'win32', escalated: false, treeCommand: `taskkill /PID ${pid} /T` };
        } catch (error) {
            gracefulError = error;
        }
        try {
            await runTaskkill(pid, true, forceMs);
            await waitForChildExit(child, forceMs);
            return {
                pid,
                platform: 'win32',
                escalated: true,
                treeCommand: `taskkill /PID ${pid} /T /F`,
                gracefulError: serializeError(gracefulError),
            };
        } catch (forceError) {
            throw new AggregateError(
                [gracefulError, forceError],
                `${label} owned process tree ${pid} survived graceful and forced termination`,
            );
        }
    }

    signalPosixTree(pid, 'SIGTERM');
    try {
        await waitForPosixTreeExit(child, pid, graceMs);
        return { pid, platform: process.platform, escalated: false, processGroup: pid };
    } catch (gracefulError) {
        signalPosixTree(pid, 'SIGKILL');
        try {
            await waitForPosixTreeExit(child, pid, forceMs);
            return {
                pid,
                platform: process.platform,
                escalated: true,
                processGroup: pid,
                gracefulError: serializeError(gracefulError),
            };
        } catch (forceError) {
            throw new AggregateError(
                [gracefulError, forceError],
                `${label} owned process group ${pid} survived SIGTERM and SIGKILL`,
            );
        }
    }
}

function runCommand(command, args, {
    cwd,
    env,
    timeoutMs,
    signal,
    logPath,
    label,
    terminationGraceMs = PROCESS_TREE_GRACE_MS,
    terminationForceMs = PROCESS_TREE_FORCE_MS,
}) {
    return new Promise((resolve, reject) => {
        const child = spawn(command, args, {
            cwd,
            env,
            shell: false,
            detached: process.platform !== 'win32',
            windowsHide: true,
            stdio: ['ignore', 'pipe', 'pipe'],
        });
        const chunks = [];
        let settled = false;
        let timer = null;
        let terminating = false;

        const finish = (error, result = null) => {
            if (settled) {
                return;
            }
            settled = true;
            if (timer) {
                clearTimeout(timer);
            }
            signal?.removeEventListener('abort', onAbort);
            if (logPath) {
                appendCapped(logPath, chunks);
            }
            if (error) {
                reject(error);
            } else {
                resolve(result);
            }
        };
        const terminate = async error => {
            if (settled || terminating) {
                return;
            }
            terminating = true;
            if (timer) {
                clearTimeout(timer);
            }
            try {
                const proof = await terminateOwnedProcessTree(child, {
                    label,
                    graceMs: terminationGraceMs,
                    forceMs: terminationForceMs,
                });
                chunks.push(`\n[supervisor-owned-tree-termination] ${JSON.stringify(proof)}\n`);
                finish(error);
            } catch (terminationError) {
                finish(new AggregateError(
                    [error, terminationError],
                    `${label} failed and its owned process tree could not be terminated`,
                ));
            }
        };
        const onAbort = () => {
            void terminate(new SupervisorError(`${label} aborted`, { cause: signal.reason }));
        };

        child.stdout?.on('data', chunk => chunks.push(chunk.toString()));
        child.stderr?.on('data', chunk => chunks.push(chunk.toString()));
        child.once('error', error => finish(new SupervisorError(`${label} failed to start`, { cause: error })));
        child.once('close', (code, childSignal) => {
            if (terminating) {
                return;
            }
            if (code === 0) {
                finish(null, { code, signal: childSignal });
            } else {
                finish(new SupervisorError(`${label} exited with code ${code ?? 'null'} signal ${childSignal ?? 'none'}`));
            }
        });
        if (signal?.aborted) {
            onAbort();
            return;
        }
        signal?.addEventListener('abort', onAbort, { once: true });
        timer = setTimeout(() => {
            void terminate(new SupervisorError(`${label} timed out after ${timeoutMs}ms`));
        }, timeoutMs);
    });
}

function waitForTcp({ host, port, timeoutMs, signal, label }) {
    const deadline = Date.now() + timeoutMs;
    return new Promise((resolve, reject) => {
        let timer = null;
        let socket = null;
        let settled = false;

        const finish = error => {
            if (settled) {
                return;
            }
            settled = true;
            if (timer) {
                clearTimeout(timer);
            }
            socket?.destroy();
            signal?.removeEventListener('abort', onAbort);
            if (error) {
                reject(error);
            } else {
                resolve();
            }
        };
        const onAbort = () => finish(new SupervisorError(`${label} aborted`, { cause: signal.reason }));
        const attempt = () => {
            if (signal?.aborted) {
                onAbort();
                return;
            }
            socket = net.createConnection({ host, port });
            socket.setTimeout(Math.min(1_000, Math.max(50, deadline - Date.now())));
            socket.once('connect', () => finish());
            const retry = () => {
                socket?.destroy();
                if (Date.now() >= deadline) {
                    finish(new SupervisorError(`${label} timed out after ${timeoutMs}ms`));
                    return;
                }
                timer = setTimeout(attempt, 50);
            };
            socket.once('error', retry);
            socket.once('timeout', retry);
        };
        signal?.addEventListener('abort', onAbort, { once: true });
        attempt();
    });
}

async function waitForHttp({ url, timeoutMs, signal, label, validate = null, child = null }) {
    const deadline = Date.now() + timeoutMs;
    let lastError = null;
    while (Date.now() < deadline) {
        if (signal?.aborted) {
            throw new SupervisorError(`${label} aborted`, { cause: signal.reason });
        }
        if (child && child.exitCode !== null) {
            throw new SupervisorError(`${label} child exited early with code ${child.exitCode}`);
        }
        const requestController = new AbortController();
        const abortRequest = () => requestController.abort(signal?.reason);
        signal?.addEventListener('abort', abortRequest, { once: true });
        const requestTimer = setTimeout(() => requestController.abort(new Error('request timeout')), 1_000);
        try {
            const response = await fetch(url, { signal: requestController.signal });
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}`);
            }
            const body = await response.text();
            if (validate) {
                await validate(body, response);
            }
            return;
        } catch (error) {
            lastError = error;
        } finally {
            clearTimeout(requestTimer);
            signal?.removeEventListener('abort', abortRequest);
        }
        await delay(75, signal);
    }
    throw new SupervisorError(`${label} timed out after ${timeoutMs}ms`, { cause: lastError });
}

function delay(milliseconds, signal = null) {
    return new Promise((resolve, reject) => {
        const finish = (error = null) => {
            clearTimeout(timer);
            signal?.removeEventListener('abort', onAbort);
            if (error) {
                reject(error);
            } else {
                resolve();
            }
        };
        const timer = setTimeout(() => finish(), milliseconds);
        const onAbort = () => {
            finish(new SupervisorError('Operation aborted', { cause: signal.reason }));
        };
        if (signal?.aborted) {
            onAbort();
            return;
        }
        signal?.addEventListener('abort', onAbort, { once: true });
        timer.unref?.();
    });
}

function spawnRustChild(command, args, { cwd, env, logPath }) {
    const child = spawn(command, args, {
        cwd,
        env,
        shell: false,
        windowsHide: true,
        stdio: ['ignore', 'pipe', 'pipe'],
    });
    const chunks = [];
    child.stdout?.on('data', chunk => chunks.push(chunk.toString()));
    child.stderr?.on('data', chunk => chunks.push(chunk.toString()));
    const flush = () => appendCapped(logPath, chunks);
    child.once('close', flush);
    child.once('error', flush);
    return {
        child,
        command: path.resolve(cwd, command),
        startedAt: new Date().toISOString(),
        logPath,
        flush,
    };
}

function waitForChildExit(child, timeoutMs) {
    if (child.exitCode !== null || child.signalCode !== null) {
        return Promise.resolve();
    }
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
            cleanup();
            reject(new SupervisorError(`PID ${child.pid} did not exit within ${timeoutMs}ms`));
        }, timeoutMs);
        const cleanup = () => {
            clearTimeout(timer);
            child.removeListener('close', onClose);
        };
        const onClose = () => {
            cleanup();
            resolve();
        };
        child.once('close', onClose);
    });
}

async function stopRustChild(rustHandle, recorder, timeoutMs) {
    if (!rustHandle) {
        if (fs.existsSync(recorder.pidPath)) {
            throw new SupervisorError('rust.pid exists but this supervisor did not create a Rust child');
        }
        return { rustPid: null, identityValidated: true, stop: 'not-started' };
    }
    const recordedPid = Number(fs.readFileSync(recorder.pidPath, 'utf8').trim());
    if (recordedPid !== rustHandle.child.pid || recorder.state.rustPid !== rustHandle.child.pid) {
        throw new SupervisorError('Rust PID identity mismatch; refusing to stop an unowned process');
    }
    if (rustHandle.child.exitCode === null && rustHandle.child.signalCode === null) {
        rustHandle.child.kill('SIGTERM');
        try {
            await waitForChildExit(rustHandle.child, timeoutMs);
        } catch {
            rustHandle.child.kill('SIGKILL');
            await waitForChildExit(rustHandle.child, timeoutMs);
        }
    }
    rustHandle.flush();
    return {
        rustPid: recordedPid,
        identityValidated: true,
        stop: rustHandle.child.signalCode ?? `exit-${rustHandle.child.exitCode}`,
        command: rustHandle.command,
        startedAt: rustHandle.startedAt,
    };
}

function removeAuthStorage(authStoragePath) {
    const resolved = path.resolve(authStoragePath);
    fs.rmSync(resolved, { force: true });
    const parent = path.dirname(resolved);
    if (fs.existsSync(parent) && fs.readdirSync(parent).length === 0) {
        fs.rmdirSync(parent);
    }
    return { authStoragePath: resolved, authStorageDeleted: !fs.existsSync(resolved) };
}

function requireLocalHttpUrl(value, label) {
    return requireControlledHttpUrl(value, label, LOCAL_HOSTS);
}

function requireControlledHttpUrl(value, label, allowedHosts) {
    let parsed;
    try {
        parsed = new URL(value);
    } catch (error) {
        throw new SupervisorError(`${label} must be an absolute URL`, { cause: error });
    }
    if (parsed.username || parsed.password) {
        throw new SupervisorError(`${label} must not embed credentials`);
    }
    if (!['http:', 'https:'].includes(parsed.protocol) || !allowedHosts.has(parsed.hostname)) {
        throw new SupervisorError(`${label} must target a controlled HTTP endpoint`);
    }
    return parsed;
}

function parseControlledPostgresEndpoint(value) {
    let parsed;
    try {
        parsed = new URL(value);
    } catch (error) {
        throw new SupervisorError('BILL_ANALYSER_POSTGRES_URL must be an absolute PostgreSQL URL', { cause: error });
    }
    if (!['postgres:', 'postgresql:'].includes(parsed.protocol) || !CONTROLLED_POSTGRES_HOSTS.has(parsed.hostname)) {
        throw new SupervisorError('BILL_ANALYSER_POSTGRES_URL must target a controlled PostgreSQL endpoint');
    }
    const port = Number(parsed.port || 5432);
    if (!Number.isInteger(port) || port <= 0 || port > 65_535) {
        throw new SupervisorError('BILL_ANALYSER_POSTGRES_URL has an invalid port');
    }
    return { host: parsed.hostname, port, evidence: `${parsed.hostname}:${port}` };
}

function parseControlledWeaviateEndpoint(value) {
    const parsed = requireControlledHttpUrl(
        value,
        'BILL_ANALYSER_WEAVIATE_ENDPOINT',
        CONTROLLED_WEAVIATE_HOSTS,
    );
    return {
        endpoint: parsed.toString().replace(/\/+$/u, ''),
        readiness: new URL('/v1/.well-known/ready', parsed).toString(),
    };
}

async function fetchJsonWithTimeout(url, options, timeoutMs, label) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(new SupervisorError(`${label} timed out after ${timeoutMs}ms`)), timeoutMs);
    try {
        const response = await fetch(url, { ...options, signal: controller.signal });
        const text = await response.text();
        let body = null;
        if (text) {
            try {
                body = JSON.parse(text);
            } catch (error) {
                throw new SupervisorError(`${label} returned non-JSON HTTP ${response.status}`, { cause: error });
            }
        }
        if (!response.ok) {
            throw new SupervisorError(`${label} returned HTTP ${response.status}`);
        }
        return body;
    } catch (error) {
        if (controller.signal.aborted && !(error instanceof SupervisorError)) {
            throw new SupervisorError(`${label} timed out after ${timeoutMs}ms`, { cause: error });
        }
        throw error;
    } finally {
        clearTimeout(timer);
    }
}

function unwrapSuccessResult(body, label) {
    if (!body || typeof body !== 'object' || body.success !== true) {
        throw new SupervisorError(`${label} did not return a successful API envelope`);
    }
    if ('result' in body) {
        return body.result;
    }
    if ('data' in body) {
        return body.data;
    }
    return body;
}

async function strictBusinessCleanup(config) {
    const apiBase = config.runtimeEnv.E2E_API_BASE_URL.replace(/\/+$/u, '');
    requireLocalHttpUrl(apiBase, 'E2E_API_BASE_URL');
    const requestTimeoutMs = Math.max(500, Math.floor(config.timeouts.cleanupMs / 4));
    const health = await fetchJsonWithTimeout(
        `${apiBase}/health`,
        { method: 'GET' },
        requestTimeoutMs,
        'strict cleanup health check',
    );
    if (health?.status !== 'ok') {
        throw new SupervisorError('strict cleanup requires backend health status ok');
    }
    if (health?.details?.weaviate_collection_prefix !== config.expectedPrefix) {
        throw new SupervisorError('strict cleanup health prefix does not match the run-scoped Weaviate prefix');
    }

    const loginBody = await fetchJsonWithTimeout(
        `${apiBase}/auth/login`,
        {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({
                loginName: config.runtimeEnv.E2E_ACCOUNT_EMAIL,
                password: config.runtimeEnv.E2E_ACCOUNT_PASSWORD,
            }),
        },
        requestTimeoutMs,
        'strict cleanup account login',
    );
    const auth = unwrapSuccessResult(loginBody, 'strict cleanup account login');
    if (!auth?.token || typeof auth.token !== 'string') {
        throw new SupervisorError('strict cleanup account login returned no access token');
    }
    const headers = {
        authorization: `Bearer ${auth.token}`,
        'content-type': 'application/json',
    };
    const clearBody = await fetchJsonWithTimeout(
        `${apiBase}/data/clear/all`,
        {
            method: 'POST',
            headers,
            body: JSON.stringify({ password: config.runtimeEnv.E2E_ACCOUNT_PASSWORD }),
        },
        requestTimeoutMs,
        'strict business data cleanup',
    );
    if (unwrapSuccessResult(clearBody, 'strict business data cleanup') !== true) {
        throw new SupervisorError('strict business data cleanup did not confirm success');
    }
    const statisticsBody = await fetchJsonWithTimeout(
        `${apiBase}/data/statistics`,
        { method: 'GET', headers: { authorization: `Bearer ${auth.token}` } },
        requestTimeoutMs,
        'strict business data cleanup verification',
    );
    const statistics = unwrapSuccessResult(statisticsBody, 'strict business data cleanup verification');
    const expectedZeroFields = ['billCount', 'accountCount', 'categoryCount', 'tagCount', 'templateCount'];
    const remaining = expectedZeroFields.filter(name => Number(statistics?.[name]) !== 0);
    if (remaining.length > 0) {
        throw new SupervisorError(`strict business cleanup left non-zero counters: ${remaining.join(', ')}`);
    }
    return {
        verified: true,
        clearCounts: clearBody.counts ?? {},
        statistics: Object.fromEntries(expectedZeroFields.map(name => [name, Number(statistics[name])])),
    };
}

async function strictWeaviateCleanup(config) {
    const endpoint = parseControlledWeaviateEndpoint(
        config.runtimeEnv.BILL_ANALYSER_WEAVIATE_ENDPOINT,
    ).endpoint;
    if (!config.expectedPrefix.startsWith('BillAnalyserE2E') || config.expectedPrefix === 'BillAnalyserE2E') {
        throw new SupervisorError(`strict Weaviate cleanup prefix is not run-scoped: ${config.expectedPrefix}`);
    }
    const deadline = Date.now() + config.timeouts.cleanupMs;
    const request = (url, options, label) => {
        const remainingMs = deadline - Date.now();
        if (remainingMs <= 0) {
            throw new SupervisorError(`strict Weaviate cleanup exceeded ${config.timeouts.cleanupMs}ms`);
        }
        return fetchJsonWithTimeout(url, options, remainingMs, label);
    };
    const schemaUrl = `${endpoint}/v1/schema`;
    const schema = await request(schemaUrl, { method: 'GET' }, 'strict Weaviate schema read');
    const names = (schema?.classes ?? [])
        .map(item => item?.class?.trim())
        .filter(name => typeof name === 'string' && name.startsWith(config.expectedPrefix));
    for (const name of names) {
        await request(
            `${schemaUrl}/${encodeURIComponent(name)}`,
            { method: 'DELETE' },
            `strict Weaviate delete ${name}`,
        );
    }
    const verification = await request(schemaUrl, { method: 'GET' }, 'strict Weaviate cleanup verification');
    const remaining = (verification?.classes ?? [])
        .map(item => item?.class?.trim())
        .filter(name => typeof name === 'string' && name.startsWith(config.expectedPrefix));
    if (remaining.length > 0) {
        throw new SupervisorError(`strict Weaviate cleanup left run-scoped classes: ${remaining.join(', ')}`);
    }
    return { verified: true, deletedCollections: names, remainingCollections: [] };
}

async function runStrictCleanupActions(config, definitions) {
    const actions = [];
    for (const [name, action] of definitions) {
        const startedAt = new Date().toISOString();
        try {
            const details = await action();
            if (details?.verified !== true) {
                throw new SupervisorError(`${name} cleanup did not return verified=true`);
            }
            actions.push({
                name,
                attempted: true,
                status: 'success',
                verified: true,
                startedAt,
                finishedAt: new Date().toISOString(),
                details,
                error: null,
            });
        } catch (error) {
            actions.push({
                name,
                attempted: true,
                status: 'failure',
                verified: false,
                startedAt,
                finishedAt: new Date().toISOString(),
                details: {},
                error: serializeError(error, config.runtimeEnv),
                failure: error,
            });
        }
    }
    const failures = actions.filter(action => action.status === 'failure');
    const evidence = {
        schemaVersion: 1,
        runId: config.runId,
        headSha: config.headSha,
        status: failures.length > 0 ? 'failure' : 'success',
        generatedAt: new Date().toISOString(),
        actions: actions.map(({ failure: _failure, ...action }) => action),
    };
    const evidencePath = path.join(config.runDirectory, 'strict-cleanup-evidence.json');
    atomicWriteJson(evidencePath, evidence);
    if (failures.length > 0) {
        throw new AggregateError(
            failures.map(action => action.failure),
            `strict cleanup failed: ${failures.map(action => action.name).join(', ')}`,
        );
    }
    return {
        strictCleanupEvidence: path.basename(evidencePath),
        cleanupActions: evidence.actions.map(action => ({
            name: action.name,
            attempted: action.attempted,
            verified: action.verified,
        })),
    };
}

function aggregateCleanupResults(actions) {
    const failures = actions.filter(item => item.error);
    if (failures.length > 0) {
        throw new AggregateError(failures.map(item => item.error), failures.map(item => item.name).join(', '));
    }
    return Object.assign({}, ...actions.map(item => item.result ?? {}));
}

async function runCleanup({ rustHandle, recorder, config, adapters }) {
    const actions = [];
    for (const [name, action] of [
        ['adapter-cleanup', async () => adapters.cleanup?.()],
        ['rust-child', () => stopRustChild(rustHandle, recorder, config.timeouts.cleanupMs)],
        ['browser-auth-storage', async () => removeAuthStorage(config.authStoragePath)],
        ['adapter-dispose', async () => adapters.dispose?.()],
    ]) {
        try {
            actions.push({ name, result: await action() });
        } catch (error) {
            actions.push({ name, error });
        }
    }
    return aggregateCleanupResults(actions);
}

function installSignalHandlers(controller) {
    const handlers = new Map();
    for (const name of ['SIGINT', 'SIGTERM']) {
        const handler = () => {
            if (!controller.signal.aborted) {
                controller.abort(new SupervisorError(`Received ${name}`));
            }
        };
        handlers.set(name, handler);
        process.once(name, handler);
    }
    return () => {
        for (const [name, handler] of handlers) {
            process.removeListener(name, handler);
        }
    };
}

async function runSupervisor(config, adapters) {
    const recorder = new EvidenceRecorder(config);
    const controller = new AbortController();
    const uninstallSignals = installSignalHandlers(controller);
    let rustHandle = null;
    let primaryError = null;
    let cleanupError = null;

    try {
        const postgresEndpoint = parseControlledPostgresEndpoint(config.runtimeEnv.BILL_ANALYSER_POSTGRES_URL);
        const weaviateEndpoint = parseControlledWeaviateEndpoint(config.runtimeEnv.BILL_ANALYSER_WEAVIATE_ENDPOINT);
        await recorder.runPhase(
            'dependency-readiness',
            { perDependencyMs: config.timeouts.dependencyMs, totalMs: config.timeouts.dependencyTotalMs },
            { postgres: postgresEndpoint.evidence, weaviate: weaviateEndpoint.readiness },
            () => adapters.waitDependencies(controller.signal),
        );
        await recorder.runPhase(
            'start-rust',
            { healthMs: config.timeouts.rustHealthMs, commandMs: config.timeouts.commandMs },
            { buildCommand: COMMANDS.build, rustCommand: COMMANDS.rust },
            async () => {
                await adapters.build(controller.signal);
                rustHandle = await adapters.startRust(controller.signal);
                recorder.recordRust(rustHandle.child.pid);
                await adapters.waitRust(rustHandle, controller.signal);
                return { rustPid: rustHandle.child.pid };
            },
        );
        await recorder.runPhase(
            'desktop-smoke',
            { commandMs: config.timeouts.commandMs },
            { command: COMMANDS.desktop },
            () => adapters.desktop(controller.signal),
        );
        await recorder.runPhase(
            'mobile-smoke',
            { commandMs: config.timeouts.commandMs },
            { command: COMMANDS.mobile },
            () => adapters.mobile(controller.signal),
        );
    } catch (error) {
        primaryError = error;
        recorder.skipPending(publicErrorMessage(error, config.runtimeEnv));
    } finally {
        try {
            await recorder.runPhase(
                'cleanup',
                { cleanupMs: config.timeouts.cleanupMs },
                { strict: true },
                () => runCleanup({ rustHandle, recorder, config, adapters }),
            );
        } catch (error) {
            cleanupError = error;
        }
        uninstallSignals();
        const finalError = cleanupError && primaryError
            ? new AggregateError([primaryError, cleanupError], 'E2E phase and cleanup both failed')
            : cleanupError ?? primaryError;
        recorder.finalize(finalError ? 'failure' : 'success', finalError);
    }

    if (cleanupError && primaryError) {
        throw new AggregateError([primaryError, cleanupError], 'E2E phase and cleanup both failed');
    }
    if (cleanupError) {
        throw cleanupError;
    }
    if (primaryError) {
        throw primaryError;
    }
    return { runDirectory: config.runDirectory, state: recorder.state, phases: recorder.phases };
}

function createRealAdapters(config) {
    const buildLog = path.join(config.runDirectory, 'rust-build.log');
    const rustLog = path.join(config.runDirectory, 'rust-server.log');
    const desktopLog = path.join(config.runDirectory, 'desktop-smoke.log');
    const mobileLog = path.join(config.runDirectory, 'mobile-smoke.log');
    const binary = process.platform === 'win32'
        ? path.join('target', 'debug', 'bill_http_server.exe')
        : path.join('target', 'debug', 'bill_http_server');
    const postgresEndpoint = parseControlledPostgresEndpoint(config.runtimeEnv.BILL_ANALYSER_POSTGRES_URL);
    const weaviateEndpoint = parseControlledWeaviateEndpoint(config.runtimeEnv.BILL_ANALYSER_WEAVIATE_ENDPOINT);
    return {
        async waitDependencies(signal) {
            await Promise.all([
                waitForTcp({
                    host: postgresEndpoint.host,
                    port: postgresEndpoint.port,
                    timeoutMs: config.timeouts.dependencyMs,
                    signal,
                    label: 'PostgreSQL readiness',
                }),
                waitForHttp({
                    url: weaviateEndpoint.readiness,
                    timeoutMs: config.timeouts.dependencyMs,
                    signal,
                    label: 'Weaviate readiness',
                }),
            ]);
        },
        build(signal) {
            return runCommand(executable('cargo'), ['build', '-p', 'bill-analyser-http', '--bin', 'bill_http_server'], {
                cwd: repoRoot,
                env: config.runtimeEnv,
                timeoutMs: config.timeouts.commandMs,
                signal,
                logPath: buildLog,
                label: COMMANDS.build,
            });
        },
        async startRust() {
            return spawnRustChild(binary, [], {
                cwd: repoRoot,
                env: config.runtimeEnv,
                logPath: rustLog,
            });
        },
        waitRust(rustHandle, signal) {
            return waitForHttp({
                url: 'http://127.0.0.1:5000/api/health',
                timeoutMs: config.timeouts.rustHealthMs,
                signal,
                label: 'Rust health readiness',
                child: rustHandle.child,
                validate: text => {
                    const health = JSON.parse(text);
                    if (health.status !== 'ok') {
                        throw new Error(`health status is ${String(health.status)}`);
                    }
                },
            });
        },
        desktop(signal) {
            return runCommand(executable('npm'), ['--prefix', 'src/web', 'run', 'e2e', '--', '--project=desktop-chromium', 'e2e/tests/desktop.route-smoke.spec.ts'], {
                cwd: repoRoot,
                env: config.runtimeEnv,
                timeoutMs: config.timeouts.commandMs,
                signal,
                logPath: desktopLog,
                label: COMMANDS.desktop,
            });
        },
        mobile(signal) {
            return runCommand(executable('npm'), ['--prefix', 'src/web', 'run', 'e2e', '--', '--project=mobile-chromium', 'e2e/tests/mobile.route-smoke.spec.ts'], {
                cwd: repoRoot,
                env: config.runtimeEnv,
                timeoutMs: config.timeouts.commandMs,
                signal,
                logPath: mobileLog,
                label: COMMANDS.mobile,
            });
        },
        cleanup() {
            return runStrictCleanupActions(config, [
                ['business-data', () => strictBusinessCleanup(config)],
                ['weaviate', () => strictWeaviateCleanup(config)],
            ]);
        },
    };
}

function listen(server, host = '127.0.0.1') {
    return new Promise((resolve, reject) => {
        server.once('error', reject);
        server.listen(0, host, () => {
            server.removeListener('error', reject);
            resolve(server.address());
        });
    });
}

function closeServer(server) {
    if (!server?.listening) {
        return Promise.resolve();
    }
    return new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
}

async function closedPort() {
    const server = net.createServer();
    const address = await listen(server);
    await closeServer(server);
    return address.port;
}

async function createControlledAdapters(config) {
    const servers = [];
    let postgresPort;
    if (config.negativeMode === 'missing-postgres') {
        postgresPort = await closedPort();
    } else {
        const postgres = net.createServer(socket => socket.end());
        const address = await listen(postgres);
        servers.push(postgres);
        postgresPort = address.port;
    }

    let weaviatePort;
    if (config.negativeMode === 'missing-weaviate') {
        weaviatePort = await closedPort();
    } else {
        const weaviate = http.createServer((request, response) => {
            if (config.negativeMode === 'readiness-timeout') {
                return;
            }
            response.writeHead(request.url === '/v1/.well-known/ready' ? 200 : 404);
            response.end(request.url === '/v1/.well-known/ready' ? 'READY' : 'not found');
        });
        const address = await listen(weaviate);
        servers.push(weaviate);
        weaviatePort = address.port;
    }

    const rustHealth = http.createServer((request, response) => {
        response.writeHead(request.url === '/api/health' ? 200 : 404, { 'content-type': 'application/json' });
        response.end(request.url === '/api/health' ? JSON.stringify({ status: 'ok' }) : JSON.stringify({ status: 'missing' }));
    });
    const rustAddress = await listen(rustHealth);
    servers.push(rustHealth);

    const runFixture = (mode, label, signal, logName, extraArgs = [], commandOptions = {}) => runCommand(process.execPath, [fixturePath, mode, label, ...extraArgs], {
        cwd: repoRoot,
        env: config.runtimeEnv,
        timeoutMs: commandOptions.timeoutMs ?? config.timeouts.commandMs,
        signal,
        logPath: path.join(config.runDirectory, logName),
        label,
        terminationGraceMs: commandOptions.terminationGraceMs,
        terminationForceMs: commandOptions.terminationForceMs,
    });

    return {
        async waitDependencies(signal) {
            await Promise.all([
                waitForTcp({
                    host: '127.0.0.1',
                    port: postgresPort,
                    timeoutMs: config.timeouts.dependencyMs,
                    signal,
                    label: 'controlled PostgreSQL readiness',
                }),
                waitForHttp({
                    url: `http://127.0.0.1:${weaviatePort}/v1/.well-known/ready`,
                    timeoutMs: config.timeouts.dependencyMs,
                    signal,
                    label: 'controlled Weaviate readiness',
                }),
            ]);
        },
        build(signal) {
            return runFixture('success', 'controlled Rust build', signal, 'rust-build.log');
        },
        async startRust() {
            return spawnRustChild(process.execPath, [fixturePath, 'serve', 'controlled Rust child'], {
                cwd: repoRoot,
                env: config.runtimeEnv,
                logPath: path.join(config.runDirectory, 'rust-server.log'),
            });
        },
        waitRust(rustHandle, signal) {
            return waitForHttp({
                url: `http://127.0.0.1:${rustAddress.port}/api/health`,
                timeoutMs: config.timeouts.rustHealthMs,
                signal,
                label: 'controlled Rust health readiness',
                child: rustHandle.child,
                validate: text => {
                    if (JSON.parse(text).status !== 'ok') {
                        throw new Error('controlled health is not ok');
                    }
                },
            });
        },
        desktop(signal) {
            if (config.forceDesktopTimeout) {
                return runFixture(
                    'spawn-tree',
                    'controlled forced desktop timeout',
                    signal,
                    'desktop-smoke.log',
                    [config.processTreeMarkerPath],
                    { timeoutMs: 150, terminationGraceMs: 1_000, terminationForceMs: 3_000 },
                );
            }
            const mode = config.negativeMode === 'desktop-spec' ? 'failure' : 'success';
            return runFixture(mode, 'controlled desktop spec', signal, 'desktop-smoke.log');
        },
        mobile(signal) {
            const mode = config.negativeMode === 'mobile-spec' ? 'failure' : 'success';
            return runFixture(mode, 'controlled mobile spec', signal, 'mobile-smoke.log');
        },
        async cleanup() {
            return runStrictCleanupActions(config, [
                ['business-data', async () => {
                    if (config.negativeMode === 'cleanup-failure' || config.forceCleanupFailure) {
                        throw new SupervisorError('controlled business cleanup failure');
                    }
                    return { verified: true, statistics: { billCount: 0 } };
                }],
                ['weaviate', async () => ({ verified: true, deletedCollections: [] })],
            ]);
        },
        async dispose() {
            await Promise.all(servers.map(server => closeServer(server)));
        },
    };
}

function readJson(target) {
    return JSON.parse(fs.readFileSync(target, 'utf8'));
}

function readOptionalJson(target) {
    try {
        return readJson(target);
    } catch (error) {
        if (error?.code === 'ENOENT') {
            return null;
        }
        throw error;
    }
}

function sanitizeDiagnosticValue(value, runtimeEnv) {
    if (typeof value === 'string') {
        return truncateUtf8(redactSensitiveText(value, runtimeEnv));
    }
    if (Array.isArray(value)) {
        return value.map(item => sanitizeDiagnosticValue(item, runtimeEnv));
    }
    if (value && typeof value === 'object') {
        return Object.fromEntries(
            Object.entries(value).map(([key, item]) => [key, sanitizeDiagnosticValue(item, runtimeEnv)]),
        );
    }
    return value;
}

function readDiagnosticLogTails(config) {
    if (!fs.existsSync(config.runDirectory)) {
        return [];
    }
    return fs.readdirSync(config.runDirectory, { withFileTypes: true })
        .filter(entry => entry.isFile() && entry.name.endsWith('.log'))
        .sort((left, right) => left.name.localeCompare(right.name))
        .slice(0, MAX_DIAGNOSTIC_LOG_FILES)
        .map(entry => {
            const target = path.join(config.runDirectory, entry.name);
            const buffer = fs.readFileSync(target);
            const truncated = buffer.length > MAX_DIAGNOSTIC_LOG_TAIL_BYTES;
            const tail = buffer.subarray(Math.max(0, buffer.length - MAX_DIAGNOSTIC_LOG_TAIL_BYTES));
            return {
                path: entry.name,
                bytes: buffer.length,
                truncated,
                tail: truncateUtf8(
                    redactSensitiveText(tail.toString('utf8'), config.runtimeEnv),
                    MAX_DIAGNOSTIC_LOG_TAIL_BYTES,
                ),
            };
        });
}

function buildFailureDiagnostic(config, error) {
    const phaseEvidence = readOptionalJson(path.join(config.runDirectory, 'phase-evidence.json'));
    const cleanupEvidence = readOptionalJson(path.join(config.runDirectory, 'strict-cleanup-evidence.json'));
    return {
        schemaVersion: 1,
        runId: config.runId,
        headSha: config.headSha,
        status: 'failure',
        error: serializeError(error, config.runtimeEnv),
        phases: (phaseEvidence?.phases ?? []).map(phase => ({
            name: phase.name,
            outcome: phase.outcome,
            details: sanitizeDiagnosticValue(phase.details ?? {}, config.runtimeEnv),
            error: phase.error ? sanitizeDiagnosticValue(phase.error, config.runtimeEnv) : null,
        })),
        cleanup: cleanupEvidence ? {
            status: cleanupEvidence.status,
            actions: (cleanupEvidence.actions ?? []).map(action => ({
                name: action.name,
                status: action.status,
                attempted: action.attempted,
                verified: action.verified,
                error: action.error ? sanitizeDiagnosticValue(action.error, config.runtimeEnv) : null,
            })),
        } : null,
        logTails: readDiagnosticLogTails(config),
        limits: {
            logTailBytes: MAX_DIAGNOSTIC_LOG_TAIL_BYTES,
            logFiles: MAX_DIAGNOSTIC_LOG_FILES,
            textBytes: MAX_DIAGNOSTIC_TEXT_BYTES,
            outputBytes: MAX_DIAGNOSTIC_OUTPUT_BYTES,
            errorNodes: MAX_ERROR_NODES,
        },
    };
}

function emitFailureDiagnostic(config, error) {
    const diagnostic = buildFailureDiagnostic(config, error);
    const target = path.join(config.runDirectory, 'failure-diagnostic.json');
    atomicWriteJson(target, diagnostic);
    try {
        writeArtifactIndex(config);
    } catch (indexError) {
        fs.rmSync(target, { force: true });
        writeArtifactIndex(config);
        throw indexError;
    }
    const payload = truncateUtf8(JSON.stringify(diagnostic, null, 2), MAX_DIAGNOSTIC_OUTPUT_BYTES);
    console.error(`E2E_FAILURE_DIAGNOSTIC ${payload}`);
    return diagnostic;
}

function isPidAlive(pid) {
    try {
        process.kill(pid, 0);
        return true;
    } catch (error) {
        if (error?.code === 'ESRCH') {
            return false;
        }
        if (error?.code === 'EPERM') {
            return true;
        }
        throw error;
    }
}

function validateScenarioEvidence(config, expectedFailurePhase = null) {
    const state = readJson(path.join(config.runDirectory, 'supervisor-state.json'));
    const evidence = readJson(path.join(config.runDirectory, 'phase-evidence.json'));
    const index = readJson(path.join(config.runDirectory, 'artifact-index.json'));
    const cleanupProof = readJson(path.join(config.runDirectory, 'strict-cleanup-evidence.json'));
    assert.equal(state.runId, config.runId);
    assert.equal(state.headSha, config.headSha);
    assert.deepEqual(evidence.phases.map(phase => phase.name), PHASE_NAMES);
    assert.equal(evidence.phases.find(phase => phase.name === 'cleanup').outcome, expectedFailurePhase === 'cleanup' ? 'failure' : 'success');
    if (expectedFailurePhase) {
        assert.equal(state.status, 'failure');
        assert.equal(evidence.phases.find(phase => phase.name === expectedFailurePhase).outcome, 'failure');
    } else {
        assert.equal(state.status, 'success');
        assert.ok(evidence.phases.every(phase => phase.outcome === 'success'));
    }
    assert.ok(index.artifacts.some(artifact => artifact.path === 'supervisor-state.json'));
    assert.ok(index.artifacts.some(artifact => artifact.path === 'phase-evidence.json'));
    assert.ok(index.artifacts.some(artifact => artifact.path === 'strict-cleanup-evidence.json'));
    assert.ok(index.totalBytes <= index.maxArtifactBytes);
    assert.equal(fs.existsSync(config.authStoragePath), false);
    assert.equal(cleanupProof.runId, config.runId);
    assert.deepEqual(cleanupProof.actions.map(action => action.name), ['business-data', 'weaviate']);
    assert.ok(cleanupProof.actions.every(action => action.attempted === true));
    assert.equal(cleanupProof.status, expectedFailurePhase === 'cleanup' ? 'failure' : 'success');
    if (expectedFailurePhase === 'cleanup') {
        assert.equal(cleanupProof.actions.find(action => action.name === 'business-data').status, 'failure');
        assert.equal(cleanupProof.actions.find(action => action.name === 'weaviate').verified, true);
    } else {
        assert.ok(cleanupProof.actions.every(action => action.verified === true));
    }
}

async function runControlledScenario({
    mode = null,
    evidenceRoot,
    runId,
    forceDesktopTimeout = false,
    forceCleanupFailure = false,
}) {
    const config = createConfig({
        negativeMode: mode,
        evidenceRoot,
        runId,
        controlled: true,
        authStoragePath: path.join(evidenceRoot, 'auth', `${runId}.json`),
        timeouts: {
            dependencyMs: 350,
            dependencyTotalMs: 1_000,
            rustHealthMs: 350,
            commandMs: 2_000,
            cleanupMs: 1_000,
        },
    });
    config.forceDesktopTimeout = forceDesktopTimeout;
    config.forceCleanupFailure = forceCleanupFailure;
    config.processTreeMarkerPath = path.join(config.runDirectory, 'owned-process-tree.json');
    fs.mkdirSync(path.dirname(config.authStoragePath), { recursive: true });
    fs.writeFileSync(config.authStoragePath, '{}\n', 'utf8');
    const adapters = await createControlledAdapters(config);
    return { config, promise: runSupervisor(config, adapters) };
}

async function selfTest() {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bill-e2e-supervisor-'));
    const expectedPhases = new Map([
        ['missing-postgres', 'dependency-readiness'],
        ['missing-weaviate', 'dependency-readiness'],
        ['readiness-timeout', 'dependency-readiness'],
        ['desktop-spec', 'desktop-smoke'],
        ['mobile-spec', 'mobile-smoke'],
        ['cleanup-failure', 'cleanup'],
    ]);
    try {
        assert.equal(
            mergeNoProxy('corp.example,.internal', 'localhost,corp.example'),
            'corp.example,.internal,localhost,127.0.0.1,::1,postgres,weaviate',
        );
        const redactionFixture = {
            BILL_ANALYSER_AUTH_JWT_SECRET: 'jwt-secret-fixture',
            E2E_ACCOUNT_PASSWORD: 'account-password-fixture',
            JWT_SECRET_KEY: 'jwt-segment-fixture',
            AWS_SECRET_ACCESS_KEY: 'aws-secret-fixture',
            SSH_PRIVATE_KEY: 'private-key-fixture',
            AWS_ACCESS_KEY_ID: 'access-key-fixture',
            SAFE_KEY: 'safe-key-visible',
        };
        const redacted = redactSensitiveText(
            'postgresql://user:db-secret@postgres:5432/database Authorization: Bearer bearer-fixture '
                + 'password=account-password-fixture secret=jwt-secret-fixture '
                + 'jwt-segment-fixture aws-secret-fixture private-key-fixture access-key-fixture safe-key-visible',
            redactionFixture,
        );
        for (const secret of [
            'user:db-secret',
            'bearer-fixture',
            'account-password-fixture',
            'jwt-secret-fixture',
            'jwt-segment-fixture',
            'aws-secret-fixture',
            'private-key-fixture',
            'access-key-fixture',
        ]) {
            assert.equal(redacted.includes(secret), false);
        }
        assert.equal(redacted.includes('safe-key-visible'), true);
        assert.match(redacted, /\[REDACTED\]/u);
        const boundedError = serializeError(
            new AggregateError([new Error('x'.repeat(MAX_DIAGNOSTIC_TEXT_BYTES * 2))], 'bounded aggregate'),
            {},
        );
        assert.equal(boundedError.errors.length, 1);
        assert.match(boundedError.errors[0].message, /truncated after/u);
        const wideAggregate = serializeError(new AggregateError(
            Array.from({ length: MAX_ERROR_NODES * 2 }, (_, index) => new Error(`wide-${index}`)),
            'wide aggregate',
        ));
        assert.ok(wideAggregate.errors.length < MAX_ERROR_NODES * 2);
        assert.ok(wideAggregate.omittedErrors > 0);
        assert.ok((JSON.stringify(wideAggregate).match(/"name":/gu) ?? []).length <= MAX_ERROR_NODES);
        assert.equal(
            publicErrorMessage(
                new Error(`${'x'.repeat(MAX_DIAGNOSTIC_TEXT_BYTES * 2)} jwt-segment-fixture`),
                redactionFixture,
            ).includes('jwt-segment-fixture'),
            false,
        );
        assert.deepEqual(
            parseControlledPostgresEndpoint('postgresql://user:secret@postgres:5432/database'),
            { host: 'postgres', port: 5432, evidence: 'postgres:5432' },
        );
        assert.equal(
            parseControlledWeaviateEndpoint('http://weaviate:8080').readiness,
            'http://weaviate:8080/v1/.well-known/ready',
        );
        assert.throws(
            () => parseControlledPostgresEndpoint('postgresql://user:secret@example.com/database'),
            /controlled PostgreSQL endpoint/,
        );
        assert.throws(
            () => parseControlledWeaviateEndpoint('https://example.com'),
            /controlled HTTP endpoint/,
        );
        assert.throws(
            () => parseControlledWeaviateEndpoint('http://user:secret@weaviate:8080'),
            /must not embed credentials/,
        );
        const success = await runControlledScenario({ evidenceRoot: root, runId: 'self-test-success' });
        await success.promise;
        validateScenarioEvidence(success.config);

        for (const [mode, expectedPhase] of expectedPhases) {
            const scenario = await runControlledScenario({
                mode,
                evidenceRoot: root,
                runId: `self-test-${mode}`,
            });
            await assert.rejects(scenario.promise);
            validateScenarioEvidence(scenario.config, expectedPhase);
        }

        const forcedTeardown = await runControlledScenario({
            evidenceRoot: root,
            runId: 'self-test-forced-teardown-cleanup-failure',
            forceDesktopTimeout: true,
            forceCleanupFailure: true,
        });
        let forcedError = null;
        try {
            await forcedTeardown.promise;
        } catch (error) {
            forcedError = error;
        }
        assert.ok(forcedError instanceof AggregateError);
        assert.equal(forcedError.errors.length, 2);
        validateScenarioEvidence(forcedTeardown.config, 'cleanup');
        const forcedState = readJson(path.join(forcedTeardown.config.runDirectory, 'supervisor-state.json'));
        assert.equal(forcedState.error.name, 'AggregateError');
        assert.equal(forcedState.error.errors.length, 2);
        const forcedPhases = readJson(path.join(forcedTeardown.config.runDirectory, 'phase-evidence.json'));
        assert.equal(forcedPhases.phases.find(phase => phase.name === 'desktop-smoke').outcome, 'failure');
        assert.equal(forcedPhases.phases.find(phase => phase.name === 'cleanup').outcome, 'failure');
        const forcedTreePids = readJson(forcedTeardown.config.processTreeMarkerPath);
        assert.equal(isPidAlive(forcedTreePids.parentPid), false);
        assert.equal(isPidAlive(forcedTreePids.grandchildPid), false);
        fs.writeFileSync(
            path.join(forcedTeardown.config.runDirectory, 'credential-fixture.log'),
            `${'padding\n'.repeat(MAX_DIAGNOSTIC_LOG_TAIL_BYTES)}\n`
                + 'postgresql://user:db-secret@postgres:5432/database\n'
                + `Authorization: Bearer bearer-fixture\npassword=${forcedTeardown.config.runtimeEnv.E2E_ACCOUNT_PASSWORD}\n`,
            'utf8',
        );
        const failureDiagnostic = buildFailureDiagnostic(forcedTeardown.config, forcedError);
        const diagnosticText = JSON.stringify(failureDiagnostic);
        for (const secret of ['user:db-secret', 'bearer-fixture', forcedTeardown.config.runtimeEnv.E2E_ACCOUNT_PASSWORD]) {
            assert.equal(diagnosticText.includes(secret), false);
        }
        assert.equal(failureDiagnostic.error.errors.length, 2);
        assert.equal(
            failureDiagnostic.logTails.find(item => item.path === 'credential-fixture.log').truncated,
            true,
        );
        const diagnosticOutput = [];
        const originalConsoleError = console.error;
        try {
            console.error = value => diagnosticOutput.push(String(value));
            emitFailureDiagnostic(forcedTeardown.config, forcedError);
        } finally {
            console.error = originalConsoleError;
        }
        assert.equal(diagnosticOutput.length, 1);
        assert.ok(Buffer.byteLength(diagnosticOutput[0], 'utf8') <= MAX_DIAGNOSTIC_OUTPUT_BYTES + 256);
        assert.equal(diagnosticOutput[0].includes(forcedTeardown.config.runtimeEnv.E2E_ACCOUNT_PASSWORD), false);
        assert.ok(fs.existsSync(path.join(forcedTeardown.config.runDirectory, 'failure-diagnostic.json')));
        const refreshedIndex = readJson(path.join(forcedTeardown.config.runDirectory, 'artifact-index.json'));
        assert.ok(refreshedIndex.artifacts.some(artifact => artifact.path === 'failure-diagnostic.json'));
        assert.ok(refreshedIndex.totalBytes <= refreshedIndex.maxArtifactBytes);

        const markerPath = path.join(root, 'owned-process-tree.json');
        const treeLogPath = path.join(root, 'owned-process-tree.log');
        let treePids = null;
        try {
            await assert.rejects(
                runCommand(process.execPath, [fixturePath, 'spawn-tree', 'controlled process tree', markerPath], {
                    cwd: repoRoot,
                    env: process.env,
                    timeoutMs: 150,
                    signal: null,
                    logPath: treeLogPath,
                    label: 'controlled process tree',
                }),
                /timed out/,
            );
            treePids = readJson(markerPath);
            assert.equal(isPidAlive(treePids.parentPid), false, 'timed-out owned child must be closed before rejection');
            assert.equal(isPidAlive(treePids.grandchildPid), false, 'timed-out owned grandchild must be closed before rejection');
            assert.match(fs.readFileSync(treeLogPath, 'utf8'), /"escalated":true/u);
        } finally {
            for (const pid of [treePids?.grandchildPid, treePids?.parentPid]) {
                if (Number.isInteger(pid) && isPidAlive(pid)) {
                    try {
                        process.kill(pid, 'SIGKILL');
                    } catch (error) {
                        if (error?.code !== 'ESRCH') {
                            throw error;
                        }
                    }
                }
            }
        }
        console.log('PASS deterministic E2E supervisor self-test (success + 6 negative modes + owned process tree)');
    } finally {
        fs.rmSync(root, { recursive: true, force: true });
    }
}

async function main(argv = process.argv.slice(2)) {
    const options = parseArgs(argv);
    if (options.selfTest) {
        await selfTest();
        return;
    }
    if (options.negativeMode) {
        const scenario = await runControlledScenario({
            mode: options.negativeMode,
            evidenceRoot: defaultEvidenceRoot,
            runId: `negative-${options.negativeMode}-${Date.now()}`,
        });
        await scenario.promise;
        throw new SupervisorError(`Negative mode ${options.negativeMode} unexpectedly succeeded`);
    }

    const config = createConfig();
    try {
        const result = await runSupervisor(config, createRealAdapters(config));
        console.log(JSON.stringify({
            status: 'passed',
            runId: config.runId,
            headSha: config.headSha,
            runDirectory: path.relative(repoRoot, result.runDirectory).replace(/\\/gu, '/'),
            phases: result.phases.map(phase => ({ name: phase.name, outcome: phase.outcome })),
        }, null, 2));
    } catch (error) {
        try {
            emitFailureDiagnostic(config, error);
        } catch (diagnosticError) {
            console.error(`E2E_DIAGNOSTIC_FAILURE ${publicErrorMessage(diagnosticError, config.runtimeEnv)}`);
        }
        throw new SupervisorError(publicErrorMessage(error, config.runtimeEnv), { cause: error });
    }
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
    try {
        await main();
    } catch (error) {
        console.error(`FAIL ${publicErrorMessage(error, process.env)}`);
        process.exitCode = 1;
    }
}

export {
    COMMANDS,
    NEGATIVE_MODES,
    PHASE_NAMES,
    createConfig,
    createControlledAdapters,
    createRealAdapters,
    buildFailureDiagnostic,
    emitFailureDiagnostic,
    mergeNoProxy,
    parseArgs,
    redactSensitiveText,
    runCommand,
    runSupervisor,
    selfTest,
};
