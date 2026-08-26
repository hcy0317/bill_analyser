import { describe, expect, jest, test } from '@jest/globals';

import {
    IMPORT_FILE_DIALOG_DEFAULT_DIRECTORY,
    openImportFileDialog,
    type BrowserImportFilePicker,
    type NativeImportFileDialog
} from '@/lib/importFileDialog.ts';

describe('import file dialog helper', () => {
    test('passes ./bills to a native-capable dialog and does not fake a browser path', async () => {
        const click = jest.fn();
        const nativeDialog = jest.fn<NativeImportFileDialog>(() => ({ handled: true }));

        const result = await openImportFileDialog({
            accept: '.csv,.xls,.xlsx,.txt',
            fileInput: { click },
            nativeDialog
        });

        expect(nativeDialog).toHaveBeenCalledWith({
            accept: '.csv,.xls,.xlsx,.txt',
            defaultDirectory: IMPORT_FILE_DIALOG_DEFAULT_DIRECTORY,
            multiple: true
        });
        expect(click).not.toHaveBeenCalled();
        expect(result).toEqual({
            defaultDirectory: IMPORT_FILE_DIALOG_DEFAULT_DIRECTORY,
            filesSelected: 0,
            method: 'native-default-directory'
        });
    });

    test('falls back to the hidden browser file input when no native dialog is available', async () => {
        const click = jest.fn();

        const result = await openImportFileDialog({
            accept: '.csv',
            fileInput: { click }
        });

        expect(click).toHaveBeenCalledTimes(1);
        expect(result).toEqual({
            defaultDirectory: IMPORT_FILE_DIALOG_DEFAULT_DIRECTORY,
            filesSelected: 0,
            method: 'browser-file-input-fallback'
        });
    });

    test('falls back honestly when the native dialog declines to handle selection', async () => {
        const click = jest.fn();
        const nativeDialog = jest.fn<NativeImportFileDialog>(() => false);

        const result = await openImportFileDialog({
            accept: '.csv',
            fileInput: { click },
            nativeDialog
        });

        expect(click).toHaveBeenCalledTimes(1);
        expect(result.method).toBe('browser-file-input-fallback');
    });

    test('opens the browser picker in the configured directory handle and returns selected files', async () => {
        const selected = new File(['bill'], 'wechat.csv');
        const directoryHandle = { kind: 'directory', name: '账单归档' } as FileSystemDirectoryHandle;
        const browserFilePicker = jest.fn<BrowserImportFilePicker>(async () => [{
            kind: 'file',
            name: selected.name,
            getFile: async () => selected
        }]);
        const onFilesSelected = jest.fn();
        const click = jest.fn();

        const result = await openImportFileDialog({
            accept: '.csv,.xlsx',
            browserFilePicker,
            defaultDirectoryHandle: directoryHandle,
            fileInput: { click },
            onFilesSelected
        });

        expect(browserFilePicker).toHaveBeenCalledWith(expect.objectContaining({
            id: 'bill-analyser-import-files',
            multiple: true,
            startIn: directoryHandle
        }));
        expect(onFilesSelected).toHaveBeenCalledWith([selected]);
        expect(click).not.toHaveBeenCalled();
        expect(result.method).toBe('browser-default-directory');
        expect(result.filesSelected).toBe(1);
    });

    test('treats browser picker cancellation as handled instead of opening a second dialog', async () => {
        const click = jest.fn();
        const browserFilePicker = jest.fn<BrowserImportFilePicker>(async () => {
            throw new DOMException('cancelled', 'AbortError');
        });

        const result = await openImportFileDialog({
            accept: '.csv',
            browserFilePicker,
            fileInput: { click }
        });

        expect(click).not.toHaveBeenCalled();
        expect(result.method).toBe('browser-default-directory');
        expect(result.filesSelected).toBe(0);
    });

    test('propagates browser picker errors instead of masking them with a second file input', async () => {
        const click = jest.fn();
        const pickerError = new DOMException('permission denied', 'SecurityError');
        const browserFilePicker = jest.fn<BrowserImportFilePicker>(async () => {
            throw pickerError;
        });

        await expect(openImportFileDialog({
            accept: '.csv',
            browserFilePicker,
            fileInput: { click }
        })).rejects.toBe(pickerError);
        expect(click).not.toHaveBeenCalled();
    });
});
