/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));

const ImportPreviewSignalCell = require(
    '@/views/desktop/transactions/import/tabs/ImportPreviewSignalCell.vue'
).default as any;
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

type SignalStatus = 'pending' | 'accepted' | 'rejected' | 'skipped';

function reviewSignal(
    status: SignalStatus,
    overrides: Record<string, unknown> = {}
): Record<string, unknown> {
    return {
        status,
        labelKey: `${status} label`,
        title: `${status} title`,
        color: 'info',
        actions: [
            { decision: 'accept', labelKey: 'Accept', color: 'success' },
            { decision: 'reject', labelKey: 'Reject', color: 'error' }
        ],
        detailLines: [`${status} detail`],
        ...overrides
    };
}

function fullViewModel(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        parser: {
            parserId: 'alipay',
            label: 'Alipay',
            color: 'primary',
            title: 'Parser title',
            detailLines: ['parser detail']
        },
        dedup: {
            dedupType: 'platform_bank',
            labelKey: 'Platform duplicate',
            label: 'Duplicate',
            title: 'Dedup title',
            color: 'warning',
            sourceCount: 2,
            detailLines: ['dedup detail']
        },
        isManuallyAnnotated: false,
        historyRewrite: reviewSignal('pending', {
            historyBillId: 42,
            actions: []
        }),
        transferSuggestion: reviewSignal('accepted'),
        investment: null,
        learning: reviewSignal('pending'),
        llm: reviewSignal('rejected', {
            actions: [
                { decision: 'accept', labelKey: 'Accept', color: 'success' },
                { decision: 'reject', labelKey: 'Reject', color: 'error' },
                { decision: 'clear', labelKey: 'Clear', color: 'warning' }
            ]
        }),
        recurring: {
            hasMatch: true,
            title: 'Monthly rent',
            candidateCount: 2,
            primaryReason: 'same merchant'
        },
        hasAnySignal: true,
        ...overrides
    };
}

function setup(props: Record<string, unknown>): {
    bindings: any;
    emit: jest.Mock;
    props: Record<string, unknown>;
} {
    const reactiveProps = actualVue.reactive(props);
    const emit = jest.fn();
    const bindings = ImportPreviewSignalCell.setup(reactiveProps, {
        attrs: {},
        slots: {},
        emit,
        expose: jest.fn()
    });
    return { bindings, emit, props: reactiveProps };
}

function render(runtime: ReturnType<typeof setup>): unknown {
    return ImportPreviewSignalCell.render(
        {},
        [],
        runtime.props,
        actualVue.proxyRefs(runtime.bindings),
        {},
        {}
    );
}

function collectVNodes(node: unknown, output: any[] = [], seen = new Set<unknown>()): any[] {
    if (node === null || node === undefined || typeof node === 'boolean') return output;
    if (Array.isArray(node)) {
        for (const child of node) collectVNodes(child, output, seen);
        return output;
    }
    if (typeof node !== 'object' || seen.has(node)) return output;
    seen.add(node);
    const vnode = node as any;
    output.push(vnode);
    collectVNodes(vnode.children, output, seen);
    if (vnode.children && typeof vnode.children === 'object' && !Array.isArray(vnode.children)) {
        for (const slot of Object.values(vnode.children)) {
            if (typeof slot !== 'function') continue;
            try {
                collectVNodes(slot({ props: { role: 'menu-activator' } }), output, seen);
            } catch {
                // Vue-owned slots may reject a generic slot payload; other slots remain testable.
            }
        }
    }
    return output;
}

function invokeClicks(nodes: any[]): void {
    for (const node of nodes) {
        const candidates = Array.isArray(node.props?.onClick)
            ? node.props.onClick
            : [node.props?.onClick];
        for (const candidate of candidates) {
            if (typeof candidate === 'function') {
                candidate({ stopPropagation: jest.fn() });
            }
        }
    }
}

function hostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {} };
}

