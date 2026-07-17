import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockPreloaderShow = jest.fn();
const mockPreloaderHide = jest.fn();
const mockDialogClose = jest.fn();
const mockPickerCreate = jest.fn((options: unknown) => ({ options }));
const mockSwipeoutDelete = jest.fn();
const mockDialogOpen = jest.fn();
const mockToastOpen = jest.fn();
const mockDialogConfigs: any[] = [];
const mockToastConfigs: any[] = [];
const mockUnwatch = jest.fn();
const mockWindowScrollTo = jest.fn();
const mockWindowOpen = jest.fn();
let mockDirection = 'ltr';
let mockDollarImpl: (selector: any) => any = () => ({ length: 0 });

const mockF7 = {
    preloader: { show: mockPreloaderShow, hide: mockPreloaderHide },
    dialog: {
        close: mockDialogClose,
        create: jest.fn((config: any) => {
            mockDialogConfigs.push(config);
            return { open: mockDialogOpen };
        })
    },
    picker: { create: (config: any) => mockPickerCreate(config) },
    swipeout: { delete: (...args: any[]) => mockSwipeoutDelete(...args) },
    toast: {
        create: jest.fn((config: any) => {
            mockToastConfigs.push(config);
            return { open: mockToastOpen };
        })
    },
    $: (selector: any) => mockDollarImpl(selector)
};

jest.mock('vue', () => ({
    watch: (source: { value: unknown }, callback: (value: unknown) => void, options: { immediate?: boolean }) => {
        if (options?.immediate) callback(source.value);
        return mockUnwatch;
    }
}));
jest.mock('framework7-vue', () => ({
    f7: mockF7,
    f7ready: (callback: (value: typeof mockF7) => unknown) => callback(mockF7)
}), { virtual: true });
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        te: (key: string) => `te:${key}`,
        getCurrentLanguageTextDirection: () => mockDirection
    })
}));
jest.mock('@/core/text.ts', () => ({ TextDirection: { LTR: 'ltr', RTL: 'rtl' } }));
jest.mock('@/core/font.ts', () => ({
    FONT_SIZE_PREVIEW_CLASSNAME_PREFIX: 'preview-',
    FontSize: {
        Default: { type: 0, className: 'font-default' },
        values: () => [
            { type: 0, className: 'font-default' },
            { type: 1, className: 'font-large' },
            { type: 2, className: 'font-huge' }
        ]
    }
}));
jest.mock('@/lib/common.ts', () => ({
    getNumberValue: (value: string | number, fallback: number) => Number.isFinite(Number(value)) ? Number(value) : fallback
}));
jest.mock('@/lib/settings.ts', () => ({ isEnableAnimate: () => true }));

const mobileUi = require('@/lib/ui/mobile.ts') as any;

function dom(overrides: Record<string, unknown> = {}): any {
    return {
        length: 1,
        find: () => dom({ length: 0 }),
        offset: () => ({ top: 0, left: 0 }),
        scrollTop: jest.fn(),
        outerHeight: () => 100,
        css: () => '0',
        ...overrides
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    jest.useFakeTimers();
    mockDialogConfigs.length = 0;
    mockToastConfigs.length = 0;
    mockDirection = 'ltr';
    mockDollarImpl = () => ({ length: 0 });
    Object.defineProperty(globalThis, 'window', {
        configurable: true,
        value: { innerHeight: 600, scrollTo: mockWindowScrollTo, open: mockWindowOpen }
    });
});

afterEach(() => {
    jest.useRealTimers();
});

