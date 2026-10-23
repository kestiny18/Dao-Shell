# 第四期独立测试与代码评审

Date: 2026-10-23
Requested checks: testing / review
Baseline: `5bf53f8f84b9071ba15a485162152d216b782f31`
Candidate: `7884984f1c4a2cfba9a22cdc1a223d7d05a63bae`
Implementation: IN_PROGRESS
Independent testing: INCOMPLETE（已执行的本地隔离检查PASS；必要实机项未验证）
Independent review: CHANGES_REQUIRED

## 独立测试

Tester context: independent，空白上下文，按候选与Plan执行。测试前后HEAD一致，生产源码、既有测试和脚本未修改。新增独立测试作为验证夹具叠加于候选，不冒充候选已有覆盖。

| 检查 | 结果与适用范围 |
| --- | --- |
| 精确身份/source/version/architecture/scope及计划过期 | PASS，模拟provider；独立测试改变5轴身份并拒绝旧计划 |
| unknown不是absent、精确卸载实例、多注册键拒绝 | PASS，确定性断言 |
| 跨用户/机器scope原生不存在判断 | NOT_VERIFIED，代码查询另一scope，缺真实provider证据 |
| 协议摘要绑定、伪造confirmed/command、本地IPC归属和重复/错误确认 | PASS，核心/会话/桌面IPC及WebView断言范围 |
| 拒绝/取消及最终身份检查期间取消 | PASS，无派发 |
| 90秒确认过期 | PASS，独立测试真实等待90.1秒，派发次数0且无回执 |
| provider假成功后核验、持久unknown、重启不重放、awaiting_reboot不因idle解除 | PASS，模拟provider与独立Engine重开测试 |
| Startup移动/恢复、冲突、内容变化、摘要与保留证明 | PASS，Windows独立临时文件夹具 |
| 真实Startup及完整崩溃/重解析点实机矩阵 | NOT_VERIFIED |
| DNS固定调用、拒绝路径、模型投影与历史不保存token/原生计划 | PASS，已有断言范围；WebView首先核对Debug DNS deny latch |
| 真实DNS清理与独立复测 | NOT_VERIFIED；endpoint复测入口未交付 |
| 真实装卸/安装器取消/提升/断网/数据选项/重启 | NOT_VERIFIED |
| 可靠provider空闲核对、人工归档、历史版本/机器scope选择 | 未交付，不以公共Engine原语冒充用户闭环 |
| 第一至三期相关自动回归 | PASS，本轮单次；历史失败未复现，不代表已修复 |

### 命令与证据

证据根目录：`.tools/phase4-independent-7884984/`。

| 实际命令 | 结果 | 日志 |
| --- | --- | --- |
| `scripts/dev.ps1 test -CargoArgs @('--locked','software::','--','--nocapture')` | 20 PASS | software.log |
| `scripts/dev.ps1 test -CargoArgs @('--locked','--all-targets')` | 各target合计271 PASS / 0 FAIL / 4 ignored，含新增独立测试4项；不重复计入上一行 | all-targets.log |
| `node --test desktop/tests/ui.test.mjs` | 74 PASS | ui.log |
| `scripts/desktop.ps1 test` | 24 PASS | desktop-test.log |
| `scripts/desktop.ps1 build` | PASS | desktop-build.log |
| `node .tools/phase4-independent-7884984/maintenance-smoke.mjs` | 8 checks PASS | webview.log |

`file_sessions` 6项全部通过（5.23秒），包括app_list/process_list契约；两个restricted_child IPC与嵌套崩溃恢复测试通过。原先timeout与worker IPC失败本轮未复现，没有证据归因环境或断言已修复。4项ignored为专用原生启动夹具、两个被其他测试调用的helper、手动CPU负载检查，未额外直接执行。

固定自然语言14场景未跑完整真实模型/安装器端到端矩阵；覆盖了其中授权拒绝、unknown、取消、协议、重启占用、Startup冲突和DNS风险边界。没有本轮复现的FAIL，但整期必要实机及未交付项仍未通过。