function mountDetailedHistory(): {
    app: any;
    historyListener: jest.Mock;
    root: any;
} {
    const renderer = actualVue.createRenderer({
        patchProp(node: any, key: string, _previous: unknown, value: unknown) {
            node.props[key] = value;
        },
        insert(child: any, parent: any, anchor: any = null) {
            child.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
        },
        remove(child: any) {
            const index = child.parent?.children.indexOf(child) ?? -1;
            if (index >= 0) child.parent.children.splice(index, 1);
        },
        createElement(type: string) {
            return hostNode(type);
        },
        createText(text: string) {
            return hostNode('#text', text);
        },
        createComment(text: string) {
            return hostNode('#comment', text);
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
        setScopeId() {},
        cloneNode(node: any) {
            return { ...node, children: [...node.children], props: { ...node.props }, parent: null };
        },
        insertStaticContent(content: string, parent: any, anchor: any) {
            const node = hostNode('#static', content);
            node.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, node);
            return [node, node];
        }
    });
    const Stub = actualVue.defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => actualVue.h(
            'stub',
            attrs,
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({ props: { role: 'menu-activator' } }) ?? [];
                } catch {
                    return [];
                }
            })
        )
    });
    const historyListener = jest.fn();
    const app = renderer.createApp(ImportPreviewSignalCell, {
        viewModel: fullViewModel({
            parser: null,
            dedup: null,
            historyRewrite: reviewSignal('pending', { historyBillId: 42 }),
            transferSuggestion: null,
            learning: null,
            llm: null,
            recurring: null
        }),
        onOpenHistoryDetail: historyListener
    });
    for (const name of ['v-menu', 'v-chip', 'v-card', 'v-card-text', 'v-btn', 'v-progress-circular']) {
        app.component(name, Stub);
    }
    app.config.warnHandler = () => undefined;
    const root = hostNode('root');
    app.mount(root);
    return { app, historyListener, root };
}

function flattenHost(node: any): any[] {
    return [node, ...(node.children || []).flatMap(flattenHost)];
}

beforeEach(() => {
    jest.clearAllMocks();
});

beforeAll(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    consoleWarnSpy.mockRestore();
});

