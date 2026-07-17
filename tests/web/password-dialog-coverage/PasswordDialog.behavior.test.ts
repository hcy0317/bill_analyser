import { afterAll, beforeEach, describe, expect, jest, test } from '@jest/globals';
import PasswordDialogSfc from '@/components/desktop/PasswordDialog.vue';

const mockTt = jest.fn((key: string, options?: Record<string, unknown>) => (
    options ? `tt:${key}:options` : `tt:${key}`
));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: mockTt })
}));

interface DialogProps {
    show?: boolean;
    title?: string;
    text?: string;
    warning?: string;
}

interface RefValue<T> {
    value: T;
}

interface DialogBindings {
    showState: RefValue<boolean>;
    password: RefValue<string>;
    showPassword: RefValue<boolean>;
    validationMessage: RefValue<string>;
    titleContent: RefValue<string>;
    textContent: RefValue<string>;
    warningContent: RefValue<string>;
    labelContent: RefValue<string>;
    placeholderContent: RefValue<string>;
    hintContent: RefValue<string>;
    open(
        titleOrText: string,
        textOrOptions?: string | Record<string, unknown>,
        options?: Record<string, unknown>
    ): Promise<string | undefined>;
    confirm(): void;
    cancel(): void;
    onKeydown(event: KeyboardEvent): void;
}

interface SetupContext {
    attrs: Record<string, unknown>;
    slots: Record<string, unknown>;
    emit: jest.Mock;
    expose(value: Record<string, unknown>): void;
}

interface PasswordDialogComponent {
    setup(props: DialogProps, context: SetupContext): DialogBindings;
    render(...args: unknown[]): unknown;
}

interface VueActual {
    reactive<T extends object>(value: T): T;
    proxyRefs<T extends object>(value: T): T;
    nextTick(): Promise<void>;
}

interface RenderEvent {
    name: string;
    callback: (...args: unknown[]) => unknown;
    node: Record<string, unknown>;
}

const actualVue = jest.requireActual('vue') as VueActual;
const PasswordDialog = PasswordDialogSfc as unknown as PasswordDialogComponent;

const consoleLogSpy = jest.spyOn(console, 'log').mockImplementation(() => undefined);
const consoleInfoSpy = jest.spyOn(console, 'info').mockImplementation(() => undefined);
const consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
const consoleErrorSpy = jest.spyOn(console, 'error').mockImplementation(() => undefined);

function setupDialog(initialProps: DialogProps = {}): {
    bindings: DialogBindings;
    emit: jest.Mock;
    exposed: Record<string, unknown>;
    props: DialogProps;
} {
    const emit = jest.fn();
    const exposed: Record<string, unknown> = {};
    const props = actualVue.reactive({ ...initialProps });
    const bindings = PasswordDialog.setup(props, {
        attrs: {},
        slots: {},
        emit,
        expose: value => Object.assign(exposed, value)
    });
    return { bindings, emit, exposed, props };
}

function createKeyboardEvent(key: string, tagName = 'DIV'): KeyboardEvent {
    return {
        key,
        preventDefault: jest.fn(),
        target: { tagName }
    } as unknown as KeyboardEvent;
}

function render(bindings: DialogBindings): unknown {
    const exposed = actualVue.proxyRefs(bindings) as unknown as Record<string, unknown>;
    return PasswordDialog.render(exposed, [], {}, exposed, {}, {});
}

