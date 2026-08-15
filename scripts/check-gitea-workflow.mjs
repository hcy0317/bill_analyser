#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const defaultRepoRoot = path.resolve(scriptDir, '..');

const RESOLVER_COMMAND = 'node scripts/resolve-ci-diff-refs.mjs resolve --event-name "$GITHUB_EVENT_NAME" --event-path "$GITHUB_EVENT_PATH" --remote origin --default-base refs/heads/main --out .omx/ultragoal/evidence/ci-diff-refs.json';
const READ_MERGE_BASE = 'MERGE_BASE_SHA="$(node scripts/resolve-ci-diff-refs.mjs read --input .omx/ultragoal/evidence/ci-diff-refs.json --field merge_base_sha)"';
const READ_HEAD = 'HEAD_SHA="$(node scripts/resolve-ci-diff-refs.mjs read --input .omx/ultragoal/evidence/ci-diff-refs.json --field head_sha)"';
const RUST_BUSINESS_DIFF_GUARD = 'git -c gc.auto=0 diff --quiet "$MERGE_BASE_SHA...$HEAD_SHA" -- src/backend || rust_business_diff_exit=$?';
const RUST_DIFF = 'git -c gc.auto=0 diff --unified=0 "$MERGE_BASE_SHA...$HEAD_SHA" -- src/backend tests/backend > .omx/ultragoal/evidence/rust-changed.diff';
const RUST_COVERAGE = 'node scripts/governance-normalizers.mjs changed-coverage --lcov workspace.lcov --diff .omx/ultragoal/evidence/rust-changed.diff --threshold 90 --require-matched-files --require-executable-lines';
const RUST_COVERAGE_BLOCK = [
    'rust_business_diff_exit=0',
    RUST_BUSINESS_DIFF_GUARD,
    'if [ "$rust_business_diff_exit" -eq 0 ]; then',
    'echo "Rust business source unchanged; changed-line coverage not applicable."',
    'elif [ "$rust_business_diff_exit" -eq 1 ]; then',
    RUST_DIFF,
    RUST_COVERAGE,
    'else',
    'exit "$rust_business_diff_exit"',
    'fi',
].join('\n');
const FRONTEND_BUSINESS_DIFF_GUARD = 'git -c gc.auto=0 diff --quiet "$MERGE_BASE_SHA...$HEAD_SHA" -- src/web/src || frontend_business_diff_exit=$?';
const FRONTEND_DIFF = 'git -c gc.auto=0 diff --unified=0 "$MERGE_BASE_SHA...$HEAD_SHA" -- src/web tests/web > .omx/ultragoal/evidence/frontend-changed.diff';
const FRONTEND_COVERAGE = 'node scripts/governance-normalizers.mjs changed-coverage --lcov src/web/coverage/lcov.info --diff .omx/ultragoal/evidence/frontend-changed.diff --threshold 90 --require-matched-files --require-executable-lines';
const FRONTEND_COVERAGE_BLOCK = [
    'frontend_business_diff_exit=0',
    FRONTEND_BUSINESS_DIFF_GUARD,
    'if [ "$frontend_business_diff_exit" -eq 0 ]; then',
    'echo "Frontend business source unchanged; changed-line coverage not applicable."',
    'elif [ "$frontend_business_diff_exit" -eq 1 ]; then',
    FRONTEND_DIFF,
    FRONTEND_COVERAGE,
    'else',
    'exit "$frontend_business_diff_exit"',
    'fi',
].join('\n');
const LOCAL_RUST_BUSINESS_DIFF_GUARD = 'git -c gc.auto=0 diff --quiet "${mergeBase}...${head}" -- src/backend';
const LOCAL_FRONTEND_BUSINESS_DIFF_GUARD = 'git -c gc.auto=0 diff --quiet "${mergeBase}...${head}" -- src/web/src';
const ROUTE_COMMAND = 'cargo test -p bill-analyser-http --test runtime_route_ownership_contract -- --nocapture';
const RUST_ONLY_COMMAND = 'node scripts/check-rust-only-source-tree.mjs';
const FRONTEND_COVERAGE_SCRIPT = 'cross-env CI=1 COVERAGE_GATE=1 TS_NODE_PROJECT="./tsconfig.jest.json" jest --maxWorkers=50% --coverage';
const E2E_SUPERVISOR_COMMAND = 'npm --prefix src/web run e2e:ci:smoke';
const E2E_SETUP_STEPS = [
    'Checkout E2E source',
    'Setup E2E Node.js 22',
    'Setup E2E Rust stable',
    'Install locked E2E dependencies',
    'Install Playwright Chromium',
];
const ACTIVE_RESIDUAL_PATTERN = /scripts(?:[/\\]+)hooks|scripts(?:[/\\]+)agent_stack_health\.py/i;

function normalizeCommand(value) {
    return String(value ?? '')
        .replace(/\r\n/g, '\n')
        .split('\n')
        .map(line => line.trim())
        .filter(Boolean)
        .join('\n');
}

