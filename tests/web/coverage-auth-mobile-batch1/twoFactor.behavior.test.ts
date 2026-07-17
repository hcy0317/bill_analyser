import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from './hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const mockShowMessage = jest.fn<(...args: any[]) => void>();
const mockShowError = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockRouteBackOnError = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockCopyTextToClipboard = jest.fn<(...args: any[]) => void>();

const mockTwoFactorAuthStore = {
    get2FAStatus: jest.fn<(...args: any[]) => Promise<any>>(),
    enable2FA: jest.fn<(...args: any[]) => Promise<any>>(),
    confirmEnable2FA: jest.fn<(...args: any[]) => Promise<any>>(),
    disable2FA: jest.fn<(...args: any[]) => Promise<any>>(),
    regenerate2FARecoveryCode: jest.fn<(...args: any[]) => Promise<any>>()
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
            name: 'TwoFactorSnackBarStub',
            setup: (_props: unknown, { expose }: any) => {
                expose({ showMessage: mockShowMessage, showError: mockShowError });
                return () => h('snack-bar-stub');
            }
        })
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showToast: mockShowToast,
        routeBackOnError: mockRouteBackOnError
    }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args)
}));
jest.mock('@/stores/twoFactorAuth.ts', () => ({
    useTwoFactorAuthStore: () => mockTwoFactorAuthStore
}));
jest.mock('@/lib/ui/common.ts', () => ({
    copyTextToClipboard: (...args: any[]) => mockCopyTextToClipboard(...args)
}));

const MobilePage = require('@/views/mobile/users/TwoFactorAuthPage.vue').default as any;
const DesktopTab = require('@/views/desktop/user/settings/tabs/UserTwoFactorAuthSettingTab.vue').default as any;

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function setupMobile(): { bindings: any; router: any } {
    const router = { back: jest.fn() };
    const bindings = MobilePage.setup({ f7router: router }, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    });
    return { bindings, router };
}

function setupDesktop(): { bindings: any; exposed: Record<string, any> } {
    const exposed: Record<string, any> = {};
    const bindings = DesktopTab.setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(),
        expose: (value: Record<string, any>) => Object.assign(exposed, value)
    });
    bindings.snackbar.value = { showMessage: mockShowMessage, showError: mockShowError };
    return { bindings, exposed };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTwoFactorAuthStore.get2FAStatus.mockResolvedValue({ enable: false });
    mockTwoFactorAuthStore.enable2FA.mockResolvedValue({
        qrcode: 'data:image/png;base64,synthetic',
        secret: 'synthetic-secret'
    });
    mockTwoFactorAuthStore.confirmEnable2FA.mockResolvedValue({ recoveryCodes: ['code-1', 'code-2'] });
    mockTwoFactorAuthStore.disable2FA.mockResolvedValue(undefined);
    mockTwoFactorAuthStore.regenerate2FARecoveryCode.mockResolvedValue({ recoveryCodes: ['new-1', 'new-2'] });
    mockShowLoading.mockImplementation((predicate?: () => unknown) => predicate?.());
});

