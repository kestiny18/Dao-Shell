# 第四期维护：本地集成与隔离自测

Date: 2026-10-22; Updated: 2026-10-23
Implementation: IN_PROGRESS（整期未就绪）
Self-test: PARTIAL（维护子范围通过；全量回归未通过）
Independent testing: NOT_RUN
Independent review: NOT_RUN

## 范围与版本

用户已批准第四期实施及小步本地提交；另一台 Windows 测试机器只能由用户操作。本轮继续交付本地能完成的实现、确认链路、模拟/临时目录自测和人工测试说明。没有推送、发布或安装系统服务。

前序第四期提交：`41d2fdd` 授权记录、`23b5c96` 类型契约和 Startup 身份移动、`eef7461` 原生 adapter 与可靠派发。
本轮核心提交 `8f6ad80`，桌面集成提交 `529b185795ee04e6255eb4eee948ecb9f53e391c`。后续 `42246a4` 加入跨范围缺失核对和多实例拒绝，`9d36170fbeba9770f45d4bc40a378c08ac690fd5` 加入 Debug DNS 禁派发闩锁和测试脚本等待上限。最终二进制对应后者；以下受影响范围自测按表中说明适用，较早桌面/UI记录适用于未改变的界面范围；全量失败记录来自整合过程，不以较早通过记录替代最终全量验收。本记录不代表已发布的 v0.1.0。

Debug `desktop/src-tauri/target/debug/dao-shell-desktop.exe` SHA256：`176DBB7CDD4023453FF5C3D81856FBEDB7FEF315F8F1F9E218D554F63404E05E`。
只读探针 `target/debug/examples/phase4_readonly.exe` SHA256：`ADF828867ACB26EB8C96C246EF5F5446D6A4557028F4D88567EA5E7E17DF1BD5`。

## 已实现

- 独立的 `software` 契约、类型化 WinGet adapter、会话 opaque 引用及严格参数校验；安装身份、版本、范围、协议摘要必须固定。引用5分钟、确认90秒；确认后的身份复核/日志写入耗时也计入执行前的有效期。
- 桌面七个维护工具及本地目录来源授权、软件查询、Startup 候选、固定 DNS 缓存维护、回执查看入口。不接模型或关闭共享时仍能本地操作；模型侧共享检查与目录联网授权分开。
- 专用维护确认 IPC，绑定 session/execution/configuration 和一次性 token；通用打开确认不能批准维护，有协议时必须通过本地勾选及匹配摘要。
- 同轮一次写操作；派发前可靠保存回执，包含前置状态、本地确认来源、上下文和协议摘要。取消/拒绝不派发；最终回执保存失败保留未知语义；崩溃只恢复为 unknown，不重放。
- 未解决的维护结果阻塞后续写操作，覆盖模型工具、本地会话及旧概览的启动/结束进程入口。
- Startup 只移动当前用户受支持 `.lnk`，保留身份和 SHA256；恢复必须匹配原始回执、不得覆盖同名文件。保留目录先固定、标记用 create_new 且读取有界；恢复证据不会被七天清理自动删除。
- DNS 只从 System32 加载固定 dnsapi 的固定函数，无模型命令、服务器或网卡参数。回执区分 API 返回与网络连通性，取消流程未调用该函数。
- 中英文维护卡片和回执；正文用 textContent。完整协议、原生路径、确认 token 和引用只在临时界面状态中，不写会话历史；模型投影使用白名单。
- 修复已有 context_reset 分支引用未定义 `s` 的错误，避免上下文清理中断并遗留维护引用。

## 隔离检查

测试使用模拟 provider、虚构软件身份及临时目录快捷方式；没有在工作机装卸软件、修改真实 Startup 或清理真实 DNS。

| 检查 | 命令（项目脚本封装 Cargo） | 结果 |
| --- | --- | --- |
| 维护契约/引擎/启动项/投影 | `scripts/dev.ps1 test -CargoArgs @('--locked','--lib','software::')` | 20 passed |
| 桌面模型协议 | `scripts/dev.ps1 test -CargoArgs @('--locked','--test','dialogue_protocol')` | 22 passed |
| 维护会话拒绝与伪造引用 | `scripts/dev.ps1 test -CargoArgs @('--locked','--test','maintenance_session')` | 1 passed，无原生写调用 |
| 桌面 Rust | `scripts/desktop.ps1 test` | 24 passed |
| 界面 | `node --test desktop/tests/ui.test.mjs` | 74 passed |
| 严格静态检查 | 核心和桌面 `clippy --all-targets -- -D warnings` | PASS |
| 格式与 Debug 构建 | 两入口 fmt、桌面 build、只读 example build | PASS |
| 隔离 WebView | `node scripts/maintenance-smoke.mjs` | 最终候选8 checks PASS |

