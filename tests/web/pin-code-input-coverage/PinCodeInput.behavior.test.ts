import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockWesternNumerals = {
    isDigit: (value: string) => value.length === 1 && value >= '0' && value <= '9',
    replaceLocalizedDigitsToWesternArabicDigits: (value: string) => value
};

const mockEasternDigitMap: Record<string, string> = {
    '٠': '0',
    '١': '1',
    '٢': '2',
    '٣': '3',
    '٤': '4',
    '٥': '5',
    '٦': '6',
    '٧': '7',
    '٨': '8',
    '٩': '9'
};

const mockEasternNumerals = {
    isDigit: (value: string) => Object.prototype.hasOwnProperty.call(mockEasternDigitMap, value),
    replaceLocalizedDigitsToWesternArabicDigits: (value: string) => (
        [...value].map(digit => mockEasternDigitMap[digit] ?? digit).join('')
    )
};

let mockCurrentNumeralSystem = mockEasternNumerals;
const mockPinCodeInputs = (jest.requireActual('vue') as any).ref(null);

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => mockPinCodeInputs
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        getCurrentNumeralSystemType: () => mockCurrentNumeralSystem
    })
}));
jest.mock('@/core/numeral.ts', () => ({
    NumeralSystem: {
        WesternArabicNumerals: mockWesternNumerals
    }
}));

import PinCodeInput from '@/components/common/PinCodeInput.vue';

type PinProps = {
    modelValue: string;
    length: number;
    disabled?: boolean;
    autofocus?: boolean;
    autoConfirm?: boolean;
    secure?: boolean;
};

function setup(overrides: Partial<PinProps> = {}): {
    bindings: any;
    emit: jest.Mock;
    props: PinProps;
} {
    const { reactive } = jest.requireActual('vue') as any;
    const props = reactive({
        modelValue: '',
        length: 4,
        disabled: false,
        autofocus: false,
        autoConfirm: false,
        secure: false,
        ...overrides
    }) as PinProps;
    const emit = jest.fn();
    const bindings = (PinCodeInput as any).setup(props, { emit, expose: jest.fn() });
    return { bindings, emit, props };
}

async function flush(times = 4): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await (jest.requireActual('vue') as any).nextTick();
}

function keyboardEvent(overrides: Record<string, unknown> = {}): any {
    return {
        key: 'x',
        code: 'KeyX',
        altKey: false,
        shiftKey: false,
        ctrlKey: false,
        metaKey: false,
        preventDefault: jest.fn(),
        ...overrides
    };
}

function clipboardEvent(text?: string, includeClipboard = true): any {
    return {
        clipboardData: includeClipboard ? { getData: jest.fn(() => text ?? '') } : null,
        preventDefault: jest.fn()
    };
}

function inputEvent(value?: string, includeTarget = true): any {
    return {
        target: includeTarget ? { value: value ?? '' } : null,
        preventDefault: jest.fn()
    };
}

function createHostNode(type: string, text = ''): any {
    return {
        type,
        text,
        children: [],
        parent: null,
        props: {},
        style: {},
        focus: jest.fn(),
        select: jest.fn()
    };
}

function mountWithHostRenderer(props: PinProps): { app: any; root: any; state: any } {
    const { createRenderer } = jest.requireActual('vue') as any;
    const renderer = createRenderer({
        patchProp(node: any, key: string, _previous: unknown, value: unknown) {
            node.props[key] = value;
        },
        insert(child: any, parent: any, anchor: any = null) {
            child.parent = parent;
            if (!anchor) {
                parent.children.push(child);
                return;
            }
            const index = parent.children.indexOf(anchor);
            parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
        },
        remove(child: any) {
            const index = child.parent?.children.indexOf(child) ?? -1;
            if (index >= 0) child.parent.children.splice(index, 1);
        },
        createElement(type: string) {
            return createHostNode(type);
        },
        createText(text: string) {
            return createHostNode('#text', text);
        },
        createComment(text: string) {
            return createHostNode('#comment', text);
        },
        setText(node: any, text: string) {
            node.text = text;
        },
        setElementText(node: any, text: string) {
            node.text = text;
            node.children = [];
        },
        parentNode(node: any) {
            return node.parent;
        },
        nextSibling(node: any) {
            const siblings = node.parent?.children ?? [];
            return siblings[siblings.indexOf(node) + 1] ?? null;
        },
        querySelector() {
            return null;
        },
        setScopeId(node: any, scopeId: string) {
            node.props[scopeId] = '';
        },
        cloneNode(node: any) {
            return { ...node, children: [...node.children], props: { ...node.props }, parent: null };
        },
        insertStaticContent(content: string, parent: any, anchor: any) {
            const node = createHostNode('#static', content);
            node.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, node);
            return [node, node];
        }
    });
    const app = renderer.createApp(PinCodeInput as any, props);
    app.config.warnHandler = () => undefined;
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState };
}

