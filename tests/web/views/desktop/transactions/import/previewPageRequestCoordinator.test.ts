import { PreviewPageRequestCoordinator } from '../../../../../../src/web/src/views/desktop/transactions/import/import-dialog/previewPageRequestCoordinator';

function deferred<T>() {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>(done => { resolve = done; });
    return { promise, resolve };
}

describe('PreviewPageRequestCoordinator', () => {
    test('coalesces equivalent header and menu triggers into one service call', async () => {
        const coordinator = new PreviewPageRequestCoordinator();
        const response = deferred<number>();
        const service = jest.fn((_signal: AbortSignal) => response.promise);
        const run = (key: string) => {
            const handle = coordinator.begin(key);
            if (!handle) return Promise.resolve();
            return service(handle.controller.signal).finally(() => coordinator.finish(handle));
        };
        const header = run('session-a?page=1&signal=learning');
        const menu = run('session-a?page=1&signal=learning');
        expect(service).toHaveBeenCalledTimes(1);
        response.resolve(1);
        await Promise.all([header, menu]);
        expect(coordinator.loading).toBe(false);
    });

    test('does not merge different session, page, or filter keys', () => {
        const coordinator = new PreviewPageRequestCoordinator();
        const first = coordinator.begin('session-a?page=1&signal=learning')!;
        expect(coordinator.begin('session-b?page=1&signal=learning')).not.toBeNull();
        expect(first.controller.signal.aborted).toBe(true);
        expect(coordinator.begin('session-b?page=2&signal=learning')).not.toBeNull();
        expect(coordinator.begin('session-b?page=2&signal=transfer')).not.toBeNull();
    });

    test('aborts the old request and ignores its late response without clearing the current request', async () => {
        const coordinator = new PreviewPageRequestCoordinator();
        const oldResponse = deferred<string>();
        const newResponse = deferred<string>();
        let state = '';
        const run = async (key: string, response: Promise<string>) => {
            const handle = coordinator.begin(key)!;
            try {
                const value = await response;
                if (coordinator.isCurrent(handle)) state = value;
            } finally {
                coordinator.finish(handle);
            }
            return handle;
        };
        const oldRun = run('session-a?page=1', oldResponse.promise);
        const newRun = run('session-a?page=2', newResponse.promise);
        expect(coordinator.loading).toBe(true);
        oldResponse.resolve('old');
        const oldHandle = await oldRun;
        expect(oldHandle.controller.signal.aborted).toBe(true);
        expect(state).toBe('');
        expect(coordinator.loading).toBe(true);
        newResponse.resolve('new');
        await newRun;
        expect(state).toBe('new');
        expect(coordinator.loading).toBe(false);
    });

    test('explicit abort clears loading and invalidates the active handle', () => {
        const coordinator = new PreviewPageRequestCoordinator();
        const handle = coordinator.begin('session-a?page=1')!;
        coordinator.abort();
        expect(handle.controller.signal.aborted).toBe(true);
        expect(coordinator.isCurrent(handle)).toBe(false);
        expect(coordinator.loading).toBe(false);
    });
});
