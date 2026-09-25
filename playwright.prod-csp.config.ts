import { defineConfig, devices } from '@playwright/test'

/**
 * 生产 CSP 回归专用 config（见 tests/prod-csp/production-csp.spec.ts）。
 *
 * 与主 config 的区别：
 * - 托管的是 **dist 生产构建产物**，不是 Vite dev server；
 * - 响应头带的是 `tauri.conf.json` 里的**生产 CSP**，不是 `devCsp`。
 *
 * 这个差异正是当初「安装版白屏、本地全绿」的根源，所以它必须是一条独立的用例。
 */
export default defineConfig({
    testDir: './tests/prod-csp',
    fullyParallel: false,
    workers: 1,
    reporter: [['list']],
    use: {
        baseURL: 'http://127.0.0.1:1431',
    },
    projects: [
        {
            name: 'chromium',
            use: {
                ...devices['Desktop Chrome'],
                // 与其它 config 一致：用完整 chromium，不用 headless shell
                channel: 'chromium',
            },
        },
    ],
    webServer: {
        command: 'node scripts/serve-dist-csp.mjs',
        url: 'http://127.0.0.1:1431',
        reuseExistingServer: !process.env.CI,
        timeout: 60_000,
    },
})
