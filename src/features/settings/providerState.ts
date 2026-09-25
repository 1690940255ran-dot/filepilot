import type { ProviderKind, ProviderProbe, ProviderSummary } from '../../api/contracts.generated'

/**
 * 提供商列表的加载状态。
 *
 * 形状与 `SettingsState` / `OcrStatusState` 一致：加载一次、成功给列表、
 * 失败给可重试的原因。三种页面状态用同一个形状，「每页覆盖
 * loading / error / ready」这条规格就只需要记一次。
 */
export type ProviderListState =
  | { status: 'loading' }
  | { status: 'ready'; providers: ProviderSummary[] }
  | { status: 'error'; message: string; retryable: boolean }

/**
 * 表单里正在编辑的那一份。
 *
 * `secret` **只在这里存在**：保存成功之后立刻清空，之后界面只显示
 * 「已保存密钥」这一个事实（见 `ProviderSummary.hasCredential`）。
 * 规格 3.3 要求「前端保存后不再读回明文」——所以这个字段的生命周期
 * 必须短到「提交那一刻」，而不是「这一页还开着的时候」。
 */
export interface ProviderDraft {
  id: string
  kind: ProviderKind
  endpoint: string
  model: string
  secret: string
}

export function emptyDraft(): ProviderDraft {
  return {
    // 用一个默认值而不是空串：空 id 会被后端拒绝，而用户看到「id 不能为空」
    // 时并不知道该填什么。默认值让他直接改端点就行。
    id: 'default',
    // 默认本地：绝大多数用户第一次配置的是本机的 Ollama，而它不需要密钥。
    kind: 'local',
    endpoint: 'http://127.0.0.1:11434',
    model: '',
    secret: '',
  }
}

/**
 * 从一条已有配置生成草稿。
 *
 * **不带密钥**：`ProviderSummary` 里根本没有密钥，而这不是限制、
 * 是设计——界面无法把密钥读回来，所以也就无法把它塞进草稿。
 */
export function draftFromProvider(provider: ProviderSummary): ProviderDraft {
  return {
    id: provider.id,
    kind: provider.kind,
    endpoint: provider.endpoint,
    model: provider.model,
    secret: '',
  }
}

/** 保存动作的状态。 */
export type ProviderSaveState =
  | { status: 'idle' }
  | { status: 'saving' }
  /**
   * 保存成功。
   *
   * **刻意不带后端的返回值**：保存之后界面需要的事实只有「成功了」，
   * 而「列表长什么样」由刷新后的 `ProviderListState` 负责——那是唯一来源。
   * 再存一份摘要，就会让「界面凭什么显示这些」多出一个可能与之不一致的来源。
   */
  | { status: 'saved' }
  | { status: 'error'; message: string }

/** 连通性测试的状态。 */
export type ProviderProbeState =
  | { status: 'idle' }
  | { status: 'testing' }
  | { status: 'done'; probe: ProviderProbe }
  | { status: 'error'; message: string }

/**
 * 提交前的前端校验。
 *
 * 只做两件后端做不了或做得更晚的事：**空值**与**地址形状**。
 * 真正的安全校验（loopback、HTTPS）在后端，前端不重复实现第二套规则——
 * 两套规则迟早会不一致，而那时以哪一套为准是个没人能回答的问题。
 */
export function draftProblem(draft: ProviderDraft): string | null {
  if (draft.id.trim() === '') return '需要一个标识，用来在本地保存这份配置。'
  if (draft.endpoint.trim() === '') return '请填写服务地址。'
  if (!/^https?:\/\//i.test(draft.endpoint.trim())) {
    return '服务地址要以 http:// 或 https:// 开头。'
  }
  if (draft.model.trim() === '') return '请填写模型名。'
  return null
}
