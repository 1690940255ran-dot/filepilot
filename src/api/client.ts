import { invoke } from '@tauri-apps/api/core'
import type { AppError } from './contracts.generated'
import { check, type ContractDefinitionName } from './validation'

/**
 * 已实现的命令白名单。
 *
 * 规格 T01：只注册当前已经实现的命令；后续阶段命令在完成前不显示为可用，
 * 不用恒成功空函数顶替。
 *
 * 规格 3.3：前端只提交 **id**（rootId / scanId / taskId），
 * 绝不提交路径——后端也不会接受前端传来的任何路径字符串。
 */
export type CommandName =
  | 'get_settings'
  | 'get_ocr_status'
  | 'save_provider'
  | 'list_providers'
  | 'test_provider'
  | 'preview_disclosure'
  | 'grant_disclosure'
  | 'start_analysis'
  | 'choose_root'
  | 'start_scan'
  | 'cancel_task'
  | 'get_task'
  | 'list_files'
  | 'create_plan'
  | 'get_plan'
  | 'update_plan'
  | 'validate_plan'
  | 'execute_plan'
  | 'get_run'
  | 'list_runs'
  | 'get_run_items'
  | 'get_recovery'
  | 'acknowledge_recovery'
  | 'recover_pending_runs'
  | 'recovery_status'
  | 'preview_undo'
  | 'execute_undo'
  | 'get_undo_report'

/** 传输层失败（IPC 不可用、Rust panic、序列化错误）的统一错误码。 */
export const TRANSPORT_ERROR_CODE = 'IPC_TRANSPORT_FAILED'
export const IPC_UNAVAILABLE_CODE = 'IPC_UNAVAILABLE'

/** 所有错误码都必须能被 UI 识别，未知码不能被静默当成成功。 */
export function isKnownErrorCode(code: string): boolean {
  return KNOWN_ERROR_CODES.has(code)
}

/**
 * 规格 8.5 的错误码表。Rust 侧返回的 code 均应落在这个集合内；
 * 出现集合外的 code 说明前后端契约不一致，调用方应把它标为未知错误而不是忽略。
 */
const KNOWN_ERROR_CODES: ReadonlySet<string> = new Set([
  'ROOT_NOT_AUTHORIZED',
  'ROOT_CHANGED',
  'UNSUPPORTED_STORAGE',
  'REPARSE_POINT',
  'INVALID_PATH',
  'RESERVED_NAME',
  'PATH_TOO_LONG',
  'TARGET_EXISTS',
  'TARGET_PARENT_IS_FILE',
  'SOURCE_CHANGED',
  'SOURCE_MISSING',
  'FILE_BUSY',
  'PERMISSION_DENIED',
  // T04 的 byMonth 规则用：文件时间戳超出可表示范围。
  // 它此前只存在于后端，前端把它当成未知错误——用户看到的是
  // 「未知错误码」而不是「这个文件的修改时间读不出来」。
  'INVALID_TIMESTAMP',
  'STALE_PLAN',
  'TOKEN_EXPIRED',
  'TOKEN_USED',
  'REQUEST_CONFLICT',
  'TASK_BUSY',
  // T10 内容解析（规格 6.2）。这几个码决定用户看到的是哪一句话：
  // 「文件太大」「编码读不出来」「文件损坏」「文件被加密」「解析进程被杀」。
  // 少了它们，界面只能笼统地说「提取失败」，而用户要做的就是在这几种
  // 完全不同的处理方式里猜。
  'EXTRACTION_TIMEOUT',
  'EXTRACTION_TOO_LARGE',
  'EXTRACTION_GARBLED',
  'EXTRACTION_CORRUPT',
  'EXTRACTION_ENCRYPTED',
  'EXTRACTION_KILLED',
  'EXTRACTION_WORKER_FAILED',
  // T11 图片 OCR（规格 6.2 第四行）。
  // `OCR_UNAVAILABLE` 说的不是「这份文件读不出来」，而是「**这台电脑**
  // 还没准备好」——用户去装个中文语言包就能用，所以它是独立的一句提示，
  // 不能被并进「提取失败」。
  'OCR_UNAVAILABLE',
  // T12 提供商与凭据。
  // `CREDENTIAL_UNAVAILABLE` 说的不是「模型不行」，而是「**这台电脑**的
  // 凭据存储用不了」——用户要去「凭据管理器」看，而不是去改 API Key。
  // `INVALID_ENDPOINT` 说的是地址栏填错了（本地写成了外网、云端写了 http）。
  'CREDENTIAL_UNAVAILABLE',
  'INVALID_ENDPOINT',
  'UNSUPPORTED_FORMAT',
  'MODEL_AUTH',
  'MODEL_TIMEOUT',
  'MODEL_INVALID_OUTPUT',
  'BUDGET_EXCEEDED',
  'JOURNAL_WRITE_FAILED',
  'DB_UNAVAILABLE',
  'RECOVERY_REQUIRED',
  'UNDO_CONFLICT',
  'INTERNAL',
  IPC_UNAVAILABLE_CODE,
  TRANSPORT_ERROR_CODE,
])

