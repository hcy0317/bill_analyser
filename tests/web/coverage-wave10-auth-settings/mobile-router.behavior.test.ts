import { jest } from '@jest/globals';

let mockLoggedIn = false;
let mockUnlocked = false;

const mobileViews = [
    'HomePage', 'LoginPage', 'SignupPage', 'UnlockPage', 'transactions/ListPage',
    'transactions/EditPage', 'transactions/AmountFilterPage', 'transactions/ImportPreviewPage',
    'accounts/ListPage', 'accounts/EditPage', 'accounts/RuleListPage',
    'accounts/ReconciliationStatementPage', 'accounts/MoveAllTransactionsPage',
    'statistics/TransactionPage', 'statistics/SettingsPage', 'settings/TextSizeSettingsPage',
    'settings/PageSettingsPage', 'settings/ApplicationCloudSyncSettingsPage',
    'settings/AccountFilterSettingsPage', 'settings/CategoryFilterSettingsPage',
    'settings/TransactionTagFilterSettingsPage', 'SettingsPage', 'ApplicationLockPage',
    'exchangerates/ListPage', 'exchangerates/UpdatePage', 'AboutPage', 'users/UserProfilePage',
    'users/DataManagementPage', 'users/TwoFactorAuthPage', 'users/SessionListPage',
    'categories/AllPage', 'categories/ListPage', 'categories/EditPage',
    'categories/PresetPage', 'tags/ListPage', 'templates/ListPage', 'budgets/ListPage'
];

for (const view of mobileViews) {
    jest.doMock(`@/views/mobile/${view}.vue`, () => ({
        __esModule: true,
        default: { name: `Stub:${view}` }
    }));
}
jest.doMock('@/lib/userstate.ts', () => ({
    isUserLogined: () => mockLoggedIn,
    isUserUnlocked: () => mockUnlocked
}));

const routes = (jest.requireActual('@/router/mobile.ts') as any).default as any[];

function route(path: string): any {
    return routes.find(item => item.path === path);
}

function navigationContext() {
    return {
        router: { navigate: jest.fn() },
        resolve: jest.fn(),
        reject: jest.fn()
    };
}

beforeEach(() => {
    mockLoggedIn = false;
    mockUnlocked = false;
});

describe('mobile router', () => {
    test('resolves every asynchronous route to its bound component', () => {
        const asyncRoutes = routes.filter(item => typeof item.async === 'function');
        expect(asyncRoutes.length).toBeGreaterThan(30);

        for (const item of asyncRoutes) {
            const resolve = jest.fn();
            item.async({ resolve });
            expect(resolve).toHaveBeenCalledWith({ component: expect.any(Object) });
        }
        expect(route('(.*)').redirect).toBe('/');
    });

    test('redirects protected routes to login or unlock before resolving unlocked users', () => {
        const guard = route('/').beforeEnter[0];
        const loggedOut = navigationContext();
        guard(loggedOut);
        expect(loggedOut.reject).toHaveBeenCalled();
        expect(loggedOut.router.navigate).toHaveBeenCalledWith('/login', {
            clearPreviousHistory: true, browserHistory: false
        });

        mockLoggedIn = true;
        const locked = navigationContext();
        guard(locked);
        expect(locked.router.navigate).toHaveBeenCalledWith('/unlock', {
            clearPreviousHistory: true, browserHistory: false
        });

        mockUnlocked = true;
        const unlocked = navigationContext();
        guard(unlocked);
        expect(unlocked.resolve).toHaveBeenCalled();
        expect(unlocked.reject).not.toHaveBeenCalled();
    });

    test('allows only locked users onto the unlock route', () => {
        const guard = route('/unlock').beforeEnter[0];
        const loggedOut = navigationContext();
        guard(loggedOut);
        expect(loggedOut.router.navigate).toHaveBeenCalledWith('/login', expect.any(Object));

        mockLoggedIn = true;
        const locked = navigationContext();
        guard(locked);
        expect(locked.resolve).toHaveBeenCalled();

        mockUnlocked = true;
        const unlocked = navigationContext();
        guard(unlocked);
        expect(unlocked.reject).toHaveBeenCalled();
        expect(unlocked.router.navigate).toHaveBeenCalledWith('/', expect.any(Object));
    });

    test('keeps login and signup public only until a session exists', () => {
        const guard = route('/login').beforeEnter[0];
        const anonymous = navigationContext();
        guard(anonymous);
        expect(anonymous.resolve).toHaveBeenCalled();

        mockLoggedIn = true;
        const locked = navigationContext();
        guard(locked);
        expect(locked.router.navigate).toHaveBeenCalledWith('/unlock', expect.any(Object));

        mockUnlocked = true;
        const unlocked = navigationContext();
        guard(unlocked);
        expect(unlocked.router.navigate).toHaveBeenCalledWith('/', expect.any(Object));
    });
});