function collectRenderEvents(
    value: unknown,
    events: RenderEvent[],
    nodes: Array<Record<string, unknown>>,
    seen = new Set<unknown>()
): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) {
        return;
    }
    seen.add(value);

    if (Array.isArray(value)) {
        for (const item of value) {
            collectRenderEvents(item, events, nodes, seen);
        }
        return;
    }

    const node = value as Record<string, unknown>;
    nodes.push(node);
    const props = node['props'];
    if (props && typeof props === 'object') {
        for (const [name, handler] of Object.entries(props as Record<string, unknown>)) {
            if (!name.startsWith('on')) {
                continue;
            }
            for (const candidate of Array.isArray(handler) ? handler : [handler]) {
                if (typeof candidate === 'function') {
                    events.push({
                        name,
                        callback: candidate as (...args: unknown[]) => unknown,
                        node
                    });
                }
            }
        }
    }

    const children = node['children'];
    if (Array.isArray(children)) {
        collectRenderEvents(children, events, nodes, seen);
    } else if (children && typeof children === 'object') {
        for (const slot of Object.values(children as Record<string, unknown>)) {
            if (typeof slot === 'function') {
                collectRenderEvents((slot as () => unknown)(), events, nodes, seen);
            } else {
                collectRenderEvents(slot, events, nodes, seen);
            }
        }
    }
}

function captureRender(bindings: DialogBindings): {
    events: RenderEvent[];
    nodes: Array<Record<string, unknown>>;
} {
    const events: RenderEvent[] = [];
    const nodes: Array<Record<string, unknown>> = [];
    collectRenderEvents(render(bindings), events, nodes);
    return { events, nodes };
}

function eventsNamed(events: RenderEvent[], name: string): RenderEvent[] {
    return events.filter(event => event.name === name);
}

function eventForColor(events: RenderEvent[], color: string): RenderEvent {
    const event = events.find(candidate => (
        candidate.name === 'onClick'
        && (candidate.node['props'] as Record<string, unknown> | undefined)?.['color'] === color
    ));
    if (!event) {
        throw new Error(`Missing click event for color ${color}`);
    }
    return event;
}

function expectSecretAbsentFromConsole(secret: string): void {
    for (const spy of [consoleLogSpy, consoleInfoSpy, consoleWarnSpy, consoleErrorSpy]) {
        expect(JSON.stringify(spy.mock.calls)).not.toContain(secret);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
});

afterAll(() => {
    consoleLogSpy.mockRestore();
    consoleInfoSpy.mockRestore();
    consoleWarnSpy.mockRestore();
    consoleErrorSpy.mockRestore();
});

