import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
    collectHostCallbacks,
    type HostNode,
    mountWithHostRenderer
} from '../coverage-auth-mobile-batch1/hostRenderer';

const mockActualVue = jest.requireActual('vue') as any;
const mockTemplateRefs = new Map<string, any>();
const mockShowMessage = jest.fn<(...args: any[]) => void>();
const mockShowError = jest.fn<(...args: any[]) => void>();
const mockConfirmOpen = jest.fn<(...args: any[]) => Promise<void>>();
const mockAvatarInputClick = jest.fn();
const mockSetCurrentUserProfile = jest.fn<(profile: any) => void>();
const mockReset = jest.fn();
const mockDoAfterProfileUpdate = jest.fn<(profile: any) => void>();
const mockToProfileUpdateRequest = jest.fn<() => Record<string, unknown>>();
const mockGetUserAvatarUrl = jest.fn<(avatar: string, cacheId: string) => string | null>();
const mockGenerateRandomUUID = jest.fn(() => '<avatar-cache-id>');
const mockCreateSelectionTexts = jest.fn<(...args: any[]) => any>();
const mockExternalTemplateBindings = jest.fn<(...args: unknown[]) => void>();

let mockVerifyEmailEnabled = true;
let mockProfileResponse: any;
let mockLastBase: any;

const mockRootStore = {
    updateUserProfile: jest.fn<(...args: any[]) => Promise<any>>(),
    resendVerifyEmailByLoginedUser: jest.fn<() => Promise<void>>()
};
const mockUserStore = {
    getCurrentUserProfile: jest.fn<() => Promise<any>>(),
    getUserAvatarUrl: (avatar: string, cacheId: string) => mockGetUserAvatarUrl(avatar, cacheId),
    updateUserAvatar: jest.fn<(...args: any[]) => Promise<any>>(),
    removeUserAvatar: jest.fn<() => Promise<any>>()
};
const mockAccountsStore = {
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>()
};
const mockCategoriesStore = mockActualVue.reactive({
    allTransactionCategories: {
        3: [{ id: 'transfer-primary', name: 'Transfer', subCategories: [] }]
    } as Record<number, any[]>,
    hasAvailableTransferCategories: true,
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>()
});

class MockAccount {
    static findAccountNameById(accounts: any[], id: string, fallback: string): string {
        return accounts.find(account => account.id === id)?.name ?? fallback;
    }
}

function createProfile(overrides: Record<string, unknown> = {}): any {
    return {
        id: 7,
        username: 'alice',
        email: 'alice@example.invalid',
        nickname: 'Alice',
        password: '',
        confirmPassword: '',
        avatar: 'avatar-initial',
        avatarProvider: 'internal',
        emailVerified: false,
        defaultAccountId: 'account-1',
        cashAccountId: 'cash-1',
        cashTransferCategoryId: 'transfer-primary',
        transactionEditScope: 1,
        language: 'zh-Hans',
        defaultCurrency: 'CNY',
        firstDayOfWeek: 1,
        fiscalYearStart: 1,
        calendarDisplayType: 1,
        dateDisplayType: 1,
        longDateFormat: 1,
        shortDateFormat: 1,
        longTimeFormat: 1,
        shortTimeFormat: 1,
        fiscalYearFormat: 1,
        currencyDisplayType: 1,
        numeralSystem: 1,
        decimalSeparator: 1,
        digitGroupingSymbol: 1,
        digitGrouping: 1,
        coordinateDisplayType: 1,
        expenseAmountColor: 1,
        incomeAmountColor: 2,
        toProfileUpdateRequest: mockToProfileUpdateRequest,
        ...overrides
    };
}

function copyProfile(profile: any): any {
    const copy = { ...profile, password: '', confirmPassword: '' };
    delete copy.accessToken;
    delete copy.refreshToken;
    return copy;
}

