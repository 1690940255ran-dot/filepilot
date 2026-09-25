import { useCallback, useEffect, useRef, useState, type JSX } from 'react'

import { call, IpcError } from '../../api/client'
import type { RecoveryReport } from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'

interface RecoveryPageProps {
  /** 要核对的执行记录。为 null 时页面提示从历史进入。 */
  runId: string | null
  /** 核对完成（不再阻塞新任务）时通知外层刷新。 */
  onSettled?: () => void
}

/**
 * 崩溃恢复核对页。
 *
 * 规格 8.3 的决策表把「能不能自动判定」和「用户接不接受这个结果」分开：
 * 前一半由后端每次重新核对磁盘得出，这一页只负责把结论讲清楚，
 * 并给一个**带理由的**「保留现状」确认入口。
 *
 * 有意**不提供**「自动修复」：
 * 应用只在一种情形下会自己动手（目标位置的文件身份与内容都对得上 → 记为已应用），
 * 其余情形都是「两处都有」或「两处都没有」，替用户猜会成为数据丢失的来源。
 */
export function RecoveryPage({ runId, onSettled }: RecoveryPageProps): JSX.Element {
  const [error, setError] = useState<string | null>(null)
  const [reason, setReason] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  const [reloadToken, setReloadToken] = useState(0)

  // 每次重新核对都会拿到新的摘要；用它做 key 可以让「确认」按钮
  // 在事实变化后重新变为可用，而不是让用户对着一个失效的摘要反复点。
  const digestRef = useRef<string | null>(null)

  /*
    报告连同「它属于哪一次核对」一起存。key 变了（换 runId 或点了重新核对）
    就当作还没有数据——这样不必在 effect 里 setState 清空，
    也避免了「先渲染上一轮的未决项、再清掉」导致的一帧错位。
  */
  const [loadedFor, setLoadedFor] = useState<{ key: string; report: RecoveryReport } | null>(null)
  const report =
    runId === null || loadedFor?.key !== `${runId}#${reloadToken}` ? null : loadedFor.report

  useEffect(() => {
    if (!runId) return

    let cancelled = false

    call<RecoveryReport>('get_recovery', { runId })
      .then((next) => {
        if (cancelled) return
        digestRef.current = next.stateDigest
        setLoadedFor({ key: `${runId}#${reloadToken}`, report: next })
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

  const reload = useCallback(() => {
    setNotice(null)
    setReloadToken((token) => token + 1)
  }, [])

  const acknowledge = useCallback(() => {
    if (!runId || !report) return
    const trimmed = reason.trim()
    if (trimmed === '') {
      setNotice(t.recovery.reasonRequired)
      return
    }

    setSubmitting(true)
    call<RecoveryReport>('acknowledge_recovery', {
      runId,
      // 带上**屏幕上这一份**的摘要。后端会拿它跟当前事实比，
      // 不一致就拒绝——那说明我们看到的已经过期了。
      stateDigest: report.stateDigest,
      reason: trimmed,
    })
      .then((next) => {
        digestRef.current = next.stateDigest
        // 用同一把 key 覆盖：runId 和 reloadToken 都没变，
        // 只是事实刷新了，不该把用户正在看的理由输入框也重置掉。
        setLoadedFor({ key: `${runId}#${reloadToken}`, report: next })
        setReason('')
        setNotice(
          next.blocksNewRuns ? t.recovery.acknowledgedStillBlocked : t.recovery.acknowledged,
        )
        if (!next.blocksNewRuns) onSettled?.()
      })
      .catch((raw: unknown) => {
        // 摘要过期是最常见的一种失败，单独给一句能指导行动的话。
        if (raw instanceof IpcError && raw.code === 'STALE_PLAN') {
          setNotice(t.recovery.stale)
          setReloadToken((token) => token + 1)
        } else {
          setNotice(raw instanceof IpcError ? raw.message : t.common.unknownError)
        }
      })
      .finally(() => {
        setSubmitting(false)
      })
  }, [runId, report, reason, reloadToken, onSettled])

  if (!runId) {
    return (
      <section className="page" aria-labelledby="recovery-heading">
        <h2 className="page-heading" id="recovery-heading">
          {t.recovery.heading}
        </h2>
        <div className="empty-state" role="status">
          <p>{t.recovery.noRun}</p>
          <p className="empty-hint">{t.recovery.noRunHint}</p>
        </div>
      </section>
    )
  }

  const pending = report === null ? [] : report.items.filter((i) => i.resolution !== 'acknowledged')
  const settled = report === null ? [] : report.items.filter((i) => i.resolution === 'acknowledged')

  return (
    <section className="page" aria-labelledby="recovery-heading">
      <h2 className="page-heading" id="recovery-heading">
        {t.recovery.heading}
      </h2>
      <p className="page-description">{t.recovery.description}</p>

      <div className="action-row">
        <button type="button" className="secondary-action" onClick={reload}>
          {t.recovery.recheck}
        </button>
      </div>

      {error && (
        <div className="error-box" role="alert">
          <p>{error}</p>
        </div>
      )}

      {report === null && !error && <p className="loading">{t.common.loading}</p>}

      {report !== null && (
        <>
          {/*
            根目录没授权时**先说这件事**。磁盘核不了，下面列出的「未决项」
            就还停留在上次的判定上；不提醒的话用户会以为那就是现状。
          */}
          {!report.rootAuthorized && (
            <div className="error-box" role="alert">
              <p>{t.recovery.rootUnauthorized}</p>
            </div>
          )}

          {report.blocksNewRuns ? (
            <div className="error-box" role="alert">
              <p>{t.recovery.blocking}</p>
            </div>
          ) : (
            <p className="notice" role="status">
              {t.recovery.clear}
            </p>
          )}

          <h3 className="section-heading">{t.recovery.pendingHeading}</h3>
          {pending.length === 0 ? (
            <p className="notice">{t.recovery.pendingEmpty}</p>
          ) : (
            <ul className="recovery-list">
              {pending.map((item) => (
                <li key={item.operationId} className="recovery-item">
                  <div className="recovery-paths">
                    <div>
                      <span className="path-label">{t.recovery.sourceLabel}</span>
                      <span className="path-value">{item.source.join('\\')}</span>
                    </div>
                    <div>
                      <span className="path-label">{t.recovery.targetLabel}</span>
                      <span className="path-value">{item.target.join('\\')}</span>
                    </div>
                  </div>
                  {/* 判定理由直接来自后端：它说的是「磁盘上现在是什么样」，不是内部状态。 */}
                  <p className="recovery-message">{item.message}</p>
                </li>
              ))}
            </ul>
          )}

          {pending.length > 0 && (
            <div className="recovery-ack">
              <h3 className="section-heading">{t.recovery.acknowledgeHeading}</h3>
              <p className="notice">{t.recovery.acknowledgeHint}</p>

              <label className="field-label" htmlFor="recovery-reason">
                {t.recovery.reasonLabel}
              </label>
              <textarea
                id="recovery-reason"
                className="text-input"
                rows={3}
                value={reason}
                placeholder={t.recovery.reasonPlaceholder}
                onChange={(event) => setReason(event.target.value)}
              />

              <div className="action-row">
                <button
                  type="button"
                  className="primary-action"
                  onClick={acknowledge}
                  disabled={submitting || !report.rootAuthorized}
                >
                  {submitting ? t.recovery.acknowledging : t.recovery.acknowledgeButton}
                </button>
              </div>

              {!report.rootAuthorized && (
                <p className="disabled-reason">{t.recovery.acknowledgeDisabled}</p>
              )}
            </div>
          )}

          {settled.length > 0 && (
            <>
              <h3 className="section-heading">{t.recovery.settledHeading}</h3>
              <ul className="recovery-list">
                {settled.map((item) => (
                  <li key={item.operationId} className="recovery-item recovery-item-settled">
                    <div className="recovery-paths">
                      <div>
                        <span className="path-label">{t.recovery.sourceLabel}</span>
                        <span className="path-value">{item.source.join('\\')}</span>
                      </div>
                      <div>
                        <span className="path-label">{t.recovery.targetLabel}</span>
                        <span className="path-value">{item.target.join('\\')}</span>
                      </div>
                    </div>
                    {/*
                      已确认的项**照样列出来**，并说明「磁盘状态没有变」。
                      把它藏起来会让用户以为应用已经处理掉了那两份文件。
                    */}
                    <p className="recovery-message">{t.recovery.settledNote}</p>
                  </li>
                ))}
              </ul>
            </>
          )}

          {notice && (
            <p className="notice" role="status">
              {notice}
            </p>
          )}
        </>
      )}
    </section>
  )
}

export default RecoveryPage
