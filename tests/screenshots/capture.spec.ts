import { expect, test } from '@playwright/test'

import { emitProgress, installIpcMock } from '../e2e/ipc-mock'

/**
 * README 截图采集（T16「真实截图」）。
 *
 * 截的是**真实界面渲染**——同一套 React 页面、同一条 IPC 链路，
 * 只是后端被严格契约 mock 替换（规格 T14 允许的 Web 模式边界）。
 * 数据全部是合成的中文样例，不含任何真实用户文件。
 *
 * 跑法：pnpm exec playwright test --config playwright.screenshots.config.ts
 * 产物：docs/screenshots/{home,preview,confirm,executing}.png
 *
 * 这组 spec 不进 `pnpm test:e2e`（独立 config、独立 testDir）：
 * 它不是回归测试，是「要发版时重新生成截图」的工具。
 */

/** 比 e2e 默认夹具更像真实桌面的合成样例。 */
const SAMPLE_FILES: Array<{ name: string; ext: string; target: string; reason: string }> = [
  { name: '2026年春季学期课表.pdf', ext: '.pdf', target: '文档', reason: '按类型' },
  { name: '实验报告-第7周.docx', ext: '.docx', target: '文档', reason: '按类型' },
  { name: '答辩PPT-终稿-v3.pptx', ext: '.pptx', target: '文档', reason: '按类型' },
  { name: 'IMG_2041.jpg', ext: '.jpg', target: '图片', reason: '按类型' },
  { name: '屏幕截图 2026-09-18.png', ext: '.png', target: '图片', reason: '按类型' },
  { name: '讲座录音-0912.mp3', ext: '.mp3', target: '音视频', reason: '按类型' },
  { name: '数据集-第一批.zip', ext: '.zip', target: '压缩包', reason: '按类型' },
  { name: '随手记.txt', ext: '.txt', target: '文档', reason: '按类型' },
]

function fingerprint(index: number) {
  return {
    volumeId: '12345678',
    fileId: `fid-${index}`,
    size: '2048',
    modifiedNs: '1700000000000000000',
    sha256: String.fromCharCode(96 + index).repeat(64),
  }
}

const files = SAMPLE_FILES.map((f, i) => ({
  id: `fid-${i + 1}`,
  scanId: 'scan-1',
  rootId: 'root-1',
  relativePath: [f.name],
  extension: f.ext,
  fingerprint: fingerprint(i + 1),
  extractionStatus: 'pending',
  skipCode: null,
}))

const planItems = SAMPLE_FILES.map((f, i) => ({
  id: `item-${i + 1}`,
  fileId: `fid-${i + 1}`,
  source: [f.name],
  target: [f.target, f.name],
  action: 'move',
  selected: true,
  origin: 'rule',
  reason: f.reason,
  expected: fingerprint(i + 1),
}))

const plan = {
  id: 'plan-1',
  rootId: 'root-1',
  scanId: 'scan-1',
  revision: 1,
  mode: 'rules',
  status: 'draft',
  createdAt: '2026-09-22T00:00:00.000Z',
  items: planItems,
}

const table: Record<string, unknown> = {
  get_settings: {
    mode: 'rules',
    scanMaxFiles: 10_000,
    scanMaxDepth: 20,
    selectedProviderId: null,
  },
  choose_root: {
    rootId: 'root-1',
    displayPath: 'C:\\Users\\示例\\Desktop\\待整理',
    volumeId: '12345678',
  },
  start_scan: 'task-scan',
  get_task: {
    taskId: 'task-scan',
    status: 'completed',
    processed: files.length,
    total: files.length,
    scanId: 'scan-1',
    error: null,
  },
  list_files: { items: files, nextCursor: null, total: files.length },
  create_plan: { plan, issues: [] },
  get_plan: plan,
  validate_plan: {
    planId: 'plan-1',
    revision: 1,
    digest: 'digest-1',
    executableCount: planItems.length,
    issues: [],
    validationToken: 'token-1',
    expiresAt: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  },
}

test.use({ viewport: { width: 1200, height: 800 }, reducedMotion: 'reduce' })

test('空态、历史、设置和窄窗口视觉检查', async ({ page }) => {
  await installIpcMock(page, {
    ...table,
    recovery_status: { blocked: false, blockedRuns: [] },
    list_runs: [],
    list_providers: [],
    get_ocr_status: { status: 'available', languages: ['zh-Hans-CN', 'en-US'], message: '识别语言可用' },
  })
  await page.goto('/')
  await expect(page.getByRole('button', { name: '选择文件夹' })).toBeVisible()
  await page.screenshot({ path: 'docs/screenshots/home-empty.png' })
  await page.getByRole('button', { name: '历史', exact: true }).click()
  await expect(page.getByText(/还没有任何整理记录/)).toBeVisible()
  await page.screenshot({ path: 'docs/screenshots/history-empty.png' })
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await expect(page.getByText('zh-Hans-CN', { exact: false })).toBeVisible()
  await page.screenshot({ path: 'docs/screenshots/settings.png' })
  for (const width of [1000, 640]) {
    await page.setViewportSize({ width, height: 800 })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    await expect(page.getByRole('heading', { name: '设置', exact: true })).toBeVisible()
    await page.screenshot({ path: `docs/screenshots/settings-${width}.png` })
  }
})

test('采集 README 四张截图', async ({ page }) => {
  await installIpcMock(page, table)
  await page.goto('/')

  // 1. 首页：选好文件夹、扫描完成、统计可见
  await page.getByRole('button', { name: '选择文件夹' }).click()
  await expect(page.getByText('C:\\Users\\示例\\Desktop\\待整理')).toBeVisible()
  await page.getByRole('button', { name: '开始扫描' }).click()
  await page.getByRole('button', { name: '生成整理建议' }).waitFor()
  await page.screenshot({ path: 'docs/screenshots/home.png' })

  // 2. 预览：计划表格
  await page.getByRole('button', { name: '生成整理建议' }).click()
  await expect(page.getByRole('heading', { name: '整理预览' })).toBeVisible()
  await expect(page.getByText('2026年春季学期课表.pdf').first()).toBeVisible()
  await page.screenshot({ path: 'docs/screenshots/preview.png', fullPage: false })

  // 3. 确认对话框
  await page.getByRole('button', { name: '校验并获取确认' }).click()
  await expect(page.getByRole('button', { name: '确认并执行' })).toBeEnabled()
  await page.getByRole('button', { name: '确认并执行' }).click()
  await expect(page.getByRole('button', { name: '确认执行' })).toBeVisible()
  await page.screenshot({ path: 'docs/screenshots/confirm.png' })

  // 4. 执行中：推两条进度，停在 5/8
  await page.getByRole('button', { name: '确认执行' }).click()
  await emitProgress(page, {
    taskId: 'task-exec',
    seq: 1,
    status: 'running',
    processed: 5,
    total: files.length,
  })
  await expect(page.getByText(/已整理\s*5\s*\/\s*8/)).toBeVisible()
  await page.screenshot({ path: 'docs/screenshots/executing.png' })
})
