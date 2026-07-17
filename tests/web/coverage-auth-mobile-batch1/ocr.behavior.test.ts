import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from './hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const mockThemeName = actualVue.ref('light');
const mockShowMessage = jest.fn<(...args: any[]) => void>();
const mockShowError = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockShowCancelableLoading = jest.fn<(...args: any[]) => void>();
const mockCloseAllDialog = jest.fn<(...args: any[]) => void>();
const mockCompressJpgImage = jest.fn<(...args: any[]) => Promise<Blob>>();
const mockCreateFileFromBlob = jest.fn<(blob: Blob, name: string) => File>();
const mockGenerateRandomUUID = jest.fn(() => 'ocr-cancel-id');
const mockLoggerError = jest.fn<(...args: any[]) => void>();

const mockTransactionsStore = {
    recognizeReceiptImage: jest.fn<(...args: any[]) => Promise<any>>(),
    cancelRecognizeReceiptImage: jest.fn<(...args: any[]) => void>()
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});
jest.mock('@/components/desktop/SnackBar.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return {
        __esModule: true,
        default: defineComponent({
            name: 'SnackBarStub',
            setup: (_props: unknown, { expose }: any) => {
                expose({ showMessage: mockShowMessage, showError: mockShowError });
                return () => h('snack-bar-stub');
            }
        })
    };
});
jest.mock('vuetify', () => ({ useTheme: () => ({ global: { name: mockThemeName } }) }));
jest.mock('@/locales/helpers.ts', () => ({ useI18n: () => ({ tt: (key: string) => `tt:${key}` }) }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/core/file.ts', () => ({
    KnownFileType: { JPG: { createFileFromBlob: (blob: Blob, name: string) => mockCreateFileFromBlob(blob, name) } }
}));
jest.mock('@/core/theme.ts', () => ({ isDarkApplicationTheme: (name: string) => name === 'dark' }));
jest.mock('@/consts/file.ts', () => ({ SUPPORTED_IMAGE_EXTENSIONS: '.jpg,.jpeg,.png' }));
jest.mock('@/models/large_language_model.ts', () => ({ RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD: 0.7 }));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => mockGenerateRandomUUID() }));
jest.mock('@/lib/ui/common.ts', () => ({
    compressJpgImage: (...args: any[]) => mockCompressJpgImage(...args)
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showCancelableLoading: mockShowCancelableLoading,
        showToast: mockShowToast
    }),
    closeAllDialog: (...args: any[]) => mockCloseAllDialog(...args)
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { error: (...args: any[]) => mockLoggerError(...args) }
}));

const DesktopDialog = require('@/views/desktop/transactions/list/dialogs/AIImageRecognitionDialog.vue').default as any;
const MobileSheet = require('@/components/mobile/AIImageRecognitionSheet.vue').default as any;

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void; reject: (reason: unknown) => void } {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

function createImageFile(name = 'receipt.png'): File {
    return new File(['synthetic-image'], name, { type: 'image/png' });
}

function setupDesktop(): { bindings: any; exposed: Record<string, any> } {
    const exposed: Record<string, any> = {};
    const bindings = DesktopDialog.setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: (value: Record<string, any>) => Object.assign(exposed, value)
    });
    bindings.snackbar.value = { showMessage: mockShowMessage, showError: mockShowError };
    bindings.imageInput.value = { click: jest.fn() };
    return { bindings, exposed };
}

function setupMobile(show = true): { bindings: any; emit: jest.Mock } {
    const emit = jest.fn();
    const bindings = MobileSheet.setup({ show }, {
        attrs: {}, slots: {}, emit, expose: jest.fn()
    });
    bindings.imageInput.value = { click: jest.fn() };
    return { bindings, emit };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockThemeName.value = 'light';
    mockCreateFileFromBlob.mockImplementation((blob, name) => new File([blob], `${name}.jpg`, { type: 'image/jpeg' }));
    mockCompressJpgImage.mockResolvedValue(new Blob(['compressed'], { type: 'image/jpeg' }));
    mockTransactionsStore.recognizeReceiptImage.mockResolvedValue({ confidence: 0.9, merchant: 'Synthetic Store' });
    Object.defineProperty(URL, 'createObjectURL', {
        configurable: true,
        value: jest.fn(() => 'blob:synthetic-image')
    });
});

