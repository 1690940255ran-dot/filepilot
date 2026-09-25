# IPC 契约

## 唯一真源

**Rust 类型是唯一真源。** 见 `docs/DECISIONS.md` 的 ADR-006。

| 真源文件 | 内容 |
|---|---|
| `src-tauri/src/domain/types.rs` | 数据模型（FileRecord、Plan、PlanItem、Fingerprint …） |
| `src-tauri/src/domain/errors.rs` | `AppError` 与规格 8.5 的错误码表 |
| `src-tauri/src/domain/ipc.rs` | IPC 返回信封 `IpcResult<T>` |

前端**不允许**手写第二套同义类型。规格 0.6：不得出现两套同义但不兼容的数据结构。

## 生成流程

```text
src-tauri/src/domain/*.rs
        │  cargo run --bin export-contracts
        ├──────────────► src/api/contracts.generated.ts     TypeScript 类型（ts-rs）
        └──────────────► src/api/contracts.schema.json      JSON Schema（schemars）
```

命令：

```powershell
pnpm contracts:generate   # 生成
pnpm contracts:check      # 重新生成并比对，不一致即失败
```

`contracts:check` 会**强制重新生成**再比对。生成器不可用时它非零退出并报告原因——它不会把仓库里已有的文件当成「通过」。

## 运行时校验

前端在 IPC 边界用 **Ajv** 消费生成的 JSON Schema（`src/api/validation.ts`）。

选择 Ajv 而不是手写 Zod schema 的原因见 ADR-006：手写 Zod 等于维护第二套类型；JSON Schema → Zod 的自动转换只会多一个可能失真的环节。

运行时 Schema 设置 `additionalProperties=false`（规格 6.4）：未知字段一律拒绝。

## 序列化约定（规格 5.1）

| 约定 | 说明 |
|---|---|
| 命名 | 统一 camelCase（serde `rename_all = "camelCase"`） |
| ID | UUID 字符串。**不等同于** Windows 文件身份，**也不等同于**内容哈希 |
| 时间 | UTC RFC3339 字符串 |
| 文件大小 | 十进制**字符串**，避免 JavaScript 数值精度损失 |
| 高精度时间戳 | `modifiedNs` 为十进制字符串 |
| 相对路径 | **组件数组**，不以字符串拼接充当安全验证 |
| 枚举 | 小写字符串，见下方清单 |

## 命令返回信封

所有命令返回 `Result<T>`：

```ts
type Result<T> = { ok: true; data: T } | { ok: false; error: AppError }
```

Rust 侧是 `IpcResult<T>`。它手写了 `Serialize` 而不用 serde 的 tag 机制，
因为 tag 会把变体名写进 `ok` 字段（`{"ok":"Ok",...}`），而契约要求布尔量。
`ipc.rs` 里有单元测试锁住这个形状。

## 类型清单

### 枚举

| 类型 | 取值 |
|---|---|
| `Mode` | `rules` \| `aiLocal` \| `aiCloud` |
| `Risk` | `info` \| `warning` \| `block` |
| `ExtractionStatus` | `pending` \| `ok` \| `partial` \| `unsupported` \| `failed` |
| `TaskStatus` | `queued` \| `running` \| `completed` \| `partial` \| `failed` \| `cancelled` \| `recoveryRequired` |
| `RunStatus` | 同 `TaskStatus` |
| `OpStatus` | `pending` \| `prepared` \| `applied` \| `failed` \| `skipped` \| `ambiguous` |
| `UndoStatus` | `notRequested` \| `prepared` \| `undone` \| `conflict` |
| `PlanStatus` | `draft` \| `validated` \| `sealed` \| `archived` |
| `PlanAction` | `move` \| `noop` |
| `PlanItemOrigin` | `rule` \| `ai` \| `user` |

### 结构体

`Fingerprint`、`FileRecord`、`Evidence`、`Extraction`、`Proposal`、`PlanItem`、
`Plan`、`Issue`、`ValidationReport`、`RunCounts`、`RunReport`、`AppError`、`AppSettings`。

字段含义以 `docs/MASTER_PLAN.md` 第 5.1 节为准。

## 三种「身份」不可互换

这是最容易出错的地方，单独列出：

| 身份 | 字段 | 能检出什么 |
|---|---|---|
| 应用内标识 | `Id`（应用 UUID） | 应用自己的引用关系 |
| Windows 卷内文件身份 | `Fingerprint.fileId` | 「同名但已是另一个文件」的替换 |
| 内容身份 | `Fingerprint.sha256` | 「size 与 mtime 恰好相同」的内容篡改 |

规格 5.1 明确三者用途不同，不能互相替代。执行前必须重新核对全部三项。

## 已实现状态（P0）

| 命令 | 状态 |
|---|---|
| `get_settings` | 已实现（只读，无副作用） |
| `choose_root` | 未实现 —— 依赖 P1 的根目录授权与扫描 |
| 其余白名单命令 | 未实现 —— 按对应阶段推进 |

规格 T01：后续阶段命令在完成前不显示为可用，**不用恒成功空函数顶替**。
前端对应的入口保持禁用并显示理由。
