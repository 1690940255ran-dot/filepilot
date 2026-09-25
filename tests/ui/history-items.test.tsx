/**
 * PR-003：整理历史必须能看到「这次具体动了哪些文件」。
 *
 * ## 用户原话
 *
 * 「这个整理历史不够详细，不能看到具体整理了什么东西，容易让整理多个文件加厚，
 * 不知道哪条历史对应哪个」
 *
 * ## 契约层面的根因
 *
 * 不是渲染 bug，而是**契约设计遗漏**：`RunReport` 里根本没有逐文件字段，
 * 而 `executor/journal.rs` 一直记着每个 operation 的 source/target
 * （撤销页正是靠它渲染清单）。数据在，契约没暴露。
 *
 * 所以这一组用例盯两件事：
 * 1. 明细**按需**、**按 runId** 拉，不塞进 `list_runs`（历史 50 条会让列表变重）；
 * 2. 三种状态（加载中 / 有内容 / 读失败）必须**互相可区分**——
 *    「没有明细」和「明细没读出来」在视觉上一模一样，而用户要做的事截然不同。
 */

import { describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { t } from '../../src/i18n/zh-CN'
import type { RunItems, RunReport } from '../../src/api/contracts.generated'

vi.mock('../../src/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/client')>()
  return { ...actual, call: vi.fn() }
})

import { call, IpcError } from '../../src/api/client'
import { HistoryPage } from '../../src/features/history/HistoryPage'

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

function run(overrides: Partial<RunReport> = {}): RunReport {
  return {
    runId: 'run-aaaa1111',
    planId: 'plan-bbbb2222',
    direction: 'apply',
    stateDigest: 'digest-1',
    status: 'completed',
    counts: { applied: 8, failed: 0, skipped: 0, pending: 0, ambiguous: 0 },
    issues: [],
    ...overrides,
  } as RunReport
}

/** 一次「顺利的整理」产生的明细：8 项全部 applied，没有任何 issue。 */
function appliedItems(runId: string): RunItems {
  return {
    runId,
    items: Array.from({ length: 8 }, (_, index) => ({
      itemId: `item-${index}`,
      source: [`未整理-${index}.pdf`],
      target: ['文档', `未整理-${index}.pdf`],
      status: 'applied' as const,
      resolution: 'open' as const,
      errorCode: null,
    })),
  }
}

async function expandFirstRun(): Promise<void> {
  await userEvent.click(await screen.findByRole('button', { name: t.history.detail }))
}

