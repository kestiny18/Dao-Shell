# P1 — 工作区与会话验收

Plan / Task: [P1 T1–T3](../plans/2026-09-24-p1-workspace-sessions.md)
Verification mode: FULL
Baseline: 3484a37ab14251a46f99a0adfca0c80f818f5ce6（开始时干净）
Final candidate: candidate-03；2026-09-28 完成独立测试与评审。
Candidate manifest SHA256: 517A67E229A2CA2B5DF2861D2345C772734327C5CEE58D1F844760DBB2D1CDC2
Environment: Windows / PowerShell；隔离配置、临时文件与本地模拟模型。

## Test verification

Tester context: independent；p1_tester_resume 和 p1_test_c03 均以 fork_turns="none" 启动。未完成的旧运行时检查不计验收。

| Criterion | Result | Evidence |
| --- | --- | --- |
| AC1 | PASS | DOM 宽度边界、折叠恢复；原生侧栏折叠与重启 |
| AC2 | PASS | 唯一标签、关闭保留再开、仅概览、设置单实例 |
| AC3 | PASS（自动化及已验原生范围） | Enter/Shift+Enter/isComposing/keyCode229；原生概览发送创建会话 |
| AC4 | PASS（已验范围） | 设置可访问名称；680px 客户区、16 个长标题会话、侧栏及标签滚动原生布局 |
| AC5 | PASS | 草稿消息隔离、bridge/file_sessions 跨会话候选拒绝；candidate-03 延迟保存交错及失败恢复 |
| AC6 | PASS | 延迟取消完成前不删除、晚到/错误会话事件拒绝、旧取消不影响下一请求；R01 修复后 12 种交错通过 |
| AC7 | PASS | 错会话确认、停止/过期/重放、配置变更失效；保存等待中确认及 context_reset 归属正确 |
| AC8 | PASS | DOM 保存恢复、删除及保存失败回滚；原生重启恢复布局/标签/消息/草稿 |
| AC9 | PASS | 恢复不 perform，无能力 ID；schema 拒绝嵌套确认和 tool 角色；重启不重放 |
| AC10 | PASS（限定范围） | 损坏/未来版本/保存失败/超限保护、遗留部分写入文件不影响有效记录；未注入真实断电 |
| AC11 | PASS | Rust、bridge、fmt、Clippy、build；最终 DOM 25/25、独立临时交错 8/8、JS 语法、diff 检查 |

初轮独立 Tester 实际检查：core all-targets 70 PASS / 1 手动 CPU ignored；新增 workspace_persistence 3 PASS；bridge 2 PASS；根/桌面 fmt、Clippy、desktop build、JS 语法与 diff 检查通过。初轮 DOM 12 PASS / 1 FAIL，设置命名修复后 candidate-02 独立复验 13/13 PASS。

最终 candidate-03 独立 Tester 命令及结果：
- `npm test --prefix desktop`：25/25 PASS。
- `node --test .tools/p1-audit/tester-c03-independent.test.mjs`：8/8 PASS。
- `node --check desktop/ui/app.js`：PASS。
- `git diff --check`：PASS（仅行尾提示）。
- 前后核对 143 个清单文件及完整 staged / unstaged patch：全部匹配。

正式独立测试增量：desktop/tests/ui.test.mjs 三用例、tests/workspace_persistence.rs 三用例。R01 另由 Coder 增加 12 项回归，独立 Tester 执行并检查。最终 Tester 另建临时 8 用例，覆盖关闭/删除 × 保存成功/失败 × 回落 A/设置，并交错 context_reset、确认开关、结果；检查草稿、消息、回滚、重启不重放。临时文件 SHA256：9587156B5D323AF64BEA06D9FA3042FE755027036994F38D157E8A51821C2497。首次临时测试因相对路径错误未加载 jsdom，修正夹具路径后通过，不属于实现缺陷。

最终 Tester 未修改生产、正式测试或报告。Rust 与桥接在 candidate-03 未变，沿用早先证据；Coder 修复后已重新构建桌面，主 Agent 核对产物哈希。

## Review

Reviewed candidate: .tools/p1-audit/candidate-03
Comparison baseline: 3484a37ab14251a46f99a0adfca0c80f818f5ce6
Reviewer context: independent；p1_review_0928 以 fork_turns="none" 启动，复验保持其独立上下文。
Conclusion: NO_BLOCKING_FINDINGS

