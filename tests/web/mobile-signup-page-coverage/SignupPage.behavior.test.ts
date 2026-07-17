import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from '../coverage-auth-mobile-batch1/hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const mockShowAlert = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockDoAfterSignupSuccess = jest.fn<(...args: any[]) => void>();
const mockFindDisplayNameByType = jest.fn<(...args: any[]) => string | null>();
const mockCategorizedArrayToPlainArray = jest.fn<(...args: any[]) => any[]>();
const mockRegister = jest.fn<(...args: any[]) => Promise<any>>();

let mockLoggedIn = false;
let mockLastBase: ReturnType<typeof createSignupBase>;

const mockLanguages = [
    { languageTag: 'en', displayName: 'English', nativeDisplayName: 'English' },
    { languageTag: 'zh_CN', displayName: 'Chinese', nativeDisplayName: 'Simplified Chinese' }
];
const mockCurrencies = [
    { currencyCode: 'CNY', displayName: 'Chinese Yuan' },
    { currencyCode: 'USD', displayName: 'US Dollar' }
];
const mockWeekDays = [
    { type: 1, displayName: 'Monday' },
    { type: 7, displayName: 'Sunday' }
];
const mockPresetCategories = {
    1: [
        {
            id: 'salary',
            name: 'Salary',
            icon: 'wallet',
            color: '#008800',
            subCategories: []
        }
    ],
    2: [
        {
            id: 'food',
            name: 'Food',
            icon: 'fork',
            color: '#cc5500',
            subCategories: [
                { id: 'breakfast', name: 'Breakfast', icon: 'sunrise', color: '#ffaa00' }
            ]
        }
    ]
};

