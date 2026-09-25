/**
 * T09：撤销预览与执行的 UI 接线。
 *
 * 这一页的风险与恢复页同类，但更贴近「会动手」：它真的会把用户的文件搬回去。
 * 因此这里重点盯三件事：
 *
 * 1. **有冲突的项默认不选中**（规格 8.4 第 3 条）——不勾的项绝不能被提交；
 * 2. **分项结果要如实**——把部分撤销标成全部成功，用户会以为文件都回去了；
 * 3. **摘要在写之前校验**——过期时提示重新预览，而不是拿着旧事实动手。
 *
 * 有意**不测**「一键全部撤销」：这一页本来就不提供那个动作。
 */

import { describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { check } from '../../src/api/validation'
import { IpcError } from '../../src/api/client'
import { t } from '../../src/i18n/zh-CN'
import { UndoPage } from '../../src/features/undo/UndoPage'
import type { UndoItem, UndoPreview, UndoReport } from '../../src/api/contracts.generated'

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

/** 可以安全撤销的一项。 */
function readyItem(overrides: Partial<UndoItem> = {}): UndoItem {
  return {
    operationId: 'op-1',
    itemId: 'item-1',
    source: ['文档', 'a.txt'],
    target: ['a.txt'],
    outcome: 'ready',
    selected: true,
    message: '可以撤销：文件仍在整理后的位置且内容未变，原位置是空的。',
    ...overrides,
  }
}

/** 有冲突的一项：原位置被占用了。 */
function conflictItem(overrides: Partial<UndoItem> = {}): UndoItem {
  return readyItem({
    operationId: 'op-2',
    itemId: 'item-2',
    source: ['文档', 'b.txt'],
    target: ['b.txt'],
    outcome: 'conflict',
    selected: false,
    message: '原位置已经有了别的文件。撤销不会覆盖它，两个文件都保留。',
    ...overrides,
  })
}

/** 已经撤销过的一项。 */
function undoneItem(overrides: Partial<UndoItem> = {}): UndoItem {
  return readyItem({
    operationId: 'op-3',
    itemId: 'item-3',
    source: ['文档', 'c.txt'],
    target: ['c.txt'],
    outcome: 'alreadyUndone',
    selected: false,
    message: '这一项此前已经撤销过，文件已经在原位置，不会再搬动它。',
    ...overrides,
  })
}

function undoPreview(overrides: Partial<UndoPreview> = {}): UndoPreview {
  const items = overrides.items ?? [readyItem()]
  const ready = items.filter((item) => item.outcome === 'ready').length
  const conflict = items.filter((item) => item.outcome === 'conflict').length
  const undone = items.filter((item) => item.outcome === 'alreadyUndone').length
  return {
    undoPlanId: 'undo-plan-1',
    originalRunId: 'run-1',
    digest: 'digest-1',
    items,
    readyCount: ready,
    conflictCount: conflict,
    alreadyUndoneCount: undone,
    undoToken: 'token-1',
    expiresAt: '2026-09-19T00:05:00.000Z',
    ...overrides,
  }
}

function undoReport(overrides: Partial<UndoReport> = {}): UndoReport {
  return {
    runId: 'undo-run-1',
    originalRunId: 'run-1',
    status: 'completed',
    reverted: 1,
    conflicted: 0,
    alreadyUndone: 0,
    untouched: 0,
    items: [readyItem({ outcome: 'alreadyUndone', selected: true })],
    warnings: [],
    ...overrides,
  }
}

/** 取出最后一次 `call` 的参数。 */
function lastArgs(command: string): Record<string, unknown> {
  const calls = mockedCall.mock.calls.filter(([name]) => name === command)
  expect(calls.length, `没有调用过 ${command}`).toBeGreaterThan(0)
  return (calls[calls.length - 1]?.[1] ?? {}) as Record<string, unknown>
}

describe('撤销页：夹具本身必须合法', () => {
  it('UndoPreview 与 UndoReport 夹具通过运行时契约校验', () => {
    // 否则下面的 UI 断言可能在「测试数据本身就不合法」的前提下通过，
    // 而真机上同样的数据会被 decodeResponse 直接拒掉。
    expect(check('UndoPreview', undoPreview()).errors).toEqual([])
    expect(
      check(
        'UndoPreview',
        undoPreview({ items: [readyItem(), conflictItem(), undoneItem()] }),
      ).errors,
    ).toEqual([])
    expect(check('UndoReport', undoReport()).errors).toEqual([])
  })
})

describe('撤销页：入口与空态', () => {
  it('没有选中整理记录时说明从哪里进', () => {
    stubIpc({})
    render(<UndoPage runId={null} />)

    expect(screen.getByText(t.undo.noRun)).toBeInTheDocument()
    expect(screen.getByText(t.undo.noRunHint)).toBeInTheDocument()
    expect(mockedCall).not.toHaveBeenCalled()
  })

  it('给出 runId 时按 runId 预览', async () => {
    stubIpc({ preview_undo: undoPreview() })
    render(<UndoPage runId="run-1" />)

    await screen.findByText(/可以撤销 1 项/)
    expect(lastArgs('preview_undo')).toMatchObject({ runId: 'run-1' })
  })
})

describe('撤销页：加载与错误', () => {
  it('加载完成前显示加载态', () => {
    stubIpc({ preview_undo: undoPreview() })
    render(<UndoPage runId="run-1" />)

    expect(screen.getByText(t.common.loading)).toBeInTheDocument()
  })

  it('预览失败时显示错误而不是空白', async () => {
    stubIpc({
      preview_undo: new IpcError({
        code: 'ROOT_NOT_AUTHORIZED',
        message: '这个文件夹在本会话里还没有重新授权',
        retryable: false,
        details: {},
      }),
    })
    render(<UndoPage runId="run-1" />)

    expect(await screen.findByText(/还没有重新授权/)).toBeInTheDocument()
  })

  it('重新预览会再调一次 preview_undo', async () => {
    stubIpc({ preview_undo: undoPreview() })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(/可以撤销 1 项/)

    await userEvent.click(screen.getByRole('button', { name: t.undo.recheck }))

    await waitFor(() => expect(mockedCall).toHaveBeenCalledTimes(2))
  })
})

describe('撤销页：分项展示', () => {
  it('把「可以撤销 / 有冲突 / 已撤销」分开列出，冲突项根本不可勾选', async () => {
    stubIpc({
      preview_undo: undoPreview({
        items: [readyItem(), conflictItem(), undoneItem()],
      }),
    })
    render(<UndoPage runId="run-1" />)

    await screen.findByText(t.undo.readyHeading)
    expect(screen.getByText(t.undo.conflictHeading)).toBeInTheDocument()
    expect(screen.getByText(t.undo.undoneHeading)).toBeInTheDocument()

    // 规格 8.4 第 3 条：冲突项默认不选中。这里做得更硬一点——
    // 冲突项**没有复选框**，因此它根本没有被提交的可能，
    // 而不是「渲染成一个未勾选的框」等着谁来点。
    expect(screen.getByText(t.undo.conflictNote)).toBeInTheDocument()
    const boxes = screen.getAllByRole('checkbox')
    expect(boxes).toHaveLength(1)
    expect(boxes[0]).toBeChecked()
  })

  it('全部已撤销时给出「无需再做」的结论，且不出现确认按钮', async () => {
    stubIpc({ preview_undo: undoPreview({ items: [undoneItem()] }) })
    render(<UndoPage runId="run-1" />)

    expect(await screen.findByText(t.undo.allClear)).toBeInTheDocument()
    expect(screen.getByText(t.undo.undoneHeading)).toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: t.undo.executeButton }),
    ).not.toBeInTheDocument()
  })
})

