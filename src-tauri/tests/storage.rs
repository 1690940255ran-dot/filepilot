//! T04 验收：SQLite 初始迁移与 storage 层（规格 8.1）。
//!
//! 这些测试验证的是**持久化的承诺**，不是"能读写一张表"：
//!
//! - 空库能初始化，再次启动不重复迁移；
//! - 迁移失败不留下"已升版本"，也不留下半张表；
//! - 计划与其计划项同生共死；
//! - 更新计划必须带上期望版本，版本不符就拒绝（乐观锁）。

use filepilot_lib::domain::errors::codes;
use filepilot_lib::domain::types::{
    ExtractionStatus, FileRecord, Fingerprint, Mode, Plan, PlanAction, PlanItem, PlanItemOrigin,
    PlanStatus, RelPath,
};
use filepilot_lib::storage::db::{migrate, Database, Migration, MIGRATIONS};
use filepilot_lib::storage::repositories::{
    count_rows, file_root_id, insert_files, insert_root, insert_scan, list_files, load_plan,
    plan_digest, plan_revision, reauthorize_matching_roots, save_plan, set_plan_digest, RootRow,
    ScanRow,
};

const SCAN_ID: &str = "scan-1";
const ROOT_ID: &str = "root-1";

fn memory_db() -> Database {
    Database::open_in_memory().expect("应能初始化内存库")
}

/// 建好外键需要的前置行：根目录与扫描。
fn seed_root_and_scan(db: &Database) {
    insert_root(
        db,
        &RootRow {
            id: ROOT_ID.to_owned(),
            canonical_path: r"C:\Users\me\资料".to_owned(),
            volume_id: "V-1234".to_owned(),
            identity: "FID-1".to_owned(),
            session_id: "session-1".to_owned(),
        },
    )
    .expect("写入根目录应成功");

    insert_scan(
        db,
        &ScanRow {
            id: SCAN_ID.to_owned(),
            root_id: ROOT_ID.to_owned(),
            status: "running".to_owned(),
            recursive: true,
            started_at: "2026-09-17T00:00:00.000Z".to_owned(),
        },
    )
    .expect("写入扫描应成功");
}

#[test]
fn reauthorization_matches_volume_and_directory_identity_not_path_alone() {
    let database = memory_db();
    seed_root_and_scan(&database);
    let matches = reauthorize_matching_roots(&database, "V-1234", "FID-1", "session-2")
        .expect("同一目录身份应能重新授权");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, ROOT_ID);

    let replaced = reauthorize_matching_roots(&database, "V-1234", "OTHER-FID", "session-3")
        .expect("查询本身成功");
    assert!(
        replaced.is_empty(),
        "同一路径被另一个目录替换时不能沿用旧 rootId"
    );
}

fn file_record(id: &str, name: &str) -> FileRecord {
    FileRecord {
        id: id.to_owned(),
        scan_id: SCAN_ID.to_owned(),
        root_id: ROOT_ID.to_owned(),
        relative_path: RelPath::from(vec![name.to_owned()]),
        extension: ".txt".to_owned(),
        fingerprint: Fingerprint {
            volume_id: "V-1234".to_owned(),
            file_id: format!("FID-{id}"),
            size: "5".to_owned(),
            modified_ns: "1700000000000000000".to_owned(),
            sha256: Some("a".repeat(64)),
        },
        extraction_status: ExtractionStatus::Pending,
        skip_code: None,
    }
}

fn plan_item(id: &str, source: &str, target: &str, selected: bool) -> PlanItem {
    PlanItem {
        id: id.to_owned(),
        file_id: format!("file-{id}"),
        source: RelPath::from(vec![source.to_owned()]),
        target: RelPath::from(target.split('\\').map(str::to_owned).collect::<Vec<_>>()),
        action: if source == target {
            PlanAction::Noop
        } else {
            PlanAction::Move
        },
        selected,
        origin: PlanItemOrigin::Rule,
        reason: "按文件类型分类".to_owned(),
        expected: Fingerprint {
            volume_id: "V-1234".to_owned(),
            file_id: format!("FID-{id}"),
            size: "5".to_owned(),
            modified_ns: "1700000000000000000".to_owned(),
            sha256: Some("b".repeat(64)),
        },
    }
}