function validateFrontendTestScripts(packageJson) {
    const actual = normalizeCommand(packageJson?.scripts?.['test:coverage']);
    if (actual !== FRONTEND_COVERAGE_SCRIPT) {
        throw new Error('src/web test:coverage must run exactly one coverage-enabled Jest command');
    }
    return { test_coverage: actual };
}

function listJsonFiles(directory, prefix) {
    if (!fs.existsSync(directory)) {
        return [];
    }
    return fs.readdirSync(directory, { withFileTypes: true })
        .filter(entry => entry.isFile() && entry.name.endsWith('.json'))
        .map(entry => ({ path: path.join(directory, entry.name), scan: true, label: `${prefix}/${entry.name}` }))
        .sort((left, right) => left.label.localeCompare(right.label));
}

function activeConfigInventory(repoRoot = defaultRepoRoot) {
    const explicit = [
        { relative: '.codex/hooks.json', scan: true, json: true },
        { relative: '.codex/config.toml', scan: true, json: false },
        { relative: 'opencode.json', scan: true, json: true },
        { relative: '.opencode/package.json', scan: false, json: true },
    ].map(item => ({
        path: path.join(repoRoot, item.relative),
        label: item.relative,
        scan: item.scan,
        json: item.json,
        explicit: true,
    }));
    const claudeDirectory = path.join(repoRoot, '.claude');
    const claudeSettings = fs.existsSync(claudeDirectory)
        ? fs.readdirSync(claudeDirectory, { withFileTypes: true })
            .filter(entry => entry.isFile() && /^settings.*\.json$/.test(entry.name))
            .map(entry => ({
                path: path.join(claudeDirectory, entry.name),
                label: `.claude/${entry.name}`,
                scan: true,
                json: true,
                explicit: false,
            }))
        : [];
    const githubHooks = listJsonFiles(path.join(repoRoot, '.github', 'hooks'), '.github/hooks')
        .map(item => ({ ...item, json: true, explicit: false }));
    return [...explicit, ...claudeSettings, ...githubHooks]
        .sort((left, right) => left.label.localeCompare(right.label));
}

function validateActiveConfigs(repoRoot = defaultRepoRoot) {
    const inventory = activeConfigInventory(repoRoot);
    const findings = [];
    for (const item of inventory) {
        if (!fs.existsSync(item.path) || !fs.statSync(item.path).isFile()) {
            if (item.explicit) {
                findings.push(`${item.label}:1 missing required active config`);
            }
            continue;
        }
        const text = fs.readFileSync(item.path, 'utf8');
        if (item.json) {
            try {
                JSON.parse(text);
            } catch (error) {
                findings.push(`${item.label}:1 invalid JSON (${error.message})`);
                continue;
            }
        }
        if (!item.scan) {
            continue;
        }
        text.split(/\r?\n/).forEach((line, index) => {
            if (ACTIVE_RESIDUAL_PATTERN.test(line)) {
                findings.push(`${item.label}:${index + 1} references removed hook/tooling command`);
            }
        });
    }
    if (findings.length > 0) {
        throw new Error(`Active config validation failed:\n${findings.join('\n')}`);
    }
    return inventory.map(item => item.label);
}

function loadLockedYaml(repoRoot = defaultRepoRoot, {
    createRequireFn = createRequire,
    lockData = null,
} = {}) {
    const packagePath = path.join(repoRoot, 'src', 'web', 'package.json');
    const lockPath = path.join(repoRoot, 'src', 'web', 'package-lock.json');
    if (!fs.existsSync(packagePath)) {
        throw new Error('Missing src/web/package.json for locked YAML resolution');
    }
    const lock = lockData ?? JSON.parse(fs.readFileSync(lockPath, 'utf8'));
    const lockedPackage = lock?.packages?.['node_modules/js-yaml'];
    if (!lockedPackage?.version) {
        throw new Error('src/web/package-lock.json is missing node_modules/js-yaml');
    }

    let scopedRequire;
    let packageJsonPath;
    let resolvedPackage;
    let yaml;
    try {
        scopedRequire = createRequireFn(packagePath);
        packageJsonPath = scopedRequire.resolve('js-yaml/package.json');
        resolvedPackage = scopedRequire(packageJsonPath);
        yaml = scopedRequire('js-yaml');
    } catch (error) {
        throw new Error(`Unable to resolve lockfile-pinned js-yaml from src/web/package.json: ${error.message}`);
    }
    if (resolvedPackage.version !== lockedPackage.version) {
        throw new Error(`js-yaml lock/resolution mismatch: lock=${lockedPackage.version}, resolved=${resolvedPackage.version}`);
    }
    const expectedRoot = path.join(repoRoot, 'src', 'web', 'node_modules', 'js-yaml');
    const relative = path.relative(expectedRoot, packageJsonPath);
    if (relative.startsWith('..') || path.isAbsolute(relative)) {
        throw new Error(`js-yaml resolved outside src/web node_modules: ${packageJsonPath}`);
    }
    if (typeof yaml?.load !== 'function') {
        throw new Error('Resolved js-yaml does not expose load()');
    }
    return {
        parser: yaml,
        version: resolvedPackage.version,
        packageJsonPath,
    };
}