function collectNodes(node: any, type: string, result: any[] = [], seen = new Set<any>()): any[] {
    if (!node || typeof node !== 'object' || seen.has(node)) return result;
    seen.add(node);
    if (node.type === type) result.push(node);
    for (const child of node.children ?? []) collectNodes(child, type, result, seen);
    return result;
}

beforeEach(() => {
    jest.clearAllMocks();
    jest.useRealTimers();
    mockCurrentNumeralSystem = mockEasternNumerals;
    mockPinCodeInputs.value = null;
    Reflect.deleteProperty(globalThis, 'length');
});

describe('PinCodeInput state and secure display contracts', () => {
    test('initializes exact length, truncates excess model input, and stops the final value at the first gap', () => {
        const secure = setup({ modelValue: '12345', length: 4, secure: true });
        expect(secure.bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['1', '2', '3', '4']);
        expect(secure.bindings.codes.value.map((code: any) => code.inputType)).toStrictEqual([
            'password', 'password', 'password', 'password'
        ]);
        expect(secure.bindings.finalPinCode.value).toBe('1234');
        expect(secure.bindings.numeralSystem.value).toBe(mockEasternNumerals);

        secure.bindings.codes.value[1].value = '';
        secure.bindings.codes.value[2].value = '9';
        expect(secure.bindings.finalPinCode.value).toBe('1');

        const plain = setup({ modelValue: '7', length: 3, secure: false });
        expect(plain.bindings.codes.value.map((code: any) => code.inputType)).toStrictEqual(['tel', 'tel', 'tel']);
        expect(plain.bindings.codes.value).toHaveLength(3);
    });

    test('fills valid digits, truncates long input, rejects non-digits, focuses the last accepted slot, and confirms completion', async () => {
        const inputs = Array.from({ length: 4 }, () => ({ focus: jest.fn(), select: jest.fn() }));
        mockPinCodeInputs.value = inputs;
        const { bindings, emit } = setup({ length: 4 });

        bindings.autoFillText(0, '123456');
        expect(bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['1', '2', '3', '4']);
        expect(bindings.finalPinCode.value).toBe('1234');
        expect(inputs[3]?.focus).toHaveBeenCalled();
        expect(inputs[3]?.select).toHaveBeenCalled();
        expect(emit).toHaveBeenCalledWith('pincode:confirm', '1234');

        bindings.autoFillText(1, '8x9');
        expect(bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['1', '8', '', '4']);
        expect(bindings.finalPinCode.value).toBe('18');
        expect(inputs[1]?.focus).toHaveBeenCalled();

        bindings.autoFillText(2, '');
        expect(inputs[2]?.focus).toHaveBeenCalled();
        await flush();
        expect(emit).toHaveBeenCalledWith('update:modelValue', '18');
    });

    test('uses component length rather than the browser global when deciding completion', () => {
        Object.defineProperty(globalThis, 'length', {
            configurable: true,
            value: 1,
            writable: true
        });
        const { bindings, emit } = setup({ length: 4 });

        bindings.autoFillText(0, '1');
        expect(emit).not.toHaveBeenCalledWith('pincode:confirm', '1');

        bindings.autoFillText(0, '1234');
        expect(emit).toHaveBeenCalledWith('pincode:confirm', '1234');
    });

    test('delays secure masking, avoids duplicate timers, and restores telephone input for cleared values', () => {
        jest.useFakeTimers();
        try {
            const { bindings, props } = setup({ length: 2, secure: false });
            bindings.codes.value[0].value = '5';
            bindings.setInputType(0);
            expect(bindings.codes.value[0].inputTimer).toBeNull();

            props.secure = true;
            bindings.setInputType(99);
            bindings.codes.value[0].inputType = 'password';
            bindings.codes.value[0].value = '';
            bindings.setInputType(0);
            expect(bindings.codes.value[0].inputType).toBe('tel');

            bindings.codes.value[0].value = '5';
            bindings.setInputType(0);
            const firstTimer = bindings.codes.value[0].inputTimer;
            expect(firstTimer).not.toBeNull();
            bindings.setInputType(0);
            expect(bindings.codes.value[0].inputTimer).toBe(firstTimer);
            jest.advanceTimersByTime(300);
            expect(bindings.codes.value[0].inputType).toBe('password');
            expect(bindings.codes.value[0].inputTimer).toBeNull();

            bindings.codes.value[1].value = '6';
            bindings.setInputType(1);
            bindings.codes.value[1].value = '';
            jest.advanceTimersByTime(300);
            expect(bindings.codes.value[1].inputType).toBe('tel');
            expect(bindings.codes.value[1].inputTimer).toBeNull();
        } finally {
            jest.runOnlyPendingTimers();
            jest.useRealTimers();
        }
    });

    test('focuses available template refs and respects previous and next boundaries', () => {
        const { bindings } = setup({ length: 3 });
        bindings.setFocus(0);
        mockPinCodeInputs.value = [];
        bindings.setFocus(0);

        const inputs = Array.from({ length: 3 }, () => ({ focus: jest.fn(), select: jest.fn() }));
        mockPinCodeInputs.value = inputs;
        bindings.setPreviousFocus(0);
        bindings.setPreviousFocus(2);
        bindings.setNextFocus(2);
        bindings.setNextFocus(0);

        expect(inputs[1]?.focus).toHaveBeenCalledTimes(2);
        expect(inputs[1]?.select).toHaveBeenCalledTimes(2);
    });
});

