import type { Plan, ValidationReport } from '../../api/contracts.generated'

/**
 * 确认有效性视图。
 *
 * 规格 10.3：这个类型表示**后端报告给前端的**已知状态，
 * 它的布尔值**不赋予前端任何执行授权**。真正的安全规则在 Rust 端，
 * 令牌伪造、过期、重用都由后端拒绝；这里只决定按钮是否可点。
 */
export type ConfirmationView = {
  planRevision: number
  reportRevision: number
  executableCount: number
  hasBlockingIssues: boolean
  token: string | null
  expiresAtMs: number
}

/**
 * 规格 10.3 固定签名的实现。
 *
 * 五个条件必须同时成立：
 * - 计划版本与报告版本一致（编辑过就作废）
 * - 至少有一项可执行（全被阻断时不能确认）
 * - 没有阻断级问题
 * - 持有令牌
 * - 尚未过期（**`>` 而不是 `>=`**：恰好到期的那一刻已经无效）
 */
export function isConfirmationCurrent(view: ConfirmationView, nowMs: number): boolean {
  return (
    view.planRevision === view.reportRevision &&
    view.executableCount > 0 &&
    !view.hasBlockingIssues &&
    view.token !== null &&
    view.expiresAtMs > nowMs
  )
}

/** 还没有任何确认时的空视图。 */
export const EMPTY_CONFIRMATION: ConfirmationView = {
  planRevision: 0,
  reportRevision: 0,
  executableCount: 0,
  hasBlockingIssues: false,
  token: null,
  expiresAtMs: 0,
}

export type PreviewState = {
  plan: Plan | null
  report: ValidationReport | null
  /**
   * 本地编辑但**尚未提交**的目标，键是 itemId。
   *
   * 单独放一层而不是直接改 `plan`：未提交的编辑不该被误当成已保存的状态，
   * 「提交」这个动作本身也要能失败（版本冲突）。
   */
  drafts: Record<string, string>
  /** 本地待提交的勾选变化，键是 itemId；`undefined` 表示跟随 plan 原值。 */
  selectionDrafts: Record<string, boolean>
  /** 正在提交中，用于禁用按钮而不是乐观地当成功。 */
  saving: boolean
  /** 最近一次操作的错误提示。 */
  error: string | null
  confirmation: ConfirmationView
}

export const INITIAL_PREVIEW_STATE: PreviewState = {
  plan: null,
  report: null,
  drafts: {},
  selectionDrafts: {},
  saving: false,
  error: null,
  confirmation: EMPTY_CONFIRMATION,
}

export type PreviewAction =
  | { type: 'planLoaded'; plan: Plan }
  | { type: 'reportLoaded'; report: ValidationReport; token: string | null; expiresAtMs: number }
  | { type: 'draftToggled'; itemId: string; selected: boolean }
  | { type: 'draftTarget'; itemId: string; target: string }
  | { type: 'draftsDiscarded' }
  | { type: 'saveStarted' }
  | { type: 'saveFailed'; message: string }
  | { type: 'confirmationCleared' }

/**
 * 使任何既有确认失效。
 *
 * 规格 7.4：**任何编辑都必须让旧确认失效**。把这件事集中在一个函数里，
 * 是为了让「新增一种编辑动作时忘了清确认」这种疏漏更难发生。
 */
function invalidate(state: PreviewState): PreviewState {
  return {
    ...state,
    confirmation: {
      ...EMPTY_CONFIRMATION,
      planRevision: state.confirmation.planRevision + 1,
      // **保留**最近一次校验对应的计划版本，不清零。
      //
      // 这个字段同时承担着「有没有校验过」的判据：清零之后
      // 「从没校验过」与「校验过、之后又被改」就长得一模一样，
      // 而这两种情况该给用户看的话完全不同（去点校验 / 计划已失效）。
      // `state.report` 本来就跨编辑保留，这里抹掉版本反而与它不一致。
      reportRevision: state.confirmation.reportRevision,
    },
  }
}

/** 是否存在未提交的本地改动。 */
export function hasPendingEdits(state: PreviewState): boolean {
  return Object.keys(state.drafts).length > 0 || Object.keys(state.selectionDrafts).length > 0
}

/**
 * 把本地草稿叠加到计划上，得到「用户现在看到的」计划。
 *
 * 界面与提交都用这一份，避免显示的和提交的不是同一个东西。
 */
export function effectivePlan(state: PreviewState): Plan | null {
  if (!state.plan) return null

  const items = state.plan.items.map((item) => {
    const target = state.drafts[item.id]
    const selected = state.selectionDrafts[item.id]
    if (target === undefined && selected === undefined) return item
    return {
      ...item,
      target: target === undefined ? item.target : splitPath(target),
      selected: selected === undefined ? item.selected : selected,
    }
  })

  return { ...state.plan, items }
}

/** 把用户输入的相对路径拆成组件。接受 `/` 与 `\` 两种分隔符。 */
export function splitPath(value: string): string[] {
  return value
    .split(/[\\/]/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
}

/** 把组件拼成展示用的相对路径。 */
export function joinPath(parts: string[]): string {
  return parts.join('\\')
}

/** 仅统计**选中**的项 —— 规格：未选中项不参与摘要、不参与执行。 */
export function selectedCount(plan: Plan | null): number {
  if (!plan) return 0
  return plan.items.filter((item) => item.selected).length
}

/** 报告中是否存在阻断级问题。 */
export function hasBlockingIssues(report: ValidationReport | null): boolean {
  if (!report) return false
  return report.issues.some((issue) => issue.severity === 'block')
}

export function previewReducer(state: PreviewState, action: PreviewAction): PreviewState {
  switch (action.type) {
    case 'planLoaded':
      return {
        ...state,
        plan: action.plan,
        drafts: {},
        selectionDrafts: {},
        saving: false,
        error: null,
        // 换了计划就必须重新确认
        confirmation: { ...EMPTY_CONFIRMATION, planRevision: action.plan.revision },
      }

    case 'reportLoaded':
      return {
        ...state,
        report: action.report,
        saving: false,
        error: null,
        confirmation: {
          planRevision: state.plan?.revision ?? action.report.revision,
          reportRevision: action.report.revision,
          executableCount: action.report.executableCount,
          hasBlockingIssues: hasBlockingIssues(action.report),
          token: action.token,
          expiresAtMs: action.expiresAtMs,
        },
      }

    case 'draftToggled': {
      const next = invalidate(state)
      return {
        ...next,
        selectionDrafts: { ...state.selectionDrafts, [action.itemId]: action.selected },
      }
    }

    case 'draftTarget': {
      const next = invalidate(state)
      return { ...next, drafts: { ...state.drafts, [action.itemId]: action.target } }
    }

    case 'draftsDiscarded':
      return { ...state, drafts: {}, selectionDrafts: {}, error: null }

    case 'saveStarted':
      return { ...state, saving: true, error: null }

    case 'saveFailed':
      return { ...state, saving: false, error: action.message }

    case 'confirmationCleared':
      return { ...state, confirmation: { ...EMPTY_CONFIRMATION } }

    default:
      return state
  }
}
