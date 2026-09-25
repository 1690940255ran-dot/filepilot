import type { OcrAvailabilityReport } from '../../api/contracts.generated'

/**
 * 本机 OCR 状态的加载状态（规格 T11：设置页显示 OCR 可用状态和原因）。
 *
 * 形状与 `SettingsState` 一致：探测一次、成功给报告、失败给可重试的原因。
 *
 * 没有「未开始」这一态，初始值就是 `loading`——探测要**起一个子进程**去问
 * 系统装了哪些识别语言，而它只在用户真的打开设置页时才发生；对用户来说
 * 「还没开始」和「正在开始」是同一件事，他看到的就是「正在检查」。
 */
export type OcrStatusState =
  | { status: 'loading' }
  | { status: 'ready'; report: OcrAvailabilityReport }
  | { status: 'error'; message: string; retryable: boolean }
