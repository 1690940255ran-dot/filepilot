/**
 * 规格 10.3 的确认有效性边界，以及预览页的真实 UI 接线。
 *
 * 规格明确要求：**不能只测纯函数而漏掉 UI 接线**。
 * 一个「逻辑正确但按钮没接上」的实现，用户照样会在不该点的时候点到按钮。
 * 所以这个文件里既有 `isConfirmationCurrent` 的边界用例，也有 Testing Library 的交互断言。
 */

import { describe, expect, it } from 'vitest'

import {
  EMPTY_CONFIRMATION,
  effectivePlan,
  hasBlockingIssues,
  hasPendingEdits,
  INITIAL_PREVIEW_STATE,
  isConfirmationCurrent,
  previewReducer,
  selectedCount,
  splitPath,
  type ConfirmationView,
  type PreviewState,
} from '../../src/features/preview/previewReducer'
import type {
  Issue,
  Plan,
  PlanItem,
  ValidationReport,
} from '../../src/api/contracts.generated'

// ---------------------------------------------------------------------------
// 规格 10.3 给出的边界用例，逐字保留
// ---------------------------------------------------------------------------

describe('规格 10.3：isConfirmationCurrent 边界', () => {
  it('invalidates confirmation after edits and at expiry', () => {
    const ready = {
      planRevision: 2,
      reportRevision: 2,
      executableCount: 3,
      hasBlockingIssues: false,
      token: 'test-token',
      expiresAtMs: 2000,
    }
    expect(isConfirmationCurrent(ready, 1000)).toBe(true)
    expect(isConfirmationCurrent({ ...ready, planRevision: 3 }, 1000)).toBe(false)
    expect(isConfirmationCurrent(ready, 2000)).toBe(false)
    expect(isConfirmationCurrent({ ...ready, executableCount: 0 }, 1000)).toBe(false)
    expect(isConfirmationCurrent({ ...ready, hasBlockingIssues: true }, 1000)).toBe(false)
  })

  it('没有令牌就不能确认', () => {
    const view: ConfirmationView = {
      planRevision: 1,
      reportRevision: 1,
      executableCount: 2,
      hasBlockingIssues: false,
      token: null,
      expiresAtMs: 9_999_999,
    }
    expect(isConfirmationCurrent(view, 0)).toBe(false)
  })

  it('过期边界是「恰好到期即无效」', () => {
    const view: ConfirmationView = {
      planRevision: 1,
      reportRevision: 1,
      executableCount: 1,
      hasBlockingIssues: false,
      token: 't',
      expiresAtMs: 1000,
    }
    expect(isConfirmationCurrent(view, 999)).toBe(true)
    expect(isConfirmationCurrent(view, 1000)).toBe(false)
    expect(isConfirmationCurrent(view, 1001)).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// reducer 行为
// ---------------------------------------------------------------------------

function item(id: string, overrides: Partial<PlanItem> = {}): PlanItem {
  return {
    id,
    fileId: `file-${id}`,
    source: ['a.txt'],
    target: ['文档', 'a.txt'],
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
    ...overrides,
  } as PlanItem
}

function plan(revision: number, items: PlanItem[]): Plan {
  return {
    id: 'plan-1',
    rootId: 'root-1',
    scanId: 'scan-1',
    revision,
    mode: 'rules',
    status: 'draft',
    items,
    createdAt: '2026-09-17T00:00:00Z',
  } as Plan
}

function report(
  revision: number,
  issues: Issue[] = [],
  executableCount = 1,
  expiresInMs = 5 * 60 * 1000,
): ValidationReport {
  return {
    planId: 'plan-1',
    revision,
    digest: 'digest-1',
    executableCount,
    issues,
    validationToken: 'token-1',
    // 必须是**未来**的时间：令牌过期后按钮本来就该禁用，
    // 用固定过去时刻的夹具会让「按钮可用」这类断言永远失败，
    // 而且掩盖掉真正的过期逻辑。
    expiresAt: new Date(Date.now() + expiresInMs).toISOString(),
  } as ValidationReport
}

const blocking: Issue = {
  code: 'TARGET_EXISTS',
  severity: 'block',
  itemId: 'i1',
  message: '目标已存在',
}

describe('previewReducer', () => {
  it('加载计划后清空草稿与既有确认', () => {
    const dirty: PreviewState = {
      ...INITIAL_PREVIEW_STATE,
      drafts: { i1: '别的\\名字.txt' },
      selectionDrafts: { i2: false },
      confirmation: { ...EMPTY_CONFIRMATION, token: 'old' },
    }
    const next = previewReducer(dirty, { type: 'planLoaded', plan: plan(4, [item('i1')]) })

    expect(next.drafts).toEqual({})
    expect(next.selectionDrafts).toEqual({})
    expect(next.confirmation.token).toBeNull()
    expect(next.confirmation.planRevision).toBe(4)
  })

  it('编辑目标会让既有确认立即失效', () => {
    const loaded = previewReducer(INITIAL_PREVIEW_STATE, {
      type: 'planLoaded',
      plan: plan(2, [item('i1')]),
    })
    const confirmed = previewReducer(loaded, {
      type: 'reportLoaded',
      report: report(2),
      token: 'token-1',
      expiresAtMs: 9_999_999_999,
    })
    expect(isConfirmationCurrent(confirmed.confirmation, 1000)).toBe(true)

    const edited = previewReducer(confirmed, {
      type: 'draftTarget',
      itemId: 'i1',
      target: '文档\\新名字.txt',
    })

    expect(edited.confirmation.token).toBeNull()
    expect(isConfirmationCurrent(edited.confirmation, 1000)).toBe(false)
    expect(
      isConfirmationCurrent(
        { ...edited.confirmation, token: 'token-1', expiresAtMs: 9_999_999_999 },
        1000,
      ),
      '即使把令牌塞回去，版本号也必须对不上',
    ).toBe(false)
  })

  it('勾选变化同样是编辑，同样让确认失效', () => {
    const loaded = previewReducer(INITIAL_PREVIEW_STATE, {
      type: 'planLoaded',
      plan: plan(1, [item('i1'), item('i2')]),
    })
    const confirmed = previewReducer(loaded, {
      type: 'reportLoaded',
      report: report(1),
      token: 't',
      expiresAtMs: 9_999_999_999,
    })
    const toggled = previewReducer(confirmed, {
      type: 'draftToggled',
      itemId: 'i2',
      selected: false,
    })
    expect(toggled.confirmation.token).toBeNull()
  })

  it('校验请求发出后又编辑，迟到的校验报告不能恢复确认', () => {
    const loaded = previewReducer(INITIAL_PREVIEW_STATE, {
      type: 'planLoaded',
      plan: plan(1, [item('i1')]),
    })
    const edited = previewReducer(loaded, {
      type: 'draftToggled',
      itemId: 'i1',
      selected: false,
    })
    const late = previewReducer(edited, {
      type: 'reportLoaded',
      report: report(1),
      token: 'late-token',
      expiresAtMs: 9_999_999_999,
    })
    expect(late.confirmation.token).toBeNull()
    expect(hasPendingEdits(late)).toBe(true)
  })

  it('丢弃草稿会清掉本地改动', () => {
    const loaded = previewReducer(INITIAL_PREVIEW_STATE, {
      type: 'planLoaded',
      plan: plan(1, [item('i1')]),
    })
    const edited = previewReducer(loaded, {
      type: 'draftTarget',
      itemId: 'i1',
      target: 'x\\y.txt',
    })
    expect(hasPendingEdits(edited)).toBe(true)

    const discarded = previewReducer(edited, { type: 'draftsDiscarded' })
    expect(hasPendingEdits(discarded)).toBe(false)
    expect(discarded.drafts).toEqual({})
  })

  it('保存失败时保留草稿，不假装已生效', () => {
    const loaded = previewReducer(INITIAL_PREVIEW_STATE, {
      type: 'planLoaded',
      plan: plan(1, [item('i1')]),
    })
    const edited = previewReducer(loaded, {
      type: 'draftTarget',
      itemId: 'i1',
      target: 'x\\y.txt',
    })
    const failed = previewReducer(edited, { type: 'saveFailed', message: '版本冲突' })

    expect(failed.saving).toBe(false)
    expect(failed.error).toBe('版本冲突')
    expect(failed.drafts, '失败后草稿必须还在，否则用户白改了').toEqual(edited.drafts)
  })
})

describe('仅选中项参与摘要与执行', () => {
  it('selectedCount 忽略未选中项', () => {
    const p = plan(1, [
      item('i1', { selected: true }),
      item('i2', { selected: false }),
      item('i3', { selected: true }),
    ])
    expect(selectedCount(p)).toBe(2)
    expect(selectedCount(null)).toBe(0)
  })

  it('effectivePlan 把草稿叠加到计划上，未改动的项保持原样', () => {
    const p = plan(1, [item('i1'), item('i2')])
    const state: PreviewState = {
      ...INITIAL_PREVIEW_STATE,
      plan: p,
      drafts: { i1: '归档\\新名字.txt' },
      selectionDrafts: { i2: false },
    }
    const effective = effectivePlan(state)
    expect(effective?.items[0]!.target).toEqual(['归档', '新名字.txt'])
    expect(effective?.items[0]!.selected).toBe(true)
    expect(effective?.items[1]!.target, '未改动的项应保持原位').toEqual(p.items[1]!.target)
    expect(effective?.items[1]!.selected).toBe(false)
  })

  it('effectivePlan 不修改原始计划对象', () => {
    const p = plan(1, [item('i1')])
    const state: PreviewState = { ...INITIAL_PREVIEW_STATE, plan: p, drafts: { i1: 'x\\y.txt' } }
    effectivePlan(state)
    expect(p.items[0]!.target, '原始计划必须保持不变').toEqual(['文档', 'a.txt'])
  })

  it('路径拆分接受两种分隔符并丢掉空段', () => {
    expect(splitPath('归档\\2026\\a.txt')).toEqual(['归档', '2026', 'a.txt'])
    expect(splitPath('归档/2026/a.txt')).toEqual(['归档', '2026', 'a.txt'])
    expect(splitPath('  归档 \\\\ a.txt ')).toEqual(['归档', 'a.txt'])
    expect(splitPath('')).toEqual([])
  })

  it('hasBlockingIssues 只认 block 级', () => {
    expect(hasBlockingIssues(report(1, [{ ...blocking, severity: 'warning' }]))).toBe(false)
    expect(hasBlockingIssues(report(1, [blocking]))).toBe(true)
    expect(hasBlockingIssues(null)).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// UI 接线：规格明确要求「不能只测纯函数而漏掉 UI 接线」
// ---------------------------------------------------------------------------

vi.mock('../../src/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/client')>()
  return { ...actual, call: vi.fn() }
})

import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, vi } from 'vitest'

import { call } from '../../src/api/client'
import { PreviewPage } from '../../src/features/preview/PreviewPage'
import { t } from '../../src/i18n/zh-CN'

const mockedCall = vi.mocked(call)

/** 让 `call` 按命令名返回预设结果；未预设的命令抛错，避免测试悄悄走到别的分支。 */
function stubIpc(responses: Record<string, unknown>): void {
  mockedCall.mockImplementation(((command: string) => {
    if (command in responses) {
      return Promise.resolve(responses[command])
    }
    return Promise.reject(new Error(`测试未预设命令 ${command}`))
  }) as typeof call)
}

const aPlan = plan(3, [item('i1'), item('i2')])
const noIssues = report(3, [], 2)

describe('预览页 UI 接线', () => {
  it('问题筛选显示校验关联项并保留未显示项的选择', async () => {
    const data = plan(3, [item('i1', { source: ['problem.txt'] }), item('i2', { source: ['normal.txt'] })])
    stubIpc({ get_plan: data, validate_plan: report(3, [{ code: 'TARGET_EXISTS', severity: 'block', itemId: 'i1', message: '目标已存在' }], 0) })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)
    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    await screen.findByText('目标已存在')
    await userEvent.selectOptions(screen.getByRole('combobox', { name: t.preview.filterLabel }), 'conflicts')
    expect(screen.getByRole('checkbox', { name: /problem.txt/ })).toBeChecked()
    expect(screen.queryByRole('checkbox', { name: /normal.txt/ })).not.toBeInTheDocument()
    await userEvent.selectOptions(screen.getByRole('combobox', { name: t.preview.filterLabel }), 'all')
    expect(screen.getByRole('checkbox', { name: /normal.txt/ })).toBeChecked()
  })
  it('搜索后批量取消只影响匹配项，清空搜索不丢选择且旧确认失效', async () => {
    const data = plan(3, [item('i1', { source: ['alpha.txt'] }), item('i2', { source: ['beta.txt'] })])
    stubIpc({ get_plan: data, validate_plan: noIssues })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)
    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    await waitFor(() => expect(screen.getByRole('button', { name: t.preview.confirmButton })).toBeEnabled())
    const search = screen.getByRole('searchbox', { name: '搜索文件或目标路径' })
    await userEvent.type(search, 'alpha')
    expect(screen.getAllByRole('checkbox')).toHaveLength(1)
    await userEvent.click(screen.getByRole('button', { name: '取消筛选项的选择' }))
    await userEvent.clear(search)
    expect(screen.getByRole('checkbox', { name: /alpha.txt/ })).not.toBeChecked()
    expect(screen.getByRole('checkbox', { name: /beta.txt/ })).toBeChecked()
    expect(screen.getByRole('button', { name: t.preview.confirmButton })).toBeDisabled()
  })

  it('没有计划时给出明确空态，而不是一片空白', () => {
    render(<PreviewPage planId={null} rootPath={null} />)
    expect(screen.getByText(t.preview.notReady)).toBeInTheDocument()
  })

  it('加载计划后渲染出每一项的源与目标', async () => {
    stubIpc({ get_plan: aPlan })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    expect((await screen.findAllByText('a.txt')).length).toBeGreaterThan(0)
    expect(screen.getAllByText('文档\\a.txt').length).toBeGreaterThan(0)
  })

  it('验证通过拿到令牌前，「确认并执行」一直是禁用的', async () => {
    stubIpc({ get_plan: aPlan, validate_plan: noIssues })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    const confirm = await screen.findByRole('button', { name: t.preview.confirmButton })
    expect(confirm, '还没点校验就不能确认').toBeDisabled()

    await userEvent.click(screen.getByRole('button', { name: t.preview.validateButton }))

    await waitFor(() => expect(confirm).toBeEnabled())
  })

  it('存在阻断项时不能确认，且理由对用户可见', async () => {
    const blocked = report(3, [blocking], 0)
    stubIpc({ get_plan: aPlan, validate_plan: blocked })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))

    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeDisabled())
    expect(screen.getByText(t.preview.blockedCannotConfirm)).toBeInTheDocument()
  })

  it('全新计划还没校验时，理由说的是「请先校验」而不是「计划已被修改」', async () => {
    // 这一条是截图验收时发现的真实缺陷：全新计划的可执行项数是**未知**，
    // 而界面写着「计划已被修改」和「存在阻断项」——两句都不成立。
    stubIpc({ get_plan: aPlan })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    const confirm = await screen.findByRole('button', { name: t.preview.confirmButton })
    expect(confirm).toBeDisabled()

    expect(screen.getByText(t.preview.needValidate)).toBeInTheDocument()
    expect(screen.queryByText(t.preview.staleAfterEdit)).not.toBeInTheDocument()
    expect(screen.queryByText(t.preview.blockedCannotConfirm)).not.toBeInTheDocument()
  })

  it('校验后被编辑，理由才变成「计划已被修改」', async () => {
    stubIpc({ get_plan: aPlan, validate_plan: noIssues })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeEnabled())

    await userEvent.click(screen.getAllByRole('button', { name: t.preview.editTarget })[0]!)
    const input = screen.getByRole('textbox', { name: t.preview.editTarget })
    await userEvent.clear(input)
    await userEvent.type(input, '归档\\改过名.txt')
    await userEvent.click(screen.getByRole('button', { name: t.preview.applyEdit }))

    await waitFor(() => expect(confirm).toBeDisabled())
    expect(screen.getByText(t.preview.saveBeforeValidate)).toBeInTheDocument()
    expect(screen.queryByText(t.preview.needValidate)).not.toBeInTheDocument()
  })

  it('编辑只让确认失效，不会把「曾经校验过」这件事一起抹掉', () => {
    let state = previewReducer(INITIAL_PREVIEW_STATE, {
      type: 'planLoaded',
      plan: plan(1, [item('i1')]),
    })
    state = previewReducer(state, {
      type: 'reportLoaded',
      report: report(1, [], 1),
      token: 'tok',
      expiresAtMs: 9_999_999,
    })
    expect(state.confirmation.reportRevision).toBe(1)

    state = previewReducer(state, { type: 'draftTarget', itemId: 'i1', target: '归档\\b.txt' })

    expect(state.confirmation.token, '令牌必须失效').toBeNull()
    expect(
      state.confirmation.reportRevision,
      '版本要留下——抹掉它就分不清「从没校验过」和「校验后又被改」',
    ).toBe(1)
  })

  it('编辑目标后旧确认立即失效，确认按钮重新禁用', async () => {
    stubIpc({ get_plan: aPlan, validate_plan: noIssues })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeEnabled())

    // 进入编辑态并改一个名字
    await userEvent.click(screen.getAllByRole('button', { name: t.preview.editTarget })[0]!)
    const input = screen.getByRole('textbox', { name: t.preview.editTarget })
    await userEvent.clear(input)
    await userEvent.type(input, '归档\\改过名.txt')
    await userEvent.click(screen.getByRole('button', { name: t.preview.applyEdit }))

    await waitFor(() => expect(confirm).toBeDisabled())
  })

  it('取消勾选同样是编辑，同样让确认失效', async () => {
    stubIpc({ get_plan: aPlan, validate_plan: noIssues })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeEnabled())

    const boxes = screen.getAllByRole('checkbox')
    await userEvent.click(boxes[0]!)

    await waitFor(() => expect(confirm).toBeDisabled())
  })

  it('未保存的勾选变化不能重新校验，更不能取得执行确认', async () => {
    stubIpc({ get_plan: aPlan, validate_plan: noIssues })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    const checkboxes = await screen.findAllByRole('checkbox', { name: /a\.txt/ })
    await userEvent.click(checkboxes[0]!)

    expect(screen.getByRole('button', { name: t.preview.validateButton })).toBeDisabled()
    expect(screen.getByRole('button', { name: t.preview.confirmButton })).toBeDisabled()
    expect(mockedCall.mock.calls.filter(([command]) => command === 'validate_plan')).toHaveLength(0)
  })

  it('只改勾选并应用时，update_plan 必须包含该项的 selected', async () => {
    stubIpc({ get_plan: aPlan, update_plan: aPlan })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    const checkboxes = await screen.findAllByRole('checkbox', { name: /a\.txt/ })
    await userEvent.click(checkboxes[0]!)
    await userEvent.click(screen.getByRole('button', { name: t.preview.applyEdit }))

    await waitFor(() => {
      expect(mockedCall).toHaveBeenCalledWith('update_plan', {
        planId: 'plan-1',
        expectedRevision: 3,
        edits: [{ itemId: 'i1', selected: false, target: null }],
      })
    })
  })

  it('本阶段明确告知执行尚未接通', async () => {
    stubIpc({ get_plan: aPlan })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    expect(await screen.findByText(t.preview.executeNotAvailable)).toBeInTheDocument()
  })

  it('确认对话框展示真实根目录与选中数量', async () => {
    stubIpc({ get_plan: aPlan, validate_plan: noIssues })
    render(<PreviewPage planId="plan-1" rootPath="C:\\资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeEnabled())
    await userEvent.click(confirm)

    const dialog = await screen.findByRole('dialog')
    // 分两段断言而不是整体比较：
    // 路径含反斜杠，而 jest-dom 的字符串参数会再做一次转义，
    // 直接比较「看起来一样」的两串反而匹配不上。
    expect(dialog).toHaveTextContent('资料')
    expect(dialog).toHaveTextContent(/即将整理\s*2\s*个文件/)
  })
})


