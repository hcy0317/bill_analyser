import type { E2EEnvironment } from './env';

export interface RouteSmokeTarget {
    readonly name: string;
    readonly path: string;
    readonly testId: string;
}

export const desktopSmokeRoutes: readonly RouteSmokeTarget[] = [
    { name: 'home', path: '/', testId: 'desktop.home.page' },
    { name: 'transactions', path: '/transaction/list?pageType=0&dateType=7', testId: 'desktop.transactions.page' },
    { name: 'statistics', path: '/statistics/transaction', testId: 'desktop.statistics.page' },
    { name: 'accounts', path: '/account/list', testId: 'desktop.accounts.page' },
    { name: 'categories', path: '/category/list', testId: 'desktop.categories.page' },
    { name: 'tags', path: '/tag/list', testId: 'desktop.tags.page' },
    { name: 'templates', path: '/template/list', testId: 'desktop.templates.page' },
    { name: 'schedules', path: '/schedule/list', testId: 'desktop.schedules.page' },
    { name: 'budgets', path: '/budget/list', testId: 'desktop.budgets.page' },
    { name: 'rules', path: '/pairing/list', testId: 'desktop.rules.page' },
    { name: 'exchange-rates', path: '/exchange_rates', testId: 'desktop.exchange-rates.page' },
    { name: 'user-settings', path: '/user/settings', testId: 'desktop.user-settings.page' },
    { name: 'app-settings', path: '/app/settings', testId: 'desktop.app-settings.page' },
    { name: 'about', path: '/about', testId: 'desktop.about.page' }
];

export const mobileSmokeRoutes: readonly RouteSmokeTarget[] = [
    { name: 'home', path: '/', testId: 'mobile.home.page' },
    { name: 'transactions', path: '/transaction/list', testId: 'mobile.transactions.page' },
    { name: 'statistics', path: '/statistic/transaction', testId: 'mobile.statistics.page' },
    { name: 'accounts', path: '/account/list', testId: 'mobile.accounts.page' },
    { name: 'categories', path: '/category/all', testId: 'mobile.categories-all.page' },
    { name: 'tags', path: '/tag/list', testId: 'mobile.tags.page' },
    { name: 'templates', path: '/template/list', testId: 'mobile.templates.page' },
    { name: 'schedules', path: '/schedule/list', testId: 'mobile.schedules.page' },
    { name: 'budgets', path: '/budgets', testId: 'mobile.budgets.page' },
    { name: 'account-rules', path: '/account/rules', testId: 'mobile.account-rules.page' },
    { name: 'exchange-rates', path: '/exchange_rates', testId: 'mobile.exchange-rates.page' },
    { name: 'settings', path: '/settings', testId: 'mobile.settings.page' },
    { name: 'data-management', path: '/user/data/management', testId: 'mobile.data-management.page' },
    { name: 'about', path: '/about', testId: 'mobile.about.page' }
];

export function desktopRoute(path: string, env: E2EEnvironment): string {
    return `${env.baseURL}/desktop.html#${normalizeAppPath(path)}`;
}

export function mobileRoute(path: string, env: E2EEnvironment): string {
    return `${env.baseURL}/mobile.html#!${normalizeAppPath(path)}`;
}

function normalizeAppPath(path: string): string {
    return path.startsWith('/') ? path : `/${path}`;
}
