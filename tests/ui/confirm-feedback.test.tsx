/**
 * PR-004（P1）：点「确认执行」后必须**立刻**有反馈。
 *
 * ## 这一条防的是什么
 *
 * 缺陷现象是「最下面那个确认没有反应」。代码上的根因是
 * `setConfirming(false)` 排在 `await execute_plan` **之后**——952 个文件要跑很久，
 * 这段时间里对话框一直亮着、按钮一直可点、主界面毫无变化。
 *
 * 用户看到「没反应」之后的自然反应是**重复点击或强杀进程**。这不是观感问题：
 * 强杀进程会打断一个正在移动文件的执行。所以这条按 P1 对待。
 *
 * ## 为什么必须用「挂起的 Promise」来测
 *
 * 用小数据集测不出来：文件少时执行很快，`await` 一闪而过，对话框瞬间关闭，
 * 断言永远是绿的。**必须让 `execute_plan` 永不 resolve**，才可能观察到
 * 「执行期间」这个中间态——这正是当初 C/D 节验收漏掉它的原因。
 *
 * 这几条用例的意义不只是「回归」，也是把那个漏掉的观测条件固定下来：
 * 以后任何人改执行流程，只要把关闭时机挪回 await 之后，这里就会红。
 */

import { describe, expect, it, vi } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { t } from '../../src/i18n/zh-CN'
import type { Plan, PlanItem, ValidationReport } from '../../src/api/contracts.generated'

vi.mock('../../src/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/client')>()
  return { ...actual, call: vi.fn() }
})

import { call, IpcError } from '../../src/api/client'

const mockedCall = vi.mocked(call)

/**
 * 让 `call` 按命令名返回预设结果。
 *
 * `Error` 实例表示**拒绝**：`Promise.resolve(new Error(...))` 会让失败的路径
 * 变成「成功但载荷是个 Error 对象」，那既不是被测行为，也会在渲染期抛出一个
 * 与被测内容无关的崩溃。未预设的命令同样拒绝，避免测试悄悄走到别的分支。
 */
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

/**
 * 造一个后端真实会发的失败。
 *
 * 页面用 `raw instanceof IpcError ? raw.message : t.common.unknownError` 取文案，
 * 所以裸 `Error` 只会渲染成「发生未知错误。」——用它做失败夹具，
 * 断言就变成了在测兜底分支，而不是在测「失败信息有没有显示出来」。
 */
function ipcFailure(message: string, code = 'INTERNAL'): IpcError {
  return new IpcError({ code, message, retryable: false, details: {} })
}

function item(id: string): PlanItem {
  return {
    id,
    fileId: `file-${id}`,
    source: [`${id}.txt`],
    target: ['文档', `${id}.txt`],
    action: 'move',
    selected: true,
    origin: 'rule',
    reason: '按类型',
    expected: {
      volumeId: '12345678',
      fileId: `fid-${id}`,
      size: '10',
      modifiedNs: '1700000000000000000',
      sha256: '0'.repeat(64),
    },
  } as PlanItem
}

const thePlan = {
  id: 'plan-1',
  rootId: 'root-1',
  scanId: 'scan-1',
  revision: 1,
  mode: 'rules',
  status: 'draft',
  items: [item('a'), item('b'), item('c')],
  createdAt: '2026-09-17T00:00:00Z',
} as Plan

const goodReport = {
  planId: 'plan-1',
  revision: 1,
  digest: 'digest-1',
  executableCount: 3,
  issues: [],
  validationToken: 'token-1',
  expiresAt: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
} as ValidationReport

/** 订阅进度。这个文件不测进度事件本身，但组件会调用它，必须给一个可用的替身。 */
vi.mock('../../src/api/events', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/events')>()
  return { ...actual, subscribeTaskProgress: vi.fn() }
})

import { subscribeTaskProgress } from '../../src/api/events'
import type { TaskProgress } from '../../src/api/events'
import { PreviewPage } from '../../src/features/preview/PreviewPage'

vi.mocked(subscribeTaskProgress).mockImplementation(async () => () => {})

function captureProgressHandler(): () => (update: TaskProgress) => void {
  let captured: ((update: TaskProgress) => void) | null = null
  vi.mocked(subscribeTaskProgress).mockImplementation(async (handler) => {
    captured = handler
    return () => {}
  })
  return () => {
    if (captured === null) throw new Error('组件还没注册进度处理器')
    return captured
  }
}

/** 走到「对话框已打开、确认按钮可点」这一步。 */
async function openConfirmDialog(): Promise<void> {
  render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)
  await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
  const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
  await waitFor(() => expect(confirm).toBeEnabled())
  await userEvent.click(confirm)
  expect(await screen.findByRole('dialog')).toBeInTheDocument()
}

