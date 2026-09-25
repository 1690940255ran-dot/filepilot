/**
 * T14：「**无网络启动不等待模型服务**；纯规则路径无需任何外网请求」。
 *
 * ## 为什么这条要写成测试
 *
 * 它现在是对的——`test_provider` 只在设置页里由用户**手动点击**触发。
 * 但「启动时不联网」这件事**没有任何东西挡着它被破坏**：将来谁在
 * `App.tsx` 里加一个 `useEffect` 顺手探一下提供商，界面就会在断网时
 * 卡住或报错，而规则模式本该是完全离线的。
 *
 * 那种回归**不会让任何现有测试变红**，只会让「断网还能用规则整理」
 * 这个承诺悄悄失效。所以这里把它钉成一条断言：**启动时发出去的命令
 * 集合里，不许有会联网的那些**。
 *
 * 「会联网的命令」是按名字列的，不是按「有没有 providerId」猜的——
 * 猜的那种写法会在新增命令时静默漏掉。
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { call } from '../../src/api/client'
import { App } from '../../src/App'

vi.mock('../../src/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/client')>()
  return { ...actual, call: vi.fn() }
})

const mockedCall = vi.mocked(call)

/**
 * 会发起网络请求的命令。
 *
 * 加新命令时**必须**同步检查这里：它漏了的话，这条测试就形同虚设。
 */
const NETWORK_COMMANDS = [
  'test_provider',
  'start_analysis',
  'preview_disclosure',
  'grant_disclosure',
]

const OFFLINE_SETTINGS = {
  mode: 'rules',
  scanMaxFiles: 10_000,
  scanMaxDepth: 20,
  selectedProviderId: null,
}

/** 记录每次调用的命令名。 */
function recordCalls(): string[] {
  const seen: string[] = []
  mockedCall.mockImplementation(((command: string) => {
    seen.push(command)
    switch (command) {
      case 'get_settings':
        return Promise.resolve(OFFLINE_SETTINGS)
      case 'recovery_status':
        return Promise.resolve({ blocked: false, blockedRuns: [] })
      case 'list_providers':
        return Promise.resolve([])
      case 'list_runs':
        return Promise.resolve([])
      case 'get_ocr_status':
        return Promise.resolve({ available: false, reason: '未安装', engine: null })
      default:
        return Promise.reject(new Error(`测试未预设命令 ${command}`))
    }
  }) as typeof call)
  return seen
}

describe('启动路径不需要网络', () => {
  it('启动时不调用任何会联网的命令', async () => {
    const seen = recordCalls()

    render(<App />)
    // 等启动时的几个本地查询落地。
    await screen.findByText(/规则/)

    const networked = seen.filter((command) => NETWORK_COMMANDS.includes(command))
    expect(
      networked,
      '断网时启动不该等模型服务；规则模式本该完全离线可用',
    ).toEqual([])
  })

  it('启动时只调用本地命令', async () => {
    // 上一条是「不许有什么」，这一条是「只允许有什么」——
    // 两个方向都要有，否则「什么都不调」也能通过第一条。
    const seen = recordCalls()

    render(<App />)
    await screen.findByText(/规则/)

    // 这几个都是**读本地状态**的命令：设置、恢复概况、历史、提供商列表。
    // 它们不发网络请求——提供商列表只是从数据库读配置，探测是另一条命令。
    const allowed = new Set(['get_settings', 'recovery_status', 'list_runs', 'list_providers'])
    const unexpected = seen.filter((command) => !allowed.has(command))
    expect(unexpected, '启动路径引入了新的命令，请确认它不发网络请求').toEqual([])
  })

  it('模型服务不可达时，启动仍然完成', async () => {
    // 「不等待模型服务」的可观测含义：即使提供商列表拉取失败，
    // 应用也要进入可用状态，而不是停在加载中。
    mockedCall.mockImplementation(((command: string) => {
      switch (command) {
        case 'get_settings':
          return Promise.resolve(OFFLINE_SETTINGS)
        case 'recovery_status':
          return Promise.resolve({ blocked: false, blockedRuns: [] })
        case 'list_providers':
          // 模拟「模型服务连不上」。
          return Promise.reject(new Error('ECONNREFUSED'))
        default:
          return Promise.reject(new Error(`测试未预设命令 ${command}`))
      }
    }) as typeof call)

    render(<App />)

    // 规则模式仍然在，界面可用。
    expect(await screen.findByText(/规则/)).toBeInTheDocument()
  })
})
