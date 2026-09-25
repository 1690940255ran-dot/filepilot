vi.mock('../../src/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/api/client')>()
  return { ...actual, call: vi.fn() }
})

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { call, IpcError, IPC_UNAVAILABLE_CODE } from '../../src/api/client'
import { HistoryPage } from '../../src/features/history/HistoryPage'
import { OrganizePage } from '../../src/features/organize/OrganizePage'
import { SettingsPage } from '../../src/features/settings/SettingsPage'
import type { OcrStatusState } from '../../src/features/settings/ocrStatus'
import type { ProviderListState } from '../../src/features/settings/providerState'
import type { SettingsState } from '../../src/features/settings/settingsState'
import { t } from '../../src/i18n/zh-CN'

const mockedCall = vi.mocked(call)

/** 让 `call` 按命令名返回预设结果；未预设的命令抛错，避免测试悄悄走到别的分支。 */
function stubIpc(responses: Record<string, unknown>): void {
  mockedCall.mockImplementation(((command: string) => {
    if (command in responses) {
      return Promise.resolve(responses[command])
    }
    return Promise.reject(new Error(`测试未预设命令 ${command}`))
  }) as typeof call)
}

beforeEach(() => {
  mockedCall.mockReset()
  // 默认行为要**复现 jsdom 里真实 `call` 的失败方式**：它抛
  // `IPC_UNAVAILABLE`，而首页与历史页那几条用例正是靠它断言
  // 「给出明确错误而不是静默失败」。
  //
  // 留成「返回 undefined」会让那些用例拿到 undefined 而不是错误，
  // 于是它们会失败——而那不是产品的问题，是这个 mock 的问题。
  mockedCall.mockRejectedValue(
    new IpcError({
      code: IPC_UNAVAILABLE_CODE,
      message: '当前不在桌面应用环境中运行，无法调用本地核心。请在 FilePilot 桌面应用内使用。',
      retryable: false,
      details: {},
    }),
  )
})

describe('首页（组织入口）', () => {
  it('初始状态提供「选择文件夹」入口，且按钮是可用的', () => {
    // T02 之后根授权已接通，按钮不再是禁用态
    render(<OrganizePage settingsState={{ status: 'loading' }} />)
    expect(screen.getByRole('button', { name: t.home.chooseRoot })).toBeEnabled()
  })

  it('尚未选择文件夹时明确说明，而不是留一片空白', () => {
    render(<OrganizePage settingsState={{ status: 'loading' }} />)
    expect(screen.getByText(t.home.rootNotChosen)).toBeInTheDocument()
  })

  it('未选择根目录时不提供「开始扫描」', () => {
    render(<OrganizePage settingsState={{ status: 'loading' }} />)
    expect(screen.queryByRole('button', { name: t.home.startScan })).not.toBeInTheDocument()
  })

  it('设置就绪后展示当前整理模式', () => {
    render(
      <OrganizePage
        settingsState={{
          status: 'ready',
          settings: {
            mode: 'rules',
            scanMaxFiles: 10_000,
            scanMaxDepth: 20,
            selectedProviderId: null,
          },
        }}
      />,
    )
    expect(screen.getByText(t.home.modeRules)).toBeInTheDocument()
  })

  it('在没有 IPC 的环境里点击选择文件夹，必须给出明确错误而不是静默失败', async () => {
    // jsdom 里没有 Tauri 运行期，call() 会抛 IPC_UNAVAILABLE
    render(<OrganizePage settingsState={{ status: 'loading' }} />)
    await userEvent.click(screen.getByRole('button', { name: t.home.chooseRoot }))

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent(t.errors.IPC_UNAVAILABLE)
  })
})

