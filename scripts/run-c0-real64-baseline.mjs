#!/usr/bin/env node

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import {
    COMMANDS,
    createConfig,
    createRealAdapters,
    emitFailureDiagnostic,
    redactSensitiveText,
    resolveNpmInvocation,
    runCommand,
    runSupervisor,
    writeArtifactIndex,
} from './run-e2e-ci-smoke.mjs';

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), '..');
const C0_EVIDENCE_ROOT = path.join(repoRoot, '.cyaness', 'evidence', 'c0-real64');
const REAL64_SPEC = 'e2e/tests/import-preview-fixes.real64.desktop.spec.ts';
const REAL64_DESKTOP_COMMAND = `npm --prefix src/web run e2e -- --project=desktop-chromium ${REAL64_SPEC}`;
const DATABASE_PREFIX = 'bill_analyser_c0_test_';
const DEFAULT_POSTGRES_CONTAINER = 'bill-analyser-postgres';
const C0_TEXT_ARTIFACT_EXTENSIONS = new Set(['.json', '.log', '.md', '.txt']);
const RAW_URL_PATTERN = /\b[A-Za-z][A-Za-z0-9+.-]*:\/\/[^\s"'`]+/gu;
const WINDOWS_ABSOLUTE_PATH_PATTERN = /(?:\b[A-Za-z]:[\\/]|\\\\)[^\r\n\t"'`]+/gu;
const APPLICATION_SESSION_KEY_PATTERN = /(?:session.?id|import.?session)/iu;
const APPLICATION_SESSION_ROUTE_PATTERN = /(\/api\/bills\/import\/v2\/(?:preview|confirm|session)\/)[A-Za-z0-9._-]+/gu;

function parseArgs(argv) {
    if (argv.length === 0) return { selfTest: false };
    if (argv.length === 1 && argv[0] === '--self-test') return { selfTest: true };
    throw new Error(`Unknown argument: ${argv.join(' ')}`);
}

function safeRunId(value = `c0-${Date.now()}-${process.pid}`) {
    const result = String(value)
        .trim()
        .replace(/[^A-Za-z0-9._-]+/gu, '-')
        .replace(/^-+|-+$/gu, '')
        .slice(0, 64);
    if (!result) throw new Error('C0 run id must contain a safe identifier character.');
    return result;
}

function databaseNameForRun(runId) {
    const suffix = safeRunId(runId)
        .toLowerCase()
        .replace(/[^a-z0-9]+/gu, '_')
        .replace(/^_+|_+$/gu, '')
        .slice(0, 63 - DATABASE_PREFIX.length);
    if (!suffix) throw new Error('C0 run id cannot produce an isolated database name.');
    return `${DATABASE_PREFIX}${suffix}`;
}

function parseContainerInspect(inspectPayload, containerName) {
    if (!/^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/u.test(containerName)) {
        throw new Error('C0 PostgreSQL container name is invalid.');
    }
    const container = Array.isArray(inspectPayload) ? inspectPayload[0] : null;
    if (!container?.State?.Running) {
        throw new Error('C0 PostgreSQL container is not running.');
    }
    const health = container.State.Health?.Status;
    if (health && health !== 'healthy') {
        throw new Error(`C0 PostgreSQL container health is ${health}.`);
    }
    const environment = Object.fromEntries((container.Config?.Env ?? []).map(entry => {
        const separator = entry.indexOf('=');
        return separator === -1 ? [entry, ''] : [entry.slice(0, separator), entry.slice(separator + 1)];
    }));
    const user = environment.POSTGRES_USER?.trim();
    const password = environment.POSTGRES_PASSWORD ?? '';
    if (!user || !password || !/^[A-Za-z0-9_.-]+$/u.test(user)) {
        throw new Error('C0 PostgreSQL container must expose a safe POSTGRES_USER and POSTGRES_PASSWORD.');
    }
    const bindings = container.HostConfig?.PortBindings?.['5432/tcp'];
    const binding = Array.isArray(bindings) ? bindings[0] : null;
    const port = Number(binding?.HostPort);
    const hostIp = String(binding?.HostIp ?? '');
    if (!Number.isInteger(port) || port <= 0 || port > 65535) {
        throw new Error('C0 PostgreSQL container must publish port 5432 to a valid host port.');
    }
    if (!['', '0.0.0.0', '127.0.0.1', '::', '::1'].includes(hostIp)) {
        throw new Error('C0 PostgreSQL container must publish only to a local host binding.');
    }
    return { containerName, user, password, host: '127.0.0.1', port };
}

function inspectPostgresContainer(containerName = DEFAULT_POSTGRES_CONTAINER) {
    let payload;
    try {
        payload = JSON.parse(execFileSync('docker', ['inspect', containerName], {
            cwd: repoRoot,
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'pipe'],
        }));
    } catch (error) {
        throw new Error('Unable to inspect the dedicated Bill Analyser PostgreSQL container.', { cause: error });
    }
    return parseContainerInspect(payload, containerName);
}

function postgresURL(target, databaseName) {
    assertDatabaseName(databaseName);
    return `postgresql://${encodeURIComponent(target.user)}:${encodeURIComponent(target.password)}`
        + `@${target.host}:${target.port}/${databaseName}`;
}

function assertDatabaseName(databaseName) {
    if (!databaseName.startsWith(DATABASE_PREFIX) || !/^[a-z0-9_]{1,63}$/u.test(databaseName)) {
        throw new Error(`C0 database must match ${DATABASE_PREFIX}*.`);
    }
}

function assertIsolatedPostgresURL(value, expectedDatabaseName) {
    const parsed = new URL(value);
    const databaseName = decodeURIComponent(parsed.pathname.replace(/^\/+|\/+$/gu, ''));
    assertDatabaseName(databaseName);
    if (databaseName !== expectedDatabaseName || !['127.0.0.1', 'localhost', '::1'].includes(parsed.hostname)) {
        throw new Error('C0 PostgreSQL URL must target the exact isolated local database.');
    }
}

function runPsql(target, sql) {
    try {
        execFileSync('docker', [
            'exec',
            target.containerName,
            'psql',
            '--username', target.user,
            '--dbname', 'postgres',
            '--set', 'ON_ERROR_STOP=1',
            '--command', sql,
        ], {
            cwd: repoRoot,
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'pipe'],
        });
    } catch (error) {
        throw new Error('Dedicated C0 PostgreSQL lifecycle command failed.', { cause: error });
    }
}

