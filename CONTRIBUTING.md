# 参与贡献

感谢你愿意花时间。这份文档说明怎么把开发环境跑起来、改完代码要过哪些关、
以及这个项目的几条硬规矩。

## 环境要求

- Windows 11 x64（项目本身是 Windows 优先的桌面应用）
- Node.js ≥ 22.12，pnpm 10（`packageManager` 字段锁定）
- Rust ≥ 1.98（`rust-toolchain.toml` 锁定）
- Python 3（`scripts/` 下的构建与检查脚本）
- 系统 WebView2（Windows 11 通常自带）

```bash
pnpm install --frozen-lockfile   # 不改 lockfile 的干净安装
pnpm dev                         # 开发模式
```

## 改完代码要过的关

提交前请在本地跑完，CI 会原样再跑一遍（见 `.github/workflows/ci.yml`）：

```bash
pnpm typecheck        # TypeScript 严格模式
pnpm lint             # eslint
pnpm test             # vitest 单元/组件测试
pnpm test:e2e         # Playwright（契约 mock，不碰真实文件）
pnpm contracts:check  # 前后端契约一致性
cargo fmt --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

契约（`src/api/contracts.schema.json`）由 Rust 类型生成：
改了 Rust 侧类型后跑 `pnpm contracts:generate`，再把生成的改动一起提交。

另外**不要提交本机路径**（源码、脚本、文档、注释里的 `C:\Users\<你>\...`）：
`python scripts/scan-release-content.py` 会把它连同密钥形状一起扫出来，
命中即非零退出。CI 之外的建议在提交前跑一次。

## 这个项目的硬规矩

这些不是风格偏好，是产品承诺。违反它们的 PR 不会被合并：

1. **AI 只建议，不执行。** 任何文件变更必须经过确定性规划器、
   路径校验、用户确认，然后由安全执行器落地。
2. **预览不写文件。** 生成计划、预览、校验这些阶段不允许产生任何磁盘写入。
3. **不覆盖、不越界。** 目标已存在就失败，绝不自动改名绕过；
   授权根目录之外的路径一个都不碰。
4. **密钥不落盘。** API Key 只进 Windows 凭据存储；
   数据库、日志、错误信息里都不该出现它。
5. **集成测试只用合成数据。** 所有测试在 `TempDir` 里造临时文件，
   绝不读写真实用户目录；测试夹具里的模型输出必须是编造的。
6. **不确定就失败，不要猜。** 解析不了、身份对不上、状态歧义时，
   明确报错并保留现状——不猜一个「可能是对的」结果继续。

## 提交与 PR

- Commit message 用英文、祈使句，说明「为什么」而不只是「做了什么」。
- 一个 PR 只做一件事；涉及行为变更的，带上测试。
- 文档里写「实测」「验证过」的地方，请附上你实际跑的命令和结果。

## 许可证

本项目以 MIT 发布（见 [LICENSE](LICENSE)）。
**提交贡献即表示你同意以 MIT 许可证发布你的贡献。**
新增第三方依赖前请先看 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)
第 1 节的审计结论——强 copyleft（GPL/AGPL/SSPL）依赖不会被接受。
添加/升级依赖后请重跑 `python scripts/third-party-notices.py` 并提交结果。
