import { useCallback, useEffect, useState, type JSX } from 'react'

import { call, IpcError } from '../../api/client'
import type { PlanBuild, RuleKind, TaskSummary } from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'
import type { SettingsState } from '../settings/settingsState'
import { DisclosureReview } from './DisclosureReview'
import { useAiDisclosure } from './useAiDisclosure'
import { useOrganizeFlow } from './useOrganizeFlow'

const MODE_LABEL: Record<string, string> = {
  rules: t.home.modeRules,
  aiLocal: t.home.modeAiLocal,
  aiCloud: t.home.modeAiCloud,
}

interface OrganizePageProps {
  settingsState: SettingsState
  /** 计划生成成功时回调，由上层切到预览页。 */
  onPlanReady?: (planId: string, rootPath: string) => void
}

export function OrganizePage({ settingsState, onPlanReady }: OrganizePageProps): JSX.Element {
  const { phase, error, notice, chooseRoot, startScan, cancelScan } = useOrganizeFlow()
  const [building, setBuilding] = useState(false)
  const [buildError, setBuildError] = useState<string | null>(null)
  const [instruction, setInstruction] = useState('')
  const ai = useAiDisclosure()

  const settings = settingsState.status === 'ready' ? settingsState.settings : null
  const mode = settings?.mode ?? 'rules'
  const isAi = mode !== 'rules'

  const openPlan = useCallback(
    (build: PlanBuild, displayPath: string) => {
      onPlanReady?.(build.plan.id, displayPath)
    },
    [onPlanReady],
  )

  const buildPlan = useCallback(
    async (kind: RuleKind) => {
      if (phase.status !== 'scanned') return
      setBuilding(true)
      setBuildError(null)
      try {
        const build = await call<PlanBuild>('create_plan', {
          scanId: phase.summary.scanId,
          ruleKind: kind,
          // 规则模式没有分析结果；显式给 `null` 而不是省略字段——
          // 后端把「两个都给」和「都不给」都当作错误，省略会让它以为
          // 前端想要的是另一条路。
          analysisId: null,
        })
        openPlan(build, phase.root.displayPath)
      } catch (raw) {
        setBuildError(raw instanceof IpcError ? raw.message : t.common.unknownError)
      } finally {
        setBuilding(false)
      }
    },
    [phase, openPlan],
  )

  /** AI 模式：用分析结果生成计划。 */
  const buildFromAnalysis = useCallback(
    async (analysisId: string) => {
      if (phase.status !== 'scanned') return
      setBuilding(true)
      setBuildError(null)
      try {
        const build = await call<PlanBuild>('create_plan', {
          scanId: phase.summary.scanId,
          ruleKind: null,
          analysisId,
        })
        openPlan(build, phase.root.displayPath)
      } catch (raw) {
        setBuildError(raw instanceof IpcError ? raw.message : t.common.unknownError)
      } finally {
        setBuilding(false)
      }
    },
    [phase, openPlan],
  )

  // 先把 id 取出来：联合类型的收窄在**闭包内部**会失效，
  // 所以在 JSX 里读 `ai.phase.analysisId` 过不了类型检查。
  const completedAnalysisId = ai.phase.status === 'done' ? ai.phase.analysisId : null
  const runningTaskId = ai.phase.status === 'running' ? ai.phase.taskId : null
  const runningAnalysisId = ai.phase.status === 'running' ? ai.phase.analysisId : null

  // 分析在后台任务里跑。**必须有人盯着它结束**，否则界面会永远停在
  // 「正在处理…」，而「生成计划」按钮永远不会出现——这是接线时最容易
  // 漏掉的一步，因为它不报错，只是什么都不发生。
  useEffect(() => {
    if (runningTaskId === null || runningAnalysisId === null) return
    let cancelled = false

    void (async () => {
      for (;;) {
        const task = await call<TaskSummary | null>('get_task', { taskId: runningTaskId })
        if (cancelled) return
        if (task === null) {
          ai.markFailed(t.errors.scanTaskMissing)
          return
        }
        if (!['queued', 'running'].includes(task.status)) {
          if (task.status === 'failed' || task.status === 'recoveryRequired') {
            // 失败也要说出来。一直转圈和「失败了」是两种完全不同的处境，
            // 而用户从转圈里读不出后者。
            ai.markFailed(task.error?.message ?? t.errors.scanFailed)
          } else {
            ai.markDone(runningAnalysisId)
          }
          return
        }
        await new Promise((resolve) => window.setTimeout(resolve, 50))
      }
    })()

    return () => {
      cancelled = true
    }
    // 依赖两个**稳定**的回调而不是整个 `ai` 对象：`ai` 每次渲染都是新的，
    // 拿它当依赖会让这个 effect 每一帧重跑一次。
    //
    // 规则看不到 `markDone` / `markFailed` 是 `useCallback` 的产物，
    // 所以它认为 `ai` 被漏掉了——这里显式说明为什么。
    // eslint-disable-next-line react-hooks/exhaustive-deps -- 见上
  }, [runningTaskId, runningAnalysisId, ai.markDone, ai.markFailed])

  const prepareAi = useCallback(async () => {
    if (phase.status !== 'scanned' || settings === null) return
    await ai.prepare({
      scanId: phase.summary.scanId,
      fileIds: phase.usableFileIds,
      mode,
      providerId: settings.selectedProviderId ?? '',
      instruction,
    })
  }, [phase, settings, mode, instruction, ai])

  const busy = phase.status === 'choosing' || phase.status === 'scanning'
  // `'root' in phase` 才能让 TypeScript 正确收窄联合类型；
  // 用提前算出的 boolean（如 hasRoot）是收窄不了的。
  const currentRoot = 'root' in phase ? phase.root : null
  const canScan = phase.status === 'ready' || phase.status === 'scanned'

  return (
    <section className="page" aria-labelledby="home-heading">
      <h2 className="page-heading" id="home-heading">
        {t.home.heading}
      </h2>
      <p className="page-description">{t.home.description}</p>

      <div className="action-row">
        <button
          type="button"
          className="primary-action"
          onClick={() => void chooseRoot()}
          disabled={busy}
        >
          {busy && phase.status === 'choosing'
            ? t.common.loading
            : currentRoot
              ? t.home.changeRoot
              : t.home.chooseRoot}
        </button>

        {canScan && (
          <button
            type="button"
            className="secondary-action"
            onClick={() => void startScan()}
          >
            {phase.status === 'scanned' ? t.home.rescan : t.home.startScan}
          </button>
        )}

        {phase.status === 'scanning' && (
          <>
            <span className="loading" role="status">
              {t.home.scanning}
            </span>
            <button
              type="button"
              className="secondary-action"
              disabled={phase.taskId === null}
              onClick={() => void cancelScan()}
            >
              {t.home.cancelScan}
            </button>
          </>
        )}
      </div>

      {/* 已选根目录：展示路径，但前端**不会**把它回传给后端 */}
      {currentRoot ? (
        <dl className="info-grid">
          <dt>{t.home.rootChosen}</dt>
          <dd className="path-text">{currentRoot.displayPath}</dd>
        </dl>
      ) : (
        <p className="disabled-reason">{t.home.rootNotChosen}</p>
      )}

      {notice && (
        <p className="notice" role="status">
          {notice}
        </p>
      )}

      {error && (
        <div className="error-box" role="alert">
          <p>{error.message}</p>
        </div>
      )}

      {phase.status === 'scanned' && (
        <>
          <h3 className="section-heading">{t.home.summaryTitle}</h3>
          <dl className="info-grid">
            <dt>{t.home.summaryTotal}</dt>
            <dd>{phase.summary.total}</dd>
            <dt>{t.home.summaryUsable}</dt>
            <dd>{phase.summary.usable}</dd>
            <dt>{t.home.summarySkipped}</dt>
            <dd>{phase.summary.skipped}</dd>
          </dl>

          {/* 规格 6.1：达到上限必须明确要求缩小范围，不能说「扫描完成」 */}
          {phase.summary.truncated && (
            <div className="error-box" role="alert">
              <p>{t.home.truncatedWarning}</p>
            </div>
          )}

          {phase.summary.usable === 0 ? (
            <p className="notice">{t.home.noFilesHint}</p>
          ) : (
            <p className="notice">{t.home.skippedHint}</p>
          )}

          {phase.summary.usable > 0 && (
            <>
              {isAi ? (
                <>
                  <h3 className="section-heading">{t.ai.heading}</h3>
                  <label className="field-label" htmlFor="ai-instruction">
                    {t.ai.instructionLabel}
                  </label>
                  <textarea
                    id="ai-instruction"
                    className="instruction-input"
                    value={instruction}
                    placeholder={t.ai.instructionPlaceholder}
                    // 规格 6.4：超过上限由后端拒绝而不是截断。前端也限一下，
                    // 但**不是因为**前端该拦——而是让用户不必先提交才知道超了。
                    maxLength={1000}
                    rows={3}
                    onChange={(event) => setInstruction(event.target.value)}
                  />
                  <p className="disabled-reason">{t.ai.instructionHint}</p>

                  <div className="action-row">
                    <button
                      type="button"
                      className="primary-action"
                      disabled={ai.phase.status === 'preparing'}
                      onClick={() => void prepareAi()}
                    >
                      {ai.phase.status === 'preparing'
                        ? t.ai.preparing
                        : t.ai.previewAction}
                    </button>
                  </div>

                  {/* 规则模式随时可用：AI 不可用不该把用户堵在这里。 */}
                  <div className="action-row">
                    <button
                      type="button"
                      className="secondary-action"
                      onClick={() => void buildPlan('byType')}
                      disabled={building}
                    >
                      {building ? t.home.building : t.home.buildPlan}
                    </button>
                  </div>
                </>
              ) : (
                <>
                  <div className="action-row">
                    <button
                      type="button"
                      className="primary-action"
                      onClick={() => void buildPlan('byType')}
                      disabled={building}
                    >
                      {building ? t.home.building : t.home.buildPlan}
                    </button>
                  </div>
                  <p className="disabled-reason">{t.home.buildPlanNotice}</p>
                </>
              )}

              {ai.error && (
                <div className="error-box" role="alert">
                  <p>{ai.error.message}</p>
                </div>
              )}

              {ai.preview && ai.phase.status !== 'running' && (
                <DisclosureReview
                  preview={ai.preview}
                  busy={ai.phase.status === 'preparing' || ai.phase.status === 'starting'}
                  granted={ai.granted}
                  canGrant={ai.canGrant}
                  canStart={ai.canStart}
                  local={mode === 'aiLocal'}
                  onToggleExcluded={(fileId) => void ai.toggleExcluded(fileId)}
                  onGrant={() => void ai.grant()}
                  onStart={() => void ai.start()}
                />
              )}

              {ai.phase.status === 'running' && (
                <p className="notice" role="status">
                  {t.ai.running}
                </p>
              )}

              {completedAnalysisId !== null && (
                <div className="action-row">
                  <button
                    type="button"
                    className="primary-action"
                    onClick={() => void buildFromAnalysis(completedAnalysisId)}
                    disabled={building}
                  >
                    {building ? t.home.building : t.ai.buildFromAnalysis}
                  </button>
                </div>
              )}
            </>
          )}

          {buildError && (
            <div className="error-box" role="alert">
              <p>{buildError}</p>
            </div>
          )}
        </>
      )}

      <dl className="info-grid">
        <dt>{t.home.modeHeading}</dt>
        <dd>
          {settingsState.status === 'ready'
            ? (MODE_LABEL[settingsState.settings.mode] ?? settingsState.settings.mode)
            : t.common.loading}
        </dd>
      </dl>
    </section>
  )
}
