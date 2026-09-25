import contractsSchema from './contracts.schema.json'
import * as precompiled from './validators.generated'

/**
 * IPC / 模型边界的运行时校验。
 *
 * 规格 5.1：Rust 类型是唯一真源，前端用 Zod **或等价**运行时校验。
 * 这里选 Ajv 直接消费生成出来的 JSON Schema（见 ADR-006），
 * 而不是手写第二套 Zod schema —— 后者正是规格明令禁止的「两套同义但不兼容的数据结构」。
 *
 * ## 校验器是**预编译**的（2026-09-24 改）
 *
 * 原先这里在模块顶层 `new Ajv()` 并按需编译 schema。Ajv 编译要走
 * `new Function()`，而生产 CSP 是 `script-src 'self'`（不含 `'unsafe-eval'`）——
 * 于是**安装版**启动时抛 `EvalError`，校验器编译失败，前端整页白屏；
 * 开发模式因为 `devCsp` 带 `'unsafe-eval'` 一切正常，所以这个缺陷只在打包后出现。
 *
 * 修法是把「编译」从运行期挪到构建期：`scripts/generate-validators.mjs` 用
 * Ajv 的 `standaloneCode()` 把 29 个契约定义序列化成普通 ES 模块
 * （`validators.generated.ts`），运行期只剩静态函数，**不再需要 eval**。
 *
 * 两条边界：
 *
 * - 改了 Rust 类型 → 必须重新生成（`pnpm contracts:generate`），
 *   否则前端校验的是旧形状；`pnpm contracts:check` 会在 CI 上挡住这件事。
 * - 生产 CSP **不要**为了这里加 `'unsafe-eval'`。
 */

export const CONTRACTS_SCHEMA_ID = contractsSchema.$id as string

/** schema 里 `definitions` 下的类型名，编译期与生成物对齐。 */
export type ContractDefinitionName = keyof typeof contractsSchema.definitions

/**
 * 预编译校验函数的形状（Ajv standalone 的产物）。
 *
 * 只用 `data` 与 `errors` 两件事，所以不依赖 Ajv 的类型——
 * Ajv 因此可以退回 devDependencies，前端运行时不再引用它。
 */
export interface PrecompiledValidator<T = unknown> {
  (data: unknown): data is T
  errors?: ReadonlyArray<{ instancePath?: string; message?: string }> | null
}

const validators = precompiled as unknown as Record<
  string,
  PrecompiledValidator | undefined
>

/**
 * 取得某个契约定义的校验函数。
 *
 * 规格 6.4：运行时 Schema 设置 `additionalProperties=false`，拒绝未知字段。
 * 该约束写在生成物里，这里不再重复实现，避免两处规则漂移。
 */
export function validatorFor<T = unknown>(
  definitionName: ContractDefinitionName,
): PrecompiledValidator<T> {
  const validator = validators[definitionName as string]
  if (!validator) {
    // 生成物与调用方不一致时立刻失败，不返回恒真校验器
    throw new Error(
      `契约定义 "${definitionName}" 的预编译校验器不存在。` +
        '请先运行 pnpm contracts:generate，并检查调用方与 Rust 类型是否一致。',
    )
  }
  return validator as PrecompiledValidator<T>
}

export interface ValidationOutcome {
  valid: boolean
  /** 人类可读的失败原因，用于诊断；不要展示给终端用户当业务错误。 */
  errors: string[]
}

/** 返回结果而不是抛异常，方便在 IPC 边界记录后决定降级策略。 */
export function check<T = unknown>(
  definitionName: ContractDefinitionName,
  data: unknown,
): ValidationOutcome {
  const validate = validatorFor<T>(definitionName)
  if (validate(data)) return { valid: true, errors: [] }

  const errors = (validate.errors ?? []).map((e) => {
    const path = !e.instancePath ? '(root)' : e.instancePath
    return `${path} ${e.message ?? '校验失败'}`
  })
  return { valid: false, errors }
}