fn sample_plan(items: Vec<PlanItem>) -> Plan {
    Plan {
        id: "plan-1".to_owned(),
        root_id: ROOT_ID.to_owned(),
        scan_id: SCAN_ID.to_owned(),
        revision: 1,
        mode: Mode::Rules,
        status: PlanStatus::Draft,
        items,
        created_at: "2026-09-17T00:00:00.000Z".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// 迁移
// ---------------------------------------------------------------------------

/// 迁移脚本的版本号序列，从真源推导。
///
/// **刻意不写死 `vec![1]`**：加一条迁移就要改一处断言，而漏改的代价是
/// 「迁移明明加上了，测试却还在替旧版本打包票」——那比没有测试更糟。
fn all_versions() -> Vec<i64> {
    MIGRATIONS.iter().map(|m| m.version).collect()
}

#[test]
fn an_on_disk_database_is_created_and_migrated() {
    let dir = tempfile::tempdir().expect("建临时目录");
    let path = dir.path().join("应用数据").join("filepilot.db");

    let database = Database::open(&path).expect("应能创建并初始化数据库");
    assert!(path.is_file(), "数据库文件应被创建");
    assert_eq!(
        database.schema_version().expect("应能读版本"),
        MIGRATIONS.last().expect("必须有迁移").version
    );
    assert_eq!(
        database.applied_migrations().expect("应能读记录"),
        all_versions()
    );
}

#[test]
fn reopening_a_database_does_not_reapply_migrations() {
    let dir = tempfile::tempdir().expect("建临时目录");
    let path = dir.path().join("filepilot.db");

    {
        let database = Database::open(&path).expect("首次打开应成功");
        seed_root_and_scan(&database);
    }

    let reopened = Database::open(&path).expect("再次打开应成功");
    assert_eq!(
        reopened.applied_migrations().expect("应能读记录"),
        all_versions(),
        "已应用的迁移不应重复执行"
    );
    assert_eq!(
        count_rows(&reopened, "roots").expect("应能统计"),
        1,
        "重新打开不应清空已有数据"
    );
}

#[test]
fn an_existing_database_is_backed_up_with_sqlite_before_migration() {
    let dir = tempfile::tempdir().expect("建临时目录");
    let path = dir.path().join("filepilot.db");

    // 模拟尚未进入迁移体系的旧数据库，并启用 WAL。备份必须包含 WAL 中已提交的内容，
    // 不能只复制主数据库文件。
    {
        let connection = rusqlite::Connection::open(&path).expect("应能创建旧库");
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;\
                 CREATE TABLE legacy_data (value TEXT NOT NULL);\
                 INSERT INTO legacy_data (value) VALUES ('需要保留');",
            )
            .expect("应能写旧库");
    }

    let database = Database::open(&path).expect("应先备份再迁移");
    assert_eq!(
        database.schema_version().expect("读版本"),
        MIGRATIONS.last().expect("必须有迁移").version,
        "旧库应当被升到最新版本"
    );
    let backup = database
        .migration_backup_path()
        .expect("已有数据库发生迁移时必须记录备份路径");
    assert!(backup.is_file(), "已有数据库迁移前必须生成 SQLite 备份");

    let backup_connection = rusqlite::Connection::open(backup).expect("备份应是可打开的 SQLite 库");
    let value: String = backup_connection
        .query_row("SELECT value FROM legacy_data", [], |row| row.get(0))
        .expect("备份必须包含迁移前已提交的数据");
    assert_eq!(value, "需要保留");
}

#[test]
fn a_failing_migration_rolls_back_and_keeps_the_previous_version() {
    let dir = tempfile::tempdir().expect("建临时目录");
    let path = dir.path().join("filepilot.db");

    // 先跑一次真实迁移，得到一个版本为 1 的库
    let database = Database::open(&path).expect("应能初始化");
    seed_root_and_scan(&database);
    drop(database);

    // 再用一个"第二条会失败"的迁移列表去升级它。
    //
    // 这一条的版本号必须**大于**当前最新版本：真实迁移里这个号已经被占掉了，
    // 复用同一个号会让 `migrate` 判成「已经应用过」而直接跳过，
    // 于是坏 SQL 根本跑不到，`expect_err` 也就成了对着一个正常结果开火。
    // 用 `latest + 1` 表达「这是未来的一条会失败的迁移」，不写死具体数字。
    let mut connection = rusqlite::Connection::open(&path).expect("应能打开同一个库");
    let failing_version = MIGRATIONS.last().expect("必须有迁移").version + 1;
    let broken = [Migration {
        version: failing_version,
        name: "999_broken",
        sql: "CREATE TABLE half (id INTEGER PRIMARY KEY);\
                  INSERT INTO missing_table (id) VALUES (1);",
    }];
    let error = migrate(&mut connection, &broken).expect_err("坏迁移必须报错");
    assert_eq!(error.code, codes::DB_UNAVAILABLE);
    drop(connection);

    // 库仍然可用，版本没被推进，且没有留下半张表
    let database = Database::open(&path).expect("库应仍然可用");
    assert_eq!(
        database.applied_migrations().expect("应能读记录"),
        all_versions()
    );
    assert_eq!(
        count_rows(&database, "roots").expect("应能统计"),
        1,
        "失败迁移不能损坏已有数据"
    );

    let connection = database.connection();
    let half_exists: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'half'",
            [],
            |row| row.get(0),
        )
        .expect("应能查询 sqlite_master");
    assert_eq!(half_exists, 0, "失败迁移的前半句也必须回滚");
}

