/**
 * e2e 用的契约数据。
 *
 * 单独成文件是为了能被 **vitest 直接校验**（见 `tests/ui/contract-fixtures.test.ts`）。
 * 把 fixtures 埋在 spec 里会得到一个很难查的失败模式：
 * 契约一改，`pnpm test:e2e` 里所有用例一起挂在某个 DOM 断言上，
 * 而真正的原因（某个字段少了一个）要看 Playwright 的 DOM 快照才能猜到。
 *
 * 字段必须与 `src/api/contracts.schema.json` **严格一致**：
 * 前端的运行时校验会拒绝任何缺字段或多字段的响应（规格 5.1）。
 */

/** 一根文件记录。扫描完成后首页用它统计「可整理」与「被跳过」。 */
function fileRecord(index: number, name: string) {
  return {
    id: `fid-${index}`,
    scanId: 'scan-1',
    rootId: 'root-1',
    relativePath: [name],
    extension: '.txt',
    fingerprint: {
      volumeId: '12345678',
      fileId: `fid-${index}`,
      size: '3',
      modifiedNs: '1700000000000000000',
      sha256: String.fromCharCode(96 + index).repeat(64),
    },
    extractionStatus: 'pending',
    // skipCode 为 null 才算「可整理」
    skipCode: null,
  }
}

/** 计划里的一项。 */
function planItem(index: number, name: string) {
  return {
    id: `item-${index}`,
    fileId: `fid-${index}`,
    source: [name],
    target: ['文档', name],
    action: 'move',
    selected: true,
    origin: 'rule',
    reason: '按类型',
    expected: {
      volumeId: '12345678',
      fileId: `fid-${index}`,
      size: '3',
      modifiedNs: '1700000000000000000',
      sha256: String.fromCharCode(96 + index).repeat(64),
    },
  }
}

const FILE_NAMES = ['a.txt', 'b.txt', 'c.txt']

export const E2E_FILES = FILE_NAMES.map((name, i) => fileRecord(i + 1, name))

export const FIXTURES = {
  settings: {
    mode: 'rules',
    scanMaxFiles: 10_000,
    scanMaxDepth: 20,
    selectedProviderId: null,
  },
  root: {
    rootId: 'root-1',
    displayPath: 'C:\\资料',
    volumeId: '12345678',
  },
  task: {
    taskId: 'task-scan',
    status: 'completed',
    processed: 3,
    total: 3,
    scanId: 'scan-1',
    error: null,
  },
  filePage: {
    items: E2E_FILES,
    nextCursor: null,
    total: FILE_NAMES.length,
  },
  plan: {
    id: 'plan-1',
    rootId: 'root-1',
    scanId: 'scan-1',
    revision: 1,
    mode: 'rules',
    status: 'draft',
    createdAt: '2026-09-17T00:00:00.000Z',
    items: FILE_NAMES.map((name, i) => planItem(i + 1, name)),
  },
  report: {
    planId: 'plan-1',
    revision: 1,
    digest: 'digest-1',
    executableCount: FILE_NAMES.length,
    issues: [],
    validationToken: 'token-1',
    // 必须是未来时间：过期后按钮本来就该禁用
    expiresAt: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  },
  executedTask: {
    taskId: 'task-exec',
    status: 'running',
    processed: 1,
    total: FILE_NAMES.length,
    scanId: null,
    error: null,
  },
}

/**
 * 命令 → 响应数据。
 *
 * 值直接是**信封里的 `data`**；信封本身由 mock 统一加，
 * 免得每处都写一遍 `{ ok: true, data }` 而漏掉一处。
 */
export function commandTable(): Record<string, unknown> {
  return {
    get_settings: FIXTURES.settings,
    choose_root: FIXTURES.root,
    // start_scan 的契约是「非空字符串 taskId」
    start_scan: 'task-scan',
    get_task: FIXTURES.task,
    list_files: FIXTURES.filePage,
    create_plan: { plan: FIXTURES.plan, issues: [] },
    get_plan: FIXTURES.plan,
    validate_plan: FIXTURES.report,
    cancel_task: FIXTURES.executedTask,
    // 这里**没有** execute_plan。
    //
    // 它需要是一个永不 resolve 的 Promise（界面停在「进行中」，测试才观察得到
    // 中间状态），而 Promise 无法通过 `addInitScript` 的参数传到页面里——
    // 传过去会变成 `{}`。所以它只在页面内构造，见 `installIpcMock`。
  }
}

/** 命令名 → 它应当通过的那个契约定义名。用于 fixtures 自检。 */
export const EXPECTED_CONTRACT: Record<string, string | null> = {
  get_settings: 'AppSettings',
  choose_root: 'RootSummary',
  get_task: 'TaskSummary',
  list_files: 'FilePage',
  create_plan: 'PlanBuild',
  get_plan: 'Plan',
  validate_plan: 'ValidationReport',
  cancel_task: 'TaskSummary',
}
