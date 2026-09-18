# Changelog

记录已经发生的变化；尚未完成的计划放在 [Roadmap](ROADMAP.md)。当前没有稳定版发布。

## Unreleased — 第一版公开开发预览

### 2026-09-18

- Compact overlapping search roots, add multi-term relative-path matching and relevance ordering, clear numbered selections on empty or failed searches, and reuse unchanged file references.
- Share formatted resource facts between terminal output and model explanations; clarify API-key errors and same-volume move limits.
- Add first-run model onboarding: provider presets, hidden key input, session-only or Windows credential storage, connection repair in place, and search-directory selection before entering chat.
- Make the default README English-first, add a separate Chinese README and bilingual getting-started guides, and translate contribution templates.
- Align the interactive prompt with `dao >`; keep the command `daosh`, package `dao-shell`, and Chinese CLI messages.
- 根据用户 Windows 实测改进配置：交互式 setup、保存前校验、原子替换、CMD 引号与密钥非法字符提示。
- 增加显式模型连接诊断，验证工具调用和结果回传；固定样例不包含本机文件或资源信息。
- 修正 Windows 进程 CPU 首次采样偏低；显示整机口径、合理小数位和可用内存。
- 空搜索保留上一组候选并提示，新增 `/results`，修正快捷入口与自然语言之间的编号不一致。
- 文件路径、大小和时间改为用户可读格式；打开请求不再输出原始 JSON，退出与终端命令误输有明确反馈。
- 记录首轮真实模型查找与打开反馈，完整场景和小批量移动验收继续保留为待完成。

### 2026-09-17

- 将用户侧可执行文件和终端命令统一为 `daosh`，同步 Windows 打包、烟雾测试和使用示例；项目名与 Cargo 包名保持不变。
- 增加英文仓库入口和中英文文档导航。

### 2026-09-16

- 实现自然语言工具调用循环，以及文件查找、打开、小批量移动和资源观测。
- 为文件修改加入本地确认、逐项结果、未知状态和重启后核对。
- 保留无需模型的直接搜索入口。
- 通过 21 个本地测试、Clippy 检查和本机便携包入口检查；真实模型和干净 Windows 环境验收尚未完成。
- 将命令行参数、显示与交互流程分开组织，补充开发、体验和贡献文档。
- 开始公开开发记录，分享范围取舍、发现的问题和下一步验证。
- 项目采用 Apache License 2.0，补齐许可证与贡献说明。