describe('PinCodeInput keyboard, paste, and input contracts', () => {
    test('allows operating-system and boundary shortcuts without intercepting them', () => {
        const { bindings } = setup({ length: 4 });
        for (const [index, overrides] of [
            [1, { key: 'a', altKey: true }],
            [1, { key: 'F1' }],
            [1, { key: 'F12' }],
            [0, { key: 'Tab', shiftKey: true }],
            [3, { key: 'Tab' }],
            [1, { key: 'v', ctrlKey: true }],
            [1, { key: 'v', metaKey: true }],
            [1, { key: 'Paste' }]
        ] as Array<[number, Record<string, unknown>]>) {
            const event = keyboardEvent(overrides);
            bindings.onKeydown(index, event);
            expect(event.preventDefault).not.toHaveBeenCalled();
        }

        const longFunctionKey = keyboardEvent({ key: 'F123' });
        bindings.onKeydown(1, longFunctionKey);
        expect(longFunctionKey.preventDefault).toHaveBeenCalled();
    });

    test('confirms complete values and implements arrow, tab, home, and end navigation', () => {
        const inputs = Array.from({ length: 4 }, () => ({ focus: jest.fn(), select: jest.fn() }));
        mockPinCodeInputs.value = inputs;
        const { bindings, emit } = setup({ modelValue: '1234', length: 4 });

        const enter = keyboardEvent({ key: 'Enter', code: 'Enter' });
        bindings.onKeydown(2, enter);
        expect(emit).toHaveBeenCalledWith('pincode:confirm', '1234');
        expect(enter.preventDefault).toHaveBeenCalled();

        for (const [index, overrides, focusedIndex] of [
            [2, { key: 'ArrowLeft' }, 1],
            [2, { key: 'Tab', shiftKey: true }, 1],
            [1, { key: 'ArrowRight' }, 2],
            [1, { key: 'Tab' }, 2],
            [2, { key: 'Home' }, 0],
            [1, { key: 'End' }, 3]
        ] as Array<[number, Record<string, unknown>, number]>) {
            const event = keyboardEvent(overrides);
            bindings.onKeydown(index, event);
            expect(inputs[focusedIndex]?.focus).toHaveBeenCalled();
            expect(event.preventDefault).toHaveBeenCalled();
        }

        bindings.codes.value[3].value = '';
        const incompleteEnter = keyboardEvent({ key: 'Enter', code: 'Enter' });
        bindings.onKeydown(2, incompleteEnter);
        expect(incompleteEnter.preventDefault).toHaveBeenCalled();
    });

    test('clears trailing slots for every delete key and moves back only for Backspace code', () => {
        const inputs = Array.from({ length: 4 }, () => ({ focus: jest.fn(), select: jest.fn() }));
        mockPinCodeInputs.value = inputs;
        const { bindings } = setup({ modelValue: '1234', length: 4, secure: false });

        const backspace = keyboardEvent({ key: 'Backspace', code: 'Backspace' });
        bindings.onKeydown(2, backspace);
        expect(bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['1', '2', '', '']);
        expect(inputs[1]?.focus).toHaveBeenCalled();
        expect(backspace.preventDefault).toHaveBeenCalled();

        bindings.autoFillText(0, '9876');
        const deleteEvent = keyboardEvent({ key: 'Delete', code: 'Delete' });
        bindings.onKeydown(1, deleteEvent);
        expect(bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['9', '', '', '']);

        bindings.autoFillText(0, '5678');
        const delEvent = keyboardEvent({ key: 'Del', code: 'Delete' });
        bindings.onKeydown(3, delEvent);
        expect(bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['5', '6', '7', '']);
    });

    test('accepts western and localized digits, rejects other characters, and auto-confirms only when configured and complete', () => {
        const inputs = Array.from({ length: 3 }, () => ({ focus: jest.fn(), select: jest.fn() }));
        mockPinCodeInputs.value = inputs;
        const { bindings, emit, props } = setup({ length: 3, autoConfirm: true });

        const western = keyboardEvent({ key: '1', code: 'Digit1' });
        bindings.onKeydown(0, western);
        expect(bindings.codes.value[0].value).toBe('1');
        expect(inputs[1]?.focus).toHaveBeenCalled();

        const localized = keyboardEvent({ key: '٢', code: 'Digit2' });
        bindings.onKeydown(1, localized);
        expect(bindings.codes.value[1].value).toBe('2');

        const invalid = keyboardEvent({ key: 'x', code: 'KeyX' });
        bindings.onKeydown(2, invalid);
        expect(bindings.codes.value[2].value).toBe('');
        expect(invalid.preventDefault).toHaveBeenCalled();

        const completing = keyboardEvent({ key: '3', code: 'Digit3' });
        bindings.onKeydown(2, completing);
        expect(emit).toHaveBeenCalledWith('pincode:confirm', '123');

        props.autoConfirm = false;
        emit.mockClear();
        const replacement = keyboardEvent({ key: '4', code: 'Digit4' });
        bindings.onKeydown(2, replacement);
        expect(emit).not.toHaveBeenCalledWith('pincode:confirm', '124');

        const escape = keyboardEvent({ key: 'Escape', code: 'Escape' });
        bindings.onKeydown(1, escape);
        expect(escape.preventDefault).toHaveBeenCalled();

        mockCurrentNumeralSystem = mockWesternNumerals;
        const westernBindings = setup({ length: 1 }).bindings;
        expect(westernBindings.numeralSystem.value.isDigit('5')).toBe(true);
        expect(westernBindings.numeralSystem.value.isDigit('\u0662')).toBe(false);
    });

    test('handles absent, empty, valid, non-numeric, and overlong clipboard or input values', () => {
        const { bindings, emit } = setup({ length: 4 });

        const noClipboard = clipboardEvent(undefined, false);
        bindings.onPaste(0, noClipboard);
        expect(noClipboard.preventDefault).toHaveBeenCalled();

        const emptyClipboard = clipboardEvent('');
        bindings.onPaste(0, emptyClipboard);
        expect(emptyClipboard.preventDefault).toHaveBeenCalled();

        const validClipboard = clipboardEvent('12345');
        bindings.onPaste(0, validClipboard);
        expect(bindings.finalPinCode.value).toBe('1234');
        expect(emit).toHaveBeenCalledWith('pincode:confirm', '1234');
        expect(validClipboard.preventDefault).toHaveBeenCalled();

        const noTarget = inputEvent(undefined, false);
        bindings.onInput(0, noTarget);
        expect(noTarget.preventDefault).toHaveBeenCalled();

        const emptyTarget = inputEvent('');
        bindings.onInput(0, emptyTarget);
        expect(emptyTarget.preventDefault).toHaveBeenCalled();

        const invalidTarget = inputEvent('8x9');
        bindings.onInput(0, invalidTarget);
        expect(bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['8', '', '3', '4']);

        const validTarget = inputEvent('98765');
        bindings.onInput(0, validTarget);
        expect(bindings.finalPinCode.value).toBe('9876');
        expect(validTarget.preventDefault).toHaveBeenCalled();
    });
});

