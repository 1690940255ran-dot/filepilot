import { expect, test } from '@playwright/test'

import { installIpcMock } from '../e2e/ipc-mock'

/**
 * **生产 CSP 下的启动回归**（2026-09-24 加）。
 *
 * 背景：安装版曾经**整页白屏**——生产 CSP 是 `script-src 'self'`，
 * 而前端用 Ajv 在运行时编译 JSON Schema（走 `new Function`），
 * 校验器编译抛 `EvalError`，前端启动即崩。
 *
 * 这个缺陷逃过了当时所有测试，因为 `pnpm dev` 与 `pnpm test:e2e` 用的都是
 * `devCsp`（含 `'unsafe-eval'`）。**两层 CSP 的差异就是缺陷的藏身处。**
 *
 * 这个文件跑在 `scripts/serve-dist-csp.mjs` 上：静态托管**生产构建产物**、
 * 并原样下发 `tauri.conf.json` 里的**生产 CSP**。所以：
 *
 * - 有人把校验改回运行时 Ajv → 这里红；
 * - 有人往 CSP 里塞 `'unsafe-eval'` 图省事 → 这里还是绿，但
 *   `docs/RELEASE_CHECKLIST.md` 的 CSP 核对会把这件事拦下来。
 */

test.describe('生产 CSP + 生产构建产物', () => {
    test('应用能启动并渲染（不是白屏）', async ({ page }) => {
        await installIpcMock(page)
        await page.goto('/')

        // 白屏的表现是"页面上什么都没有"：这里盯住真实元素，
        // 而不是断言"没有报错"——CSP 拒绝脚本时页面不会报错，只会空白。
        await expect(page.getByRole('heading', { name: /文件领航|FilePilot/ })).toBeVisible()
        await expect(page.getByRole('button', { name: '选择文件夹' })).toBeVisible()
    })

    test('契约校验在工作（IPC 响应按 schema 校验后才进界面）', async ({ page }) => {
        await installIpcMock(page)
        await page.goto('/')

        // get_settings 的响应会被 AppSettings 契约校验。
        // 校验器若没起来，这一步要么白屏、要么界面拿不到设置。
        await expect(page.getByRole('button', { name: '选择文件夹' })).toBeEnabled()

        await page.getByRole('button', { name: '选择文件夹' }).click()
        // 根目录展示走的是 RootSummary 契约
        await expect(page.getByText('C:\\资料')).toBeVisible()
    })
})
