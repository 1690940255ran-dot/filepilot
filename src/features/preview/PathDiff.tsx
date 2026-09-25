import type { JSX } from 'react'

import { joinPath } from './previewReducer'

interface PathDiffProps {
  source: string[]
  target: string[]
}

/**
 * 展示「当前位置 → 整理后」。
 *
 * 源与目标都是**组件数组**而不是拼接好的字符串：路径比较与展示都要按组件来，
 * 拼成字符串再比较会出现 `C:\Data` 与 `C:\Database` 那类前缀误判（规格 7.3）。
 */
export function PathDiff({ source, target }: PathDiffProps): JSX.Element {
  const same =
    source.length === target.length && source.every((part, i) => part === target[i])

  return (
    <span className="path-diff">
      <span className="path-text">{joinPath(source)}</span>
      {!same && (
        <>
          <span className="path-arrow" aria-hidden="true">
            →
          </span>
          <span className="path-text path-target">{joinPath(target)}</span>
        </>
      )}
    </span>
  )
}
