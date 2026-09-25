import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { TaskStatus } from './contracts.generated'

/**
 * 规格 5.2：事件通道**仅**使用 `task-progress`。
 * 载荷不含正文；事件丢失可用 get_task 恢复；seq 对每个任务单调递增。
 */
export const TASK_PROGRESS_CHANNEL = 'task-progress'

export interface TaskProgress {
  taskId: string
  /** 对每个任务单调递增。调用方应丢弃 seq 不大于已见值的乱序事件。 */
  seq: number
  status: TaskStatus
  processed: number
  /** null 表示总量尚未确定。 */
  total: number | null
}

/** 结构化校验事件载荷：外部输入不可信，不能直接当成 TaskProgress 使用。 */
export function isTaskProgress(value: unknown): value is TaskProgress {
  if (typeof value !== 'object' || value === null) return false
  const c = value as Record<string, unknown>
  return (
    typeof c.taskId === 'string' &&
    typeof c.seq === 'number' &&
    Number.isFinite(c.seq) &&
    typeof c.status === 'string' &&
    typeof c.processed === 'number' &&
    (c.total === null || typeof c.total === 'number')
  )
}

/**
 * 订阅任务进度。
 *
 * 返回取消订阅函数；调用方必须在组件卸载时调用它，否则重开页面会重复累积监听器，
 * 规格 INV-10 明确要求重试、重复点击、双窗口不得让同一操作执行两次。
 */
export async function subscribeTaskProgress(
  handler: (progress: TaskProgress) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASK_PROGRESS_CHANNEL, (event) => {
    if (!isTaskProgress(event.payload)) return
    handler(event.payload)
  })
}

/**
 * 单调序号守卫：丢弃乱序或重放的事件。
 * 页面重开后 seq 从头开始，因此调用方需要按 taskId 重置守卫。
 */
export class SequenceGuard {
  private readonly seen = new Map<string, number>()

  /** 返回 true 表示该事件是新的、应当被处理。 */
  accept(progress: TaskProgress): boolean {
    const previous = this.seen.get(progress.taskId)
    if (previous !== undefined && progress.seq <= previous) {
      return false
    }
    this.seen.set(progress.taskId, progress.seq)
    return true
  }

  forget(taskId: string): void {
    this.seen.delete(taskId)
  }
}
