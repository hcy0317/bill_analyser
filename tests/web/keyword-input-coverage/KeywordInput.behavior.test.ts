import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockTt = jest.fn<(key: string) => string>();

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => mockTt(key) }),
}));

const KeywordInput = require('@/components/common/KeywordInput.vue').default as any;
const keywordExpression = require('@/components/common/keywordExpression.ts') as any;

interface SetupResult {
    bindings: any;
    emitted: Array<[string, ...unknown[]]>;
    exposed: Record<string, unknown>;
    props: any;
}

function setupInput(overrides: Record<string, unknown> = {}): SetupResult {
    const props = actualVue.reactive({
        modelValue: '',
        disabled: false,
        regexEnabled: false,
        expressionFormat: undefined,
        format: 'composite',
        title: undefined,
        emptyStateText: undefined,
        helpText: undefined,
        exampleText: undefined,
        addButtonText: undefined,
        showHeader: true,
        ...overrides,
    });
    const emitted: Array<[string, ...unknown[]]> = [];
    const exposed: Record<string, unknown> = {};
    const bindings = KeywordInput.setup(props, {
        emit: (event: string, ...args: unknown[]) => emitted.push([event, ...args]),
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value),
    });
    return { bindings, emitted, exposed, props };
}

function clause(overrides: Record<string, unknown> = {}): any {
    return keywordExpression.createRuleClause({
        id: 'clause-default',
        joiner: 'AND',
        negated: false,
        operator: 'OR',
        terms: ['term'],
        openParens: 0,
        closeParens: 0,
        startsExpression: false,
        ...overrides,
    });
}

async function flush(): Promise<void> {
    await Promise.resolve();
    await actualVue.nextTick();
    await Promise.resolve();
}

type CapturedCallback = { name: string; callback: (...args: any[]) => unknown };

function createSsrStub(name: string): any {
    return actualVue.defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => actualVue.h(
            'div',
            { ...attrs, 'data-stub': name },
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            }),
        ),
    });
}

async function renderSsrInput(
    props: Record<string, unknown>,
    mutate: (bindings: any) => void = () => undefined,
): Promise<{ bindings: any; vnode: any }> {
    const { createSSRApp } = actualVue;
    const { renderToString } = require('vue/server-renderer') as any;
    let bindings: any;
    let vnode: any;
    const RuntimeInput = {
        ...KeywordInput,
        setup(runtimeProps: any, context: any) {
            bindings = KeywordInput.setup(runtimeProps, context);
            mutate(bindings);
            return bindings;
        },
        render(...args: any[]) {
            vnode = KeywordInput.render.apply(this, args);
            return vnode;
        },
    };
    const app = createSSRApp(RuntimeInput, { modelValue: '', ...props });
    for (const componentName of [
        'v-spacer', 'v-btn', 'v-alert', 'v-textarea', 'v-select', 'v-combobox',
    ]) {
        app.component(componentName, createSsrStub(`KeywordInput${componentName}`));
    }
    app.config.warnHandler = () => undefined;
    await renderToString(app);
    return { bindings, vnode };
}

function exerciseRenderTree(
    value: any,
    callbacks: CapturedCallback[],
    seen = new Set<any>(),
): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) return;
    seen.add(value);

    if (Array.isArray(value)) {
        for (const item of value) exerciseRenderTree(item, callbacks, seen);
        return;
    }

    if (value.props && typeof value.props === 'object') {
        for (const [name, prop] of Object.entries(value.props)) {
            for (const candidate of (Array.isArray(prop) ? prop : [prop])) {
                if (name.startsWith('on') && typeof candidate === 'function') {
                    callbacks.push({ name, callback: candidate as (...args: any[]) => unknown });
                }
            }
        }
    }

    if (value.children && typeof value.children === 'object' && !Array.isArray(value.children)) {
        for (const child of Object.values(value.children)) {
            if (typeof child === 'function') {
                try {
                    exerciseRenderTree((child as (...args: any[]) => unknown)({}), callbacks, seen);
                } catch {
                    // Slots use heterogeneous payloads; incompatible probes are ignored.
                }
            } else {
                exerciseRenderTree(child, callbacks, seen);
            }
        }
    } else {
        exerciseRenderTree(value.children, callbacks, seen);
    }

    exerciseRenderTree(value.dynamicChildren, callbacks, seen);
    exerciseRenderTree(value.ssContent, callbacks, seen);
    exerciseRenderTree(value.ssFallback, callbacks, seen);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTt.mockImplementation(key => `translated:${key}`);
});

