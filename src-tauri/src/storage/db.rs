//! SQLite 打开、PRAGMA 与迁移（规格 8.1）。
//!
//! 三件必须成立的事，每件都有对应测试：
//!
//! 1. **空数据库能初始化**：第一次启动就能建出全部表；
//! 2. **再次启动不重复迁移**：已记录的版本不会被重新执行；
//! 3. **失败的迁移不留下"已升版本"**：迁移本体与版本记录在同一事务里，
//!    中途失败整份回滚。
//!
//! 规格 8.1 还要求：启用 `foreign_keys`、`WAL`、`synchronous=FULL`，
//! 并使用**串行写入队列**。串行写入由 [`Database`] 内部的那把互斥锁保证——
//! 连接不暴露给外部，所有写操作都必须经过 [`Database::with_transaction`]。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags, Transaction};
use uuid::Uuid;

use crate::domain::errors::{codes, AppError};
use crate::domain::time::civil_from_unix_ns;

/// 一条迁移。
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// 全部迁移。顺序必须与版本号一致。
///
/// SQL 用 `include_str!` 嵌入二进制：安装包里不需要额外分发 .sql 文件，
/// 也就不存在"程序与迁移文件版本不一致"这种故障。
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "001_initial",
        sql: include_str!("../../migrations/001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "002_execution_requests",
        sql: include_str!("../../migrations/002_execution_requests.sql"),
    },
    Migration {
        version: 3,
        name: "003_undo_requests",
        sql: include_str!("../../migrations/003_undo_requests.sql"),
    },
];

/// 迁移记录表。它在任何迁移之前就要存在，因此单独建。
const SCHEMA_MIGRATIONS_DDL: &str = "\
CREATE TABLE IF NOT EXISTS schema_migrations (
    version   INTEGER PRIMARY KEY,
    name      TEXT    NOT NULL,
    appliedAt TEXT    NOT NULL
)";

/// 应用数据库句柄。
///
/// 连接不对外暴露：所有访问都必须经过这里的方法，写入因此天然串行。
pub struct Database {
    connection: Mutex<Connection>,
    path: Option<PathBuf>,
    migration_backup_path: Option<PathBuf>,
}

impl Database {
    /// 打开（必要时创建）磁盘上的数据库，并把迁移跑到最新。
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let existed_before_open = path.is_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                AppError::new(
                    codes::DB_UNAVAILABLE,
                    format!("无法创建应用数据目录: {error}"),
                )
            })?;
        }

        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_db_error)?;

        // 规格 8.1：已有数据库在迁移前必须通过 SQLite Online Backup API 备份。
        // 不能复制主数据库文件，因为 WAL 中可能还有已提交、尚未 checkpoint 的页。
        let migration_backup_path = if existed_before_open && needs_migration(&connection)? {
            Some(backup_before_migration(&connection, path)?)
        } else {
            None
        };

        let mut database = Self {
            connection: Mutex::new(connection),
            path: Some(path.to_path_buf()),
            migration_backup_path,
        };
        database.apply_pragmas(true)?;
        database.migrate()?;
        Ok(database)
    }

    /// 内存数据库。仅用于测试与一次性计算。
    ///
    /// 内存库不支持 WAL，因此这里跳过 journal_mode；其余 PRAGMA 一致，
    /// 否则测试验证的就不是生产的那套语义了。
    pub fn open_in_memory() -> Result<Self, AppError> {
        let connection = Connection::open_in_memory().map_err(map_db_error)?;
        let mut database = Self {
            connection: Mutex::new(connection),
            path: None,
            migration_backup_path: None,
        };
        database.apply_pragmas(false)?;
        database.migrate()?;
        Ok(database)
    }

    /// 数据库文件路径；内存库为 `None`。
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// 本次打开时生成的迁移前备份；没有发生迁移或是内存库时为 `None`。
    pub fn migration_backup_path(&self) -> Option<&Path> {
        self.migration_backup_path.as_deref()
    }

    /// 迁移到最新版本，返回本次**新执行**的版本号。
    pub fn migrate(&mut self) -> Result<Vec<i64>, AppError> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        migrate(&mut connection, MIGRATIONS)
    }

    /// 当前 schema 版本（已应用的最大版本号；空库为 0）。
    pub fn schema_version(&self) -> Result<i64, AppError> {
        let connection = self.connection();
        read_schema_version(&connection)
    }

    /// 已应用的迁移版本，升序。
    pub fn applied_migrations(&self) -> Result<Vec<i64>, AppError> {
        let connection = self.connection();
        read_applied(&connection)
    }

    /// 只读访问。写入请走 [`Database::with_transaction`]。
    pub fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 在一个事务里执行写入。闭包返回 `Err` 或 panic 时事务回滚。
    pub fn with_transaction<T>(
        &self,
        body: impl FnOnce(&Transaction<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(map_db_error)?;
        let value = body(&transaction)?;
        transaction.commit().map_err(map_db_error)?;
        Ok(value)
    }

    fn apply_pragmas(&mut self, on_disk: bool) -> Result<(), AppError> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // 外键默认是关的。规格 8.1 要求显式启用，否则 REFERENCES 只是注释。
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(map_db_error)?;

        if on_disk {
            // WAL 提升并发读性能；synchronous=FULL 保证掉电后已提交事务不丢。
            // 规格 8.1 点名了这两项。
            connection
                .execute_batch("PRAGMA journal_mode = WAL;")
                .map_err(map_db_error)?;
            connection
                .execute_batch("PRAGMA synchronous = FULL;")
                .map_err(map_db_error)?;
        }

        // 外键必须真的生效，不能只写一句 PRAGMA 就当完成了。
        let enabled: i64 = connection
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .map_err(map_db_error)?;
        if enabled != 1 {
            return Err(AppError::new(
                codes::DB_UNAVAILABLE,
                "无法启用 SQLite 外键约束，拒绝在未启用约束的数据库上运行",
            ));
        }
        Ok(())
    }
}

