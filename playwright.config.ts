import { defineConfig, devices } from '@playwright/test'

/**
 * 注意（规格 T14 硬性要求）：
 * Playwright 的 Web 模式只验证**界面在严格契约 mock 下的行为**，
 * 它不验证真实文件操作。任何据此得出的结论都必须在报告里表明这一限制。
 */
export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  // 单实例执行任务，e2e 也应串行，避免并发干扰共享状态
  workers: 1,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://127.0.0.1:1420',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        // `channel: 'chromium'` 让 Playwright 使用**完整的 chromium** 并开它自带的
        // 新 headless 模式，而不是单独下载的 `chrome-headless-shell`。
        //
        // 这不是随手加的开关：headless shell 在部分环境里下不动（本机就遇到了，
        // install 卡死几十分钟），而完整 chromium 已经装好。少一份二进制、
        // 少一个环境依赖，跑的也仍是真实的 Chromium。
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
