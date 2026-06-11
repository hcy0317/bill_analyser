import { defineConfig, devices } from '@playwright/test';

const port = Number(process.env['E2E_WEB_PORT'] ?? '8081');
const host = process.env['E2E_WEB_HOST'] ?? '127.0.0.1';
const baseURL = process.env['E2E_BASE_URL'] ?? `http://${host}:${port}`;

export default defineConfig({
    testDir: './e2e',
    outputDir: './test-results',
    globalTeardown: './e2e/setup/global-teardown.ts',
    fullyParallel: false,
    forbidOnly: !!process.env['CI'],
    retries: process.env['CI'] ? 2 : 0,
    workers: 1,
    reporter: [
        ['list'],
        ['html', { outputFolder: 'playwright-report', open: 'never' }]
    ],
    use: {
        baseURL,
        trace: 'on-first-retry',
        screenshot: 'only-on-failure',
        video: 'retain-on-failure'
    },
    webServer: {
        command: 'npm run dev',
        url: baseURL,
        reuseExistingServer: !process.env['CI'],
        timeout: 120_000
    },
    projects: [
        {
            name: 'setup',
            testMatch: /.*\.setup\.ts/
        },
        {
            name: 'desktop-chromium',
            dependencies: ['setup'],
            testMatch: [
                /.*\.desktop\.spec\.ts/,
                /desktop\..*\.spec\.ts/,
                /import-preview-statistics\.spec\.ts/
            ],
            use: {
                ...devices['Desktop Chrome'],
                viewport: { width: 1440, height: 900 },
                storageState: './e2e/.auth/e2e-user.json'
            }
        },
        {
            name: 'mobile-chromium',
            dependencies: ['setup'],
            testMatch: [
                /.*\.mobile\.spec\.ts/,
                /mobile\..*\.spec\.ts/
            ],
            use: {
                ...devices['Pixel 7'],
                storageState: './e2e/.auth/e2e-user.json'
            }
        }
    ]
});
