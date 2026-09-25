/**
 * T08：恢复核对的 UI 接线。
 *
 * 这一页的风险不在「算错了」，而在**看起来能做其实不能**：
 * 让用户对着一个已经过期的摘要点确认，或者把「需要人工核对」的事实
 * 悄悄藏起来，界面会显得一切正常，而这正是数据丢失的来源。
 * 所以这里既有渲染断言，也有交互断言——尤其是「摘要过期必须重新核对」。
 *
 * 有意**不测**「应用自动修复」：这一页本来就不提供那个动作。
 * 如果哪天有人加上了，这里的用例会全部通过，但那正是要警惕的信号。
 */

import { describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { check } from '../../src/api/validation'
import { IpcError } from '../../src/api/client'
import { t } from '../../src/i18n/zh-CN'
import { RecoveryPage } from '../../src/features/recovery/RecoveryPage'
import type { RecoveryItem, RecoveryReport } from '../../src/api/contracts.generated'

vi.mock('../../src/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/client')>()
  return { ...actual, call: vi.fn() }
})

import { call } from '../../src/api/client'

const mockedCall = vi.mocked(call)

/** 让 `call` 按命令名返回预设结果；未预设的命令抛错，避免测试悄悄走到别的分支。 */
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

/** 一个待核对的项：磁盘上两处都有，判定不出来。 */
function unresolvedItem(overrides: Partial<RecoveryItem> = {}): RecoveryItem {
  return {
    operationId: 'op-1',
    itemId: 'item-1',
    source: ['报告', 'a.txt'],
    target: ['文档', 'a.txt'],
    status: 'ambiguous',
    resolution: 'open',
    message: '原位置与整理后位置都存在同名文件，无法判断哪一份是本次结果。',
    ...overrides,
  }
}

/** 已被用户确认保留现状的项。 */
function settledItem(overrides: Partial<RecoveryItem> = {}): RecoveryItem {
  return unresolvedItem({
    operationId: 'op-2',
    itemId: 'item-2',
    source: ['报告', 'b.txt'],
    target: ['文档', 'b.txt'],
    resolution: 'acknowledged',
    ...overrides,
  })
}

function recoveryReport(overrides: Partial<RecoveryReport> = {}): RecoveryReport {
  return {
    runId: 'run-1',
    planId: 'plan-1',
    status: 'recoveryRequired',
    stateDigest: 'digest-1',
    items: [unresolvedItem()],
    counts: { applied: 1, failed: 0, skipped: 0, pending: 0, ambiguous: 1 },
    blocksNewRuns: true,
    rootAuthorized: true,
    ...overrides,
  }
}

describe('恢复页：夹具本身必须合法', () => {
  it('RecoveryReport 夹具通过运行时契约校验', () => {
    // 否则下面的 UI 断言可能在「测试数据本身就不合法」的前提下通过，
    // 而真机上同样的数据会被 decodeResponse 直接拒掉。
    expect(check('RecoveryReport', recoveryReport()).errors).toEqual([])
    expect(
      check(
        'RecoveryReport',
        recoveryReport({
          items: [unresolvedItem(), settledItem()],
          blocksNewRuns: false,
        }),
      ).errors,
    ).toEqual([])
  })
})

describe('恢复页：入口与空态', () => {
  it('没有执行记录时给出明确空态，并指明从哪里进来', () => {
    stubIpc({})
    render(<RecoveryPage runId={null} />)

    expect(screen.getByText(t.recovery.noRun)).toBeInTheDocument()
    expect(screen.getByText(t.recovery.noRunHint)).toBeInTheDocument()
    // 没有 runId 就不该发起 IPC，否则「空态」会闪一下就变成错误
    expect(mockedCall).not.toHaveBeenCalled()
  })
})