function createSignupBase(): any {
    const { ref } = jest.requireActual('vue') as any;
    return {
        user: ref({
            username: 'synthetic-user',
            nickname: 'Synthetic User',
            email: 'synthetic@example.invalid',
            password: 'synthetic-password',
            confirmPassword: 'synthetic-password',
            defaultCurrency: 'CNY',
            firstDayOfWeek: 1
        }),
        submitting: ref(false),
        languageTitle: ref('Language'),
        currentLocale: ref('en'),
        currentLanguageName: ref('English'),
        inputEmptyProblemMessage: ref(null as string | null),
        inputInvalidProblemMessage: ref(null as string | null),
        inputIsEmpty: ref(false),
        inputIsInvalid: ref(false),
        getCategoryTypeName: jest.fn((type: string | number) => `category-type:${type}`),
        doAfterSignupSuccess: mockDoAfterSignupSuccess
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getAllLanguageOptions: (includeAuto: boolean) => includeAuto ? [] : mockLanguages,
        getAllCurrencies: () => mockCurrencies,
        getAllWeekDays: () => mockWeekDays,
        getAllTransactionDefaultCategories: (_depth: number, locale: string) => (
            locale === 'empty' ? {} : mockPresetCategories
        ),
        getCurrencyName: (currency: string) => `currency:${currency}`
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showToast: mockShowToast
    }),
    showLoading: (...args: any[]) => mockShowLoading(...args),
    hideLoading: (...args: any[]) => mockHideLoading(...args)
}));
jest.mock('@/views/base/SignupPageBase.ts', () => ({
    useSignupPageBase: () => {
        mockLastBase = createSignupBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => ({ register: mockRegister }) }));
jest.mock('@/core/category.ts', () => ({ CategoryType: { Income: 1, Expense: 2 } }));
jest.mock('@/lib/common.ts', () => ({
    findDisplayNameByType: (...args: any[]) => mockFindDisplayNameByType(...args),
    categorizedArrayToPlainArray: (...args: any[]) => mockCategorizedArrayToPlainArray(...args)
}));
jest.mock('@/lib/userstate.ts', () => ({ isUserLogined: () => mockLoggedIn }));

import SignupPageComponent from '@/views/mobile/SignupPage.vue';

const SignupPage = SignupPageComponent as any;

function setup(): { bindings: any; router: any } {
    const router = { navigate: jest.fn() };
    const bindings = SignupPage.setup(
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
    mockLoggedIn = false;
    mockFindDisplayNameByType.mockImplementation((items: any[], type: number) => (
        items.find(item => item.type === type)?.displayName ?? null
    ));
    mockCategorizedArrayToPlainArray.mockReturnValue([
        mockPresetCategories[1][0],
        mockPresetCategories[2][0]
    ]);
    mockRegister.mockResolvedValue({
        presetCategoriesSaved: true,
        needVerifyEmail: false,
        user: { id: 'synthetic-user-id' }
    });
    mockShowLoading.mockImplementation((predicate?: () => unknown) => predicate?.());
});

describe('mobile SignupPage state and registration', () => {
    test('projects language, currency, weekday, and locale-sensitive category options', () => {
        const { bindings } = setup();
        expect(bindings.allLanguages.value).toEqual(mockLanguages);
        expect(bindings.allCurrencies.value).toEqual(mockCurrencies);
        expect(bindings.allWeekDays.value).toEqual(mockWeekDays);
        expect(bindings.allPresetCategories.value).toEqual(mockPresetCategories);
        expect(bindings.currentDayOfWeekName.value).toBe('Monday');
        expect(mockFindDisplayNameByType).toHaveBeenCalledWith(mockWeekDays, 1);

        bindings.currentLocale.value = 'empty';
        expect(bindings.allPresetCategories.value).toEqual({});
    });

    test('blocks empty and invalid submissions before loading starts', () => {
        const { bindings } = setup();
        bindings.inputEmptyProblemMessage.value = 'Username cannot be blank';
        bindings.submit();
        expect(mockShowAlert).toHaveBeenLastCalledWith('Username cannot be blank');

        bindings.inputEmptyProblemMessage.value = null;
        bindings.inputInvalidProblemMessage.value = 'Password mismatch';
        bindings.submit();
        expect(mockShowAlert).toHaveBeenLastCalledWith('Password mismatch');
        expect(mockRegister).not.toHaveBeenCalled();
        expect(mockShowLoading).not.toHaveBeenCalled();
    });

    test('registers without optional presets and navigates after ordinary success', async () => {
        const { bindings, router } = setup();
        bindings.submit();
        expect(bindings.submitting.value).toBe(true);
        expect(mockShowLoading).toHaveBeenCalledWith(expect.any(Function));
        expect(mockRegister).toHaveBeenCalledWith({
            user: mockLastBase.user.value,
            presetCategories: [],
            defaultPackage: undefined
        });

        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('You have been successfully registered');
        expect(router.navigate).toHaveBeenCalledWith('/');
        expect(bindings.submitting.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();
    });

    test('flattens selected presets and opts into the standard daily package', async () => {
        const { bindings } = setup();
        bindings.usePresetCategories.value = true;
        bindings.useStandardDailyPackage.value = true;
        bindings.submit();

        expect(mockCategorizedArrayToPlainArray).toHaveBeenCalledWith(mockPresetCategories);
        expect(mockRegister).toHaveBeenCalledWith(expect.objectContaining({
            presetCategories: [mockPresetCategories[1][0], mockPresetCategories[2][0]],
            defaultPackage: 'standard_daily_v1'
        }));
        await flush();
    });

    test('distinguishes preset persistence and email verification for logged-out users', async () => {
        mockRegister.mockResolvedValueOnce({ presetCategoriesSaved: false, needVerifyEmail: false });
        const presetFailure = setup();
        presetFailure.bindings.usePresetCategories.value = true;
        presetFailure.bindings.submit();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith(
            expect.stringContaining('failure when adding preset categories'),
            5000
        );
        expect(presetFailure.router.navigate).toHaveBeenCalledWith('/');

        mockRegister.mockResolvedValueOnce({ presetCategoriesSaved: true, needVerifyEmail: true });
        const verification = setup();
        verification.bindings.submit();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith(expect.stringContaining('activation link'), 5000);
        expect(verification.router.navigate).toHaveBeenCalledWith('/');
    });

    test('runs logged-in post-registration behavior for complete and partial preset outcomes', async () => {
        mockLoggedIn = true;
        const response = {
            user: { id: 'logged-in-user' },
            presetCategoriesSaved: true,
            needVerifyEmail: false
        };
        mockRegister.mockResolvedValueOnce(response);
        const success = setup();
        success.bindings.submit();
        await flush();
        expect(mockDoAfterSignupSuccess).toHaveBeenCalledWith(response);
        expect(mockShowToast).toHaveBeenCalledWith('You have been successfully registered');
        expect(success.router.navigate).toHaveBeenCalledWith('/');

        mockRegister.mockResolvedValueOnce({ ...response, presetCategoriesSaved: false });
        const partial = setup();
        partial.bindings.usePresetCategories.value = true;
        partial.bindings.submit();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith(
            expect.stringContaining('failure when adding preset categories')
        );
        expect(partial.router.navigate).toHaveBeenCalledWith('/');
    });

    test('reports only unprocessed failures and supports readable and raw errors', async () => {
        const { bindings } = setup();
        mockRegister.mockRejectedValueOnce({ processed: false, message: 'registration failed' });
        bindings.submit();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('registration failed');
        expect(bindings.submitting.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();

        mockRegister.mockRejectedValueOnce({ processed: false });
        bindings.submit();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith(expect.objectContaining({ processed: false }));

        mockRegister.mockRejectedValueOnce({ processed: true, message: 'handled registration failure' });
        bindings.submit();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled registration failure');
        expect(bindings.submitting.value).toBe(false);
    });
});

describe('mobile SignupPage production template', () => {
    test('renders all conditional surfaces and executes visible event wrappers', async () => {
        const router = { navigate: jest.fn() };
        const mounted = mountWithHostRenderer(SignupPage, { f7router: router }, [
            'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
            'f7-list', 'f7-list-input', 'f7-list-item', 'f7-block', 'f7-toggle', 'f7-popup',
            'f7-block-title', 'f7-accordion-content', 'f7-actions', 'f7-actions-group',
            'f7-actions-button', 'list-item-selection-popup', 'list-item-selection-sheet', 'item-icon'
        ]);
        try {
            mounted.state.inputIsInvalid = true;
            mounted.state.usePresetCategories = true;
            mounted.state.useStandardDailyPackage = true;
            mounted.state.showLanguagePopup = true;
            mounted.state.showDefaultCurrencyPopup = true;
            mounted.state.showFirstDayOfWeekPopup = true;
            mounted.state.showPresetCategories = true;
            mounted.state.showPresetCategoriesMoreActionSheet = true;
            mounted.state.showPresetCategoriesChangeLocaleSheet = true;
            await actualVue.nextTick();

            const enabledCallbacks = collectHostCallbacks(mounted.root);
            expect(enabledCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onClick', 'onUpdate:value', 'onUpdate:show', 'onUpdate:modelValue',
                'onToggle:change', 'onPopup:closed', 'onActions:closed'
            ]));
            for (const { name, callback } of enabledCallbacks) {
                if (name === 'onUpdate:value') callback('synthetic-value');
                else if (name === 'onUpdate:show') callback(false);
                else if (name === 'onUpdate:modelValue') callback('en');
                else if (name === 'onToggle:change') callback(false);
                else callback();
                await flush(1);
            }

            mounted.state.inputIsInvalid = false;
            mounted.state.usePresetCategories = false;
            mounted.state.showPresetCategories = true;
            mounted.state.showPresetCategoriesMoreActionSheet = false;
            mounted.state.showPresetCategoriesChangeLocaleSheet = false;
            await actualVue.nextTick();
            const disabledCallbacks = collectHostCallbacks(mounted.root);
            for (const { name, callback } of disabledCallbacks) {
                if (name === 'onUpdate:value') callback('synthetic-value');
                else if (name === 'onUpdate:show') callback(true);
                else if (name === 'onUpdate:modelValue') callback('zh_CN');
                else if (name === 'onToggle:change') callback(true);
                else callback();
                await flush(1);
            }
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