function provisionDatabase(target, databaseName) {
    assertDatabaseName(databaseName);
    runPsql(target, `CREATE DATABASE "${databaseName}"`);
    return postgresURL(target, databaseName);
}

function dropDatabase(target, databaseName) {
    assertDatabaseName(databaseName);
    runPsql(target, `DROP DATABASE IF EXISTS "${databaseName}" WITH (FORCE)`);
}

function createC0Config({ runId, evidenceRoot = C0_EVIDENCE_ROOT, postgresUrl, databaseName }) {
    assertIsolatedPostgresURL(postgresUrl, databaseName);
    const previous = process.env.BILL_ANALYSER_POSTGRES_URL;
    process.env.BILL_ANALYSER_POSTGRES_URL = postgresUrl;
    let config;
    try {
        config = createConfig({
            evidenceRoot,
            runId,
            commands: { ...COMMANDS, desktop: REAL64_DESKTOP_COMMAND },
            timeouts: { commandMs: 20 * 60_000 },
        });
    } finally {
        if (previous === undefined) delete process.env.BILL_ANALYSER_POSTGRES_URL;
        else process.env.BILL_ANALYSER_POSTGRES_URL = previous;
    }
    config.runtimeEnv.E2E_REAL64_EVIDENCE_DIR = config.runDirectory;
    config.runtimeEnv.E2E_ALLOW_SHARED_LOCAL_DATABASE = 'false';
    return config;
}

function createC0Adapters(config, databaseName) {
    const adapters = createRealAdapters(config);
    return {
        ...adapters,
        async build(signal) {
            await adapters.build(signal);
            writeRuntimeProvenance(config, databaseName);
        },
        desktop(signal) {
            const invocation = resolveNpmInvocation([
                '--prefix', 'src/web', 'run', 'e2e', '--',
                '--project=desktop-chromium', REAL64_SPEC,
            ], { npmExecPath: config.runtimeEnv.npm_execpath });
            return runCommand(invocation.command, invocation.args, {
                cwd: repoRoot,
                env: config.runtimeEnv,
                timeoutMs: config.timeouts.commandMs,
                signal,
                logPath: path.join(config.runDirectory, 'desktop-real64.log'),
                label: config.commands.desktop,
            });
        },
    };
}