### 修改范围与身份

- 新增 `tests/phase4_independent_7884984.rs`，4项独立确定性测试。SHA256：`C70FF556B6A40A39BABF7AB8E9DC25D5DB90E68EE7D98C654998539BCBBFB49C`。
- 独立WebView脚本仅替换输出根目录，不改变原脚本验证逻辑。输出 `.tools/phase4-independent-webview-7884984/`（配置、进程身份、日志、WebView数据和evidence）。
- 正常构建输出 `target/`、`desktop/src-tauri/target/`；测试框架使用并清理独立系统临时夹具。
- 桌面Debug exe SHA256：`176DBB7CDD4023453FF5C3D81856FBEDB7FEF315F8F1F9E218D554F63404E05E`，与已交付便携包一致。
- 未执行真实装卸、真实Startup修改、DNS清理、提权、外部模型或环境安装。未提交、推送、发布；主Agent单独归档本报告。
- 既有3项无关文档改动保留；本轮独立验证Plan不改变候选代码。

## 独立代码评审

Reviewer context: independent，空白上下文先读需求/diff，再核对Tester报告。结论 CHANGES_REQUIRED。

### R1 — P1：Startup派发前未保存身份与摘要，移动后崩溃丢失安全恢复依据

位置：[src/software/store.rs:326](../../src/software/store.rs#L326)。派发前receipt只有original/target/name，冻结的 `p.identity` 和 `p.digest` 在移动成功后才写入evidence（330–331行）。Engine先保存dispatching（engine.rs:51–55），再调用移动；若移动已成功但最终保存前崩溃或最终数据库写失败，持久化记录只有缺少身份/摘要的dispatching，重开后变为unknown。

影响：快捷方式已移出Startup，但 `startup_targets()`（store.rs:261–267）仅从verified且有retained/identity/digest的记录创建恢复候选。缺少原始证明就不能安全核对保留文件，产品无法提供恢复入口；unknown又阻塞后续维护写操作。这是已交付Startup路径的具体崩溃缺陷，不能归入已记录的软件provider人工归档缺口。

证据：`store.rs:326,330–331`；`engine.rs:51–55,63–73,157–162`；`application_actions/journal.rs:37–38`；`store.rs:261–267`。主Agent复核并更正Reviewer原报告中最终保存函数的行号（实际157–162，不是176–181）。本项为静态代码证据，未注入实际崩溃，现有测试未覆盖此窗口。

建议修复：移动前可靠保存两端路径及冻结身份/摘要，并提供基于持久证明的只读核对/恢复入口；不得直接把unknown改为verified或自动重放。补充“移动成功后最终写失败/进程退出”的独立夹具回归，再复验。符合Plan第6节禁用前记录身份及内容核验依据、逐项崩溃核对要求。

已检查contract/engine/store/typed、Startup原子移动与防覆盖、DNS固定调用、journal、session/runtime跨工具闩锁、desktop bridge/确认IPC/UI/模型投影，抽查bindings相关ABI调用和来源。未发现其他有充分证据的阻塞问题；不代表全部正确。

Reviewer未修改文件或运行写工作区命令。测试结果与静态finding无冲突。真实provider/软件装卸/安装器/UAC/重启/Startup/DNS和跨scope原生矩阵仍未验证；机器scope/历史版本、可靠空闲核对、归档、DNS endpoint复测仍有未交付项。

## 主Agent结论

本轮隔离测试通过；整体独立测试保持INCOMPLETE，第四期Implementation仍IN_PROGRESS。独立评审CHANGES_REQUIRED，R1需修复并覆盖故障窗口后复验。本轮按“先测试和评审”范围交付结果，未修改生产代码。初始与最终tracked差异SHA256一致：B49D576DEAB0356B466FBF4CC3B8BFDFA87BDBE089BB2F922AA12D946BDBE55F，候选源码和无关文档改动保持原样。后续仅归档验证文档与新增测试，不改变本报告适用候选。