describe('mobile UI framework helpers', () => {
    test('shows and hides loading states with immediate and delayed conditions', () => {
        mobileUi.showLoading();
        expect(mockPreloaderShow).toHaveBeenCalledTimes(1);
        mobileUi.showLoading(() => true, 50);
        mobileUi.showLoading(() => false, 0);
        jest.runAllTimers();
        expect(mockPreloaderShow).toHaveBeenCalledTimes(2);
        mobileUi.hideLoading();
        mobileUi.closeAllDialog();
        expect(mockPreloaderHide).toHaveBeenCalledTimes(1);
        expect(mockDialogClose).toHaveBeenCalledTimes(1);
    });

    test('creates pickers, reports modals, deletes swipeouts, and resizes textareas', () => {
        const change = jest.fn();
        expect(mobileUi.createInlinePicker('#container', '#input', [{ values: ['a'] }], ['a'], { change }))
            .toEqual({ options: expect.objectContaining({ containerEl: '#container', inputEl: '#input', on: { change } }) });
        mobileUi.createInlinePicker('#container', '#input', [], []);
        expect(mockPickerCreate).toHaveBeenLastCalledWith(expect.objectContaining({ on: {} }));

        mockDollarImpl = selector => selector === '.modal-in' ? { length: 2 } : ({ length: 0 });
        expect(mobileUi.isModalShowing()).toBe(2);
        const deleted = jest.fn();
        mobileUi.onSwipeoutDeleted('row-1', deleted);
        expect(mockSwipeoutDelete).toHaveBeenCalledWith('#row-1', deleted);

        const textarea = { scrollTop: 7, scrollHeight: 88, style: { height: '10px' } };
        mockDollarImpl = () => ({ find: () => ({ each: (callback: (value: any) => void) => callback(textarea) }) });
        mobileUi.autoChangeTextareaSize({});
        expect(textarea).toEqual(expect.objectContaining({ scrollTop: 0, style: { height: '88px' } }));
    });

    test('changes font classes and resolves preview fallbacks', () => {
        const classes = new Set<string>(['font-default']);
        const html = {
            hasClass: (name: string) => classes.has(name),
            addClass: (name: string) => classes.add(name),
            removeClass: (name: string) => classes.delete(name)
        };
        mockDollarImpl = () => html;
        mobileUi.setAppFontSize(1);
        expect(classes).toEqual(new Set(['font-large']));
        mobileUi.setAppFontSize(1);
        expect(mobileUi.getFontSizePreviewClassName(2)).toBe('preview-font-huge');
        expect(mobileUi.getFontSizePreviewClassName(999)).toBe('preview-font-default');
    });

    test('reads element heights and bounding rectangles defensively', () => {
        mockDollarImpl = () => ({ length: 0 });
        expect(mobileUi.getElementActualHeights('.none')).toEqual({});
        expect(mobileUi.getElementBoundingRect('.none')).toBeNull();

        const rect = { height: 42 };
        const first = { id: 'first', getBoundingClientRect: () => rect };
        const elements = { 0: first, 1: undefined, length: 2 };
        mockDollarImpl = () => elements;
        expect(mobileUi.getElementActualHeights('.rows')).toEqual({ first: 42 });
        expect(mobileUi.getElementBoundingRect('.rows')).toBe(rect);
        mockDollarImpl = () => ({ 0: undefined, length: 1 });
        expect(mobileUi.getElementBoundingRect('.missing-first')).toBeNull();
    });

    test('centers one or multiple selected items and handles invalid collections', () => {
        mobileUi.scrollToSelectedItem(null, '.container', '.selected');
        const emptyParent = dom({ find: () => dom({ length: 0 }) });
        mobileUi.scrollToSelectedItem(emptyParent, '.container', '.selected');

        const scroll = jest.fn();
        const container = dom({ offset: () => ({ top: 10, left: 0 }), outerHeight: () => 100, css: () => '20', scrollTop: scroll });
        const single = dom({ offset: () => ({ top: 90, left: 0 }), outerHeight: () => 20 });
        const parent = dom({ find: (selector: string) => selector === '.container' ? container : single });
        mobileUi.scrollToSelectedItem(parent, '.container', '.selected');
        expect(scroll).toHaveBeenCalledWith(30);

        const firstElement: any = {};
        const lastElement: any = {};
        firstElement.__dom = dom({ offset: () => ({ top: 30, left: 0 }), outerHeight: () => 20 });
        lastElement.__dom = dom({ offset: () => ({ top: 250, left: 0 }), outerHeight: () => 20 });
        const multiple = dom({ 0: firstElement, 1: lastElement, length: 2 });
        mockDollarImpl = selector => typeof selector === 'object' ? selector.__dom : dom({ length: 0 });
        const multiParent = dom({ find: (selector: string) => selector === '.container' ? container : multiple });
        mobileUi.scrollToSelectedItem(multiParent, '.container', '.selected');
        expect(scroll).toHaveBeenLastCalledWith(10);

        const invalidMultiple = dom({ 0: undefined, 1: lastElement, length: 2 });
        mobileUi.scrollToSelectedItem(dom({ find: (selector: string) => selector === '.container' ? container : invalidMultiple }), '.container', '.selected');
        const above = dom({ offset: () => ({ top: 0, left: 0 }), outerHeight: () => 20 });
        mobileUi.scrollToSelectedItem(dom({ find: (selector: string) => selector === '.container' ? container : above }), '.container', '.selected');
    });

    test('scrolls sheets after keyboards resize the viewport and binds infinite scrolling', () => {
        mobileUi.scrollSheetToTop(undefined, 800);
        mobileUi.scrollSheetToTop({ offsetHeight: 900 }, 800);
        mobileUi.scrollSheetToTop({ offsetHeight: 400 }, 800);
        (globalThis.window as any).innerHeight = 600;
        jest.runAllTimers();
        expect(mockWindowScrollTo).toHaveBeenCalledWith({ top: 376, behavior: 'smooth' });

        const callback = jest.fn();
        const on = jest.fn((_event: string, handler: (event: Event) => void) => handler({ type: 'scroll' } as Event));
        mockDollarImpl = () => ({ on });
        mobileUi.onInfiniteScrolling(callback);
        expect(callback).toHaveBeenCalledWith(expect.objectContaining({ type: 'scroll' }));
    });
});