describe('desktop AI image recognition dialog', () => {
    test('opens, loads selected and dropped images, and resolves low-confidence recognition', async () => {
        const { bindings, exposed } = setupDesktop();
        const resultPromise = exposed['open']();
        expect(bindings.showState.value).toBe(true);
        expect(bindings.isDarkMode.value).toBe(false);
        mockThemeName.value = 'dark';
        expect(bindings.isDarkMode.value).toBe(true);

        const selected = createImageFile();
        const input = { files: [selected], value: 'selected' };
        bindings.openImage({ target: input });
        await flush();
        expect(input.value).toBe('');
        expect(mockCompressJpgImage).toHaveBeenCalledWith(selected, 1280, 1280, 0.8);
        expect(bindings.imageFile.value.name).toBe('image.jpg');
        expect(bindings.imageSrc.value).toBe('blob:synthetic-image');

        bindings.onDragEnter();
        expect(bindings.isDragOver.value).toBe(true);
        bindings.onDragLeave();
        expect(bindings.isDragOver.value).toBe(false);
        const dropped = createImageFile('dropped.png');
        bindings.onDrop({ dataTransfer: { files: [dropped] } });
        await flush();
        expect(mockCompressJpgImage).toHaveBeenCalledWith(dropped, 1280, 1280, 0.8);

        const response = { confidence: 0.2, merchant: 'Low Confidence Store' };
        mockTransactionsStore.recognizeReceiptImage.mockResolvedValueOnce(response);
        bindings.recognize();
        expect(mockTransactionsStore.recognizeReceiptImage).toHaveBeenCalledWith({
            imageFile: bindings.imageFile.value,
            cancelableUuid: 'ocr-cancel-id'
        });
        await expect(resultPromise).resolves.toEqual(response);
        expect(mockShowMessage).toHaveBeenCalledWith('Low confidence recognition, please verify');
        expect(bindings.showState.value).toBe(false);
    });

    test('guards disabled interactions and accepts pasted image clipboard items', async () => {
        const { bindings } = setupDesktop();
        bindings.showOpenImageDialog();
        expect(bindings.imageInput.value.click).toHaveBeenCalledTimes(1);

        bindings.loading.value = true;
        bindings.showOpenImageDialog();
        bindings.onDragEnter();
        bindings.onDrop({ dataTransfer: { files: [createImageFile()] } });
        bindings.recognize();
        expect(bindings.imageInput.value.click).toHaveBeenCalledTimes(1);
        expect(bindings.isDragOver.value).toBe(false);
        expect(mockTransactionsStore.recognizeReceiptImage).not.toHaveBeenCalled();

        bindings.loading.value = false;
        const preventDefault = jest.fn();
        bindings.onPaste({ clipboardData: null, preventDefault });
        expect(preventDefault).toHaveBeenCalled();
        bindings.onPaste({
            clipboardData: {
                items: [
                    { type: 'text/plain', getAsFile: () => null },
                    { type: 'image/png', getAsFile: () => createImageFile('pasted.png') }
                ]
            },
            preventDefault
        });
        await flush();
        expect(preventDefault).toHaveBeenCalledTimes(2);
        expect(bindings.imageFile.value.name).toBe('image.jpg');

        bindings.openImage({ target: null });
        bindings.openImage({ target: { files: [] } });
        bindings.onDrop({ dataTransfer: null });
    });

    test('reports compression failures and every typed recognition failure', async () => {
        const { bindings } = setupDesktop();
        mockCompressJpgImage.mockRejectedValueOnce(new Error('synthetic compression failure'));
        bindings.openImage({ target: { files: [createImageFile()], value: 'selected' } });
        await flush();
        expect(mockLoggerError).toHaveBeenCalledWith('failed to compress image', expect.any(Error));
        expect(mockShowError).toHaveBeenCalledWith('Unable to load image');

        const errors: Array<[unknown, string | null]> = [
            [{ errorCode: 'cancelled' }, null],
            [{ errorCode: 'provider_unconfigured' }, 'OCR recognition requires configuration in Rule Center'],
            [{ errorCode: 'timeout' }, 'Recognition timed out, please try again'],
            [{ errorCode: 'parse_error' }, 'Could not parse this image, please try a clearer one'],
            [{ errorCode: 'rate_limited' }, 'Too many requests, please wait a moment'],
            [new Error('unknown failure'), 'Unable to recognize image']
        ];
        for (const [error, message] of errors) {
            bindings.imageFile.value = createImageFile();
            mockTransactionsStore.recognizeReceiptImage.mockRejectedValueOnce(error);
            bindings.recognize();
            await flush();
            if (message) expect(mockShowError).toHaveBeenCalledWith(message);
            expect(bindings.recognizing.value).toBe(false);
            expect(bindings.cancelRecognizingUuid.value).toBeUndefined();
        }
    });

    test('cancels pending recognition and rejects an open dialog on ordinary cancel', async () => {
        const pending = deferred<any>();
        mockTransactionsStore.recognizeReceiptImage.mockReturnValueOnce(pending.promise);
        const { bindings, exposed } = setupDesktop();
        const openPromise = exposed['open']();
        bindings.imageFile.value = createImageFile();
        bindings.recognize();
        bindings.cancelRecognize();
        expect(mockTransactionsStore.cancelRecognizeReceiptImage).toHaveBeenCalledWith('ocr-cancel-id');
        expect(mockShowMessage).toHaveBeenCalledWith('User Canceled');
        bindings.cancelRecognize();

        bindings.cancel();
        await expect(openPromise).rejects.toBeUndefined();
        expect(bindings.imageFile.value).toBeNull();
        pending.reject({ errorCode: 'cancelled' });
        await flush();
    });
});