describe('恢复页：加载与错误', () => {
  it('加载完成后把每一项的原位置与整理后位置都摆出来', async () => {
    stubIpc({ get_recovery: recoveryReport() })
    render(<RecoveryPage runId="run-1" />)

    expect(await screen.findByText('报告\\a.txt')).toBeInTheDocument()
    expect(screen.getByText('文档\\a.txt')).toBeInTheDocument()
    expect(mockedCall).toHaveBeenCalledWith('get_recovery', { runId: 'run-1' })
  })

  it('后端的判定理由原样展示，而不是界面自己编一句话', async () => {
    const message = '两个位置都能看到这个文件，无法判断哪一份是本次移动的结果。'
    stubIpc({ get_recovery: recoveryReport({ items: [unresolvedItem({ message })] }) })
    render(<RecoveryPage runId="run-1" />)

    expect(await screen.findByText(message)).toBeInTheDocument()
  })

  it('读取失败时给出错误，而不是一直停在加载中', async () => {
    stubIpc({
      get_recovery: new IpcError({
        code: 'DB_UNAVAILABLE',
        message: '本地数据库暂时不可用。',
        retryable: true,
        details: {},
      }),
    })
    render(<RecoveryPage runId="run-1" />)

    expect(await screen.findByRole('alert')).toHaveTextContent('本地数据库暂时不可用。')
    expect(screen.queryByText(t.common.loading)).not.toBeInTheDocument()
  })
})

describe('恢复页：待核对与已确认必须分开', () => {
  it('已确认的项仍然列出来，但不会混进待核对清单', async () => {
    stubIpc({
      get_recovery: recoveryReport({
        items: [unresolvedItem(), settledItem()],
        blocksNewRuns: true,
      }),
    })
    render(<RecoveryPage runId="run-1" />)

    const pendingHeading = await screen.findByText(t.recovery.pendingHeading)
    const pendingList = pendingHeading.nextElementSibling as HTMLElement
    expect(within(pendingList).getByText('报告\\a.txt')).toBeInTheDocument()
    expect(
      within(pendingList).queryByText('报告\\b.txt'),
      '已确认的项不该再要求用户核对一次',
    ).not.toBeInTheDocument()

    // 但它必须还在页面上——藏起来会让用户以为那两份文件已经被处理掉了
    const settledHeading = screen.getByText(t.recovery.settledHeading)
    const settledList = settledHeading.nextElementSibling as HTMLElement
    expect(within(settledList).getByText('报告\\b.txt')).toBeInTheDocument()
    expect(within(settledList).getByText(t.recovery.settledNote)).toBeInTheDocument()
  })

  it('全部确认后不再显示待核对清单，也不显示确认区', async () => {
    stubIpc({
      get_recovery: recoveryReport({
        items: [settledItem()],
        blocksNewRuns: false,
        counts: { applied: 1, failed: 0, skipped: 0, pending: 0, ambiguous: 0 },
      }),
    })
    render(<RecoveryPage runId="run-1" />)

    expect(await screen.findByText(t.recovery.clear)).toBeInTheDocument()
    expect(screen.getByText(t.recovery.pendingEmpty)).toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: t.recovery.acknowledgeButton }),
    ).not.toBeInTheDocument()
  })
})

describe('恢复页：阻塞说明', () => {
  it('仍有未决项时明确说「不接受新的整理任务」', async () => {
    stubIpc({ get_recovery: recoveryReport({ blocksNewRuns: true }) })
    render(<RecoveryPage runId="run-1" />)

    expect(await screen.findByText(t.recovery.blocking)).toBeInTheDocument()
    expect(screen.queryByText(t.recovery.clear)).not.toBeInTheDocument()
  })

  it('根目录授权失效时如实说明，并且不允许确认', async () => {
    stubIpc({ get_recovery: recoveryReport({ rootAuthorized: false }) })
    render(<RecoveryPage runId="run-1" />)

    expect(await screen.findByText(t.recovery.rootUnauthorized)).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: t.recovery.acknowledgeButton }),
    ).toBeDisabled()
    expect(screen.getByText(t.recovery.acknowledgeDisabled)).toBeInTheDocument()
  })
})

