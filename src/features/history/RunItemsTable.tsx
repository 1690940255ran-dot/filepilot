import { useEffect, useState, type JSX } from 'react'

import { call, IpcError } from '../../api/client'
import type { RunItem, RunItems } from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'

interface RunItemsTableProps {
  /** 要展开的那条记录。 */
  runId: string
}

/** 与 `OpStatus` 一一对应。缺项时回落到原始值，不静默显示空白。 */
const STATUS_LABEL: Record<string, string> = {
  pending: '未执行',
  prepared: '已准备',
  applied: '已处理',
  skipped: '已跳过',
  failed: '失败',
  ambiguous: '需要核对',
}

/** 这几类需要人工关注，用醒目颜色标出。 */
const ATTENTION: ReadonlySet<string> = new Set(['failed', 'ambiguous'])

/**
 * 一次整理的逐文件明细（PR-003）。
 *
 * ## 为什么单独拉而不是随列表返回
 *
 * 历史可达 50 条、每条 8+ 项。把这些路径全部塞进 `list_runs`，会让「打开历史页」
 * 这件事的成本与「用户一共整理过多少文件」成正比——而用户此时只想看列表。
 * 所以明细**按需**、**按 runId** 拉：展开哪条拉哪条。
 *
 * ## 为什么加载态与错误态必须显式渲染
 *
 * 这个组件一挂载就发 IPC。如果失败时只 `console.error`，用户看到的会是一个
 * 永远空着的区域——「没有明细」和「明细没读出来」在视觉上完全一样，
 * 而这两件事要用户做的事截然不同（一个是「本来就没事」，一个是「重试」）。
 *
 * ## 关于「换一条记录」这件事
 *
 * 外层只负责标记**当前要展开哪条**，真正的加载放在按 `runId` 键入的子组件里。
 * 这样切换 runId 时 React 直接换掉整棵子树：新的一次加载从干净的初始状态开始，
 * 不需要在 effect 里先 `setState` 清空上一次的结果
 * （那是一次多余的级联渲染，也是 react-hooks/set-state-in-effect 拦下来的问题）。
 * 直接摆结果、再在下一次渲染里清空，会让用户瞥见**属于另一次整理**的路径。
 */
export function RunItemsTable({ runId }: RunItemsTableProps): JSX.Element {
  // 重试就是把它加一，连同 runId 一起构成子组件的挂载键。
  const [attempt, setAttempt] = useState(0)

  return (
    <RunItemsLoader
      key={`${runId}#${attempt}`}
      runId={runId}
      onRetry={() => setAttempt((value) => value + 1)}
    />
  )
}

interface RunItemsLoaderProps {
  runId: string
  onRetry: () => void
}

/** 状态机：结果要么是数据、要么是错误，不存在「正在加载」这个存储态。 */
type Outcome =
  | { kind: 'ok'; data: RunItems }
  | { kind: 'error'; message: string }

/**
 * 按 `runId` 键入的一次加载。**不要**在这里处理「runId 变了」——那是父组件的
 * `key` 的职责；这个组件一旦挂载就只管一个 runId。
 */
function RunItemsLoader({ runId, onRetry }: RunItemsLoaderProps): JSX.Element {
  const [outcome, setOutcome] = useState<Outcome | null>(null)

  useEffect(() => {
    let cancelled = false

    call<RunItems | null>('get_run_items', { runId })
      .then((data) => {
        if (cancelled) return
        if (data === null) {
          // run 不存在（例如刚被清理）。这说明不了「没有明细」，
          // 所以走错误态而不是空态——空态会被读成「这次没动文件」。
          setOutcome({ kind: 'error', message: t.history.itemDetailLoadFailed })
          return
        }
        setOutcome({ kind: 'ok', data })
      })
      .catch((raw: unknown) => {
        if (cancelled) return
        setOutcome({
          kind: 'error',
          message:
            raw instanceof IpcError ? raw.message : t.history.itemDetailLoadFailed,
        })
      })

    return () => {
      cancelled = true
    }
  }, [runId])

  if (outcome === null) {
    return <p className="loading">{t.history.itemDetailLoading}</p>
  }

  if (outcome.kind === 'error') {
    return (
      <div role="alert">
        <p className="error-box">{outcome.message}</p>
        <button type="button" className="secondary-action" onClick={onRetry}>
          {t.history.itemRetry}
        </button>
      </div>
    )
  }

  const { data } = outcome

  if (data.items.length === 0) {
    return <p className="notice">{t.history.itemDetailEmpty}</p>
  }

  return (
    <>
      <h4 className="section-heading">{t.history.itemDetailHeading}</h4>
      <p className="notice">{summarize(data.items)}</p>
      <ul className="history-items">
        {data.items.map((item) => (
          <li
            key={item.itemId}
            className={
              ATTENTION.has(item.status)
                ? 'history-item history-item-attention'
                : 'history-item'
            }
          >
            <div className="history-item-paths">
              <span className="path-label">{t.history.itemColumnSource}</span>
              <span className="path-value">{item.source.join('\\')}</span>
            </div>
            <div className="history-item-paths">
              <span className="path-label">{t.history.itemColumnTarget}</span>
              <span className="path-value">{item.target.join('\\')}</span>
            </div>
            <span className="history-item-status">
              {STATUS_LABEL[item.status] ?? item.status}
              {/* 错误码单独显示：它是用户去查日志、提问题的唯一线索。 */}
              {item.errorCode !== null && (
                <span className="history-item-code">{item.errorCode}</span>
              )}
            </span>
          </li>
        ))}
      </ul>
    </>
  )
}

/** 一句话说清「总共几项、几项没成」，比让用户自己数列表强。 */
function summarize(items: RunItem[]): string {
  const failed = items.filter((item) => ATTENTION.has(item.status)).length
  return t.history.itemStatusSummary(items.length, failed)
}

export default RunItemsTable
