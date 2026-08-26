import { nextTick, ref, type Ref } from 'vue';

import type { ImportSessionSummary } from '@/models/import_preview.ts';
import type { ImportTransaction } from '@/models/imported_transaction.ts';
import { getCurrentToken } from '@/lib/userstate.ts';
import { getDefaultImportDirectoryHandle } from '@/lib/importDirectoryPreference.ts';
import { openImportFileDialog } from '@/lib/importFileDialog.ts';
import services from '@/lib/services.ts';
import logger from '@/lib/logger.ts';

import type { ImportPreviewMetadata } from '../importPreviewIndex.ts';
import type { ImportTransactionDialogStep } from './types.ts';

interface PendingPreviewPageRequest {
    page: number;
    pageSize: number;
    sortBy: string;
    sortDirection: 'asc' | 'desc';
}

interface ConfirmDialogBridge {
    open(
        title: string,
        text: string,
        options: Record<string, unknown>
    ): Promise<unknown>;
}

interface ImportEntryContinuityOptions {
    readonly currentStep: Ref<ImportTransactionDialogStep>;
    readonly formatCount: (count: number) => string;
    readonly getConfiguredDirectoryName: () => string;
    readonly getConfirmDialog: () => ConfirmDialogBridge | null;
    readonly importProcess: Ref<number>;
    readonly importTransactions: Ref<ImportTransaction[] | undefined>;
    readonly pendingInitialCheckDataPageRequest: Ref<PendingPreviewPageRequest | null>;
    readonly previewMetadata: Ref<ImportPreviewMetadata | null>;
    readonly previewPageSortBy: Ref<string>;
    readonly previewPageSortDirection: Ref<'asc' | 'desc'>;
    readonly previewTotalCount: Ref<number>;
    readonly serverPagedPreviewMode: Ref<boolean>;
    readonly serverSessionId: Ref<string>;
    readonly showError: (error: unknown) => void;
    readonly showState: Ref<boolean>;
}

export function useImportEntryContinuity(options: ImportEntryContinuityOptions) {
    const defaultDirectoryHandle = ref<FileSystemDirectoryHandle | null>(null);

    function reportImportContinuityError(message: string, error: unknown): void {
        logger.warn(message, error);
        options.showError(error);
    }

    async function loadDefaultImportDirectory(): Promise<void> {
        try {
            defaultDirectoryHandle.value = await getDefaultImportDirectoryHandle();
        } catch (error) {
            defaultDirectoryHandle.value = null;
            logger.warn('[三阶段导入] 读取默认导入位置失败:', error);
        }
    }

    async function openConfiguredImportFileDialog({
        accept,
        fileInput,
        onFilesSelected
    }: {
        accept: string;
        fileInput: HTMLInputElement | null;
        onFilesSelected: (files: readonly File[]) => void;
    }): Promise<void> {
        try {
            await openImportFileDialog({
                accept,
                defaultDirectoryHandle: options.getConfiguredDirectoryName()
                    ? defaultDirectoryHandle.value
                    : null,
                fileInput,
                onFilesSelected
            });
        } catch (error) {
            reportImportContinuityError('[三阶段导入] 打开文件选择器失败:', error);
        }
    }

    async function deleteServerSession(sessionId: string): Promise<void> {
        if (!sessionId) return;
        const token = getCurrentToken();
        const headers: Record<string, string> = {};
        if (token) headers['Authorization'] = `Bearer ${token}`;
        const response = await fetch(`/api/bills/import/v2/session/${encodeURIComponent(sessionId)}`, {
            method: 'DELETE',
            headers
        });
        if (!response.ok) {
            throw new Error(`清理导入会话失败（HTTP ${response.status}）`);
        }
        logger.info(`[三阶段导入] 已清理会话: ${sessionId}`);
    }

    async function cleanupServerSession(): Promise<void> {
        if (!options.serverSessionId.value) return;
        const sessionId = options.serverSessionId.value;
        await deleteServerSession(sessionId);
        if (options.serverSessionId.value === sessionId) {
            options.serverSessionId.value = '';
            options.previewMetadata.value = null;
        }
    }

    function resumeImportSession(session: ImportSessionSummary): void {
        options.serverSessionId.value = session.session_id;
        options.previewTotalCount.value = Math.max(Number(session.preview_count) || 0, 0);
        options.previewMetadata.value = null;
        options.serverPagedPreviewMode.value = true;
        options.previewPageSortBy.value = '';
        options.previewPageSortDirection.value = 'asc';
        options.importTransactions.value = [];
        options.pendingInitialCheckDataPageRequest.value = {
            page: 1,
            pageSize: 10,
            sortBy: '',
            sortDirection: 'asc'
        };
        options.currentStep.value = 'checkData';
        options.importProcess.value = 100;
    }

    async function offerRecoverableImportSession(): Promise<void> {
        try {
            const response = await services.getRecoverableImportSessions();
            const sessions = Array.isArray(response.data?.result) ? response.data.result : [];
            const latest = sessions[0];
            if (!options.showState.value || !latest?.session_id) return;
            await nextTick();
            if (!options.showState.value) return;
            const shouldResume = await options.getConfirmDialog()?.open(
                'Unfinished import session found',
                'Resume the previous import preview?',
                {
                    count: options.formatCount(latest.preview_count || 0),
                    details: [`${latest.created_at} · ${options.formatCount(latest.preview_count || 0)}`]
                }
            );
            if (shouldResume) {
                resumeImportSession(latest);
                return;
            }
            await Promise.all(sessions.map(session => deleteServerSession(session.session_id)));
        } catch (error) {
            logger.warn('[三阶段导入] 检查残留会话失败:', error);
            throw error;
        }
    }

    return {
        cleanupServerSession,
        loadDefaultImportDirectory,
        offerRecoverableImportSession,
        openConfiguredImportFileDialog,
        reportImportContinuityError
    };
}
