import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import {
    mountWithHostRenderer,
    type HostNode,
} from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;

const mockTemplateRefs = new Map<string, any>();
const mockGenerateToken = jest.fn<(...args: any[]) => Promise<any>>();
const mockCopyTextToClipboard = jest.fn<(...args: any[]) => void>();
const mockMountedSnackbarShowError = jest.fn<(...args: any[]) => void>();
const mockMountedSnackbarShowMessage = jest.fn<(...args: any[]) => void>();
const mockLogger = {
    debug: jest.fn(),
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn(),
};

let mockApiTokenEnabled = true;
let mockMcpServerEnabled = true;

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => (
            values ? `tt:${key}:${JSON.stringify(values)}` : `tt:${key}`
        ),
    }),
}));
jest.mock('@/stores/token.ts', () => ({
    useTokensStore: () => ({ generateToken: mockGenerateToken }),
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isAPITokenEnabled: () => mockApiTokenEnabled,
    isMCPServerEnabled: () => mockMcpServerEnabled,
}));
jest.mock('@/lib/ui/common.ts', () => ({
    copyTextToClipboard: (...args: any[]) => mockCopyTextToClipboard(...args),
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: mockLogger,
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: {
        name: 'TokenDialogSnackBarStub',
        setup: (_props: unknown, { expose }: { expose: (value: Record<string, unknown>) => void }) => {
            expose({
                showError: (...args: any[]) => mockMountedSnackbarShowError(...args),
                showMessage: (...args: any[]) => mockMountedSnackbarShowMessage(...args),
            });
            return () => null;
        },
    },
}));

import UserGenerateTokenDialogComponent from '@/views/desktop/user/settings/dialogs/UserGenerateTokenDialog.vue';

const UserGenerateTokenDialog = UserGenerateTokenDialogComponent as any;

interface SnackbarHarness {
    showError: jest.Mock;
    showMessage: jest.Mock;
}

interface SetupHarness {
    bindings: any;
    exposed: Record<string, any>;
    snackbar: SnackbarHarness;
    buttonContainer: { id: string };
}

function setup(installTemplateRefs = true): SetupHarness {
    mockTemplateRefs.clear();
    const exposed: Record<string, any> = {};
    const bindings = UserGenerateTokenDialog.setup({}, {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: (value: Record<string, any>) => Object.assign(exposed, value),
    });
    const snackbar: SnackbarHarness = {
        showError: jest.fn(),
        showMessage: jest.fn(),
    };
    const buttonContainer = { id: 'token-dialog-actions' };
    if (installTemplateRefs) {
        bindings.snackbar.value = snackbar;
        bindings.buttonContainer.value = buttonContainer;
    }
    return { bindings, exposed, snackbar, buttonContainer };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

async function settleCancellation(pending: Promise<void>, cancel: () => void): Promise<unknown> {
    const settled = pending.then(
        value => value,
        error => error,
    );
    cancel();
    return settled;
}

function createDeferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (reason: unknown) => void;
} {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

interface HostEvent {
    name: string;
    callback: (...args: any[]) => unknown;
    props: Record<string, unknown>;
}

function collectHostEvents(node: HostNode, events: HostEvent[] = [], seen = new Set<HostNode>()): HostEvent[] {
    if (!node || seen.has(node)) return events;
    seen.add(node);
    for (const [name, value] of Object.entries(node.props)) {
        if (!name.startsWith('on')) continue;
        for (const candidate of Array.isArray(value) ? value : [value]) {
            if (typeof candidate === 'function') {
                events.push({
                    name,
                    callback: candidate as (...args: any[]) => unknown,
                    props: node.props,
                });
            }
        }
    }
    for (const child of node.children) collectHostEvents(child, events, seen);
    return events;
}

function collectHostNodes(node: HostNode, nodes: HostNode[] = [], seen = new Set<HostNode>()): HostNode[] {
    if (!node || seen.has(node)) return nodes;
    seen.add(node);
    nodes.push(node);
    for (const child of node.children) collectHostNodes(child, nodes, seen);
    return nodes;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    mockApiTokenEnabled = true;
    mockMcpServerEnabled = true;
    mockGenerateToken.mockResolvedValue({
        token: 'generated-api-secret',
        apiBaseUrl: 'https://api.example.test',
    });
});

