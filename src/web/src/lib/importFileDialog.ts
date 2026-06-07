export const IMPORT_FILE_DIALOG_DEFAULT_DIRECTORY = './bills';

export interface NativeImportFileDialogOptions {
    readonly accept: string;
    readonly defaultDirectory: string;
    readonly multiple: boolean;
}

export interface NativeImportFileDialogHandledResult {
    readonly cancelled?: boolean;
    readonly files?: FileList | readonly File[];
    readonly handled: true;
}

export type NativeImportFileDialogResult =
    | NativeImportFileDialogHandledResult
    | FileList
    | readonly File[]
    | boolean
    | null
    | undefined;

export type NativeImportFileDialog = (
    options: NativeImportFileDialogOptions
) => NativeImportFileDialogResult | Promise<NativeImportFileDialogResult>;

export interface ImportFileDialogOpenOptions {
    readonly accept: string;
    readonly defaultDirectory?: string;
    readonly fileInput?: Pick<HTMLInputElement, 'click'> | null;
    readonly multiple?: boolean;
    readonly nativeDialog?: NativeImportFileDialog;
    readonly onFilesSelected?: (files: readonly File[]) => void;
}

export interface ImportFileDialogOpenResult {
    readonly defaultDirectory: string;
    readonly filesSelected: number;
    readonly method: 'native-default-directory' | 'browser-file-input-fallback';
}

interface WindowWithNativeImportDialog extends Window {
    readonly billAnalyserImportFileDialog?: {
        readonly openImportFiles?: NativeImportFileDialog;
    };
    readonly billAnalyserNative?: {
        readonly openImportFiles?: NativeImportFileDialog;
    };
}

function isObjectLike(value: unknown): value is Record<string, unknown> {
    return !!value && typeof value === 'object';
}

function isFileListLike(value: unknown): value is FileList {
    return isObjectLike(value)
        && typeof value['length'] === 'number'
        && typeof value['item'] === 'function';
}

function extractNativeFiles(result: NativeImportFileDialogResult): readonly File[] | null {
    if (Array.isArray(result)) {
        return result;
    }

    if (isFileListLike(result)) {
        return Array.from(result);
    }

    if (isObjectLike(result) && result['handled'] === true) {
        const files = result['files'];
        if (Array.isArray(files)) {
            return files;
        }
        if (isFileListLike(files)) {
            return Array.from(files);
        }
    }

    return null;
}

function isNativeDialogHandled(result: NativeImportFileDialogResult): boolean {
    if (result === true || Array.isArray(result) || isFileListLike(result)) {
        return true;
    }

    return isObjectLike(result) && result['handled'] === true;
}

function resolveNativeImportFileDialog(): NativeImportFileDialog | undefined {
    if (typeof window === 'undefined') {
        return undefined;
    }

    const nativeWindow = window as WindowWithNativeImportDialog;
    const bridge = nativeWindow.billAnalyserImportFileDialog ?? nativeWindow.billAnalyserNative;
    const openImportFiles = bridge?.openImportFiles;

    return typeof openImportFiles === 'function'
        ? openImportFiles.bind(bridge)
        : undefined;
}

export async function openImportFileDialog(options: ImportFileDialogOpenOptions): Promise<ImportFileDialogOpenResult> {
    const defaultDirectory = options.defaultDirectory ?? IMPORT_FILE_DIALOG_DEFAULT_DIRECTORY;
    const nativeDialog = options.nativeDialog ?? resolveNativeImportFileDialog();
    const multiple = options.multiple ?? true;

    if (nativeDialog) {
        const nativeResult = await nativeDialog({
            accept: options.accept,
            defaultDirectory,
            multiple
        });
        const files = extractNativeFiles(nativeResult);

        if (files?.length) {
            options.onFilesSelected?.(files);
        }

        if (isNativeDialogHandled(nativeResult)) {
            return {
                defaultDirectory,
                filesSelected: files?.length ?? 0,
                method: 'native-default-directory'
            };
        }
    }

    options.fileInput?.click();

    return {
        defaultDirectory,
        filesSelected: 0,
        method: 'browser-file-input-fallback'
    };
}
