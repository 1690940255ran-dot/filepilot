/**
 * T14：计划表格的窗口化（规格：「虚拟列表支持 10,000 条记录，
 * **筛选和编辑不丢选中状态**」）。
 *
 * 这一份只测 hook 本身——它是个纯计算，不需要渲染整张表：
 *
 * 1. **只渲染视口内的行**：10,000 条时不能真的建 10,000 个节点；
 * 2. **占位高度守恒**：上占位 + 渲染的行 + 下占位 = 总数 × 行高。
 *    它错了的表现是**滚动条长度乱跳**，而那是最难从代码上看出来的一种错；
 * 3. **裁剪只影响渲染**：hook 不碰数据本身，所以滚出去的行不会被改动
 *    ——「不丢选中状态」这条要靠它加上「状态按 id 存」共同保证。
 */

import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import {
  PLAN_ROW_HEIGHT,
  useWindowedRows,
} from '../../src/features/preview/useWindowedRows'

const TEN_THOUSAND = Array.from({ length: 10_000 }, (_, index) => ({
  id: `item-${index}`,
}))

/** 造一个假的滚动事件——`currentTarget` 就是容器本身。 */
function scrollTo(element: HTMLDivElement, top: number): void {
  Object.defineProperty(element, 'scrollTop', { value: top, writable: true })
}

describe('计划表格的窗口化', () => {
  it('10,000 条时只渲染视口内的那一小段', () => {
    const { result } = renderHook(() => useWindowedRows(TEN_THOUSAND))

    // 视口还没量出来时按「一屏多一点」渲染，也远小于 10,000。
    expect(result.current.visible.length).toBeLessThan(100)
    expect(result.current.visible.length).toBeGreaterThan(0)
  })

  it('渲染的是开头那一段，而不是随便一段', () => {
    const { result } = renderHook(() => useWindowedRows(TEN_THOUSAND))

    expect(result.current.startIndex).toBe(0)
    expect(result.current.visible[0]?.id).toBe('item-0')
    // 开头没有上方占位。
    expect(result.current.paddingTop).toBe(0)
    // 下方占位是「剩下的那些行」的高度。
    expect(result.current.paddingBottom).toBe(
      (TEN_THOUSAND.length - result.current.visible.length) * PLAN_ROW_HEIGHT,
    )
  })

  it('占位高度守恒：上 + 渲染 + 下 = 总数 × 行高', () => {
    // 这条是窗口化最容易错的地方。它错了的表现是滚动条长度随滚动乱跳
    // ——用户能感觉到，但从代码上很难看出来。
    const { result } = renderHook(() => useWindowedRows(TEN_THOUSAND))

    const total =
      result.current.paddingTop +
      result.current.visible.length * PLAN_ROW_HEIGHT +
      result.current.paddingBottom
    expect(total).toBe(TEN_THOUSAND.length * PLAN_ROW_HEIGHT)
  })

  it('滚动之后仍然只渲染一小段，而且占位高度仍然守恒', () => {
    const { result } = renderHook(() => useWindowedRows(TEN_THOUSAND))

    // 容器挂不上（jsdom 里没有布局），所以直接驱动 `onScroll`：
    // 它读的是 `event.currentTarget.scrollTop`。
    const element = document.createElement('div')
    scrollTo(element, 500 * PLAN_ROW_HEIGHT)

    act(() => {
      result.current.onScroll({
        currentTarget: element,
      } as unknown as React.UIEvent<HTMLDivElement>)
    })

    expect(result.current.visible.length).toBeLessThan(100)
    expect(result.current.startIndex).toBeGreaterThan(400)
    // 滚过之后上方出现占位，下方仍然守恒。
    expect(result.current.paddingTop).toBeGreaterThan(0)
    const total =
      result.current.paddingTop +
      result.current.visible.length * PLAN_ROW_HEIGHT +
      result.current.paddingBottom
    expect(total).toBe(TEN_THOUSAND.length * PLAN_ROW_HEIGHT)
  })

  it('滚到最底部时下方占位归零，且不会越界', () => {
    const { result } = renderHook(() => useWindowedRows(TEN_THOUSAND))

    const element = document.createElement('div')
    // 滚到最后一行。
    scrollTo(element, TEN_THOUSAND.length * PLAN_ROW_HEIGHT)

    act(() => {
      result.current.onScroll({
        currentTarget: element,
      } as unknown as React.UIEvent<HTMLDivElement>)
    })

    expect(result.current.paddingBottom).toBe(0)
    // 最后一项必须在渲染出来的那一段里——否则底部会是一片空白。
    const last = result.current.visible[result.current.visible.length - 1]
    expect(last?.id).toBe('item-9999')
  })

  it('空列表不炸', () => {
    const { result } = renderHook(() => useWindowedRows([]))

    expect(result.current.visible).toEqual([])
    expect(result.current.paddingTop).toBe(0)
    expect(result.current.paddingBottom).toBe(0)
  })

  it('短列表（比视口还少）全部渲染——不该为了「窗口化」把内容藏起来', () => {
    const few = TEN_THOUSAND.slice(0, 5)
    const { result } = renderHook(() => useWindowedRows(few))

    expect(result.current.visible).toHaveLength(5)
    expect(result.current.paddingBottom).toBe(0)
  })

  it('裁剪不碰数据本身', () => {
    // 窗口化只决定「渲染哪几行」。选中与草稿按 itemId 存在 reducer 里，
    // 所以滚出去的行被卸载后状态仍在——这是「不丢选中状态」的一半。
    const rows = TEN_THOUSAND.slice(0, 200)
    const snapshot = rows.map((row) => row.id)
    const { result } = renderHook(() => useWindowedRows(rows))

    expect(result.current.visible).not.toHaveLength(rows.length)
    // 原始数组一个元素都没少、没变。
    expect(rows.map((row) => row.id)).toEqual(snapshot)
  })
})
