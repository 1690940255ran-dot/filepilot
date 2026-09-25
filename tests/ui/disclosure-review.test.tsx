/**
 * T13：待发送内容审阅界面的渲染。
 *
 * 这一页用户做的决定是「这些内容允不允许离开这台电脑」，所以测试盯的是
 * **如实**：被截断的要标出来、四种「没有正文」不能混成一句、关掉正文之后
 * 要说明文件名仍会发送。
 *
 * 有意**不测**按钮的点击后行为：那是 `useAiDisclosure` 的职责，
 * 已经在 `ai-disclosure.test.tsx` 里盯过了。
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import type { DisclosureItemPreview, DisclosurePreview } from '../../src/api/contracts.generated'
import { t } from '../../src/i18n/zh-CN'
import { DisclosureReview } from '../../src/features/organize/DisclosureReview'

function item(overrides: Partial<DisclosureItemPreview> = {}): DisclosureItemPreview {
  return {
    fileId: 'f1',
    fileName: '报告.txt',
    characterCount: 12,
    truncated: false,
    excerpt: '本季度营收增长',
    textStatus: 'present',
    ...overrides,
  }
}

function preview(items: DisclosureItemPreview[]): DisclosurePreview {
  return {
    payloadDigest: 'digest-1',
    providerId: 'p1',
    model: 'test-model',
    instruction: '按主题分类',
    fileCount: items.length,
    characterCount: BigInt(24),
    items,
  }
}

function renderReview(
  items: DisclosureItemPreview[],
  overrides: Partial<Parameters<typeof DisclosureReview>[0]> = {},
) {
  const props = {
    preview: preview(items),
    busy: false,
    granted: false,
    canGrant: true,
    canStart: false,
    local: false,
    onToggleExcluded: vi.fn(),
    onGrant: vi.fn(),
    onStart: vi.fn(),
    ...overrides,
  }
  render(<DisclosureReview {...props} />)
  return props
}

describe('待发送内容审阅', () => {
  it('展示实际会发出去的文件名、字数与内容开头', () => {
    renderReview([item()])

    expect(screen.getByText('报告.txt')).toBeDefined()
    expect(screen.getByText('本季度营收增长')).toBeDefined()
    // `characterCount` 是 bigint（Rust 的 u64），渲染前必须转成字符串，
    // 否则 React 会直接抛错。
    expect(screen.getByText('24')).toBeDefined()
    expect(screen.getByText('test-model')).toBeDefined()
    expect(screen.getByText('按主题分类')).toBeDefined()
  })

  it('标出被截断的正文', () => {
    // 规格原话：「文本裁剪不是隐私脱敏保证」。界面不能让人以为
    // 剩下的内容不会发出去。
    renderReview([item({ truncated: true })])
    expect(screen.getByText(t.ai.textTruncated)).toBeDefined()
  })

  it('把四种「没有正文」分开显示', () => {
    // 混成一句「没有正文」，用户会去检查一个完全正常的文件，
    // 或者白白放弃一个其实能用的文件。
    renderReview([
      item({ fileId: 'f1', fileName: 'a.txt', textStatus: 'present' }),
      item({
        fileId: 'f2',
        fileName: 'b.png',
        textStatus: 'empty',
        characterCount: 0,
        excerpt: '',
      }),
      item({
        fileId: 'f3',
        fileName: 'c.pdf',
        textStatus: 'unsupported',
        characterCount: 0,
        excerpt: '',
      }),
      item({
        fileId: 'f4',
        fileName: 'd.docx',
        textStatus: 'failed',
        characterCount: 0,
        excerpt: '',
      }),
    ])

    expect(screen.getByText(t.ai.statusEmpty)).toBeDefined()
    expect(screen.getByText(t.ai.statusUnsupported)).toBeDefined()
    // 「提取失败」是最需要用户知道的一种，必须有自己的一句话。
    expect(screen.getByText(t.ai.statusFailed)).toBeDefined()
  })

  it('说明关掉正文之后文件名仍会发送', () => {
    renderReview([item({ textStatus: 'excluded', characterCount: 0, excerpt: '' })])

    expect(screen.getByText(t.ai.excludeHint)).toBeDefined()
    // 文件名照常出现在表格里——它不是被删掉了。
    expect(screen.getByText('报告.txt')).toBeDefined()
  })

  it('本地模式不显示授权按钮，并说明原因', () => {
    renderReview([item()], { local: true, canStart: true })

    expect(screen.queryByText(t.ai.grantAction)).toBeNull()
    expect(screen.getByText(t.ai.localNotice)).toBeDefined()
    // 本地模式一预览完就能开始。
    const start = screen.getByText(t.ai.startAction) as HTMLButtonElement
    expect(start.disabled).toBe(false)
  })

  it('云端模式在未授权时禁用开始按钮', () => {
    renderReview([item()], { canStart: false, canGrant: true })

    expect(screen.getByText(t.ai.cloudNotice)).toBeDefined()
    const start = screen.getByText(t.ai.startAction) as HTMLButtonElement
    expect(start.disabled).toBe(true)
  })
})
