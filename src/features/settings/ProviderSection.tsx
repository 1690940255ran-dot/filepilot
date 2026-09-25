import { useState, type FormEvent, type JSX } from 'react'
import { call, IpcError } from '../../api/client'
import type { ProviderProbe, ProviderSummary } from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'
import {
  draftFromProvider,
  draftProblem,
  emptyDraft,
  type ProviderDraft,
  type ProviderListState,
  type ProviderProbeState,
  type ProviderSaveState,
} from './providerState'

interface ProviderSectionProps {
  listState: ProviderListState
  onRetryList: () => void
  /** 保存成功后刷新列表——列表才是「磁盘上有什么」的唯一来源。 */
  onSaved: () => void
}

/**
 * 提供商配置区。
 *
 * ## 密钥在这个组件里的生命周期
 *
 * 它**只**存在于 `draft.secret` 这一个字符串里，而那个字符串在保存成功
 * 之后立刻被清空。此后界面只知道「有一条密钥」（`hasCredential`），
 * 拿不到它的内容——规格 3.3 要求「前端保存后不再读回明文」。
 *
 * 所以这里没有、也不该有「显示/隐藏密钥」的开关：能显示就意味着能读回，
 * 而那是这条规格明确排除的能力。
 */
export function ProviderSection({
  listState,
  onRetryList,
  onSaved,
}: ProviderSectionProps): JSX.Element {
  const [draft, setDraft] = useState<ProviderDraft>(emptyDraft)
  const [saveState, setSaveState] = useState<ProviderSaveState>({ status: 'idle' })
  const [probeState, setProbeState] = useState<ProviderProbeState>({ status: 'idle' })
  /** 正在测试哪一个。`null` 表示没有测试在进行。 */
  const [probingId, setProbingId] = useState<string | null>(null)

  const problem = draftProblem(draft)
  const saving = saveState.status === 'saving'

  function update<K extends keyof ProviderDraft>(key: K, value: ProviderDraft[K]): void {
    setDraft((current) => ({ ...current, [key]: value }))
  }

  async function submit(event: FormEvent): Promise<void> {
    event.preventDefault()
    if (problem !== null) return

    setSaveState({ status: 'saving' })
    setProbeState({ status: 'idle' })
    try {
      // 返回值只用到这里：它的类型让 `call` 知道该按哪份 schema 校验响应。
      // 界面不保存它——保存后的事实来源是刷新过的列表。
      await call<ProviderSummary>('save_provider', {
        providerId: draft.id.trim(),
        kind: draft.kind,
        endpoint: draft.endpoint.trim(),
        model: draft.model.trim(),
        // **留空 = 不改动已有密钥**，而不是「清掉密钥」。
        // 用户改端点时不该被要求重填密钥。
        secret: draft.secret === '' ? null : draft.secret,
      })
      setSaveState({ status: 'saved' })
      // 密钥从这一刻起不在界面里存在。
      setDraft((current) => ({ ...current, secret: '' }))
      onSaved()
    } catch (error: unknown) {
      const appError = error instanceof IpcError ? error.toAppError() : null
      setSaveState({
        status: 'error',
        message: appError?.message ?? t.common.unknownError,
      })
    }
  }

  async function probe(id: string): Promise<void> {
    setProbingId(id)
    setProbeState({ status: 'testing' })
    try {
      const probe = await call<ProviderProbe>('test_provider', { providerId: id })
      setProbeState({ status: 'done', probe })
    } catch (error: unknown) {
      const appError = error instanceof IpcError ? error.toAppError() : null
      setProbeState({
        status: 'error',
        message: appError?.message ?? t.common.unknownError,
      })
    } finally {
      setProbingId(null)
    }
  }

  return (
    <section className="provider-section" aria-labelledby="provider-heading">
      <h3 className="section-heading" id="provider-heading">
        {t.settings.providerHeading}
      </h3>
      <p className="notice">{t.settings.providerDescription}</p>

      {listState.status === 'loading' && (
        <p className="loading" role="status">
          {t.common.loading}
        </p>
      )}

      {listState.status === 'error' && (
        <div className="error-box" role="alert">
          <p>{listState.message}</p>
          {listState.retryable && (
            <button type="button" className="secondary-action" onClick={onRetryList}>
              {t.common.retry}
            </button>
          )}
        </div>
      )}

      {listState.status === 'ready' && listState.providers.length === 0 && (
        <p className="empty">{t.settings.providerEmpty}</p>
      )}

      {listState.status === 'ready' && listState.providers.length > 0 && (
        // `aria-label` 不只是为了测试能精确定位：一个列表如果没有可访问
        // 名称，屏幕阅读器只会念「列表，7 项」，用户不知道那是什么的列表。
        <ul className="provider-list" aria-label={t.settings.providerListLabel}>
          {listState.providers.map((provider) => (
            <li className="provider-item" key={provider.id}>
              <span className="provider-id">{provider.id}</span>
              <span className="provider-kind">
                {provider.kind === 'local'
                  ? t.settings.providerKindLocal
                  : t.settings.providerKindCompatible}
              </span>
              <span className="provider-endpoint">{provider.endpoint}</span>
              <span className="provider-model">{provider.model}</span>
              {/* 只报「有没有」，不报内容。 */}
              <span className="provider-credential">
                {provider.hasCredential
                  ? t.settings.providerSecretSaved
                  : t.settings.providerSecretAbsent}
              </span>
              <button
                type="button"
                className="secondary-action"
                onClick={() => setDraft(draftFromProvider(provider))}
              >
                {t.settings.providerEdit}
              </button>
              <button
                type="button"
                className="secondary-action"
                disabled={probingId !== null}
                onClick={() => void probe(provider.id)}
              >
                {probingId === provider.id ? t.settings.providerTesting : t.settings.providerTest}
              </button>
            </li>
          ))}
        </ul>
      )}

      <form className="provider-form" onSubmit={(event) => void submit(event)}>
        <div className="field">
          <label htmlFor="provider-id">{t.settings.providerIdLabel}</label>
          <input
            id="provider-id"
            type="text"
            value={draft.id}
            onChange={(event) => update('id', event.target.value)}
          />
        </div>

        <fieldset className="field">
          <legend>{t.settings.providerKindLabel}</legend>
          <label>
            <input
              type="radio"
              name="provider-kind"
              checked={draft.kind === 'local'}
              onChange={() => update('kind', 'local')}
            />
            {t.settings.providerKindLocal}
          </label>
          <label>
            <input
              type="radio"
              name="provider-kind"
              checked={draft.kind === 'compatible'}
              onChange={() => update('kind', 'compatible')}
            />
            {t.settings.providerKindCompatible}
          </label>
        </fieldset>

        <div className="field">
          <label htmlFor="provider-endpoint">{t.settings.providerEndpointLabel}</label>
          <input
            id="provider-endpoint"
            type="text"
            value={draft.endpoint}
            onChange={(event) => update('endpoint', event.target.value)}
          />
          <p className="field-hint">
            {draft.kind === 'local'
              ? t.settings.providerLocalHint
              : t.settings.providerCloudHint}
          </p>
        </div>

        <div className="field">
          <label htmlFor="provider-model">{t.settings.providerModelLabel}</label>
          <input
            id="provider-model"
            type="text"
            value={draft.model}
            onChange={(event) => update('model', event.target.value)}
          />
        </div>

        <div className="field">
          <label htmlFor="provider-secret">{t.settings.providerSecretLabel}</label>
          <input
            id="provider-secret"
            type="password"
            autoComplete="off"
            value={draft.secret}
            placeholder={t.settings.providerSecretPlaceholder}
            onChange={(event) => update('secret', event.target.value)}
          />
          {/* 密钥不会显示回来——这句提示告诉用户「留空不等于清空」。 */}
          <p className="field-hint">{t.settings.providerSecretHint}</p>
        </div>

        <button type="submit" className="primary-action" disabled={saving || problem !== null}>
          {saving ? t.settings.providerSaving : t.settings.providerSave}
        </button>

        {problem !== null && <p className="field-problem">{problem}</p>}
      </form>

      {saveState.status === 'saved' && (
        <p className="notice" role="status">
          {t.settings.providerSaved}
        </p>
      )}

      {saveState.status === 'error' && (
        <div className="error-box" role="alert">
          <p>{saveState.message}</p>
        </div>
      )}

      {probeState.status === 'testing' && (
        <p className="loading" role="status">
          {t.settings.providerTesting}
        </p>
      )}

      {probeState.status === 'done' && (
        <p className="notice" role="status">
          {probeState.probe.message}
        </p>
      )}

      {probeState.status === 'error' && (
        <div className="error-box" role="alert">
          <p>{probeState.message}</p>
        </div>
      )}
    </section>
  )
}