// ---------------------------------------------------------------------------
// T07：进度与取消
// ---------------------------------------------------------------------------

vi.mock('../../src/api/events', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/events')>()
  return { ...actual, subscribeTaskProgress: vi.fn() }
})

import { subscribeTaskProgress } from '../../src/api/events'
import type { TaskProgress } from '../../src/api/events'

const mockedSubscribe = vi.mocked(subscribeTaskProgress)

// 模块级的 `vi.mock` 会让**整个文件**都用这个替身，包括不关心进度的那些用例。
// 不给默认实现的话它返回 `undefined`，组件里的 `.then` 会直接抛错——
// 表现是一堆看起来毫不相关的用例集体失败。
beforeEach(() => {
  mockedSubscribe.mockImplementation(async () => () => {})
})

/** 抓住 PreviewPage 注册的处理器，测试里手动喂事件。 */
function captureProgressHandler(): () => (update: TaskProgress) => void {
  let captured: ((update: TaskProgress) => void) | null = null
  mockedSubscribe.mockImplementation(async (handler) => {
    captured = handler
    return () => {}
  })
  return () => {
    if (captured === null) throw new Error('组件还没注册进度处理器')
    return captured
  }
}

function progressEvent(overrides: Partial<TaskProgress> = {}): TaskProgress {
  return {
    taskId: 'task-1',
    seq: 1,
    status: 'running',
    processed: 1,
    total: 3,
    ...overrides,
  }
}