function sha256File(target) {
    return createHash('sha256').update(fs.readFileSync(target)).digest('hex');
}

function hashText(value) {
    return createHash('sha256').update(value).digest('hex');
}

function writeJson(target, value) {
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

function redactC0Text(value, privateValues) {
    let result = String(value);
    for (const privateValue of [...privateValues].filter(Boolean).sort((left, right) => right.length - left.length)) {
        result = result.replaceAll(privateValue, '[REDACTED_PRIVATE_VALUE]');
    }
    for (const repoPath of [repoRoot, repoRoot.replace(/\\/gu, '/'), repoRoot.replace(/\\/gu, '\\\\')]) {
        result = result.replaceAll(repoPath, '[REDACTED_REPO]');
    }
    result = result.replace(RAW_URL_PATTERN, '[REDACTED_URL]');
    result = result.replace(WINDOWS_ABSOLUTE_PATH_PATTERN, '[REDACTED_PATH]');
    result = result.replace(APPLICATION_SESSION_ROUTE_PATTERN, '$1[REDACTED_SESSION]');
    return result;
}

function sanitizeC0JsonValue(value, privateValues, key = '') {
    if (APPLICATION_SESSION_KEY_PATTERN.test(key)) {
        return '[REDACTED_SESSION]';
    }
    if (typeof value === 'string') {
        return redactC0Text(value, privateValues);
    }
    if (Array.isArray(value)) {
        return value.map(item => sanitizeC0JsonValue(item, privateValues));
    }
    if (value && typeof value === 'object') {
        return Object.fromEntries(
            Object.entries(value).map(([childKey, child]) => [
                childKey,
                sanitizeC0JsonValue(child, privateValues, childKey),
            ]),
        );
    }
    return value;
}

function regexMatches(pattern, value) {
    pattern.lastIndex = 0;
    const matches = pattern.test(value);
    pattern.lastIndex = 0;
    return matches;
}

function assertC0TextPrivacy(text, entryName, privateValues) {
    if (privateValues.some(privateValue => privateValue && text.includes(privateValue))) {
        throw new Error(`C0 artifact ${entryName} contains a private corpus identity.`);
    }
    if (text.includes(repoRoot) || text.includes(repoRoot.replace(/\\/gu, '/'))) {
        throw new Error(`C0 artifact ${entryName} contains the repository absolute path.`);
    }
    if (regexMatches(RAW_URL_PATTERN, text) || regexMatches(WINDOWS_ABSOLUTE_PATH_PATTERN, text)) {
        throw new Error(`C0 artifact ${entryName} contains a raw URL or absolute path.`);
    }
}

function assertC0JsonPrivacy(value, entryName, privateValues, key = '') {
    if (APPLICATION_SESSION_KEY_PATTERN.test(key) && value !== '[REDACTED_SESSION]') {
        throw new Error(`C0 artifact ${entryName} contains an application session identity.`);
    }
    if (typeof value === 'string') {
        assertC0TextPrivacy(value, entryName, privateValues);
        return;
    }
    if (Array.isArray(value)) {
        value.forEach(item => assertC0JsonPrivacy(item, entryName, privateValues));
        return;
    }
    if (value && typeof value === 'object') {
        for (const [childKey, child] of Object.entries(value)) {
            assertC0JsonPrivacy(child, entryName, privateValues, childKey);
        }
    }
}

function assertC0ArtifactPrivacy(config, privateValues) {
    for (const entry of fs.readdirSync(config.runDirectory, { withFileTypes: true })) {
        const extension = path.extname(entry.name).toLowerCase();
        if (!entry.isFile() || !C0_TEXT_ARTIFACT_EXTENSIONS.has(extension)) {
            continue;
        }
        const text = fs.readFileSync(path.join(config.runDirectory, entry.name), 'utf8');
        if (extension === '.json') {
            assertC0JsonPrivacy(JSON.parse(text), entry.name, privateValues);
        } else {
            assertC0TextPrivacy(text, entry.name, privateValues);
        }
    }
}

function sanitizeC0Artifacts(config, privateValues = null) {
    const identities = privateValues ?? fs.readdirSync(path.join(repoRoot, 'bills'));
    for (const entry of fs.readdirSync(config.runDirectory, { withFileTypes: true })) {
        if (!entry.isFile() || !C0_TEXT_ARTIFACT_EXTENSIONS.has(path.extname(entry.name).toLowerCase())) {
            continue;
        }
        const target = path.join(config.runDirectory, entry.name);
        if (path.extname(entry.name).toLowerCase() === '.json') {
            const parsed = JSON.parse(fs.readFileSync(target, 'utf8'));
            writeJson(target, sanitizeC0JsonValue(parsed, identities));
        } else {
            fs.writeFileSync(target, redactC0Text(fs.readFileSync(target, 'utf8'), identities), 'utf8');
        }
    }
    assertC0ArtifactPrivacy(config, identities);
}

function writeRuntimeProvenance(config, databaseName) {
    const binaryRelative = process.platform === 'win32'
        ? path.join('target', 'debug', 'bill_http_server.exe')
        : path.join('target', 'debug', 'bill_http_server');
    const binary = path.join(repoRoot, binaryRelative);
    const stat = fs.statSync(binary);
    const evidence = {
        schemaVersion: 1,
        generatedAt: new Date().toISOString(),
        driver: 'cyanflow',
        runId: config.runId,
        headSha: config.headSha,
        binary: {
            relativePath: binaryRelative.replace(/\\/gu, '/'),
            bytes: stat.size,
            sha256: sha256File(binary),
        },
        runtime: {
            postgres: { isolated: true, databaseNameSha256: hashText(databaseName) },
            weaviate: { enabled: true, runScopedPrefixSha256: hashText(config.expectedPrefix) },
            userSettingsSource: 'dedicated-e2e-defaults',
            llmConfigured: false,
            llmReason: 'No user secrets or saved provider configuration are copied into the isolated database.',
        },
    };
    const serialized = JSON.stringify(evidence);
    if (redactSensitiveText(serialized, config.runtimeEnv) !== serialized) {
        throw new Error('C0 runtime provenance contains a sensitive value.');
    }
    writeJson(path.join(config.runDirectory, 'c0-runtime-provenance.json'), evidence);
}

function writeDatabaseCleanupEvidence(config, databaseName, status, error = null) {
    const evidence = {
        schemaVersion: 1,
        generatedAt: new Date().toISOString(),
        databaseNameSha256: hashText(databaseName),
        status,
        error: error ? redactSensitiveText(errorMessage(error), config.runtimeEnv) : null,
    };
    writeJson(path.join(config.runDirectory, 'c0-database-cleanup.json'), evidence);
}

function errorMessage(error) {
    return error instanceof Error ? error.message : String(error);
}

async function selfTest() {
    const fixture = [{
        State: { Running: true, Health: { Status: 'healthy' } },
        Config: { Env: ['POSTGRES_USER=fixture', 'POSTGRES_PASSWORD=fixture-secret'] },
        HostConfig: { PortBindings: { '5432/tcp': [{ HostIp: '127.0.0.1', HostPort: '55432' }] } },
    }];
    const target = parseContainerInspect(fixture, DEFAULT_POSTGRES_CONTAINER);
    assert.equal(target.user, 'fixture');
    assert.equal(target.port, 55432);
    const databaseName = databaseNameForRun('Self Test 1');
    assert.equal(databaseName, 'bill_analyser_c0_test_self_test_1');
    const url = postgresURL(target, databaseName);
    assertIsolatedPostgresURL(url, databaseName);
    assert.throws(() => assertIsolatedPostgresURL(
        'postgresql://fixture:secret@127.0.0.1:55432/bill_analyser',
        databaseName,
    ), /must match/);
    assert.throws(() => parseContainerInspect([{
        ...fixture[0],
        State: { Running: true, Health: { Status: 'unhealthy' } },
    }], DEFAULT_POSTGRES_CONTAINER), /health is unhealthy/);
    const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'bill-c0-real64-'));
    try {
        const config = createC0Config({
            runId: 'self-test',
            evidenceRoot: temporary,
            postgresUrl: url,
            databaseName,
        });
        assert.equal(config.commands.desktop, REAL64_DESKTOP_COMMAND);
        assert.equal(config.runtimeEnv.E2E_REAL64_EVIDENCE_DIR, config.runDirectory);
        assert.equal(config.runtimeEnv.E2E_ALLOW_SHARED_LOCAL_DATABASE, 'false');
        writeJson(path.join(config.runDirectory, 'privacy-fixture.json'), {
            endpoint: 'http://127.0.0.1:5000/api/private',
            authStoragePath: 'C:\\private\\e2e-user.json',
            command: path.join(repoRoot, 'target', 'debug', 'bill_http_server.exe'),
            sessionId: 'application-session-secret',
        });
        fs.writeFileSync(
            path.join(config.runDirectory, 'privacy-fixture.log'),
            `private-a.csv ${repoRoot} https://example.test/private`,
            'utf8',
        );
        sanitizeC0Artifacts(config, ['private-a.csv']);
        const sanitized = fs.readFileSync(path.join(config.runDirectory, 'privacy-fixture.json'), 'utf8')
            + fs.readFileSync(path.join(config.runDirectory, 'privacy-fixture.log'), 'utf8');
        assert.equal(sanitized.includes('private-a.csv'), false);
        assert.equal(sanitized.includes(repoRoot), false);
        assert.equal(sanitized.includes('http://'), false);
        assert.equal(sanitized.includes('https://'), false);
        assert.equal(sanitized.includes('application-session-secret'), false);
    } finally {
        fs.rmSync(temporary, { recursive: true, force: true });
    }
    console.log('PASS Cyanflow C0 real64 baseline runner self-test');
}