describe('恢复页：确认保留现状', () => {
  it('没写理由时拒绝提交，并给出可指导行动的提示', async () => {
    stubIpc({ get_recovery: recoveryReport() })
    render(<RecoveryPage runId="run-1" />)

    await userEvent.click(await screen.findByRole('button', { name: t.recovery.acknowledgeButton }))

    expect(screen.getByText(t.recovery.reasonRequired)).toBeInTheDocument()
    // 只调过 get_recovery：这次点击根本没有发出确认请求
    expect(mockedCall).toHaveBeenCalledTimes(1)
    expect(mockedCall).not.toHaveBeenCalledWith('acknowledge_recovery', expect.anything())
  })

  it('只写空白字符同样算没写理由', async () => {
    stubIpc({ get_recovery: recoveryReport() })
    render(<RecoveryPage runId="run-1" />)

    await userEvent.type(
      await screen.findByRole('textbox', { name: t.recovery.reasonLabel }),
      '   ',
    )
    await userEvent.click(screen.getByRole('button', { name: t.recovery.acknowledgeButton }))

    expect(screen.getByText(t.recovery.reasonRequired)).toBeInTheDocument()
    expect(mockedCall).not.toHaveBeenCalledWith('acknowledge_recovery', expect.anything())
  })

  it('确认时带上屏幕上的摘要与去除首尾空白的理由', async () => {
    const acked = recoveryReport({
      stateDigest: 'digest-2',
      items: [settledItem()],
      blocksNewRuns: false,
      counts: { applied: 1, failed: 0, skipped: 0, pending: 0, ambiguous: 0 },
    })
    stubIpc({ get_recovery: recoveryReport(), acknowledge_recovery: acked })
    const onSettled = vi.fn()
    render(<RecoveryPage runId="run-1" onSettled={onSettled} />)

    await userEvent.type(
      await screen.findByRole('textbox', { name: t.recovery.reasonLabel }),
      '  两份都要留  ',
    )
    await userEvent.click(screen.getByRole('button', { name: t.recovery.acknowledgeButton }))

    await waitFor(() => {
      expect(mockedCall).toHaveBeenCalledWith('acknowledge_recovery', {
        runId: 'run-1',
        // 这里的 digest 必须是**这一屏看到的那一份**，
        // 换成别的值就等于让后端去校验一个用户没看过的状态
        stateDigest: 'digest-1',
        reason: '两份都要留',
      })
    })
  })

  it('一批里还剩别的项时，确认后清空理由输入框', async () => {
    // 用「确认掉第一项、还有第二项待核对」的场景：这时确认区仍然存在，
    // 输入框留着上一段的理由，用户很容易连点两次把同一段理由套到别的事实上。
    stubIpc({
      get_recovery: recoveryReport({
        items: [unresolvedItem(), unresolvedItem({ operationId: 'op-9', itemId: 'item-9' })],
        counts: { applied: 0, failed: 0, skipped: 0, pending: 0, ambiguous: 2 },
      }),
      acknowledge_recovery: recoveryReport({
        stateDigest: 'digest-2',
        blocksNewRuns: true,
        items: [settledItem(), unresolvedItem({ operationId: 'op-9', itemId: 'item-9' })],
      }),
    })
    render(<RecoveryPage runId="run-1" />)

    const input = await screen.findByRole('textbox', { name: t.recovery.reasonLabel })
    await userEvent.type(input, '两份都要留')
    await userEvent.click(screen.getByRole('button', { name: t.recovery.acknowledgeButton }))

    await screen.findByText(t.recovery.acknowledgedStillBlocked)
    await waitFor(() => expect(input).toHaveValue(''))
  })

  it('全部处理完时通知外层，仍有未决项时不通知', async () => {
    const onSettled = vi.fn()
    stubIpc({
      get_recovery: recoveryReport(),
      acknowledge_recovery: recoveryReport({
        stateDigest: 'digest-2',
        blocksNewRuns: true,
        items: [unresolvedItem(), settledItem()],
      }),
    })
    render(<RecoveryPage runId="run-1" onSettled={onSettled} />)

    await userEvent.type(
      await screen.findByRole('textbox', { name: t.recovery.reasonLabel }),
      '先记这一项',
    )
    await userEvent.click(screen.getByRole('button', { name: t.recovery.acknowledgeButton }))

    expect(await screen.findByText(t.recovery.acknowledgedStillBlocked)).toBeInTheDocument()
    expect(onSettled, '还有未决项就不能告诉外层「已经清干净了」').not.toHaveBeenCalled()
  })

  it('全部处理完时既要提示用户，也要通知外层', async () => {
    const onSettled = vi.fn()
    stubIpc({
      get_recovery: recoveryReport(),
      acknowledge_recovery: recoveryReport({
        stateDigest: 'digest-2',
        items: [settledItem()],
        blocksNewRuns: false,
      }),
    })
    render(<RecoveryPage runId="run-1" onSettled={onSettled} />)

    await userEvent.type(
      await screen.findByRole('textbox', { name: t.recovery.reasonLabel }),
      '两份都要留',
    )
    await userEvent.click(screen.getByRole('button', { name: t.recovery.acknowledgeButton }))

    expect(await screen.findByText(t.recovery.acknowledged)).toBeInTheDocument()
    expect(onSettled).toHaveBeenCalledTimes(1)
  })
})

