import type { Ref } from 'vue';

export interface ImportFlowProfilerOptions {
    importDialogOpenedAt: Ref<number | null>;
    importSubmitStartedAt: Ref<number | null>;
    logger: {
        info(message: string, context?: Record<string, unknown>): void;
    };
}

function importFlowElapsedMs(startedAt: number | null): number | null {
    return startedAt === null ? null : Math.max(Date.now() - startedAt, 0);
}

// 导入流程 profile 只记录阶段里程碑和耗时，不参与业务状态迁移，避免观测逻辑影响导入结果。
export function createImportFlowMilestoneLogger(options: ImportFlowProfilerOptions) {
    return (milestone: string, context: Record<string, unknown> = {}): void => {
        options.logger.info(`[三阶段导入-profile] ${milestone}`, {
            ...context,
            import_dialog_opened_elapsed_ms: importFlowElapsedMs(options.importDialogOpenedAt.value),
            elapsed_to_first_operable_preview_ms: importFlowElapsedMs(options.importSubmitStartedAt.value)
        });
    };
}