describe('PR-004：确认执行的即时反馈', () => {
  it('执行一开始对话框就关闭，不必等 execute_plan 返回', async () => {
    stubIpc({
      get_plan: thePlan,
      validate_plan: goodReport,
      // **永不 resolve**：模拟 952 个文件的长时间执行。
      // 用一个立刻完成的 Promise 会让这条用例失去意义。
      execute_plan: new Promise(() => {}),
    })
    await openConfirmDialog()

    await userEvent.click(screen.getByRole('button', { name: t.preview.confirmInDialog }))

    // 关键断言：对话框消失，而 execute_plan 仍然挂着。
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  it('执行期间立刻显示进度面板，哪怕后端一条进度事件都还没推来', async () => {
    stubIpc({
      get_plan: thePlan,
      validate_plan: goodReport,
      execute_plan: new Promise(() => {}),
    })
    await openConfirmDialog()
    await userEvent.click(screen.getByRole('button', { name: t.preview.confirmInDialog }))

    /*
      这一条覆盖的是原实现的第二个漏洞：
      进度文字的条件是 `busy && progress`，而 `progress` 要等后端推第一条
      事件才有值。执行最开始的那几秒（也可能是几十秒——952 个文件的第一项
      要重新核对指纹）`progress` 一直是 null，界面上什么都没有。
      正确的条件是只看 `busy`。

      **用 «可访问名 + status 角色» 定位，不能用文字定位**：执行期间主按钮的标签
      也是「正在整理…」，按文字查会同时命中两个元素（按钮 + 面板）。
      而按裸 `role="status"` 查也不唯一——页面上禁用理由列表、停止提示
      同样是 status 区域。所以进度面板带了 `aria-label`。
    */
    const panel = await screen.findByRole('status', { name: t.preview.progressRegionLabel })
    expect(panel).toHaveTextContent(t.preview.executing)
  })

  it('收到进度事件后，面板从「正在整理…」换成具体计数', async () => {
    const handler = captureProgressHandler()
    stubIpc({
      get_plan: thePlan,
      validate_plan: goodReport,
      execute_plan: new Promise(() => {}),
    })
    await openConfirmDialog()
    await userEvent.click(screen.getByRole('button', { name: t.preview.confirmInDialog }))

    expect(
      await screen.findByRole('status', { name: t.preview.progressRegionLabel }),
    ).toHaveTextContent(t.preview.executing)

    act(() => handler()({ taskId: 'task-1', seq: 1, status: 'running', processed: 2, total: 3 }))

    expect(await screen.findByText(/已整理\s*2\s*\/\s*3/)).toBeInTheDocument()
  })

  it('执行期间主按钮变成「正在整理…」并禁用，防止重复点击', async () => {
    stubIpc({
      get_plan: thePlan,
      validate_plan: goodReport,
      execute_plan: new Promise(() => {}),
    })
    await openConfirmDialog()
    await userEvent.click(screen.getByRole('button', { name: t.preview.confirmInDialog }))

    const main = await screen.findByRole('button', { name: t.preview.executing })
    expect(main).toBeDisabled()
  })

  it('执行完成后再打开对话框，按钮恢复为「确认执行」而不是卡在执行中', async () => {
    stubIpc({
      get_plan: thePlan,
      validate_plan: goodReport,
      execute_plan: {
        runId: 'run-1',
        planId: 'plan-1',
        direction: 'apply',
        stateDigest: 'digest-1',
        status: 'completed',
        counts: { applied: 3, failed: 0, skipped: 0, pending: 0, ambiguous: 0 },
        issues: [],
      },
    })
    await openConfirmDialog()
    await userEvent.click(screen.getByRole('button', { name: t.preview.confirmInDialog }))

    // 执行完成：进度面板消失、执行结果显示出来
    await waitFor(() => {
      expect(
        screen.queryByRole('status', { name: t.preview.progressRegionLabel }),
      ).not.toBeInTheDocument()
    })
    expect(screen.getByText(t.preview.runHeading)).toBeInTheDocument()
  })

  it('执行失败时错误可见，不因为「对话框已关」而丢掉失败信息', async () => {
    /*
      这是「先关对话框」这个改动的**代价边界**：关闭之后如果失败，
      用户不能再从对话框里看到错误。所以必须确认错误仍然会显示在主界面上。
      不做这一条的话，这个改动等于用一个静默失败换掉了一个「没反应」。

      夹具必须是 `IpcError`：页面只对它取 `message`，裸 `Error` 会走到
      「发生未知错误。」的兜底分支——那样测的是兜底文案，不是失败信息的可见性。
    */
    stubIpc({
      get_plan: thePlan,
      validate_plan: goodReport,
      execute_plan: ipcFailure('磁盘写入失败', 'JOURNAL_WRITE_FAILED'),
    })
    await openConfirmDialog()
    await userEvent.click(screen.getByRole('button', { name: t.preview.confirmInDialog }))

    expect(await screen.findByRole('alert')).toHaveTextContent('磁盘写入失败')
  })
})
