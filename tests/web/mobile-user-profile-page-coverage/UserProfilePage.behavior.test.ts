import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { collectHostCallbacks, mountWithHostRenderer } from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const mockShowAlert = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockRouteBackOnError = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockSetCurrentUserProfile = jest.fn<(...args: any[]) => void>();
const mockDoAfterProfileUpdate = jest.fn<(...args: any[]) => void>();
const mockToProfileUpdateRequest = jest.fn<(...args: any[]) => Record<string, unknown>>();
const mockFindDisplayNameByType = jest.fn<(...args: any[]) => string | null>();
const mockUseExternalTemplateBindings = jest.fn<(...args: any[]) => void>();
const mockFindAccountNameById = jest.fn<(...args: any[]) => string>();

let mockTextDirection = 'ltr';
let mockVerifyEmailEnabled = true;
let mockLastBase: ReturnType<typeof createProfileBase>;

const mockLanguages = [
    { languageTag: 'en', displayName: 'English', nativeDisplayName: 'English' },
    { languageTag: 'zh_CN', displayName: 'Chinese', nativeDisplayName: 'Simplified Chinese' }
];
const mockCurrencies = [
    { currencyCode: 'CNY', displayName: 'Chinese Yuan' },
    { currencyCode: 'USD', displayName: 'US Dollar' }
];
const mockAccounts = [{ id: 'account-1', name: 'Wallet' }];
const mockOptions = [{ type: 1, displayName: 'Option One' }];

const mockRootStore = {
    updateUserProfile: jest.fn<(...args: any[]) => Promise<any>>(),
    resendVerifyEmailByLoginedUser: jest.fn<(...args: any[]) => Promise<void>>()
};
const mockUserStore = {
    getCurrentUserProfile: jest.fn<(...args: any[]) => Promise<any>>()
};
const mockAccountsStore = {
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>()
};

function createProfile(overrides: Record<string, unknown> = {}): any {
    return {
        id: 7,
        username: 'synthetic-user',
        password: '',
        confirmPassword: '',
        email: 'synthetic@example.invalid',
        nickname: 'Synthetic User',
        defaultAccountId: 'account-1',
        transactionEditScope: 1,
        language: 'en',
        defaultCurrency: 'CNY',
        firstDayOfWeek: 1,
        fiscalYearStart: 101,
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
        incomeAmountColor: 1,
        emailVerified: false,
        noPassword: false,
        toProfileUpdateRequest: mockToProfileUpdateRequest,
        ...overrides
    };
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
        allAccounts: ref(mockAccounts),
        allVisibleAccounts: ref(mockAccounts),
        allVisibleCategorizedAccounts: ref([{ category: 'cash', accounts: mockAccounts }]),
        allWeekDays: ref([{ type: 1, displayName: 'Monday' }]),
        allCalendarDisplayTypes: ref(mockOptions),
        allDateDisplayTypes: ref(mockOptions),
        allLongDateFormats: ref(mockOptions),
        allShortDateFormats: ref(mockOptions),
        allLongTimeFormats: ref(mockOptions),
        allShortTimeFormats: ref(mockOptions),
        allFiscalYearFormats: ref(mockOptions),
        allCurrencyDisplayTypes: ref(mockOptions),
        allNumeralSystemTypes: ref(mockOptions),
        allDecimalSeparators: ref(mockOptions),
        allDigitGroupingSymbols: ref(mockOptions),
        allDigitGroupingTypes: ref(mockOptions),
        allCoordinateDisplayTypes: ref(mockOptions),
        allExpenseAmountColorTypes: ref(mockOptions),
        allIncomeAmountColorTypes: ref(mockOptions),
        allTransactionEditScopeTypes: ref(mockOptions),
        languageTitle: ref('Language'),
        supportDigitGroupingSymbol: ref(true),
        inputIsNotChangedProblemMessage: ref(null as string | null),
        inputInvalidProblemMessage: ref(null as string | null),
        langAndRegionInputInvalidProblemMessage: ref(null as string | null),
        extendInputInvalidProblemMessage: ref(null as string | null),
        inputIsNotChanged: ref(false),
        inputIsInvalid: ref(false),
        langAndRegionInputIsInvalid: ref(false),
        extendInputIsInvalid: ref(false),
        setCurrentUserProfile: mockSetCurrentUserProfile,
        doAfterProfileUpdate: mockDoAfterProfileUpdate
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageTextDirection: () => mockTextDirection,
        getAllLanguageOptions: (includeAuto: boolean) => includeAuto ? mockLanguages : [],
        getAllCurrencies: () => mockCurrencies,
        getCurrencyName: (currency: string) => `currency:${currency}`,
        formatFiscalYearStartToGregorianLikeLongMonth: (value: number) => `fiscal:${value}`
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showToast: mockShowToast,
        routeBackOnError: mockRouteBackOnError
    }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args)
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
jest.mock('@/core/text.ts', () => ({ TextDirection: { LeftToRight: 'ltr', RightToLeft: 'rtl' } }));
jest.mock('@/core/numeral.ts', () => ({
    NumeralSystem: {
        valueOf: (type: number) => type === 1 ? { type: 1, name: 'western' } : undefined,
        Default: { type: 0, name: 'default' }
    }
}));
jest.mock('@/models/account.ts', () => ({
    Account: { findAccountNameById: (...args: any[]) => mockFindAccountNameById(...args) }
}));
jest.mock('@/lib/common.ts', () => ({
    findDisplayNameByType: (...args: any[]) => mockFindDisplayNameByType(...args)
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isUserVerifyEmailEnabled: () => mockVerifyEmailEnabled
}));
jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: (...args: any[]) => mockUseExternalTemplateBindings(...args)
}));