describe('PasswordDialog open contracts', () => {
    test('initializes from props and resets state when controlled show becomes true', async () => {
        const { bindings, exposed, props } = setupDialog({
            show: false,
            title: 'Initial title',
            text: 'Initial text',
            warning: 'Initial warning'
        });

        expect(exposed).toEqual({ open: bindings.open });
        expect(bindings.showState.value).toBe(false);
        expect(bindings.titleContent.value).toBe('Initial title');
        expect(bindings.textContent.value).toBe('Initial text');
        expect(bindings.warningContent.value).toBe('Initial warning');
        expect(bindings.labelContent.value).toBe('tt:Operation Password');
        expect(bindings.placeholderContent.value).toBe('tt:Please enter operation password');
        expect(bindings.hintContent.value).toContain('BILL_ANALYSER_OPERATION_PASSWORD');

        bindings.password.value = '<stale-password>';
        bindings.validationMessage.value = 'stale validation';
        bindings.showPassword.value = true;
        props.show = true;
        await actualVue.nextTick();

        expect(bindings.showState.value).toBe(true);
        expect(bindings.password.value).toBe('');
        expect(bindings.validationMessage.value).toBe('');
        expect(bindings.showPassword.value).toBe(false);

        bindings.password.value = '<kept-while-closing>';
        props.show = false;
        await actualVue.nextTick();
        expect(bindings.showState.value).toBe(false);
        expect(bindings.password.value).toBe('<kept-while-closing>');
    });

    test('opens a default-title prompt from one localized text argument', async () => {
        const { bindings } = setupDialog();
        bindings.password.value = 'stale';
        bindings.validationMessage.value = 'stale';
        bindings.showPassword.value = true;
        bindings.warningContent.value = 'stale';
        bindings.labelContent.value = 'stale';
        bindings.placeholderContent.value = 'stale';
        bindings.hintContent.value = 'stale';

        const pending = bindings.open('Delete all data?');
        expect(bindings.showState.value).toBe(true);
        expect(bindings.password.value).toBe('');
        expect(bindings.validationMessage.value).toBe('');
        expect(bindings.showPassword.value).toBe(false);
        expect(bindings.titleContent.value).toBe('tt:Verify Operation Password');
        expect(bindings.textContent.value).toBe('tt:Delete all data?');
        expect(bindings.warningContent.value).toBe('');
        expect(bindings.labelContent.value).toBe('tt:Operation Password');
        expect(bindings.placeholderContent.value).toBe('tt:Please enter operation password');
        expect(bindings.hintContent.value).toContain('BILL_ANALYSER_OPERATION_PASSWORD');

        bindings.password.value = '<single-argument-secret>';
        bindings.confirm();
        await expect(pending).resolves.toBe('<single-argument-secret>');
    });

    test('localizes object options and ignores non-string display overrides', async () => {
        const { bindings } = setupDialog();
        const options = {
            account: 'Cash',
            warning: 'Irreversible warning',
            label: 'Step-up password',
            placeholder: 'Enter step-up password',
            hint: 'Custom hint'
        };
        const customized = bindings.open('Delete account {account}?', options);

        expect(bindings.titleContent.value).toBe('tt:Verify Operation Password');
        expect(bindings.textContent.value).toBe('tt:Delete account {account}?:options');
        expect(bindings.warningContent.value).toBe('tt:Irreversible warning:options');
        expect(bindings.labelContent.value).toBe('tt:Step-up password:options');
        expect(bindings.placeholderContent.value).toBe('tt:Enter step-up password:options');
        expect(bindings.hintContent.value).toBe('tt:Custom hint:options');

        const customizedRejection = expect(customized).rejects.toBeUndefined();
        bindings.cancel();
        await customizedRejection;

        const invalidOverrides = bindings.open('Prompt', {
            warning: 1,
            label: false,
            placeholder: null,
            hint: { nested: true }
        });
        expect(bindings.warningContent.value).toBe('');
        expect(bindings.labelContent.value).toBe('tt:Operation Password');
        expect(bindings.placeholderContent.value).toBe('tt:Please enter operation password');
        expect(bindings.hintContent.value).toContain('BILL_ANALYSER_OPERATION_PASSWORD');
        bindings.password.value = '<invalid-options-secret>';
        bindings.confirm();
        await expect(invalidOverrides).resolves.toBe('<invalid-options-secret>');
    });

    test('supports explicit title/text with and without localized options', async () => {
        const { bindings } = setupDialog();
        const plain = bindings.open('Verify login password', 'Enter your login password');
        expect(bindings.titleContent.value).toBe('tt:Verify login password');
        expect(bindings.textContent.value).toBe('tt:Enter your login password');
        bindings.password.value = '<plain-secret>';
        bindings.confirm();
        await expect(plain).resolves.toBe('<plain-secret>');

        const options = {
            user: 'Alice',
            warning: 'Warning for {user}',
            label: 'Password for {user}',
            placeholder: 'Password placeholder',
            hint: ''
        };
        const localized = bindings.open('Verify {user}', 'Enter password for {user}', options);
        expect(bindings.titleContent.value).toBe('tt:Verify {user}:options');
        expect(bindings.textContent.value).toBe('tt:Enter password for {user}:options');
        expect(bindings.warningContent.value).toBe('tt:Warning for {user}:options');
        expect(bindings.labelContent.value).toBe('tt:Password for {user}:options');
        expect(bindings.placeholderContent.value).toBe('tt:Password placeholder:options');
        expect(bindings.hintContent.value).toBe('tt::options');
        const localizedRejection = expect(localized).rejects.toBeUndefined();
        bindings.cancel();
        await localizedRejection;

        const invalidOverrides = bindings.open('Verify', 'Enter password', {
            warning: 1,
            label: false,
            placeholder: null,
            hint: { nested: true }
        });
        expect(bindings.warningContent.value).toBe('');
        expect(bindings.labelContent.value).toBe('tt:Operation Password');
        expect(bindings.placeholderContent.value).toBe('tt:Please enter operation password');
        expect(bindings.hintContent.value).toContain('BILL_ANALYSER_OPERATION_PASSWORD');
        bindings.password.value = '<invalid-explicit-options-secret>';
        bindings.confirm();
        await expect(invalidOverrides).resolves.toBe('<invalid-explicit-options-secret>');

        const unsupportedRuntimeInput = bindings.open(
            'Runtime-only input',
            42 as unknown as string
        );
        bindings.password.value = '<runtime-input-secret>';
        bindings.confirm();
        await expect(unsupportedRuntimeInput).resolves.toBe('<runtime-input-secret>');
    });
});

