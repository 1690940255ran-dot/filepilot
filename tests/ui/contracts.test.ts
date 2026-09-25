import { describe, expect, it } from 'vitest'

import { check, CONTRACTS_SCHEMA_ID, validatorFor } from '../../src/api/validation'

/**
 * 契约运行时校验（规格 5.1：前端用 Zod 或等价运行时校验验证 IPC/模型边界）。
 *
 * 这些测试锁住的是「前端不会把不符合契约的东西当合法数据使用」。
 * 它们不验证 Rust 侧是否真的会返回这些形状，那属于 IPC 集成测试（T02 起）。
 */
describe('IPC 契约运行时校验', () => {
  it('schema 有稳定的 $id，前端据此定位定义', () => {
    expect(CONTRACTS_SCHEMA_ID).toBe('https://filepilot.local/contracts.schema.json')
  })

  it('接受合法的 AppSettings', () => {
    const outcome = check('AppSettings', {
      mode: 'rules',
      scanMaxFiles: 10_000,
      scanMaxDepth: 20,
      selectedProviderId: null,
    })
    expect(outcome.valid).toBe(true)
    expect(outcome.errors).toEqual([])
  })

  it('接受带 provider 的 AppSettings', () => {
    const outcome = check('AppSettings', {
      mode: 'aiLocal',
      scanMaxFiles: 5_000,
      scanMaxDepth: 10,
      selectedProviderId: 'ollama-local',
    })
    expect(outcome.valid).toBe(true)
  })

  it('拒绝附加的未知字段', () => {
    // 这是关键防线：如果密钥、正文之类的字段意外混进设置契约，
    // additionalProperties=false 会让它在开发期就暴露，而不是被静默接受。
    const outcome = check('AppSettings', {
      mode: 'rules',
      scanMaxFiles: 10_000,
      scanMaxDepth: 20,
      selectedProviderId: null,
      apiKey: 'sk-should-not-be-here',
    })
    expect(outcome.valid).toBe(false)
    expect(outcome.errors.join(' ')).toContain('additional')
  })

  it('拒绝契约之外的整理模式', () => {
    const outcome = check('AppSettings', {
      mode: 'autoMagic',
      scanMaxFiles: 10_000,
      scanMaxDepth: 20,
      selectedProviderId: null,
    })
    expect(outcome.valid).toBe(false)
  })

  it('拒绝用字符串冒充数值上限', () => {
    // 规格 5.1 要求文件大小与高精度时间戳用十进制字符串，
    // 但设置里的数量上限是普通 u32，前端不应接受字符串形式。
    const outcome = check('AppSettings', {
      mode: 'rules',
      scanMaxFiles: '10000',
      scanMaxDepth: 20,
      selectedProviderId: null,
    })
    expect(outcome.valid).toBe(false)
  })

  it('拒绝负数的扫描上限', () => {
    // schemars 只输出 `minimum: 0`（u32 的 format 标记不参与校验），
    // 所以这道约束完全靠 minimum 生效，值得单独钉住
    const outcome = check('AppSettings', {
      mode: 'rules',
      scanMaxFiles: -1,
      scanMaxDepth: 20,
      selectedProviderId: null,
    })
    expect(outcome.valid).toBe(false)
  })

  it('拒绝缺少必填字段的 AppSettings', () => {
    const outcome = check('AppSettings', { mode: 'rules' })
    expect(outcome.valid).toBe(false)
    expect(outcome.errors.length).toBeGreaterThan(0)
  })

  it('接受合法的 AppError', () => {
    const outcome = check('AppError', {
      code: 'FILE_BUSY',
      message: '文件正被其他程序占用。',
      retryable: true,
      details: {},
    })
    expect(outcome.valid).toBe(true)
  })

  it('拒绝 details 值不是字符串的 AppError', () => {
    const outcome = check('AppError', {
      code: 'INTERNAL',
      message: 'x',
      retryable: false,
      details: { nested: { fileText: '不该出现在错误明细里' } },
    })
    expect(outcome.valid).toBe(false)
  })

  it('拒绝缺少 retryable 的 AppError', () => {
    const outcome = check('AppError', { code: 'X', message: 'y', details: {} })
    expect(outcome.valid).toBe(false)
  })

  it('对不存在的定义名立即失败，而不是返回恒真校验器', () => {
    // @ts-expect-error 故意传一个不在生成物里的定义名，验证失败是显式的
    expect(() => validatorFor('NotAContract')).toThrowError(/NotAContract/)
  })
})
