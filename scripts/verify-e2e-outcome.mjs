#!/usr/bin/env node
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);

function verifyE2eOutcome({
    scopeResult,
    scopeRequired,
    scopeReason,
    heavyResult,
}) {
    const normalizedScopeResult = String(scopeResult ?? '').trim();
    const normalizedRequired = String(scopeRequired ?? '').trim();
    const normalizedReason = String(scopeReason ?? '').trim();
    const normalizedHeavyResult = String(heavyResult ?? '').trim();

    if (normalizedScopeResult !== 'success') {
        throw new Error(`E2E scope job did not pass: ${normalizedScopeResult || 'missing'}`);
    }
    if (normalizedRequired !== 'true' && normalizedRequired !== 'false') {
        throw new Error(`E2E scope required output is invalid: ${normalizedRequired || 'missing'}`);
    }
    if (!normalizedReason) {
        throw new Error('E2E scope reason output is missing');
    }

    if (normalizedRequired === 'true') {
        if (normalizedHeavyResult !== 'success') {
            throw new Error(`Required E2E heavy job did not pass: ${normalizedHeavyResult || 'missing'}`);
        }
        return { status: 'full_e2e_passed', reason: normalizedReason };
    }

    if (normalizedHeavyResult !== 'skipped') {
        throw new Error(`Irrelevant E2E heavy job was not skipped: ${normalizedHeavyResult || 'missing'}`);
    }
    return { status: 'heavy_e2e_skipped', reason: normalizedReason };
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

function main(argv = process.argv.slice(2)) {
    const args = parseArgs(argv);
    const result = verifyE2eOutcome({
        scopeResult: args.get('--scope-result'),
        scopeRequired: args.get('--scope-required'),
        scopeReason: args.get('--scope-reason'),
        heavyResult: args.get('--heavy-result'),
    });
    console.log(JSON.stringify(result));
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
    try {
        main();
    } catch (error) {
        console.error(`FAIL ${error.message}`);
        process.exitCode = 1;
    }
}

export { parseArgs, verifyE2eOutcome };
