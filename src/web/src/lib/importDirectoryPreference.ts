const IMPORT_DIRECTORY_DATABASE = 'bill-analyser-local-preferences';
const IMPORT_DIRECTORY_STORE = 'file-system-handles';
const IMPORT_DIRECTORY_KEY = 'bill-import-default-directory';
const IMPORT_FILE_PICKER_ID = 'bill-analyser-import-files';

export interface ImportDirectoryHandleStore {
    readonly clear: () => Promise<void>;
    readonly load: () => Promise<FileSystemDirectoryHandle | null>;
    readonly save: (handle: FileSystemDirectoryHandle) => Promise<void>;
}

export interface ImportDirectoryPickerOptions {
    readonly id: string;
    readonly mode: 'read';
}

export type ImportDirectoryPicker = (
    options: ImportDirectoryPickerOptions
) => Promise<FileSystemDirectoryHandle>;

interface ImportDirectoryWindow extends Window {
    readonly showDirectoryPicker?: ImportDirectoryPicker;
}

function openPreferenceDatabase(): Promise<IDBDatabase> {
    return new Promise((resolve, reject) => {
        if (typeof indexedDB === 'undefined') {
            reject(new Error('Browser directory preferences are not supported'));
            return;
        }
        const request = indexedDB.open(IMPORT_DIRECTORY_DATABASE, 1);
        request.onupgradeneeded = () => {
            if (!request.result.objectStoreNames.contains(IMPORT_DIRECTORY_STORE)) {
                request.result.createObjectStore(IMPORT_DIRECTORY_STORE);
            }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error ?? new Error('Unable to open directory preferences'));
    });
}

function completeRequest<T>(request: IDBRequest<T>): Promise<T> {
    return new Promise((resolve, reject) => {
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error ?? new Error('Unable to update directory preferences'));
    });
}

const browserHandleStore: ImportDirectoryHandleStore = {
    async clear(): Promise<void> {
        const database = await openPreferenceDatabase();
        try {
            const transaction = database.transaction(IMPORT_DIRECTORY_STORE, 'readwrite');
            await completeRequest(transaction.objectStore(IMPORT_DIRECTORY_STORE).delete(IMPORT_DIRECTORY_KEY));
        } finally {
            database.close();
        }
    },
    async load(): Promise<FileSystemDirectoryHandle | null> {
        if (typeof indexedDB === 'undefined') {
            return null;
        }
        const database = await openPreferenceDatabase();
        try {
            const transaction = database.transaction(IMPORT_DIRECTORY_STORE, 'readonly');
            const value = await completeRequest<unknown>(
                transaction.objectStore(IMPORT_DIRECTORY_STORE).get(IMPORT_DIRECTORY_KEY)
            );
            return value && typeof value === 'object' && (value as { kind?: unknown }).kind === 'directory'
                ? value as FileSystemDirectoryHandle
                : null;
        } finally {
            database.close();
        }
    },
    async save(handle: FileSystemDirectoryHandle): Promise<void> {
        const database = await openPreferenceDatabase();
        try {
            const transaction = database.transaction(IMPORT_DIRECTORY_STORE, 'readwrite');
            await completeRequest(transaction.objectStore(IMPORT_DIRECTORY_STORE).put(handle, IMPORT_DIRECTORY_KEY));
        } finally {
            database.close();
        }
    }
};

function resolveDirectoryPicker(): ImportDirectoryPicker | undefined {
    if (typeof window === 'undefined') {
        return undefined;
    }
    const picker = (window as ImportDirectoryWindow).showDirectoryPicker;
    return typeof picker === 'function' ? picker.bind(window) : undefined;
}

export async function chooseDefaultImportDirectory({
    picker = resolveDirectoryPicker(),
    store = browserHandleStore
}: {
    picker?: ImportDirectoryPicker;
    store?: ImportDirectoryHandleStore;
} = {}): Promise<{ name: string }> {
    if (!picker) {
        throw new Error('This browser does not support choosing a default import folder');
    }
    const handle = await picker({ id: IMPORT_FILE_PICKER_ID, mode: 'read' });
    await store.save(handle);
    return { name: handle.name };
}

export function getDefaultImportDirectoryHandle(
    store: ImportDirectoryHandleStore = browserHandleStore
): Promise<FileSystemDirectoryHandle | null> {
    return store.load();
}

export function clearDefaultImportDirectory(
    store: ImportDirectoryHandleStore = browserHandleStore
): Promise<void> {
    return store.clear();
}
