import { useCallback, useState } from 'react'

import { call, IpcError } from '../../api/client'
import type {
  FilePage,
  RootSummary,
  ScanSummary,
  TaskSummary,
} from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'

/**
 * 首页的工作流状态。
 *
 * 用显式阶段而不是若干 boolean：`chosen` + `scanning` 这样的组合可以表达出
 * 「已选文件夹且正在扫描但也出错了」这种非法状态，联合类型不行。
 */
export type OrganizePhase =
  | { status: 'idle' }
  | { status: 'choosing' }
  | { status: 'ready'; root: RootSummary }
  | { status: 'scanning'; root: RootSummary; taskId: string | null }
  | {
      status: 'scanned'
      root: RootSummary
      summary: ScanSummary
      /**
       * 可整理文件的 id，按扫描结果的顺序。
       *
       * AI 模式要用它构造待发送载荷（规格 6.4）。放在这里而不是让页面再拉一次
       * `list_files`：`summarizeScan` 已经把每一页都翻过了，**顺便记下 id 是
       * 零成本**，而重拉一次不但多一轮 IPC，还可能因为期间的重扫而拿到
       * **不一致的一份**。
       */
      usableFileIds: string[]
    }

export interface OrganizeError {
  message: string
  retryable: boolean
}

export interface OrganizeFlow {
  phase: OrganizePhase
  error: OrganizeError | null
  /** 用户取消选择文件夹时的轻提示，不是错误。 */
  notice: string | null
  chooseRoot: () => Promise<void>
  startScan: () => Promise<void>
  cancelScan: () => Promise<void>
}

function toOrganizeError(raw: unknown): OrganizeError {
  if (raw instanceof IpcError) {
    return { message: raw.message, retryable: raw.retryable }
  }
  return {
    message: raw instanceof Error ? raw.message : t.common.unknownError,
    retryable: false,
  }
}

/**
 * 首页的「选文件夹 → 扫描」流程。
 *
 * 规格 3.3：前端只提交 rootId；路径由后端返回、仅用于展示。
 * 规格 INV-10：正在进行的操作不接受重复触发（按钮会被禁用，函数本身也做了守卫）。
 */
export function useOrganizeFlow(): OrganizeFlow {
  const [phase, setPhase] = useState<OrganizePhase>({ status: 'idle' })
  const [error, setError] = useState<OrganizeError | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

  const chooseRoot = useCallback(async () => {
    // INV-10 守卫：已在流程中就直接返回，不叠加第二次调用
    if (phase.status === 'choosing' || phase.status === 'scanning') return

    setError(null)
    setNotice(null)
    setPhase({ status: 'choosing' })

    try {
      const picked = await call<RootSummary | null>('choose_root')
      if (picked === null) {
        // 用户取消是正常流程，回到 idle，不当作错误
        setNotice(t.home.rootSelectionCancelled)
        setPhase({ status: 'idle' })
        return
      }
      setPhase({ status: 'ready', root: picked })
    } catch (raw) {
      setError(toOrganizeError(raw))
      setPhase({ status: 'idle' })
    }
  }, [phase.status])

  const startScan = useCallback(async () => {
    if (phase.status !== 'ready' && phase.status !== 'scanned') return

    const root = phase.root
    setError(null)
    setNotice(null)
    setPhase({ status: 'scanning', root, taskId: null })

    try {
      const taskId = await call<string>('start_scan', {
        rootId: root.rootId,
        recursive: true,
      })
      setPhase({ status: 'scanning', root, taskId })

      const task = await waitForTask(taskId)
      if (task.status === 'cancelled') {
        setNotice(t.home.scanCancelled)
        setPhase({ status: 'ready', root })
        return
      }
      if (task.status === 'failed' || task.status === 'recoveryRequired') {
        throw new IpcError({
          code: task.error?.code ?? 'IPC_TRANSPORT_FAILED',
          message: task.error?.message ?? t.errors.scanFailed,
          retryable: task.error?.retryable ?? false,
          details: {},
        })
      }
      if (!task.scanId) throw new Error(t.errors.scanIdMissing)

      const { summary, usableFileIds } = await summarizeScan(
        taskId,
        task.scanId,
        root.rootId,
        task.status === 'partial',
      )
      setPhase({ status: 'scanned', root, summary, usableFileIds })
    } catch (raw) {
      setError(toOrganizeError(raw))
      // 失败后退回 ready：用户可以重试，rootId 仍然有效
      setPhase({ status: 'ready', root })
    }
  }, [phase])

  const cancelScan = useCallback(async () => {
    if (phase.status !== 'scanning' || phase.taskId === null) return
    try {
      await call<TaskSummary | null>('cancel_task', { taskId: phase.taskId })
    } catch (raw) {
      setError(toOrganizeError(raw))
    }
  }, [phase])

  return { phase, error, notice, chooseRoot, startScan, cancelScan }
}

async function waitForTask(taskId: string): Promise<TaskSummary> {
  for (;;) {
    const task = await call<TaskSummary | null>('get_task', { taskId })
    if (task === null) throw new Error(t.errors.scanTaskMissing)
    if (!['queued', 'running'].includes(task.status)) return task
    await new Promise((resolve) => window.setTimeout(resolve, 50))
  }
}

async function summarizeScan(
  taskId: string,
  scanId: string,
  rootId: string,
  truncated: boolean,
): Promise<{ summary: ScanSummary; usableFileIds: string[] }> {
  let cursor: string | null = null
  let total: number | undefined
  let usable = 0
  let skipped = 0
  const usableFileIds: string[] = []
  do {
    const page: FilePage | null = await call<FilePage | null>('list_files', {
      scanId,
      cursor,
      limit: 200,
    })
    if (page === null) throw new Error(t.errors.scanResultMissing)
    total = page.total
    for (const item of page.items) {
      if (item.skipCode === null) {
        usable += 1
        // 顺手记下 id：AI 模式要用它构造载荷，而这一页已经在这里了。
        usableFileIds.push(item.id)
      } else {
        skipped += 1
      }
    }
    cursor = page.nextCursor
  } while (cursor !== null)

  return {
    summary: { taskId, scanId, rootId, total: total ?? 0, usable, skipped, truncated },
    usableFileIds,
  }
}
