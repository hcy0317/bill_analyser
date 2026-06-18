import { isEnableDebug } from './settings.ts';

type ConsoleWriter = (message?: unknown, ...optionalParams: unknown[]) => void;

function timestamp(): string {
    return new Date().toISOString();
}

function formatPrefix(level: 'Debug' | 'Info' | 'Warn' | 'Error'): string {
    return `[bill analyser ${level}] ${timestamp()}`;
}

function writeLog(writer: ConsoleWriter, level: 'Debug' | 'Info' | 'Warn' | 'Error', msg: string, obj?: unknown): void {
    const message = `${formatPrefix(level)} ${msg}`;
    if (obj !== undefined) {
        writer(message, obj);
    } else {
        writer(message);
    }
}

function logDebug(msg: string, obj?: unknown): void {
    if (isEnableDebug()) {
        writeLog(console.debug, 'Debug', msg, obj);
    }
}

function logInfo(msg: string, obj?: unknown): void {
    writeLog(console.info, 'Info', msg, obj);
}

function logWarn(msg: string, obj?: unknown): void {
    writeLog(console.warn, 'Warn', msg, obj);
}

function logError(msg: string, obj?: unknown): void {
    writeLog(console.error, 'Error', msg, obj);
}

export default {
    debug: logDebug,
    info: logInfo,
    warn: logWarn,
    error: logError
};