describe('mobile AI image recognition sheet', () => {
    test('loads an image and emits a successful low-confidence result', async () => {
        const { bindings, emit } = setupMobile();
        bindings.onSheetOpen();
        bindings.showOpenImage();
        expect(bindings.imageInput.value.click).toHaveBeenCalled();
        const input = { files: [createImageFile()], value: 'selected' };
        bindings.openImage({ target: input });
        await flush();
        expect(input.value).toBe('');

        const response = { confidence: 0.1, merchant: 'Mobile Store' };
        mockTransactionsStore.recognizeReceiptImage.mockResolvedValueOnce(response);
        bindings.confirm();
        expect(mockShowCancelableLoading).toHaveBeenCalledWith(
            'Recognizing',
            'AI can make mistakes. Check important info.',
            'Cancel Recognition',
            expect.any(Function)
        );
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('Low confidence recognition, please verify');
        expect(mockCloseAllDialog).toHaveBeenCalled();
        expect(emit).toHaveBeenCalledWith('update:show', false);
        expect(emit).toHaveBeenCalledWith('recognition:change', response);
    });

    test('keeps cancel sentinels silent and maps all service error codes to user feedback', async () => {
        const { bindings, emit } = setupMobile();
        const errors: Array<[unknown, string | null, boolean]> = [
            [{ canceled: true }, null, false],
            [{ errorCode: 'cancelled' }, null, true],
            [{ errorCode: 'provider_unconfigured' }, 'OCR recognition requires configuration in Rule Center', true],
            [{ errorCode: 'timeout' }, 'Recognition timed out, please try again', true],
            [{ errorCode: 'parse_error' }, 'Could not parse this image, please try a clearer one', true],
            [{ errorCode: 'rate_limited' }, 'Too many requests, please wait a moment', true],
            [{}, 'Unable to recognize image', true]
        ];
        for (const [error, message, closesDialog] of errors) {
            bindings.imageFile.value = createImageFile();
            mockTransactionsStore.recognizeReceiptImage.mockRejectedValueOnce(error);
            const closeCount = mockCloseAllDialog.mock.calls.length;
            bindings.confirm();
            await flush();
            if (message) expect(mockShowToast).toHaveBeenCalledWith(message);
            expect(mockCloseAllDialog.mock.calls.length).toBe(closeCount + (closesDialog ? 1 : 0));
            expect(bindings.recognizing.value).toBe(false);
        }
        expect(emit).not.toHaveBeenCalledWith('update:show', false);
    });

    test('cancels recognition, resets lifecycle state, and reports compression failure', async () => {
        const pending = deferred<any>();
        mockTransactionsStore.recognizeReceiptImage.mockReturnValueOnce(pending.promise);
        const { bindings, emit } = setupMobile();
        bindings.imageFile.value = createImageFile();
        bindings.confirm();
        bindings.cancelRecognize();
        expect(mockTransactionsStore.cancelRecognizeReceiptImage).toHaveBeenCalledWith('ocr-cancel-id');
        expect(mockShowToast).toHaveBeenCalledWith('User Canceled');
        bindings.cancelRecognize();

        bindings.cancel();
        expect(emit).toHaveBeenCalledWith('update:show', false);
        bindings.loading.value = true;
        bindings.recognizing.value = true;
        bindings.showOpenImage();
        expect(bindings.imageInput.value.click).not.toHaveBeenCalled();

        bindings.onSheetOpen();
        expect(bindings.loading.value).toBe(false);
        bindings.onSheetClosed();
        expect(emit).toHaveBeenLastCalledWith('update:show', false);

        mockCompressJpgImage.mockRejectedValueOnce(new Error('mobile compression failure'));
        bindings.openImage({ target: { files: [createImageFile()], value: 'selected' } });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('Unable to load image');
        bindings.openImage({ target: null });
        bindings.openImage({ target: { files: [] } });

        pending.reject({ errorCode: 'cancelled' });
        await flush();
    });
});

