-- FilePilot 初始数据库迁移（T04）
--
-- 真源：docs/MASTER_PLAN.md 第 8.1 节的表与字段清单。
-- 本文件由 `src/storage/db.rs` 通过 include_str! 嵌入，迁移在**单个事务**里执行：
-- 中途失败时整份迁移回滚，schema_migrations 不会留下"已升版本"的记录。
--
-- 通用约定：
--   * 时间统一存 UTC RFC3339 文本（规格 5.1）；
--   * 路径与指纹存 JSON 文本（relativePathJson / fingerprintJson 等），
--     因为它们是**组件数组**而不是字符串，用分隔符拼接会在遇到含分隔符的名字时产生歧义；
--   * 枚举存 camelCase 文本，与 IPC 线上取值一致；
--   * 布尔存 INTEGER 0/1（SQLite 没有布尔类型）。
--
-- 数据库位于系统提供的应用数据目录，**绝不**放在用户待整理的根目录内（规格 8.1）。

-- 根目录授权。canonicalPath 是规范化的真实路径，只保存在本地，不跨 IPC 传输。
CREATE TABLE roots (
    id            TEXT    PRIMARY KEY,
    canonicalPath TEXT    NOT NULL,
    volumeId      TEXT    NOT NULL,
    identity      TEXT    NOT NULL,
    sessionId     TEXT    NOT NULL
);

