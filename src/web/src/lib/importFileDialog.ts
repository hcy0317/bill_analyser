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
    readonly browserFilePicker?: BrowserImportFilePicker;
    readonly defaultDirectory?: string;
    readonly defaultDirectoryHandle?: FileSystemDirectoryHandle | null;
    readonly fileInput?: Pick<HTMLInputElement, 'click'> | null;
    readonly multiple?: boolean;
    readonly nativeDialog?: NativeImportFileDialog;
    readonly onFilesSelected?: (files: readonly File[]) => void;
}

export interface ImportFileDialogOpenResult {
    readonly defaultDirectory: string;
    readonly filesSelected: number;
    readonly method: 'native-default-directory' | 'browser-default-directory' | 'browser-file-input-fallback';
}

export interface BrowserImportFileHandle {
    readonly getFile: () => Promise<File>;
}

export interface BrowserImportFilePickerOptions {
    readonly excludeAcceptAllOption: boolean;
    readonly id: string;
    readonly multiple: boolean;
    readonly startIn?: FileSystemDirectoryHandle;
    readonly types: ReadonlyArray<{
        readonly description: string;
        readonly accept: Readonly<Record<string, readonly string[]>>;
    }>;
}

export type BrowserImportFilePicker = (
    options: BrowserImportFilePickerOptions
) => Promise<readonly BrowserImportFileHandle[]>;

interface WindowWithNativeImportDialog extends Window {
    readonly showOpenFilePicker?: BrowserImportFilePicker;
    readonly billAnalyserImportFileDialog?: {
        readonly openImportFiles?: NativeImportFileDialog;
    };
    readonly billAnalyserNative?: {
        readonly openImportFiles?: NativeImportFileDialog;
    };
}

const IMPORT_FILE_PICKER_ID = 'bill-analyser-import-files';

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

function resolveBrowserImportFilePicker(): BrowserImportFilePicker | undefined {
    if (typeof window === 'undefined') {
        return undefined;
    }
    const picker = (window as WindowWithNativeImportDialog).showOpenFilePicker;
    return typeof picker === 'function' ? picker.bind(window) : undefined;
}

function browserPickerTypes(accept: string): BrowserImportFilePickerOptions['types'] {
    const extensions = accept
        .split(',')
        .map(value => value.trim().toLowerCase())
        .filter(value => /^\.[a-z0-9]+$/.test(value));
    return extensions.length > 0 ? [{
        description: 'Bill files',
        accept: { 'application/octet-stream': extensions }
    }] : [];
}

function isAbortError(error: unknown): boolean {
    return error instanceof DOMException && error.name === 'AbortError';
}

export async function openImportFileDialog(options: ImportFileDialogOpenOptions): Promise<ImportFileDialogOpenResult> {
    const defaultDirectory = options.defaultDirectory ?? IMPORT_FILE_DIALOG_DEFAULT_DIRECTORY;
    const nativeDialog = options.nativeDialog ?? resolveNativeImportFileDialog();
    const browserFilePicker = options.browserFilePicker ?? resolveBrowserImportFilePicker();
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


    if (browserFilePicker) {
        try {
            const handles = await browserFilePicker({
                excludeAcceptAllOption: false,
                id: IMPORT_FILE_PICKER_ID,
                multiple,
                ...(options.defaultDirectoryHandle ? { startIn: options.defaultDirectoryHandle } : {}),
                types: browserPickerTypes(options.accept)
            });
            const files = await Promise.all(handles.map(handle => handle.getFile()));
            if (files.length > 0) {
                options.onFilesSelected?.(files);
            }
            return {
                defaultDirectory,
                filesSelected: files.length,
                method: 'browser-default-directory'
            };
        } catch (error) {
            if (isAbortError(error)) {
                return {
                    defaultDirectory,
                    filesSelected: 0,
                    method: 'browser-default-directory'
                };
            }
            throw error;
        }
    }

    options.fileInput?.click();

    return {
        defaultDirectory,
        filesSelected: 0,
        method: 'browser-file-input-fallback'
    };
}