describe('ImportPreviewSignalCell production-loaded behavior', () => {
    test('renders every detailed signal and dispatches all actionable events', () => {
        const runtime = setup({
            viewModel: fullViewModel(),
            disabled: false,
            hasSession: true,
            rowBusy: false,
            transferBusy: false,
            learningBusy: false,
            llmBusy: false
        });

        const nodes = collectVNodes(render(runtime));
        invokeClicks(nodes);

        const historyRuntime = setup({
            viewModel: fullViewModel({
                parser: null,
                dedup: null,
                historyRewrite: reviewSignal('pending', {
                    historyBillId: 42,
                    detailLines: [],
                    actions: []
                }),
                transferSuggestion: null,
                learning: null,
                llm: null,
                recurring: null
            })
        });
        invokeClicks(collectVNodes(render(historyRuntime)));

        expect(runtime.emit.mock.calls).toEqual(expect.arrayContaining([
            ['reviewTransfer', 'accept'],
            ['reviewTransfer', 'reject'],
            ['reviewLearning', 'accept'],
            ['reviewLearning', 'reject'],
            ['reviewLlm', 'accept'],
            ['reviewLlm', 'reject'],
            ['openRecurring'],
            ['clearRecurring']
        ]));
        expect(historyRuntime.emit).toHaveBeenCalledWith('openHistoryDetail', 42);
        const mountedHistory = mountDetailedHistory();
        const detailedHistoryChip = flattenHost(mountedHistory.root).find(node => (
            node.props?.title === 'pending title' && typeof node.props?.onClick === 'function'
        ));
        expect(detailedHistoryChip).toBeDefined();
        detailedHistoryChip.props.onClick({ stopPropagation: jest.fn() });
        expect(mountedHistory.historyListener).toHaveBeenCalledWith(42);
        mountedHistory.app.unmount();
        expect(runtime.emit).not.toHaveBeenCalledWith('reviewLlm', 'clear');
        expect(nodes.some(node => String(node.props?.class || '').includes('signal-action-row--split')))
            .toBe(true);
    });

    test('renders detail-free chips, single actions, absent history id, and unmatched recurring state', () => {
        const singleAction = [{ decision: 'clear', labelKey: 'Clear', color: 'warning' }];
        const runtime = setup({
            viewModel: fullViewModel({
                parser: {
                    parserId: 'wechat', label: 'WeChat', color: 'success', title: '', detailLines: []
                },
                dedup: {
                    dedupType: 'same_batch', labelKey: 'Duplicate', label: 'Duplicate',
                    title: '', color: 'warning', sourceCount: 1, detailLines: []
                },
                historyRewrite: reviewSignal('skipped', { historyBillId: undefined, detailLines: [] }),
                transferSuggestion: reviewSignal('rejected', { actions: singleAction, detailLines: [] }),
                learning: reviewSignal('accepted', { actions: singleAction, detailLines: [] }),
                llm: reviewSignal('skipped', { actions: singleAction, detailLines: [] }),
                recurring: {
                    hasMatch: false,
                    title: '',
                    candidateCount: 0,
                    primaryReason: ''
                }
            }),
            disabled: true,
            hasSession: false,
            rowBusy: true,
            transferBusy: true,
            learningBusy: true,
            llmBusy: true
        });

        const nodes = collectVNodes(render(runtime));
        invokeClicks(nodes);

        expect(runtime.emit).not.toHaveBeenCalledWith('openHistoryDetail', expect.anything());
        expect(runtime.emit).not.toHaveBeenCalledWith('reviewLlm', expect.anything());
        expect(nodes.some(node => String(node.props?.class || '').includes('signal-action-row--single')))
            .toBe(true);
    });

    test('covers empty root, optional details, status icons, and direct LLM guards', () => {
        expect(render(setup({ viewModel: undefined }))).toBeDefined();
        expect(render(setup({ viewModel: fullViewModel({
            parser: null,
            dedup: null,
            historyRewrite: null,
            transferSuggestion: null,
            learning: null,
            llm: null,
            recurring: null,
            hasAnySignal: false
        }) }))).toBeDefined();

        const runtime = setup({ viewModel: fullViewModel() });
        expect(runtime.bindings.hasSignalDetails(undefined)).toBe(false);
        expect(runtime.bindings.hasSignalDetails([])).toBe(false);
        expect(runtime.bindings.hasSignalDetails(['detail'])).toBe(true);
        expect(runtime.bindings.getActionRowClass(0)).toContain('signal-action-row--single');
        expect(runtime.bindings.getActionRowClass(1)).toContain('signal-action-row--single');
        expect(runtime.bindings.getActionRowClass(2)).toContain('signal-action-row--split');

        const accepted = runtime.bindings.getStatusIcon('accepted');
        const rejected = runtime.bindings.getStatusIcon('rejected');
        const pending = runtime.bindings.getStatusIcon('pending');
        expect(new Set([accepted, rejected, pending]).size).toBe(3);
        expect(runtime.bindings.getStatusIcon('skipped')).toBe(pending);
        expect(runtime.bindings.getLearningIcon('pending')).not.toBe(pending);
        expect(runtime.bindings.getLearningIcon('accepted')).toBe(accepted);
        expect(runtime.bindings.getLearningIcon('rejected')).toBe(rejected);

        runtime.bindings.emitLLMReview('clear');
        expect(runtime.emit).not.toHaveBeenCalled();
        runtime.bindings.emitLLMReview('accept');
        runtime.bindings.emitLLMReview('reject');
        expect(runtime.emit.mock.calls).toEqual([
            ['reviewLlm', 'accept'],
            ['reviewLlm', 'reject']
        ]);
    });
});
