import { defineConfig, devices } from '@playwright/test'

/**
 * 截图采集专用 config（见 tests/screenshots/capture.spec.ts 的文件头）。
 *
 * 与主 config 的唯一区别是 testDir：截图不是回归测试，
 * 不能让它混进 `pnpm test:e2e` 的通过数里。
 */
export default defineConfig({
  testDir: './tests/screenshots',
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:1420',
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        // 与 playwright.config.ts 一致：用完整 chromium，不是 headless shell
        channel: 'chromium',
      },
    },
  ],
  webServer: {
    command: 'pnpm dev:web',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
})
