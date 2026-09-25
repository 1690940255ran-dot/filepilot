/**
 * PR-001 / PR-002：展示层缺陷的**结构**回归。
 *
 * ## 为什么这一组测的是 CSS 文件而不是渲染结果
 *
 * jsdom 不实现 CSS 级联，更不算布局。任何「间距够不够」「数字对没对齐」的断言
 * 在 jsdom 里都是假的——它会绿，但绿得毫无意义，反而给人「已覆盖」的错觉。
 * 那种测试比没有测试更糟。
 *
 * 所以这里只盯**能被确定性判定**的那部分：
 *
 * 1. 组件用到的每个类名，样式表里**必须有对应规则**。
 *    PR-001 的真正根因就是这个——`.issue*` 与 `.modal*` 一个规则都没有，
 *    于是三个 `<span>` 按行内默认排布紧贴在一起。这不是「间距不够」，
 *    是整块样式缺失；而「样式表里少了规则」是可以用脚本确定性判定的。
 * 2. 对齐/间距相关的**关键声明**存在（gap 而非相邻 margin、tabular-nums 等）。
 *
 * 诚实边界：下面这些用例**不能**证明视觉上好看，只能证明「不会退回到
 * 完全没有样式」和「当初写下的关键声明还在」。真正的观感必须靠截图验收
 * （见 docs/CLEAN_MACHINE_ACCEPTANCE.md）。
 */

import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

const PROJECT = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const css = readFileSync(resolve(PROJECT, 'src/styles.css'), 'utf8')

/** 取某个类名的规则体；找不到返回 null。 */
function ruleBody(selector: string): string | null {
  // 逐字符扫描而不是正则：要正确处理嵌套大括号与注释，正则在这里很容易漏。
  let index = 0
  while (index < css.length) {
    const open = css.indexOf('{', index)
    if (open === -1) return null
    const close = matchingBrace(open)
    if (close === -1) return null
    const selectorText = stripComments(css.slice(index, open))
    // 精确按逗号分隔的选择器列表匹配，避免 `.issue` 命中 `.issue-list`
    const selectors = selectorText
      .split(',')
      .map((part) => part.trim())
      .filter((part) => part.length > 0)
    if (selectors.includes(selector)) {
      return css.slice(open + 1, close)
    }
    index = close + 1
  }
  return null
}

function matchingBrace(open: number): number {
  let depth = 0
  for (let i = open; i < css.length; i += 1) {
    if (css[i] === '{') depth += 1
    else if (css[i] === '}') {
      depth -= 1
      if (depth === 0) return i
    }
  }
  return -1
}

function stripComments(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, '')
}

/** 注释里提到某个类名不算「有样式」——必须真的有规则体。 */
function expectRule(selector: string): string {
  const body = ruleBody(selector)
  expect(body, `样式表里没有 ${selector} 的规则（PR-001 的根因就是这个）`).not.toBeNull()
  return body as string
}

describe('PR-001：问题列表的三个类名必须有真实规则', () => {
  /*
    组件里实际用到的类名（src/features/preview/IssueList.tsx:36-48）。
    这一个列表就是缺陷的完整范围：三个 span 各自有类名，
    但样式表里一条规则都没有，于是三者紧贴。
  */
  const REQUIRED = ['issue-list', 'issue', 'issue-badge', 'issue-message', 'issue-scope']

  it.each(REQUIRED)('.%s 有规则', (name) => {
    expect(ruleBody(`.${name}`)).not.toBeNull()
  })

  it('间距由 flex gap 提供，不依赖相邻元素的 margin', () => {
    /*
      这一条是**修法的核心**，不只是风格偏好：

      相邻 span 的 `margin-right` 在换行时会消失（换行后它是行尾，
      下一条元信息跑到下一行，两者之间的水平间距就没了）。
      `gap` 由容器统一给，换行与不换行都成立。

      原缺陷正是「三个 span 贴在一起」——用 gap 修掉才不会再回来。
    */
    const body = expectRule('.issue')
    expect(body).toMatch(/display:\s*flex/)
    expect(body, '必须用 gap，而不是靠子元素 margin').toMatch(/gap:\s*\d/)
  })

  it('作用域标注与正文在**形状上**可区分，不只靠颜色', () => {
    /*
      UUID 是机器标识，长相上就该和中文正文不一样。
      只改颜色是不够的：色弱用户区分不出，而且它仍会被读成正文的一部分。

      等宽字体 + 独立底色 + 独立边框，这三样任一都在提供「这不是正文」
      的形状信号；这里要求至少具备等宽与底色两项。
    */
    const body = expectRule('.issue-scope')
    expect(body, '等宽字体：让 UUID 看起来就不是中文正文').toMatch(/font-family:[^;]*monospace/)
    expect(body, '独立底色：形状上把它和正文分开').toMatch(/background:/)
  })

  it('三种严重度都有徽标样式', () => {
    // 组件用 `badge-${issue.severity}`，风险值是 info / warning / block
    for (const name of ['badge-info', 'badge-warning', 'badge-block']) {
      expect(ruleBody(`.${name}`), `.${name} 缺样式`.trim()).not.toBeNull()
    }
  })
})