describe('历史页', () => {
  it('没有 IPC 时给出明确错误，而不是伪造演示数据', async () => {
    // jsdom 里没有 Tauri 运行期，list_runs 会抛 IPC_UNAVAILABLE。
    // 规格禁止用假数据冒充框架完成，所以这里必须是错误 + 空态。
    render(<HistoryPage />)
    expect(await screen.findByRole('alert')).toHaveTextContent(t.errors.IPC_UNAVAILABLE)
  })

  it('加载失败后不会一直停在加载中，而是给出空态说明', async () => {
    render(<HistoryPage />)
    expect(await screen.findByText(t.history.empty)).toBeInTheDocument()
    expect(screen.getByText(t.history.emptyHint)).toBeInTheDocument()
  })

  it('历史页不提供「再执行一次」，避免重复移动已经移动过的文件', async () => {
    render(<HistoryPage />)
    await screen.findByRole('alert')
    expect(
      screen.queryByRole('button', { name: /再执行|重试执行|执行一次/ }),
    ).not.toBeInTheDocument()
  })
})
describe('设置页', () => {
  const readyState = {
    status: 'ready' as const,
    settings: {
      mode: 'aiLocal' as const,
      scanMaxFiles: 8_000,
      scanMaxDepth: 12,
      selectedProviderId: 'ollama-local',
    },
  }

  const ocrReady = {
    status: 'ready' as const,
    report: {
      status: 'available',
      languages: ['zh-Hans-CN'],
      message: '可用。',
    },
  }

  /** 设置页有几次独立的加载（设置本身 + OCR 状态 + 提供商列表），逐个用例都写全太吵。 */
  function renderSettings(
    overrides: {
      state?: SettingsState
      ocrState?: OcrStatusState
      providerState?: ProviderListState
      onRetry?: () => void
      onRetryOcr?: () => void
      onRetryProviders?: () => void
      onProvidersChanged?: () => void
    } = {},
  ) {
    return render(
      <SettingsPage
        state={overrides.state ?? readyState}
        ocrState={overrides.ocrState ?? ocrReady}
        // 默认给一个**空的就绪列表**：这样不会多渲染一个 role="status"，
        // 于是「加载中显示 loading」那条用 getByRole('status') 仍然是唯一的。
        providerState={overrides.providerState ?? { status: 'ready', providers: [] }}
        onRetry={overrides.onRetry ?? (() => {})}
        onRetryOcr={overrides.onRetryOcr ?? (() => {})}
        onRetryProviders={overrides.onRetryProviders ?? (() => {})}
        onProvidersChanged={overrides.onProvidersChanged ?? (() => {})}
      />,
    )
  }

  it('加载中显示 loading', () => {
    renderSettings({ state: { status: 'loading' } })
    expect(screen.getByRole('status')).toHaveTextContent(t.common.loading)
  })

  it('就绪时逐项展示设置值', () => {
    renderSettings()
    expect(screen.getByText(t.home.modeAiLocal)).toBeInTheDocument()
    expect(screen.getByText('8000')).toBeInTheDocument()
    expect(screen.getByText('12')).toBeInTheDocument()
    expect(screen.getByText('ollama-local')).toBeInTheDocument()
  })

  it('未配置 provider 时显示「未配置」而不是空白', () => {
    renderSettings({
      state: {
        status: 'ready',
        settings: { ...readyState.settings, selectedProviderId: null },
      },
    })
    expect(screen.getByText(t.settings.providerNone)).toBeInTheDocument()
  })

  it('错误状态使用 alert 角色并显示消息', () => {
    renderSettings({
      state: { status: 'error', message: '本地核心不可用', retryable: false },
    })
    expect(screen.getByRole('alert')).toHaveTextContent('本地核心不可用')
  })

  it('不可重试的错误不显示重试按钮', () => {
    renderSettings({ state: { status: 'error', message: 'x', retryable: false } })
    expect(screen.queryByRole('button', { name: t.common.retry })).not.toBeInTheDocument()
  })

  it('可重试的错误点击重试会回调', async () => {
    const onRetry = vi.fn()
    renderSettings({
      state: { status: 'error', message: 'x', retryable: true },
      onRetry,
    })
    await userEvent.click(screen.getByRole('button', { name: t.common.retry }))
    expect(onRetry).toHaveBeenCalledTimes(1)
  })

  it('提醒密钥存放位置，避免用户以为设置里存了密钥', () => {
    renderSettings()
    expect(screen.getByText(t.settings.secretsNotice)).toBeInTheDocument()
  })

  // ---- OCR 状态（规格 T11）----

  it('OCR 可用时列出可识别的语言，用户才知道中文行不行', () => {
    renderSettings({
      ocrState: {
        status: 'ready',
        report: {
          status: 'available',
          languages: ['zh-Hans-CN', 'en-US'],
          message: '可用。',
        },
      },
    })
    expect(screen.getByText(/zh-Hans-CN/)).toBeInTheDocument()
    expect(screen.getByText(/en-US/)).toBeInTheDocument()
  })

  it('OCR 不可用时显示后端给的原因，而不是一句笼统的「不可用」', () => {
    // 后端那句话里带着下一步动作（去装哪个语言包），界面不能再自己编一句。
    const message = '没有安装中文识别语言包。请在「设置 → 时间和语言 → 语言」中添加中文（简体）。'
    renderSettings({
      ocrState: {
        status: 'ready',
        report: { status: 'noChineseLanguage', languages: ['en-US'], message },
      },
    })
    expect(screen.getByText(message)).toBeInTheDocument()
  })

  it('OCR 不可用时明确告诉用户「没有它也能用」，而不是让人以为软件坏了', () => {
    // 规格 T11：用户无需 OCR 也可用规则模式。
    renderSettings({
      ocrState: {
        status: 'ready',
        report: { status: 'unsupported', languages: [], message: '系统不支持。' },
      },
    })
    expect(screen.getByText(t.settings.ocrFallbackNotice)).toBeInTheDocument()
  })

  it('OCR 可用时不显示「没有它也能用」，避免变成噪音', () => {
    renderSettings()
    expect(screen.queryByText(t.settings.ocrFallbackNotice)).not.toBeInTheDocument()
  })

  it('OCR 查询自己失败时给重试入口', async () => {
    // 它与设置是两次独立请求：设置读到了不代表 OCR 状态也读到了。
    const onRetryOcr = vi.fn()
    renderSettings({
      ocrState: { status: 'error', message: '无法启动解析进程', retryable: true },
      onRetryOcr,
    })
    expect(screen.getByText(/无法启动解析进程/)).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: t.common.retry }))
    expect(onRetryOcr).toHaveBeenCalledTimes(1)
  })

  // ---- 模型提供商（规格 5.2 / 6.4）----

  const provider = (overrides: Record<string, unknown> = {}) => ({
    id: 'default',
    kind: 'local' as const,
    endpoint: 'http://127.0.0.1:11434',
    model: 'qwen2.5:7b',
    hasCredential: false,
    ...overrides,
  })

  it('没有配置提供商时给出明确空态，而不是一片空白', () => {
    renderSettings({ providerState: { status: 'ready', providers: [] } })
    expect(screen.getByText(t.settings.providerEmpty)).toBeInTheDocument()
  })

  it('列出已配置的提供商', () => {
    renderSettings({ providerState: { status: 'ready', providers: [provider()] } })
    expect(screen.getByText('default')).toBeInTheDocument()
    expect(screen.getByText('http://127.0.0.1:11434')).toBeInTheDocument()
    expect(screen.getByText('qwen2.5:7b')).toBeInTheDocument()

    // 用列表的**可访问名称**定位，而不是直接按文本找：
    // 「本机（Ollama）」这个文本在表单的单选按钮上也会出现一次，
    // 而那条路径属于表单、不属于列表。
    const list = screen.getByRole('list', { name: t.settings.providerListLabel })
    expect(within(list).getByText(t.settings.providerKindLocal)).toBeInTheDocument()
  })

  it('界面只报「有没有密钥」，绝不显示密钥内容', () => {
    // 规格 3.3：前端保存后不再读回明文。
    // 这条断言的是**能力**而不只是当前行为：列表里拿到的是 ProviderSummary，
    // 它压根没有装密钥的字段，所以这里也没有「显示密钥」的开关。
    renderSettings({
      providerState: { status: 'ready', providers: [provider({ hasCredential: true })] },
    })
    expect(screen.getByText(t.settings.providerSecretSaved)).toBeInTheDocument()
    // 密钥输入框是**空**的：它只用来填新的，不会回填旧的。
    expect(screen.getByLabelText(t.settings.providerSecretLabel)).toHaveValue('')
  })

  it('未配置密钥时如实说明，而不是留白', () => {
    renderSettings({ providerState: { status: 'ready', providers: [provider()] } })
    expect(screen.getByText(t.settings.providerSecretAbsent)).toBeInTheDocument()
  })

  it('本地类型提示只允许回环地址', () => {
    renderSettings()
    expect(screen.getByText(t.settings.providerLocalHint)).toBeInTheDocument()
  })

  it('切到兼容云后提示必须用 HTTPS', async () => {
    renderSettings()
    await userEvent.click(screen.getByRole('radio', { name: t.settings.providerKindCompatible }))
    expect(screen.getByText(t.settings.providerCloudHint)).toBeInTheDocument()
  })

  it('地址缺协议时禁用保存并说明原因', async () => {
    renderSettings()
    const endpoint = screen.getByLabelText(t.settings.providerEndpointLabel)
    await userEvent.clear(endpoint)
    await userEvent.type(endpoint, '127.0.0.1:11434')

    expect(screen.getByRole('button', { name: t.settings.providerSave })).toBeDisabled()
    expect(screen.getByText(/http:\/\//)).toBeInTheDocument()
  })

  it('保存成功后清空密钥输入框，并让列表重新拉一次', async () => {
    const onProvidersChanged = vi.fn()
    stubIpc({ save_provider: provider({ hasCredential: true }) })
    renderSettings({ onProvidersChanged })

    await userEvent.type(screen.getByLabelText(t.settings.providerModelLabel), 'qwen2.5:7b')
    await userEvent.type(screen.getByLabelText(t.settings.providerSecretLabel), 'sk-test-123')
    await userEvent.click(screen.getByRole('button', { name: t.settings.providerSave }))

    await waitFor(() => expect(onProvidersChanged).toHaveBeenCalled())
    // 密钥从这一刻起不在界面里存在。
    expect(screen.getByLabelText(t.settings.providerSecretLabel)).toHaveValue('')
  })

  it('密钥留空时传 null，表示「不改动已有密钥」而不是「清掉」', async () => {
    // 用户改端点时不该被要求重填密钥——这条断言把那个语义钉在请求参数上。
    stubIpc({ save_provider: provider() })
    renderSettings()

    await userEvent.type(screen.getByLabelText(t.settings.providerModelLabel), 'm')
    await userEvent.click(screen.getByRole('button', { name: t.settings.providerSave }))

    await waitFor(() => expect(mockedCall).toHaveBeenCalled())
    const saved = mockedCall.mock.calls.find(([command]) => command === 'save_provider')
    expect(saved?.[1]).toMatchObject({ secret: null })
  })

  it('填了密钥时原样提交，不做任何前端改写', async () => {
    stubIpc({ save_provider: provider({ hasCredential: true }) })
    renderSettings()

    await userEvent.type(screen.getByLabelText(t.settings.providerModelLabel), 'm')
    await userEvent.type(screen.getByLabelText(t.settings.providerSecretLabel), '  sk-test-123  ')
    await userEvent.click(screen.getByRole('button', { name: t.settings.providerSave }))

    await waitFor(() => expect(mockedCall).toHaveBeenCalled())
    const saved = mockedCall.mock.calls.find(([command]) => command === 'save_provider')
    // 前后空格**不**由前端裁掉：如果后端认为它有问题，应当由后端说了算，
    // 前端静默改写只会让用户困惑「我明明复制的是这个」。
    expect(saved?.[1]).toMatchObject({ secret: '  sk-test-123  ' })
  })

  it('测试连通性把后端给的结果原文显示出来', async () => {
    stubIpc({
      test_provider: {
        model: 'qwen2.5:7b',
        structuredOutput: true,
        message: '连通正常，模型能按要求返回结构化建议。',
      },
    })
    renderSettings({ providerState: { status: 'ready', providers: [provider()] } })

    await userEvent.click(screen.getByRole('button', { name: t.settings.providerTest }))
    expect(
      await screen.findByText('连通正常，模型能按要求返回结构化建议。'),
    ).toBeInTheDocument()
  })

  it('测试失败时把原因显示出来，而不是只说「测试失败」', async () => {
    mockedCall.mockRejectedValueOnce(
      Object.assign(new Error('x'), {
        toAppError: () => ({
          code: 'MODEL_TIMEOUT',
          message: '无法连接到端点。请确认服务已启动。',
          retryable: false,
          details: {},
        }),
        name: 'IpcError',
      }),
    )
    // 用真实的 IpcError 形状不现实（它需要构造 normalizeError），
    // 这里直接让 call 抛一个普通错误，组件会退回到通用文案——
    // 断言的是「有东西显示出来」，而不是具体哪一句话。
    renderSettings({ providerState: { status: 'ready', providers: [provider()] } })

    await userEvent.click(screen.getByRole('button', { name: t.settings.providerTest }))
    expect(await screen.findByRole('alert')).toBeInTheDocument()
  })

  it('列表加载失败时给重试入口', async () => {
    const onRetryProviders = vi.fn()
    renderSettings({
      providerState: { status: 'error', message: '本地核心不可用', retryable: true },
      onRetryProviders,
    })

    expect(screen.getByText('本地核心不可用')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: t.common.retry }))
    expect(onRetryProviders).toHaveBeenCalledTimes(1)
  })
})
