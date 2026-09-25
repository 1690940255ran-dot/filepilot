import { useCallback, useMemo, useRef, useState } from 'react'

import { call, IpcError } from '../../api/client'
import type {
  AnalysisStart,
  DisclosureGrant,
  DisclosurePreview,
  Mode,
} from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'

/**
 * AI 模式的状态。
 *
 * 用联合类型而不是若干 boolean：`previewed` + `granted` + `analyzing`
 * 这样的组合能表达出「还没预览却已经授权」这种非法状态，而联合类型不能。
 */
export type DisclosurePhase =
  | { status: 'idle' }
  | { status: 'preparing' }
  | { status: 'ready'; preview: DisclosurePreview }
  | { status: 'granting'; preview: DisclosurePreview }
  | { status: 'starting'; preview: DisclosurePreview }
  | { status: 'running'; preview: DisclosurePreview; taskId: string; analysisId: string }
  | { status: 'done'; analysisId: string }

export interface DisclosureError {
  message: string
  retryable: boolean
}

export interface PrepareInput {
  scanId: string
  fileIds: string[]
  mode: Mode
  providerId: string
  instruction: string
}

export interface AiDisclosureFlow {
  phase: DisclosurePhase
  error: DisclosureError | null
  /** 用户点了「这个文件不发正文」的那些 fileId。 */
  excluded: string[]
  /** 当前这次预览的待发送内容；还没预览过就是 `null`。 */
  preview: DisclosurePreview | null
  /** 现在能不能开始分析。 */
  canStart: boolean
  /** 现在能不能授权。云模式必须先授权。 */
  canGrant: boolean
  /** 云模式是否已经拿到授权。 */
  granted: boolean
  prepare: (input: PrepareInput) => Promise<void>
  toggleExcluded: (fileId: string) => Promise<void>
  grant: () => Promise<void>
  start: () => Promise<void>
  markDone: (analysisId: string) => void
  /** 后台任务失败时由调用方告知。界面不能一直停在「正在处理…」。 */
  markFailed: (message: string) => void
  reset: () => void
}

function toError(raw: unknown): DisclosureError {
  if (raw instanceof IpcError) {
    return { message: raw.message, retryable: raw.retryable }
  }
  return {
    message: raw instanceof Error ? raw.message : t.common.unknownError,
    retryable: false,
  }
}

/** 本地模式的数据不出本机，所以规格允许它不带授权 id（`consentId=null`）。 */
function isLocal(mode: Mode): boolean {
  return mode === 'aiLocal'
}

/**
 * 「预览待发送内容 → 授权 → 开始分析」的流程（规格 6.4 的前四步）。
 *
 * ## 每次改动都重新预览
 *
 * `toggleExcluded` **不**只是改一个本地数组：它带着新的排除集合重新调
 * `preview_disclosure`。因为载荷的摘要会随之改变，而授权绑定的正是那个摘要
 * ——本地改一下、却拿旧摘要去授权，后端会拒绝，而用户看到的是一句莫名其妙
 * 的「载荷已变化」。
 *
 * 同理，**改要求或换提供商必须重新 `prepare`**，而不是在本地记一个新值
 * 然后拿旧授权去开始：规格 6.4 说得很直接——「编辑 instruction 同样令旧
 * 授权失效」。与其在前端自己判断「这次改动要不要作废授权」（判断错了就是
 * **用旧授权发了新内容**），不如让任何改动都回到起点。
 */
