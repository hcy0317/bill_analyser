import { describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/components/common/CategoryRuleBuilderFields.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return {
        __esModule: true,
        default: defineComponent({
            name: 'CategoryRuleBuilderFieldsStub',
            inheritAttrs: false,
            setup: (_props: unknown, { attrs }: any) => () => h('rule-builder-stub', attrs)
        })
    };
});
jest.mock('@/components/desktop/ItemIcon.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return {
        __esModule: true,
        default: defineComponent({
            name: 'AccountRuleItemIconStub',
            inheritAttrs: false,
            setup: (_props: unknown, { attrs }: any) => () => h('item-icon-stub', attrs)
        })
    };
});

import AccountRuleDialogs from '@/views/desktop/pairingcenter/components/AccountRuleDialogs.vue';

const uiComponents = [
    'v-dialog', 'v-card', 'v-card-text', 'v-card-title', 'v-card-actions', 'v-form',
    'v-row', 'v-col', 'v-select', 'v-list-item', 'v-text-field', 'v-textarea',
    'v-alert', 'v-spacer', 'v-btn'
];

function createRule(overrides: Record<string, unknown> = {}): any {
    return {
        id: 7,
        accountId: 'wallet',
        name: 'Wallet merchant',
        priority: 10,
        ruleExpression: 'coffee !refund',
        regexEnabled: false,
        enabled: true,
        ...overrides
    };
}

function createProps(overrides: Record<string, unknown> = {}): any {
    return {
        editingRule: null,
        accountOptions: [
            { id: 'wallet', name: 'Wallet', icon: 'wallet', color: '#112233' },
            { id: 'bank', name: 'Bank', icon: 'bank', color: '#445566' }
        ],
        autoRuleName: 'Wallet merchant',
        saving: false,
        isAccountLocked: false,
        testRuleName: 'Wallet merchant',
        testResult: null,
        testing: false,
        deletingRule: null,
        deleting: false,
        showEditDialog: true,
        showTestDialog: true,
        showDeleteDialog: true,
        ruleForm: {
            accountId: 'wallet',
            name: 'Wallet merchant',
            priority: 10,
            ruleExpression: 'coffee',
            regexEnabled: false,
            enabled: true
        },
        ruleBuilderModel: {
            priority: 10,
            ruleExpression: 'coffee',
            regexEnabled: false,
            enabled: true
        },
        testText: 'coffee shop',
        ...overrides
    };
}

function createEvent(): any {
    return {
        stopPropagation: jest.fn(),
        preventDefault: jest.fn(),
        target: {}
    };
}

async function flush(): Promise<void> {
    await Promise.resolve();
    await actualVue.nextTick();
}

async function exerciseCallbacks(root: any): Promise<string[]> {
    const callbacks = collectHostCallbacks(root);
    for (const { name, callback } of callbacks) {
        if (name === 'onUpdate:modelValue') callback('updated-value');
        else callback(createEvent());
        await flush();
    }
    return callbacks.map(item => item.name);
}

describe('AccountRuleDialogs model state and events', () => {
    test('renders create, empty-test, and empty-delete states and emits all dialog actions', async () => {
        const onSave = jest.fn();
        const onRunTest = jest.fn();
        const onDelete = jest.fn();
        const onEditVisibility = jest.fn();
        const onTestVisibility = jest.fn();
        const onDeleteVisibility = jest.fn();
        const onRuleBuilder = jest.fn();
        const onTestText = jest.fn();
        const mounted = mountWithHostRenderer(AccountRuleDialogs as any, createProps({
            onSave,
            onRunTest,
            'onRun-test': onRunTest,
            onDelete,
            'onUpdate:showEditDialog': onEditVisibility,
            'onUpdate:showTestDialog': onTestVisibility,
            'onUpdate:showDeleteDialog': onDeleteVisibility,
            'onUpdate:ruleBuilderModel': onRuleBuilder,
            'onUpdate:testText': onTestText
        }), uiComponents);
        try {
            const callbackNames = await exerciseCallbacks(mounted.root);
            expect(callbackNames).toEqual(expect.arrayContaining(['onClick', 'onUpdate:modelValue']));
            expect(onSave).toHaveBeenCalled();
            expect(onRunTest).toHaveBeenCalled();
            expect(onDelete).toHaveBeenCalled();
            expect(onEditVisibility).toHaveBeenCalled();
            expect(onTestVisibility).toHaveBeenCalled();
            expect(onDeleteVisibility).toHaveBeenCalled();
            expect(onRuleBuilder).toHaveBeenCalled();
            expect(onTestText).toHaveBeenCalled();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });

    test('renders edit, locked, saving, testing, deleting, and successful-match states', async () => {
        const mounted = mountWithHostRenderer(AccountRuleDialogs as any, createProps({
            editingRule: createRule(),
            deletingRule: createRule({ name: 'Delete me' }),
            saving: true,
            isAccountLocked: true,
            testResult: true,
            testing: true,
            deleting: true
        }), uiComponents);
        try {
            expect(mounted.root.children.length).toBeGreaterThan(0);
            expect(collectHostCallbacks(mounted.root).map(item => item.name)).toContain('onClick');
        } finally {
            mounted.app.unmount();
        }
    });

    test('renders failed-match and unlocked idle branches', () => {
        const mounted = mountWithHostRenderer(AccountRuleDialogs as any, createProps({
            editingRule: createRule({ name: 'Editable rule' }),
            deletingRule: createRule({ name: 'Delete rule' }),
            saving: false,
            isAccountLocked: false,
            testResult: false,
            testing: false,
            deleting: false,
            accountOptions: []
        }), uiComponents);
        try {
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