/** 前端侧的 IPC 错误。调用方捕获它比捕获裸字符串更安全。 */
export class IpcError extends Error {
  readonly code: string
  readonly retryable: boolean
  readonly details: Record<string, string>

  constructor(error: AppError) {
    super(error.message)
    this.name = 'IpcError'
    this.code = error.code
    this.retryable = error.retryable
    // 复制一份，避免调用方改动共享对象
    this.details = { ...error.details }
  }

  toAppError(): AppError {
    return {
      code: this.code,
      message: this.message,
      retryable: this.retryable,
      details: { ...this.details },
    }
  }
}

/** 结构化地判断一个未知值是不是合法的 AppError，避免把任意 reject 值当成错误对象使用。 */
export function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.code === 'string' &&
    typeof candidate.message === 'string' &&
    typeof candidate.retryable === 'boolean' &&
    typeof candidate.details === 'object' &&
    candidate.details !== null
  )
}

/**
 * 把任意失败源归一化成 AppError。
 *
 * 关键约束：`details` 必须脱敏——不放入文件正文、绝对路径和密钥。
 * 这里只做类型归一，不做内容清洗，内容清洗是 Rust 侧的职责（规格 8.5）。
 */
export function normalizeError(raw: unknown): AppError {
  if (isAppError(raw)) {
    return {
      code: raw.code,
      message: raw.message,
      retryable: raw.retryable,
      details: { ...raw.details },
    }
  }

  if (raw instanceof Error) {
    return {
      code: TRANSPORT_ERROR_CODE,
      message: raw.message,
      retryable: false,
      details: { kind: raw.name },
    }
  }

  if (typeof raw === 'string') {
    return {
      code: TRANSPORT_ERROR_CODE,
      message: raw,
      retryable: false,
      details: {},
    }
  }

  return {
    code: TRANSPORT_ERROR_CODE,
    message: 'IPC 调用失败，且返回了无法识别的错误载荷。',
    retryable: false,
    details: {},
  }
}

/** 浏览器里直接打开（非 Tauri 壳）时没有 IPC，必须明确告知而不是静默失败。 */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/**
 * 调用一个已注册的 Rust 命令。
 *
 * 全命令返回 `Result<T>`（规格 5.2），因此这里把 `ok: false` 转成抛出的 `IpcError`，
 * 让调用方可以用 try/catch 统一处理，而不是在每个调用点判断联合类型。
 */
export async function call<T>(
  command: CommandName,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (!isTauriRuntime()) {
    throw new IpcError({
      code: IPC_UNAVAILABLE_CODE,
      message: '当前不在桌面应用环境中运行，无法调用本地核心。请在 FilePilot 桌面应用内使用。',
      retryable: false,
      details: { command },
    })
  }

  let raw: unknown
  try {
    raw = await invoke<unknown>(command, args)
  } catch (transportFailure) {
    throw new IpcError(normalizeError(transportFailure))
  }

  return decodeResponse<T>(command, raw)
}

type ResponseContract =
  | { kind: 'id' }
  | { kind: 'schema'; definition: ContractDefinitionName }
  | { kind: 'array'; item: ContractDefinitionName }