import UserProfilePageComponent from '@/views/mobile/users/UserProfilePage.vue';

const UserProfilePage = UserProfilePageComponent as any;

function createRenderableUserProfilePage(): any {
    const sourcePath = resolve(
        __dirname,
        '../../../src/web/src/views/mobile/users/UserProfilePage.vue'
    );
    const templatePath = resolve(
        __dirname,
        '../../../src/web/src/views/mobile/users/profile/UserProfilePage.template.html'
    );
    const source = readFileSync(sourcePath, 'utf8');
    const template = readFileSync(templatePath, 'utf8');
    const { compile } = jest.requireActual('@vue/compiler-dom') as any;
    const { compileScript, parse } = jest.requireActual('@vue/compiler-sfc') as any;
    const { descriptor, errors } = parse(source, { filename: sourcePath });
    if (errors.length) throw errors[0];
    const script = compileScript(descriptor, { id: 'mobile-user-profile-page-coverage' });
    const { code } = compile(template, {
        mode: 'function',
        prefixIdentifiers: true,
        bindingMetadata: script.bindings
    });
    const render = new Function('Vue', code)(actualVue);
    return { ...UserProfilePage, render };
}

const RenderableUserProfilePage = createRenderableUserProfilePage();

function createRouter(): any {
    return { back: jest.fn(), navigate: jest.fn() };
}

function setup(): { bindings: any; router: any } {
    const router = createRouter();
    const bindings = UserProfilePage.setup(
        { f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() }
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTextDirection = 'ltr';
    mockVerifyEmailEnabled = true;
    mockToProfileUpdateRequest.mockImplementation(password => ({
        nickname: 'Synthetic User',
        currentPassword: password
    }));
    mockFindDisplayNameByType.mockImplementation((items: any[], type: number) => (
        items.find(item => item.type === type)?.displayName ?? null
    ));
    mockFindAccountNameById.mockReturnValue('Wallet');
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockUserStore.getCurrentUserProfile.mockResolvedValue(createProfile());
    mockRootStore.updateUserProfile.mockResolvedValue({ user: createProfile({ nickname: 'Updated User' }) });
    mockRootStore.resendVerifyEmailByLoginedUser.mockResolvedValue(undefined);
    mockShowLoading.mockImplementation((predicate?: () => unknown) => predicate?.());
});