describe('撤销页：提交的内容必须与屏幕上看到的一致', () => {
  it('只提交勾选的那些项', async () => {
    stubIpc({
      preview_undo: undoPreview({ items: [readyItem(), conflictItem()] }),
      execute_undo: undoReport(),
    })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    await waitFor(() => expect(lastArgs('execute_undo')).toBeTruthy())
    const args = lastArgs('execute_undo')
    expect(args.selected).toEqual(['op-1'])
    // 摘要与令牌必须原样带回：后端靠它们拒绝一份过期的确认。
    expect(args.digest).toBe('digest-1')
    expect(args.undoToken).toBe('token-1')
    expect(args.undoPlanId).toBe('undo-plan-1')
    expect(args.originalRunId).toBe('run-1')
  })

  it('取消勾选之后就不再提交那一项', async () => {
    stubIpc({
      preview_undo: undoPreview({ items: [readyItem(), readyItem({ operationId: 'op-9', itemId: 'item-9', source: ['文档', 'z.txt'], target: ['z.txt'] })] }),
      execute_undo: undoReport({ reverted: 1, untouched: 1, status: 'partial' }),
    })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('checkbox', { name: /z\.txt/ }))
    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    await waitFor(() => expect(lastArgs('execute_undo')).toBeTruthy())
    expect(lastArgs('execute_undo').selected).toEqual(['op-1'])
  })

  it('一项都没勾选时执行按钮不可用', async () => {
    stubIpc({ preview_undo: undoPreview({ items: [readyItem()] }) })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('checkbox', { name: /a\.txt/ }))

    expect(screen.getByRole('button', { name: t.undo.executeButton })).toBeDisabled()
    expect(mockedCall).toHaveBeenCalledTimes(1)
  })
})

