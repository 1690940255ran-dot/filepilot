import type { Page } from '@playwright/test'

import { commandTable } from './fixtures'

/**
 * e2e 共用的 Tauri IPC 替身。
 *
 * 从 `cancel.spec.ts` 提取而来：截图脚本（tests/screenshots/）需要
 * 同一个 mock 让真实界面渲染出「有数据」的状态。
 *
 * `isTauriAvailable()` 只检查 `window.__TAURI_INTERNALS__` 是否存在，
 * 所以注入 `invoke` 与 `transformCallback` 就足以让整条 IPC 链路跑起来。
 *
 * **重要边界**（规格 T14 硬性要求）：这只验证**界面在严格契约 mock 下的
 * 行为**，不验证真实文件操作。
 */

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
    __FILPILOT_CALLS__?: Array<{ command: string; args: unknown }>
    __FILPILOT_EMIT__?: (event: string, payload: unknown) => void
  }
}

/**
 * 安装 IPC 替身。不传 table 时用 `fixtures.ts` 的默认数据；
 * 调用方可以传入自己的表（同样会被前端运行时校验按契约检查）。
 */
export async function installIpcMock(
  page: Page,
  table?: Record<string, unknown>,
): Promise<void> {
  const responses = table ?? commandTable()

  await page.addInitScript(
    (serializable) => {
      const callbacks = new Map<number, (event: unknown) => void>()
      const listeners = new Map<string, Map<number, number>>()
      const calls: Array<{ command: string; args: unknown }> = []
      let nextId = 1

      const table: Record<string, unknown> = {
        ...serializable,
        // 在页面内构造：Promise 无法作为参数传进来
        execute_plan: new Promise(() => {}),
      }

      window.__FILPILOT_CALLS__ = calls

      window.__TAURI_INTERNALS__ = {
        transformCallback(callback: (event: unknown) => void) {
          const id = nextId++
          callbacks.set(id, callback)
          return id
        },
        async invoke(command: string, args: { event?: string; handler?: number }) {
          if (command === 'plugin:event|listen') {
            if (args.event !== undefined && args.handler !== undefined) {
              const eventId = nextId++
              const eventListeners = listeners.get(args.event) ?? new Map<number, number>()
              eventListeners.set(eventId, args.handler)
              listeners.set(args.event, eventListeners)
              return eventId
            }
            throw new Error('e2e mock 的事件监听参数不完整')
          }
          if (command === 'plugin:event|unlisten') return null

          calls.push({ command, args })
          if (!(command in table)) {
            throw new Error(`e2e mock 未预设命令 ${command}`)
          }
          // IPC 信封是 `{ ok: true, data }`，不是裸数据。
          // 直接返回裸数据会被前端的运行时校验拒掉——那条校验是对的，
          // 它挡下的正是「后端契约变了但前端不知道」这类问题。
          return { ok: true, data: await table[command] }
        },
      }

      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener(event: string, eventId: number) {
          const eventListeners = listeners.get(event)
          if (!eventListeners) return
          const handlerId = eventListeners.get(eventId)
          eventListeners.delete(eventId)
          if (handlerId !== undefined) callbacks.delete(handlerId)
          if (eventListeners.size === 0) listeners.delete(event)
        },
      }

      // 供测试从外面推进度事件
      window.__FILPILOT_EMIT__ = (event: string, payload: unknown) => {
        const eventListeners = listeners.get(event)
        if (!eventListeners) return
        for (const [eventId, handlerId] of eventListeners) {
          callbacks.get(handlerId)?.({ event, id: eventId, payload })
        }
      }
    },
    responses,
  )
}

/** 让后端推一条进度事件。 */
export async function emitProgress(
  page: Page,
  payload: Record<string, unknown>,
): Promise<void> {
  await page.evaluate(
    ([event, body]) => window.__FILPILOT_EMIT__?.(event as string, body),
    ['task-progress', payload] as const,
  )
}