describe('PinCodeInput prop watchers and production template', () => {
    test('resets from model and length changes while avoiding a redundant equal-value reinitialization', async () => {
        const { bindings, emit, props } = setup({ modelValue: '12', length: 4, secure: true });
        bindings.codes.value[0].focused = true;
        bindings.autoFillText(0, '34');
        await flush();
        expect(emit).toHaveBeenCalledWith('update:modelValue', '34');

        props.modelValue = '34';
        await flush();
        expect(bindings.codes.value[0].focused).toBe(true);

        props.modelValue = '';
        await flush();
        expect(bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['', '', '', '']);
        expect(bindings.codes.value.every((code: any) => code.inputType === 'tel')).toBe(true);

        props.modelValue = '9876';
        await flush();
        props.length = 2;
        await flush();
        expect(bindings.codes.value.map((code: any) => code.value)).toStrictEqual(['9', '8']);
        expect(bindings.finalPinCode.value).toBe('98');
    });

    test('renders secure, disabled, autofocus, focus-state, and all native event bindings', async () => {
        const mounted = mountWithHostRenderer({
            modelValue: '12',
            length: 3,
            disabled: true,
            autofocus: true,
            autoConfirm: true,
            secure: true
        });
        try {
            await flush();
            let inputs = collectNodes(mounted.root, 'input');
            expect(inputs).toHaveLength(3);
            expect(inputs.map(input => input.props.type)).toStrictEqual(['password', 'password', 'tel']);
            expect(inputs.every(input => input.props.disabled === true)).toBe(true);
            expect(inputs[0]?.props.autofocus).toBe(true);
            expect(inputs[1]?.props.autofocus).toBeUndefined();
            expect(inputs[0]?.props.readonly).toBeUndefined();
            expect(inputs[0]?.props.error).toBeUndefined();
            expect(collectNodes(mounted.root, 'div')[0]?.props.style).toContain('repeat(3');

            inputs[0]?.props.onFocus();
            await flush();
            expect(mounted.state.codes[0].focused).toBe(true);
            inputs = collectNodes(mounted.root, 'input');
            expect(inputs[0]?.parent?.props.class).toContain('pin-code-input-focued');
            inputs[0]?.props.onBlur();
            await flush();
            expect(mounted.state.codes[0].focused).toBe(false);
        } finally {
            mounted.app.unmount();
        }

        const interactive = mountWithHostRenderer({
            modelValue: '',
            length: 2,
            disabled: false,
            autofocus: false,
            autoConfirm: false,
            secure: false
        });
        try {
            await flush();
            const inputs = collectNodes(interactive.root, 'input');
            expect(inputs.every(input => input.props.disabled === undefined)).toBe(true);
            expect(inputs.every(input => input.props.autofocus === undefined)).toBe(true);

            const keydown = keyboardEvent({ key: '1', code: 'Digit1' });
            inputs[0]?.props.onKeydown(keydown);
            const paste = clipboardEvent('2');
            inputs[1]?.props.onPaste(paste);
            const change = inputEvent('7');
            inputs[0]?.props.onChange(change);
            await flush();

            expect(keydown.preventDefault).toHaveBeenCalled();
            expect(paste.preventDefault).toHaveBeenCalled();
            expect(change.preventDefault).toHaveBeenCalled();
            expect(interactive.state.finalPinCode).toBe('72');
        } finally {
            interactive.app.unmount();
        }
    });
});