async function main(argv = process.argv.slice(2)) {
    const options = parseArgs(argv);
    if (options.selfTest) {
        await selfTest();
        return;
    }

    const runId = safeRunId(process.env.E2E_RUN_ID);
    const databaseName = databaseNameForRun(runId);
    const target = inspectPostgresContainer(
        process.env.BILL_ANALYSER_C0_POSTGRES_CONTAINER?.trim() || DEFAULT_POSTGRES_CONTAINER,
    );
    let config = null;
    let primaryError = null;
    let cleanupError = null;
    let evidenceError = null;
    let databaseCreated = false;

    try {
        const databaseUrl = provisionDatabase(target, databaseName);
        databaseCreated = true;
        config = createC0Config({ runId, postgresUrl: databaseUrl, databaseName });
        await runSupervisor(config, createC0Adapters(config, databaseName));
    } catch (error) {
        primaryError = error;
        if (config) {
            try {
                emitFailureDiagnostic(config, error);
            } catch {
                // The primary failure remains authoritative; diagnostics are best-effort only here.
            }
        }
    } finally {
        if (databaseCreated) {
            try {
                dropDatabase(target, databaseName);
            } catch (error) {
                cleanupError = error;
            }
        }
        if (config) {
            try {
                writeDatabaseCleanupEvidence(
                    config,
                    databaseName,
                    cleanupError ? 'failure' : 'success',
                    cleanupError,
                );
                sanitizeC0Artifacts(config);
                writeArtifactIndex(config);
            } catch (error) {
                evidenceError = error;
            }
        }
    }

    const failures = [primaryError, cleanupError, evidenceError].filter(Boolean);
    if (failures.length > 1) {
        throw new AggregateError(failures, 'C0 baseline finalization encountered multiple failures');
    }
    if (failures.length === 1) throw failures[0];
    console.log(JSON.stringify({
        status: 'passed',
        runId: config.runId,
        headSha: config.headSha,
        runDirectory: path.relative(repoRoot, config.runDirectory).replace(/\\/gu, '/'),
        databaseCleanup: 'success',
    }, null, 2));
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
    try {
        await main();
    } catch (error) {
        console.error(`FAIL ${redactSensitiveText(errorMessage(error), process.env)}`);
        process.exitCode = 1;
    }
}

export {
    C0_EVIDENCE_ROOT,
    REAL64_DESKTOP_COMMAND,
    assertIsolatedPostgresURL,
    createC0Config,
    databaseNameForRun,
    parseArgs,
    parseContainerInspect,
    selfTest,
};