/// 当前库是否存在尚未应用的内置迁移。
///
/// 这里只读 `sqlite_master`，不会为了探测版本而先创建迁移表；否则备份就已经
/// 不是严格意义上的「迁移前」状态了。
fn needs_migration(connection: &Connection) -> Result<bool, AppError> {
    let has_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get(0),
        )
        .map_err(map_db_error)?;
    if has_table == 0 {
        return Ok(!MIGRATIONS.is_empty());
    }

    let applied = read_applied(connection)?;
    validate_applied_versions(&applied, MIGRATIONS)?;
    Ok(MIGRATIONS
        .iter()
        .any(|migration| !applied.contains(&migration.version)))
}

/// 用 SQLite Online Backup API 生成一个唯一的迁移前快照。
fn backup_before_migration(connection: &Connection, path: &Path) -> Result<PathBuf, AppError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("filepilot.db");
    let backup_path =
        path.with_file_name(format!("{file_name}.pre-migration-{}.bak", Uuid::new_v4()));

    let mut destination = Connection::open_with_flags(
        &backup_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_db_error)?;
    let backup = Backup::new(connection, &mut destination).map_err(map_db_error)?;
    backup
        .run_to_completion(128, Duration::from_millis(10), None)
        .map_err(map_db_error)?;
    drop(backup);
    drop(destination);

    Ok(backup_path)
}

/// 按顺序执行尚未应用的迁移。
///
/// 返回本次新执行的版本号（已应用过的不会重复执行，也不出现在返回值里）。
pub fn migrate(
    connection: &mut Connection,
    migrations: &[Migration],
) -> Result<Vec<i64>, AppError> {
    connection
        .execute_batch(SCHEMA_MIGRATIONS_DDL)
        .map_err(map_db_error)?;

    let applied = read_applied(connection)?;

    let mut ordered: Vec<&Migration> = migrations.iter().collect();
    ordered.sort_by_key(|migration| migration.version);
    validate_migration_catalog(&ordered)?;
    validate_applied_versions(&applied, migrations)?;

    let mut newly_applied = Vec::new();
    for migration in ordered {
        if applied.contains(&migration.version) {
            continue;
        }

        // 关键：迁移本体与版本记录在**同一个事务**里。
        // 只要有一条语句失败，`?` 会让事务被 drop → 回滚 →
        // 既不留下半张表，也不留下"已升版本"的记录。
        let transaction = connection.transaction().map_err(map_db_error)?;
        transaction.execute_batch(migration.sql).map_err(|error| {
            AppError::new(
                codes::DB_UNAVAILABLE,
                format!("迁移 {} 执行失败: {error}", migration.name),
            )
        })?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, appliedAt) VALUES (?1, ?2, ?3)",
                rusqlite::params![migration.version, migration.name, now_rfc3339()],
            )
            .map_err(map_db_error)?;
        transaction.commit().map_err(map_db_error)?;

        newly_applied.push(migration.version);
    }

    Ok(newly_applied)
}

