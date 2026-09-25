import { useCallback, useEffect, useState, type JSX } from 'react'

import { call, IpcError } from '../../api/client'
import type { RunReport } from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'
import { IssueList } from '../preview/IssueList'

const PAGE_SIZE = 50

const STATUS_LABEL: Record<string, string> = {
  completed: t.history.statusCompleted,
  partial: t.history.statusPartial,
  failed: t.history.statusFailed,
  cancelled: t.history.statusCancelled,
  recoveryRequired: t.history.statusRecoveryRequired,
  running: t.history.statusRunning,
  queued: t.history.statusQueued,
}

interface HistoryPageProps {
  /** 用户选择「重新预览」时回到首页。 */
  onRestart?: () => void
  /**
   * 用户选择「去核对」时打开恢复页。
   *
   * 只有 `recoveryRequired` 的记录才会用到它——其余记录没有需要人工判断的东西，
   * 给它们也放一个入口只会稀释这个按钮的含义。
   */
  onInspectRecovery?: (runId: string) => void
  /**
   * 用户选择「撤销」时打开撤销页。
   *
   * 只对 `direction === 'apply'` 的记录给这个入口：撤销记录本身不能再被撤销
   * （那只会把两个位置再翻一次），给它一个点了注定被拒的按钮是误导。
   */
  onUndo?: (runId: string) => void
  /** 当前是否有任务在跑；有的话不给出口，避免误触。 */
  busy?: boolean
}

/**
 * 整理历史。
 *
 * 规格 T07：历史必须**从数据库读**，而不是内存里的任务表——
 * 应用重启后内存里的任务没了，但历史还在，这正是这个页面的意义。
 *
 * 这里**不提供「再执行一次」**：重复执行会把已经移动过的文件再挪一遍。
 * 规格明确要求「不显示可能重复执行的盲重试按钮」。
 */
export function HistoryPage({
  onRestart,
  onInspectRecovery,
  onUndo,
  busy = false,
}: HistoryPageProps): JSX.Element {
  const [runs, setRuns] = useState<RunReport[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [expanded, setExpanded] = useState<string | null>(null)
  const [reloadToken, setReloadToken] = useState(0)

  useEffect(() => {
    let cancelled = false

    call<RunReport[]>('list_runs', { limit: PAGE_SIZE })
      .then((list) => {
        if (cancelled) return
        setRuns(list)
        setError(null)
      })
      .catch((raw: unknown) => {
        if (cancelled) return
        setError(raw instanceof IpcError ? raw.message : t.common.unknownError)
        // 出错时也要给出一个非 loading 的状态，否则界面会一直停在「加载中」
        setRuns([])
      })

    return () => {
      cancelled = true
    }
  }, [reloadToken])

  const reload = useCallback(() => {
    setRuns(null)
    setError(null)
    setReloadToken((token) => token + 1)
  }, [])

  return (
    <section className="page" aria-labelledby="history-heading">
      <h2 className="page-heading" id="history-heading">
        {t.history.heading}
      </h2>

      <div className="action-row">
        <button type="button" className="secondary-action" onClick={reload}>
          {t.history.reload}
        </button>
      </div>

      {error && (
        <div className="error-box" role="alert">
          <p>{error}</p>
        </div>
      )}

      {runs === null && !error && <p className="loading">{t.common.loading}</p>}

      {runs !== null && runs.length === 0 && (
        <div className="empty-state" role="status">
          <p>{t.history.empty}</p>
          <p className="empty-hint">{t.history.emptyHint}</p>
        </div>
      )}

      {runs !== null && runs.length > 0 && (
        <ul className="run-list">
          {runs.map((run) => (
            <li key={run.runId} className="run-item">
              <div className="run-summary">
                <span className={`run-status status-${run.status}`}>
                  {STATUS_LABEL[run.status] ?? run.status}
                </span>
                {/*
                  撤销记录与整理记录在同一条时间线上，但「已完成 3 项」
                  在两者里的意思是相反的（搬走 vs 搬回）。标出来，
                  免得用户把一次撤销读成一次整理。
                */}
                <span className="run-direction">
                  {run.direction === 'undo' ? t.undo.undoRecordLabel : t.undo.applyRecordLabel}
                </span>
                <span className="run-counts">
                  {t.history.appliedColumn} {run.counts.applied}
                  {' · '}
                  {t.history.failedColumn} {run.counts.failed}
                  {' · '}
                  {t.history.skippedColumn} {run.counts.skipped}
                </span>
                <button
                  type="button"
                  className="link-button"
                  onClick={() =>
                    setExpanded((current) => (current === run.runId ? null : run.runId))
                  }
                >
                  {expanded === run.runId ? t.history.collapse : t.history.detail}
                </button>
              </div>

              {expanded === run.runId && (
                <div className="run-detail">
                  <IssueList issues={run.issues} />

                  {/*
                    recoveryRequired 时只说明情况，不给「重试」：
                    规格要求存在未决恢复项时后端禁止新任务，
                    界面上也不该出现一个点了必然失败的按钮。
                    给的是「去核对」——那是唯一能真正解决这件事的动作。
                  */}
                  {run.status === 'recoveryRequired' ? (
                    <>
                      <div className="error-box" role="alert">
                        <p>{t.history.recoveryHint}</p>
                      </div>
                      <button
                        type="button"
                        className="primary-action"
                        onClick={() => onInspectRecovery?.(run.runId)}
                        disabled={busy || !onInspectRecovery}
                      >
                        {t.history.recoveryOpen}
                      </button>
                    </>
                  ) : (
                    <>
                      <p className="notice">{t.history.reproviewHint}</p>
                      <div className="action-row">
                        <button
                          type="button"
                          className="secondary-action"
                          onClick={onRestart}
                          disabled={busy}
                        >
                          {t.history.reproviewButton}
                        </button>
                        {/*
                          撤销入口只给整理记录，且只在它确实改动过文件时出现：
                          一项都没搬动的记录没有可撤销的内容。
                          已撤销过多少项由撤销页自己去核对，这里不预判——
                          预判意味着这个按钮会基于一份可能过期的记忆显示或消失。
                        */}
                        {run.direction === 'apply' && run.counts.applied > 0 && (
                          <button
                            type="button"
                            className="secondary-action"
                            onClick={() => onUndo?.(run.runId)}
                            disabled={busy || !onUndo}
                          >
                            {t.undo.openFromHistory}
                          </button>
                        )}
                      </div>
                    </>
                  )}
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