#[test]
fn the_settings_table_accepts_a_quoted_keyword_column() {
    // `key` 是 SQLite 关键字，DDL 里必须加引号才能当列名用。
    // 这条测试锁住这个细节：改名或去掉引号会让它直接失败。
    let database = memory_db();
    let connection = database.connection();
    connection
        .execute(
            r#"INSERT INTO settings ("key", valueJson) VALUES (?1, ?2)"#,
            rusqlite::params!["mode", r#""rules""#],
        )
        .expect("写入设置应成功");
    let stored: String = connection
        .query_row(
            r#"SELECT valueJson FROM settings WHERE "key" = ?1"#,
            ["mode"],
            |row| row.get(0),
        )
        .expect("应能读回设置");
    assert_eq!(stored, r#""rules""#);
}

// ---------------------------------------------------------------------------
// 文件与计划
// ---------------------------------------------------------------------------

#[test]
fn files_round_trip_through_the_database() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let records = vec![
        file_record("f1", "a.txt"),
        file_record("f2", "中文 报告.txt"),
    ];
    assert_eq!(
        insert_files(&database, &records).expect("写入文件应成功"),
        2
    );

    let loaded = list_files(&database, SCAN_ID).expect("读回文件应成功");
    assert_eq!(loaded, records, "读回的记录必须与写入的完全一致");
    assert_eq!(
        file_root_id(&database, SCAN_ID).expect("应能读到 rootId"),
        Some(ROOT_ID.to_owned())
    );
}

#[test]
fn a_duplicate_file_in_the_same_scan_is_rejected() {
    let database = memory_db();
    seed_root_and_scan(&database);

    insert_files(&database, &[file_record("f1", "a.txt")]).expect("首次写入应成功");
    // 同一 scan 下同一相对路径：unique(scanId, relativePathJson) 必须挡住
    let error =
        insert_files(&database, &[file_record("f2", "a.txt")]).expect_err("重复路径必须被拒绝");
    assert_eq!(
        error.code,
        codes::INVALID_PATH,
        "约束冲突是数据问题，不该报成数据库不可用"
    );
    assert_eq!(
        count_rows(&database, "files").expect("应能统计"),
        1,
        "失败的那一批不能留下半条记录"
    );
}

#[test]
fn plans_round_trip_with_their_items_in_order() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let plan = sample_plan(vec![
        plan_item("i1", "a.txt", "文档\\a.txt", true),
        plan_item("i2", "b.txt", "图片\\b.txt", true),
        plan_item("i3", "c.txt", "c.txt", false),
    ]);
    save_plan(&database, &plan, None).expect("保存计划应成功");

    let loaded = load_plan(&database, &plan.id)
        .expect("读取计划应成功")
        .expect("计划应存在");
    assert_eq!(loaded, plan, "读回的计划必须与保存的完全一致");
    assert_eq!(
        plan_revision(&database, &plan.id).expect("应能读版本"),
        Some(1)
    );
    assert!(
        plan_digest(&database, &plan.id)
            .expect("应能读摘要")
            .is_none(),
        "刚创建的计划还没有校验摘要"
    );
}

#[test]
fn saving_a_plan_without_a_known_revision_does_not_overwrite_an_existing_one() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let plan = sample_plan(vec![plan_item("i1", "a.txt", "文档\\a.txt", true)]);
    save_plan(&database, &plan, None).expect("首次保存应成功");

    let error = save_plan(&database, &plan, None).expect_err("重复新建必须被拒绝");
    assert_eq!(error.code, codes::STALE_PLAN);
}