describe('KeywordInput production-loaded defaults and parsing', () => {
    test('resolves default and custom copy, regex labels, format precedence, and exposed actions', () => {
        const { bindings, exposed, props } = setupInput();

        expect(bindings.resolvedFormat.value).toBe('composite');
        expect(bindings.resolvedTitle.value).toBe('translated:Rule Matching Expression');
        expect(bindings.resolvedAddButtonText.value).toBe('translated:Add Matching Expression');
        expect(bindings.resolvedEmptyStateText.value).toBe('translated:No rule clauses yet');
        expect(bindings.resolvedHelpText.value).toBe('');
        expect(bindings.resolvedExampleText.value).toBe('');
        expect(bindings.termsLabel.value).toBe('translated:Matching Expression Terms');
        expect(bindings.termsPlaceholder.value).toBe('translated:Enter terms and press Enter');
        expect(bindings.supportsCompositeGrouping.value).toBe(true);
        expect(bindings.types.value.map((item: any) => item.value)).toStrictEqual(['OR', 'AND', 'NOT']);
        expect(exposed).toEqual({ addExpression: bindings.addExpression, canAddExpression: bindings.canAddExpression });

        props.title = 'Custom title';
        props.addButtonText = 'Append';
        props.emptyStateText = 'Nothing here';
        props.helpText = 'Help';
        props.exampleText = 'Example';
        props.regexEnabled = true;
        props.expressionFormat = 'legacy';
        expect(bindings.resolvedTitle.value).toBe('Custom title');
        expect(bindings.resolvedAddButtonText.value).toBe('Append');
        expect(bindings.resolvedEmptyStateText.value).toBe('Nothing here');
        expect(bindings.resolvedHelpText.value).toBe('Help');
        expect(bindings.resolvedExampleText.value).toBe('Example');
        expect(bindings.termsLabel.value).toBe('translated:Regex Patterns');
        expect(bindings.termsPlaceholder.value).toBe('translated:Enter regex patterns and press Enter');
        expect(bindings.resolvedFormat.value).toBe('legacy');
        expect(bindings.supportsCompositeGrouping.value).toBe(false);

        props.title = '';
        props.addButtonText = '';
        props.emptyStateText = '';
        props.helpText = null;
        props.exampleText = null;
        expect(bindings.resolvedTitle.value).toBe('translated:Rule Matching Expression');
        expect(bindings.resolvedAddButtonText.value).toBe('translated:Add Matching Expression');
        expect(bindings.resolvedEmptyStateText.value).toBe('translated:No rule clauses yet');
        expect(bindings.resolvedHelpText.value).toBe('');
        expect(bindings.resolvedExampleText.value).toBe('');
    });

    test('normalizes composite input, repairs serialization, groups expressions, and short-circuits equal watches', async () => {
        const { bindings, emitted, props } = setupInput({
            modelValue: '  OR={ coffee ,coffee, tea }  ',
        });

        expect(bindings.clauses.value).toMatchObject([
            {
                id: 'rule-clause-local-1',
                joiner: 'AND',
                negated: false,
                startsExpression: false,
                operator: 'OR',
                terms: ['coffee', 'tea'],
            },
        ]);
        expect(bindings.draftTerms.value).toEqual({ 'rule-clause-local-1': '' });
        expect(bindings.getCurrentSerializedExpression()).toBe('OR={coffee,tea}');
        expect(emitted).toContainEqual(['update:modelValue', 'OR={coffee,tea}']);

        bindings.parseModelValue('OR={same}');
        const emitCountBeforeEqualWatch = emitted.length;
        props.modelValue = 'OR={same}';
        await flush();
        expect(emitted).toHaveLength(emitCountBeforeEqualWatch);
        expect(bindings.clauses.value[0].terms).toStrictEqual(['same']);

        props.modelValue = 'OR={breakfast}|AND={coffee}';
        await flush();
        expect(bindings.expressionGroups.value).toHaveLength(2);
        expect(bindings.expressionGroups.value.map((group: any) => group.clauses.length)).toStrictEqual([1, 1]);
        expect(bindings.expressionGroups.value[0].id).toContain('expression-rule-clause-local-');

        props.expressionFormat = undefined;
        props.format = 'composite';
        await flush();
        expect(bindings.resolvedFormat.value).toBe('composite');
    });

    test('preserves unsupported input as read-only raw text and resumes parsing after prop changes', async () => {
        const { bindings, emitted, props } = setupInput({ modelValue: 'unsupported syntax' });

        expect(bindings.clauses.value).toStrictEqual([]);
        expect(bindings.rawExpression.value).toBe('unsupported syntax');
        expect(bindings.parseErrorKey.value).toBe(
            keywordExpression.RULE_EXPRESSION_UNPARSEABLE_KEY
        );
        expect(bindings.isRawMode.value).toBe(true);
        expect(bindings.canAddExpression()).toBe(false);
        expect(bindings.getCurrentSerializedExpression()).toBe('');

        const emitCount = emitted.length;
        bindings.parseModelValue('another unsupported value');
        props.modelValue = 'another unsupported value';
        await flush();
        expect(bindings.rawExpression.value).toBe('another unsupported value');
        expect(emitted).toHaveLength(emitCount);

        props.modelValue = 'AND={safe}';
        await flush();
        expect(bindings.rawExpression.value).toBe('');
        expect(bindings.parseErrorKey.value).toBe('');
        expect(bindings.clauses.value[0].terms).toStrictEqual(['safe']);
        expect(bindings.canAddExpression()).toBe(true);

        bindings.parseModelValue('');
        expect(bindings.clauses.value).toStrictEqual([]);
        expect(bindings.validationErrorKey.value).toBe('');
    });
});

