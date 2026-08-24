import { jest } from '@jest/globals';

let mockLoggedIn = false;
let mockUnlocked = false;
let mockRouterOptions: any;

const desktopViews = [
    'MainLayout', 'LoginPage', 'SignupPage', 'VerifyEmailPage', 'ForgetPasswordPage',
    'ResetPasswordPage', 'OAuth2CallbackPage', 'UnlockPage', 'HomePage',
    'transactions/ListPage', 'statistics/TransactionPage', 'accounts/ListPage',
    'categories/ListPage', 'tags/ListPage', 'templates/ListPage',
    'user/UserSettingsPage', 'app/AppSettingsPage', 'exchangerates/ListPage',
    'AboutPage', 'budgets/ListPage', 'pairingcenter/ListPage', 'recurring/DiscoverPage',
    'insights/InsightsPage'
];

for (const view of desktopViews) {
    jest.doMock(`@/views/desktop/${view}.vue`, () => ({
        __esModule: true,
        default: { name: `Stub:${view}` }
    }));
}

jest.doMock('vue-router', () => ({
    createWebHashHistory: () => 'synthetic-history',
    createRouter: (options: any) => {
        mockRouterOptions = options;
        return { options };
    }
}));
jest.doMock('@/lib/userstate.ts', () => ({
    isUserLogined: () => mockLoggedIn,
    isUserUnlocked: () => mockUnlocked
}));
jest.doMock('@/core/template.ts', () => ({
    TemplateType: { Normal: { type: 1 }, Schedule: { type: 2 } }
}));

const desktopRouter = (jest.requireActual('@/router/desktop.ts') as any).default;

function route(path: string): any {
    const topLevel = mockRouterOptions.routes.find((item: any) => item.path === path);
    if (topLevel) return topLevel;
    return mockRouterOptions.routes[0].children.find((item: any) => item.path === path);
}

beforeEach(() => {
    mockLoggedIn = false;
    mockUnlocked = false;
});

describe('desktop router', () => {
    test('creates hash router with the declared authenticated route tree', () => {
        expect(desktopRouter.options).toBe(mockRouterOptions);
        expect(mockRouterOptions.history).toBe('synthetic-history');
        expect(route('/').children.length).toBeGreaterThan(10);
        expect(route('/template/list').props).toEqual({ initType: 1 });
        expect(route('/schedule/list').props).toEqual({ initType: 2 });
        expect(route('/action-center').component).toEqual(expect.objectContaining({
            name: 'Stub:insights/InsightsPage'
        }));
    });

    test('protects signed-in routes for logged-out, locked, and unlocked states', () => {
        const guard = route('/').beforeEnter;
        expect(guard()).toEqual({ path: '/login', replace: true });

        mockLoggedIn = true;
        expect(guard()).toEqual({ path: '/unlock', replace: true });

        mockUnlocked = true;
        expect(guard()).toBe(true);
    });

    test('protects unlock and public auth routes according to session state', () => {
        const unlockGuard = route('/unlock').beforeEnter;
        const loginGuard = route('/login').beforeEnter;

        expect(unlockGuard()).toEqual({ path: '/login', replace: true });
        expect(loginGuard()).toBe(true);

        mockLoggedIn = true;
        expect(unlockGuard()).toBe(true);
        expect(loginGuard()).toEqual({ path: '/unlock', replace: true });

        mockUnlocked = true;
        expect(unlockGuard()).toEqual({ path: '/', replace: true });
        expect(loginGuard()).toEqual({ path: '/', replace: true });
    });

    test('projects query parameters into transaction, statistics, budget, pairing, and settings props', () => {
        const query = new Proxy({}, { get: (_target, key) => `query:${String(key)}` });
        const routeLike = { query };

        expect(route('/transaction/list').props(routeLike)).toEqual(expect.objectContaining({
            initPageType: 'query:pageType',
            initAmountFilterCents: 'query:amountFilterCents',
            initKeyword: 'query:keyword'
        }));
        expect(route('/statistics/transaction').props(routeLike)).toEqual(expect.objectContaining({
            initAnalysisType: 'query:analysisType',
            initTrendDateAggregationType: 'query:trendDateAggregationType',
            initAssetTrendsDateAggregationType: 'query:assetTrendsDateAggregationType'
        }));
        expect(route('/budget/list').props(routeLike)).toEqual({
            initType: 'query:type',
            initPeriodType: 'query:periodType',
            initViewMode: 'query:viewMode'
        });
        expect(route('/pairing/list').props(routeLike)).toEqual({
            initDomain: 'query:domain', initTab: 'query:tab'
        });
        expect(route('/user/settings').props(routeLike)).toEqual({ initTab: 'query:tab' });
        expect(route('/app/settings').props(routeLike)).toEqual({ initTab: 'query:tab' });
    });

    test('projects verification, reset, and OAuth callback contracts', () => {
        const query = {
            email: 'alice@example.invalid', token: 'token', emailSent: 'true',
            provider: 'oidc', platform: 'desktop', userName: 'alice',
            errorCode: 'none', message: 'ok'
        };

        expect(route('/verify_email').props({ query })).toEqual({
            email: 'alice@example.invalid', token: 'token', hasValidEmailVerifyToken: true
        });
        expect(route('/verify_email').props({ query: { ...query, emailSent: 'false' } }).hasValidEmailVerifyToken).toBe(false);
        expect(route('/resetpassword').props({ query })).toEqual({ token: 'token' });
        expect(route('/oauth2_callback').props({ query })).toEqual({
            token: 'token', provider: 'oidc', platform: 'desktop', userName: 'alice',
            errorCode: 'none', message: 'ok'
        });
    });

    test('redirects unknown desktop paths through the guarded home route', async () => {
        const vueRouter = jest.requireActual('vue-router') as {
            createMemoryHistory: () => unknown;
            createRouter: (options: { history: unknown; routes: unknown[] }) => {
                push: (path: string) => Promise<unknown>;
                currentRoute: {
                    value: {
                        fullPath: string;
                        matched: unknown[];
                    };
                };
            };
        };
        const runtimeRouter = vueRouter.createRouter({
            history: vueRouter.createMemoryHistory(),
            routes: mockRouterOptions.routes
        });

        await runtimeRouter.push('/missing-desktop-route');
        expect(runtimeRouter.currentRoute.value.fullPath).toBe('/login');
        expect(runtimeRouter.currentRoute.value.matched.length).toBeGreaterThan(0);

        mockLoggedIn = true;
        mockUnlocked = true;
        await runtimeRouter.push('/another-missing-route');
        expect(runtimeRouter.currentRoute.value.fullPath).toBe('/');
        expect(runtimeRouter.currentRoute.value.matched.length).toBeGreaterThan(0);
    });
});
