# 使用反馈指南（如何提交高质量问题）

感谢使用 **wallpaper-rs**！为了让反馈能被**快速定位、复现和修复**，请在提交 issue 前花 2 分钟阅读本指南。

## 反馈入口

- **新建问题 / 建议**：<https://github.com/shuying-code/wallpaper-rs/issues/new/choose>
  - 🐞 **Bug / 问题反馈** —— 可复现的功能异常、崩溃、报错
  - 💡 **功能建议** —— 新功能或改进
  - ❓ **使用咨询 / 反馈** —— 用法、配置、一般性反馈

## 处理节奏

- 维护者**每周集中处理 / 回复一次** issue。
- 能复现的会标记优先级并排期；缺少关键信息的会标注 `need-repro` 并请你补充。
- 请**不要重复提交**同一问题；补充信息请直接在原 issue 里追加评论。

## 提交前检查清单

- [ ] 已搜索 [现有 issues](https://github.com/shuying-code/wallpaper-rs/issues?q=is%3Aissue)，没有重复。
- [ ] 已查阅 [README](../README.md)。
- [ ] 使用的是**最新版本**（见 [Releases](https://github.com/shuying-code/wallpaper-rs/releases)）。
- [ ] 已移除 **密码 / 隐私数据**等敏感信息。

## 需要提供的关键信息（越全越好）

| 项目 | 说明 | 示例 |
|---|---|---|
| 版本 / 构建号 | Release 文件名 / commit | `v0.9.1 · abcdefg` |
| 运行方式 | GUI / 源码构建 | `GUI（Windows 可执行文件）` |
| 系统与架构 | OS + CPU 架构 | `Windows 11 x64` |
| 复现步骤 | 一步一步，别人照做能重现 | 见下方示例 |
| 预期结果 / 实际结果 | 应该发生什么 / 实际发生了什么 | 可附截图 |
| 日志 / 报错 | 终端或界面完整报错文本 | |
| 出现频率 | 必现 / 偶现 | `必现` |

## 敏感信息脱敏

提交前务必替换：密码、token、内网域名/IP、真实数据等，用 `xxx` 或示例值代替。

## 安全问题（漏洞）

请**不要**在公开 issue 中披露安全漏洞。请使用 GitHub 仓库 **Security → Report a vulnerability** 私下上报。

## Issue 标签与处理流程

| 标签 | 含义 |
|---|---|
| `bug` | 已确认为缺陷 |
| `enhancement` | 功能建议 / 改进 |
| `question` | 使用咨询 |
| `need-repro` | 信息不足，等待补充 |
| `duplicate` | 与已有 issue 重复 |
| `wontfix` | 暂不处理（附原因） |

---

反馈越具体，修复越快。谢谢你的支持！🙌