describe('mobile UserProfilePage initialization and identity state', () => {
    test('loads accounts and profile together and projects passwordless identity', async () => {
        const profile = createProfile({ noPassword: true, emailVerified: true });
        mockUserStore.getCurrentUserProfile.mockResolvedValueOnce(profile);
        const { bindings } = setup();
        expect(bindings.loading.value).toBe(true);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockUserStore.getCurrentUserProfile).toHaveBeenCalledTimes(1);
        expect(bindings.allLanguages.value).toEqual(mockLanguages);
        expect(bindings.allCurrencies.value).toEqual(mockCurrencies);

        await flush();
        expect(mockSetCurrentUserProfile).toHaveBeenCalledWith(profile);
        expect(bindings.currentNoPassword.value).toBe(true);
        expect(bindings.loading.value).toBe(false);
        expect(bindings.currentLanguageName.value).toBe('English');
        expect(bindings.currentDayOfWeekName.value).toBe('Monday');
        expect(mockUseExternalTemplateBindings).toHaveBeenCalled();
    });

    test('distinguishes processed, readable, and raw initialization failures', async () => {
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({
            processed: true,
            message: 'handled account loading failure'
        });
        const processed = setup();
        await flush();
        expect(processed.bindings.loading.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalledWith('handled account loading failure');

        const readableError = { processed: false, message: 'profile loading failed' };
        mockUserStore.getCurrentUserProfile.mockRejectedValueOnce(readableError);
        const readable = setup();
        await flush();
        expect(readable.bindings.loadingError.value).toEqual(readableError);
        expect(mockShowToast).toHaveBeenCalledWith('profile loading failed');

        mockUserStore.getCurrentUserProfile.mockRejectedValueOnce('raw profile loading failure');
        const raw = setup();
        await flush();
        expect(raw.bindings.loadingError.value).toBe('raw profile loading failure');
        expect(mockShowToast).toHaveBeenCalledWith('raw profile loading failure');
    });

    test('routes the captured initialization error on page entry', async () => {
        const { bindings, router } = setup();
        await flush();
        bindings.loadingError.value = { message: 'synthetic profile error' };
        bindings.onPageAfterIn();
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });
});

