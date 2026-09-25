/**
 * T13：AI 模式在首页的接线。
 *
 * 前面两个文件各测了一半：`ai-disclosure.test.tsx` 测状态机、
 * `disclosure-review.test.tsx` 测审阅界面的渲染。这一份测的是**它们接上了**——
 * 也就是「扫描完 → 输入要求 → 预览 → 授权 → 分析 → 生成计划」这条路
 * 在真实页面里走得通。
 *
 * 重点盯两件容易在接线时出错的事：
 *
 * 1. **预览要带上扫到的文件 id**（`usableFileIds`）——少一个或多一个，
 *    用户看到的待发送内容就与实际不符；
 * 2. **生成计划带的是 `analysisId` 而不是 `ruleKind`**——两条路走错一条，
 *    后端会拒绝（它要求恰好给一个），但用户会看到一句莫名其妙的错误。
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { call } from '../../src/api/client'
import type { DisclosurePreview } from '../../src/api/contracts.generated'
import { OrganizePage } from '../../src/features/organize/OrganizePage'
import { t } from '../../src/i18n/zh-CN'
import { E2E_FILES, FIXTURES } from '../e2e/fixtures'

vi.mock('../../src/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/client')>()
  return { ...actual, call: vi.fn() }
})

const mockedCall = vi.mocked(call)

function stubIpc(responses: Record<string, unknown>): void {
  mockedCall.mockImplementation(((command: string) => {
    if (command in responses) {
      const value = responses[command]
      if (value instanceof Error) return Promise.reject(value)
      return Promise.resolve(value)
    }
    return Promise.reject(new Error(`测试未预设命令 ${command}`))
  }) as typeof call)
}

function preview(): DisclosurePreview {
  return {
    payloadDigest: 'digest-1',
    providerId: 'p1',
    model: 'test-model',
    instruction: '按主题分类',
    fileCount: E2E_FILES.length,
    characterCount: BigInt(30),
    items: E2E_FILES.map((file) => ({
      fileId: file.id,
      // `FileRecord` 只有 `relativePath`，没有单独的 name。
      fileName: file.relativePath[file.relativePath.length - 1] ?? file.id,
      characterCount: 10,
      truncated: false,
      excerpt: '内容开头',
      textStatus: 'present' as const,
    })),
  }
}

/** 走完「选文件夹 → 扫描」，停在「已扫描」。 */
async function scanUntilReady(mode: 'rules' | 'aiLocal' | 'aiCloud') {
  stubIpc({
    choose_root: FIXTURES.root,
    start_scan: FIXTURES.task.taskId,
    get_task: FIXTURES.task,
    list_files: FIXTURES.filePage,
    preview_disclosure: preview(),
    start_analysis: { taskId: 'task-analysis', analysisId: 'analysis-1' },
    create_plan: { plan: FIXTURES.plan, issues: [] },
  })

  render(
    <OrganizePage
      settingsState={{
        status: 'ready',
        settings: { ...FIXTURES.settings, mode, selectedProviderId: 'p1' },
      }}
    />,
  )

  await userEvent.click(screen.getByRole('button', { name: t.home.chooseRoot }))
  await userEvent.click(await screen.findByRole('button', { name: t.home.startScan }))
  await screen.findByText(t.home.summaryTitle)
}

describe('首页的 AI 模式接线', () => {
  it('规则模式下不出现 AI 的要求输入', async () => {
    await scanUntilReady('rules')

    // 规则模式不联网、也不做提取，摆一个「整理要求」输入框只会让人以为
    // 它起作用。
    expect(screen.queryByLabelText(t.ai.instructionLabel)).toBeNull()
    expect(screen.getByRole('button', { name: t.home.buildPlan })).toBeEnabled()
  })

  it('AI 模式下出现要求输入与预览入口', async () => {
    await scanUntilReady('aiCloud')

    expect(screen.getByLabelText(t.ai.instructionLabel)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: t.ai.previewAction })).toBeEnabled()
    // 规则模式仍然随时可用：AI 不可用不该把用户堵在这里。
    expect(screen.getByRole('button', { name: t.home.buildPlan })).toBeEnabled()
  })

  it('预览时带上扫到的每一个文件 id', async () => {
    await scanUntilReady('aiCloud')

    await userEvent.type(screen.getByLabelText(t.ai.instructionLabel), '按主题分类')
    await userEvent.click(screen.getByRole('button', { name: t.ai.previewAction }))

    const call_ = mockedCall.mock.calls.find((args) => args[0] === 'preview_disclosure')
    expect(call_?.[1]).toMatchObject({
      scanId: FIXTURES.task.scanId,
      mode: 'aiCloud',
      providerId: 'p1',
      instruction: '按主题分类',
      // 少一个或多一个，用户看到的待发送内容就与实际不符。
      selectedFileIds: E2E_FILES.map((file) => file.id),
    })

    // 预览回来后审阅界面就位。
    expect(await screen.findByText(t.ai.reviewHeading)).toBeInTheDocument()
  })

  it('本地模式不需要授权就能开始分析', async () => {
    await scanUntilReady('aiLocal')

    await userEvent.type(screen.getByLabelText(t.ai.instructionLabel), '按主题分类')
    await userEvent.click(screen.getByRole('button', { name: t.ai.previewAction }))
    await screen.findByText(t.ai.reviewHeading)

    // 本地模式不显示授权按钮，并说明为什么。
    expect(screen.queryByText(t.ai.grantAction)).toBeNull()
    expect(screen.getByText(t.ai.localNotice)).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: t.ai.startAction }))

    const call_ = mockedCall.mock.calls.find((args) => args[0] === 'start_analysis')
    expect(call_?.[1]).toMatchObject({ consentId: null, mode: 'aiLocal' })
  })

  it('分析完成后用 analysisId 生成计划，而不是 ruleKind', async () => {
    await scanUntilReady('aiLocal')

    await userEvent.type(screen.getByLabelText(t.ai.instructionLabel), '按主题分类')
    await userEvent.click(screen.getByRole('button', { name: t.ai.previewAction }))
    await screen.findByText(t.ai.reviewHeading)
    await userEvent.click(screen.getByRole('button', { name: t.ai.startAction }))

    // `start_analysis` 返回的 taskId 要能轮询到完成——这里夹具直接给
    // 已完成状态，所以下一步按钮立刻出现。
    const buildButton = await screen.findByRole('button', { name: t.ai.buildFromAnalysis })
    await userEvent.click(buildButton)

    const call_ = mockedCall.mock.calls.find((args) => args[0] === 'create_plan')
    expect(call_?.[1]).toMatchObject({
      scanId: FIXTURES.task.scanId,
      analysisId: 'analysis-1',
      // 两条路只能走一条，后端会拒绝「都给」。
      ruleKind: null,
    })
  })
})