describe('mobile localized dialog helpers', () => {
    test('routes back on immediate errors and builds alert/confirm order in both directions', () => {
        const router = { back: jest.fn() };
        const ui = mobileUi.useI18nUIComponents();
        ui.routeBackOnError(router, { value: { message: 'bad' } });
        jest.runAllTimers();
        expect(mockUnwatch).toHaveBeenCalled();
        expect(router.back).toHaveBeenCalled();
        ui.routeBackOnError(router, { value: null });

        const confirm = jest.fn();
        const cancel = jest.fn();
        ui.showAlert('alert', confirm);
        expect(mockDialogConfigs.at(-1)).toEqual(expect.objectContaining({
            text: 'te:alert', buttons: [expect.objectContaining({ text: 'tt:OK', onClick: confirm })]
        }));
        ui.showConfirm('confirm', confirm, cancel);
        expect(mockDialogConfigs.at(-1).buttons.map((button: any) => button.text)).toEqual(['tt:Cancel', 'tt:OK']);
        mockDirection = 'rtl';
        ui.showConfirm('confirm', confirm, cancel);
        expect(mockDialogConfigs.at(-1).buttons.map((button: any) => button.text)).toEqual(['tt:OK', 'tt:Cancel']);
        expect(mockDialogOpen).toHaveBeenCalledTimes(3);
    });

    test('passes prompt values to callbacks and covers absent callbacks', () => {
        const ui = mobileUi.useI18nUIComponents();
        const confirm = jest.fn();
        const cancel = jest.fn();
        ui.showPrompt('prompt', 'seed', confirm, cancel);
        const config = mockDialogConfigs.at(-1);
        expect(config.content).toContain('value="seed"');
        const dialog = { $el: { find: () => ({ val: () => 'typed' }) } };
        config.buttons[0].onClick(dialog, { type: 'cancel' });
        config.buttons[1].onClick(dialog, { type: 'confirm' });
        expect(cancel).toHaveBeenCalledWith('typed', dialog, expect.anything());
        expect(confirm).toHaveBeenCalledWith('typed', dialog, expect.anything());
        ui.showPrompt('prompt');
        const noCallbacks = mockDialogConfigs.at(-1);
        noCallbacks.buttons[0].onClick(dialog, {});
        noCallbacks.buttons[1].onClick(dialog, {});
        mockDirection = 'rtl';
        ui.showPrompt('prompt', '');
        expect(mockDialogConfigs.at(-1).buttons.map((button: any) => button.text)).toEqual(['tt:OK', 'tt:Cancel']);
    });

    test('builds cancelable loading, toast defaults, and external URL confirmation', () => {
        const ui = mobileUi.useI18nUIComponents();
        const cancel = jest.fn();
        ui.showCancelableLoading('Loading', 'Please wait', 'Stop', cancel);
        let config = mockDialogConfigs.at(-1);
        expect(config.content).toContain('preloader-inner-line');
        expect(config.content).toContain('tt:Please wait');
        config.buttons[0].onClick({ id: 'dialog' }, { type: 'click' });
        expect(cancel).toHaveBeenCalled();
        ui.showCancelableLoading('Loading', '', 'Stop');
        config = mockDialogConfigs.at(-1);
        expect(config.content).not.toContain('margin-top');
        config.buttons[0].onClick({}, {});

        ui.showToast('saved');
        expect(mockToastConfigs.at(-1)).toEqual(expect.objectContaining({ text: 'te:saved', closeTimeout: 1500 }));
        ui.showToast('saved', 9_000);
        expect(mockToastConfigs.at(-1).closeTimeout).toBe(9_000);

        ui.openExternalUrl('https://example.com');
        config = mockDialogConfigs.at(-1);
        expect(config.buttons.map((button: any) => button.text)).toEqual(['tt:Cancel', 'tt:OK']);
        config.buttons[1].onClick();
        expect(mockWindowOpen).toHaveBeenCalledWith('https://example.com', '_blank');
        mockDirection = 'rtl';
        ui.openExternalUrl('https://example.com/rtl');
        expect(mockDialogConfigs.at(-1).buttons.map((button: any) => button.text)).toEqual(['tt:OK', 'tt:Cancel']);
    });
});