describe('mobile UserProfilePage profile save', () => {
    test('reports each validation layer before opening password confirmation', async () => {
        const { bindings } = setup();
        await flush();
        const validationCases = [
            ['inputIsNotChangedProblemMessage', 'Nothing has been modified'],
            ['inputInvalidProblemMessage', 'Email address cannot be blank'],
            ['extendInputInvalidProblemMessage', 'Extended setting is invalid'],
            ['langAndRegionInputInvalidProblemMessage', 'Language setting is invalid']
        ] as const;

        for (const [field, message] of validationCases) {
            mockLastBase.inputIsNotChangedProblemMessage.value = null;
            mockLastBase.inputInvalidProblemMessage.value = null;
            mockLastBase.extendInputInvalidProblemMessage.value = null;
            mockLastBase.langAndRegionInputInvalidProblemMessage.value = null;
            mockLastBase[field].value = message;
            bindings.save();
            expect(mockShowAlert).toHaveBeenLastCalledWith(message);
        }
        expect(mockRootStore.updateUserProfile).not.toHaveBeenCalled();
    });

    test('requires confirmation for password changes but skips it for confirmed identity', async () => {
        const { bindings } = setup();
        await flush();
        bindings.newProfile.value.password = 'new-password';
        bindings.showInputPasswordSheet.value = false;
        bindings.save();
        expect(bindings.showInputPasswordSheet.value).toBe(true);
        expect(mockRootStore.updateUserProfile).not.toHaveBeenCalled();

        bindings.currentPassword.value = 'current-password';
        bindings.save(true);
        expect(mockToProfileUpdateRequest).toHaveBeenCalledWith('current-password', expect.any(Object));
        expect(mockRootStore.updateUserProfile).toHaveBeenCalledWith({
            nickname: 'Synthetic User',
            currentPassword: 'current-password'
        });
        await flush();
    });

    test('saves, synchronizes profile state, and navigates back when direction is stable', async () => {
        const response = { user: createProfile({ nickname: 'Updated User' }) };
        mockRootStore.updateUserProfile.mockResolvedValueOnce(response);
        const { bindings, router } = setup();
        await flush();
        bindings.currentPassword.value = 'current-password';

        bindings.save(true);
        expect(bindings.saving.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(expect.any(Function));
        await flush();

        expect(bindings.saving.value).toBe(false);
        expect(bindings.currentPassword.value).toBe('');
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockDoAfterProfileUpdate).toHaveBeenCalledWith(response.user);
        expect(mockShowToast).toHaveBeenCalledWith('Your profile has been successfully updated');
        expect(router.back).toHaveBeenCalledTimes(1);
    });

    test('does not navigate after a successful update changes text direction', async () => {
        const { bindings, router } = setup();
        await flush();
        mockRootStore.updateUserProfile.mockImplementationOnce(async () => {
            mockTextDirection = 'rtl';
            return { user: createProfile({ language: 'ar' }) };
        });

        bindings.save(true);
        await flush();
        expect(mockDoAfterProfileUpdate).toHaveBeenCalled();
        expect(router.back).not.toHaveBeenCalled();
    });

    test('cleans up and reports only unprocessed readable or raw save failures', async () => {
        const { bindings } = setup();
        await flush();
        bindings.currentPassword.value = 'current-password';

        mockRootStore.updateUserProfile.mockRejectedValueOnce({
            processed: true,
            message: 'handled profile update failure'
        });
        bindings.save(true);
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled profile update failure');
        expect(bindings.currentPassword.value).toBe('');

        mockRootStore.updateUserProfile.mockRejectedValueOnce({
            processed: false,
            message: 'profile update failed'
        });
        bindings.save(true);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('profile update failed');

        mockRootStore.updateUserProfile.mockRejectedValueOnce('raw profile update failure');
        bindings.save(true);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw profile update failure');
        expect(bindings.saving.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalledTimes(3);
    });
});

describe('mobile UserProfilePage email verification', () => {
    test('resends validation email and releases loading state', async () => {
        const { bindings } = setup();
        await flush();
        bindings.resendVerifyEmail();
        expect(bindings.resending.value).toBe(true);
        expect(mockRootStore.resendVerifyEmailByLoginedUser).toHaveBeenCalledTimes(1);
        await flush();
        expect(bindings.resending.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('Validation email has been sent');
    });

    test('reports readable and raw resend failures but suppresses processed failures', async () => {
        const { bindings } = setup();
        await flush();
        const failures = [
            { value: { processed: true, message: 'handled resend failure' }, toast: null },
            { value: { processed: false, message: 'resend failed' }, toast: 'resend failed' },
            { value: 'raw resend failure', toast: 'raw resend failure' }
        ];

        for (const failure of failures) {
            mockRootStore.resendVerifyEmailByLoginedUser.mockRejectedValueOnce(failure.value);
            bindings.resendVerifyEmail();
            await flush();
            expect(bindings.resending.value).toBe(false);
            if (failure.toast) expect(mockShowToast).toHaveBeenCalledWith(failure.toast);
            else expect(mockShowToast).not.toHaveBeenCalledWith('handled resend failure');
        }
    });
});

describe('mobile UserProfilePage production template', () => {
    test('renders loading and editable identity states and executes Framework7 events', async () => {
        const router = createRouter();
        const mounted = mountWithHostRenderer(RenderableUserProfilePage, { f7router: router }, [
            'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
            'f7-list', 'f7-list-input', 'f7-list-item', 'f7-block', 'f7-actions',
            'f7-actions-group', 'f7-actions-button', 'two-column-list-item-selection-sheet',
            'list-item-selection-popup', 'fiscal-year-start-selection-sheet', 'password-input-sheet'
        ]);
        try {
            expect(mounted.state.loading).toBe(true);
            expect(mounted.root.children.length).toBeGreaterThan(0);
            await flush();

            mounted.state.loading = false;
            mounted.state.emailVerified = false;
            mounted.state.inputIsNotChanged = false;
            mounted.state.inputIsInvalid = true;
            mounted.state.extendInputIsInvalid = true;
            mounted.state.langAndRegionInputIsInvalid = true;
            mounted.state.currentNoPassword = true;
            mounted.state.showInputPasswordSheet = true;
            mounted.state.showMoreActionSheet = true;
            mounted.state.newProfile.numeralSystem = 999;
            await actualVue.nextTick();

            const callbacks = collectHostCallbacks(mounted.root);
            expect(callbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onPage:afterin', 'onClick', 'onUpdate:value', 'onUpdate:show',
                'onUpdate:modelValue', 'onActions:closed', 'onPassword:confirm'
            ]));
            for (const { name, callback } of callbacks) {
                if (name === 'onUpdate:value') callback('synthetic-value');
                else if (name === 'onUpdate:show') callback(false);
                else if (name === 'onUpdate:modelValue') callback(1);
                else if (name === 'onPassword:confirm') callback('current-password');
                else callback();
                await flush(1);
            }

            mounted.state.emailVerified = true;
            mounted.state.inputIsInvalid = false;
            mounted.state.extendInputIsInvalid = false;
            mounted.state.langAndRegionInputIsInvalid = false;
            mounted.state.supportDigitGroupingSymbol = false;
            mounted.state.saving = true;
            mounted.state.resending = true;
            mockVerifyEmailEnabled = false;
            await actualVue.nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