describe('预览页的进度与取消', () => {
  it('执行中收到进度后显示进度与停止按钮', async () => {
    const handler = captureProgressHandler()
    stubIpc({
      get_plan: aPlan,
      validate_plan: noIssues,
      // 让执行挂起，界面停在「进行中」
      execute_plan: new Promise(() => {}),
    })
    render(<PreviewPage planId="plan-1" rootPath="C://资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeEnabled())
    await userEvent.click(confirm)
    await userEvent.click(
      await screen.findByRole('button', { name: t.preview.confirmInDialog }),
    )

    // 后端开始派发，推来一条进度
    act(() => handler()(progressEvent()))
    expect(await screen.findByText(/已整理\s*1\s*\/\s*3/)).toBeInTheDocument()

    expect(
      screen.getByRole('button', { name: t.preview.cancelExecution }),
    ).toBeInTheDocument()
  })

  it('点停止会调用 cancel_task，并明确说明「已请求」而不是「已停止」', async () => {
    const handler = captureProgressHandler()
    stubIpc({
      get_plan: aPlan,
      validate_plan: noIssues,
      execute_plan: new Promise(() => {}),
      cancel_task: null,
    })
    render(<PreviewPage planId="plan-1" rootPath="C://资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeEnabled())
    await userEvent.click(confirm)
    await userEvent.click(
      await screen.findByRole('button', { name: t.preview.confirmInDialog }),
    )

    act(() => handler()(progressEvent()))
    await userEvent.click(
      await screen.findByRole('button', { name: t.preview.cancelExecution }),
    )

    await waitFor(() => {
      expect(mockedCall).toHaveBeenCalledWith('cancel_task', { taskId: 'task-1' })
    })
    // 规格 T07：必须区分「请求取消」与「已取消」
    expect(screen.getByText(t.preview.cancelRequested)).toBeInTheDocument()
  })

  it('后端报告终态后不再显示停止入口', async () => {
    const handler = captureProgressHandler()
    stubIpc({
      get_plan: aPlan,
      validate_plan: noIssues,
      execute_plan: new Promise(() => {}),
    })
    render(<PreviewPage planId="plan-1" rootPath="C://资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeEnabled())
    await userEvent.click(confirm)
    await userEvent.click(
      await screen.findByRole('button', { name: t.preview.confirmInDialog }),
    )

    act(() => handler()(progressEvent()))
    expect(
      await screen.findByRole('button', { name: t.preview.cancelExecution }),
    ).toBeInTheDocument()

    act(() => handler()(progressEvent({ seq: 2, status: 'cancelled', processed: 3 })))

    await waitFor(() => {
      expect(
        screen.queryByRole('button', { name: t.preview.cancelExecution }),
      ).not.toBeInTheDocument()
    })
  })

  it('乱序的进度事件会被序号守卫丢掉', async () => {
    const handler = captureProgressHandler()
    stubIpc({
      get_plan: aPlan,
      validate_plan: noIssues,
      execute_plan: new Promise(() => {}),
    })
    render(<PreviewPage planId="plan-1" rootPath="C://资料" />)

    await userEvent.click(await screen.findByRole('button', { name: t.preview.validateButton }))
    const confirm = screen.getByRole('button', { name: t.preview.confirmButton })
    await waitFor(() => expect(confirm).toBeEnabled())
    await userEvent.click(confirm)
    await userEvent.click(
      await screen.findByRole('button', { name: t.preview.confirmInDialog }),
    )

    act(() => handler()(progressEvent({ seq: 5, processed: 2 })))
    expect(await screen.findByText(/已整理\s*2\s*\/\s*3/)).toBeInTheDocument()

    // seq 比已见的小：应当被丢弃，界面保持 2
    act(() => handler()(progressEvent({ seq: 3, processed: 1 })))
    expect(screen.getByText(/已整理\s*2\s*\/\s*3/)).toBeInTheDocument()
  })
})