#[test]
fn a_new_plan_must_start_at_revision_one() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let mut plan = sample_plan(vec![plan_item("i1", "a.txt", "文档\\a.txt", true)]);
    plan.revision = 2;
    let error = save_plan(&database, &plan, None).expect_err("新计划必须从版本 1 开始");
    assert_eq!(error.code, codes::STALE_PLAN);
    assert_eq!(count_rows(&database, "plans").expect("统计"), 0);
}

#[test]
fn updating_a_plan_requires_the_expected_revision() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let mut plan = sample_plan(vec![plan_item("i1", "a.txt", "文档\\a.txt", true)]);
    save_plan(&database, &plan, None).expect("首次保存应成功");

    // 期望版本写错 → 拒绝（这就是"编辑使旧确认失效"依赖的机制）
    let error = save_plan(&database, &plan, Some(7)).expect_err("版本不符必须被拒绝");
    assert_eq!(error.code, codes::STALE_PLAN);

    // 期望版本正确 → 允许，并写入新版本
    plan.revision = 2;
    plan.items[0].selected = false;
    save_plan(&database, &plan, Some(1)).expect("按期望版本更新应成功");
    assert_eq!(
        plan_revision(&database, &plan.id).expect("应能读版本"),
        Some(2)
    );

    let loaded = load_plan(&database, &plan.id)
        .expect("应能读取")
        .expect("计划应存在");
    assert!(!loaded.items[0].selected, "更新后的选中集合必须落库");
}

#[test]
fn updating_a_plan_replaces_its_items_instead_of_accumulating_them() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let mut plan = sample_plan(vec![
        plan_item("i1", "a.txt", "文档\\a.txt", true),
        plan_item("i2", "b.txt", "文档\\b.txt", true),
    ]);
    save_plan(&database, &plan, None).expect("首次保存应成功");
    assert_eq!(count_rows(&database, "plan_items").expect("应能统计"), 2);

    plan.revision = 2;
    plan.items.truncate(1);
    save_plan(&database, &plan, Some(1)).expect("更新应成功");

    assert_eq!(
        count_rows(&database, "plan_items").expect("应能统计"),
        1,
        "旧计划项必须被替换，而不是与新项并存"
    );
}

#[test]
fn updating_a_plan_clears_the_previous_digest() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let mut plan = sample_plan(vec![plan_item("i1", "a.txt", "文档\\a.txt", true)]);
    save_plan(&database, &plan, None).expect("首次保存应成功");
    set_plan_digest(&database, &plan.id, 1, "digest-of-revision-1").expect("写入摘要应成功");
    assert_eq!(
        plan_digest(&database, &plan.id)
            .expect("应能读摘要")
            .as_deref(),
        Some("digest-of-revision-1")
    );

    plan.revision = 2;
    plan.items[0].target = RelPath::from(vec!["其他".to_owned(), "a.txt".to_owned()]);
    save_plan(&database, &plan, Some(1)).expect("更新应成功");

    assert!(
        plan_digest(&database, &plan.id)
            .expect("应能读摘要")
            .is_none(),
        "编辑之后旧摘要必须失效，否则可以用旧校验结果确认新计划"
    );
}

#[test]
fn a_digest_cannot_be_written_for_a_different_revision() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let plan = sample_plan(vec![plan_item("i1", "a.txt", "文档\\a.txt", true)]);
    save_plan(&database, &plan, None).expect("首次保存应成功");

    let error =
        set_plan_digest(&database, &plan.id, 2, "too-late").expect_err("版本不符时摘要不能写入");
    assert_eq!(error.code, codes::STALE_PLAN);
}