function createProfileBase(): any {
    const { ref } = jest.requireActual('vue') as any;
    return {
        newProfile: ref(createProfile()),
        oldProfile: ref(createProfile()),
        emailVerified: ref(false),
        loading: ref(false),
        resending: ref(false),
        saving: ref(false),
        allAccounts: ref([
            { id: 'account-1', name: 'Wallet' },
            { id: 'cash-1', name: 'Cash' }
        ]),
        allVisibleAccounts: ref([
            { id: 'account-1', name: 'Wallet' },
            { id: 'cash-1', name: 'Cash' }
        ]),
        allVisibleCategorizedAccounts: ref([
            { category: 'cash', accounts: [{ id: 'cash-1', name: 'Cash' }] }
        ]),
        allWeekDays: ref([{ type: 1, displayName: 'Monday' }]),
        allCalendarDisplayTypes: ref([{ type: 1, displayName: 'Gregorian' }]),
        allDateDisplayTypes: ref([{ type: 1, displayName: 'Date' }]),
        allLongDateFormats: ref([{ type: 1, displayName: 'Long Date' }]),
        allShortDateFormats: ref([{ type: 1, displayName: 'Short Date' }]),
        allLongTimeFormats: ref([{ type: 1, displayName: 'Long Time' }]),
        allShortTimeFormats: ref([{ type: 1, displayName: 'Short Time' }]),
        allFiscalYearFormats: ref([{ type: 1, displayName: 'Fiscal' }]),
        allCurrencyDisplayTypes: ref([{ type: 1, displayName: 'Currency' }]),
        allNumeralSystemTypes: ref([{ type: 1, displayName: 'Numeral' }]),
        allDecimalSeparators: ref([{ type: 1, displayName: 'Decimal' }]),
        allDigitGroupingSymbols: ref([{ type: 1, displayName: 'Grouping Symbol' }]),
        allDigitGroupingTypes: ref([{ type: 1, displayName: 'Grouping' }]),
        allCoordinateDisplayTypes: ref([{ type: 1, displayName: 'Coordinate' }]),
        allExpenseAmountColorTypes: ref([{ type: 1, displayName: 'Expense' }]),
        allIncomeAmountColorTypes: ref([{ type: 2, displayName: 'Income' }]),
        allTransactionEditScopeTypes: ref([{ type: 1, displayName: 'Scope' }]),
        languageTitle: ref('Language'),
        supportDigitGroupingSymbol: ref(true),
        inputIsNotChangedProblemMessage: ref(null),
        inputInvalidProblemMessage: ref(null),
        langAndRegionInputInvalidProblemMessage: ref(null),
        extendInputInvalidProblemMessage: ref(null),
        inputIsNotChanged: ref(false),
        inputIsInvalid: ref(false),
        setCurrentUserProfile: mockSetCurrentUserProfile,
        reset: mockReset,
        doAfterProfileUpdate: mockDoAfterProfileUpdate
    };
}

mockSetCurrentUserProfile.mockImplementation(profile => {
    if (!mockLastBase) return;
    const safeProfile = copyProfile(profile);
    mockLastBase.oldProfile.value = { ...safeProfile };
    mockLastBase.newProfile.value = { ...safeProfile };
    mockLastBase.emailVerified.value = Boolean(profile.emailVerified);
});

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        }
    };
});

const mockChildComponent = (name: string, exposed: Record<string, unknown> = {}) => ({
    __esModule: true,
    default: mockActualVue.defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, expose, slots }: any) => {
            expose(exposed);
            return () => mockActualVue.h(
                `${name}-stub`,
                attrs,
                Object.values(slots).flatMap((slot: any) => slot?.({}) ?? [])
            );
        }
    })
});

jest.mock('@/components/desktop/ConfirmDialog.vue', () => mockChildComponent(
    'UserBasicConfirmDialog',
    { open: mockConfirmOpen }
));
jest.mock('@/components/desktop/SnackBar.vue', () => mockChildComponent(
    'UserBasicSnackBar',
    { showMessage: mockShowMessage, showError: mockShowError }
));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/views/base/users/UserProfilePageBase.ts', () => ({
    useUserProfilePageBase: () => {
        mockLastBase = createProfileBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => mockRootStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoriesStore
}));
jest.mock('@/core/category.ts', () => ({ CategoryType: { Transfer: 3 } }));
jest.mock('@/models/account.ts', () => ({ Account: MockAccount }));
jest.mock('@/consts/file.ts', () => ({ SUPPORTED_IMAGE_EXTENSIONS: '.png,.jpg,.jpeg' }));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => mockGenerateRandomUUID() }));
jest.mock('@/lib/server_settings.ts', () => ({
    isUserVerifyEmailEnabled: () => mockVerifyEmailEnabled
}));
jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: (...args: unknown[]) => mockExternalTemplateBindings(...args)
}));
jest.mock('@/views/desktop/user/settings/tabs/basic/accountCategoryLabels.ts', () => ({
    createAccountCategorySelectionTexts: (...args: any[]) => mockCreateSelectionTexts(...args)
}));
jest.mock('@mdi/js', () => ({
    mdiAccount: 'account-icon',
    mdiAccountEditOutline: 'account-edit-icon'
}));

