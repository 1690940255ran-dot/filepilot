import { describe, expect, it } from 'vitest'

import {
  call,
  decodeResponse,
  IpcError,
  IPC_UNAVAILABLE_CODE,
  isAppError,
  isTauriRuntime,
  normalizeError,
  TRANSPORT_ERROR_CODE,
} from '../../src/api/client'

describe('错误归一化', () => {
  it('原样保留后端返回的 AppError，并做一次拷贝', () => {
    const raw = {
      code: 'FILE_BUSY',
      message: '文件正被占用。',
      retryable: true,
      details: { hint: '关闭占用程序后重试' },
    }
    const normalized = normalizeError(raw)
    expect(normalized).toEqual(raw)
    expect(normalized.details).not.toBe(raw.details)
  })

  it('把裸 Error 视为传输层故障', () => {
    const normalized = normalizeError(new TypeError('invoke is not a function'))
    expect(normalized.code).toBe(TRANSPORT_ERROR_CODE)
    expect(normalized.message).toContain('invoke is not a function')
    expect(normalized.details.kind).toBe('TypeError')
    // 传输层故障不标记为可重试：Rust 没告诉我们它是不是瞬时故障
    expect(normalized.retryable).toBe(false)
  })

  it('把裸字符串视为传输层故障', () => {
    const normalized = normalizeError('something exploded')
    expect(normalized.code).toBe(TRANSPORT_ERROR_CODE)
    expect(normalized.message).toBe('something exploded')
  })

  it('对无法识别的载荷返回通用错误，而不是抛异常', () => {
    const normalized = normalizeError(undefined)
    expect(normalized.code).toBe(TRANSPORT_ERROR_CODE)
    expect(normalized.message.length).toBeGreaterThan(0)
    expect(normalized.details).toEqual({})
  })

  it('不会把只有部分字段的对象当合法 AppError', () => {
    expect(isAppError({ code: 'X', message: 'y' })).toBe(false)
    expect(isAppError({ code: 'X', message: 'y', retryable: false, details: null })).toBe(false)
    expect(isAppError({ code: 'X', message: 'y', retryable: false, details: {} })).toBe(true)
  })
})

describe('运行环境判断', () => {
  it('jsdom 里没有 Tauri 运行期，应被判为不可用', () => {
    expect(isTauriRuntime()).toBe(false)
  })

  it('非 Tauri 环境下 call 明确失败，而不是静默返回空数据', async () => {
    await expect(call('get_settings')).rejects.toBeInstanceOf(IpcError)

    await call('get_settings').catch((error: unknown) => {
      expect(error).toBeInstanceOf(IpcError)
      const ipcError = error as IpcError
      expect(ipcError.code).toBe(IPC_UNAVAILABLE_CODE)
      // 界面需要据此提示「请在桌面应用内运行」
      expect(ipcError.message).toContain('桌面应用')
      expect(ipcError.retryable).toBe(false)
    })
  })
})

describe('IpcError 与 AppError 的互转', () => {
  it('往返转换保持字段不变', () => {
    const original = {
      code: 'STALE_PLAN',
      message: '计划已被修改。',
      retryable: false,
      details: { expectedRevision: '2' },
    }
    const roundTripped = new IpcError(original).toAppError()
    expect(roundTripped).toEqual(original)
  })

  it('构造时复制 details，避免调用方改动共享对象', () => {
    const details = { a: '1' }
    const error = new IpcError({ code: 'X', message: 'y', retryable: false, details })
    details.a = 'mutated'
    expect(error.toAppError().details.a).toBe('1')
  })
})

describe('IPC 响应契约接线', () => {
  it('接受与命令匹配的合法数据', () => {
    expect(
      decodeResponse('get_settings', {
        ok: true,
        data: {
          mode: 'rules',
          scanMaxFiles: 10_000,
          scanMaxDepth: 20,
          selectedProviderId: null,
        },
      }),
    ).toMatchObject({ mode: 'rules' })
  })

  it('拒绝类型错误和未知字段，而不是相信 TypeScript 泛型', () => {
    expect(() =>
      decodeResponse('get_settings', {
        ok: true,
        data: {
          mode: 'rules',
          scanMaxFiles: '10000',
          scanMaxDepth: 20,
          selectedProviderId: null,
          secret: 'must-not-pass',
        },
      }),
    ).toThrow(IpcError)
  })

  it('拒绝畸形的错误信封', () => {
    expect(() =>
      decodeResponse('get_settings', {
        ok: false,
        error: { code: 'X', message: 'bad', retryable: false },
      }),
    ).toThrow(IpcError)
  })

  it('start_scan 只接受非空 taskId', () => {
    expect(decodeResponse<string>('start_scan', { ok: true, data: 'task-1' })).toBe('task-1')
    expect(() => decodeResponse('start_scan', { ok: true, data: '' })).toThrow(IpcError)
    expect(() => decodeResponse('start_scan', { ok: true, data: { taskId: 'x' } })).toThrow(
      IpcError,
    )
  })

  it('历史与恢复补跑接受契约正确的数组，并逐项拒绝坏数据', () => {
    expect(decodeResponse('list_runs', { ok: true, data: [] })).toEqual([])
    expect(decodeResponse('recover_pending_runs', { ok: true, data: [] })).toEqual([])
    expect(() => decodeResponse('list_runs', { ok: true, data: ['not-a-run'] })).toThrow(IpcError)
    expect(() =>
      decodeResponse('recover_pending_runs', { ok: true, data: [{ status: 'completed' }] }),
    ).toThrow(IpcError)
  })
})
