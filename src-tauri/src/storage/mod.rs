//! 持久化层（规格 8.1）。
//!
//! 规格 3.2 的分工：`storage` 负责 SQLite 单写入队列、迁移、计划版本与日志事务。
//! 这一层不做业务判断，只保证三件事：
//!
//! - 结构由**带版本的迁移**演进，失败不留下中间态；
//! - 涉及文件变更的计划、操作、撤销计划与确认**必须持久化**
//!   （运行期的任务与授权状态可以只放内存）；
//! - 数据库位于系统提供的应用数据目录，**绝不**放进用户待整理的根目录。

pub mod db;
pub mod execution;
pub mod repositories;
pub mod runs;
pub mod undo_execution;

pub use db::{Database, Migration, MIGRATIONS};
