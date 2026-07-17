import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { collectHostCallbacks, mountWithHostRenderer } from './hostRenderer';

const actualVue = jest.requireActual('vue') as any;
const mockRouter = {
    push: jest.fn<(...args: any[]) => void>(),
    replace: jest.fn<(...args: any[]) => void>()
};
const mockThemeName = actualVue.ref('light');
const mockShowMessage = jest.fn<(...args: any[]) => void>();
const mockShowError = jest.fn<(...args: any[]) => void>();
const mockDoAfterSignupSuccess = jest.fn<(...args: any[]) => void>();
const mockCategorizedArrayToPlainArray = jest.fn<(...args: any[]) => any[]>();
const mockRegister = jest.fn<(...args: any[]) => Promise<any>>();

let mockLoggedIn = false;
let mockLastBase: ReturnType<typeof createSignupBase>;

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
                { id: 'breakfast', name: 'Breakfast', icon: 'sunrise', color: '#ffaa00' },
                { id: 'dinner', name: 'Dinner', icon: 'moon', color: '#663399' }
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
        inputEmptyProblemMessage: ref(null as string | null),
        inputInvalidProblemMessage: ref(null as string | null),
        getCategoryTypeName: jest.fn((type: string | number) => `category-type:${type}`),
        doAfterSignupSuccess: mockDoAfterSignupSuccess
    };
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});
jest.mock('vue-router', () => ({ useRouter: () => mockRouter }));
jest.mock('vuetify', () => ({ useTheme: () => ({ global: { name: mockThemeName } }) }));
jest.mock('@/components/desktop/SnackBar.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return {
        __esModule: true,
        default: defineComponent({
            name: 'SignupSnackBarStub',
            emits: ['update:show'],
            setup: (_props: unknown, { expose }: any) => {
                expose({ showMessage: mockShowMessage, showError: mockShowError });
                return () => h('snack-bar-stub');
            }
        })
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getAllWeekDays: () => mockWeekDays,
        getAllTransactionDefaultCategories: (_depth: number, locale: string) => (
            locale === 'empty' ? {} : mockPresetCategories
        )
    })
}));
jest.mock('@/views/base/SignupPageBase.ts', () => ({
    useSignupPageBase: () => {
        mockLastBase = createSignupBase();
        return mockLastBase;
    }
}));
jest.mock('@/stores/index.ts', () => ({ useRootStore: () => ({ register: mockRegister }) }));
jest.mock('@/core/category.ts', () => ({ CategoryType: { Income: 1, Expense: 2 } }));
jest.mock('@/core/theme.ts', () => ({ isDarkApplicationTheme: (name: string) => name === 'dark' }));
jest.mock('@/consts/asset.ts', () => ({ APPLICATION_LOGO_PATH: '/img/synthetic-logo.svg' }));
jest.mock('@/lib/common.ts', () => ({
    categorizedArrayToPlainArray: (...args: any[]) => mockCategorizedArrayToPlainArray(...args)
}));
jest.mock('@/lib/userstate.ts', () => ({ isUserLogined: () => mockLoggedIn }));
jest.mock('@mdi/js', () => ({
    mdiArrowLeft: 'arrow-left',
    mdiArrowRight: 'arrow-right',
    mdiCheck: 'check'
}));

const SignupPage = require('@/views/desktop/SignupPage.vue').default as any;

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function setup(): { bindings: any } {
    const bindings = SignupPage.setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn()
    });
    bindings.snackbar.value = { showMessage: mockShowMessage, showError: mockShowError };
    return { bindings };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockLoggedIn = false;
    mockThemeName.value = 'light';
    mockRegister.mockResolvedValue({
        presetCategoriesSaved: true,
        needVerifyEmail: false,
        user: { id: 'synthetic-user-id' }
    });
    mockCategorizedArrayToPlainArray.mockReturnValue([
        mockPresetCategories[1][0],
        mockPresetCategories[2][0]
    ]);
});

