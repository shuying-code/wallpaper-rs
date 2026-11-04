# 贡献指南（Contributing to wallpaper-rs）

感谢你愿意为 **wallpaper-rs** 贡献力量！提交前请先阅读本指南，以便协作顺畅。

## 行为准则

参与本项目即表示你同意遵守 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

## 环境要求

- Rust 稳定版工具链（`rustup` 安装，`stable`）。
- 桌面 GUI 依赖：Linux 需要 `libxkbcommon-dev libgtk-3-dev libgl1-mesa-dev` 及 X11 相关开发包；Windows / macOS 使用系统自带组件。
- 建议使用 `cargo fmt` + `cargo clippy` 保持代码风格一致。

## 本地构建与测试

```bash
cargo build            # 调试构建
cargo build --release  # 发布构建
cargo test             # 运行测试
cargo fmt              # 格式化
cargo clippy -- -D warnings
./target/release/wallpaper-rs --selftest   # 无界面自检（若支持）
```

## 代码风格

- 遵循仓库根目录 [rustfmt.toml](rustfmt.toml) 与 [.editorconfig](.editorconfig)。
- 文本文件统一使用 **UTF-8 + LF** 换行（见 [.gitattributes](.gitattributes)）。
- 提交前请确保 `cargo fmt --check` 与 `cargo clippy` 通过。

## 提交信息规范

采用语义化前缀，标题使用祈使句、简明描述变更：

- `feat:` 新功能
- `fix:` 缺陷修复
- `docs:` 文档
- `refactor:` 重构（不改变行为）
- `perf:` 性能优化
- `test:` 测试
- `ci:` 持续集成 / 构建
- `chore:` 杂项

示例：`fix(ui): 修复表格列宽拖拽越界`

## 分支与 Pull Request

1. 从 `main` 切出功能分支：`feat/xxx`、`fix/xxx`。
2. 保持提交聚焦、单一目的；必要时可拆分多个提交。
3. 提交 PR 时请填写变更动机、影响范围与验证方式（截图 / 日志更佳）。
4. 合并前需通过 CI，并至少获得一次评审。

## 问题反馈

- 提 issue 前请先阅读 [docs/FEEDBACK.md](docs/FEEDBACK.md)。
- 安全漏洞请勿公开披露，改用 [SECURITY.md](SECURITY.md) 的私下上报流程。
