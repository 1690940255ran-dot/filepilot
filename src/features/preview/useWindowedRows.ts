import { useCallback, useEffect, useState, type UIEvent } from 'react'

/**
 * 表格一行的像素高度。
 *
 * 它必须与 CSS 里 `.plan-table tbody tr` 的高度一致——见 `useWindowedRows`
 * 的文档里「为什么必须是常量」。
 */
export const PLAN_ROW_HEIGHT = 40

/**
 * 上下各多渲染几行。
 *
 * 必要性在于**滚动是离散的**：用户一拽就可能跨过好几行，而重渲染总比
 * 滚动事件晚一拍。没有 overscan 的话，快速滚动会露出空白行。
 */
const OVERSCAN = 8

export interface WindowedRows<T> {
  /** 当前该渲染的那一段。 */
  visible: T[]
  /** 这一段在整体里的起始下标。 */
  startIndex: number
  /** 绑到滚动容器上的回调 ref。 */
  attachContainer: (element: HTMLDivElement | null) => void
  /** 绑到滚动容器上的滚动处理。 */
  onScroll: (event: UIEvent<HTMLDivElement>) => void
  /** 上方占位的高度（px）。 */
  paddingTop: number
  /** 下方占位的高度（px）。 */
  paddingBottom: number
}

/**
 * 只渲染视口内的那几行（规格 T14：「虚拟列表支持 10,000 条记录」）。
 *
 * ## 为什么是窗口化而不是分页
 *
 * 规格说的是「**虚拟列表**」：用户看到的仍然是一张连续的表，可以一路滚下去。
 * 分页会把「这一页选中的项」变成一个新的状态要维护，而「**筛选和编辑不丢
 * 选中状态**」正是同一句话的下半句——多一个状态就多一处会丢的地方。
 *
 * 所以选中状态按 `itemId` 存在 reducer 里，窗口化只影响**渲染哪几行**：
 * 滚出去的行被卸载，但它的勾选与草稿留在状态里，滚回来时照旧。
 *
 * ## 行高为什么必须是常量
 *
 * 因为占位高度 = 行数 × 行高。行高变成「渲染后才知道」的话，占位高度就
 * 只能靠猜，而猜错的直接后果是**滚动条长度乱跳**——用户在滚动时最反感的
 * 就是这个。所以这里固定行高，并在 CSS 里用同一个值。
 *
 * ## 为什么用 `useState` 而不是 `useRef` 存容器
 *
 * 本项目启用了 React Compiler 的 `react-hooks/refs` 规则，而它的判定**很保守**：
 * 只要调用一个内部用过 `useRef` 的 hook，**返回值的每一个属性**都会被当成
 * 「在渲染期访问 ref」——连 `paddingTop` 这种纯数字也报。
 *
 * 把容器放进 state 就没有这个问题，而且语义上更准：容器**确实**参与渲染
 * （它的高度决定渲染哪几行）。代价是挂载时多一次渲染，而那一次恰好就是
 * 用来量高度的。
 */
export function useWindowedRows<T>(
  rows: T[],
  rowHeight: number = PLAN_ROW_HEIGHT,
): WindowedRows<T> {
  const [container, setContainer] = useState<HTMLDivElement | null>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(0)

  const attachContainer = useCallback((element: HTMLDivElement | null) => {
    setContainer(element)
  }, [])

  // 视口高度只能量。挂载时读一次，窗口尺寸变化时再读。
  useEffect(() => {
    if (container === null) return

    const measure = () => setViewportHeight(container.clientHeight)
    measure()

    // `ResizeObserver` 不可用时就退化成「只在挂载时量一次」：
    // 那种情况下窗口缩放后**多渲染**几行——多渲染是安全的，少渲染才会露白。
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(measure)
    observer.observe(container)
    return () => observer.disconnect()
  }, [container])

  const onScroll = useCallback((event: UIEvent<HTMLDivElement>) => {
    setScrollTop(event.currentTarget.scrollTop)
  }, [])

  // 视口还没量出来时先按「一屏多一点」渲染：宁可多渲染几行，也不要先闪空白。
  const effectiveViewport = viewportHeight > 0 ? viewportHeight : rowHeight * 20

  const first = Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN)
  const visibleCount = Math.ceil(effectiveViewport / rowHeight) + OVERSCAN * 2
  const end = Math.min(rows.length, first + visibleCount)

  return {
    visible: rows.slice(first, end),
    startIndex: first,
    attachContainer,
    onScroll,
    paddingTop: first * rowHeight,
    paddingBottom: Math.max(0, (rows.length - end) * rowHeight),
  }
}