- P1-T01（P2，AC4）：折叠后设置按钮仅有符号名称。Coder 添加 aria-label/title；独立 DOM 与原生控件树确认“设置”，已关闭。
- P1-R01（P2，AC5/AC6）：关闭/删除 B 改 active=A 后等待保存，DOM 仍属 B，A 后台事件把 B 草稿写入 A。candidate-02 只读复现，评审曾为 CHANGES_REQUIRED。Coder 将草稿捕获绑定实际呈现的 renderedTab，并在等待保存前同步切换界面，失败同步回滚。candidate-03 独立 Tester 与 Reviewer 复验通过；同一复现中 A/B 草稿均保持正确，已关闭。

Reviewer 初轮完整检查生产差异、新 workspace 模块、独立测试、桥接与 Tauri 命令/权限、RequestControl、FileSession、设置和概览周边逻辑。最终重新检查整个 app.js，包括切换、新建、关闭/删除、保存队列及回滚、请求事件/完成、确认、设置失效、窗口关闭及新增测试；其余已审文件哈希未变。先独立读代码，再核对 Tester 证据，结论一致。没有新增阻塞发现或范围扩展，Reviewer 无文件修改。

## Native verification

主 Agent 使用 Computer Use 的 node_repl + @oai/sky；原生证据不冒充独立角色检查。

- .tools/p1-native-fixture/config.json 仅使用本地固定响应模型和虚构文件。
- candidate-01：概览发送创建会话、搜索返回两项、新建第二会话不带入草稿；关闭第一标签仍留侧栏，重开草稿/候选保留。
- 重启：两个会话、标签顺序、当前标签、草稿及搜索路径历史恢复；上下文失效提示出现，旧候选操作按钮不恢复。
- 折叠图标栏、设置标签、Alt+F4 正常退出后保存布局和历史。
- candidate-02 原生控件树确认设置名称。
- .tools/p1-layout-fixture 的 16 个虚构长标题会话：Windows 系统菜单缩到最小宽度（截图含边框 682px，客户区约 680px）。侧栏滚动、标签横向滚动、长标题截断及完整可访问名称、消息/草稿/发送区均正常，无页面横向溢出；正常宽度和折叠模式亦检查。
- candidate-03 仅调整草稿捕获归属与关闭/删除的渲染时序；原生布局证据沿用，受影响的时序与恢复行为由独立自动化复验。

原生测试产物：candidate-01 EXE SHA256 5E9CB7A83C4EB54EFA3AE076F36D2E41780A47DD828440E5E2ED5AE892A5D632；candidate-02 为 AEF9907A109A088FF7A53834970A8FADDB971F78C6AA3D8DB27B398C7D530F05。

## Candidate changes and revalidation

不入库快照 .tools/p1-audit/candidate-01、candidate-02、candidate-03 保存完整 staged / unstaged 差异、新文件内容、HEAD 与文件哈希。

- candidate-01 manifest：B61E55E6B936321F3505BC868D8F73C661F2F49C41FB98B2F1D769D8859E8BAB。
- candidate-02 manifest：68E77A91177534F068BBC84949C3E4E820A3D1725D7783677C7E067E00B8A0D0。相较 01 增加独立测试、设置名称修复和报告。
- candidate-03 manifest 见本文顶部；相较 02 执行文件仅 app.js 与 ui.test.mjs；其余为三个 Plan/报告文档。Tester、Reviewer 均核对候选一致。
- 最终交付仅更新本报告、P1 Plan 与执行索引的非执行状态/证据，生产和测试保持 candidate-03；不机械重跑未受影响测试。
- 最终 debug 产物：desktop/src-tauri/target/debug/dao-shell-desktop.exe；SHA256 9A24F2B65869EED6DB9E0EEE83BC646288B4F0ADB1645E6CE82660F4252A334D。来自 candidate-03 执行代码，构建成功并由主 Agent 核对。

## Remaining gaps

- 中文 IME 仅验证 isComposing/keyCode229 自动化事件，未逐输入法原生实测。
- 未实际断电；中断写入通过残留部分写入文件和原记录保护验证。
- 上述为验收证据的适用限制，无已知未处理阻塞。真实模型质量、干净机器、安装包验证不属于本轮 P1。

## Orchestrator decision

Status: DONE
Reason: T1–T3 必需验收在注明范围内通过，FULL 独立 Tester/Reviewer 完成；T01/R01 均已关闭，无角色越界或候选不一致。
Delivery state: 工作区代码、Plan、验收报告及本地 debug 构建；未提交、推送、发布或安装。P2–P6 未启动。