describe('PasswordDialog confirmation, cancellation, and keyboard behavior', () => {
    test('validates empty confirmation and settles successful confirmation without logging the password', async () => {
        const { bindings, emit } = setupDialog();
        bindings.confirm();
        expect(bindings.validationMessage.value).toBe('tt:Password cannot be empty');
        expect(emit).not.toHaveBeenCalled();

        const secret = '<sensitive-operation-password>';
        const pending = bindings.open('Verify');
        bindings.password.value = secret;
        bindings.confirm();

        await expect(pending).resolves.toBe(secret);
        expect(bindings.showState.value).toBe(false);
        expect(emit).toHaveBeenCalledWith('update:show', false);
        expectSecretAbsentFromConsole(secret);

        const detached = setupDialog();
        detached.bindings.password.value = '<detached-secret>';
        detached.bindings.confirm();
        expect(detached.emit).toHaveBeenCalledWith('update:show', false);
        expectSecretAbsentFromConsole('<detached-secret>');
    });

    test('rejects cancellation and safely closes even before open has installed a reject callback', async () => {
        const { bindings, emit } = setupDialog();
        bindings.cancel();
        expect(bindings.showState.value).toBe(false);
        expect(emit).toHaveBeenCalledWith('update:show', false);

        emit.mockClear();
        const pending = bindings.open('Verify');
        const rejection = expect(pending).rejects.toBeUndefined();
        bindings.cancel();
        await rejection;
        expect(bindings.showState.value).toBe(false);
        expect(emit).toHaveBeenCalledWith('update:show', false);
    });

    test('handles Enter, Escape, Backspace focus boundaries, and unrelated keys', async () => {
        const enterDialog = setupDialog();
        const enterPending = enterDialog.bindings.open('Verify');
        enterDialog.bindings.password.value = '<enter-secret>';
        const enterEvent = createKeyboardEvent('Enter');
        enterDialog.bindings.onKeydown(enterEvent);
        await expect(enterPending).resolves.toBe('<enter-secret>');
        expect(enterEvent.preventDefault).toHaveBeenCalledTimes(1);

        const emptyEnter = setupDialog();
        const emptyEnterEvent = createKeyboardEvent('Enter');
        emptyEnter.bindings.onKeydown(emptyEnterEvent);
        expect(emptyEnterEvent.preventDefault).not.toHaveBeenCalled();
        expect(emptyEnter.emit).not.toHaveBeenCalled();

        const escapeDialog = setupDialog();
        const escapePending = escapeDialog.bindings.open('Verify');
        const escapeRejection = expect(escapePending).rejects.toBeUndefined();
        const escapeEvent = createKeyboardEvent('Escape');
        escapeDialog.bindings.onKeydown(escapeEvent);
        await escapeRejection;
        expect(escapeEvent.preventDefault).toHaveBeenCalledTimes(1);

        const backspaceDialog = setupDialog();
        const backspacePending = backspaceDialog.bindings.open('Verify');
        const inputBackspace = createKeyboardEvent('Backspace', 'INPUT');
        backspaceDialog.bindings.onKeydown(inputBackspace);
        expect(inputBackspace.preventDefault).not.toHaveBeenCalled();
        expect(backspaceDialog.bindings.showState.value).toBe(true);

        const divBackspace = createKeyboardEvent('Backspace', 'DIV');
        const backspaceRejection = expect(backspacePending).rejects.toBeUndefined();
        backspaceDialog.bindings.onKeydown(divBackspace);
        await backspaceRejection;
        expect(divBackspace.preventDefault).toHaveBeenCalledTimes(1);

        const otherDialog = setupDialog();
        const otherEvent = createKeyboardEvent('ArrowDown');
        otherDialog.bindings.onKeydown(otherEvent);
        expect(otherEvent.preventDefault).not.toHaveBeenCalled();
        expect(otherDialog.emit).not.toHaveBeenCalled();
    });
});