describe('KeywordInput production-loaded editing behavior', () => {
    test('adds expression groups and inserts rows while keeping draft state synchronized', () => {
        const { bindings, emitted } = setupInput();

        bindings.addExpression();
        expect(bindings.clauses.value).toMatchObject([
            { joiner: 'AND', operator: 'OR', startsExpression: false, terms: [] },
        ]);
        const firstId = bindings.clauses.value[0].id;
        bindings.draftTerms.value[firstId] = 'pending input';
        bindings.draftTerms.value.stale = 'remove me';
        bindings.syncDraftTerms();
        expect(bindings.draftTerms.value).toEqual({ [firstId]: 'pending input' });

        bindings.addExpression();
        expect(bindings.clauses.value[1]).toMatchObject({
            joiner: 'OR',
            operator: 'OR',
            startsExpression: true,
        });
        const secondId = bindings.clauses.value[1].id;
        bindings.insertClauseAfterRow(bindings.clauses.value[0]);
        expect(bindings.clauses.value.map((item: any) => item.id)).toStrictEqual([
            firstId,
            'rule-clause-local-3',
            secondId,
        ]);
        expect(bindings.clauses.value[1]).toMatchObject({ joiner: 'AND', operator: 'OR' });
        expect(emitted.some(item => item[0] === 'update:modelValue')).toBe(true);
    });

    test('normalizes connector choices and replaces only existing clauses', () => {
        const { bindings } = setupInput({ modelValue: 'OR={one}+AND={two}' });
        const [first, second] = bindings.clauses.value;

        expect(bindings.getClauseConnector(first)).toBe('AND');
        second.joiner = 'OR';
        second.negated = false;
        expect(bindings.getClauseConnector(second)).toBe('OR');
        second.negated = true;
        expect(bindings.getClauseConnector(second)).toBe('NOT');

        bindings.updateClauseConnector(second, 'OR');
        expect(bindings.clauses.value[1]).toMatchObject({ joiner: 'OR', negated: false, startsExpression: false });
        bindings.updateClauseConnector(bindings.clauses.value[1], 'NOT');
        expect(bindings.clauses.value[1]).toMatchObject({ joiner: 'AND', negated: true });
        bindings.updateClauseConnector(bindings.clauses.value[1], 'unexpected');
        expect(bindings.clauses.value[1]).toMatchObject({ joiner: 'AND', negated: false });

        const before = [...bindings.clauses.value];
        bindings.replaceClause(clause({ id: 'missing', terms: ['ignored'] }));
        expect(bindings.clauses.value).toStrictEqual(before);
        bindings.replaceClause({ ...bindings.clauses.value[0], terms: ['replaced'] });
        expect(bindings.clauses.value[0].terms).toStrictEqual(['replaced']);
    });

    test('removes missing, only, first, middle, and last clauses without corrupting following joiners', () => {
        const { bindings } = setupInput();

        bindings.clauses.value = [clause({ id: 'only' })];
        bindings.removeClause('missing');
        expect(bindings.clauses.value).toHaveLength(1);
        bindings.removeClause('only');
        expect(bindings.clauses.value).toStrictEqual([]);

        bindings.clauses.value = [
            clause({ id: 'first' }),
            clause({ id: 'second', joiner: 'OR', negated: true, startsExpression: true }),
        ];
        bindings.removeClause('first');
        expect(bindings.clauses.value[0]).toMatchObject({
            id: 'second', joiner: 'AND', negated: false, startsExpression: false,
        });

        bindings.clauses.value = [
            clause({ id: 'head' }),
            clause({ id: 'removed', joiner: 'OR', negated: true, startsExpression: true }),
            clause({ id: 'following', joiner: 'AND', negated: false, startsExpression: false }),
        ];
        bindings.removeClause('removed');
        expect(bindings.clauses.value[1]).toMatchObject({
            id: 'following', joiner: 'OR', negated: true, startsExpression: true,
        });

        bindings.clauses.value = [
            clause({ id: 'head' }),
            clause({ id: 'normal', joiner: 'AND', negated: false }),
            clause({ id: 'tail', joiner: 'AND', negated: false }),
        ];
        bindings.removeClause('normal');
        expect(bindings.clauses.value[1]).toMatchObject({ id: 'tail', joiner: 'AND', negated: false });
        bindings.removeClause('tail');
        expect(bindings.clauses.value.map((item: any) => item.id)).toStrictEqual(['head']);
    });

    test('normalizes chip input and escapes expression syntax without interpreting markup', () => {
        const { bindings, emitted } = setupInput({ modelValue: 'OR={seed}' });
        const target = bindings.clauses.value[0];
        target.terms = [' <script>alert(1)</script> ', 'a,b', 'a,b', '', '  '];

        bindings.onTermsUpdated(target);
        expect(target.terms).toStrictEqual(['<script>alert(1)</script>', 'a,b']);
        expect(emitted.at(-1)).toStrictEqual([
            'update:modelValue',
            'OR={<script>alert\\(1\\)<\\/script>,a\\,b}',
        ]);
        expect(KeywordInput.render.toString()).not.toContain('v-html');
    });

    test('blocks raw and unbalanced serialization, then emits once the expression is valid', () => {
        const { bindings, emitted } = setupInput();

        bindings.rawExpression.value = 'raw preserved';
        bindings.serializeKeywords();
        expect(emitted).toStrictEqual([]);

        bindings.rawExpression.value = '';
        bindings.clauses.value = [clause({ id: 'unbalanced', openParens: 1, closeParens: 0 })];
        expect(bindings.getCurrentSerializedExpression()).toBeNull();
        bindings.serializeKeywords();
        expect(bindings.validationErrorKey.value).toBe(keywordExpression.RULE_EXPRESSION_UNBALANCED_KEY);
        expect(emitted).toStrictEqual([]);

        bindings.clauses.value = [clause({ id: 'balanced', terms: ['safe'] })];
        bindings.serializeKeywords();
        expect(bindings.validationErrorKey.value).toBe('');
        expect(emitted).toStrictEqual([['update:modelValue', 'OR={safe}']]);
    });

    test('normalizes first-clause invariants and edits bounded parenthesis counters', () => {
        const { bindings } = setupInput();

        bindings.normalizeClauseJoiners();
        bindings.clauses.value = [clause({
            id: 'paren', joiner: 'OR', negated: true, startsExpression: true, openParens: 0, closeParens: 0,
        })];
        bindings.normalizeClauseJoiners();
        expect(bindings.clauses.value[0]).toMatchObject({ joiner: 'AND', negated: false, startsExpression: false });

        let target = bindings.clauses.value[0];
        bindings.incrementOpenParen(target);
        target = bindings.clauses.value[0];
        expect(target.openParens).toBe(1);
        bindings.decrementOpenParen(target);
        target = bindings.clauses.value[0];
        expect(target.openParens).toBe(0);
        bindings.decrementOpenParen(target);
        expect(bindings.clauses.value[0].openParens).toBe(0);

        target = bindings.clauses.value[0];
        bindings.incrementCloseParen(target);
        target = bindings.clauses.value[0];
        expect(target.closeParens).toBe(1);
        bindings.decrementCloseParen(target);
        target = bindings.clauses.value[0];
        expect(target.closeParens).toBe(0);
        bindings.decrementCloseParen(target);
        expect(bindings.clauses.value[0].closeParens).toBe(0);
    });

    test('formats compact and overflow parenthesis stacks', () => {
        const { bindings } = setupInput();

        expect(bindings.formatParenStack('(', 0)).toBe('(');
        expect(bindings.formatParenStack(')', -1)).toBe(')');
        expect(bindings.formatParenStack('(', 1)).toBe('(');
        expect(bindings.formatParenStack('(', 3)).toBe('(((');
        expect(bindings.formatParenStack(')', 4)).toBe('))))');
        expect(bindings.formatParenStack(')', 6)).toBe('))))))');
        expect(bindings.formatParenStack('(', 7)).toBe('((((((×7');
    });
});