-- 一次扫描。
CREATE TABLE scans (
    id         TEXT    PRIMARY KEY,
    rootId     TEXT    NOT NULL REFERENCES roots (id),
    status     TEXT    NOT NULL,
    recursive  INTEGER NOT NULL,
    startedAt  TEXT    NOT NULL,
    finishedAt TEXT,
    truncated  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_scans_root ON scans (rootId);

-- 扫描到的文件。skipCode 为 NULL 表示可参与整理。
--
-- extractionStatus 是规格 8.1 字段清单之外的补充列：IPC 契约里的 `FileRecord`
-- 带 `extractionStatus`（规格 5.1），少了这一列，从库里读回来的记录就与它
-- 代表的类型不一致——"能存不能还原"比多一列更糟。分析结果本身仍存在 analyses 表。
CREATE TABLE files (
    id                TEXT    PRIMARY KEY,
    scanId            TEXT    NOT NULL REFERENCES scans (id) ON DELETE CASCADE,
    relativePathJson  TEXT    NOT NULL,
    fingerprintJson   TEXT    NOT NULL,
    extension         TEXT    NOT NULL,
    extractionStatus  TEXT    NOT NULL DEFAULT 'pending',
    skipCode          TEXT,
    UNIQUE (scanId, relativePathJson)
);

CREATE INDEX idx_files_scan ON files (scanId);

-- 一次分析（规则或 AI）。**不保存正文**：规格 6.2 要求提取内容只留在内存
-- 或会话临时缓存里。proposalJson 只包含建议，不含绝对路径。
CREATE TABLE analyses (
    id                     TEXT PRIMARY KEY,
    scanId                 TEXT NOT NULL REFERENCES scans (id) ON DELETE CASCADE,
    mode                   TEXT NOT NULL,
    providerId             TEXT,
    status                 TEXT NOT NULL,
    proposalJson           TEXT,
    inputFingerprintsJson  TEXT,
    promptVersion          TEXT
);

CREATE INDEX idx_analyses_scan ON analyses (scanId);

-- 计划。digest 在校验通过后写入（规格 7.2）。
CREATE TABLE plans (
    id        TEXT    PRIMARY KEY,
    scanId    TEXT    NOT NULL REFERENCES scans (id),
    rootId    TEXT    NOT NULL REFERENCES roots (id),
    revision  INTEGER NOT NULL,
    status    TEXT    NOT NULL,
    digest    TEXT,
    mode      TEXT    NOT NULL,
    createdAt TEXT    NOT NULL
);

CREATE INDEX idx_plans_scan ON plans (scanId);

-- 计划项。expectedJson 是执行前必须重新核对的源指纹。
--
-- fileId 与 action 同样是规格字段清单之外的补充列：IPC 的 `PlanItem` 带
-- `fileId` 与 `action`（规格 5.1），少了它们，从库里读回来的计划项就与
-- 用户确认过的那个对象不是同一个东西——而执行器只认库里的计划。
CREATE TABLE plan_items (
    id           TEXT    PRIMARY KEY,
    planId       TEXT    NOT NULL REFERENCES plans (id) ON DELETE CASCADE,
    fileId       TEXT    NOT NULL,
    sourceJson   TEXT    NOT NULL,
    targetJson   TEXT    NOT NULL,
    action       TEXT    NOT NULL,
    selected     INTEGER NOT NULL,
    origin       TEXT    NOT NULL,
    expectedJson TEXT    NOT NULL,
    reason       TEXT    NOT NULL
);

CREATE INDEX idx_plan_items_plan ON plan_items (planId);

-- 一次性确认。tokenHash 唯一，且与 digest 绑定（规格 7.2）。
CREATE TABLE confirmations (
    tokenHash  TEXT PRIMARY KEY,
    planId     TEXT NOT NULL REFERENCES plans (id) ON DELETE CASCADE,
    revision   INTEGER NOT NULL,
    digest     TEXT NOT NULL,
    expiresAt  TEXT NOT NULL,
    consumedAt TEXT
);

-- 一次执行（含撤销执行，direction = 'undo'）。
CREATE TABLE runs (
    id         TEXT PRIMARY KEY,
    planId     TEXT NOT NULL REFERENCES plans (id),
    requestId  TEXT NOT NULL UNIQUE,
    direction  TEXT NOT NULL,
    status     TEXT NOT NULL,
    startedAt  TEXT NOT NULL,
    finishedAt TEXT
);

CREATE INDEX idx_runs_plan ON runs (planId);

-- 单个文件操作。resolution 表达"人工确认保留现状"这一审计事实（规格 8.2）。
CREATE TABLE operations (
    id                    TEXT PRIMARY KEY,
    runId                 TEXT NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    itemId                TEXT NOT NULL,
    originalOperationId   TEXT REFERENCES operations (id),
    sequence              INTEGER NOT NULL,
    sourceJson            TEXT NOT NULL,
    targetJson            TEXT NOT NULL,
    expectedJson          TEXT NOT NULL,
    status                TEXT NOT NULL,
    undoStatus            TEXT NOT NULL,
    resolution            TEXT NOT NULL CHECK (resolution IN ('open', 'acknowledged')),
    errorCode             TEXT,
    UNIQUE (runId, itemId)
);

CREATE INDEX idx_operations_run ON operations (runId, sequence);

-- 操作事件。**追加写**，不覆盖，构成审计轨迹（规格 8.2）。
CREATE TABLE operation_events (
    id          TEXT PRIMARY KEY,
    operationId TEXT NOT NULL REFERENCES operations (id) ON DELETE CASCADE,
    phase       TEXT NOT NULL,
    timestamp   TEXT NOT NULL,
    payloadJson TEXT
);

CREATE INDEX idx_operation_events_operation ON operation_events (operationId, timestamp);

-- 由本次执行创建的目录。撤销时只有这里记录、身份一致且已空的目录才能清理。
CREATE TABLE created_dirs (
    id                TEXT PRIMARY KEY,
    runId             TEXT NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    relativePathJson  TEXT NOT NULL,
    directoryIdentity TEXT NOT NULL,
    createdByRun      INTEGER NOT NULL,
    state             TEXT NOT NULL
);

CREATE INDEX idx_created_dirs_run ON created_dirs (runId);

-- 非敏感设置。**禁止**在这里存密钥：API Key 只进 Windows 凭据存储（规格 3.3）。
-- `key` 是 SQLite 关键字，必须加引号当标识符用。
CREATE TABLE settings (
    "key"     TEXT PRIMARY KEY,
    valueJson TEXT NOT NULL
);

-- 提供商配置。credentialRef 只是引用，不是密钥本身。
CREATE TABLE providers (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    endpoint      TEXT NOT NULL,
    model         TEXT NOT NULL,
    credentialRef TEXT
);

-- 撤销计划。规格 8.1：不能借前端本地状态替代，必须持久化。
CREATE TABLE undo_plans (
    id            TEXT PRIMARY KEY,
    originalRunId TEXT NOT NULL REFERENCES runs (id),
    digest        TEXT NOT NULL,
    itemsJson     TEXT NOT NULL,
    tokenHash     TEXT NOT NULL UNIQUE,
    expiresAt     TEXT NOT NULL,
    consumedAt    TEXT
);

-- 注意：这里**没有** schema_migrations。
-- 那张表由迁移器自己在执行任何迁移之前创建（见 src/storage/db.rs），
-- 因为"记录迁移的表"必须先于第一条迁移存在。把它写进迁移本体只会造成
-- 重复创建——这是本文件第一版真实踩到的错误，测试直接把它挡了下来。
