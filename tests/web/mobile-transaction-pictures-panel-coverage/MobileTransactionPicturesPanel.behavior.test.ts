/* eslint-disable @typescript-eslint/no-explicit-any */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

jest.mock('@/views/base/transactions/TransactionEditPageBase.ts', () => ({
    TransactionEditPageMode: { Add: 'add', Edit: 'edit', View: 'view' }
}));

import MobileTransactionPicturesPanel from '@/views/mobile/transactions/components/MobileTransactionPicturesPanel.vue';
import type { TransactionPictureInfoBasicResponse } from '@/models/transaction_picture_info.ts';

const actualVue = jest.requireActual('vue') as any;
const TransactionEditPageMode = { Add: 'add', Edit: 'edit', View: 'view' } as const;
const pictures: TransactionPictureInfoBasicResponse[] = [
    { pictureId: 'picture-1', originalUrl: '/picture-1.jpg' },
    { pictureId: 'picture-2', originalUrl: '/picture-2.jpg' }
];
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

interface Runtime {
    readonly props: Record<string, any>;
    readonly bindings: Record<string, any>;
    readonly emit: jest.Mock;
}

function setupPanel(overrides: Record<string, unknown> = {}): Runtime {
    const props = actualVue.reactive({
        show: true,
        pictures,
        mode: TransactionEditPageMode.Add,
        submitting: false,
        uploadingPicture: false,
        recognizingPicture: false,
        removingPictureId: null,
        canAddTransactionPicture: true,
        tt: (key: string) => `translated:${key}`,
        getTransactionPictureUrl: (picture?: TransactionPictureInfoBasicResponse | null) => picture?.originalUrl,
        ...overrides
    });
    const emit = jest.fn();
    const bindings = (MobileTransactionPicturesPanel as any).setup(props, {
        attrs: {},
        slots: {},
        emit,
        expose: jest.fn()
    });
    return { props, bindings, emit };
}

function renderPanel(runtime: Runtime): any {
    return (MobileTransactionPicturesPanel as any).render(
        {}, [], runtime.props, actualVue.proxyRefs(runtime.bindings), {}, {}
    );
}

function collectNodes(node: any, nodes: any[] = []): any[] {
    if (!node) return nodes;
    if (Array.isArray(node)) {
        for (const child of node) collectNodes(child, nodes);
        return nodes;
    }
    if (typeof node !== 'object') return nodes;

    nodes.push(node);
    if (Array.isArray(node.children)) {
        collectNodes(node.children, nodes);
    } else if (node.children && typeof node.children === 'object') {
        for (const slot of Object.values(node.children)) {
            if (typeof slot === 'function') {
                collectNodes(slot(), nodes);
            }
        }
    }
    return nodes;
}

function collectClickHandlers(node: any): Array<() => void> {
    return collectNodes(node)
        .flatMap(candidate => Array.isArray(candidate.props?.onClick)
            ? candidate.props.onClick
            : [candidate.props?.onClick])
        .filter((handler): handler is () => void => typeof handler === 'function');
}

beforeAll(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    consoleWarnSpy.mockRestore();
});

beforeEach(() => {
    jest.clearAllMocks();
});

describe('MobileTransactionPicturesPanel behavior', () => {
    test('declares the Framework7 list-item contract and emits picture actions', () => {
        expect(MobileTransactionPicturesPanel.name).toBe('f7-list-item-mobile-transaction-pictures-panel');

        const runtime = setupPanel();
        const rendered = renderPanel(runtime);
        const nodes = collectNodes(rendered);
        expect(nodes.filter(node => node.props?.src)).toHaveLength(2);
        expect(nodes.some(node => node.props?.f7 === 'trash')).toBe(true);
        expect(nodes.some(node => node.props?.f7 === 'plus')).toBe(true);

        const clickHandlers = collectClickHandlers(rendered);
        expect(clickHandlers).toHaveLength(3);
        clickHandlers[0]?.();
        clickHandlers[clickHandlers.length - 1]?.();
        expect(runtime.emit).toHaveBeenNthCalledWith(1, 'viewOrRemovePicture', pictures[0]);
        expect(runtime.emit).toHaveBeenNthCalledWith(2, 'openPictureDialog');
    });

    test('renders removal, upload, recognition, view, and hidden states', () => {
        const removingNodes = collectNodes(renderPanel(setupPanel({ removingPictureId: 'picture-1' })));
        expect(removingNodes.some(node => node.props?.f7 === 'trash')).toBe(true);
        expect(removingNodes.some(node => node.props?.size === 28)).toBe(true);

        const uploadingNodes = collectNodes(renderPanel(setupPanel({ uploadingPicture: true })));
        expect(uploadingNodes.some(node => node.props?.f7 === 'plus')).toBe(false);
        expect(uploadingNodes.some(node => node.props?.size === 28)).toBe(true);

        const recognizingNodes = collectNodes(renderPanel(setupPanel({ recognizingPicture: true, submitting: true })));
        expect(recognizingNodes.some(node => node.props?.f7 === 'plus')).toBe(false);
        expect(recognizingNodes.some(node => node.props?.size === 28)).toBe(true);

        const readonlyNodes = collectNodes(renderPanel(setupPanel({
            mode: TransactionEditPageMode.View,
            canAddTransactionPicture: false
        })));
        expect(readonlyNodes.some(node => node.props?.class?.includes?.('transaction-picture-control-backdrop'))).toBe(false);
        expect(collectClickHandlers(renderPanel(setupPanel({
            mode: TransactionEditPageMode.View,
            canAddTransactionPicture: false
        })))).toHaveLength(2);

        const hidden = renderPanel(setupPanel({ show: false }));
        expect(hidden.type).toBe(actualVue.Comment);
    });
});
