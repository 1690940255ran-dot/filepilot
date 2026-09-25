import { useCallback, useEffect, useReducer, useRef, useState, type JSX } from 'react'

import { call, IpcError } from '../../api/client'
import {
  SequenceGuard,
  subscribeTaskProgress,
  type TaskProgress,
} from '../../api/events'
import type {
  Plan,
  PlanItem,
  RunReport,
  ValidationReport,
} from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'
import { ConfirmDialog, disabledReasons } from './ConfirmDialog'
import { IssueList } from './IssueList'
import { PathDiff } from './PathDiff'
import { useWindowedRows } from './useWindowedRows'
import {
  effectivePlan,
  hasPendingEdits,
  INITIAL_PREVIEW_STATE,
  isConfirmationCurrent,
  joinPath,
  previewReducer,
  selectedCount,
} from './previewReducer'

interface PreviewPageProps {
  /** 要预览的计划。为 null 时显示空态。 */
  planId: string | null
  /** 真实根目录路径，用于最终确认时展示。 */
  rootPath: string | null
}

/** 令牌过期由后端判定；界面只是提前把按钮灰掉，每秒重算一次即可。 */
const TICK_MS = 1000

export function PreviewPage({ planId, rootPath }: PreviewPageProps): JSX.Element {
  const [state, dispatch] = useReducer(previewReducer, INITIAL_PREVIEW_STATE)
  const [nowMs, setNowMs] = useState(() => Date.now())
  const [editing, setEditing] = useState<string | null>(null)
  const [draftText, setDraftText] = useState('')
  const [confirming, setConfirming] = useState(false)
  const [busy, setBusy] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [report, setReport] = useState<RunReport | null>(null)
  /** 同一次确认复用同一个 requestId —— 双击也只会产生一个 run。 */
  const executeRequestId = useRef<string | null>(null)
  /** 同步的「正在执行」标记。state 更新是异步的，挡不住同一轮事件里的第二次点击。 */
  const executingRef = useRef(false)
  /** 执行进度。为 null 表示当前没有在跑的执行。 */
  const [progress, setProgress] = useState<TaskProgress | null>(null)
  const [cancelRequested, setCancelRequested] = useState(false)
  const requestId = useRef(0)

  // 加载计划
  useEffect(() => {
    if (!planId) return
    const current = ++requestId.current

    call<Plan | null>('get_plan', { planId })
      .then((plan) => {
        // 丢弃过期响应：用户可能已经切到另一个计划
        if (current !== requestId.current) return
        if (plan === null) {
          setLoadError(t.preview.notReady)
          return
        }
        setLoadError(null)
        dispatch({ type: 'planLoaded', plan })
      })
      .catch((raw: unknown) => {
        if (current !== requestId.current) return
        setLoadError(raw instanceof IpcError ? raw.message : t.common.unknownError)
      })
  }, [planId])

  // 订阅执行进度。
  //
  // 规格 INV-10：组件卸载必须取消订阅，否则重开页面会累积监听器，
  // 同一个任务会被重复处理。`SequenceGuard` 丢弃乱序与重放事件。
  useEffect(() => {
    let unlisten: (() => void) | null = null
    let disposed = false
    const guard = new SequenceGuard()

    subscribeTaskProgress((update) => {
      if (!guard.accept(update)) return
      setProgress(update)
      if (update.status !== 'running') {
        // 终态：清掉「正在停止」的提示
        setCancelRequested(false)
      }
    })
      .then((fn) => {
        if (disposed) {
          fn()
        } else {
          unlisten = fn
        }
      })
      .catch(() => {
        // 监听不可用（例如不在桌面环境里、或事件通道初始化失败）。
        // 实时进度拿不到，但 `execute_plan` 的返回值仍会给出最终结果，
        // 功能不受影响。**必须显式捕获**：未处理的 rejection 会在窗口里
        // 冒成一条错误，用户看到一个跟当前操作无关的报错。
      })

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  // 让「确认是否还在有效期」随真实时间推进，而不是只在渲染时算一次
  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), TICK_MS)
    return () => window.clearInterval(timer)
  }, [])

  const plainPlan = effectivePlan(state)
  // 窗口化只影响**渲染哪几行**。勾选与草稿按 itemId 存在 reducer 里，
  // 所以滚出视口的行被卸载后，它的状态仍然在——滚回来时照旧。
  // 规格 T14 那句「筛选和编辑不丢选中状态」说的正是这件事。
  const windowed = useWindowedRows(plainPlan?.items ?? [])
  const selected = selectedCount(plainPlan)
  const canConfirm = isConfirmationCurrent(state.confirmation, nowMs)

  const startEdit = useCallback((item: PlanItem) => {
    setEditing(item.id)
    setDraftText(joinPath(item.target))
  }, [])

  const applyEdit = useCallback(() => {
    if (editing === null) return
    dispatch({ type: 'draftTarget', itemId: editing, target: draftText })
    setEditing(null)
  }, [editing, draftText])

  const runValidate = useCallback(async () => {
    if (!planId) return
    setBusy(true)
    try {
      const report = await call<ValidationReport | null>('validate_plan', { planId })
      if (report === null) {
        setLoadError(t.preview.notReady)
        return
      }
      dispatch({
        type: 'reportLoaded',
        report,
        token: report.validationToken,
        // 后端给的是 UTC RFC3339；换算失败时按「已过期」处理，宁可多让用户点一次
        expiresAtMs: parseInstant(report.expiresAt),
      })
    } catch (raw) {
      dispatch({
        type: 'saveFailed',
        message: raw instanceof IpcError ? raw.message : t.common.unknownError,
      })
    } finally {
      setBusy(false)
    }
  }, [planId])

  const saveEdits = useCallback(async () => {
    if (!planId || !state.plan) return
    setBusy(true)
    dispatch({ type: 'saveStarted' })
    try {
      const updated = await call<Plan>('update_plan', {
        planId,
        expectedRevision: state.plan.revision,
        edits: Object.entries(state.drafts).map(([itemId, target]) => ({
          itemId,
          selected: state.selectionDrafts[itemId] ?? null,
          target: target.split(/[\\/]/).filter((part) => part.length > 0),
        })),
      })
      dispatch({ type: 'planLoaded', plan: updated })
    } catch (raw) {
      dispatch({
        type: 'saveFailed',
        message: raw instanceof IpcError ? raw.message : t.common.unknownError,
      })
    } finally {
      setBusy(false)
    }
  }, [planId, state.plan, state.drafts, state.selectionDrafts])

  /**
   * 请求停止执行。
   *
   * 规格 T07：这里发的是**请求**，不是「已停止」。正在处理的那一项会先走完
   * ——半途而废会留下一个无法判定的状态，反而更危险。界面必须把这两件事分开说。
   */
  const requestCancel = useCallback(async () => {
    const taskId = progress?.taskId
    if (!taskId) return
    setCancelRequested(true)
    try {
      await call('cancel_task', { taskId })
    } catch {
      // 取消失败不改执行结果；终态事件会覆盖这个提示
      setCancelRequested(false)
    }
  }, [progress?.taskId])

  const execute = useCallback(async () => {
    if (!planId || state.confirmation.token === null) return

    // **同步**守卫，不能用 `busy` 这个 state。
    //
    // 两次点击可能在同一个事件循环里到达，而 `setBusy(true)` 要到下一次渲染
    // 才生效——那之间 `busy` 仍是 false，第二次点击会真的再发一次请求。
    // requestId 幂等能保证后端不会产生两个 run，但无谓的请求不该发出去。
    if (executingRef.current) return
    executingRef.current = true

    setBusy(true)
    setCancelRequested(false)
    setProgress(null)
    dispatch({ type: 'saveStarted' })
    try {
      // 第一次点生成一次 requestId，之后复用它。
      // 后端用它做幂等键，因此重复点击不会产生第二个 run。
      executeRequestId.current ??= crypto.randomUUID()
      const result = await call<RunReport>('execute_plan', {
        planId,
        requestId: executeRequestId.current,
        validationToken: state.confirmation.token,
      })
      setReport(result)
      setConfirming(false)
      // 令牌已被后端消费，本地确认随之失效
      dispatch({ type: 'confirmationCleared' })
    } catch (raw) {
      dispatch({
        type: 'saveFailed',
        message: raw instanceof IpcError ? raw.message : t.common.unknownError,
      })
    } finally {
      executingRef.current = false
      setBusy(false)
    }
  }, [planId, state.confirmation.token])

  if (!planId) {
    return (
      <section className="page" aria-labelledby="preview-heading">
        <h2 className="page-heading" id="preview-heading">
          {t.preview.heading}
        </h2>
        <p className="notice">{t.preview.notReady}</p>
      </section>
    )
  }

  return (
    <section className="page" aria-labelledby="preview-heading">
      <h2 className="page-heading" id="preview-heading">
        {t.preview.heading}
      </h2>
      <p className="page-description">{t.preview.description}</p>

      {loadError && (
        <div className="error-box" role="alert">
          <p>{loadError}</p>
        </div>
      )}

      {state.error && (
        <div className="error-box" role="alert">
          <p>{state.error}</p>
        </div>
      )}

      {plainPlan && (
        <>
          <dl className="info-grid">
            <dt>{t.preview.summarySelected}</dt>
            <dd>
              {selected}
              {t.preview.summaryUnit}
            </dd>
            <dt>{t.preview.summaryExecutable}</dt>
            <dd>
              {state.report?.executableCount ?? '—'}
              {t.preview.summaryUnit}
            </dd>
          </dl>

          {state.report && <IssueList issues={state.report.issues} />}

          {/* 滚动容器。窗口化只渲染视口内的行，其余用上下两行占位撑开
              滚动条——所以占位高度必须等于「行数 × 行高」，而行高是常量。

              `eslint-disable` 的理由：`react-hooks/refs` 把「把回调传给
              `ref=`」也判定成「在渲染期访问 ref」，而这里**没有**读
              `ref.current`——那正是回调 ref 这个模式的全部要点。规则在这个
              形状上误报，所以就地关掉，而不是为了过 lint 去改成更难懂的写法。 */}
          {/* eslint-disable react-hooks/refs */}
          <div
            className="plan-scroll"
            ref={windowed.attachContainer}
            onScroll={windowed.onScroll}
          >
          <table className="plan-table">
            <thead>
              <tr>
                <th scope="col">{t.preview.columnSelect}</th>
                <th scope="col">{t.preview.columnSource}</th>
                <th scope="col">{t.preview.columnTarget}</th>
                <th scope="col">{t.preview.columnAction}</th>
                <th scope="col">{t.preview.columnOrigin}</th>
              </tr>
            </thead>
            <tbody>
              {/* 上方占位：撑出「已经滚过去的那几行」的高度。 */}
              {windowed.paddingTop > 0 && (
                <tr aria-hidden="true" style={{ height: windowed.paddingTop }}>
                  <td colSpan={5} />
                </tr>
              )}
              {windowed.visible.map((item) => (
                <tr key={item.id}>
                  <td>
                    <input
                      type="checkbox"
                      checked={item.selected}
                      aria-label={`${t.preview.columnSelect} ${joinPath(item.source)}`}
                      onChange={(event) =>
                        dispatch({
                          type: 'draftToggled',
                          itemId: item.id,
                          selected: event.target.checked,
                        })
                      }
                    />
                  </td>
                  <td>
                    <span className="path-text">{joinPath(item.source)}</span>
                  </td>
                  <td>
                    {editing === item.id ? (
                      <span className="inline-edit">
                        <input
                          type="text"
                          value={draftText}
                          aria-label={t.preview.editTarget}
                          onChange={(event) => setDraftText(event.target.value)}
                        />
                        <button type="button" onClick={applyEdit}>
                          {t.preview.applyEdit}
                        </button>
                        <button type="button" onClick={() => setEditing(null)}>
                          {t.preview.cancelEdit}
                        </button>
                      </span>
                    ) : (
                      <span className="target-cell">
                        <PathDiff source={item.source} target={item.target} />
                        <button
                          type="button"
                          className="link-button"
                          onClick={() => startEdit(item)}
                        >
                          {t.preview.editTarget}
                        </button>
                      </span>
                    )}
                  </td>
                  <td>
                    {item.action === 'move' ? t.preview.actionMove : t.preview.actionNoop}
                  </td>
                  <td>
                    {item.origin === 'rule'
                      ? t.preview.originRule
                      : item.origin === 'ai'
                        ? t.preview.originAi
                        : t.preview.originUser}
                  </td>
                </tr>
              ))}
              {/* 下方占位：撑出「还没滚到的那几行」的高度。 */}
              {windowed.paddingBottom > 0 && (
                <tr aria-hidden="true" style={{ height: windowed.paddingBottom }}>
                  <td colSpan={5} />
                </tr>
              )}
            </tbody>
          </table>
          </div>
          {/* eslint-enable react-hooks/refs */}

          <div className="action-row">
            {hasPendingEdits(state) && (
              <>
                <button type="button" className="secondary-action" onClick={() => void saveEdits()} disabled={busy}>
                  {t.preview.applyEdit}
                </button>
                <button
                  type="button"
                  className="secondary-action"
                  onClick={() => dispatch({ type: 'draftsDiscarded' })}
                >
                  {t.preview.discardEdits}
                </button>
              </>
            )}

            <button
              type="button"
              className="secondary-action"
              onClick={() => void runValidate()}
              disabled={busy}
            >
              {busy ? t.preview.validating : t.preview.validateButton}
            </button>

            <button
              type="button"
              className="primary-action"
              disabled={!canConfirm || busy}
              onClick={() => setConfirming(true)}
            >
              {busy ? t.preview.executing : t.preview.confirmButton}
            </button>

            {/* 停止入口只在**后端明确说还在跑**时出现。
                只用 `busy` 判断是不够的：收到 `cancelled` 之后执行器已经停了，
                但 `execute_plan` 要等整个循环走完才返回，那段时间里
                按钮会一直挂着，用户以为还能再停一次。 */}
            {busy && progress?.status === 'running' && !cancelRequested && (
              <button
                type="button"
                className="secondary-action"
                onClick={() => void requestCancel()}
              >
                {t.preview.cancelExecution}
              </button>
            )}
            {cancelRequested && (
              <span className="notice" role="status">
                {t.preview.cancelling}
              </span>
            )}
          </div>

          {busy && progress && (
            <p className="notice" role="status">
              {t.preview.progressLabel} {progress.processed}
              {progress.total === null ? '' : ` / ${progress.total}`}
            </p>
          )}

          {cancelRequested && <p className="disabled-reason">{t.preview.cancelRequested}</p>}

          {/*
            禁用理由必须**留在按钮旁边**，不能只放在对话框里：
            对话框默认是关的，用户看到的会是一个灰掉却不说原因的按钮，
            只能靠反复点击去猜。规格 T14 明确要求禁用理由对用户可见。
          */}
          {!canConfirm && (
            <ul className="disabled-reasons" role="status">
              {disabledReasons(state.confirmation, selected, nowMs).map((reason) => (
                <li key={reason}>{reason}</li>
              ))}
            </ul>
          )}

          <p className="disabled-reason">{t.preview.executeNotAvailable}</p>

          {report && (
            <>
              <h3 className="section-heading">{t.preview.runHeading}</h3>
              <dl className="info-grid">
                <dt>{t.preview.runApplied}</dt>
                <dd>{report.counts.applied}</dd>
                <dt>{t.preview.runFailedCount}</dt>
                <dd>{report.counts.failed}</dd>
                <dt>{t.preview.runSkipped}</dt>
                <dd>{report.counts.skipped}</dd>
                <dt>{t.preview.runPending}</dt>
                <dd>{report.counts.pending}</dd>
              </dl>
              <p className="notice" role="status">
                {runStatusLabel(report.status)}
              </p>
              <IssueList issues={report.issues} />
            </>
          )}

          <ConfirmDialog
            open={confirming}
            rootPath={rootPath}
            selectedCount={selected}
            confirmation={state.confirmation}
            nowMs={nowMs}
            onCancel={() => setConfirming(false)}
            onConfirm={() => void execute()}
          />
        </>
      )}
    </section>
  )
}

/** 把执行状态翻成用户能懂的一句话。 */
function runStatusLabel(status: RunReport['status']): string {
  switch (status) {
    case 'completed':
      return t.preview.runCompleted
    case 'partial':
      return t.preview.runPartial
    case 'recoveryRequired':
      return t.preview.runRecoveryRequired
    default:
      return t.preview.runFailed
  }
}

/** 解析后端的 UTC RFC3339。解析不了就当作已过期。 */
function parseInstant(value: string | null): number {
  if (!value) return 0
  const parsed = Date.parse(value)
  return Number.isNaN(parsed) ? 0 : parsed
}