describe('PR-003：历史明细', () => {
  it('展开一条顺利的整理，能看到具体文件路径，而不是只有「没有发现问题」', async () => {
    /*
      这一条直接对应缺陷现场：一次顺利的整理 `issues` 为空，
      原实现的明细区只渲染 `IssueList`，于是展开后除了
      「没有发现问题。」什么都没有——用户以为功能坏了。
    */
    const theRun = run()
    stubIpc({
      list_runs: [theRun],
      get_run_items: appliedItems(theRun.runId),
    })
    render(<HistoryPage />)
    await expandFirstRun()

    // 逐文件清单必须出现
    expect(await screen.findByText('未整理-0.pdf')).toBeInTheDocument()
    // 目标路径是「文档 + 文件名」，要能看到落在哪个目录
    expect(screen.getByText(/文档\s*[\\/]\s*未整理-7\.pdf/)).toBeInTheDocument()
    // 而且不能只剩下「没有发现问题。」
    expect(screen.getByText(t.history.itemDetailHeading)).toBeInTheDocument()
  })

  it('明细按 runId 单独拉，不在 list_runs 里带', async () => {
    /*
      这是有意设计的克制：历史可达 50 条、每条 8+ 项，
      一次性带回全部路径会让 list_runs 变重。
      MASTER_PLAN:387 对历史分页提出过「不将正文带回前端」的同一原则。
    */
    const theRun = run()
    stubIpc({ list_runs: [theRun], get_run_items: appliedItems(theRun.runId) })
    render(<HistoryPage />)

    // 只列列表时**不该**有人去拉明细
    await screen.findByText(t.history.detail)
    expect(mockedCall).not.toHaveBeenCalledWith('get_run_items', expect.anything())

    await expandFirstRun()
    await waitFor(() => {
      expect(mockedCall).toHaveBeenCalledWith('get_run_items', { runId: theRun.runId })
    })
  })

  it('明细尚未返回时显示加载态，而不是空白', async () => {
    const theRun = run()
    stubIpc({
      list_runs: [theRun],
      // 挂住：观察「正在读取」这个中间态
      get_run_items: new Promise(() => {}),
    })
    render(<HistoryPage />)
    await expandFirstRun()

    expect(await screen.findByText(t.history.itemDetailLoading)).toBeInTheDocument()
  })

  it('明细读取失败时给出错误与重试入口，不伪装成「没有明细」', async () => {
    /*
      「读不出来」和「本来就没有」必须分开：
      前者用户该点重试，后者用户什么都不用做。
      两者长得一样的话，用户会以为这次整理真的没动任何文件——
      那是一个关于**他的文件去了哪里**的错误结论。
    */
    stubIpc({
      list_runs: [run()],
      get_run_items: new Error('数据库暂时不可用'),
    })
    render(<HistoryPage />)
    await expandFirstRun()

    expect(await screen.findByRole('alert')).toHaveTextContent(t.history.itemDetailLoadFailed)
    expect(screen.getByRole('button', { name: t.history.itemRetry })).toBeInTheDocument()
  })

  it('重试会重新发起请求', async () => {
    const theRun = run()
    let attempt = 0
    mockedCall.mockImplementation(((command: string) => {
      if (command === 'list_runs') return Promise.resolve([theRun])
      if (command === 'get_run_items') {
        attempt += 1
        return attempt === 1
          ? Promise.reject(new Error('第一次失败'))
          : Promise.resolve(appliedItems(theRun.runId))
      }
      return Promise.reject(new Error(`测试未预设命令 ${command}`))
    }) as typeof call)

    render(<HistoryPage />)
    await expandFirstRun()
    await userEvent.click(await screen.findByRole('button', { name: t.history.itemRetry }))

    expect(await screen.findByText('未整理-0.pdf')).toBeInTheDocument()
    expect(attempt).toBe(2)
  })

  it('run 不存在（返回 null）走错误态而不是空态', async () => {
    /*
      后端对「查不到这个 runId」返回 null。这不是「没有明细」，
      而是「这条记录已经不在了」——按空态渲染会说成
      「这次执行没有产生任何文件操作记录」，那是错的。
    */
    stubIpc({ list_runs: [run()], get_run_items: null })
    render(<HistoryPage />)
    await expandFirstRun()

    expect(await screen.findByRole('alert')).toHaveTextContent(t.history.itemDetailLoadFailed)
    expect(screen.queryByText(t.history.itemDetailEmpty)).not.toBeInTheDocument()
  })

  it('明细为空时才说「没有产生任何文件操作记录」', async () => {
    const theRun = run()
    stubIpc({ list_runs: [theRun], get_run_items: { runId: theRun.runId, items: [] } })
    render(<HistoryPage />)
    await expandFirstRun()

    expect(await screen.findByText(t.history.itemDetailEmpty)).toBeInTheDocument()
  })

  it('失败的项被标出来，并带上错误码', async () => {
    const theRun = run({
      status: 'partial',
      counts: { applied: 1, failed: 1, skipped: 0, pending: 0, ambiguous: 0 },
    })
    stubIpc({
      list_runs: [theRun],
      get_run_items: {
        runId: theRun.runId,
        items: [
          {
            itemId: 'ok',
            source: ['a.txt'],
            target: ['文档', 'a.txt'],
            status: 'applied',
            resolution: 'open',
            errorCode: null,
          },
          {
            itemId: 'bad',
            source: ['b.txt'],
            target: ['文档', 'b.txt'],
            status: 'failed',
            resolution: 'open',
            errorCode: 'FILE_BUSY',
          },
        ],
      },
    })
    render(<HistoryPage />)
    await expandFirstRun()

    // 错误码是用户去查日志、提问题的唯一线索，必须显示出来
    expect(await screen.findByText('FILE_BUSY')).toBeInTheDocument()
    // 汇总句要说清「几项没成」，而不是让用户自己数
    expect(screen.getByText(/共\s*2\s*项，其中\s*1\s*项未完成/)).toBeInTheDocument()
  })

  it('列表行显示计划标识，用来区分「哪条历史对应哪次整理」', async () => {
    /*
      这一条解决用户原话的后半句：「不知道哪条历史对应哪个」。
      只给「已完成 8 · 0 · 0」时，两次各 8 个文件的整理长得一模一样。
      planId 本来就随 RunReport 返回，零额外成本。
    */
    stubIpc({ list_runs: [run()] })
    render(<HistoryPage />)

    // 截断到 8 位，plan-bbbb2222 的前 8 位是 plan-bbb
    expect(await screen.findByText(/plan-bbb/)).toBeInTheDocument()
  })

  it('两次不同的整理在列表上可区分', async () => {
    stubIpc({
      list_runs: [
        run({ runId: 'run-1', planId: 'plan-1111aaaa' }),
        run({ runId: 'run-2', planId: 'plan-2222bbbb' }),
      ],
    })
    render(<HistoryPage />)

    // 断言与前 8 位截断保持一致，别写成完整的 planId
    expect(await screen.findByText(/plan-111/)).toBeInTheDocument()
    expect(screen.getByText(/plan-222/)).toBeInTheDocument()
  })
})

describe('PR-003：契约形状与生成物对齐', () => {
  it('RunItems 的运行时校验器认得后端会发的形状', async () => {
    /*
      这一条防的是 8.8 那个白屏缺陷的同类复发：
      `deny_unknown_fields` + 预编译校验器意味着——Rust 类型改了但
      忘了重跑 `pnpm contracts:generate`，前端校验的就是旧形状，
      而那种失败发生在**运行时**，表现为「页面打不开」。
    */
    const { check } = await import('../../src/api/validation')
    const outcome = check('RunItems', appliedItems('run-1'))
    expect(outcome.valid, `校验失败：${outcome.errors.join('; ')}`).toBe(true)
  })

  it('RunItems 拒绝未知字段（deny_unknown_fields 的运行时体现）', async () => {
    const { check } = await import('../../src/api/validation')
    const outcome = check('RunItems', {
      runId: 'run-1',
      items: [],
      extraField: '多余字段',
    })
    expect(outcome.valid).toBe(false)
  })
})

/** IpcError 至少要能被这个文件用到，避免 import 被 lint 判定为无用。 */
void IpcError
