/**
 * T13：AI 模式的前端流程（预览 → 授权 → 分析）。
 *
 * 重点盯四件事：
 *
 * 1. **云端模式在没有授权之前不发**（规格 6.4）——「内容会发送到提供商」
 *    这件事必须由用户明确确认过；
 * 2. **本地模式不需要授权**（规格：「本地模式 `consentId=null`」）——
 *    数据不出本机，再要求一次确认只是形式主义；
 * 3. **关闭正文要重新预览**——载荷摘要会随之改变，拿旧摘要去授权会被后端
 *    拒绝，而用户看到的是一句莫名其妙的「载荷已变化」；
 * 4. **预览失败要留在原地并说明原因**，而不是把界面卡在「准备中」。
 *
 * 有意**不测**「模型返回了什么建议」：那是后端的事，前端只负责把
 * `analysisId` 交给下一步。
 */

import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { call } from '../../src/api/client'
import type { DisclosurePreview } from '../../src/api/contracts.generated'
import { t } from '../../src/i18n/zh-CN'
import {
  useAiDisclosure,
  type PrepareInput,
} from '../../src/features/organize/useAiDisclosure'

vi.mock('../../src/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/client')>()
  return { ...actual, call: vi.fn() }
})

const mockedCall = vi.mocked(call)

/** 让 `call` 按命令名返回预设结果；未预设的命令抛错，避免测试悄悄走到别的分支。 */
function stubIpc(responses: Record<string, unknown>): void {
  mockedCall.mockImplementation(((command: string) => {
    if (command in responses) {
      const value = responses[command]
      if (value instanceof Error) return Promise.reject(value)
      return Promise.resolve(value)
    }
    return Promise.reject(new Error(`测试未预设命令 ${command}`))
  }) as typeof call)
}

/** 发出去的命令名，按调用顺序。 */
function commands(): string[] {
  return mockedCall.mock.calls.map((args) => String(args[0]))
}

function previewItem(fileId: string, overrides: Partial<DisclosurePreview['items'][number]> = {}) {
  return {
    fileId,
    fileName: `${fileId}.txt`,
    characterCount: 10,
    truncated: false,
    excerpt: '内容开头',
    textStatus: 'present' as const,
    ...overrides,
  }
}

function preview(overrides: Partial<DisclosurePreview> = {}): DisclosurePreview {
  return {
    payloadDigest: 'digest-1',
    providerId: 'p1',
    model: 'm1',
    instruction: '按主题分类',
    fileCount: 2,
    characterCount: BigInt(20),
    items: [previewItem('f1'), previewItem('f2')],
    ...overrides,
  }
}

const CLOUD_INPUT: PrepareInput = {
  scanId: 'scan-1',
  fileIds: ['f1', 'f2'],
  mode: 'aiCloud',
  providerId: 'p1',
  instruction: '按主题分类',
}

const LOCAL_INPUT: PrepareInput = { ...CLOUD_INPUT, mode: 'aiLocal' }