function requiredJob(workflow, jobId) {
    const job = workflow?.jobs?.[jobId];
    if (!job) {
        throw new Error(`Missing required job ${jobId}`);
    }
    if (job.if !== undefined) {
        throw new Error(`Required job ${jobId} must not be condition-skipped`);
    }
    if (job.needs !== undefined) {
        throw new Error(`Required job ${jobId} must remain independent`);
    }
    if (!Array.isArray(job.steps)) {
        throw new Error(`Required job ${jobId} has no steps`);
    }
    return job;
}

function requiredStep(job, jobId, name, exactCommand = null) {
    const matches = job.steps.filter(step => step?.name === name);
    if (matches.length !== 1) {
        throw new Error(`${jobId} must contain exactly one required step ${name}`);
    }
    const step = matches[0];
    if (step.if !== undefined) {
        throw new Error(`${jobId}/${name} must not be condition-skipped`);
    }
    if (exactCommand !== null && normalizeCommand(step.run) !== normalizeCommand(exactCommand)) {
        throw new Error(`${jobId}/${name} command drift`);
    }
    return step;
}

function requireFragments(step, jobId, fragments) {
    const command = normalizeCommand(step.run);
    let previous = -1;
    for (const fragment of fragments) {
        const normalized = normalizeCommand(fragment);
        const index = command.indexOf(normalized);
        if (index === -1) {
            throw new Error(`${jobId}/${step.name} is missing required command: ${normalized}`);
        }
        if (index <= previous) {
            throw new Error(`${jobId}/${step.name} command order drift at: ${normalized}`);
        }
        previous = index;
    }
}

function stepIndex(job, name) {
    return job.steps.findIndex(step => step?.name === name);
}

function validateCheckout(job, jobId) {
    const checkout = job.steps.find(step => step?.uses === 'https://github.com/actions/checkout@v4');
    if (!checkout) {
        throw new Error(`${jobId} is missing absolute actions/checkout@v4`);
    }
    if (checkout.if !== undefined) {
        throw new Error(`${jobId} checkout must not be skipped`);
    }
    if (Number(checkout.with?.['fetch-depth']) !== 0) {
        throw new Error(`${jobId} checkout must use fetch-depth: 0`);
    }
}

function validateCleanupOrder(job, jobId, gateName, cleanupName) {
    const gate = stepIndex(job, gateName);
    const cleanup = stepIndex(job, cleanupName);
    if (gate === -1 || cleanup === -1 || gate >= cleanup) {
        throw new Error(`${jobId} must run ${gateName} before ${cleanupName}`);
    }
}

function validateRequiredE2e(workflow) {
    const job = requiredJob(workflow, 'e2e-ci');
    if (job['runs-on'] !== 'ubuntu-latest') {
        throw new Error('e2e-ci must run on ubuntu-latest');
    }
    const postgres = job.services?.postgres;
    const weaviate = job.services?.weaviate;
    if (postgres?.image !== 'postgres:16-alpine') {
        throw new Error('e2e-ci postgres service image/tag drift');
    }
    if (postgres?.ports !== undefined) {
        throw new Error('e2e-ci must not publish a fixed PostgreSQL host port');
    }
    if (postgres?.env?.POSTGRES_DB !== 'bill_analyser_e2e') {
        throw new Error('e2e-ci postgres database drift');
    }
    if (postgres?.env?.POSTGRES_USER !== 'bill_analyser_e2e'
        || postgres?.env?.POSTGRES_PASSWORD !== 'bill_analyser_e2e') {
        throw new Error('e2e-ci postgres test credentials drift');
    }
    if (weaviate?.image !== 'cr.weaviate.io/semitechnologies/weaviate:1.37.4') {
        throw new Error('e2e-ci Weaviate service image/tag drift');
    }
    if (weaviate?.ports !== undefined) {
        throw new Error('e2e-ci must not publish a fixed Weaviate host port');
    }
    const expectedEnv = {
        BILL_ANALYSER_DATABASE_BACKEND: 'postgres',
        BILL_ANALYSER_POSTGRES_URL: 'postgresql://bill_analyser_e2e:bill_analyser_e2e@postgres:5432/bill_analyser_e2e',
        BILL_ANALYSER_WEAVIATE_ENABLED: 'true',
        BILL_ANALYSER_WEAVIATE_ENDPOINT: 'http://weaviate:8080',
        BILL_ANALYSER_HTTP_BIND: '127.0.0.1:5000',
        BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION: 'true',
        BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION: 'false',
        E2E_STRICT_CLEANUP: 'true',
        E2E_ALLOW_CI_SERVICE_HOSTS: 'true',
    };
    for (const [name, expected] of Object.entries(expectedEnv)) {
        if (String(job.env?.[name] ?? '') !== expected) {
            throw new Error(`e2e-ci environment ${name} drift`);
        }
    }
    for (const name of ['BILL_ANALYSER_AUTH_JWT_SECRET', 'E2E_RUN_ID', 'E2E_EXPECTED_WEAVIATE_PREFIX', 'BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX']) {
        if (!String(job.env?.[name] ?? '').trim()) {
            throw new Error(`e2e-ci environment ${name} must be non-empty`);
        }
    }
    if (job.env.E2E_EXPECTED_WEAVIATE_PREFIX !== job.env.BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX
        || !String(job.env.E2E_EXPECTED_WEAVIATE_PREFIX).startsWith('BillAnalyserE2E')) {
        throw new Error('e2e-ci Weaviate collection prefix drift');
    }

    const setupNames = job.steps
        .filter(step => step?.name !== 'Run deterministic E2E supervisor')
        .map(step => step?.name);
    assert.deepEqual(setupNames, E2E_SETUP_STEPS, 'e2e-ci setup step inventory drift');
    for (const setupName of E2E_SETUP_STEPS) {
        requiredStep(job, 'e2e-ci', setupName);
    }
    validateCheckout(job, 'e2e-ci');
    const setupNode = requiredStep(job, 'e2e-ci', 'Setup E2E Node.js 22');
    if (setupNode.uses !== 'https://github.com/actions/setup-node@v4'
        || String(setupNode.with?.['node-version']) !== '22') {
        throw new Error('e2e-ci Node.js setup drift');
    }
    const setupRust = requiredStep(job, 'e2e-ci', 'Setup E2E Rust stable');
    if (setupRust.uses !== 'https://github.com/dtolnay/rust-toolchain@stable') {
        throw new Error('e2e-ci Rust setup drift');
    }
    requiredStep(job, 'e2e-ci', 'Install locked E2E dependencies', 'npm ci --prefix src/web');
    requiredStep(job, 'e2e-ci', 'Install Playwright Chromium', 'npm --prefix src/web run e2e:install -- --with-deps');
    requiredStep(job, 'e2e-ci', 'Run deterministic E2E supervisor', E2E_SUPERVISOR_COMMAND);
}