describe('mobile two-factor authentication page', () => {
    test('loads status and routes back only after an unprocessed initialization error', async () => {
        const success = setupMobile();
        await flush();
        expect(success.bindings.status.value).toBe(false);
        expect(success.bindings.loading.value).toBe(false);
        success.bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(success.router, success.bindings.loadingError);

        mockTwoFactorAuthStore.get2FAStatus.mockRejectedValueOnce({ processed: true });
        const processed = setupMobile();
        await flush();
        expect(processed.bindings.loading.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalled();

        mockTwoFactorAuthStore.get2FAStatus.mockRejectedValueOnce({ processed: false, message: 'status failed' });
        const failed = setupMobile();
        await flush();
        expect(failed.bindings.loadingError.value).toMatchObject({ message: 'status failed' });
        expect(mockShowToast).toHaveBeenCalledWith('status failed');

        mockTwoFactorAuthStore.get2FAStatus.mockRejectedValueOnce('raw status failure');
        setupMobile();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw status failure');
    });

    test('enables 2FA, confirms a passcode, and displays recovery codes', async () => {
        const { bindings } = setupMobile();
        await flush();
        bindings.enable();
        expect(bindings.enabling.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(expect.any(Function));
        await flush();
        expect(bindings.new2FAQRCode.value).toContain('synthetic');
        expect(bindings.new2FASecret.value).toBe('synthetic-secret');
        expect(bindings.showInputPasscodeSheetForEnable.value).toBe(true);

        bindings.currentPasscodeForEnable.value = '123456';
        bindings.enableConfirm();
        expect(mockTwoFactorAuthStore.confirmEnable2FA).toHaveBeenCalledWith({
            secret: 'synthetic-secret',
            passcode: '123456'
        });
        await flush();
        expect(bindings.status.value).toBe(true);
        expect(bindings.currentBackupCode.value).toBe('code-1\ncode-2');
        expect(bindings.showBackupCodeSheet.value).toBe(true);
        expect(bindings.showInputPasscodeSheetForEnable.value).toBe(false);

        mockTwoFactorAuthStore.confirmEnable2FA.mockResolvedValueOnce({ recoveryCodes: [] });
        bindings.new2FASecret.value = 'second-secret';
        bindings.currentPasscodeForEnable.value = '654321';
        bindings.showBackupCodeSheet.value = false;
        bindings.enableConfirm();
        await flush();
        expect(bindings.showBackupCodeSheet.value).toBe(false);
    });

    test('reports processed and readable enable or confirmation failures', async () => {
        const { bindings } = setupMobile();
        await flush();
        mockTwoFactorAuthStore.enable2FA.mockRejectedValueOnce({ processed: true, message: 'handled enable' });
        bindings.enable();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled enable');

        mockTwoFactorAuthStore.enable2FA.mockRejectedValueOnce({ processed: false, message: 'enable failed' });
        bindings.enable();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('enable failed');

        mockTwoFactorAuthStore.confirmEnable2FA.mockRejectedValueOnce({ processed: false, message: 'confirm failed' });
        bindings.enableConfirm();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('confirm failed');

        mockTwoFactorAuthStore.confirmEnable2FA.mockRejectedValueOnce({ processed: true, message: 'handled confirm' });
        bindings.enableConfirm();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled confirm');
        expect(mockHideLoading).toHaveBeenCalled();
    });

    test('opens password sheets, disables 2FA, and handles failure outcomes', async () => {
        const { bindings } = setupMobile();
        await flush();
        bindings.disable(null);
        expect(bindings.showInputPasswordSheetForDisable.value).toBe(true);
        expect(mockTwoFactorAuthStore.disable2FA).not.toHaveBeenCalled();

        bindings.disable('synthetic-password');
        expect(mockTwoFactorAuthStore.disable2FA).toHaveBeenCalledWith({ password: 'synthetic-password' });
        await flush();
        expect(bindings.status.value).toBe(false);
        expect(bindings.showInputPasswordSheetForDisable.value).toBe(false);
        expect(mockShowToast).toHaveBeenCalledWith('Two-factor authentication has been disabled');

        mockTwoFactorAuthStore.disable2FA.mockRejectedValueOnce({ processed: false, message: 'disable failed' });
        bindings.disable('bad-password');
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('disable failed');

        mockTwoFactorAuthStore.disable2FA.mockRejectedValueOnce({ processed: true, message: 'handled disable' });
        bindings.disable('bad-password');
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled disable');
    });

    test('regenerates recovery codes, reports failures, and confirms copy feedback', async () => {
        const { bindings } = setupMobile();
        await flush();
        bindings.regenerateBackupCode(null);
        expect(bindings.showInputPasswordSheetForRegenerate.value).toBe(true);

        bindings.regenerateBackupCode('synthetic-password');
        expect(mockTwoFactorAuthStore.regenerate2FARecoveryCode).toHaveBeenCalledWith({ password: 'synthetic-password' });
        await flush();
        expect(bindings.currentBackupCode.value).toBe('new-1\nnew-2');
        expect(bindings.showBackupCodeSheet.value).toBe(true);

        mockTwoFactorAuthStore.regenerate2FARecoveryCode.mockRejectedValueOnce({ processed: false, message: 'regen failed' });
        bindings.regenerateBackupCode('bad-password');
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('regen failed');

        mockTwoFactorAuthStore.regenerate2FARecoveryCode.mockRejectedValueOnce({ processed: true, message: 'handled regen' });
        bindings.regenerateBackupCode('bad-password');
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled regen');

        bindings.onBackupCodeCopied();
        expect(mockShowToast).toHaveBeenCalledWith('Backup codes copied');
    });
});

describe('desktop two-factor authentication settings tab', () => {
    test('loads enabled and disabled states and reports initialization errors', async () => {
        mockTwoFactorAuthStore.get2FAStatus.mockResolvedValueOnce({ enable: true });
        const enabled = setupDesktop();
        await flush();
        expect(enabled.bindings.status.value).toBe(true);
        expect(enabled.bindings.loading.value).toBe(false);

        mockTwoFactorAuthStore.get2FAStatus.mockRejectedValueOnce({ processed: false, message: 'desktop status failed' });
        const failed = setupDesktop();
        await flush();
        expect(failed.bindings.loading.value).toBe(false);
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'desktop status failed' }));

        mockTwoFactorAuthStore.get2FAStatus.mockRejectedValueOnce({ processed: true, message: 'handled status' });
        setupDesktop();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled status' }));
    });

    test('enables and confirms 2FA with input and duplicate-submit guards', async () => {
        const { bindings } = setupDesktop();
        await flush();
        bindings.currentBackupCode.value = 'stale';
        bindings.enable();
        expect(bindings.currentBackupCode.value).toBe('');
        await flush();
        expect(bindings.new2FASecret.value).toBe('synthetic-secret');

        bindings.enableConfirm();
        expect(mockShowMessage).toHaveBeenCalledWith('Passcode cannot be blank');
        bindings.currentPasscode.value = '123456';
        bindings.enableConfirming.value = true;
        bindings.enableConfirm();
        expect(mockTwoFactorAuthStore.confirmEnable2FA).not.toHaveBeenCalled();

        bindings.enableConfirming.value = false;
        bindings.enableConfirm();
        expect(mockTwoFactorAuthStore.confirmEnable2FA).toHaveBeenCalledWith({
            secret: 'synthetic-secret',
            passcode: '123456'
        });
        await flush();
        expect(bindings.status.value).toBe(true);
        expect(bindings.currentBackupCode.value).toBe('code-1\ncode-2');

        mockTwoFactorAuthStore.confirmEnable2FA.mockResolvedValueOnce({ recoveryCodes: [] });
        bindings.currentPasscode.value = '654321';
        bindings.new2FASecret.value = 'second-secret';
        bindings.enableConfirm();
        await flush();
        expect(bindings.currentBackupCode.value).toBe('');
    });

    test('reports enable and confirm failures without retaining busy state', async () => {
        const { bindings } = setupDesktop();
        await flush();
        mockTwoFactorAuthStore.enable2FA.mockRejectedValueOnce({ processed: false, message: 'desktop enable failed' });
        bindings.enable();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'desktop enable failed' }));
        expect(bindings.enabling.value).toBe(false);

        mockTwoFactorAuthStore.enable2FA.mockRejectedValueOnce({ processed: true, message: 'handled enable' });
        bindings.enable();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled enable' }));

        bindings.currentPasscode.value = '123456';
        mockTwoFactorAuthStore.confirmEnable2FA.mockRejectedValueOnce({ processed: false, message: 'desktop confirm failed' });
        bindings.enableConfirm();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'desktop confirm failed' }));

        bindings.currentPasscode.value = '654321';
        mockTwoFactorAuthStore.confirmEnable2FA.mockRejectedValueOnce({ processed: true, message: 'handled confirm' });
        bindings.enableConfirm();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled confirm' }));
    });

    test('disables with guards and handles success plus both error classes', async () => {
        const { bindings } = setupDesktop();
        await flush();
        bindings.disable();
        expect(mockShowMessage).toHaveBeenCalledWith('Current password cannot be blank');
        bindings.currentPassword.value = 'synthetic-password';
        bindings.disabling.value = true;
        bindings.disable();
        expect(mockTwoFactorAuthStore.disable2FA).not.toHaveBeenCalled();

        bindings.disabling.value = false;
        bindings.disable();
        await flush();
        expect(bindings.status.value).toBe(false);
        expect(mockShowMessage).toHaveBeenCalledWith('Two-factor authentication has been disabled');

        bindings.currentPassword.value = 'bad-password';
        mockTwoFactorAuthStore.disable2FA.mockRejectedValueOnce({ processed: false, message: 'desktop disable failed' });
        bindings.disable();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'desktop disable failed' }));

        bindings.currentPassword.value = 'bad-password';
        mockTwoFactorAuthStore.disable2FA.mockRejectedValueOnce({ processed: true, message: 'handled disable' });
        bindings.disable();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled disable' }));
    });

    test('regenerates, copies, resets, and handles recovery-code failures', async () => {
        const { bindings, exposed } = setupDesktop();
        await flush();
        bindings.regenerateBackupCode();
        expect(mockShowMessage).toHaveBeenCalledWith('Current password cannot be blank');
        bindings.currentPassword.value = 'synthetic-password';
        bindings.regenerating.value = true;
        bindings.regenerateBackupCode();
        expect(mockTwoFactorAuthStore.regenerate2FARecoveryCode).not.toHaveBeenCalled();

        bindings.regenerating.value = false;
        bindings.regenerateBackupCode();
        await flush();
        expect(bindings.currentBackupCode.value).toBe('new-1\nnew-2');
        bindings.copyBackupCodes();
        expect(mockCopyTextToClipboard).toHaveBeenCalledWith('new-1\nnew-2');
        expect(mockShowMessage).toHaveBeenCalledWith('Backup codes copied');

        bindings.currentPassword.value = 'bad-password';
        mockTwoFactorAuthStore.regenerate2FARecoveryCode.mockRejectedValueOnce({ processed: false, message: 'desktop regen failed' });
        bindings.regenerateBackupCode();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'desktop regen failed' }));

        bindings.currentPassword.value = 'bad-password';
        mockTwoFactorAuthStore.regenerate2FARecoveryCode.mockRejectedValueOnce({ processed: true, message: 'handled regen' });
        bindings.regenerateBackupCode();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled regen' }));

        bindings.currentPassword.value = 'temporary';
        bindings.currentPasscode.value = '123456';
        bindings.currentBackupCode.value = 'temporary-code';
        exposed['reset']();
        expect(bindings.currentPassword.value).toBe('');
        expect(bindings.currentPasscode.value).toBe('');
        expect(bindings.currentBackupCode.value).toBe('');
    });
});

