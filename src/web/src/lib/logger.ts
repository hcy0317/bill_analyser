import { isEnableDebug } from './settings.ts';

function logDebug(msg: string, obj?: unknown): void {
    if (isEnableDebug()) {
        if (obj) {
            console.debug('[bill analyser Debug] ' + msg, obj);
        } else {
            console.debug('[bill analyser Debug] ' + msg);
        }
    }
}

function logInfo(msg: string, obj?: unknown): void {
    if (obj) {
        console.info('[bill analyser Info] ' + msg, obj);
    } else {
        console.info('[bill analyser Info] ' + msg);
    }
}

function logWarn(msg: string, obj?: unknown): void {
    if (obj) {
        console.warn('[bill analyser Warn] ' + msg, obj);
    } else {
        console.warn('[bill analyser Warn] ' + msg);
    }
}

function logError(msg: string, obj?: unknown): void {
    if (obj) {
        console.error('[bill analyser Error] ' + msg, obj);
    } else {
        console.error('[bill analyser Error] ' + msg);
    }
}

export default {
    debug: logDebug,
    info: logInfo,
    warn: logWarn,
    error: logError
};