export function useAiDisclosure(): AiDisclosureFlow {
  const [phase, setPhase] = useState<DisclosurePhase>({ status: 'idle' })
  const [error, setError] = useState<DisclosureError | null>(null)
  const [excluded, setExcluded] = useState<string[]>([])
  const [consentId, setConsentId] = useState<string | null>(null)
  // 记住上一次的输入，好在排除项变化时重新预览。
  const [input, setInput] = useState<PrepareInput | null>(null)

  /**
   * 单调递增的请求序号。**只接受最新那次的结果。**
   *
   * ## 它防的是什么
   *
   * 用户快改两下要求（或连着开关几个文件的正文），于是有两个
   * `preview_disclosure` 同时在飞。若**先发的后回**，它带回来的是
   * **旧要求**对应的预览——而用户正盯着屏幕以为自己看的是新要求的内容。
   *
   * 这个错误最坏的地方在于它**看起来完全正常**：有文件名、有字符数、
   * 有内容开头，只是全部对应上一次的输入。用户据此点「确认并授权」，
   * 授权的就是一份他以为已经改掉的载荷。
   *
   * 后端的摘要校验会挡住它（载荷确实变了），但那时用户看到的是一句
   * 「载荷已变化」——一句与真实原因（「你的前一个请求回来晚了」）
   * 完全对不上的话。
   *
   * 各页面原有的 `cancelled` flag 只防「卸载后 setState」，
   * **不防这件事**：组件还在，只是状态被旧数据盖了。
   */
  const requestSeq = useRef(0)

  /** 这次请求是不是最新的一次。 */
  const isLatest = useCallback((seq: number) => seq === requestSeq.current, [])

  /** 作废所有在飞的请求（用户重新开始时用）。 */
  const invalidatePending = useCallback(() => {
    requestSeq.current += 1
  }, [])

  /** 真正调预览的那一段——初次预览与切换排除项共用。 */
  const runPreview = useCallback(
    async (target: PrepareInput, nextExcluded: string[]) => {
      const seq = ++requestSeq.current
      setPhase({ status: 'preparing' })
      try {
        const preview = await call<DisclosurePreview>('preview_disclosure', {
          scanId: target.scanId,
          selectedFileIds: target.fileIds,
          mode: target.mode,
          providerId: target.providerId,
          instruction: target.instruction,
          // 关掉正文**不**把文件从选中列表里去掉：关的是正文，不是文件。
          excludedFileIds: nextExcluded,
        })
        if (!isLatest(seq)) return
        setPhase({ status: 'ready', preview })
      } catch (raw) {
        if (!isLatest(seq)) return
        setError(toError(raw))
        setPhase({ status: 'idle' })
      }
    },
    [isLatest],
  )

  const prepare = useCallback(
    async (next: PrepareInput) => {
      setError(null)
      // 新的输入 = 新的载荷，旧授权一律作废。
      setConsentId(null)
      setInput(next)
      await runPreview(next, excluded)
    },
    [excluded, runPreview],
  )

  const toggleExcluded = useCallback(
    async (fileId: string) => {
      if (input === null) return
      const next = excluded.includes(fileId)
        ? excluded.filter((id) => id !== fileId)
        : [...excluded, fileId]
      setExcluded(next)
      // 载荷会变，授权也随之作废。
      setConsentId(null)
      setError(null)
      await runPreview(input, next)
    },
    [excluded, input, runPreview],
  )

  const grant = useCallback(async () => {
    if (phase.status !== 'ready' || input === null) return
    const current = phase.preview
    const seq = ++requestSeq.current
    setError(null)
    setPhase({ status: 'granting', preview: current })
    try {
      const granted = await call<DisclosureGrant>('grant_disclosure', {
        payloadDigest: current.payloadDigest,
        providerId: input.providerId,
      })
      if (!isLatest(seq)) return
      setConsentId(granted.consentId)
      setPhase({ status: 'ready', preview: current })
    } catch (raw) {
      if (!isLatest(seq)) return
      setError(toError(raw))
      setPhase({ status: 'ready', preview: current })
    }
  }, [phase, input, isLatest])

  const start = useCallback(async () => {
    if (phase.status !== 'ready' || input === null) return

    // 云模式没有授权 id 就**不发**：后端也会拒绝，但在这里挡住能给出
    // 一句人话，而不是让用户看一个「必须带上授权 id」的接口错误。
    if (!isLocal(input.mode) && consentId === null) {
      setError({ message: t.ai.needGrant, retryable: false })
      return
    }

    const current = phase.preview
    const seq = ++requestSeq.current
    setError(null)
    setPhase({ status: 'starting', preview: current })
    try {
      const started = await call<AnalysisStart>('start_analysis', {
        scanId: input.scanId,
        selectedFileIds: input.fileIds,
        mode: input.mode,
        providerId: input.providerId,
        instruction: input.instruction,
        // 本地模式传 `null`（规格：「本地模式 `consentId=null`」）。
        consentId: isLocal(input.mode) ? null : consentId,
      })
      if (!isLatest(seq)) return
      setPhase({
        status: 'running',
        preview: current,
        taskId: started.taskId,
        analysisId: started.analysisId,
      })
    } catch (raw) {
      if (!isLatest(seq)) return
      setError(toError(raw))
      setPhase({ status: 'ready', preview: current })
    }
  }, [phase, input, consentId, isLatest])

  const markDone = useCallback((analysisId: string) => {
    setPhase({ status: 'done', analysisId })
  }, [])

  const markFailed = useCallback((message: string) => {
    // 回到起点而不是停在 `running`：用户可以改要求重来，而界面上……
    // 不能一直转圈。载荷没有变，所以他不必重新预览。
    setError({ message, retryable: true })
    setPhase((current) =>
      'preview' in current
        ? { status: 'ready', preview: current.preview }
        : { status: 'idle' },
    )
  }, [])

  const reset = useCallback(() => {
    // 作废在飞的请求：用户已经重新开始了，它的结果不该再落到界面上。
    invalidatePending()
    setPhase({ status: 'idle' })
    setError(null)
    setExcluded([])
    setConsentId(null)
    setInput(null)
  }, [invalidatePending])

  const preview = useMemo(
    () => ('preview' in phase ? phase.preview : null),
    [phase],
  )

  const ready = phase.status === 'ready'
  const local = input !== null && isLocal(input.mode)

  return {
    phase,
    error,
    excluded,
    preview,
    granted: consentId !== null,
    // 本地模式不需要授权，所以它一预览完就能开始。
    canStart: ready && (local || consentId !== null),
    canGrant: ready && consentId === null,
    prepare,
    toggleExcluded,
    grant,
    start,
    markDone,
    markFailed,
    reset,
  }
}
