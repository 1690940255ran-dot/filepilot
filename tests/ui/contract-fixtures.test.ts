import { describe, expect, it } from 'vitest'

import { check, type ContractDefinitionName } from '../../src/api/validation'
import { commandTable, EXPECTED_CONTRACT } from '../e2e/fixtures'

/**
 * e2e fixtures 必须逐个通过契约校验。
 *
 * 这条测试防的是一种很难查的失败：契约改一个字段，`pnpm test:e2e` 里所有用例
 * 一起挂在某个 DOM 断言上，而真正的原因要看 Playwright 的 DOM 快照才能猜到。
 * 在这里先校验一遍，失败信息会直接指出是哪个命令的哪个字段。
 */
describe('e2e fixtures 与契约', () => {
  const table = commandTable()

  for (const [command, definition] of Object.entries(EXPECTED_CONTRACT)) {
    it(`${command} 的响应符合 ${definition ?? '(字符串)'}`, () => {
      const data = table[command]

      if (definition === null) {
        // `start_scan` 这类命令的契约是「非空字符串」
        expect(typeof data).toBe('string')
        expect((data as string).length).toBeGreaterThan(0)
        return
      }

      const outcome = check(definition as ContractDefinitionName, data)
      expect(
        outcome.errors,
        `${command} 的 fixture 不符合契约：${outcome.errors.join('; ')}`,
      ).toEqual([])
    })
  }

  it('文件数与计划项数一致，首页才能算出可整理数量', () => {
    // 首页扫描完成后会用 list_files 统计 usable；若这里为 0，
    // 「生成整理建议」按钮根本不会出现，e2e 会卡在一个看起来毫不相关的地方。
    const page = table.list_files as { items: unknown[]; nextCursor: null; total: number }
    expect(page.items.length).toBeGreaterThan(0)
    expect(page.total).toBe(page.items.length)
    expect(page.nextCursor).toBeNull()

    const build = table.create_plan as { plan: { items: unknown[] } }
    expect(build.plan.items.length).toBe(page.items.length)
  })
})
