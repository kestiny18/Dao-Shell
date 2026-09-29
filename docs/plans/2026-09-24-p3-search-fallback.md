# P3 — 统一输入与搜索回退

Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: 用户已授权顺序执行 P2–P6，每5小时检查并续作，全部完成后停用检查；独立验证未触发。
Delivery: 工作区原型与自测记录；本轮未授权提交、推送、发布。

## Goal and Scope
移除输入模式选择；模型优先；无模型及尚未产生实际操作的模型可用性失败时回退文件名搜索，明确提示。

## Non-goals
不推进其他 Plan，不进行历史迁移、发布或后台框架建设。

## Tasks
执行前按可独立验收的行为细化 Task、候选基线和允许修改范围。

## Acceptance
模型正常、无模型、连接失败有覆盖；取消、权限拒绝、正常无结果不回退；部分执行不重复执行；结果归属原会话。

## Decisions and constraints
遵守 [总索引](2026-09-24-desktop-trial-index.md)、AGENTS.md 和 docs/workflow/README.md。
使用隔离夹具；测试与评审绑定最终候选；未验证项如实记录。

## Evidence
待执行。

## Changes to approved scope
None.


## Current execution — P3 then P4
Authorization: 当前用户明确启动 P3、P4；本会话顺序执行，独立测试/评审未触发，不执行 P5/P6、不提交推送或创建定时任务。
Baseline: HEAD 2e02e2d1781212993845db6a4033cd07ab8b5fa4，加已完成 P2 的未提交代码与已有计划文档改动，全部保留。

### T1 — 统一入口及受控回退
- 移除模式选择，统一输入优先使用配置模型；无模型直接按文件名搜索并说明。
- 仅模型可用性故障且尚无工具执行/确认/操作时允许一次只读文件名回退，明确模型不可用及正在尝试文件名搜索。
- 取消、权限拒绝、正常空结果、工具错误或已经部分执行均不回退；不能通过错误文本模糊匹配把权限错误当模型故障。
- 回退使用原输入作为文件名查询，不擅自生成新指令，不执行打开/移动；保留原会话及同一请求取消边界，不重放模型操作。
- 不改变显式候选打开确认、请求隔离与全局单请求机制。
Allowed changes: 必要 file-session/model 错误分类与desktop桥接/UI/测试及README；不扩展P2设置、无新框架。
Self-test: 模拟模型正常/无配置/连接故障，取消/拒绝/空结果/已执行工具后失败，跨会话归属；相关Rust/DOM、fmt/Clippy、构建检查。

## P3 implementation evidence
- Version: HEAD 2e02e2d + P2 + 本轮P3工作区。实现/自测 READY/PASS，独立两角色 NOT_RUN。
- 统一输入走Say。无模型/初始化不可用按原输入本地搜索；ModelUnavailable区分连接/超时/响应读取/429/5xx，只有本轮零工具调用包装UnavailableBeforeTools才回退。401/403/400、协议异常、取消和任何工具调用后失败均不回退。
- 不解释回退输入或自动操作，UI提示可用简短文件名关键词；原会话及确认/取消边界保持。
- Coder自测：dialogue_protocol 14、model_setup 8、bridge 3、DOM 29全部通过；核心/桌面Clippy all-targets、fmt、JS语法、diff检查通过。
- 修改：src/session.rs、dialogue.rs、model.rs、model/transport.rs，tests/dialogue_protocol.rs；desktop app/index/style/ui.test/README。未改计划或P2配置逻辑。
- 原生UI、真实模型、安装包及真实60秒超时未实测；本地HTTP故障、连接失败和等待中取消用模拟验证。最终可执行文件与P4合并构建后记录，不沿用P2产物声称含P3。

## Final build — 2026-09-29
P4完成后统一desktop build成功，包含P2/P3/P4当前工作区。产物SHA256：3B1E6A2D5BA7E8A0A9C28D105C57F7665AD63888A2D1A740DC3D71995B1850AB；主Agent核对一致。P4未修改P3执行逻辑，最终共享DOM回归30/30；P3其余未变代码沿用上述自测证据。无提交推送，独立验证仍NOT_RUN。