describe('KeywordInput production template behavior', () => {
    test('renders empty, raw/error, grouped, validation, copy, disabled, and non-composite branches', async () => {
        const callbacks: CapturedCallback[] = [];

        const empty = await renderSsrInput({});
        exerciseRenderTree(empty.vnode, callbacks);

        const raw = await renderSsrInput({
            modelValue: 'unsupported syntax',
            showHeader: false,
            helpText: 'Help text',
            exampleText: 'Example text',
        });
        exerciseRenderTree(raw.vnode, callbacks);

        const grouped = await renderSsrInput({
            modelValue: '(OR={one}/AND={two})|NOT={three}',
            disabled: false,
        }, bindings => {
            bindings.validationErrorKey.value = keywordExpression.RULE_EXPRESSION_UNBALANCED_KEY;
        });
        exerciseRenderTree(grouped.vnode, callbacks);

        const nonComposite = await renderSsrInput({
            modelValue: 'OR={one}+AND={two}',
            disabled: true,
            expressionFormat: 'legacy',
        });
        exerciseRenderTree(nonComposite.vnode, callbacks);

        expect(callbacks.filter(item => item.name === 'onClick').length).toBeGreaterThan(10);
        expect(callbacks.filter(item => item.name.startsWith('onUpdate:')).length).toBeGreaterThan(2);

        for (const { name, callback } of callbacks) {
            try {
                if (name === 'onUpdate:search') {
                    callback('draft candidate');
                } else if (name.startsWith('onUpdate:')) {
                    callback([' entered ', 'entered', '']);
                } else {
                    callback({ preventDefault: jest.fn(), stopPropagation: jest.fn() });
                }
            } catch {
                // Generated handlers close over rows that earlier callbacks may remove.
            }
        }

        expect(callbacks.some(item => item.name === 'onUpdate:search')).toBe(true);
        expect(callbacks.some(item => item.name === 'onUpdate:modelValue')).toBe(true);
    });
});
