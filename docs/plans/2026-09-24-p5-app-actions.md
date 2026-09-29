# P5 — 应用操作

Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: 用户已授权顺序执行 P2–P6，每5小时检查并续作，全部完成后停用检查；独立验证未触发。
Delivery: 工作区原型与自测记录；本轮未授权提交、推送、发布。

## Goal and Scope
可靠启动目标的普通运行、管理员运行、打开位置；卸载转入 Windows 系统卸载界面。

## Non-goals
不推进其他 Plan，不进行历史迁移、发布或后台框架建设。

## Tasks
执行前按可独立验收的行为细化 Task、候选基线和允许修改范围。

## Acceptance
双击与运行一致；无可靠目标动作不可用并说明原因；提权取消不报成功；核验目标身份；不执行任意注册表卸载命令，不开放模型自主卸载。

## Decisions and constraints
遵守 [总索引](2026-09-24-desktop-trial-index.md)、AGENTS.md 和 docs/workflow/README.md。
使用隔离夹具；测试与评审绑定最终候选；未验证项如实记录。

## Evidence
见下方交付证据。

## Changes to approved scope
None.


## Current execution — P5
Authorization: 当前用户明确开始P5；只实现和自测，不执行P6、不提交推送、不创建定时任务或独立验收。
Baseline: HEAD 2e02e2d1781212993845db6a4033cd07ab8b5fa4 加P2–P4未提交实现及已有计划文档改动，全部保留。

### T1 — 交互与操作
- 应用双击与右键“运行”一致；菜单包含运行、以管理员身份运行、打开文件位置、卸载。支持键盘入口；不可用动作显示具体原因。
- 启动目标必须有可靠来源，不把DisplayIcon中任意exe（可能是卸载器/安装器）自动当应用启动入口，不拼接注册表命令或调用shell解释字符串。
- UI只提交后端绑定的应用/动作标识，不能自行提供任意路径、参数或命令；执行前重新核验目标身份，失效拒绝并要求刷新。
- 管理员运行明确由用户动作触发，保留系统UAC，取消不能报成功。普通运行结果只表示请求已交给系统，不宣称应用已经成功运行。
- 打开位置仅定位可靠文件/目录。卸载只打开Windows系统已安装应用页面，由用户在系统中选择确认；不执行UninstallString、不传任意URI、不向模型开放此入口。
- 全局操作互斥及明确状态反馈，快速重复触发不重复派发；刷新后旧目标引用不可复用。

### Implementation / self-test
允许必要computer应用目标采集/身份校验、Windows平台窄执行器、Tauri命令权限、home UI和相关测试/README；保持进程内结构，不扩成通用程序执行服务。
Coder先确定可靠启动目标来源并报告；来源缺失时显式不可用，不猜测可执行文件。
使用临时虚构文件、模拟系统执行器验证双击/菜单、无效目标、替换目标、取消/失败、重复触发及固定卸载URI；不启动真实应用、实际提权或卸载。运行受影响Rust/DOM、fmt/Clippy及最终桌面构建。原生UAC与实际启动未测时须注明，真实使用前建议相关验证，不自动派发角色。

## P5 prototype delivery — 2026-09-29
- Implementation READY；Self-test PASS；Independent testing/review NOT_RUN。
- 双击运行，右键/Shift+F10/Menu键打开运行、管理员运行、打开位置、卸载菜单；方向键与Escape可用，不可用动作说明原因。
- 后端生成应用引用，概览刷新使旧引用失效；共享全局互斥，UI不传任意路径/命令。卸载只进入固定 ms-settings:appsfeatures，回执仅表示系统交接。
- 可靠目标限制：唯一同名开始菜单.lnk、固定本地磁盘EXE、无参数、工作目录为空或目标父目录。拒绝命令解释器、明显安装/卸载/维护入口、网络与重解析路径。DisplayIcon不作为启动依据。枚举最多4096条、8层，Store/便携应用及带参数或自定义工作目录的入口通常不可用。
- 执行前重验快捷方式、EXE和父目录身份并保持只读共享句柄至系统交接；取消在交接前检查，UAC取消独立状态，不宣称实际应用启动成功。
- Coder自测：desktop test 10/10（6应用边界+4桥接），DOM 33/33；fmt、Clippy all-targets -D warnings、JS语法、diff检查与desktop build通过。临时虚构EXE/.lnk及mock执行器覆盖真实COM保存/解析、参数拒绝、身份替换/句柄锁、取消/失败、旧引用、重复触发与菜单分派。
- 变动：desktop Tauri新app_actions.rs及命令权限、bridge/main/build/Cargo；desktop app/home/style/UI tests/README。Cargo无版本升级，仅已有依赖关联。原有P2–P4修改保留。
- 候选：HEAD 2e02e2d1781212993845db6a4033cd07ab8b5fa4 + 当前P2–P5未提交实现；最终desktop/src-tauri/target/debug/dao-shell-desktop.exe SHA256 00E10847B55D55CF3D584469E696DAFE7927FB307B1F482678528CE01474F6C6，主Agent核对一致。
- 未验证：原生菜单视觉、真实软件覆盖、实际启动/定位/系统卸载设置页/UAC。测试未真实启动、提权或卸载应用。真实使用会运行所选本机程序、管理员动作可能获提升权限；建议先用专门测试应用验证目标对应、取消及系统交接，再用于真实软件；不自动派发独立角色。
- 当前仅原型交付；未提交推送，未启动P6。
