import type { AppSettings } from '../../api/contracts.generated'

/**
 * 设置的加载状态。规格 T14 要求每页覆盖 loading / empty / error / ready，
 * 因此用显式联合类型而不是散落的 boolean，避免出现「既在加载又有错误」的非法态。
 */
export type SettingsState =
  | { status: 'loading' }
  | { status: 'ready'; settings: AppSettings }
  | { status: 'error'; message: string; retryable: boolean }