describe('撤销页：分项结果不能报成全部成功', () => {
  it('收到报告后保留结果，只有用户返回历史时才离开', async () => {
    const onDone = vi.fn()
    stubIpc({
      preview_undo: undoPreview({ items: [readyItem()] }),
      execute_undo: undoReport(),
    })
    render(<UndoPage runId="run-1" onDone={onDone} />)
    await screen.findByText(t.undo.readyHeading)
    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    expect(await screen.findByText(t.undo.resultHeading)).toBeInTheDocument()
    expect(onDone).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: t.undo.backToHistory }))
    expect(onDone).toHaveBeenCalledTimes(1)
  })

  it('需要恢复时绝不显示全部成功', async () => {
    stubIpc({
      preview_undo: undoPreview({ items: [readyItem()] }),
      execute_undo: undoReport({ status: 'recoveryRequired', reverted: 0 }),
    })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)
    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    expect(await screen.findByText(t.undo.recoveryRequired)).toBeInTheDocument()
    expect(screen.queryByText(t.undo.completed)).not.toBeInTheDocument()
  })

  it('有冲突时明确说「没有全部撤回去」', async () => {
    stubIpc({
      preview_undo: undoPreview({ items: [readyItem()] }),
      execute_undo: undoReport({ status: 'partial', reverted: 1, conflicted: 1 }),
    })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    expect(await screen.findByText(t.undo.partialWarning)).toBeInTheDocument()
    // 分项计数要逐条能看见，而不是只给一句「完成」。
    expect(screen.getByText(`${t.undo.resultConflicted} 1`)).toBeInTheDocument()
    expect(screen.getByText(`${t.undo.resultReverted} 1`)).toBeInTheDocument()
  })

  it('全部成功时才说「都已搬回」', async () => {
    stubIpc({
      preview_undo: undoPreview({ items: [readyItem()] }),
      execute_undo: undoReport({ status: 'completed', reverted: 1 }),
    })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    expect(await screen.findByText(t.undo.completed)).toBeInTheDocument()
  })

  it('目录清理失败的告警单独列出，并说明文件本身是安全的', async () => {
    stubIpc({
      preview_undo: undoPreview({ items: [readyItem()] }),
      execute_undo: undoReport({
        warnings: [
          { code: 'UNDO_CONFLICT', severity: 'warning', itemId: null, message: '目录「文档」未能清理：目录未清空' },
        ],
      }),
    })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    expect(await screen.findByText(t.undo.warningsHeading)).toBeInTheDocument()
    expect(screen.getByText(/目录「文档」未能清理/)).toBeInTheDocument()
    // 文件已经安全回去了，不能把这条告警说成撤销失败。
    expect(screen.getByText(t.undo.completed)).toBeInTheDocument()
  })
})

