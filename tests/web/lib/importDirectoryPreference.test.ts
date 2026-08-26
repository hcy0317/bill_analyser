import { describe, expect, jest, test } from '@jest/globals';

import {
    chooseDefaultImportDirectory,
    clearDefaultImportDirectory,
    getDefaultImportDirectoryHandle,
    type ImportDirectoryPicker
} from '@/lib/importDirectoryPreference.ts';

function memoryHandleStore() {
    let saved: FileSystemDirectoryHandle | null = null;
    return {
        clear: jest.fn(async () => { saved = null; }),
        load: jest.fn(async () => saved),
        save: jest.fn(async (handle: FileSystemDirectoryHandle) => { saved = handle; })
    };
}

describe('default bill import directory preference', () => {
    test('persists a user-selected directory handle and exposes only its safe display name', async () => {
        const store = memoryHandleStore();
        const handle = { kind: 'directory', name: '我的账单' } as FileSystemDirectoryHandle;
        const picker = jest.fn<ImportDirectoryPicker>(async () => handle);

        const selection = await chooseDefaultImportDirectory({ picker, store });

        expect(picker).toHaveBeenCalledWith(expect.objectContaining({
            id: 'bill-analyser-import-files',
            mode: 'read'
        }));
        expect(store.save).toHaveBeenCalledWith(handle);
        expect(selection).toEqual({ name: '我的账单' });
        await expect(getDefaultImportDirectoryHandle(store)).resolves.toBe(handle);
    });

    test('clears the persisted handle without inventing a filesystem path', async () => {
        const store = memoryHandleStore();
        const handle = { kind: 'directory', name: '账单' } as FileSystemDirectoryHandle;
        await store.save(handle);

        await clearDefaultImportDirectory(store);

        await expect(getDefaultImportDirectoryHandle(store)).resolves.toBeNull();
        expect(store.clear).toHaveBeenCalledTimes(1);
    });

    test('uses IndexedDB for the production browser store across choose, load, and clear', async () => {
        const values = new Map<string, unknown>();
        const request = <T>(result: T): IDBRequest<T> => {
            const state: Record<string, unknown> = { result, error: null };
            queueMicrotask(() => (state['onsuccess'] as (() => void) | undefined)?.());
            return state as unknown as IDBRequest<T>;
        };
        const objectStore = {
            delete: (key: string) => {
                values.delete(key);
                return request(undefined);
            },
            get: (key: string) => request(values.get(key)),
            put: (value: unknown, key: string) => {
                values.set(key, value);
                return request(key);
            }
        };
        const database = {
            close: jest.fn(),
            createObjectStore: jest.fn(() => objectStore),
            objectStoreNames: { contains: () => false },
            transaction: jest.fn(() => ({ objectStore: () => objectStore }))
        };
        const open = jest.fn(() => {
            const state: Record<string, unknown> = { result: database, error: null };
            queueMicrotask(() => {
                (state['onupgradeneeded'] as (() => void) | undefined)?.();
                (state['onsuccess'] as (() => void) | undefined)?.();
            });
            return state as unknown as IDBOpenDBRequest;
        });
        const originalIndexedDb = Object.getOwnPropertyDescriptor(globalThis, 'indexedDB');
        Object.defineProperty(globalThis, 'indexedDB', {
            configurable: true,
            value: { open }
        });
        const handle = { kind: 'directory', name: '长期账单目录' } as FileSystemDirectoryHandle;
        const picker = jest.fn<ImportDirectoryPicker>(async () => handle);

        try {
            await expect(chooseDefaultImportDirectory({ picker })).resolves.toEqual({ name: '长期账单目录' });
            await expect(getDefaultImportDirectoryHandle()).resolves.toBe(handle);
            await clearDefaultImportDirectory();
            await expect(getDefaultImportDirectoryHandle()).resolves.toBeNull();
            expect(open).toHaveBeenCalledTimes(4);
            expect(database.close).toHaveBeenCalledTimes(4);
        } finally {
            if (originalIndexedDb) {
                Object.defineProperty(globalThis, 'indexedDB', originalIndexedDb);
            } else {
                delete (globalThis as { indexedDB?: IDBFactory }).indexedDB;
            }
        }
    });

    test('reports unsupported folder selection and treats missing IndexedDB as no preference', async () => {
        const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
        const originalIndexedDb = Object.getOwnPropertyDescriptor(globalThis, 'indexedDB');
        Object.defineProperty(globalThis, 'window', { configurable: true, value: {} });
        Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: undefined });
        try {
            await expect(chooseDefaultImportDirectory()).rejects.toThrow('does not support');
            await expect(getDefaultImportDirectoryHandle()).resolves.toBeNull();
        } finally {
            if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow);
            if (originalIndexedDb) Object.defineProperty(globalThis, 'indexedDB', originalIndexedDb);
        }
    });
});
