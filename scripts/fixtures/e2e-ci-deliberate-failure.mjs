#!/usr/bin/env node

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const [mode = 'success', label = 'fixture', markerPath = ''] = process.argv.slice(2);
const scriptPath = fileURLToPath(import.meta.url);

if (mode === 'serve') {
    const timer = setInterval(() => undefined, 1_000);
    const stop = () => {
        clearInterval(timer);
        process.exit(0);
    };
    process.once('SIGINT', stop);
    process.once('SIGTERM', stop);
    console.log(`READY ${label}`);
} else if (mode === 'failure') {
    console.error(`DELIBERATE_FAILURE ${label}`);
    process.exitCode = 86;
} else if (mode === 'success') {
    console.log(`PASS ${label}`);
} else if (mode === 'spawn-tree') {
    const grandchild = spawn(process.execPath, [scriptPath, 'serve-ignore-term', `${label} grandchild`], {
        detached: false,
        stdio: 'ignore',
        windowsHide: true,
    });
    fs.writeFileSync(markerPath, `${JSON.stringify({ parentPid: process.pid, grandchildPid: grandchild.pid })}\n`, 'utf8');
    process.once('SIGTERM', () => undefined);
    process.once('SIGINT', () => undefined);
    setTimeout(() => process.exit(0), 5_000).unref();
    setInterval(() => undefined, 1_000);
} else if (mode === 'serve-ignore-term') {
    process.once('SIGTERM', () => undefined);
    process.once('SIGINT', () => undefined);
    setTimeout(() => process.exit(0), 5_000).unref();
    setInterval(() => undefined, 1_000);
} else {
    console.error(`Unknown fixture mode ${mode}`);
    process.exitCode = 2;
}
