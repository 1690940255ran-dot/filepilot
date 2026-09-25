import type { JSX } from 'react'

import { t } from '../../i18n/zh-CN'
import type { ConfirmationView } from './previewReducer'

interface ConfirmDialogProps {
  open: boolean
  /** 真实根目录路径，用于最终确认时让用户看清整理的是哪里。 */
  rootPath: string | null
  /** 参与执行的项数（只看**选中**的项）。 */
  selectedCount: number
  confirmation: ConfirmationView
  /** 当前时间，由调用方传入，便于测试控制过期。 */
  nowMs: number
  onCancel: () => void
  onConfirm: () => void
}

/**
 * 最终确认对话框。
 *
 * 规格 7.4：确认时必须显示**真实根目录与数量**——用户确认的是「整理这个目录里的
 * 这些文件」，只显示一个「确认」按钮等于让用户在不知道对象的情况下授权。
 *
 * 按钮的禁用理由始终可见：一个灰掉却不说明原因的按钮只会让人反复点击。
 */
export function ConfirmDialog({
  open,
  rootPath,
  selectedCount,
  confirmation,
  nowMs,
  onCancel,
  onConfirm,
}: ConfirmDialogProps): JSX.Element | null {
  if (!open) return null

  const reasons = disabledReasons(confirmation, selectedCount, nowMs)
  const disabled = reasons.length > 0

  return (
    <div className="modal-backdrop">
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="confirm-heading">
        <h3 className="section-heading" id="confirm-heading">
          {t.preview.confirmHeading}
        </h3>

        <p className="confirm-summary">
          {t.preview.confirmSummaryPrefix}
          <strong className="confirm-count">{selectedCount}</strong>
          {t.preview.confirmSummaryUnit}
          <span className="path-text">{rootPath ?? '—'}</span>
        </p>

        <p className="confirm-irreversible">{t.preview.confirmIrreversible}</p>

        {disabled && (
          <ul className="disabled-reasons" role="status">
            {reasons.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        )}

        <div className="modal-actions">
          <button type="button" className="secondary-action" onClick={onCancel}>
            {t.preview.cancelEdit}
          </button>
          <button
            type="button"
            className="primary-action"
            onClick={onConfirm}
            disabled={disabled}
          >
            {t.preview.confirmInDialog}
          </button>
        </div>
      </div>
    </div>
  )
}

/**
 * 列出「为什么现在不能确认」。
 *
 * 与 `isConfirmationCurrent` 的五个条件一一对应：能确认时列表为空。
 * 分开写是因为布尔值判不了因果，而用户需要的是原因。
 *
 * 三种「没令牌」要分开说：**还没点校验**、**改过东西**、**已过期**——
 * 用户要做的事完全不同（去点校验 / 重新校验 / 重新校验）。
 */
// 该纯函数与对话框共享同一组禁用规则，并被单元测试直接验证。
// eslint-disable-next-line react-refresh/only-export-components
export function disabledReasons(
  confirmation: ConfirmationView,
  selectedCount: number,
  nowMs: number,
): string[] {
  const reasons: string[] = []

  if (selectedCount === 0) {
    reasons.push(t.preview.nothingSelected)
  }
  if (confirmation.hasBlockingIssues) {
    reasons.push(t.preview.blockedCannotConfirm)
  }

  // 计划从 revision 1 开始（`planner/build.rs`），所以 reportRevision 为 0
  // 只可能是「这份计划**从未**校验过」。必须把两种情况分开：
  //   - 从未校验过 → 让用户去点校验
  //   - 校验过、之后又被改（编辑只清令牌，版本留着）→ 才说「计划已被修改」
  //
  // 不分开的后果不是崩溃，而是**两句都不成立的话**：全新计划的按钮旁边写着
  // 「计划已被修改」（用户什么都没改），以及「存在阻断项」
  // （校验还没跑过，阻断项是未知，不是存在）。
  const neverValidated = confirmation.reportRevision === 0
  const revisionMismatch = confirmation.planRevision !== confirmation.reportRevision

  if (confirmation.token === null) {
    reasons.push(neverValidated ? t.preview.needValidate : t.preview.staleAfterEdit)
  } else if (confirmation.expiresAtMs <= nowMs) {
    reasons.push(t.preview.tokenExpired)
  }

  // 「一项都不可执行」只有在校验结果**确实对应当前计划**时才成立；
  // 否则 executableCount 的 0 含义是「还不知道」——界面也正是这么显示（「—」）。
  if (
    !neverValidated &&
    !revisionMismatch &&
    !confirmation.hasBlockingIssues &&
    confirmation.executableCount === 0
  ) {
    reasons.push(t.preview.blockedCannotConfirm)
  }

  return [...new Set(reasons)]
}