describe('UserGenerateTokenDialog availability and lifecycle', () => {
    test('exposes open and resets every field while preferring the enabled token type', async () => {
        const { bindings, exposed } = setup();
        expect(exposed).toEqual({ open: bindings.open });

        bindings.showState.value = false;
        bindings.currentPassword.value = 'old-password';
        bindings.tokenType.value = 'api';
        bindings.tokenExpirationTime.value = -1;
        bindings.tokenCustomExpirationTime.value = 7;
        bindings.generating.value = true;
        bindings.showAPIExample.value = true;
        bindings.showMCPConfiguration.value = true;
        bindings.serverUrl.value = 'https://old.example.test';
        bindings.generatedToken.value = 'old-secret';

        mockApiTokenEnabled = false;
        mockMcpServerEnabled = true;
        const pending = exposed['open']();

        expect(bindings.showState.value).toBe(true);
        expect(bindings.currentPassword.value).toBe('');
        expect(bindings.tokenType.value).toBe('mcp');
        expect(bindings.tokenExpirationTime.value).toBe(86_400);
        expect(bindings.tokenCustomExpirationTime.value).toBe(86_400);
        expect(bindings.generating.value).toBe(false);
        expect(bindings.showAPIExample.value).toBe(false);
        expect(bindings.showMCPConfiguration.value).toBe(false);
        expect(bindings.serverUrl.value).toBe('');
        expect(bindings.generatedToken.value).toBe('');

        expect(await settleCancellation(pending, bindings.cancel)).toBeUndefined();
        expect(bindings.showState.value).toBe(false);
    });

    test('falls back to API when neither capability is enabled and resolves only on close', async () => {
        mockApiTokenEnabled = false;
        mockMcpServerEnabled = false;
        const { bindings } = setup();
        bindings.cancel();
        bindings.close();

        const pending = bindings.open();
        expect(bindings.tokenType.value).toBe('api');
        bindings.close();
        await expect(pending).resolves.toBeUndefined();
        expect(bindings.showState.value).toBe(false);
    });

    test('prefers API whenever API token generation is enabled', async () => {
        mockApiTokenEnabled = true;
        mockMcpServerEnabled = true;
        const { bindings } = setup();
        const pending = bindings.open();
        expect(bindings.tokenType.value).toBe('api');
        bindings.close();
        await expect(pending).resolves.toBeUndefined();
    });

    test.each([
        [true, true, ['api', 'mcp']],
        [true, false, ['api']],
        [false, true, ['mcp']],
        [false, false, []],
    ])('offers only enabled token types for API=%s MCP=%s', (apiEnabled, mcpEnabled, expected) => {
        mockApiTokenEnabled = apiEnabled;
        mockMcpServerEnabled = mcpEnabled;
        const { bindings } = setup();
        expect(bindings.tokenTypeOptions.value.map((option: { value: string }) => option.value)).toEqual(expected);
    });
});

