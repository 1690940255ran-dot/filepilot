import type { JSX } from 'react'

import type {
  DisclosureItemPreview,
  DisclosurePreview,
  DisclosureTextStatus,
} from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'

/**
 * 「将要发送给模型的内容」的审阅界面（规格 6.4）。
 *
 * ## 这一页的全部意义是**如实**
 *
 * 用户在这里做的决定是「这些内容允不允许离开这台电脑」。所以它必须展示
 * **实际**会发出去的东西，而不是一个概括：文件名、字数、内容开头都是真的。
 *
 * 三处容易图省事的地方，都按「如实」处理了：
 *
 * 1. **被截断的正文要标出来**——规格原话是「文本裁剪不是隐私脱敏保证」，
 *    界面上不能让人以为剩下的内容不会发出去；
 * 2. **「没有正文」要区分四种原因**（本来没有 / 格式读不出 / 提取失败 /
 *    用户关掉了）。混成一句，用户会去检查一个完全正常的文件；
 * 3. **关掉正文不等于删掉文件**——文件名照发，界面要说清这一点。
 */

const STATUS_LABEL: Record<DisclosureTextStatus, string> = {
  present: t.ai.statusPresent,
  empty: t.ai.statusEmpty,
  unsupported: t.ai.statusUnsupported,
  failed: t.ai.statusFailed,
  excluded: t.ai.statusExcluded,
  pending: t.ai.statusEmpty,
}

/** 哪些状态按「用户自己关掉的」来显示（可点击切回来）。 */
function isExcluded(item: DisclosureItemPreview): boolean {
  return item.textStatus === 'excluded'
}

interface DisclosureReviewProps {
  preview: DisclosurePreview
  /** 正在准备下一次预览（切换正文开关时）——此时按钮要禁用。 */
  busy: boolean
  /** 已授权。 */
  granted: boolean
  canGrant: boolean
  canStart: boolean
  /** 本地模式：不需要授权，界面上要说清为什么。 */
  local: boolean
  onToggleExcluded: (fileId: string) => void
  onGrant: () => void
  onStart: () => void
}

export function DisclosureReview({
  preview,
  busy,
  granted,
  canGrant,
  canStart,
  local,
  onToggleExcluded,
  onGrant,
  onStart,
}: DisclosureReviewProps): JSX.Element {
  return (
    <section className="disclosure-review" aria-labelledby="disclosure-heading">
      <h3 className="section-heading" id="disclosure-heading">
        {t.ai.reviewHeading}
      </h3>
      <p className="page-description">{t.ai.reviewDescription}</p>

      <dl className="info-grid">
        <dt>{t.ai.reviewFiles}</dt>
        <dd>{preview.fileCount}</dd>
        <dt>{t.ai.reviewCharacters}</dt>
        {/* `characterCount` 在契约里是 bigint（Rust 的 u64），不能直接渲染。 */}
        <dd>{preview.characterCount.toString()}</dd>
        <dt>{t.ai.reviewModel}</dt>
        <dd>{preview.model}</dd>
        <dt>{t.ai.reviewInstruction}</dt>
        <dd className="path-text">{preview.instruction}</dd>
      </dl>

      <p className="notice">{local ? t.ai.localNotice : t.ai.cloudNotice}</p>

      <table className="disclosure-table">
        {/* 表格的说明与页面标题**不能是同一句话**：屏幕阅读器会把它读两遍，
            而测试里的 `getByText` 也会因为找到两个元素而失败。 */}
        <caption className="visually-hidden">{t.ai.reviewTableCaption}</caption>
        <thead>
          <tr>
            <th scope="col">{t.ai.columnFile}</th>
            <th scope="col">{t.ai.columnChars}</th>
            <th scope="col">{t.ai.columnExcerpt}</th>
            <th scope="col">{t.ai.columnAction}</th>
          </tr>
        </thead>
        <tbody>
          {preview.items.map((item) => (
            <tr key={item.fileId}>
              <td className="path-text">{item.fileName}</td>
              <td>
                {item.characterCount}
                {item.truncated && <span className="badge">{t.ai.textTruncated}</span>}
              </td>
              <td className="excerpt-cell">
                {item.excerpt === '' ? (
                  <span className="disabled-reason">{t.ai.emptyExcerpt}</span>
                ) : (
                  item.excerpt
                )}
              </td>
              <td>
                <button
                  type="button"
                  className="link-action"
                  disabled={busy || item.textStatus === 'pending'}
                  onClick={() => onToggleExcluded(item.fileId)}
                >
                  {isExcluded(item) ? t.ai.statusExcluded : t.ai.statusPresent}
                </button>
                <span className={`status-tag status-${item.textStatus}`}>
                  {STATUS_LABEL[item.textStatus]}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {/* 关掉正文只影响内容，文件名照发——不说清的话，用户会以为
          这个文件整个不会被提到。 */}
      <p className="disabled-reason">{t.ai.excludeHint}</p>

      <div className="action-row">
        {!local && (
          <button
            type="button"
            className="secondary-action"
            disabled={!canGrant || busy}
            onClick={onGrant}
          >
            {granted ? t.ai.grantedNotice : t.ai.grantAction}
          </button>
        )}
        <button
          type="button"
          className="primary-action"
          disabled={!canStart || busy}
          onClick={onStart}
        >
          {t.ai.startAction}
        </button>
      </div>

      {granted && (
        <p className="notice" role="status">
          {t.ai.grantedNotice}
        </p>
      )}
    </section>
  )
}