describe('desktop SignupPage navigation and computed state', () => {
    test('projects locale options, preset categories, theme, and completion step', () => {
        const { bindings } = setup();
        expect(bindings.allWeekDays.value).toEqual(mockWeekDays);
        expect(bindings.allPresetCategories.value).toEqual(mockPresetCategories);
        expect(bindings.isDarkMode.value).toBe(false);
        expect(bindings.allSteps.value.map((step: any) => step.name)).toEqual([
            'basicSetting', 'presetCategories'
        ]);

        mockThemeName.value = 'dark';
        expect(bindings.isDarkMode.value).toBe(true);
        bindings.currentLocale.value = 'empty';
        expect(bindings.allPresetCategories.value).toEqual({});
        bindings.finalResultMessage.value = 'Registration completed';
        expect(bindings.allSteps.value.map((step: any) => step.name)).toEqual([
            'basicSetting', 'presetCategories', 'finalResult'
        ]);
    });

    test('validates forward navigation and honors busy, final, and redirect guards', () => {
        const { bindings } = setup();
        bindings.inputEmptyProblemMessage.value = 'Username cannot be blank';
        bindings.switchToNextTab();
        expect(mockShowMessage).toHaveBeenCalledWith('Username cannot be blank');
        expect(bindings.currentStep.value).toBe('basicSetting');

        bindings.inputEmptyProblemMessage.value = null;
        bindings.inputInvalidProblemMessage.value = 'Password mismatch';
        bindings.switchToTab('presetCategories');
        expect(mockShowMessage).toHaveBeenCalledWith('Password mismatch');

        bindings.inputInvalidProblemMessage.value = null;
        bindings.switchToNextTab();
        expect(bindings.currentStep.value).toBe('presetCategories');
        bindings.switchToPreviousTab();
        expect(bindings.currentStep.value).toBe('basicSetting');

        bindings.switchToTab('unknown-step');
        expect(bindings.currentStep.value).toBe('basicSetting');
        bindings.submitting.value = true;
        bindings.switchToTab('presetCategories');
        expect(bindings.currentStep.value).toBe('basicSetting');
        bindings.submitting.value = false;
        bindings.currentStep.value = 'finalResult';
        bindings.switchToTab('basicSetting');
        expect(bindings.currentStep.value).toBe('finalResult');
        bindings.currentStep.value = 'basicSetting';
        bindings.navigateToHomePage.value = true;
        bindings.switchToTab('presetCategories');
        expect(bindings.currentStep.value).toBe('basicSetting');
    });

    test('navigates through explicit continue and snackbar-close paths only', () => {
        const { bindings } = setup();
        bindings.navigateToLogin();
        expect(mockRouter.push).toHaveBeenCalledWith('/');

        bindings.onSnackbarShowStateChanged(false);
        bindings.navigateToHomePage.value = true;
        bindings.onSnackbarShowStateChanged(true);
        expect(mockRouter.replace).not.toHaveBeenCalled();
        bindings.onSnackbarShowStateChanged(false);
        expect(mockRouter.replace).toHaveBeenCalledWith('/');
    });
});