describe('UserGenerateTokenDialog generation and errors', () => {
    test('blocks empty and duplicate submissions, then generates an API token with the standard expiry', async () => {
        const deferred = createDeferred<{ token: string; apiBaseUrl: string }>();
        mockGenerateToken.mockReturnValueOnce(deferred.promise);
        const { bindings, snackbar } = setup();

        bindings.generateToken();
        expect(mockGenerateToken).not.toHaveBeenCalled();

        bindings.currentPassword.value = 'current-password-secret';
        bindings.tokenType.value = 'api';
        bindings.tokenExpirationTime.value = 3_600;
        bindings.generateToken();
        bindings.generateToken();

        expect(bindings.generating.value).toBe(true);
        expect(mockGenerateToken).toHaveBeenCalledTimes(1);
        expect(mockGenerateToken).toHaveBeenCalledWith({
            type: 'api',
            expiresInSeconds: 3_600,
            password: 'current-password-secret',
        });

        deferred.resolve({
            token: 'api-token-secret',
            apiBaseUrl: 'https://api.example.test',
        });
        await flush();

        expect(bindings.generating.value).toBe(false);
        expect(bindings.currentPassword.value).toBe('');
        expect(bindings.serverUrl.value).toBe('https://api.example.test');
        expect(bindings.generatedToken.value).toBe('api-token-secret');
        expect(bindings.apiExample.value).toBe(
            "curl -H 'Authorization: Bearer api-token-secret' 'https://api.example.test/profile'",
        );
        expect(snackbar.showError).not.toHaveBeenCalled();
        expect(snackbar.showMessage).not.toHaveBeenCalled();
        expect(mockLogger.info).not.toHaveBeenCalled();
        expect(mockLogger.warn).not.toHaveBeenCalled();
        expect(mockLogger.error).not.toHaveBeenCalled();
    });

    test('uses the custom zero-second boundary and builds an MCP configuration', async () => {
        mockGenerateToken.mockResolvedValueOnce({
            token: 'mcp-token-secret',
            mcpUrl: 'https://mcp.example.test/endpoint',
        });
        const { bindings } = setup();
        bindings.currentPassword.value = 'mcp-password';
        bindings.tokenType.value = 'mcp';
        bindings.tokenExpirationTime.value = -1;
        bindings.tokenCustomExpirationTime.value = 0;

        bindings.generateToken();
        await flush();

        expect(mockGenerateToken).toHaveBeenCalledWith({
            type: 'mcp',
            expiresInSeconds: 0,
            password: 'mcp-password',
        });
        expect(bindings.serverUrl.value).toBe('https://mcp.example.test/endpoint');
        expect(bindings.generatedToken.value).toBe('mcp-token-secret');
        expect(JSON.parse(bindings.mcpServerConfiguration.value)).toEqual({
            mcpServers: {
                'bill_analyser-mcp': {
                    type: 'streamable-http',
                    url: 'https://mcp.example.test/endpoint',
                    headers: { Authorization: 'Bearer mcp-token-secret' },
                },
            },
        });
    });

    test('keeps a generated token even for an unexpected store discriminator without inventing a URL', async () => {
        mockGenerateToken.mockResolvedValueOnce({ token: 'opaque-token-secret' });
        const { bindings } = setup();
        bindings.currentPassword.value = 'password';
        bindings.tokenType.value = 'unexpected';

        bindings.generateToken();
        await flush();

        expect(bindings.serverUrl.value).toBe('');
        expect(bindings.generatedToken.value).toBe('opaque-token-secret');
    });

    test('shows only unprocessed failures and safely handles a missing snackbar ref', async () => {
        const processedError = { processed: true, message: 'already handled' };
        mockGenerateToken.mockRejectedValueOnce(processedError);
        const processed = setup();
        processed.bindings.currentPassword.value = 'password';
        processed.bindings.generateToken();
        await flush();
        expect(processed.bindings.generating.value).toBe(false);
        expect(processed.snackbar.showError).not.toHaveBeenCalled();

        const unprocessedError = { processed: false, message: 'generation failed' };
        mockGenerateToken.mockRejectedValueOnce(unprocessedError);
        const unprocessed = setup();
        unprocessed.bindings.currentPassword.value = 'password';
        unprocessed.bindings.generateToken();
        await flush();
        expect(unprocessed.snackbar.showError).toHaveBeenCalledWith(unprocessedError);
        expect(unprocessed.snackbar.showError).not.toHaveBeenCalledWith(expect.stringContaining('token-secret'));

        mockGenerateToken.mockRejectedValueOnce(unprocessedError);
        const withoutSnackbar = setup(false);
        withoutSnackbar.bindings.currentPassword.value = 'password';
        withoutSnackbar.bindings.generateToken();
        await expect(flush()).resolves.toBeUndefined();
        expect(withoutSnackbar.bindings.generating.value).toBe(false);
    });
});

describe('UserGenerateTokenDialog copying and secret boundaries', () => {
    test('copies API examples, MCP configurations, and raw tokens only to the clipboard sink', () => {
        const { bindings, snackbar, buttonContainer } = setup();
        bindings.generatedToken.value = 'clipboard-only-secret';
        bindings.serverUrl.value = 'https://service.example.test';

        bindings.tokenType.value = 'api';
        bindings.showAPIExample.value = true;
        bindings.copy();
        expect(mockCopyTextToClipboard).toHaveBeenLastCalledWith(
            "curl -H 'Authorization: Bearer clipboard-only-secret' 'https://service.example.test/profile'",
            buttonContainer,
        );

        bindings.showAPIExample.value = false;
        bindings.copy();
        expect(mockCopyTextToClipboard).toHaveBeenLastCalledWith('clipboard-only-secret', buttonContainer);

        bindings.tokenType.value = 'mcp';
        bindings.showMCPConfiguration.value = true;
        bindings.copy();
        expect(mockCopyTextToClipboard).toHaveBeenLastCalledWith(
            expect.stringContaining('Bearer clipboard-only-secret'),
            buttonContainer,
        );

        bindings.showMCPConfiguration.value = false;
        bindings.copy();
        expect(mockCopyTextToClipboard).toHaveBeenLastCalledWith('clipboard-only-secret', buttonContainer);

        expect(snackbar.showMessage).toHaveBeenCalledTimes(4);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Data copied');
        for (const call of snackbar.showMessage.mock.calls) {
            expect(call).not.toContain('clipboard-only-secret');
        }
        expect(snackbar.showError).not.toHaveBeenCalled();
        expect(mockLogger.error).not.toHaveBeenCalled();
    });

    test('copies without exposing the token when the snackbar ref is absent', () => {
        const { bindings } = setup(false);
        bindings.generatedToken.value = 'raw-token-secret';
        bindings.copy();
        expect(mockCopyTextToClipboard).toHaveBeenCalledWith('raw-token-secret', null);
        expect(mockLogger.warn).not.toHaveBeenCalled();
        expect(mockLogger.error).not.toHaveBeenCalled();
    });
});