/**
 * 一个可以稍后再兑现的 promise。
 *
 * 竞态用例必须能精确控制「谁先回来」——用 `setTimeout` 猜顺序既慢又不可靠。
 */
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('AI 模式的授权流程', () => {
  it('云端模式在没有授权之前不会发出分析请求', async () => {
    stubIpc({ preview_disclosure: preview() })
    const { result } = renderHook(() => useAiDisclosure())

    await act(async () => {
      await result.current.prepare(CLOUD_INPUT)
    })

    // 预览完成，但还不能开始——必须先授权。
    expect(result.current.phase.status).toBe('ready')
    expect(result.current.canGrant).toBe(true)
    expect(result.current.canStart).toBe(false)

    await act(async () => {
      await result.current.start()
    })

    expect(commands()).not.toContain('start_analysis')
    expect(result.current.error?.message).toBe(t.ai.needGrant)
  })

  it('授权之后可以开始，并把 consentId 交给后端', async () => {
    stubIpc({
      preview_disclosure: preview(),
      grant_disclosure: { consentId: 'consent-1', payloadDigest: 'digest-1' },
      start_analysis: { taskId: 'task-1', analysisId: 'analysis-1' },
    })
    const { result } = renderHook(() => useAiDisclosure())

    await act(async () => {
      await result.current.prepare(CLOUD_INPUT)
    })
    await act(async () => {
      await result.current.grant()
    })

    expect(result.current.granted).toBe(true)
    expect(result.current.canStart).toBe(true)

    await act(async () => {
      await result.current.start()
    })

    expect(result.current.phase.status).toBe('running')

    const startCall = mockedCall.mock.calls.find((args) => args[0] === 'start_analysis')
    expect(startCall?.[1]).toMatchObject({
      consentId: 'consent-1',
      mode: 'aiCloud',
      instruction: '按主题分类',
    })
  })

  it('本地模式不需要授权', async () => {
    stubIpc({
      preview_disclosure: preview(),
      start_analysis: { taskId: 'task-1', analysisId: 'analysis-1' },
    })
    const { result } = renderHook(() => useAiDisclosure())

    await act(async () => {
      await result.current.prepare(LOCAL_INPUT)
    })

    // 本地模式一预览完就能开始。
    expect(result.current.canStart).toBe(true)
    expect(result.current.granted).toBe(false)

    await act(async () => {
      await result.current.start()
    })

    expect(result.current.phase.status).toBe('running')

    const startCall = mockedCall.mock.calls.find((args) => args[0] === 'start_analysis')
    expect(startCall?.[1]).toMatchObject({ consentId: null, mode: 'aiLocal' })
    // 本地模式不该去换授权。
    expect(commands()).not.toContain('grant_disclosure')
  })

  it('关闭某个文件的正文会带着新的排除集合重新预览', async () => {
    stubIpc({
      preview_disclosure: preview(),
      grant_disclosure: { consentId: 'consent-1', payloadDigest: 'digest-1' },
    })
    const { result } = renderHook(() => useAiDisclosure())

    await act(async () => {
      await result.current.prepare(CLOUD_INPUT)
    })
    await act(async () => {
      await result.current.grant()
    })
    expect(result.current.granted).toBe(true)

    await act(async () => {
      await result.current.toggleExcluded('f2')
    })

    // 载荷变了 → 旧授权作废 → 必须重新确认。
    expect(result.current.excluded).toEqual(['f2'])
    expect(result.current.granted).toBe(false)
    expect(result.current.canStart).toBe(false)

    const previewCalls = mockedCall.mock.calls.filter(
      (args) => args[0] === 'preview_disclosure',
    )
    expect(previewCalls).toHaveLength(2)
    expect(previewCalls[1]?.[1]).toMatchObject({ excludedFileIds: ['f2'] })
  })

  it('再次点击会把文件放回正文里', async () => {
    stubIpc({ preview_disclosure: preview() })
    const { result } = renderHook(() => useAiDisclosure())

    await act(async () => {
      await result.current.prepare(CLOUD_INPUT)
    })
    await act(async () => {
      await result.current.toggleExcluded('f2')
    })
    await act(async () => {
      await result.current.toggleExcluded('f2')
    })

    expect(result.current.excluded).toEqual([])
    const previewCalls = mockedCall.mock.calls.filter(
      (args) => args[0] === 'preview_disclosure',
    )
    expect(previewCalls.at(-1)?.[1]).toMatchObject({ excludedFileIds: [] })
  })

  it('预览失败时回到起点并保留原因', async () => {
    stubIpc({ preview_disclosure: new Error('后端说不行') })
    const { result } = renderHook(() => useAiDisclosure())

    await act(async () => {
      await result.current.prepare(CLOUD_INPUT)
    })

    expect(result.current.phase.status).toBe('idle')
    expect(result.current.error?.message).toBe('后端说不行')
    expect(result.current.preview).toBeNull()
  })

  it('重新准备会清掉上一次的授权', async () => {
    stubIpc({
      preview_disclosure: preview(),
      grant_disclosure: { consentId: 'consent-1', payloadDigest: 'digest-1' },
    })
    const { result } = renderHook(() => useAiDisclosure())

    await act(async () => {
      await result.current.prepare(CLOUD_INPUT)
    })
    await act(async () => {
      await result.current.grant()
    })
    expect(result.current.granted).toBe(true)

    // 用户改了要求——这是**新的**载荷，旧授权不能带走。
    await act(async () => {
      await result.current.prepare({ ...CLOUD_INPUT, instruction: '改成按时间排序' })
    })

    expect(result.current.granted).toBe(false)
    expect(result.current.canStart).toBe(false)
  })

  it('预览里如实带出每一种「没有正文」的原因', async () => {
    // 后端算出的 textStatus 必须原样传到界面：把它们混成一句
    // 「没有正文」，用户会去检查一个完全正常的文件。
    stubIpc({
      preview_disclosure: preview({
        items: [
          previewItem('f1'),
          previewItem('f2', { textStatus: 'failed', characterCount: 0, excerpt: '' }),
          previewItem('f3', { textStatus: 'excluded', characterCount: 0, excerpt: '' }),
        ],
        fileCount: 3,
      }),
    })
    const { result } = renderHook(() => useAiDisclosure())

    await act(async () => {
      await result.current.prepare(CLOUD_INPUT)
    })

    const statuses = result.current.preview?.items.map((entry) => entry.textStatus)
    expect(statuses).toEqual(['present', 'failed', 'excluded'])
  })

  // -------------------------------------------------------------------------
  // 旧请求返回（规格 T14：「忽略旧请求返回」）
  // -------------------------------------------------------------------------

  it('先发后回的旧预览不会覆盖新的那一份', async () => {
    // 用户快改两下要求，于是有两个 `preview_disclosure` 同时在飞，
    // 而**先发的后回**。它带回来的是旧要求对应的预览。
    //
    // 这个错误最坏的地方在于它看起来完全正常：有文件名、有字符数、
    // 有内容开头，只是全部对应上一次的输入。用户据此点「确认并授权」，
    // 授权的就是一份他以为已经改掉的载荷。
    const first = deferred<DisclosurePreview>()
    const second = deferred<DisclosurePreview>()

    let seen = 0
    mockedCall.mockImplementation(((command: string) => {
      if (command === 'preview_disclosure') {
        seen += 1
        return seen === 1 ? first.promise : second.promise
      }
      return Promise.reject(new Error(`测试未预设命令 ${command}`))
    }) as typeof call)

    const { result } = renderHook(() => useAiDisclosure())

    // 第一次请求发出去（还没有结果）。
    let pending: Promise<void>
    act(() => {
      pending = result.current.prepare(CLOUD_INPUT)
    })

    // 第二次：用户改了要求又发了一次，而且这次**先**回来。
    await act(async () => {
      const secondCall = result.current.prepare({
        ...CLOUD_INPUT,
        instruction: '改成按时间排序',
      })
      await act(async () => {
        second.resolve(preview({ instruction: '改成按时间排序' }))
      })
      await secondCall
    })

    expect(result.current.preview?.instruction).toBe('改成按时间排序')

    // 现在旧的（第一次的）才回来。它必须被丢掉。
    await act(async () => {
      first.resolve(preview({ instruction: '按主题分类' }))
      await pending
    })

    expect(
      result.current.preview?.instruction,
      '旧的预览不该盖掉新的：用户正以为自己看的是新要求对应的内容',
    ).toBe('改成按时间排序')
  })

  it('旧请求失败时也不该把已经就位的新预览打回起点', async () => {
    // 同上，只是旧的那次是**失败**的。它同样不该影响界面——
    // 否则用户会看到一句与自己刚做的操作毫无关系的错误。
    const first = deferred<DisclosurePreview>()
    const second = deferred<DisclosurePreview>()

    let seen = 0
    mockedCall.mockImplementation(((command: string) => {
      if (command === 'preview_disclosure') {
        seen += 1
        return seen === 1 ? first.promise : second.promise
      }
      return Promise.reject(new Error(`测试未预设命令 ${command}`))
    }) as typeof call)

    const { result } = renderHook(() => useAiDisclosure())

    let pending: Promise<void>
    act(() => {
      pending = result.current.prepare(CLOUD_INPUT)
    })

    await act(async () => {
      const secondCall = result.current.prepare({
        ...CLOUD_INPUT,
        instruction: '改成按时间排序',
      })
      await act(async () => {
        second.resolve(preview({ instruction: '改成按时间排序' }))
      })
      await secondCall
    })

    await act(async () => {
      first.reject(new Error('旧请求失败了'))
      await pending
    })

    expect(result.current.error).toBeNull()
    expect(result.current.phase.status).toBe('ready')
    expect(result.current.preview?.instruction).toBe('改成按时间排序')
  })

  it('reset 之后，在飞的旧请求不再落到界面上', async () => {
    const slow = deferred<DisclosurePreview>()
    stubIpc({ preview_disclosure: slow.promise })
    const { result } = renderHook(() => useAiDisclosure())

    let pending: Promise<void>
    act(() => {
      pending = result.current.prepare(CLOUD_INPUT)
    })

    act(() => {
      result.current.reset()
    })

    await act(async () => {
      slow.resolve(preview())
      await pending
    })

    expect(result.current.phase.status).toBe('idle')
    expect(result.current.preview).toBeNull()
  })
})