import UserBasicSettingTabModule from '@/views/desktop/user/settings/tabs/UserBasicSettingTab.vue';

const UserBasicSettingTab = UserBasicSettingTabModule as any;

function createRenderableUserBasicSettingTab(): any {
    const sourcePath = resolve(
        __dirname,
        '../../../src/web/src/views/desktop/user/settings/tabs/UserBasicSettingTab.vue'
    );
    const templatePath = resolve(
        __dirname,
        '../../../src/web/src/views/desktop/user/settings/tabs/basic/UserBasicSettingTab.template.html'
    );
    const source = readFileSync(sourcePath, 'utf8');
    const template = readFileSync(templatePath, 'utf8');
    const { compile } = jest.requireActual('@vue/compiler-dom') as any;
    const { compileScript, parse } = jest.requireActual('@vue/compiler-sfc') as any;
    const { descriptor, errors } = parse(source, { filename: sourcePath });
    if (errors.length) throw errors[0];
    const script = compileScript(descriptor, { id: 'desktop-user-basic-setting-tab-coverage' });
    const { code } = compile(template, {
        mode: 'function',
        prefixIdentifiers: true,
        bindingMetadata: script.bindings
    });
    const render = new Function('Vue', code)(mockActualVue);
    return { ...UserBasicSettingTab, render };
}

const RenderableUserBasicSettingTab = createRenderableUserBasicSettingTab();

function setup(): any {
    mockTemplateRefs.clear();
    const bindings = UserBasicSettingTab.setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    });
    bindings.confirmDialog.value = { open: mockConfirmOpen };
    bindings.snackbar.value = { showMessage: mockShowMessage, showError: mockShowError };
    bindings.avatarInput.value = { click: mockAvatarInputClick };
    return bindings;
}

async function flush(times = 10): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
}

function findNodes(node: HostNode, predicate: (candidate: HostNode) => boolean): HostNode[] {
    const matches = predicate(node) ? [node] : [];
    for (const child of node.children) matches.push(...findNodes(child, predicate));
    return matches;
}