function validateLocalCiScript(text) {
    const required = [
        'node scripts/resolve-ci-diff-refs.mjs --self-test',
        'node scripts/check-governance-normalizers.mjs',
        'node scripts/check-gitea-workflow.mjs --self-test',
        'node scripts/check-gitea-workflow.mjs',
        'node scripts/check-rust-backend-structure.mjs',
        ROUTE_COMMAND,
        LOCAL_RUST_BUSINESS_DIFF_GUARD,
        RUST_COVERAGE,
        'npm --prefix src/web run structure:check',
        LOCAL_FRONTEND_BUSINESS_DIFF_GUARD,
        FRONTEND_COVERAGE,
        'trim_ci_caches.ps1',
        '[scriptblock]::Create($Command)',
        'throw "$Name failed with exit code $childExitCode"',
        'PASS local CI fail-closed self-test',
    ];
    for (const fragment of required) {
        if (!text.includes(fragment)) {
            throw new Error(`scripts/run_ci_local.ps1 is missing: ${fragment}`);
        }
    }
    requireFragments({
        name: 'Rust changed-line coverage',
        run: text,
    }, 'local-ci', [
        LOCAL_RUST_BUSINESS_DIFF_GUARD,
        '$rustBusinessDiffExit = $LASTEXITCODE',
        'if ($rustBusinessDiffExit -eq 0)',
        'elseif ($rustBusinessDiffExit -eq 1)',
        RUST_COVERAGE,
        'else { exit $rustBusinessDiffExit }',
    ]);
    requireFragments({
        name: 'Frontend changed-line coverage',
        run: text,
    }, 'local-ci', [
        LOCAL_FRONTEND_BUSINESS_DIFF_GUARD,
        '$frontendBusinessDiffExit = $LASTEXITCODE',
        'if ($frontendBusinessDiffExit -eq 0)',
        'elseif ($frontendBusinessDiffExit -eq 1)',
        FRONTEND_COVERAGE,
        'else { exit $frontendBusinessDiffExit }',
    ]);
    const trim = text.lastIndexOf('trim_ci_caches.ps1');
    for (const gate of [RUST_COVERAGE, FRONTEND_COVERAGE]) {
        const gateIndex = text.indexOf(gate);
        if (gateIndex === -1 || trim === -1 || gateIndex >= trim) {
            throw new Error('scripts/run_ci_local.ps1 must run changed coverage before cache trim');
        }
    }
}