describe('two-factor production templates', () => {
    test('renders loading, enabled, setup, and recovery-code branches with real event wrappers', async () => {
        const mobile = mountWithHostRenderer(MobilePage, { f7router: { back: jest.fn() } }, [
            'f7-page', 'f7-navbar', 'f7-list', 'f7-list-item', 'f7-list-button',
            'passcode-input-sheet', 'password-input-sheet', 'information-sheet'
        ]);
        const desktop = mountWithHostRenderer(DesktopTab, {}, [
            'v-row', 'v-col', 'v-card', 'v-card-text', 'v-progress-circular', 'v-skeleton-loader',
            'v-img', 'v-text-field', 'v-btn', 'v-icon', 'v-tooltip', 'v-textarea'
        ]);
        try {
            await flush();
            mobile.state.loading = false;
            mobile.state.status = true;
            mobile.state.currentBackupCode = 'mobile-code';
            mobile.state.showBackupCodeSheet = true;
            desktop.state.loading = false;
            desktop.state.status = true;
            desktop.state.currentPassword = 'synthetic-password';
            desktop.state.currentBackupCode = 'desktop-code';
            await actualVue.nextTick();

            const mobileCallbacks = collectHostCallbacks(mobile.root);
            const desktopCallbacks = collectHostCallbacks(desktop.root);
            expect(mobileCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onPage:afterin', 'onClick', 'onPasscode:confirm', 'onPassword:confirm', 'onInfo:copied'
            ]));
            expect(desktopCallbacks.map(item => item.name)).toContain('onClick');

            for (const { name, callback } of mobileCallbacks) {
                if (name === 'onPassword:confirm') callback('synthetic-password');
                else if (name === 'onPasscode:confirm') callback();
                else if (name.startsWith('onUpdate:modelValue')) callback('123456');
                else if (name.startsWith('onUpdate:show')) callback(false);
                else callback();
                await flush(1);
            }
            for (const { name, callback } of desktopCallbacks) {
                if (name.startsWith('onUpdate:')) callback('synthetic-password');
                else callback();
                await flush(1);
            }

            mobile.state.status = false;
            mobile.state.enabling = true;
            desktop.state.status = false;
            desktop.state.new2FAQRCode = 'data:image/png;base64,synthetic';
            desktop.state.currentPasscode = '123456';
            await actualVue.nextTick();

            const setupCallbacks = collectHostCallbacks(desktop.root);
            expect(setupCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onClick', 'onKeyup'
            ]));
            for (const { name, callback } of setupCallbacks) {
                if (name.startsWith('onUpdate:')) callback('123456');
                else if (name === 'onKeyup') callback({ key: 'Enter' });
                else if (name === 'onClick') callback();
                await flush(1);
            }
            const mobileSetupCallbacks = collectHostCallbacks(mobile.root);
            for (const { name, callback } of mobileSetupCallbacks) {
                if (name === 'onPassword:confirm') callback('synthetic-password');
                else if (name.startsWith('onUpdate:modelValue')) callback('123456');
                else if (name.startsWith('onUpdate:show')) callback(false);
                else callback();
                await flush(1);
            }
            expect(mobile.root.children.length).toBeGreaterThan(0);
            expect(desktop.root.children.length).toBeGreaterThan(0);
        } finally {
            mobile.app.unmount();
            desktop.app.unmount();
        }
    });
});