async function invokeTemplateCallbacks(root: HostNode): Promise<void> {
    const avatarFile = new File(['avatar'], 'template-avatar.png', { type: 'image/png' });
    for (const { name, callback } of collectHostCallbacks(root)) {
        if (name.startsWith('onUpdate:')) {
            callback('template-value');
        } else if (name === 'onChange') {
            callback({ target: { files: [avatarFile], value: 'C:\\fakepath\\template-avatar.png' } });
        } else {
            callback({ preventDefault: jest.fn(), stopPropagation: jest.fn() });
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    mockVerifyEmailEnabled = true;
    mockProfileResponse = createProfile({
        accessToken: '<must-not-forward-access-token>',
        refreshToken: '<must-not-forward-refresh-token>',
        password: '<server-secret>',
        confirmPassword: '<server-secret>'
    });
    mockCategoriesStore.allTransactionCategories = {
        3: [{ id: 'transfer-primary', name: 'Transfer', subCategories: [] }]
    };
    mockCategoriesStore.hasAvailableTransferCategories = true;
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockCategoriesStore.loadAllCategories.mockResolvedValue(undefined);
    mockUserStore.getCurrentUserProfile.mockResolvedValue(mockProfileResponse);
    mockUserStore.updateUserAvatar.mockResolvedValue(createProfile({
        avatar: 'avatar-updated',
        avatarProvider: 'internal'
    }));
    mockUserStore.removeUserAvatar.mockResolvedValue(createProfile({
        avatar: '',
        avatarProvider: 'internal'
    }));
    mockRootStore.updateUserProfile.mockResolvedValue({
        user: createProfile({ nickname: 'Updated Alice' })
    });
    mockRootStore.resendVerifyEmailByLoginedUser.mockResolvedValue(undefined);
    mockConfirmOpen.mockResolvedValue(undefined);
    mockGetUserAvatarUrl.mockImplementation((avatar, cacheId) => (
        avatar ? `https://avatar.example.invalid/${avatar}?cache=${cacheId}` : null
    ));
    mockToProfileUpdateRequest.mockReturnValue({
        nickname: 'Alice Updated',
        email: 'alice-updated@example.invalid',
        password: '<new-password>'
    });
    mockCreateSelectionTexts.mockImplementation(() => ({
        defaultAccountSelectionText: mockActualVue.ref('Wallet'),
        cashAccountSelectionText: mockActualVue.ref('Cash'),
        cashTransferCategoryPrimaryText: mockActualVue.ref('Transfer'),
        cashTransferCategorySecondaryText: mockActualVue.ref('')
    }));
});

describe('desktop UserBasicSettingTab initialization and projection', () => {
    test('loads dependencies, sanitizes the profile through the base, and projects avatar/category state', async () => {
        const bindings = setup();
        expect(bindings.loading.value).toBe(true);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockCategoriesStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(mockUserStore.getCurrentUserProfile).toHaveBeenCalledTimes(1);
        await flush();

        expect(mockSetCurrentUserProfile).toHaveBeenCalledWith(mockProfileResponse);
        expect(bindings.loading.value).toBe(false);
        expect(bindings.avatarUrl.value).toBe('avatar-initial');
        expect(bindings.avatarProvider.value).toBe('internal');
        expect(bindings.currentUserAvatar.value).toContain('avatar-initial');
        expect(bindings.newProfile.value.password).toBe('');
        expect(bindings.newProfile.value.confirmPassword).toBe('');
        expect(bindings.newProfile.value.accessToken).toBeUndefined();
        expect(bindings.newProfile.value.refreshToken).toBeUndefined();
        expect(bindings.allCategories.value).toBe(mockCategoriesStore.allTransactionCategories);
        expect(bindings.hasAvailableTransferCategories.value).toBe(true);
        expect(mockCreateSelectionTexts).toHaveBeenCalledWith(expect.objectContaining({
            allAccounts: expect.anything(),
            allCategories: expect.anything(),
            newProfile: expect.anything(),
            tt: expect.any(Function)
        }));
        expect(mockExternalTemplateBindings).toHaveBeenCalled();
    });

    test('clears visible identity fields and maps only unprocessed initialization errors', async () => {
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({
            processed: true,
            message: 'handled initialization failure'
        });
        const processed = setup();
        await flush();
        expect(processed.loading.value).toBe(false);
        expect(processed.oldProfile.value).toMatchObject({ nickname: '', email: '' });
        expect(processed.newProfile.value).toMatchObject({ nickname: '', email: '' });
        expect(mockShowError).not.toHaveBeenCalled();

        const unprocessed = { processed: false, message: 'initialization failed' };
        mockUserStore.getCurrentUserProfile.mockRejectedValueOnce(unprocessed);
        setup();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(unprocessed);
    });
});

describe('desktop UserBasicSettingTab profile save', () => {
    test('maps validation messages in precedence order without sending profile data', async () => {
        const bindings = setup();
        await flush();
        const cases: Array<[string, string]> = [
            ['inputIsNotChangedProblemMessage', 'Nothing has been modified'],
            ['inputInvalidProblemMessage', 'Nickname cannot be blank'],
            ['extendInputInvalidProblemMessage', 'Extended setting invalid'],
            ['langAndRegionInputInvalidProblemMessage', 'Locale setting invalid']
        ];

        for (const [field, message] of cases) {
            for (const resetField of cases.map(item => item[0])) bindings[resetField].value = null;
            bindings[field].value = message;
            bindings.save();
            expect(mockShowMessage).toHaveBeenLastCalledWith(message);
        }
        expect(mockRootStore.updateUserProfile).not.toHaveBeenCalled();
    });

    test('sends only the profile update request and applies the returned user', async () => {
        const bindings = setup();
        await flush();
        bindings.newProfile.value.password = '<new-password>';
        bindings.newProfile.value.confirmPassword = '<new-password>';
        (bindings.newProfile.value as any).accessToken = '<must-not-forward>';

        bindings.save();
        expect(bindings.saving.value).toBe(true);
        expect(mockToProfileUpdateRequest).toHaveBeenCalledTimes(1);
        expect(mockRootStore.updateUserProfile).toHaveBeenCalledWith({
            nickname: 'Alice Updated',
            email: 'alice-updated@example.invalid',
            password: '<new-password>'
        });
        const payload = mockRootStore.updateUserProfile.mock.calls[0]?.[0];
        expect(payload).not.toHaveProperty('confirmPassword');
        expect(payload).not.toHaveProperty('accessToken');
        expect(payload).not.toHaveProperty('refreshToken');
        await flush();

        expect(bindings.saving.value).toBe(false);
        expect(mockDoAfterProfileUpdate).toHaveBeenCalledWith(expect.objectContaining({
            nickname: 'Updated Alice'
        }));
        expect(mockShowMessage).toHaveBeenCalledWith('Your profile has been successfully updated');
    });

    test('reports only unprocessed save failures and releases saving state', async () => {
        const bindings = setup();
        await flush();

        const processed = { processed: true, message: 'handled save failure' };
        mockRootStore.updateUserProfile.mockRejectedValueOnce(processed);
        bindings.save();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(processed);

        const unprocessed = { processed: false, message: 'save failed' };
        mockRootStore.updateUserProfile.mockRejectedValueOnce(unprocessed);
        bindings.save();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(unprocessed);
        expect(bindings.saving.value).toBe(false);
    });
});

describe('desktop UserBasicSettingTab avatar actions', () => {
    test('guards missing avatar input and uploads only the selected file', async () => {
        const bindings = setup();
        await flush();
        bindings.updateAvatar(null as never);
        bindings.updateAvatar({ target: null } as never);
        bindings.updateAvatar({ target: { files: null } } as never);
        bindings.updateAvatar({ target: { files: [] } } as never);
        bindings.updateAvatar({ target: { files: [undefined] } } as never);
        expect(mockUserStore.updateUserAvatar).not.toHaveBeenCalled();

        const avatarFile = new File(['avatar'], 'avatar.png', { type: 'image/png' });
        const input = { files: [avatarFile], value: 'C:\\fakepath\\avatar.png' };
        bindings.updateAvatar({ target: input } as never);
        expect(input.value).toBe('');
        expect(bindings.saving.value).toBe(true);
        expect(mockUserStore.updateUserAvatar).toHaveBeenCalledWith({ avatarFile });
        expect(mockUserStore.updateUserAvatar.mock.calls[0]?.[0]).not.toHaveProperty('password');
        await flush();

        expect(bindings.avatarUrl.value).toBe('avatar-updated');
        expect(bindings.avatarProvider.value).toBe('internal');
        expect(bindings.avatarNoCacheId.value).toBe('<avatar-cache-id>');
        expect(mockGenerateRandomUUID).toHaveBeenCalledTimes(1);
        expect(mockSetCurrentUserProfile).toHaveBeenLastCalledWith(expect.objectContaining({
            avatar: 'avatar-updated'
        }));
        expect(mockShowMessage).toHaveBeenCalledWith('Your avatar has been successfully updated');

        mockUserStore.updateUserAvatar.mockResolvedValueOnce(null);
        bindings.updateAvatar({ target: { files: [avatarFile], value: 'again' } } as never);
        await flush();
        expect(mockGenerateRandomUUID).toHaveBeenCalledTimes(1);
    });

    test('reports only unprocessed avatar upload failures', async () => {
        const bindings = setup();
        await flush();
        const avatarFile = new File(['avatar'], 'avatar.png', { type: 'image/png' });

        const processed = { processed: true, message: 'handled avatar failure' };
        mockUserStore.updateUserAvatar.mockRejectedValueOnce(processed);
        bindings.updateAvatar({ target: { files: [avatarFile], value: 'one' } } as never);
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(processed);

        const unprocessed = { processed: false, message: 'avatar failed' };
        mockUserStore.updateUserAvatar.mockRejectedValueOnce(unprocessed);
        bindings.updateAvatar({ target: { files: [avatarFile], value: 'two' } } as never);
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(unprocessed);
        expect(bindings.saving.value).toBe(false);
    });

    test('opens the file picker only when its template ref is available', async () => {
        const bindings = setup();
        await flush();
        bindings.showOpenAvatarDialog();
        expect(mockAvatarInputClick).toHaveBeenCalledTimes(1);
        bindings.avatarInput.value = null;
        expect(() => bindings.showOpenAvatarDialog()).not.toThrow();
    });
});

describe('desktop UserBasicSettingTab avatar removal and email verification', () => {
    test('removes avatar after confirmation and handles empty responses', async () => {
        const bindings = setup();
        await flush();
        bindings.confirmDialog.value = null;
        bindings.removeAvatar();
        expect(mockUserStore.removeUserAvatar).not.toHaveBeenCalled();

        bindings.confirmDialog.value = { open: mockConfirmOpen };
        bindings.removeAvatar();
        expect(mockConfirmOpen).toHaveBeenCalledWith('Are you sure you want to remove avatar?');
        await flush();
        expect(mockUserStore.removeUserAvatar).toHaveBeenCalledTimes(1);
        expect(bindings.avatarUrl.value).toBe('');
        expect(bindings.avatarProvider.value).toBe('internal');
        expect(mockShowMessage).toHaveBeenCalledWith('Your profile has been successfully updated');

        mockUserStore.removeUserAvatar.mockResolvedValueOnce(null);
        bindings.removeAvatar();
        await flush();
        expect(mockUserStore.removeUserAvatar).toHaveBeenCalledTimes(2);
    });

    test('reports only unprocessed avatar-removal failures', async () => {
        const bindings = setup();
        await flush();

        const processed = { processed: true, message: 'handled removal failure' };
        mockUserStore.removeUserAvatar.mockRejectedValueOnce(processed);
        bindings.removeAvatar();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(processed);

        const unprocessed = { processed: false, message: 'remove avatar failed' };
        mockUserStore.removeUserAvatar.mockRejectedValueOnce(unprocessed);
        bindings.removeAvatar();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(unprocessed);
        expect(bindings.saving.value).toBe(false);
    });

    test('resends validation email and maps processed and unprocessed failures', async () => {
        const bindings = setup();
        await flush();
        bindings.resendVerifyEmail();
        expect(bindings.resending.value).toBe(true);
        expect(mockRootStore.resendVerifyEmailByLoginedUser).toHaveBeenCalledWith();
        await flush();
        expect(bindings.resending.value).toBe(false);
        expect(mockShowMessage).toHaveBeenCalledWith('Validation email has been sent');

        const processed = { processed: true, message: 'handled resend failure' };
        mockRootStore.resendVerifyEmailByLoginedUser.mockRejectedValueOnce(processed);
        bindings.resendVerifyEmail();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(processed);

        const unprocessed = { processed: false, message: 'resend failed' };
        mockRootStore.resendVerifyEmailByLoginedUser.mockRejectedValueOnce(unprocessed);
        bindings.resendVerifyEmail();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(unprocessed);
        expect(bindings.resending.value).toBe(false);
    });
});

describe('desktop UserBasicSettingTab production template', () => {
    test('renders and executes loaded, input, loading, avatar, and verification states', async () => {
        const mounted = mountWithHostRenderer(RenderableUserBasicSettingTab, {}, [
            'v-row', 'v-col', 'v-card', 'v-card-text', 'v-progress-circular', 'v-avatar',
            'v-img', 'v-icon', 'v-menu', 'v-list', 'v-list-item', 'v-skeleton-loader',
            'v-btn', 'v-divider', 'v-form', 'v-text-field', 'two-column-select', 'v-select',
            'language-select', 'currency-select', 'fiscal-year-start-select'
        ]);
        try {
            expect(mounted.state.loading).toBe(true);
            await flush();
            expect(mounted.state.currentUserAvatar).toContain('avatar-initial');
            expect(mounted.state.avatarProvider).toBe('internal');
            expect(findNodes(mounted.root, node => node.props['accept'] === '.png,.jpg,.jpeg')).toHaveLength(1);

            const callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onClick', 'onChange', 'onUpdate:modelValue'
            ]));
            await invokeTemplateCallbacks(mounted.root);
            expect(mockRootStore.updateUserProfile).toHaveBeenCalled();
            expect(mockUserStore.updateUserAvatar).toHaveBeenCalled();
            expect(mockUserStore.removeUserAvatar).toHaveBeenCalled();
            expect(mockRootStore.resendVerifyEmailByLoginedUser).toHaveBeenCalled();
            expect(mockReset).toHaveBeenCalled();

            mounted.state.loading = true;
            mounted.state.saving = true;
            mounted.state.resending = true;
            mounted.state.emailVerified = true;
            mounted.state.avatarProvider = 'external';
            mounted.state.avatarUrl = '';
            mounted.state.allVisibleAccounts = [];
            mounted.state.supportDigitGroupingSymbol = false;
            mounted.state.inputIsNotChanged = true;
            mounted.state.inputIsInvalid = true;
            mockCategoriesStore.hasAvailableTransferCategories = false;
            mockCategoriesStore.allTransactionCategories = {};
            mockVerifyEmailEnabled = false;
            await flush();
            expect(mounted.state.avatarProvider).toBe('external');
            expect(mounted.state.currentUserAvatar).toBeNull();
            expect(mounted.state.loading).toBe(true);

            mounted.state.loading = false;
            mounted.state.saving = false;
            mounted.state.resending = false;
            mounted.state.emailVerified = false;
            mounted.state.avatarProvider = 'internal';
            mounted.state.inputIsNotChanged = false;
            mounted.state.inputIsInvalid = false;
            mockVerifyEmailEnabled = true;
            await flush();
            expect(mounted.state.avatarProvider).toBe('internal');
            expect(mounted.state.currentUserAvatar).toBeNull();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
