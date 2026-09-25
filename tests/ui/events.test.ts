import { describe, expect, it } from 'vitest'

import { isTaskProgress, SequenceGuard } from '../../src/api/events'

const base = {
  taskId: 'task-1',
  seq: 1,
  status: 'running' as const,
  processed: 3,
  total: 10,
}

describe('任务进度事件载荷校验', () => {
  it('接受合法载荷', () => {
    expect(isTaskProgress(base)).toBe(true)
  })

  it('接受 total 为 null（总量尚未确定）', () => {
    expect(isTaskProgress({ ...base, total: null })).toBe(true)
  })

  it('拒绝缺少 taskId 的载荷', () => {
    const { taskId: _omitted, ...rest } = base
    expect(isTaskProgress(rest)).toBe(false)
  })

  it('拒绝 seq 不是有限数的载荷', () => {
    expect(isTaskProgress({ ...base, seq: Number.NaN })).toBe(false)
    expect(isTaskProgress({ ...base, seq: '1' })).toBe(false)
  })

  it('拒绝 total 既不是数字也不是 null', () => {
    expect(isTaskProgress({ ...base, total: '10' })).toBe(false)
  })

  it('拒绝非对象载荷', () => {
    expect(isTaskProgress(null)).toBe(false)
    expect(isTaskProgress('running')).toBe(false)
  })
})

describe('序号守卫', () => {
  it('接受首次出现的事件', () => {
    const guard = new SequenceGuard()
    expect(guard.accept(base)).toBe(true)
  })

  it('丢弃重复序号，避免同一进度被处理两次', () => {
    const guard = new SequenceGuard()
    guard.accept(base)
    expect(guard.accept(base)).toBe(false)
  })

  it('丢弃乱序到达的旧事件', () => {
    const guard = new SequenceGuard()
    guard.accept({ ...base, seq: 5 })
    expect(guard.accept({ ...base, seq: 3 })).toBe(false)
    expect(guard.accept({ ...base, seq: 6 })).toBe(true)
  })

  it('按 taskId 独立计数', () => {
    const guard = new SequenceGuard()
    guard.accept({ ...base, taskId: 'a', seq: 9 })
    expect(guard.accept({ ...base, taskId: 'b', seq: 1 })).toBe(true)
  })

  it('forget 之后可以从头接受，用于页面重开场景', () => {
    const guard = new SequenceGuard()
    guard.accept({ ...base, seq: 7 })
    guard.forget('task-1')
    expect(guard.accept({ ...base, seq: 1 })).toBe(true)
  })
})
