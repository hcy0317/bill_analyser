import { describe, expect, jest, test } from '@jest/globals';

import {
    IMPORT_FILE_DIALOG_DEFAULT_DIRECTORY,
    openImportFileDialog,
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
});
