#!/usr/bin/env node
import assert from 'node:assert/strict';

import { verifyE2eOutcome } from './verify-e2e-outcome.mjs';

assert.deepEqual(
    verifyE2eOutcome({
        scopeResult: 'success',
        scopeRequired: 'true',
        scopeReason: 'matched_e2e_paths:3',
        heavyResult: 'success',
    }),
    { status: 'full_e2e_passed', reason: 'matched_e2e_paths:3' },
);

assert.deepEqual(
    verifyE2eOutcome({
        scopeResult: 'success',
        scopeRequired: 'false',
        scopeReason: 'no_e2e_relevant_paths',
        heavyResult: 'skipped',
    }),
    { status: 'heavy_e2e_skipped', reason: 'no_e2e_relevant_paths' },
);

for (const fixture of [
    { scopeResult: 'failure', scopeRequired: 'false', scopeReason: 'no_e2e_relevant_paths', heavyResult: 'skipped' },
    { scopeResult: 'success', scopeRequired: '', scopeReason: 'no_e2e_relevant_paths', heavyResult: 'skipped' },
    { scopeResult: 'success', scopeRequired: 'unknown', scopeReason: 'no_e2e_relevant_paths', heavyResult: 'skipped' },
    { scopeResult: 'success', scopeRequired: 'false', scopeReason: '', heavyResult: 'skipped' },
    { scopeResult: 'success', scopeRequired: 'false', scopeReason: '   ', heavyResult: 'skipped' },
    { scopeResult: 'success', scopeRequired: 'false', scopeReason: 'no_e2e_relevant_paths', heavyResult: 'success' },
    { scopeResult: 'success', scopeRequired: 'true', scopeReason: 'matched_e2e_paths:1', heavyResult: 'skipped' },
]) {
    assert.throws(() => verifyE2eOutcome(fixture));
}

console.log('PASS E2E outcome verifier tests');