/// 计划内容变了却没有升版本时，乐观锁必须拒绝。
///
/// 只检查「期望版本 == 当前版本」是不够的：两个并发编辑都拿着版本 1，
/// 谁先写谁就把版本改成 2；但调用方一旦忘记升版本，两条保护同时失效，
/// 后写的内容会静默覆盖先写的。规格 7.2 要求「编辑后 revision 加一」，
/// 因此在存储层把这条不变量兜住。
#[test]
fn updating_a_plan_must_increase_the_revision() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let mut plan = sample_plan(vec![plan_item("i1", "a.txt", "文档\\a.txt", true)]);
    save_plan(&database, &plan, None).expect("首次保存应成功");

    // 内容改了，版本没改
    plan.items[0].target = RelPath::from(vec!["其他".to_owned(), "a.txt".to_owned()]);
    let error = save_plan(&database, &plan, Some(1)).expect_err("内容变了却不升版本，必须被拒绝");
    assert_eq!(error.code, codes::STALE_PLAN);
    assert!(
        error.message.contains("递增"),
        "错误信息要说清是版本没递增：{}",
        error.message
    );

    // 库里的内容必须保持原样
    let loaded = load_plan(&database, &plan.id)
        .expect("应能读取")
        .expect("计划应存在");
    assert_eq!(
        loaded.items[0].target,
        RelPath::from(vec!["文档".to_owned(), "a.txt".to_owned()]),
        "被拒绝的写入不能改动库里已有的计划"
    );

    // 升了版本就能写入
    plan.revision = 2;
    save_plan(&database, &plan, Some(1)).expect("升版本后应可更新");
}

#[test]
fn updating_a_plan_must_increment_the_revision_by_exactly_one() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let mut plan = sample_plan(vec![plan_item("i1", "a.txt", "文档\\a.txt", true)]);
    save_plan(&database, &plan, None).expect("首次保存应成功");

    plan.revision = 3;
    let error = save_plan(&database, &plan, Some(1)).expect_err("版本不能从 1 跳到 3");
    assert_eq!(error.code, codes::STALE_PLAN);
    assert_eq!(plan_revision(&database, &plan.id).expect("读版本"), Some(1));
}

#[test]
fn updating_a_plan_cannot_change_its_immutable_identity() {
    let database = memory_db();
    seed_root_and_scan(&database);

    let mut plan = sample_plan(vec![plan_item("i1", "a.txt", "文档\\a.txt", true)]);
    save_plan(&database, &plan, None).expect("首次保存应成功");

    plan.revision = 2;
    plan.root_id = "other-root".to_owned();
    let error = save_plan(&database, &plan, Some(1)).expect_err("更新不能换根目录身份");
    assert_eq!(error.code, codes::STALE_PLAN);

    let loaded = load_plan(&database, &plan.id)
        .expect("应能读取")
        .expect("计划应存在");
    assert_eq!(loaded.root_id, ROOT_ID);
    assert_eq!(loaded.revision, 1);
}

/// 删除扫描记录必须级联删除其文件；计划不能引用不存在的扫描。
#[test]
fn foreign_keys_cascade_files_and_reject_orphan_plans() {
    let database = memory_db();
    seed_root_and_scan(&database);
    insert_files(&database, &[file_record("f1", "a.txt")]).expect("写文件应成功");
    assert_eq!(count_rows(&database, "files").expect("统计"), 1);

    database
        .with_transaction(|transaction| {
            transaction
                .execute("DELETE FROM scans WHERE id = ?1", [SCAN_ID])
                .map_err(|error| {
                    filepilot_lib::domain::AppError::new(codes::DB_UNAVAILABLE, error.to_string())
                })?;
            Ok(())
        })
        .expect("删除扫描应成功");

    assert_eq!(
        count_rows(&database, "files").expect("统计"),
        0,
        "删除扫描必须级联清理它的文件记录，不能留下孤儿行"
    );

    // 计划引用不存在的扫描：外键必须挡住
    let orphan = database.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO plans (id, scanId, rootId, revision, status, mode, createdAt) \
                 VALUES ('p-orphan', 'no-such-scan', ?1, 1, 'draft', 'rules', '2026-09-17T00:00:00.000Z')",
                [ROOT_ID],
            )
            .map_err(|error| {
                filepilot_lib::domain::AppError::new(codes::DB_UNAVAILABLE, error.to_string())
            })?;
        Ok(())
    });
    assert!(orphan.is_err(), "计划不能引用不存在的扫描");
}

#[test]
fn plan_items_cannot_be_orphaned_from_their_plan() {
    let database = memory_db();
    let connection = database.connection();

    // 外键必须真的生效：指向不存在的计划插入计划项应失败。
    let result = connection.execute(
        "INSERT INTO plan_items \
         (id, planId, fileId, sourceJson, targetJson, action, selected, origin, expectedJson, reason) \
         VALUES ('i1', 'no-such-plan', 'f1', '[\"a.txt\"]', '[\"文档\",\"a.txt\"]', 'move', 1, 'rule', '{}', '')",
        [],
    );
    assert!(result.is_err(), "外键约束必须挡住孤儿计划项");
}