describe('恢复页：摘要过期', () => {
  it('STALE_PLAN 时提示「请再看一遍」，并自动重新核对', async () => {
    const stale = new IpcError({
      code: 'STALE_PLAN',
      message: '原始报文里的技术措辞，不该出现在这里',
      retryable: false,
      details: {},
    })

    let attempt = 0
    mockedCall.mockImplementation(((command: string) => {
      if (command === 'get_recovery') {
        attempt += 1
        return Promise.resolve(recoveryReport({ stateDigest: `digest-${attempt}` }))
      }
      if (command === 'acknowledge_recovery') return Promise.reject(stale)
      return Promise.reject(new Error(`测试未预设命令 ${command}`))
    }) as typeof call)

    render(<RecoveryPage runId="run-1" />)

    await userEvent.type(
      await screen.findByRole('textbox', { name: t.recovery.reasonLabel }),
      '两份都要留',
    )
    await userEvent.click(screen.getByRole('button', { name: t.recovery.acknowledgeButton }))

    expect(await screen.findByText(t.recovery.stale)).toBeInTheDocument()
    // 关键：不能只提示，还得把新的事实拉回来——
    // 否则用户看着「请再看一遍」却只能自己猜哪里变了。
    await waitFor(() =>
      expect(mockedCall).toHaveBeenCalledWith('get_recovery', { runId: 'run-1' }),
    )
    await waitFor(() => expect(mockedCall).toHaveBeenCalledTimes(3))
  })

  it('非 STALE_PLAN 的错误照实展示，不谎报成摘要过期', async () => {
    stubIpc({
      get_recovery: recoveryReport(),
      acknowledge_recovery: new IpcError({
        code: 'REQUEST_CONFLICT',
        message: '这一项已经不是待确认的状态了。',
        retryable: false,
        details: {},
      }),
    })
    render(<RecoveryPage runId="run-1" />)

    await userEvent.type(
      await screen.findByRole('textbox', { name: t.recovery.reasonLabel }),
      '两份都要留',
    )
    await userEvent.click(screen.getByRole('button', { name: t.recovery.acknowledgeButton }))

    expect(await screen.findByText('这一项已经不是待确认的状态了。')).toBeInTheDocument()
    expect(screen.queryByText(t.recovery.stale)).not.toBeInTheDocument()
  })
})

describe('恢复页：重新核对', () => {
  it('点「重新核对」会重新拉取磁盘事实', async () => {
    stubIpc({ get_recovery: recoveryReport() })
    render(<RecoveryPage runId="run-1" />)

    await screen.findByText('报告\\a.txt')
    await userEvent.click(screen.getByRole('button', { name: t.recovery.recheck }))

    await waitFor(() => expect(mockedCall).toHaveBeenCalledTimes(2))
  })
})
