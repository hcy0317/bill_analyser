export interface PreviewPageRequestHandle {
    readonly key: string;
    readonly generation: number;
    readonly controller: AbortController;
}

export interface PreviewPageRequestStartOptions {
    replaceActive?: boolean;
}

export class PreviewPageRequestCoordinator {
    private generation = 0;
    private active: PreviewPageRequestHandle | null = null;

    get loading(): boolean {
        return this.active !== null;
    }

    begin(key: string, options: PreviewPageRequestStartOptions = {}): PreviewPageRequestHandle | null {
        if (this.active?.key === key && !options.replaceActive) {
            return null;
        }
        this.active?.controller.abort();
        const handle: PreviewPageRequestHandle = {
            key,
            generation: ++this.generation,
            controller: new AbortController()
        };
        this.active = handle;
        return handle;
    }

    isCurrent(handle: PreviewPageRequestHandle): boolean {
        return this.active === handle && !handle.controller.signal.aborted;
    }

    finish(handle: PreviewPageRequestHandle): void {
        if (this.active === handle) {
            this.active = null;
        }
    }

    abort(): void {
        this.generation += 1;
        this.active?.controller.abort();
        this.active = null;
    }
}
