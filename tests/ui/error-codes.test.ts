import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { isKnownErrorCode } from '../../src/api/client'

/**
 * 前端「已知错误码」集合与 Rust 真源的一致性。
 *
 * ## 为什么需要一条跨语言的测试
 *
 * `src/api/client.ts` 里的 `KNOWN_ERROR_CODES` 是**手写**的，而 Rust 的
 * `domain/errors.rs` 是唯一的真源（ADR-006）。两者之间原先没有任何自动比对，
 * 结果是这个缺口**真实发生过两次**：
 *
 * * T11 新增 `OCR_UNAVAILABLE` 时漏了同步；
 * * T12 新增 `CREDENTIAL_UNAVAILABLE` / `INVALID_ENDPOINT` 时也漏了。
 *
 * 漏掉的后果不是崩溃，而是**界面把一句有下一步动作的提示降级成「未知错误」**
 * ——用户看到「出现未知错误」，而实际上后端说的可能是「去凭据管理器看看」。
 *
 * 这条测试直接读 Rust 源码来比对。跨语言读取不是最优雅的做法，
 * 但它守的是一个**已经发生过两次**的真实缺陷，而代价只有一次文件读取。
 */
describe('错误码：前端已知集合必须与 Rust 真源一致', () => {
  function rustCodes(): Set<string> {
    const root = process.cwd()
    const source = readFileSync(join(root, 'src-tauri', 'src', 'domain', 'errors.rs'), 'utf-8')
    const block = source.slice(source.indexOf('pub mod codes'))
    return new Set(
      [...block.matchAll(/pub const [A-Z_0-9]+: &str = "([A-Z_0-9]+)"/g)].map(
        (match) => match[1] as string,
      ),
    )
  }

  it('Rust 里每一个码，前端都认识', () => {
    const rust = rustCodes()
    // 前置检查：真的读到了东西。否则一个空集合会让这条测试永远通过。
    expect(rust.size).toBeGreaterThan(20)

    const unknown = [...rust].filter((code) => !isKnownErrorCode(code)).sort()
    expect(
      unknown,
      `这些码后端会产生、而前端会当成「未知错误」展示：${unknown.join(', ')}`,
    ).toEqual([])
  })

  it('读到的确实是 Rust 的码表，不是别的东西', () => {
    // 防止正则或路径写错之后这条测试变成「永远绿」。
    const rust = rustCodes()
    for (const known of ['MODEL_AUTH', 'INTERNAL', 'TASK_BUSY']) {
      expect(rust.has(known), `码表里应当有 ${known}`).toBe(true)
    }
  })
})
