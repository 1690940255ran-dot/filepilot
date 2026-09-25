import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./tests/setup.ts'],
    // 只跑单元 / 组件测试；e2e 由 Playwright 负责（它是真实浏览器，不是 jsdom）
    include: ['tests/**/*.{test,spec}.{ts,tsx}'],
    // `tests/screenshots` 与 `tests/prod-csp` 也是 Playwright 的（各自独立 config）。
    // 不收留它们不是为了整齐：被 jsdom 收走会以
    // 「Playwright Test did not expect test.use() to be called here」失败，
    // 而那句话完全看不出「这个文件本来就不该在这里跑」。
    exclude: [
      'tests/e2e/**',
      'tests/screenshots/**',
      'tests/prod-csp/**',
      'node_modules/**',
      'src-tauri/**',
    ],
    // 规格要求：没有测试文件时不允许用 passWithNoTests 伪造"通过"
    passWithNoTests: false,
    css: false,
    restoreMocks: true,
  },
})
