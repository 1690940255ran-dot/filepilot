import type { JSX } from 'react'

import type { Issue } from '../../api/contracts.generated'
import { t } from '../../i18n/zh-CN'

interface IssueListProps {
  issues: Issue[]
  /** 需要隐藏的项（例如当前被「仅看有问题的」筛掉的项）。 */
  hiddenItemIds?: ReadonlySet<string>
}

const SEVERITY_LABEL: Record<string, string> = {
  info: '提示',
  warning: '注意',
  block: '阻断',
}

/**
 * 问题列表。
 *
 * 阻断项排在最前：用户最需要先看到「什么让这次整理做不了」，
 * 而不是按后端返回顺序读一遍提示。
 */
export function IssueList({ issues, hiddenItemIds }: IssueListProps): JSX.Element {
  const visible = issues.filter(
    (issue) => issue.itemId === null || !hiddenItemIds?.has(issue.itemId),
  )

  if (visible.length === 0) {
    return <p className="notice">{t.preview.noIssues}</p>
  }

  const ordered = [...visible].sort((a, b) => rank(a.severity) - rank(b.severity))

  return (
    <ul className="issue-list">
      {ordered.map((issue) => (
        <li
          key={`${issue.code}:${issue.itemId ?? '*'}`}
          className={`issue issue-${issue.severity}`}
        >
          <span className={`issue-badge badge-${issue.severity}`}>
            {SEVERITY_LABEL[issue.severity] ?? issue.severity}
          </span>
          <span className="issue-message">{issue.message}</span>
          <span className="issue-scope">
            {issue.itemId === null ? t.preview.globalIssue : issue.itemId}
          </span>
        </li>
      ))}
    </ul>
  )
}

function rank(severity: string): number {
  if (severity === 'block') return 0
  if (severity === 'warning') return 1
  return 2
}
