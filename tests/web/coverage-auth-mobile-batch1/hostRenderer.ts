import { jest } from '@jest/globals';

export interface HostNode {
    type: string;
    text: string;
    children: HostNode[];
    parent: HostNode | null;
    props: Record<string, unknown>;
    style: Record<string, unknown>;
    click?: jest.Mock;
}

function createHostNode(type: string, text = ''): HostNode {
    const node: HostNode = {
        type,
        text,
        children: [],
        parent: null,
        props: {},
        style: {}
    };

    if (type === 'input') {
        node.click = jest.fn();
    }

    return node;
}

export function mountWithHostRenderer(
    component: any,
    props: Record<string, unknown> = {},
    componentNames: string[] = []
): { app: any; root: HostNode; state: any } {
    const { createRenderer, defineComponent, h } = jest.requireActual('vue') as any;
    const renderer = createRenderer({
        patchProp(node: HostNode, key: string, _previous: unknown, value: unknown) {
            node.props[key] = value;
        },
        insert(child: HostNode, parent: HostNode, anchor: HostNode | null = null) {
            child.parent = parent;
            if (!anchor) {
                parent.children.push(child);
                return;
            }
            const index = parent.children.indexOf(anchor);
            parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
        },
        remove(child: HostNode) {
            const index = child.parent?.children.indexOf(child) ?? -1;
            if (index >= 0) child.parent?.children.splice(index, 1);
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
        setText(node: HostNode, text: string) {
            node.text = text;
        },
        setElementText(node: HostNode, text: string) {
            node.text = text;
            node.children = [];
        },
        parentNode(node: HostNode) {
            return node.parent;
        },
        nextSibling(node: HostNode) {
            const siblings = node.parent?.children ?? [];
            return siblings[siblings.indexOf(node) + 1] ?? null;
        },
        querySelector() {
            return null;
        },
        setScopeId(node: HostNode, scopeId: string) {
            node.props[scopeId] = '';
        },
        cloneNode(node: HostNode) {
            return {
                ...node,
                children: [...node.children],
                props: { ...node.props },
                parent: null
            };
        },
        insertStaticContent(content: string, parent: HostNode, anchor: HostNode | null) {
            const node = createHostNode('#static', content);
            node.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, node);
            return [node, node];
        }
    });

    const SlotHost = defineComponent({
        name: 'CoverageSlotHost',
        inheritAttrs: false,
        setup: (_slotProps: unknown, { attrs, slots }: any) => () => h(
            'stub',
            attrs,
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            })
        )
    });

    const app = renderer.createApp(component, props);
    app.config.warnHandler = () => undefined;
    for (const name of componentNames) app.component(name, SlotHost);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState };
}

export function collectHostCallbacks(
    node: HostNode,
    callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [],
    seen = new Set<HostNode>()
): Array<{ name: string; callback: (...args: any[]) => unknown }> {
    if (!node || seen.has(node)) return callbacks;
    seen.add(node);

    for (const [name, value] of Object.entries(node.props)) {
        if (!name.startsWith('on')) continue;
        for (const candidate of Array.isArray(value) ? value : [value]) {
            if (typeof candidate === 'function') {
                callbacks.push({ name, callback: candidate as (...args: any[]) => unknown });
            }
        }
    }

    for (const child of node.children) collectHostCallbacks(child, callbacks, seen);
    return callbacks;
}
