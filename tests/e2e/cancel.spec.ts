import { expect, test, type Page } from '@playwright/test'

import { emitProgress, installIpcMock } from './ipc-mock'

/**
 * 规格 T07 的 e2e：取消与重复点击。
 *
 * **重要边界**（规格 T14 硬性要求）：Playwright 的 Web 模式只验证
 * **界面在严格契约 mock 下的行为**，它不验证真实文件操作。
 * 「已完成项保留、未派发项原位」这类磁盘事实由 `tests/execute_windows.rs`
 * 的 Rust 集成测试覆盖——那边跑在真实 NTFS 上，并逐个比对内容哈希。
 *
 * 这里的价值在于：真实浏览器里的**事件接线、按钮禁用、请求次数**，
 * 那是 jsdom 单测不容易覆盖的部分。
 */

/** 走完「选文件夹 → 扫描 → 生成建议」，停在预览页。 */
async function reachPreview(page: Page): Promise<void> {
  await page.goto('/')

  await page.getByRole('button', { name: '选择文件夹' }).click()
  await expect(page.getByText('C:\\资料')).toBeVisible()

  await page.getByRole('button', { name: '开始扫描' }).click()
  await page.getByRole('button', { name: '生成整理建议' }).click()

  await expect(page.getByRole('heading', { name: '整理预览' })).toBeVisible()
}

/** 点校验拿到令牌，再点确认打开对话框、点对话框按钮开始执行。 */
async function startExecution(page: Page): Promise<void> {
  await page.getByRole('button', { name: '校验并获取确认' }).click()
  await expect(page.getByRole('button', { name: '确认并执行' })).toBeEnabled()

  await page.getByRole('button', { name: '确认并执行' }).click()
  await page.getByRole('button', { name: '确认执行' }).click()
}

test.describe('执行中的取消', () => {
  test('第 1 项之后取消：界面区分「已请求停止」与「已停止」', async ({ page }) => {
    await installIpcMock(page)
    await reachPreview(page)
    await startExecution(page)

    // 后端开始派发第一项
    await emitProgress(page, {
      taskId: 'task-exec',
      seq: 1,
      status: 'running',
      processed: 1,
      total: 3,
    })

    await expect(page.getByText(/已整理\s*1\s*\/\s*3/)).toBeVisible()

    const stop = page.getByRole('button', { name: '停止整理' })
    await expect(stop).toBeVisible()
    await stop.click()

    // 必须明确说「请求」而不是「已停止」——后者会让用户以为文件已经不再变动
    await expect(
      page.getByText('已发出停止请求。正在处理的那一项会先完成（半途而废会留下无法判定的状态），之后的项不会再动。'),
    ).toBeVisible()

    const calls = await page.evaluate(() => window.__FILPILOT_CALLS__ ?? [])
    const cancelCalls = calls.filter((entry) => entry.command === 'cancel_task')
    expect(cancelCalls.length, 'cancel_task 必须被调用').toBe(1)
    expect(cancelCalls[0]?.args, '取消要带上具体的 taskId').toEqual({
      taskId: 'task-exec',
    })
  })

  test('后端报告终态后，停止入口消失', async ({ page }) => {
    await installIpcMock(page)
    await reachPreview(page)
    await startExecution(page)

    await emitProgress(page, {
      taskId: 'task-exec',
      seq: 1,
      status: 'running',
      processed: 1,
      total: 3,
    })
    await expect(page.getByRole('button', { name: '停止整理' })).toBeVisible()

    // 执行器在安全点停下并写入终态
    await emitProgress(page, {
      taskId: 'task-exec',
      seq: 2,
      status: 'cancelled',
      processed: 3,
      total: 3,
    })

    await expect(page.getByRole('button', { name: '停止整理' })).toBeHidden()
  })

  test('乱序的进度事件不会让界面倒退', async ({ page }) => {
    await installIpcMock(page)
    await reachPreview(page)
    await startExecution(page)

    await emitProgress(page, {
      taskId: 'task-exec',
      seq: 5,
      status: 'running',
      processed: 2,
      total: 3,
    })
    await expect(page.getByText(/已整理\s*2\s*\/\s*3/)).toBeVisible()

    // seq 比已见的小：序号守卫应当丢弃它
    await emitProgress(page, {
      taskId: 'task-exec',
      seq: 3,
      status: 'running',
      processed: 1,
      total: 3,
    })
    await expect(page.getByText(/已整理\s*2\s*\/\s*3/)).toBeVisible()
  })
})

test.describe('重复点击', () => {
  test('连点确认只会发出一次执行请求，且复用同一个 requestId', async ({ page }) => {
    await installIpcMock(page)
    await reachPreview(page)
    await page.getByRole('button', { name: '校验并获取确认' }).click()
    await expect(page.getByRole('button', { name: '确认并执行' })).toBeEnabled()

    await page.getByRole('button', { name: '确认并执行' }).click()

    // 同一事件循环内连续触发两次 click，验证同步的 executingRef 守卫。
    // 不对第一次点击后已消失的按钮再用 Locator.click：那只会等待到测试超时。
    const confirmInDialog = page.getByRole('button', { name: '确认执行' })
    await confirmInDialog.evaluate((button) => {
      const target = button as HTMLButtonElement
      target.click()
      target.click()
    })

    const calls = await page.evaluate(() => window.__FILPILOT_CALLS__ ?? [])
    const executes = calls.filter((entry) => entry.command === 'execute_plan')

    expect(executes.length, '执行请求不应被重复发出').toBe(1)
    const first = executes[0]?.args as { requestId?: string } | undefined
    expect(typeof first?.requestId, '必须带上 requestId，后端才有幂等键').toBe(
      'string',
    )
  })
})
