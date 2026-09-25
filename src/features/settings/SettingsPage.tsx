import type { JSX } from 'react'
import { t } from '../../i18n/zh-CN'
import { ProviderSection } from './ProviderSection'
import type { ProviderListState } from './providerState'
import type { OcrStatusState } from './ocrStatus'
import type { SettingsState } from './settingsState'

const MODE_LABEL: Record<string, string> = {
  rules: t.home.modeRules,
  aiLocal: t.home.modeAiLocal,
  aiCloud: t.home.modeAiCloud,
}

interface SettingsPageProps {
  state: SettingsState
  ocrState: OcrStatusState
  providerState: ProviderListState
  onRetry: () => void
  onRetryOcr: () => void
  onRetryProviders: () => void
  /** 保存成功后重新拉列表——列表才是「磁盘上有什么」的唯一来源。 */
  onProvidersChanged: () => void
}

/**
 * 把 OCR 状态翻译成一句给用户看的话。
 *
 * 可用时**留下语言列表**：用户真正想知道的是「中文行不行」，
 * 只说「可用」他还得自己去猜。
 *
 * 不可用时直接显示后端给的原因，而不是在这里另写一套——
 * 那句话里带着下一步动作（去装哪个语言包），是后端算得最准的一部分。
 */
function ocrSummary(ocrState: OcrStatusState): { text: string; fallback: boolean } {
  switch (ocrState.status) {
    case 'loading':
      return { text: t.settings.ocrChecking, fallback: false }
    case 'error':
      return { text: `${t.settings.ocrUnreadable}（${ocrState.message}）`, fallback: false }
    case 'ready':
      return ocrState.report.status === 'available'
        ? {
            text: `${t.settings.ocrAvailable}${ocrState.report.languages.join('、')}`,
            fallback: false,
          }
        : { text: ocrState.report.message, fallback: true }
  }
}

export function SettingsPage({
  state,
  ocrState,
  providerState,
  onRetry,
  onRetryOcr,
  onRetryProviders,
  onProvidersChanged,
}: SettingsPageProps): JSX.Element {
  const ocr = ocrSummary(ocrState)

  return (
    <section className="page" aria-labelledby="settings-heading">
      <h2 className="page-heading" id="settings-heading">
        {t.settings.heading}
      </h2>

      {state.status === 'loading' && (
        <p className="loading" role="status">
          {t.common.loading}
        </p>
      )}

      {state.status === 'error' && (
        <div className="error-box" role="alert">
          <p>{state.message}</p>
          {state.retryable && (
            <button type="button" className="secondary-action" onClick={onRetry}>
              {t.common.retry}
            </button>
          )}
        </div>
      )}

      {state.status === 'ready' && (
        <dl className="info-grid">
          <dt>{t.settings.modeLabel}</dt>
          <dd>{MODE_LABEL[state.settings.mode] ?? state.settings.mode}</dd>

          <dt>{t.settings.scanMaxFilesLabel}</dt>
          <dd>{state.settings.scanMaxFiles}</dd>

          <dt>{t.settings.scanMaxDepthLabel}</dt>
          <dd>{state.settings.scanMaxDepth}</dd>

          <dt>{t.settings.providerLabel}</dt>
          <dd>{state.settings.selectedProviderId ?? t.settings.providerNone}</dd>

          <dt>{t.settings.ocrLabel}</dt>
          <dd>
            {ocrState.status === 'loading' ? (
              <span className="loading" role="status">
                {ocr.text}
              </span>
            ) : (
              ocr.text
            )}
          </dd>
        </dl>
      )}

      {/* OCR 查询自己失败时给一个重试入口：它和设置是两次独立请求，
          设置读到了不代表 OCR 状态也读到了。 */}
      {ocrState.status === 'error' && (
        <button type="button" className="secondary-action" onClick={onRetryOcr}>
          {t.common.retry}
        </button>
      )}

      {/* 规格 T11：用户无需 OCR 也可用规则模式。
          这句只在**不可用**时出现——可用的时候说它纯属噪音。 */}
      {ocr.fallback && <p className="notice">{t.settings.ocrFallbackNotice}</p>}

      <ProviderSection
        listState={providerState}
        onRetryList={onRetryProviders}
        onSaved={onProvidersChanged}
      />

      <p className="notice">{t.settings.readOnlyNotice}</p>
      <p className="notice">{t.settings.secretsNotice}</p>
    </section>
  )
}