describe('PasswordDialog production template behavior', () => {
    test('renders conditional content and wires model, visibility, validation, keyboard, and action events', async () => {
        const { bindings, emit } = setupDialog({
            title: 'Rendered title',
            text: 'Rendered text',
            warning: '<strong>Rendered warning</strong>'
        });
        bindings.validationMessage.value = 'Rendered validation';
        const firstRender = captureRender(bindings);

        const textField = firstRender.nodes.find(node => {
            const props = node['props'] as Record<string, unknown> | undefined;
            return props?.['label'] === 'tt:Operation Password';
        });
        expect(textField?.['props']).toEqual(expect.objectContaining({
            type: 'password',
            'append-inner-icon': 'mdi-eye',
            'error-messages': 'Rendered validation'
        }));
        const confirmButton = firstRender.nodes.find(node => {
            const props = node['props'] as Record<string, unknown> | undefined;
            return props?.['color'] === 'error' && Reflect.has(props, 'disabled');
        });
        expect((confirmButton?.['props'] as Record<string, unknown>)['disabled']).toBe(true);

        for (const event of eventsNamed(firstRender.events, 'onUpdate:modelValue')) {
            const props = event.node['props'] as Record<string, unknown> | undefined;
            event.callback(props?.['label'] === 'tt:Operation Password' ? '<template-secret>' : true);
        }
        expect(bindings.validationMessage.value).toBe('');

        const appendEvent = firstRender.events.find(event => event.name.toLowerCase().includes('click:append'));
        expect(appendEvent).toBeDefined();
        appendEvent?.callback();
        expect(bindings.showPassword.value).toBe(true);

        const cardKeydown = eventsNamed(firstRender.events, 'onKeydown')[0];
        expect(cardKeydown).toBeDefined();
        cardKeydown?.callback(createKeyboardEvent('ArrowDown'));

        bindings.password.value = '<template-secret>';
        const secondRender = captureRender(bindings);
        const visibleTextField = secondRender.nodes.find(node => {
            const props = node['props'] as Record<string, unknown> | undefined;
            return props?.['type'] === 'text' && props?.['append-inner-icon'] === 'mdi-eye-off';
        });
        expect(visibleTextField).toBeDefined();

        const renderedPending = bindings.open('Rendered confirmation');
        bindings.password.value = '<render-confirm-secret>';
        const confirmRender = captureRender(bindings);
        eventForColor(confirmRender.events, 'error').callback();
        await expect(renderedPending).resolves.toBe('<render-confirm-secret>');
        expect(emit).toHaveBeenCalledWith('update:show', false);
        expectSecretAbsentFromConsole('<render-confirm-secret>');

        const cancelPending = bindings.open('Rendered cancellation');
        const cancelRejection = expect(cancelPending).rejects.toBeUndefined();
        const cancelRender = captureRender(bindings);
        eventForColor(cancelRender.events, 'gray').callback();
        await cancelRejection;

        bindings.textContent.value = '';
        bindings.warningContent.value = '';
        bindings.hintContent.value = '';
        expect(() => captureRender(bindings)).not.toThrow();
    });
});