describe('UserGenerateTokenDialog production template', () => {
    const componentNames = [
        'v-dialog',
        'v-card',
        'v-card-text',
        'v-switch',
        'v-row',
        'v-col',
        'v-select',
        'v-text-field',
        'v-textarea',
        'v-alert',
        'v-btn',
        'v-progress-circular',
    ];

    test('renders form, generating, API, and MCP states and invokes real event wrappers', async () => {
        const mounted = mountWithHostRenderer(UserGenerateTokenDialog, {}, componentNames);
        try {
            mounted.state.showState = true;
            mounted.state.currentPassword = 'template-password';
            mounted.state.tokenExpirationTime = -1;
            mounted.state.tokenCustomExpirationTime = 0;
            mounted.state.generating = true;
            await actualVue.nextTick();

            const formNodes = collectHostNodes(mounted.root);
            expect(formNodes.some(node => node.props['type'] === 'password')).toBe(true);
            expect(formNodes.some(node => node.props['indeterminate'] === '')).toBe(true);
            const formEvents = collectHostEvents(mounted.root);
            expect(formEvents.map(event => event.name)).toEqual(expect.arrayContaining([
                'onClick',
                'onKeyup',
                'onUpdate:modelValue',
            ]));

            for (const event of formEvents) {
                const label = String(event.props['label'] ?? '');
                if (event.name === 'onUpdate:modelValue') {
                    if (label === 'tt:Token Type') event.callback('api');
                    else if (label === 'tt:Expiration Time (Seconds)') event.callback(3_600);
                    else if (label === 'tt:Custom Expiration Time (Seconds)') event.callback(0);
                    else if (label === 'tt:Current Password') event.callback('template-password');
                    else event.callback(true);
                } else if (event.name === 'onKeyup') {
                    event.callback({ key: 'Enter' });
                } else if (event.name === 'onClick') {
                    event.callback({ type: 'synthetic-click' });
                }
            }
            await flush();

            mounted.state.generating = false;
            mounted.state.generatedToken = 'template-token-secret';
            mounted.state.serverUrl = 'https://api.example.test';
            mounted.state.tokenType = 'api';
            mounted.state.showAPIExample = false;
            await actualVue.nextTick();
            let tokenNodes = collectHostNodes(mounted.root);
            expect(tokenNodes.some(node => node.props['rows'] === 4 && node.props['value'] === 'template-token-secret')).toBe(true);

            mounted.state.showAPIExample = true;
            await actualVue.nextTick();
            tokenNodes = collectHostNodes(mounted.root);
            expect(tokenNodes.some(node => (
                node.props['rows'] === 5 && String(node.props['value']).includes('Bearer template-token-secret')
            ))).toBe(true);

            for (const event of collectHostEvents(mounted.root)) {
                if (event.name === 'onUpdate:modelValue') event.callback(false);
                if (event.name === 'onClick') event.callback({ type: 'synthetic-click' });
            }
            await flush();

            mounted.state.showState = true;
            mounted.state.generatedToken = 'mcp-template-secret';
            mounted.state.serverUrl = 'https://mcp.example.test';
            mounted.state.tokenType = 'mcp';
            mounted.state.showMCPConfiguration = false;
            await actualVue.nextTick();
            tokenNodes = collectHostNodes(mounted.root);
            expect(tokenNodes.some(node => node.props['rows'] === 4 && node.props['value'] === 'mcp-template-secret')).toBe(true);

            mounted.state.showMCPConfiguration = true;
            await actualVue.nextTick();
            tokenNodes = collectHostNodes(mounted.root);
            expect(tokenNodes.some(node => (
                node.props['rows'] === 15 && String(node.props['value']).includes('Bearer mcp-template-secret')
            ))).toBe(true);

            const configurationSwitchEvents = collectHostEvents(mounted.root).filter(event => (
                event.props['label'] === 'tt:Configuration'
            ));
            expect(configurationSwitchEvents.map(event => event.name)).toEqual(expect.arrayContaining([
                'onClick',
                'onUpdate:modelValue',
            ]));
            for (const event of configurationSwitchEvents) {
                if (event.name === 'onUpdate:modelValue') event.callback(false);
                if (event.name === 'onClick') event.callback({ type: 'synthetic-click' });
            }
            await actualVue.nextTick();

            mounted.state.serverUrl = '';
            await actualVue.nextTick();
            tokenNodes = collectHostNodes(mounted.root);
            expect(tokenNodes.some(node => node.props['rows'] === 4)).toBe(true);
        } finally {
            mounted.app.unmount();
        }
    });
});