describe('desktop SignupPage registration outcomes', () => {
    test('blocks invalid submission and sends no preset package by default', async () => {
        const { bindings } = setup();
        bindings.inputEmptyProblemMessage.value = 'Email cannot be blank';
        bindings.submit();
        expect(mockShowMessage).toHaveBeenCalledWith('Email cannot be blank');
        expect(mockRegister).not.toHaveBeenCalled();

        bindings.inputEmptyProblemMessage.value = null;
        bindings.submit();
        expect(mockRegister).toHaveBeenCalledWith({
            user: mockLastBase.user.value,
            presetCategories: [],
            defaultPackage: undefined
        });
        await flush();
        expect(mockShowMessage).toHaveBeenCalledWith('You have been successfully registered');
        expect(bindings.navigateToHomePage.value).toBe(true);
        expect(bindings.submitting.value).toBe(false);
    });

    test('flattens selected preset categories and requests the standard daily package', async () => {
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

    test('shows a final result when preset persistence fails for an unlogged user', async () => {
        mockRegister.mockResolvedValueOnce({ presetCategoriesSaved: false, needVerifyEmail: false });
        const { bindings } = setup();
        bindings.usePresetCategories.value = true;
        bindings.submit();
        await flush();
        expect(bindings.currentStep.value).toBe('finalResult');
        expect(bindings.finalResultMessage.value).toContain('failure when adding preset categories');
        expect(bindings.navigateToHomePage.value).toBe(false);
    });

    test('shows email-verification completion before allowing navigation', async () => {
        mockRegister.mockResolvedValueOnce({ presetCategoriesSaved: true, needVerifyEmail: true });
        const { bindings } = setup();
        bindings.submit();
        await flush();
        expect(bindings.currentStep.value).toBe('finalResult');
        expect(bindings.finalResultMessage.value).toContain('activation link');
        expect(mockRouter.replace).not.toHaveBeenCalled();
    });

    test('completes logged-in registration and handles both preset result messages', async () => {
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
        expect(mockShowMessage).toHaveBeenCalledWith('You have been successfully registered');
        expect(mockRouter.replace).toHaveBeenCalledWith('/');
        expect(success.bindings.navigateToHomePage.value).toBe(true);

        mockRegister.mockResolvedValueOnce({ ...response, presetCategoriesSaved: false });
        const partial = setup();
        partial.bindings.usePresetCategories.value = true;
        partial.bindings.submit();
        await flush();
        expect(mockShowMessage).toHaveBeenCalledWith(expect.stringContaining('failure when adding preset categories'));
        expect(partial.bindings.navigateToHomePage.value).toBe(true);
    });

    test('reports only unprocessed registration errors and always releases submitting state', async () => {
        const { bindings } = setup();
        mockRegister.mockRejectedValueOnce({ processed: false, message: 'registration failed' });
        bindings.submit();
        await flush();
        expect(mockShowError).toHaveBeenCalledWith(expect.objectContaining({ message: 'registration failed' }));
        expect(bindings.submitting.value).toBe(false);

        mockRegister.mockRejectedValueOnce({ processed: true, message: 'handled registration failure' });
        bindings.submit();
        await flush();
        expect(mockShowError).not.toHaveBeenCalledWith(expect.objectContaining({ message: 'handled registration failure' }));
        expect(bindings.submitting.value).toBe(false);
    });
});

describe('desktop SignupPage production template', () => {
    test('renders and executes basic, preset, busy, dark, and completion states', async () => {
        const mounted = mountWithHostRenderer(SignupPage, {}, [
            'router-link', 'v-row', 'v-col', 'v-img', 'v-card', 'steps-bar', 'v-window', 'v-form',
            'v-window-item', 'v-text-field', 'language-select', 'currency-select', 'v-select', 'v-switch',
            'language-select-button', 'v-expansion-panels', 'v-expansion-panel', 'v-expansion-panel-title',
            'v-expansion-panel-text', 'v-list', 'v-list-item', 'v-divider', 'v-btn', 'v-progress-circular',
            'item-icon'
        ]);
        try {
            await actualVue.nextTick();
            const basicCallbacks = collectHostCallbacks(mounted.root);
            expect(basicCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onStep:change', 'onUpdate:modelValue', 'onClick'
            ]));
            for (const { name, callback } of basicCallbacks) {
                if (name === 'onStep:change') callback('presetCategories');
                else if (name === 'onUpdate:modelValue') callback('synthetic-value');
                else if (name === 'onClick') callback();
                await flush(1);
            }

            mounted.state.currentStep = 'presetCategories';
            mounted.state.usePresetCategories = true;
            mounted.state.useStandardDailyPackage = true;
            await actualVue.nextTick();
            const presetCallbacks = collectHostCallbacks(mounted.root);
            expect(presetCallbacks.map(item => item.name)).toEqual(expect.arrayContaining([
                'onUpdate:modelValue', 'onClick'
            ]));
            for (const { name, callback } of presetCallbacks) {
                if (name === 'onStep:change') callback('basicSetting');
                else if (name === 'onUpdate:modelValue') callback(true);
                else if (name === 'onClick') callback();
                await flush(1);
            }

            mounted.state.finalResultMessage = 'Synthetic registration complete';
            mounted.state.currentStep = 'finalResult';
            mounted.state.submitting = false;
            mounted.state.navigateToHomePage = false;
            mockThemeName.value = 'dark';
            await actualVue.nextTick();
            const finalCallbacks = collectHostCallbacks(mounted.root);
            for (const { name, callback } of finalCallbacks) {
                if (name === 'onUpdate:show') callback(false);
                else if (name === 'onClick') callback();
                await flush(1);
            }

            mounted.state.currentStep = 'presetCategories';
            mounted.state.submitting = true;
            mounted.state.navigateToHomePage = true;
            mounted.state.usePresetCategories = false;
            await actualVue.nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