最后一次 WebView 证据：`.tools/maintenance-smoke-1792671105801/evidence.json`；UI 输出：`.tools/phase4/ui-tests.txt`。这些是本机开发证据，未作为独立验收报告。

**全量回归仍有失败**，没有放宽断言或增加生产超时来掩盖：

- `core-tests.txt`：整合过程 `file_sessions` 的 `five_local_read_only_tools_have_timestamped_contracts_and_no_action_references` 返回 `app_list: timeout`，该组5通过/1失败。
- `core-tests-retry.txt`：随后全量尝试在既有索引 worker IPC/预算/受限子进程恢复测试出现6项失败（cancelled、budget_retrying、子进程计数不符），核心库170通过/6失败/3忽略；后续组未继续。
- `file-sessions-serial.txt`：单独串行复测仍为5通过/1失败，该用例这次为 `process_list: timeout`。
- 失败原因尚未确认，不能直接归因环境负载，也没有证据证明这些失败由第四期改动引入。维护子范围通过不等于第一至四期整体回归通过；最终候选的全量回归保持 NOT_PASS，后续需单独定位。

WebView 拒绝流程已完成7项检查：无模型/关闭共享仍有本地维护；伪造 approved 参数不能进入确认；DNS 方案显示不可恢复影响；通用确认 IPC 不能批准维护；本地拒绝返回 cancelled_before_dispatch；无派发回执；workspace 不保存 token/原生方案。

该 WebView 试用只证明本地拒绝链路，不证明 WinGet、真实 DNS 清理、安装器、UAC 或重启结果。开发实现和自测由 Coder 起步、主 Agent 接续；没有独立 Tester/Reviewer。

## 阻塞、限制与用户试验

1. 本机原生 PackageManager 激活返回 `0x80073D54`，结构化 CLI export 也不可用。原生接口实现通过编译/模拟测试，不等于真实软件装卸闭环可用。CLI 可运行也不能充当原生提供程序的成功证据。
2. 本地发现目前只有当前用户范围、目录默认版本、最多20个候选；机器范围和任意历史版本选择尚未交付。状态不可靠时停止，不把缺失条目当作 absent，也不自动接受来源协议。
   启动项操作须先在本地维护面板查询候选；普通 `startup_list` 的观察引用不能直接作为写操作引用。DNS 复测也须单独通过诊断目标授权，不自动联网复测。
3. 软件异步操作的可靠空闲核对和人工归档入口尚未交付；未知/待交互/待重启保持阻塞。不能删除日志绕过，不能仅凭版本恰好一致推断上次动作成功。
4. 真实安装/更新/卸载、安装器继续运行后的取消、权限提升拒绝、崩溃与重启核对、真实 Startup 登记变更和 DNS 缓存状态都为 NOT_VERIFIED。跨 Windows/WinGet 版本、真实模型选工具、无 WebView2、新机器与系统整体验收也未覆盖。
5. 第二期正常关闭应用仍 BLOCKED；本轮维护不改变其状态。

下一步由用户按[另一台机器人工验证说明](../testing/phase4-other-machine.md)先运行只读预检，反馈 native.available/HRESULT 和环境版本。原生接口不可用时停止软件写测试，再决定该环境的受支持接入方式；不自动安装/重新注册 AppInstaller、提权或扩大下载来源。

第四期继续保持 IN_PROGRESS。实现子范围、开发自测、用户实机试验和独立验证分别记账，不能把上述缺口折算为整期 READY。

## 2026-10-23 收尾

- 当前用户范围未发现软件时，额外查询另一安装范围；另一范围存在或查询不可靠时不得认定整机 absent。多个不同注册键不作为单一安装实例。
- Debug WebView 试验先验证 DNS 禁派发闩锁；闩锁只能拒绝真实 API 调用，不能伪造成功。正式人工测试启动器不设置此变量。清理进程核对 PID、创建时间和可执行文件路径。
- 保留 `.tools/maintenance-smoke-1792672413658/evidence.json` 的连接失败；本轮 `.tools/maintenance-smoke-1792721418706` 因连接重试超出预算中止，未执行原生写操作，专属进程已清理。测试脚本改为墙钟60秒上限和单请求1秒超时，不改变产品超时。

最终交付代码候选：`62bec9bb8195b0a5ef69cbbba61999c18ebd21e2`（最后一项提交仅修复测试启动脚本，二进制仍对应9d36170）。最终 WebView `.tools/maintenance-smoke-1792721647832/evidence.json`：8 checks PASS，含 DNS 禁派发保护核对。前两次本轮试验失败原因为启动输出管道未关闭，已保留失败 evidence；修复通过文件回传进程身份、忽略继承输出并限制启动等待，不改产品超时。

便携开发测试包：`.tools/phase4-trial-62bec9b.zip`；内含文件SHA256与精确候选 manifest。整包SHA256另见同名 `.zip.sha256`。包只做CRC、清单和启动脚本语法检查；尚未在另一台机器运行。