describe('OCR production templates', () => {
    test('renders desktop and mobile state branches and exposes real event wrappers', async () => {
        const desktop = mountWithHostRenderer(DesktopDialog, {}, [
            'v-dialog', 'v-card', 'v-card-text', 'v-img', 'v-btn', 'v-progress-circular'
        ]);
        const mobile = mountWithHostRenderer(MobileSheet, { show: true }, [
            'f7-sheet', 'f7-toolbar', 'f7-link', 'f7-page-content'
        ]);
        try {
            desktop.state.showState = true;
            desktop.state.loading = false;
            desktop.state.imageFile = createImageFile();
            desktop.state.imageSrc = 'blob:desktop';
            desktop.state.isDragOver = true;
            await actualVue.nextTick();
            desktop.state.isDragOver = false;
            desktop.state.recognizing = true;
            desktop.state.cancelRecognizingUuid = 'ocr-cancel-id';

            mobile.state.loading = false;
            mobile.state.imageFile = createImageFile();
            mobile.state.imageSrc = 'blob:mobile';
            mobile.state.recognizing = true;
            await actualVue.nextTick();

            const desktopCallbacks = collectHostCallbacks(desktop.root);
            const mobileCallbacks = collectHostCallbacks(mobile.root);
            expect(desktopCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onPaste', 'onDragenter', 'onDragleave', 'onDrop', 'onClick', 'onChange'
            ]));
            expect(mobileCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onSheet:open', 'onSheet:closed', 'onClick', 'onChange'
            ]));

            for (const { name, callback } of desktopCallbacks) {
                if (name === 'onPaste') callback({ clipboardData: null, preventDefault: jest.fn() });
                else if (name === 'onDragenter' || name === 'onDragover' || name === 'onDragleave') {
                    callback({ preventDefault: jest.fn() });
                } else if (name === 'onDrop') callback({ preventDefault: jest.fn(), dataTransfer: null });
                else if (name === 'onChange') callback({ target: { files: [] } });
                else if (name.startsWith('onUpdate:')) callback(false);
                else if (name === 'onClick') callback();
                await flush(1);
            }

            for (const { name, callback } of mobileCallbacks) {
                if (name === 'onChange') callback({ target: { files: [] } });
                else if (name.startsWith('onUpdate:')) callback(false);
                else callback();
                await flush(1);
            }

            desktop.state.loading = true;
            desktop.state.recognizing = false;
            mobile.state.loading = true;
            mobile.state.imageSrc = undefined;
            await actualVue.nextTick();
            expect(desktop.root.children.length).toBeGreaterThan(0);
            expect(mobile.root.children.length).toBeGreaterThan(0);
        } finally {
            desktop.app.unmount();
            mobile.app.unmount();
        }
    });
});
