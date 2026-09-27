import { useCallback, useEffect, useRef, useState, type JSX } from 'react'
import { call, IpcError } from './api/client'
import type {
  AppSettings,
  OcrAvailabilityReport,
  ProviderSummary,
  RecoveryStatus,
} from './api/contracts.generated'
import { t } from './i18n/zh-CN'
import { OrganizePage } from './features/organize/OrganizePage'
import { PreviewPage } from './features/preview/PreviewPage'
import { HistoryPage } from './features/history/HistoryPage'
import { RecoveryPage } from './features/recovery/RecoveryPage'
import { SettingsPage } from './features/settings/SettingsPage'
import { UndoPage } from './features/undo/UndoPage'
import type { OcrStatusState } from './features/settings/ocrStatus'
import type { ProviderListState } from './features/settings/providerState'
import type { SettingsState } from './features/settings/settingsState'

type Page = 'home' | 'preview' | 'history' | 'recovery' | 'undo' | 'settings'

/**
 * 主导航。
 *
 * 「预览」与「恢复」不在其中：它们只有存在对应内容时才作为一个入口出现——
 * 一个永远点不进去的导航项只会让人以为功能坏了。
 */
const BASE_PAGES: ReadonlyArray<{ id: Page; label: string }> = [
  { id: 'home', label: t.nav.home },
  { id: 'history', label: t.nav.history },
  { id: 'settings', label: t.nav.settings },
]