function validateWorkflow(workflow) {
    if (!workflow || typeof workflow !== 'object') {
        throw new Error('Workflow YAML did not parse to an object');
    }
    if (workflow.concurrency !== undefined) {
        throw new Error('Gitea workflow must not use top-level concurrency');
    }
    const dispatch = workflow.on?.workflow_dispatch;
    if (!dispatch || typeof dispatch !== 'object' || !dispatch.inputs?.base_sha) {
        throw new Error('workflow_dispatch must declare optional base_sha input');
    }

    for (const [jobId, job] of Object.entries(workflow.jobs ?? {})) {
        if (job?.['timeout-minutes'] !== undefined || job?.environment !== undefined) {
            throw new Error(`${jobId} uses an unsupported Gitea job field`);
        }
        for (const step of job?.steps ?? []) {
            if (step?.['continue-on-error'] !== undefined) {
                throw new Error(`${jobId}/${step.name ?? '<unnamed>'} must not continue on error`);
            }
            if (step?.uses && !String(step.uses).startsWith('https://github.com/')) {
                throw new Error(`${jobId}/${step.name ?? '<unnamed>'} must use an absolute GitHub action URL`);
            }
            const command = String(step?.run ?? '');
            if (
                step?.name === 'Check removed runtime/tooling references'
                || /paths=["'][^\r\n]*AGENTS\.md[^\r\n]*\bdocs\b/.test(command)
            ) {
                throw new Error(`${jobId}/${step.name ?? '<unnamed>'} uses a forbidden broad residual scan`);
            }
        }
    }

    const backend = requiredJob(workflow, 'backend-ci');
    const frontend = requiredJob(workflow, 'frontend-ci');
    const governance = requiredJob(workflow, 'repo-governance');
    for (const [jobId, job] of [['backend-ci', backend], ['frontend-ci', frontend], ['repo-governance', governance]]) {
        validateCheckout(job, jobId);
    }

    requiredStep(backend, 'backend-ci', 'Resolve immutable CI diff refs', RESOLVER_COMMAND);
    requiredStep(backend, 'backend-ci', 'Run Rust backend structure check', 'node scripts/check-rust-backend-structure.mjs');
    requiredStep(backend, 'backend-ci', 'Run assembled runtime route ownership contract', ROUTE_COMMAND);
    const rustCoverage = requiredStep(backend, 'backend-ci', 'Enforce Rust changed-line coverage');
    requireFragments(rustCoverage, 'backend-ci', [READ_MERGE_BASE, READ_HEAD, RUST_COVERAGE_BLOCK]);
    validateCleanupOrder(backend, 'backend-ci', 'Enforce Rust changed-line coverage', 'Trim backend caches before cache save');

    requiredStep(frontend, 'frontend-ci', 'Resolve immutable CI diff refs', RESOLVER_COMMAND);
    requiredStep(frontend, 'frontend-ci', 'Run frontend structure check', 'npm --prefix src/web run structure:check');
    const frontendCoverage = requiredStep(frontend, 'frontend-ci', 'Enforce frontend changed-line coverage');
    requireFragments(frontendCoverage, 'frontend-ci', [READ_MERGE_BASE, READ_HEAD, FRONTEND_COVERAGE_BLOCK]);
    validateCleanupOrder(frontend, 'frontend-ci', 'Enforce frontend changed-line coverage', 'Trim frontend caches before cache save');

    requiredStep(governance, 'repo-governance', 'Install locked workflow checker dependencies', 'npm ci --prefix src/web');
    requiredStep(governance, 'repo-governance', 'Self-test immutable CI diff resolver', 'node scripts/resolve-ci-diff-refs.mjs --self-test');
    requiredStep(governance, 'repo-governance', 'Self-test Gitea workflow checker', 'node scripts/check-gitea-workflow.mjs --self-test');
    requiredStep(governance, 'repo-governance', 'Check Gitea workflow', 'node scripts/check-gitea-workflow.mjs');
    requiredStep(governance, 'repo-governance', 'Ensure tracked source tree is Rust-only', RUST_ONLY_COMMAND);
    const installIndex = stepIndex(governance, 'Install locked workflow checker dependencies');
    const checkerIndex = stepIndex(governance, 'Check Gitea workflow');
    if (installIndex === -1 || checkerIndex === -1 || installIndex >= checkerIndex) {
        throw new Error('repo-governance must install locked dependencies before workflow checker');
    }
    validateRequiredE2e(workflow);
    return {
        jobs: Object.keys(workflow.jobs),
        route_command: ROUTE_COMMAND,
        immutable_resolver_command: RESOLVER_COMMAND,
    };
}

function workflowFixture() {
    const checkout = () => ({
        name: 'Checkout',
        uses: 'https://github.com/actions/checkout@v4',
        with: { 'fetch-depth': 0 },
    });
    return {
        name: 'CI fixture',
        on: { workflow_dispatch: { inputs: { base_sha: { required: false } } } },
        jobs: {
            'backend-ci': {
                'runs-on': 'ubuntu-latest',
                steps: [
                    checkout(),
                    { name: 'Resolve immutable CI diff refs', run: RESOLVER_COMMAND },
                    { name: 'Run Rust backend structure check', run: 'node scripts/check-rust-backend-structure.mjs' },
                    { name: 'Run assembled runtime route ownership contract', run: ROUTE_COMMAND },
                    {
                        name: 'Enforce Rust changed-line coverage',
                        run: [READ_MERGE_BASE, READ_HEAD, RUST_COVERAGE_BLOCK].join('\n'),
                    },
                    { name: 'Trim backend caches before cache save', run: 'true' },
                ],
            },
            'frontend-ci': {
                'runs-on': 'ubuntu-latest',
                steps: [
                    checkout(),
                    { name: 'Resolve immutable CI diff refs', run: RESOLVER_COMMAND },
                    { name: 'Run frontend structure check', run: 'npm --prefix src/web run structure:check' },
                    {
                        name: 'Enforce frontend changed-line coverage',
                        run: [READ_MERGE_BASE, READ_HEAD, FRONTEND_COVERAGE_BLOCK].join('\n'),
                    },
                    { name: 'Trim frontend caches before cache save', run: 'true' },
                ],
            },
            'repo-governance': {
                'runs-on': 'ubuntu-latest',
                steps: [
                    checkout(),
                    { name: 'Install locked workflow checker dependencies', run: 'npm ci --prefix src/web' },
                    { name: 'Self-test immutable CI diff resolver', run: 'node scripts/resolve-ci-diff-refs.mjs --self-test' },
                    { name: 'Self-test Gitea workflow checker', run: 'node scripts/check-gitea-workflow.mjs --self-test' },
                    { name: 'Check Gitea workflow', run: 'node scripts/check-gitea-workflow.mjs' },
                    { name: 'Ensure tracked source tree is Rust-only', run: RUST_ONLY_COMMAND },
                ],
            },
            'e2e-ci': {
                'runs-on': 'ubuntu-latest',
                env: {
                    BILL_ANALYSER_DATABASE_BACKEND: 'postgres',
                    BILL_ANALYSER_POSTGRES_URL: 'postgresql://bill_analyser_e2e:bill_analyser_e2e@postgres:5432/bill_analyser_e2e',
                    BILL_ANALYSER_WEAVIATE_ENABLED: 'true',
                    BILL_ANALYSER_WEAVIATE_ENDPOINT: 'http://weaviate:8080',
                    BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX: 'BillAnalyserE2Efixture',
                    BILL_ANALYSER_HTTP_BIND: '127.0.0.1:5000',
                    BILL_ANALYSER_AUTH_JWT_SECRET: 'fixture-secret',
                    BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION: 'true',
                    BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION: 'false',
                    E2E_RUN_ID: 'fixture',
                    E2E_EXPECTED_WEAVIATE_PREFIX: 'BillAnalyserE2Efixture',
                    E2E_STRICT_CLEANUP: 'true',
                    E2E_ALLOW_CI_SERVICE_HOSTS: 'true',
                },
                services: {
                    postgres: {
                        image: 'postgres:16-alpine',
                        env: {
                            POSTGRES_DB: 'bill_analyser_e2e',
                            POSTGRES_USER: 'bill_analyser_e2e',
                            POSTGRES_PASSWORD: 'bill_analyser_e2e',
                        },
                    },
                    weaviate: {
                        image: 'cr.weaviate.io/semitechnologies/weaviate:1.37.4',
                    },
                },
                steps: [
                    { ...checkout(), name: 'Checkout E2E source' },
                    { name: 'Setup E2E Node.js 22', uses: 'https://github.com/actions/setup-node@v4', with: { 'node-version': '22' } },
                    { name: 'Setup E2E Rust stable', uses: 'https://github.com/dtolnay/rust-toolchain@stable' },
                    { name: 'Install locked E2E dependencies', run: 'npm ci --prefix src/web' },
                    { name: 'Install Playwright Chromium', run: 'npm --prefix src/web run e2e:install -- --with-deps' },
                    { name: 'Run deterministic E2E supervisor', run: E2E_SUPERVISOR_COMMAND },
                ],
            },
        },
    };
}

function clone(value) {
    return JSON.parse(JSON.stringify(value));
}

function expectWorkflowFailure(base, mutate, pattern) {
    const fixture = clone(base);
    mutate(fixture);
    assert.throws(() => validateWorkflow(fixture), pattern);
}

function writeActiveFixture(root) {
    const files = {
        '.codex/hooks.json': '{}\n',
        '.codex/config.toml': 'model = "fixture"\n',
        'opencode.json': '{}\n',
        '.opencode/package.json': '{"note":"scripts/hooks is explanatory only"}\n',
        '.claude/settings.json': '{}\n',
        '.github/hooks/repo-guard.json': '{}\n',
        'docs/note.md': 'scripts/hooks and scripts/agent_stack_health.py are historical documentation\n',
    };
    for (const [relative, content] of Object.entries(files)) {
        const target = path.join(root, relative);
        fs.mkdirSync(path.dirname(target), { recursive: true });
        fs.writeFileSync(target, content, 'utf8');
    }
}

function selfTest(parser) {
    const fixture = workflowFixture();
    const roundTrip = parser.load(parser.dump(fixture));
    validateWorkflow(roundTrip);
    assert.throws(() => parser.load('jobs:\n  broken: [\n'), /unexpected end|unexpected end of the stream|missed comma/i);

    validateFrontendTestScripts({ scripts: { 'test:coverage': FRONTEND_COVERAGE_SCRIPT } });
    assert.throws(
        () => validateFrontendTestScripts({
            scripts: { 'test:coverage': `npm run test && ${FRONTEND_COVERAGE_SCRIPT}` },
        }),
        /exactly one coverage-enabled Jest command/,
    );
    assert.throws(
        () => validateFrontendTestScripts({
            scripts: { 'test:coverage': FRONTEND_COVERAGE_SCRIPT.replace('COVERAGE_GATE=1 ', '') },
        }),
        /exactly one coverage-enabled Jest command/,
    );
    assert.throws(
        () => validateFrontendTestScripts({
            scripts: { 'test:coverage': FRONTEND_COVERAGE_SCRIPT.replace(' --coverage', '') },
        }),
        /exactly one coverage-enabled Jest command/,
    );

    expectWorkflowFailure(fixture, value => {
        value.jobs['backend-ci'].steps = value.jobs['backend-ci'].steps.filter(step => step.name !== 'Run assembled runtime route ownership contract');
    }, /assembled runtime route ownership/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['backend-ci'].steps.find(step => step.name === 'Run assembled runtime route ownership contract').if = 'false';
    }, /condition-skipped/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['backend-ci'].steps.find(step => step.name === 'Run assembled runtime route ownership contract').run += ' --ignored';
    }, /command drift/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['backend-ci'].steps.find(step => step.uses).with['fetch-depth'] = 1;
    }, /fetch-depth/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['backend-ci'].steps = value.jobs['backend-ci'].steps.filter(step => step.name !== 'Run Rust backend structure check');
    }, /Rust backend structure check/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['backend-ci'].steps.find(step => step.name === 'Run Rust backend structure check').if = 'false';
    }, /condition-skipped/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['backend-ci'].steps.find(step => step.name === 'Run Rust backend structure check').run = 'node scripts/check-rust-backend-structure.mjs --print-baseline';
    }, /command drift/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['frontend-ci'].steps = value.jobs['frontend-ci'].steps.filter(step => step.name !== 'Run frontend structure check');
    }, /frontend structure check/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['frontend-ci'].steps.find(step => step.name === 'Run frontend structure check').if = 'false';
    }, /condition-skipped/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['frontend-ci'].steps.find(step => step.name === 'Run frontend structure check').run = 'npm --prefix src/web run lint:ci';
    }, /command drift/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['frontend-ci'].steps.find(step => step.name === 'Enforce frontend changed-line coverage').run = 'echo skipped';
    }, /missing required command/);
    expectWorkflowFailure(fixture, value => {
        const step = value.jobs['backend-ci'].steps.find(item => item.name === 'Enforce Rust changed-line coverage');
        step.run = step.run.replace(RUST_BUSINESS_DIFF_GUARD, RUST_BUSINESS_DIFF_GUARD.replace('src/backend', 'tests/backend'));
    }, /src\/backend/);
    expectWorkflowFailure(fixture, value => {
        const step = value.jobs['backend-ci'].steps.find(item => item.name === 'Enforce Rust changed-line coverage');
        step.run = step.run.replace(`${RUST_BUSINESS_DIFF_GUARD}\n`, '');
    }, /src\/backend/);
    expectWorkflowFailure(fixture, value => {
        const step = value.jobs['frontend-ci'].steps.find(item => item.name === 'Enforce frontend changed-line coverage');
        step.run = step.run.replace(FRONTEND_BUSINESS_DIFF_GUARD, FRONTEND_BUSINESS_DIFF_GUARD.replace('src/web/src', 'src/web/e2e'));
    }, /src\/web\/src/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['backend-ci'].steps.reverse();
    }, /must run Enforce Rust changed-line coverage before/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['repo-governance'].steps.find(step => step.name === 'Install locked workflow checker dependencies').run = 'npm install js-yaml';
    }, /command drift/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['repo-governance'].steps = value.jobs['repo-governance'].steps.filter(step => step.name !== 'Ensure tracked source tree is Rust-only');
    }, /tracked source tree is Rust-only/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['repo-governance'].steps.find(step => step.name === 'Ensure tracked source tree is Rust-only').run = 'git ls-files *.py';
    }, /command drift/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['repo-governance'].steps.push({
            name: 'Renamed broad scan',
            run: 'paths="AGENTS.md CLAUDE.md docs scripts"; git grep legacy -- $paths',
        });
    }, /forbidden broad residual scan/);

    expectWorkflowFailure(fixture, value => {
        delete value.jobs['e2e-ci'];
    }, /Missing required job e2e-ci/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].if = 'false';
    }, /condition-skipped/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].steps.find(step => step.name === 'Run deterministic E2E supervisor').run += ' --drift';
    }, /command drift/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].steps = value.jobs['e2e-ci'].steps.filter(step => step.name !== 'Run deterministic E2E supervisor');
    }, /setup step inventory drift|deterministic E2E supervisor/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].steps.find(step => step.name === 'Run deterministic E2E supervisor').if = 'false';
    }, /condition-skipped/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].services.weaviate.image = 'weaviate:latest';
    }, /Weaviate service image/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].services.postgres.image = 'postgres:16';
    }, /postgres service image/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].services.postgres.ports = ['5432:5432'];
    }, /fixed PostgreSQL host port/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].services.weaviate.ports = ['8088:8080'];
    }, /fixed Weaviate host port/);
    expectWorkflowFailure(fixture, value => {
        value.jobs['e2e-ci'].services.postgres.env.POSTGRES_PASSWORD = 'wrong';
    }, /test credentials/);

    const activeRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'bill-active-config-'));
    try {
        writeActiveFixture(activeRoot);
        validateActiveConfigs(activeRoot);
        fs.writeFileSync(path.join(activeRoot, '.codex', 'hooks.json'), '{\n  "command": "scripts/hooks/legacy.py"\n}\n');
        assert.throws(() => validateActiveConfigs(activeRoot), /.codex\/hooks.json:2/);
        fs.writeFileSync(path.join(activeRoot, '.codex', 'hooks.json'), '{}\n');
        fs.writeFileSync(path.join(activeRoot, '.codex', 'config.toml'), 'command = "scripts\\\\agent_stack_health.py"\n');
        assert.throws(() => validateActiveConfigs(activeRoot), /.codex\/config.toml:1/);
        fs.writeFileSync(path.join(activeRoot, '.codex', 'config.toml'), 'model = "fixture"\n');
        fs.writeFileSync(path.join(activeRoot, 'opencode.json'), '{bad json');
        assert.throws(() => validateActiveConfigs(activeRoot), /invalid JSON/);
    } finally {
        fs.rmSync(activeRoot, { recursive: true, force: true });
    }

    const actualLock = JSON.parse(fs.readFileSync(path.join(defaultRepoRoot, 'src', 'web', 'package-lock.json'), 'utf8'));
    const mismatchedLock = clone(actualLock);
    mismatchedLock.packages['node_modules/js-yaml'].version = '0.0.0-fixture';
    assert.throws(() => loadLockedYaml(defaultRepoRoot, { lockData: mismatchedLock }), /lock\/resolution mismatch/);
    const missingLock = clone(actualLock);
    delete missingLock.packages['node_modules/js-yaml'];
    assert.throws(() => loadLockedYaml(defaultRepoRoot, { lockData: missingLock }), /missing node_modules\/js-yaml/);
    assert.throws(() => loadLockedYaml(defaultRepoRoot, {
        createRequireFn: () => {
            const unavailable = () => { throw new Error('fixture unavailable'); };
            unavailable.resolve = () => { throw new Error('fixture unavailable'); };
            return unavailable;
        },
        lockData: actualLock,
    }), /Unable to resolve/);

    const localCiText = fs.readFileSync(path.join(defaultRepoRoot, 'scripts', 'run_ci_local.ps1'), 'utf8');
    validateLocalCiScript(localCiText);
    assert.throws(
        () => validateLocalCiScript(localCiText.replace(ROUTE_COMMAND, 'cargo test --workspace')),
        /runtime_route_ownership_contract/,
    );
    assert.throws(
        () => validateLocalCiScript(localCiText.replace(LOCAL_RUST_BUSINESS_DIFF_GUARD, LOCAL_RUST_BUSINESS_DIFF_GUARD.replace('src/backend', 'tests/backend'))),
        /src\/backend/,
    );
    assert.throws(
        () => validateLocalCiScript(localCiText.replace(LOCAL_RUST_BUSINESS_DIFF_GUARD, '')),
        /src\/backend/,
    );
    assert.throws(
        () => validateLocalCiScript(localCiText.replace(LOCAL_FRONTEND_BUSINESS_DIFF_GUARD, LOCAL_FRONTEND_BUSINESS_DIFF_GUARD.replace('src/web/src', 'src/web/e2e'))),
        /src\/web\/src/,
    );

    console.log('PASS Gitea workflow checker self-tests');
}