describe('撤销页：摘要过期', () => {
  it('STALE_PLAN 时提示「请再看一遍」，并自动重新预览', async () => {
    let attempt = 0
    mockedCall.mockImplementation(((command: string) => {
      if (command === 'preview_undo') {
        attempt += 1
        return Promise.resolve(undoPreview({ digest: `digest-${attempt}` }))
      }
      if (command === 'execute_undo') {
        return Promise.reject(
          new IpcError({
            code: 'STALE_PLAN',
            message: '撤销预览之后文件状态发生了变化',
            retryable: false,
            details: {},
          }),
        )
      }
      return Promise.reject(new Error(`测试未预设命令 ${command}`))
    }) as typeof call)

    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    expect(await screen.findByText(t.undo.stale)).toBeInTheDocument()
    // 提示之后必须真的重新拿一份事实，否则用户只能对着失效的摘要反复点。
    await waitFor(() => expect(attempt).toBe(2))
  })

  it('任务忙时给出可以照做的解释', async () => {
    stubIpc({
      preview_undo: undoPreview(),
      execute_undo: new IpcError({
        code: 'TASK_BUSY',
        message: '已经有一个整理任务正在执行',
        retryable: false,
        details: {},
      }),
    })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    expect(await screen.findByText(t.undo.busy)).toBeInTheDocument()
  })
})

describe('撤销页：按钮的可点性要有理由', () => {
  it('没有令牌时确认按钮不出现（没有任何可撤销的项）', async () => {
    stubIpc({ preview_undo: undoPreview({ items: [conflictItem()], undoToken: null }) })
    render(<UndoPage runId="run-1" />)

    await screen.findByText(t.undo.conflictHeading)
    expect(
      screen.queryByRole('button', { name: t.undo.executeButton }),
    ).not.toBeInTheDocument()
  })

  it('外部标记为忙时确认按钮不可用', async () => {
    stubIpc({ preview_undo: undoPreview() })
    render(<UndoPage runId="run-1" busy />)
    await screen.findByText(t.undo.readyHeading)

    expect(screen.getByRole('button', { name: t.undo.executeButton })).toBeDisabled()
  })

  it('每个项都显示可操作的原因说明', async () => {
    stubIpc({
      preview_undo: undoPreview({ items: [readyItem(), conflictItem(), undoneItem()] }),
    })
    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    // 判定文案来自后端对磁盘的核对，直接展示给用户——不替换成内部状态描述。
    expect(screen.getByText(readyItem().message)).toBeInTheDocument()
    expect(screen.getByText(conflictItem().message)).toBeInTheDocument()
    expect(screen.getByText(undoneItem().message)).toBeInTheDocument()
  })
})

describe('撤销页：执行期间的状态', () => {
  it('提交中按钮不可重复点击，双击也只发一个请求', async () => {
    mockedCall.mockImplementation(((command: string) => {
      if (command === 'preview_undo') return Promise.resolve(undoPreview())
      // 永不 resolve：把界面定格在「进行中」那一刻，好在那里做断言。
      if (command === 'execute_undo') return new Promise(() => {})
      return Promise.reject(new Error(`测试未预设命令 ${command}`))
    }) as typeof call)

    render(<UndoPage runId="run-1" />)
    await screen.findByText(t.undo.readyHeading)

    await userEvent.click(screen.getByRole('button', { name: t.undo.executeButton }))

    const pending = screen.getByRole('button', { name: t.undo.executing })
    expect(pending).toBeDisabled()

    // 规格 INV-10：重复点击不得让同一操作执行两次。
    await userEvent.click(pending)
    expect(
      mockedCall.mock.calls.filter(([name]) => name === 'execute_undo'),
    ).toHaveLength(1)
  })
})