fn validate_migration_catalog(ordered: &[&Migration]) -> Result<(), AppError> {
    let mut previous = None;
    for migration in ordered {
        if migration.version <= 0 || previous == Some(migration.version) {
            return Err(AppError::new(
                codes::DB_UNAVAILABLE,
                format!("迁移版本必须是唯一的正整数，发现 {}", migration.version),
            ));
        }
        previous = Some(migration.version);
    }
    Ok(())
}

/// 已应用版本必须是当前程序迁移表的一个前缀。
///
/// 若数据库来自更新版本，或版本记录有缺口，继续运行旧代码会把未知 schema 当作
/// 自己理解的结构来写入。此时必须停止，而不是悄悄接受。
fn validate_applied_versions(applied: &[i64], migrations: &[Migration]) -> Result<(), AppError> {
    let mut known: Vec<i64> = migrations
        .iter()
        .map(|migration| migration.version)
        .collect();
    known.sort_unstable();
    if applied.len() > known.len() || applied != &known[..applied.len()] {
        return Err(AppError::new(
            codes::DB_UNAVAILABLE,
            format!(
                "数据库迁移版本 {:?} 与当前程序支持的版本 {:?} 不兼容",
                applied, known
            ),
        ));
    }
    Ok(())
}

fn read_applied(connection: &Connection) -> Result<Vec<i64>, AppError> {
    let mut statement = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .map_err(map_db_error)?;
    let rows = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(map_db_error)?;

    let mut versions = Vec::new();
    for row in rows {
        versions.push(row.map_err(map_db_error)?);
    }
    Ok(versions)
}

fn read_schema_version(connection: &Connection) -> Result<i64, AppError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(map_db_error)
}

/// 当前 UTC 时间，RFC3339 格式。
fn now_rfc3339() -> String {
    let ns = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i128,
        Err(error) => -(error.duration().as_nanos() as i128),
    };
    civil_from_unix_ns(ns)
        .map(|value| value.to_rfc3339_utc())
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_owned())
}

