import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Tauri 期望固定端口：strictPort 让端口冲突时直接失败，而不是静默换端口导致桌面壳连不上。
const DEV_PORT = 1420

export default defineConfig({
  plugins: [react()],
  // Tauri CLI 自己会打印构建信息，别让 Vite 清屏把它冲掉
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: DEV_PORT,
    strictPort: true,
    watch: {
      // src-tauri 由 cargo 监听，Vite 再监听一遍会触发无意义的重启
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    // Windows WebView2 基于 Chromium，target 对齐可安全使用现代语法
    target: 'chrome120',
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: true,
  },
})