function parseWorkflow(parser, repoRoot = defaultRepoRoot) {
    const workflowPath = path.join(repoRoot, '.gitea', 'workflows', 'ci.yml');
    let workflow;
    try {
        workflow = parser.load(fs.readFileSync(workflowPath, 'utf8'));
    } catch (error) {
        throw new Error(`Unable to parse .gitea/workflows/ci.yml: ${error.message}`);
    }
    return workflow;
}

function main(argv = process.argv.slice(2)) {
    const lockedYaml = loadLockedYaml();
    if (argv.includes('--self-test')) {
        selfTest(lockedYaml.parser);
        return;
    }
    const activeFiles = validateActiveConfigs();
    const result = validateWorkflow(parseWorkflow(lockedYaml.parser));
    validateLocalCiScript(fs.readFileSync(path.join(defaultRepoRoot, 'scripts', 'run_ci_local.ps1'), 'utf8'));
    const frontendTestScripts = validateFrontendTestScripts(JSON.parse(
        fs.readFileSync(path.join(defaultRepoRoot, 'src', 'web', 'package.json'), 'utf8'),
    ));
    console.log(JSON.stringify({
        status: 'passed',
        yaml_parser: {
            version: lockedYaml.version,
            package_json_path: path.relative(defaultRepoRoot, lockedYaml.packageJsonPath).replace(/\\/g, '/'),
        },
        active_configs: activeFiles,
        frontend_test_scripts: frontendTestScripts,
        ...result,
    }, null, 2));
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
    activeConfigInventory,
    loadLockedYaml,
    validateActiveConfigs,
    validateFrontendTestScripts,
    validateLocalCiScript,
    validateRequiredE2e,
    validateWorkflow,
};