/// 把 rusqlite 的错误映射成结构化错误。
///
/// 规格 8.5：`DB_UNAVAILABLE` 的处理约定是「停止执行，先恢复数据库可用性」。
/// `details` 只放 SQLite 的错误码数字，不放 SQL 原文或路径——它们是本地信息，
/// 但没有必要进入可能被展示或上报的结构里。
pub fn map_db_error(error: rusqlite::Error) -> AppError {
    let code = match &error {
        rusqlite::Error::SqliteFailure(inner, _) => inner.extended_code,
        _ => 0,
    };
    AppError::new(codes::DB_UNAVAILABLE, format!("数据库操作失败: {error}"))
        .with_detail("sqliteCode", code.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("filepilot.db")
    }

    /// 迁移脚本的版本号序列，从真源推导。
    ///
    /// 刻意**不写死 `vec![1]`**：加一条迁移就要改一处断言，而漏改的代价是
    /// 「迁移明明加上了，测试却还在替旧版本打包票」——那比没有测试更糟。
    fn all_versions() -> Vec<i64> {
        MIGRATIONS.iter().map(|m| m.version).collect()
    }

    #[test]
    fn an_empty_database_is_initialized_to_the_latest_version() {
        let database = Database::open_in_memory().expect("应能初始化内存库");
        assert_eq!(
            database.schema_version().expect("应能读取版本"),
            MIGRATIONS.last().expect("必须有迁移").version
        );
        assert_eq!(
            database.applied_migrations().expect("应能读取"),
            all_versions()
        );
    }

    #[test]
    fn reopening_does_not_reapply_migrations() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = temp_db_path(&dir);

        let first = Database::open(&path).expect("首次打开应成功");
        assert_eq!(
            first.applied_migrations().expect("应能读取"),
            all_versions()
        );
        drop(first);

        let second = Database::open(&path).expect("再次打开应成功");
        assert!(
            second.applied_migrations().expect("应能读取") == all_versions(),
            "版本不应重复记录"
        );
        // 直接调用 migrate 也必须返回「没有新执行任何迁移」
        let mut third = Database::open(&path).expect("第三次打开应成功");
        assert!(
            third.migrate().expect("重复迁移应成功").is_empty(),
            "重复迁移不应再次执行 SQL"
        );
    }

    #[test]
    fn a_failing_migration_leaves_neither_partial_tables_nor_a_bumped_version() {
        let mut connection = Connection::open_in_memory().expect("应能打开内存库");
        let migrations = [
            Migration {
                version: 1,
                name: "001_ok",
                sql: "CREATE TABLE good (id INTEGER PRIMARY KEY);",
            },
            Migration {
                version: 2,
                name: "002_broken",
                // 前半句合法、后半句引用不存在的表：验证「同一事务里前半句也回滚」
                sql: "CREATE TABLE partial (id INTEGER PRIMARY KEY);\
                      INSERT INTO not_a_table (id) VALUES (1);",
            },
        ];

        let error = migrate(&mut connection, &migrations).expect_err("坏迁移必须报错");
        assert_eq!(error.code, codes::DB_UNAVAILABLE);

        assert_eq!(read_schema_version(&connection).expect("应能读版本"), 1);
        let partial_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'partial'",
                [],
                |row| row.get(0),
            )
            .expect("应能查询 sqlite_master");
        assert_eq!(
            partial_exists, 0,
            "失败迁移的前半句也必须回滚，不能留下半张表"
        );
    }

    #[test]
    fn a_failed_migration_can_be_retried_after_the_sql_is_fixed() {
        let mut connection = Connection::open_in_memory().expect("应能打开内存库");
        let broken = [Migration {
            version: 1,
            name: "001",
            sql: "THIS IS NOT SQL;",
        }];
        assert!(migrate(&mut connection, &broken).is_err());

        // 修好之后重跑：版本 1 尚未记录，因此会真正执行
        let fixed = [Migration {
            version: 1,
            name: "001",
            sql: "CREATE TABLE good (id INTEGER PRIMARY KEY);",
        }];
        assert_eq!(migrate(&mut connection, &fixed).expect("应能重跑"), vec![1]);
    }

    #[test]
    fn foreign_keys_are_actually_enabled() {
        let database = Database::open_in_memory().expect("应能初始化内存库");
        let connection = database.connection();
        let enabled: i64 = connection
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .expect("应能查询 PRAGMA");
        assert_eq!(
            enabled, 1,
            "规格 8.1 要求外键真的生效，而不是只写一句 PRAGMA"
        );
    }

    #[test]
    fn on_disk_databases_use_wal_and_full_synchronous() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let database = Database::open(&temp_db_path(&dir)).expect("应能打开");

        let connection = database.connection();
        let journal: String = connection
            .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .expect("应能查询 journal_mode");
        assert_eq!(journal.to_lowercase(), "wal");

        let synchronous: i64 = connection
            .query_row("PRAGMA synchronous;", [], |row| row.get(0))
            .expect("应能查询 synchronous");
        assert_eq!(synchronous, 2, "FULL = 2");
    }

    #[test]
    fn the_spec_tables_all_exist() {
        let database = Database::open_in_memory().expect("应能初始化内存库");
        let connection = database.connection();

        for table in [
            "roots",
            "scans",
            "files",
            "analyses",
            "plans",
            "plan_items",
            "confirmations",
            "runs",
            "operations",
            "operation_events",
            "created_dirs",
            "settings",
            "providers",
            "undo_plans",
            "schema_migrations",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("应能查询 sqlite_master");
            assert_eq!(count, 1, "规格 8.1 的表 {table} 缺失");
        }
    }

    #[test]
    fn migrations_are_applied_in_version_order() {
        let mut connection = Connection::open_in_memory().expect("应能打开内存库");
        let migrations = [
            Migration {
                version: 5,
                name: "005",
                sql: "CREATE TABLE five (id INTEGER PRIMARY KEY);",
            },
            Migration {
                version: 3,
                name: "003",
                sql: "CREATE TABLE three (id INTEGER PRIMARY KEY);",
            },
        ];
        assert_eq!(
            migrate(&mut connection, &migrations).expect("应能迁移"),
            vec![3, 5],
            "必须按版本升序执行，与数组顺序无关"
        );
    }

    #[test]
    fn a_database_from_a_newer_or_inconsistent_schema_is_rejected() {
        let mut connection = Connection::open_in_memory().expect("应能打开内存库");
        connection
            .execute_batch(SCHEMA_MIGRATIONS_DDL)
            .expect("建版本表");
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, appliedAt) VALUES (999, 'future', 'now')",
                [],
            )
            .expect("写未来版本");

        let error = migrate(&mut connection, MIGRATIONS).expect_err("不能用旧程序打开未来 schema");
        assert_eq!(error.code, codes::DB_UNAVAILABLE);
        assert!(error.message.contains("不兼容"));
    }
}