export function App(): JSX.Element {
  const [page, setPage] = useState<Page>('home')
  const mainRef = useRef<HTMLElement | null>(null)
  useEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = 0
  }, [page])
  const [settingsState, setSettingsState] = useState<SettingsState>({ status: 'loading' })
  // 递增该值即重新拉取设置。用计数器而不是把 loading 写进 effect 主体，
  // 是为了避免「在 effect 里同步 setState」造成的级联渲染。
  const [reloadToken, setReloadToken] = useState(0)
  /** 已生成的计划。为 null 时没有可预览的内容。 */
  const [plan, setPlan] = useState<{ id: string; rootPath: string } | null>(null)

  useEffect(() => {
    let cancelled = false

    // setState 只发生在 .then / .catch 回调里（异步），不在 effect 主体内同步调用。
    void call<AppSettings>('get_settings')
      .then((settings) => {
        if (cancelled) return
        setSettingsState({ status: 'ready', settings })
      })
      .catch((error: unknown) => {
        if (cancelled) return
        const appError = error instanceof IpcError ? error.toAppError() : null
        setSettingsState({
          status: 'error',
          message: appError?.message ?? t.common.unknownError,
          retryable: appError?.retryable ?? false,
        })
      })

    return () => {
      cancelled = true
    }
  }, [reloadToken])

  const retry = useCallback(() => {
    setSettingsState({ status: 'loading' })
    setReloadToken((token) => token + 1)
  }, [])

  /*
    OCR 状态（规格 T11：设置页显示可用状态和原因）。

    它**按需加载**，而不是像设置那样启动时就拉：探测要走一个子进程去问
    系统装了哪些识别语言，而绝大多数用户从不打开设置页。

    「按需」在这里必须看 `page`，不能靠组件挂载——所有页面都是常挂载的
    （切换导航只切 hidden），靠挂载会在启动时就触发。

    初始值就是 `loading` 而不是某个「未开始」态：用户打开设置页的那一刻
    请求就发出去了，他看到的理应是「正在检查」。这样 effect 里也不必
    同步 setState——那条 lint 规则（react-hooks/set-state-in-effect）
    正是为了防级联渲染。
  */
  const [ocrState, setOcrState] = useState<OcrStatusState>({ status: 'loading' })
  const [ocrReloadToken, setOcrReloadToken] = useState(0)

  useEffect(() => {
    if (page !== 'settings') return

    let cancelled = false

    void call<OcrAvailabilityReport>('get_ocr_status')
      .then((report) => {
        if (cancelled) return
        setOcrState({ status: 'ready', report })
      })
      .catch((error: unknown) => {
        if (cancelled) return
        const appError = error instanceof IpcError ? error.toAppError() : null
        setOcrState({
          status: 'error',
          message: appError?.message ?? t.common.unknownError,
          retryable: appError?.retryable ?? false,
        })
      })

    return () => {
      cancelled = true
    }
  }, [page, ocrReloadToken])

  const retryOcr = useCallback(() => {
    setOcrState({ status: 'loading' })
    setOcrReloadToken((token) => token + 1)
  }, [])

  /*
    提供商列表。

    与 OCR 状态一样**按需加载**（只在打开设置页时拉），但理由不同：
    它只是一次本地 SQLite 查询，代价很小；放在这里是为了让「设置页要用
    的东西都在同一个地方装配」，而不是每个区块各自去发请求——那样
    区块就没法脱离 IPC 单独渲染与测试了。
  */
  const [providerState, setProviderState] = useState<ProviderListState>({ status: 'loading' })
  const [providerReloadToken, setProviderReloadToken] = useState(0)

  useEffect(() => {
    if (page !== 'settings') return

    let cancelled = false

    void call<ProviderSummary[]>('list_providers')
      .then((providers) => {
        if (cancelled) return
        setProviderState({ status: 'ready', providers })
      })
      .catch((error: unknown) => {
        if (cancelled) return
        const appError = error instanceof IpcError ? error.toAppError() : null
        setProviderState({
          status: 'error',
          message: appError?.message ?? t.common.unknownError,
          retryable: appError?.retryable ?? false,
        })
      })

    return () => {
      cancelled = true
    }
  }, [page, providerReloadToken])

  const reloadProviders = useCallback(() => {
    setProviderState({ status: 'loading' })
    setProviderReloadToken((token) => token + 1)
  }, [])

  const openPreview = useCallback((planId: string, rootPath: string) => {
    setPlan({ id: planId, rootPath })
    setPage('preview')
  }, [])

  /*
    恢复页要核对的执行记录。它**不在导航里**：入口是历史页上那条
    「需要人工核对」的记录——没有未决项时，一个空的恢复页只会让人困惑。
  */
  const [recoveryRunId, setRecoveryRunId] = useState<string | null>(null)

  const openRecovery = useCallback((runId: string) => {
    setRecoveryRunId(runId)
    setPage('recovery')
  }, [])

  /*
    撤销页要撤销的那次整理。与恢复页同源：**不在导航里**，
    入口是历史页上那条整理记录的「撤销」按钮——没有任何整理记录时，
    一个空的撤销页只会让人以为功能坏了。
  */
  const [undoRunId, setUndoRunId] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void call<RecoveryStatus>('recovery_status')
      .then((status) => {
        if (cancelled || !status.blocked) return
        const first = status.blockedRuns[0]
        if (!first) return
        setRecoveryRunId(first.runId)
        setPage('recovery')
      })
      // 设置加载仍会展示自己的错误；恢复概况失败不能把整个应用变成空白页。
      .catch(() => undefined)

    return () => {
      cancelled = true
    }
  }, [])

  const openUndo = useCallback((runId: string) => {
    setUndoRunId(runId)
    setPage('undo')
  }, [])

  const pages: ReadonlyArray<{ id: Page; label: string }> = plan
    ? [BASE_PAGES[0]!, { id: 'preview', label: t.nav.preview }, ...BASE_PAGES.slice(1)]
    : BASE_PAGES

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        {t.a11y.skipToContent}
      </a>

      <header className="app-header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7"><path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/><path d="m9 14 2 2 4-4"/></svg>
          </span>
          <div>
            <h1 className="brand-name">{t.app.name}</h1>
            <p className="brand-tagline">{t.app.nameEn}</p>
          </div>
        </div>

        <nav className="app-nav" aria-label={t.nav.ariaLabel}>
          {pages.map((entry, index) => (
            <button
              key={entry.id}
              type="button"
              className={entry.id === page ? 'nav-item nav-item-active' : 'nav-item'}
              aria-current={entry.id === page ? 'page' : undefined}
              onClick={() => setPage(entry.id)}
            >
              <span className="nav-index" aria-hidden="true">{String(index + 1).padStart(2, '0')}</span>
              {entry.label}
            </button>
          ))}
        </nav>
        <div className="sidebar-note"><strong>{t.app.safety}</strong><p>{t.app.safetyHint}</p></div>
      </header>

      <main className="app-main" id="main-content" ref={mainRef}>
        {/* 保持页面挂载，避免切换导航时丢失已授权根和扫描状态。 */}
        <div hidden={page !== 'home'}>
          <OrganizePage settingsState={settingsState} onPlanReady={openPreview} />
        </div>
        <div hidden={page !== 'preview'}>
          <PreviewPage key={plan?.id ?? 'empty'} planId={plan?.id ?? null} rootPath={plan?.rootPath ?? null} />
        </div>
        <div hidden={page !== 'history'}>
          {/* 历史页的「重新预览」就是回首页重新走一遍：
              执行过的计划不能再跑第二次，所以只给这一条出口。
              「去核对」则把需要人工核对的那条记录交给恢复页。 */}
          <HistoryPage
            active={page === 'history'}
            onRestart={() => setPage('home')}
            onInspectRecovery={openRecovery}
            onUndo={openUndo}
          />
        </div>
        <div hidden={page !== 'recovery'}>
          <RecoveryPage
            runId={recoveryRunId}
            onSettled={() => setRecoveryRunId(null)}
          />
        </div>
        <div hidden={page !== 'undo'}>
          <UndoPage
            runId={undoRunId}
            onDone={() => {
              setUndoRunId(null)
              setPage('history')
            }}
          />
        </div>
        <div hidden={page !== 'settings'}>
          <SettingsPage
            state={settingsState}
            ocrState={ocrState}
            providerState={providerState}
            onRetry={retry}
            onRetryOcr={retryOcr}
            onRetryProviders={reloadProviders}
            onProvidersChanged={reloadProviders}
          />
        </div>
      </main>
    </div>
  )
}

export default App
