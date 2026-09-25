import { useCallback, useEffect, useRef, useState, type JSX } from 'react'

import { call, IpcError } from '../../api/client'
import { SequenceGuard, subscribeTaskProgress, type TaskProgress } from '../../api/events'
import type { UndoPreview, UndoReport } from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'

interface UndoPageProps {
  /** 要撤销的那次整理记录。为 null 时提示从历史进入。 */
  runId: string | null
  /** 撤销结束（或用户离开）时通知外层。 */
  onDone?: () => void
  /** 外部认为当前有任务在跑时禁用入口。 */
  busy?: boolean
}

/**
 * 撤销预览与执行页。
 *
 * 规格 8.4 把撤销定义成**另一个可预览、可确认、可失败的操作**，
 * 所以这一页的形状与整理前的预览页是同构的：先给清单和摘要，
 * 用户看完再确认。刻意不提供「一键全部撤销」——有冲突的项默认不选中，
 * 跳过这一步就等于让用户在没看到冲突的情况下把文件搬回去。
 *
 * 结果区把「已撤销 / 有冲突 / 未处理」**分开计数**：把部分撤销说成全部成功，
 * 会让用户以为文件都回去了，而实际上还有几项留在原地。
 */
export function UndoPage({ runId, onDone, busy = false }: UndoPageProps): JSX.Element {
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const [reloadToken, setReloadToken] = useState(0)
  /** 用户对勾选的改动：只存「与预览默认值不同」的那些项。 */
  const [deselected, setDeselected] = useState<Record<string, boolean>>({})
  const [result, setResult] = useState<UndoReport | null>(null)
  const [progress, setProgress] = useState<TaskProgress | null>(null)
  const [cancelRequested, setCancelRequested] = useState(false)
  const submittingRef = useRef(false)
  const activeTaskId = useRef<string | null>(null)

  /*
    预览连同「它属于哪一次打开」一起存。key 变了就当作还没有数据——
    这样不必在 effect 里 setState 清空，也就不会出现
    「先渲染上一轮那份清单、下一帧再清掉」的错位。
  */
  const [loadedFor, setLoadedFor] = useState<{ key: string; preview: UndoPreview } | null>(null)
  const preview =
    runId === null || loadedFor?.key !== `${runId}#${reloadToken}` ? null : loadedFor.preview

  useEffect(() => {
    if (!runId) return

    let cancelled = false

    call<UndoPreview>('preview_undo', { runId })
      .then((next) => {
        if (cancelled) return
        setLoadedFor({ key: `${runId}#${reloadToken}`, preview: next })
        setError(null)
      })
      .catch((raw: unknown) => {
        if (cancelled) return
        setError(raw instanceof IpcError ? raw.message : t.common.unknownError)
      })

    return () => {
      cancelled = true
    }
  }, [runId, reloadToken])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | null = null
    const guard = new SequenceGuard()
    subscribeTaskProgress((update) => {
      if (!submittingRef.current || !guard.accept(update)) return
      if (activeTaskId.current === null && update.status === 'running') {
        activeTaskId.current = update.taskId
      }
      if (activeTaskId.current !== update.taskId) return
      setProgress(update)
      if (update.status !== 'running') setCancelRequested(false)
    })
      .then((fn) => {
        if (disposed) fn()
        else unlisten = fn
      })
      .catch(() => {})
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  /* 重新预览时把勾选改回后端给的默认值：那是一份**新的**磁盘事实。 */
  const reload = useCallback(() => {
    setNotice(null)
    setDeselected({})
    setReloadToken((token) => token + 1)
  }, [])

  const items = preview === null ? [] : preview.items

  /**
   * 某一项当前是否被勾选。
   *
   * `deselected` 只记「与预览默认值不同」的那些项，所以默认值仍然来自后端
   * （`Conflict` / `AlreadyUndone` 默认不选中）。用 `useCallback` 包一层而不是
   * 直接写成函数字面量：它会被 `execute` 闭包捕获，每次渲染换一份新引用
   * 会让依赖数组要么漏报、要么永远失效。
   */
  const isSelected = useCallback(
    (operationId: string, fallback: boolean): boolean =>
      deselected[operationId] === undefined ? fallback : !deselected[operationId],
    [deselected],
  )

  const toggle = useCallback((operationId: string, currently: boolean) => {
    setDeselected((current) => ({ ...current, [operationId]: currently }))
  }, [])

  const execute = useCallback(() => {
    if (!runId || !preview) return
    const token = preview.undoToken
    if (token === null) {
      // 没有令牌意味着后端这次没有签发（例如全部项都不可撤销）。
      // 不能拿一个空字符串去试着提交——那只会得到一个含混的失败。
      setNotice(t.undo.nothingReady)
      return
    }

    const selected = preview.items
      .filter((item) => isSelected(item.operationId, item.selected))
      .map((item) => item.operationId)

    submittingRef.current = true
    activeTaskId.current = null
    setProgress(null)
    setCancelRequested(false)
    setSubmitting(true)
    call<UndoReport>('execute_undo', {
      undoPlanId: preview.undoPlanId,
      originalRunId: runId,
      // 带上**屏幕上这一份**的摘要。后端拿它跟当前事实比，不一致就拒绝——
      // 那说明我们看到的已经过期了。
      digest: preview.digest,
      undoToken: token,
      selected,
      /*
        requestId 由撤销计划 id 派生，**不是**每次点击新生成一个。
        规格 INV-10 要求双击不得执行两次：每次点都换 id 的话，
        后端会把第二次当成一个全新请求，于是又多出一条撤销记录。
        绑在计划 id 上天然幂等——一份预览最多对应一次撤销；
        想再撤一批，用户本来就得重新预览，那时计划 id 也换了。
      */
      requestId: `undo:${preview.undoPlanId}`,
    })
      .then((report) => {
        setResult(report)
        setNotice(null)
      })
      .catch((raw: unknown) => {
        if (raw instanceof IpcError && raw.code === 'STALE_PLAN') {
          setNotice(t.undo.stale)
          setReloadToken((value) => value + 1)
        } else if (raw instanceof IpcError && raw.code === 'TASK_BUSY') {
          setNotice(t.undo.busy)
        } else {
          setNotice(raw instanceof IpcError ? raw.message : t.common.unknownError)
        }
      })
      .finally(() => {
        submittingRef.current = false
        setSubmitting(false)
      })
  }, [runId, preview, isSelected])

  const requestCancel = useCallback(async () => {
    const taskId = activeTaskId.current
    if (!taskId) return
    setCancelRequested(true)
    try {
      await call('cancel_task', { taskId })
    } catch {
      setCancelRequested(false)
    }
  }, [])

  if (!runId) {
    return (
      <section className="page" aria-labelledby="undo-heading">
        <h2 className="page-heading" id="undo-heading">
          {t.undo.heading}
        </h2>
        <div className="empty-state" role="status">
          <p>{t.undo.noRun}</p>
          <p className="empty-hint">{t.undo.noRunHint}</p>
        </div>
      </section>
    )
  }

  const ready = items.filter((item) => item.outcome === 'ready')
  const conflicts = items.filter((item) => item.outcome === 'conflict')
  const alreadyUndone = items.filter((item) => item.outcome === 'alreadyUndone')
  const selectedCount = ready.filter((item) => isSelected(item.operationId, item.selected)).length

  return (
    <section className="page" aria-labelledby="undo-heading">
      <h2 className="page-heading" id="undo-heading">
        {t.undo.heading}
      </h2>
      <p className="page-description">{t.undo.description}</p>

      <div className="action-row">
        <button type="button" className="secondary-action" onClick={reload}>
          {t.undo.recheck}
        </button>
      </div>

      {error && (
        <div className="error-box" role="alert">
          <p>{error}</p>
        </div>
      )}

      {preview === null && !error && <p className="loading">{t.common.loading}</p>}

      {preview !== null && (
        <>
          <p className="notice" role="status">
            {preview.readyCount === 0 && preview.alreadyUndoneCount > 0
              ? t.undo.allClear
              : t.undo.summary(
                  preview.readyCount,
                  preview.conflictCount,
                  preview.alreadyUndoneCount,
                )}
          </p>

          {ready.length > 0 && (
            <>
              <h3 className="section-heading">{t.undo.readyHeading}</h3>
              <ul className="undo-list">
                {ready.map((item) => (
                  <li key={item.operationId} className="undo-item">
                    <label className="undo-check">
                      <input
                        type="checkbox"
                        checked={isSelected(item.operationId, item.selected)}
                        onChange={() => toggle(item.operationId, isSelected(item.operationId, item.selected))}
                      />
                      <span className="undo-paths">
                        <span className="path-label">{t.undo.sourceLabel}</span>
                        <span className="path-value">{item.source.join('\\')}</span>
                        <span className="path-label">{t.undo.targetLabel}</span>
                        <span className="path-value">{item.target.join('\\')}</span>
                      </span>
                    </label>
                    <p className="undo-message">{item.message}</p>
                  </li>
                ))}
              </ul>
            </>
          )}

          {conflicts.length > 0 && (
            <>
              <h3 className="section-heading">{t.undo.conflictHeading}</h3>
              <p className="notice">{t.undo.conflictNote}</p>
              <ul className="undo-list">
                {conflicts.map((item) => (
                  <li key={item.operationId} className="undo-item undo-item-conflict">
                    <span className="undo-paths">
                      <span className="path-label">{t.undo.sourceLabel}</span>
                      <span className="path-value">{item.source.join('\\')}</span>
                      <span className="path-label">{t.undo.targetLabel}</span>
                      <span className="path-value">{item.target.join('\\')}</span>
                    </span>
                    {/* 冲突原因来自后端对磁盘的判定，说的都是用户能据以行动的事。 */}
                    <p className="undo-message">{item.message}</p>
                  </li>
                ))}
              </ul>
            </>
          )}

          {alreadyUndone.length > 0 && (
            <>
              <h3 className="section-heading">{t.undo.undoneHeading}</h3>
              <p className="notice">{t.undo.undoneNote}</p>
              <ul className="undo-list">
                {alreadyUndone.map((item) => (
                  <li key={item.operationId} className="undo-item undo-item-undone">
                    <span className="undo-paths">
                      <span className="path-label">{t.undo.sourceLabel}</span>
                      <span className="path-value">{item.source.join('\\')}</span>
                      <span className="path-label">{t.undo.targetLabel}</span>
                      <span className="path-value">{item.target.join('\\')}</span>
                    </span>
                    <p className="undo-message">{item.message}</p>
                  </li>
                ))}
              </ul>
            </>
          )}

          {ready.length > 0 && (
            <div className="undo-confirm">
              <h3 className="section-heading">{t.undo.confirmHeading}</h3>
              <p className="notice">{t.undo.confirmHint}</p>
              <p className="hint">{t.undo.tokenExpiry}</p>

              <div className="action-row">
                <button
                  type="button"
                  className="primary-action"
                  onClick={execute}
                  disabled={submitting || busy || selectedCount === 0}
                >
                  {submitting ? t.undo.executing : t.undo.executeButton}
                </button>
                {progress?.status === 'running' && (
                  <button
                    type="button"
                    className="secondary-action"
                    onClick={requestCancel}
                    disabled={cancelRequested}
                  >
                    {cancelRequested ? t.undo.cancelling : t.undo.cancelExecution}
                  </button>
                )}
              </div>
              {progress?.status === 'running' && (
                <p className="notice" role="status">
                  {cancelRequested
                    ? t.undo.cancelRequested
                    : t.undo.progress(progress.processed, progress.total ?? selectedCount)}
                </p>
              )}
            </div>
          )}

          {notice && (
            <p className="notice" role="status">
              {notice}
            </p>
          )}

          {result !== null && (
            <>
              <h3 className="section-heading">{t.undo.resultHeading}</h3>
              <ul className="undo-counts">
                <li>
                  {t.undo.resultReverted} {result.reverted}
                </li>
                <li>
                  {t.undo.resultConflicted} {result.conflicted}
                </li>
                <li>
                  {t.undo.resultUntouched} {result.untouched}
                </li>
                <li>
                  {t.undo.resultAlreadyUndone} {result.alreadyUndone}
                </li>
              </ul>

              {/*
                只有真的一项冲突都没有时才能说「全部搬回」。
                任何一项没回去（冲突或未处理）都要给出 partialWarning，
                否则界面会把一次部分撤销读成完全成功。
              */}
              <p className="notice" role="status">
                {result.status === 'recoveryRequired'
                  ? t.undo.recoveryRequired
                  : result.status === 'completed' &&
                      result.conflicted === 0 &&
                      result.untouched === 0
                    ? t.undo.completed
                    : t.undo.partialWarning}
              </p>

              <ul className="undo-list">
                {result.items.map((item) => (
                  <li key={`${item.operationId}:${item.itemId}`} className="undo-item">
                    <span className="undo-paths">
                      <span className="path-label">{t.undo.sourceLabel}</span>
                      <span className="path-value">{item.source.join('\\')}</span>
                      <span className="path-label">{t.undo.targetLabel}</span>
                      <span className="path-value">{item.target.join('\\')}</span>
                    </span>
                    <p className="undo-message">{item.message}</p>
                  </li>
                ))}
              </ul>

              {result.warnings.length > 0 && (
                <>
                  <h3 className="section-heading">{t.undo.warningsHeading}</h3>
                  <p className="notice">{t.undo.warningsNote}</p>
                  <ul className="undo-list">
                    {result.warnings.map((warning) => (
                      <li key={warning.message} className="undo-item undo-item-warning">
                        <p className="undo-message">{warning.message}</p>
                      </li>
                    ))}
                  </ul>
                </>
              )}

              {onDone && (
                <div className="action-row">
                  <button type="button" className="secondary-action" onClick={onDone}>
                    {t.undo.backToHistory}
                  </button>
                </div>
              )}
            </>
          )}
        </>
      )}
    </section>
  )
}

export default UndoPage