const RESPONSE_CONTRACT: Record<CommandName, ResponseContract> = {
  get_settings: { kind: 'schema', definition: 'AppSettings' },
  get_ocr_status: { kind: 'schema', definition: 'OcrAvailabilityReport' },
  save_provider: { kind: 'schema', definition: 'ProviderSummary' },
  list_providers: { kind: 'array', item: 'ProviderSummary' },
  test_provider: { kind: 'schema', definition: 'ProviderProbe' },
  preview_disclosure: { kind: 'schema', definition: 'DisclosurePreview' },
  grant_disclosure: { kind: 'schema', definition: 'DisclosureGrant' },
  start_analysis: { kind: 'schema', definition: 'AnalysisStart' },
  choose_root: { kind: 'schema', definition: 'RootSummary' },
  start_scan: { kind: 'id' },
  cancel_task: { kind: 'schema', definition: 'TaskSummary' },
  get_task: { kind: 'schema', definition: 'TaskSummary' },
  list_files: { kind: 'schema', definition: 'FilePage' },
  create_plan: { kind: 'schema', definition: 'PlanBuild' },
  get_plan: { kind: 'schema', definition: 'Plan' },
  update_plan: { kind: 'schema', definition: 'Plan' },
  validate_plan: { kind: 'schema', definition: 'ValidationReport' },
  execute_plan: { kind: 'schema', definition: 'RunReport' },
  get_run: { kind: 'schema', definition: 'RunReport' },
  list_runs: { kind: 'array', item: 'RunReport' },
  get_run_items: { kind: 'schema', definition: 'RunItems' },
  get_recovery: { kind: 'schema', definition: 'RecoveryReport' },
  acknowledge_recovery: { kind: 'schema', definition: 'RecoveryReport' },
  recover_pending_runs: { kind: 'array', item: 'RecoveryReport' },
  recovery_status: { kind: 'schema', definition: 'RecoveryStatus' },
  preview_undo: { kind: 'schema', definition: 'UndoPreview' },
  execute_undo: { kind: 'schema', definition: 'UndoReport' },
  get_undo_report: { kind: 'schema', definition: 'UndoReport' },
}

const NULLABLE_RESPONSE: ReadonlySet<CommandName> = new Set([
  'choose_root',
  'get_task',
  'list_files',
  'cancel_task',
  // 查不到计划/报告不是错误，而是「这个 id 在本会话里不存在」
  'get_plan',
  'validate_plan',
  'get_run',
  // PR-003：明细同理——run 不存在就是 null，与 get_run 口径一致。
  'get_run_items',
  // 撤销报告同理：按 runId 查不到就是「这个 id 不存在」，不是错误。
  'get_undo_report',
  // 恢复命令一律返回真实报告，不允许为 null：
  // 「查不到」在这里是一个必须讲清楚的结论，而不是「没有内容」。
])

/** 验证 IPC 信封和命令对应的数据契约；泛型本身不能验证运行时输入。 */
export function decodeResponse<T>(command: CommandName, raw: unknown): T {
  if (typeof raw !== 'object' || raw === null) {
    throw invalidContract(command, ['响应不是对象'])
  }
  const envelope = raw as Record<string, unknown>
  if (envelope.ok === true) {
    if (!hasExactKeys(envelope, ['ok', 'data'])) {
      throw invalidContract(command, ['成功响应含未知字段或缺少 data'])
    }
    if (envelope.data === null && NULLABLE_RESPONSE.has(command)) {
      return null as T
    }
    const contract = RESPONSE_CONTRACT[command]
    if (contract.kind === 'id') {
      if (typeof envelope.data !== 'string' || envelope.data.length === 0) {
        throw invalidContract(command, ['taskId 必须是非空字符串'])
      }
      return envelope.data as T
    }
    if (contract.kind === 'array') {
      if (!Array.isArray(envelope.data)) {
        throw invalidContract(command, ['响应必须是数组'])
      }
      const errors = envelope.data.flatMap((item, index) => {
        const outcome = check(contract.item, item)
        return outcome.valid ? [] : outcome.errors.map((error) => `[${index}] ${error}`)
      })
      if (errors.length > 0) throw invalidContract(command, errors)
      return envelope.data as T
    }
    const outcome = check(contract.definition, envelope.data)
    if (!outcome.valid) throw invalidContract(command, outcome.errors)
    return envelope.data as T
  }
  if (envelope.ok === false) {
    if (!hasExactKeys(envelope, ['ok', 'error'])) {
      throw invalidContract(command, ['失败响应含未知字段或缺少 error'])
    }
    const outcome = check('AppError', envelope.error)
    if (!outcome.valid) throw invalidContract(command, outcome.errors)
    throw new IpcError(envelope.error as AppError)
  }
  throw invalidContract(command, ['响应缺少布尔字段 ok'])
}

function hasExactKeys(value: Record<string, unknown>, expected: string[]): boolean {
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  return actual.length === wanted.length && actual.every((key, i) => key === wanted[i])
}

function invalidContract(command: CommandName, errors: string[]): IpcError {
  return new IpcError({
    code: TRANSPORT_ERROR_CODE,
    message: '本地核心返回的数据不符合应用契约，已拒绝使用。',
    retryable: false,
    details: { command, validation: errors.join('; ').slice(0, 500) },
  })
}