describe('PR-001：对话框类名同样不能缺失', () => {
  /*
    同一类缺陷：`ConfirmDialog.tsx` 用了 .modal-backdrop / .modal / .modal-actions，
    这些此前也**完全没有规则**。没有 `.modal-backdrop` 的定位，
    对话框就不是浮层，而是插在页面流里的一个普通 div。
  */
  it('.modal-backdrop 是固定定位的浮层', () => {
    const body = expectRule('.modal-backdrop')
    expect(body).toMatch(/position:\s*fixed/)
    // 没有 inset 的话浮层只覆盖自身大小，遮罩形同虚设
    expect(body).toMatch(/inset:\s*0/)
  })

  it('.modal-actions 让按钮组有间距', () => {
    const body = expectRule('.modal-actions')
    expect(body).toMatch(/gap:\s*\d/)
  })

  it('.disabled-reasons 有列表样式', () => {
    /*
      「为什么不能确认」这组 <li> 此前也没有规则，于是带着默认列表符号挤在一起。
      规格 T14 要求禁用理由对用户**可见**——可见不只是「出现在 DOM 里」。
    */
    expect(ruleBody('.disabled-reasons')).not.toBeNull()
  })
})

describe('PR-004：进度面板的样式前提', () => {
  it('.progress-panel 存在且贴在视口底部', () => {
    /*
      952 个文件的场景下，把进度放在页面最下方等于没有反馈——
      用户看不到。`sticky` 是「长列表滚动时仍在视野内」的实现方式。
    */
    const body = expectRule('.progress-panel')
    expect(body).toMatch(/position:\s*sticky/)
    expect(body).toMatch(/bottom:\s*0/)
  })

  it('.progress-panel 用等宽数字，避免数值跳动时整行左右抖', () => {
    expect(expectRule('.progress-panel')).toMatch(/font-variant-numeric:\s*tabular-nums/)
  })
})

describe('PR-002：统计数字列对齐', () => {
  it('数值列右对齐且用等宽数字', () => {
    /*
      缺陷现象是「已移动 8」里的 8 明显右偏，与下面三行的 0 不在同一列。
      两个原因都要修：
      1. `text-align` 默认 left → 值贴在各自 dt 的右边缘，而 dt 宽度不同；
      2. 数字不是等宽 → 位数变化（8 → 10）时右边缘会漂。
    */
    const body = expectRule('.info-grid dd')
    expect(body, '右对齐：让所有值落在同一条右基准线上').toMatch(/text-align:\s*right/)
    expect(body, '等宽数字：位数变化时宽度不跳').toMatch(/font-variant-numeric:\s*tabular-nums/)
  })

  it('第一列宽度由布局统一决定，不是 auto', () => {
    /*
      `grid-template-columns` 用 `auto` 时每一行各算各的列宽，
      数值列就会左右错位——这正是 PR-002 的成因之一。
      必须是 max-content / 固定值 / minmax 这类**统一**的宽度。
    */
    const body = expectRule('.info-grid')
    const declared = /grid-template-columns:\s*([^;]+);/.exec(body)?.[1] ?? ''
    expect(declared).not.toBe('')
    expect(declared.trim().split(/\s+/)[0], '第一列不能是 auto').not.toMatch(/^auto$/)
  })
})
