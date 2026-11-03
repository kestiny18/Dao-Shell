# Dao-Shell 工程清单审查 — 2026-10-27

本轮结论：发现 5 项经隔离探针复现的问题，以及 5 项静态代码/配置可确认的风险或流程缺口。没有证据证明已发生真实密钥泄露、真实费用损失或用户文件丢失。按影响和触发条件排序列出下面 10 项，不把 SaaS 身份、支付、合规功能机械套到本地桌面产品。

Candidate: `41fc4cc0fbacf2202cca9d9e9162cc818a73a41a`。
范围：Windows 桌面/CLI 当前源码、配置、模型传输、凭据、文件打开/移动、会话保存、维护回执、索引权限关键路径、UI及CI/打包配置。
当前源码比公开 v0.1.0 多出能力；本报告不是对公开安装包的重新认证。
授权：仅审查与隔离验证；不修复生产代码、不提交/推送、不安装/更改服务、不调用真实模型或WinGet写操作。
Review: CHANGES_REQUIRED（本轮主审查，未额外启动独立Reviewer）。
Independent testing: NOT_RUN；本轮主审查的定向测试与探针列在后文，不冒充独立验收。
开始时既有改动：`docs/plans/2026-10-17-p15-release-0.1.0.md`已修改；两份2026-10-20发布验证Plan/Review为untracked。它们未被修改。

P1=高：优先修复，触发后可能外发非预期数据、丢失记录或持续阻塞核心操作。
P2=中：特定故障下可用性/费用/发布保障不足。
P3=低：文档或生命周期易用性缺口；不表示可忽略。

## 优先处理的10项

### F01 · P1 · 普通“打开文件”把模型密钥环境变量传给启动程序（已复现）

位置：[src/platform/windows.rs:198](../../src/platform/windows.rs#L198)、[src/platform/windows.rs:212](../../src/platform/windows.rs#L212)；类似旧入口为[desktop/src-tauri/src/app_actions.rs:350](../../desktop/src-tauri/src/app_actions.rs#L350)。

普通打开调用ShellExecuteW，没有提供隔离环境；路径身份复核不能阻止环境变量继承。新的app_launch路径却会通过CreateEnvironmentBlock并排除配置的密钥变量（src/application_actions/native.rs:84），保护没有覆盖旧入口。

触发：通过环境变量配置API Key后，用户或经本地确认的模型打开一个会启动新进程的EXE。该子程序获得父进程环境中的Key；程序自身的诊断日志/插件也可能扩大暴露范围。这里没有声称Windows凭据库里的Key自动变成环境变量，也不把同用户进程当成OS安全隔离边界。

证据：专用无窗口child.exe只检测假变量DAO_REVIEW_FAKE_API_KEY并写入固定夹具回执。调用真实open_document后结果为`file_open_fake_key=inherited`。没有读取或打印真实密钥。
建议：将可执行启动路径统一到明确环境策略，检查文档关联处理程序及旧概览启动入口；不要仅修新app_launch。

### F02 · P1 · “解释电脑概览”仍能搜索文件并把结果发送给模型（已复现）

位置：[src/session.rs:260](../../src/session.rs#L260)、[src/session.rs:280](../../src/session.rs#L280)、[src/dialogue.rs:184](../../src/dialogue.rs#L184)。

ExplainComputer只在自然语言中要求“不调用文件工具”，实际调用通用dialogue.turn，发送完整工具列表并执行模型返回的工具调用。用户点击解释资源概览即可被模型扩展为文件发现，文件名/完整路径进入后续模型请求，超出了该按钮的“只分析已有数据”含义。

证据：本机假模型先返回file_search；在授权的虚构目录发现review-private-marker.txt，下一次模型请求包含此文件名。结果：`overview_explain_tools_exposed=true; searched_file_sent=true; reply_error=false`。没有越过文件Scope，也没有读文件正文；问题是解释入口不应开放这一能力。
建议：复用短时诊断已有的explain_observation无工具单次请求，同时只携带本次允许解释的投影，隔离先前会话文件上下文。

### F03 · P1 · 维护记录进入未知/等待重启后，缺少可达的恢复闭环（已复现状态机，静态核查入口）

位置：[src/software/engine.rs:135](../../src/software/engine.rs#L135)、[src/software/engine.rs:226](../../src/software/engine.rs#L226)、[src/software/store.rs:76](../../src/software/store.rs#L76)、[desktop/src-tauri/src/bridge.rs:278](../../desktop/src-tauri/src/bridge.rs#L278)。

unknown/awaiting_user/awaiting_reboot会阻止后续维护及文件打开/应用动作。Engine::reconcile只有测试调用，实际Store/UI没有提供软件提供者空闲证明及完成核对入口；即使直接传provider_idle=true，awaiting_reboot也永远不离开阻塞状态。重启读取原回执并不会消除这个状态。Startup专项placement恢复已有，但不能代替软件/DNS等恢复。

证据：在临时journal写入虚构awaiting_reboot记录，重新打开Engine并调用idle reconcile后仍`unresolved=true`。未安装/卸载真实软件、未重启机器。
建议：建立真实可核验的重启/提供者空闲及人工核对流程；保持未知结果不重放，不能简单删journal或强制标success。

### F04 · P1 · 两个桌面实例会静默覆盖彼此的会话和草稿（已复现）

位置：[src/workspace.rs:122](../../src/workspace.rs#L122)、[src/workspace.rs:129](../../src/workspace.rs#L129)、[desktop/src-tauri/src/main.rs:244](../../desktop/src-tauri/src/main.rs#L244)。

WorkspaceStore只有实例内Mutex，没有跨进程锁或expected_revision；保存时self.load只验证现有内容格式，随后用旧内存快照替换整个文件。Tauri启动也未配置单实例约束。原子写入避免半文件，不能避免丢失更新。

证据：两个WorkspaceStore读取同一临时文件，A保存新草稿，B保存旧快照，最后A的新草稿消失且B返回成功：`workspace_stale_write_overwrites=true`。不涉及真实workspace。
建议：单实例所有权锁，或带跨进程互斥的revision比较并给出冲突处理。

### F05 · P1 · 历史达上限后整个工作区不能保存，且没有产品内清理途径（已复现边界）

位置：[src/workspace.rs:55](../../src/workspace.rs#L55)、[src/workspace.rs:69](../../src/workspace.rs#L69)、[src/workspace.rs:131](../../src/workspace.rs#L131)、[desktop/ui/app.js:189](../../desktop/ui/app.js#L189)、[desktop/src-tauri/src/bridge.rs:105](../../desktop/src-tauri/src/bridge.rs#L105)。

前端不断messages.push、newSession不断追加会话；关闭标签只移除tab。后端限制每会话2000消息、整个workspace 8MiB和100会话。超出消息/字节上限后任何标签、草稿与新消息保存都失败；重新尝试相同快照无效，重启丢弃未保存内容。100会话是另一终身上限，没有删除/归档入口解除。错误提示主要让人检查磁盘/权限，不能修复内容配额问题。

证据：2000条虚构消息保存成功，追加第2001条后保存被拒绝：`workspace_2001_blocks_save=true`；静态核查没有对应裁剪/归档或删除UI。原有持久化测试验证“拒绝且保留旧文件”，没有覆盖用户恢复闭环。
建议：保存前配额提示、可明确选择的导出/归档/删除流程或分会话存储；不可静默删除用户历史。

### F06 · P2 · 工具次数限制不是模型费用上限（静态确认）

位置：[src/model/transport.rs:122](../../src/model/transport.rs#L122)、[src/dialogue.rs:184](../../src/dialogue.rs#L184)。

请求未设置max_tokens/max_completion_tokens等输出预算；常规对话丢弃返回的usage。已有12次工具调用、128KiB上下文和请求超时，但一次输出或多轮重复大上下文仍按提供商计费，没有累计token/预算提示或本机停用阈值。512KiB响应读取限制也不能约束服务商已生成/已计费token。
风险：用户BYOK下的意外费用；没有执行真实付费请求，实际额度损失无法确认。
建议：提供与提供商兼容的输出预算、usage显示与可选会话/日预算；金额硬限制还应结合提供商账户额度，不承诺取消请求能撤销费用。

### F07 · P2 · WinGet同步目录调用不可取消，可长期占用全局请求槽（静态确认，真实卡住未复现）

位置：[src/software/typed.rs:53](../../src/software/typed.rs#L53)、[src/software/typed.rs:83](../../src/software/typed.rs#L83)、[src/software/typed.rs:112](../../src/software/typed.rs#L112)、[src/software/typed.rs:253](../../src/software/typed.rs#L253)。

catalog.Connect、FindPackages及逐包describe为同步COM调用，search/inspect不接收Cancellation，也没有应用级期限。派发安装后的wait有15分钟预算，不能保护此前目录连接/查询。Bridge在完成前持有请求准入；若COM/网络阻塞，点击停止仅设置标志，新操作和正常关闭等待原请求完成。
风险：特定提供者/网络故障导致应用操作长期不可用。未制造真实WinGet故障，不声称已经测得卡住时长。
建议：将不可中断的提供者调用放入可隔离、可超时回收的执行单元；不能在原任务仍可能运行时直接释放写操作保护。

### F08 · P2 · 打包流程未绑定测试通过的准确候选（流程缺口）

位置：[scripts/package-desktop-windows.ps1:17](../../scripts/package-desktop-windows.ps1#L17)、[scripts/package-desktop-windows.ps1:50](../../scripts/package-desktop-windows.ps1#L50)、[scripts/package-desktop-windows.ps1:72](../../scripts/package-desktop-windows.ps1#L72)、[.github/workflows/ci.yml](../../.github/workflows/ci.yml)。

CI运行Rust/UI测试，但打包脚本自行从当前工作区构建并复制installer到dist，只检查构建/PE/文件/校验和，不运行或验证对应SHA的测试结果，也不拒绝未验证的相关工作区差异。测试CI和可分发产物之间没有仓库内强制绑定。
风险：开发者忘记手工验证时仍能产出分发包。没有证据表明已发布包未测试，GitHub外部发布审批/保护配置无法确认。
建议：从确定源码快照生成制品并保存相同候选的检查证据，将这个约束接入打包/发布交付；保留预览与完整实机验收区分。

### F09 · P2 · CI缺少依赖漏洞扫描与维护反馈（流程缺口）

位置：[.github/workflows/ci.yml](../../.github/workflows/ci.yml)、[Cargo.lock](../../Cargo.lock)、[desktop/src-tauri/Cargo.lock](../../desktop/src-tauri/Cargo.lock)、[desktop/package.json](../../desktop/package.json)。

CI有编译、fmt、clippy和测试，没有cargo audit/deny、npm audit等安全检查；未找到Dependabot/Renovate配置。锁定依赖有助重现，不会告知锁住的版本之后出现漏洞。
证据：本轮单独npm audit报告0个已知漏洞（72依赖），所以不能声称npm当前有漏洞。Rust两份锁文件未做完整实时漏洞扫描；外部GitHub安全功能是否启用无法确认。
建议：对两份Cargo锁和npm锁建立可维护的扫描/更新流程，按实际可利用性处理报告，不机械升级全部依赖。

### F10 · P3 · 用户入口文档低估当前代码能执行的操作（静态确认）

位置：[README.zh-CN.md:35](../../README.zh-CN.md#L35)、[ROADMAP.md:11](../../ROADMAP.md#L11)、[src/software/typed.rs:314](../../src/software/typed.rs#L314)、[src/capabilities.rs:129](../../src/capabilities.rs#L129)。

README描述应用“卸载”只打开Windows设置；ROADMAP仍写没有模型进程终止工具。当前代码已经向桌面模型暴露process_terminate和软件维护工具，并可走UninstallPackageAsync。ROADMAP注明2026-10-15，旧版本描述有历史依据，但它仍是项目入口引用的范围依据，当前源码使用者容易误判能力。
风险：审查者/试用者依据旧范围做授权和试用安排。实际本地确认仍存在，不能把这项说成无确认卸载。
建议：明确区分已发布v0.1.0与当前源码原型，更新入口能力、数据分享和操作风险说明，保留历史文档的版本限定。

## 已检查到的有效保护

- 模型远程HTTPS、禁止URL内凭据与重定向、错误不回显远程body；响应512KiB和工具数量限制。
- Config不保存原始Key；Windows凭据库和端点绑定；配置保存有revision检查。
- 文件授权Scope、文件/父目录身份复核、不覆盖的同卷小批量移动、操作前持久回执。
- Tauri主窗口窄IPC capability与CSP；UI动态内容用textContent，既有恶意名称用例通过。
- 索引服务调用者SID/交互会话校验和候选逐项模拟调用者可见性检查，不能仅因SYSTEM扫描就断言跨用户泄露。
- 模型文字不能自行批准写操作，取消/一次性确认/未知结果不重放有测试。这些保护不消除F01–F05。

## 本轮执行证据与限制

- `npm test --prefix desktop`：75 passed，0 failed。
- `.\\scripts\\dev.ps1 test -CargoArgs @('--locked','--test','workspace_persistence','--test','dialogue_protocol','--test','settings_and_access','--test','maintenance_session')`：22+1+5+3=31 passed，0 failed。
- `npm audit --prefix desktop --json --ignore-scripts`：0已知漏洞；只覆盖这份npm依赖图。
- 本轮临时Rust harness直接依赖当前仓库dao-shell，使用offline Cargo构建及独立临时数据；测试辅助文件在忽略的`.tools/review-2026-10-27/`，输出见[results.txt](../../.tools/review-2026-10-27/results.txt)。它不属于生产源码/正式回归测试，若修复应再提炼成正式回归用例。
- 五个探针的可观察输出：

```text
workspace_stale_write_overwrites=true
workspace_2001_blocks_save=true
maintenance_reboot_remains_blocked_after_idle_reconcile=true
overview_explain_tools_exposed=true; searched_file_sent=true; reply_error=false
file_open_fake_key=inherited
```

没有检查到的内容保持“无法确认”：真实模型质量/计费、真实WinGet写入/故障、SYSTEM服务安装升级与重启、原生屏幕阅读器、干净机器安装、旧电脑/规模负载、完整Rust漏洞库扫描、全Git历史秘密扫描、全部源码逐行审计、外部仓库保护/账户权限、合规合同/法律适用范围。
本轮没有修改生产代码或配置，未自动实施任何建议。只新增本报告及忽略目录下的审查夹具/构建输出。

## 逐项适用性及检查结果

以下沿用原清单顺序，1–147为工程项目，148–160为末尾人员职责。每条都做适用性判断；“不适用”项到此跳过，不提出新增功能。“已检查局部”不是整项通过；未执行的实机/组织检查明确保留。

| # | 原清单项目 | 适用性/状态 | 当前证据与限制 |
|---|---|---|---|
| 1 | a schema someone designed, instead of one the AI improvised table by table | 适用·已检查局部 | Workspace 为显式 v1 schema，索引有 application_id/user_version，持久动作有版本字段；并非无结构数据。多实例保存有 F04。 |
| 2 | login that still works when someone puts a plus sign in their email | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 3 | password reset links that expire and work once | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 4 | MFA, plus a recovery path for the user who lost their phone | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 5 | sessions that time out, and a way to kill every session for one user | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 6 | SSO for the enterprise customer who won't sign without it | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 7 | SAML, because that customer's IT team doesn't do magic links | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 8 | SCIM, so a fired employee loses access the moment IT offboards them | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 9 | roles and permissions deeper than "admin" and "everyone else" | 部分适用 | 按本地权限和会话隔离检查：Scope、会话私有对象引用、管道调用者身份；不要求 SaaS RBAC/租户系统。参见 src/files.rs、src/session.rs、src/file_index/worker/security.rs。完整跨 Windows 账户实机攻击测试无法确认。 |
| 10 | checks that stop user A from reading user B's data by changing one number in the URL | 部分适用 | 按本地权限和会话隔离检查：Scope、会话私有对象引用、管道调用者身份；不要求 SaaS RBAC/租户系统。参见 src/files.rs、src/session.rs、src/file_index/worker/security.rs。完整跨 Windows 账户实机攻击测试无法确认。 |
| 11 | tenant isolation, so one customer's bug never shows another customer's records | 部分适用 | 按本地权限和会话隔离检查：Scope、会话私有对象引用、管道调用者身份；不要求 SaaS RBAC/租户系统。参见 src/files.rs、src/session.rs、src/file_index/worker/security.rs。完整跨 Windows 账户实机攻击测试无法确认。 |
| 12 | an admin "log in as this user" button for support, with an audit trail on every use | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 13 | rate limits on login, so nobody can guess passwords at scale | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 14 | lockouts that stop attackers without locking out your CFO the night before the board meeting | 不适用 | 没有产品账户、登录或企业身份系统；桌面会话不是登录 session。 |
| 15 | database migrations that run forward and roll back | 部分适用 | 索引拒绝旧/未知 schema；workspace 未知版本保留不覆盖。本地缓存可重建；未验证每种升级/降级迁移与数据恢复，不能称迁移验收通过。 |
| 16 | migrations that don't lock the main table at 2pm on a Tuesday | 部分适用 | 索引拒绝旧/未知 schema；workspace 未知版本保留不覆盖。本地缓存可重建；未验证每种升级/降级迁移与数据恢复，不能称迁移验收通过。 |
| 17 | indexes, because the query that was instant on 50 test rows times out on 5 million real ones | 适用·已检查局部 | 索引 names_file 索引、按 row_id/generation 的游标与容量/过期限制已有；搜索没有被证明在 500 万条或第 400 页满足性能要求，本轮未跑大规模负载。src/file_index/store.rs、worker/mod.rs。 |
| 18 | connection pooling, so a traffic spike doesn't exhaust the database | 不适用 | 本地 SQLite/可重建索引，没有中心数据库连接池或线上业务库 PITR 的当前需求。 |
| 19 | backups every night | 部分适用·缺口 | 不可重建的 workspace/动作历史应有备份恢复；目前原子写入只保护单次更新。未找到自动历史备份流程，恢复演练无法确认；不要求给可重建文件索引做 SaaS 每夜备份。 |
| 20 | a restore from those backups, tested, because a backup you've never restored is a hope | 部分适用·缺口 | 不可重建的 workspace/动作历史应有备份恢复；目前原子写入只保护单次更新。未找到自动历史备份流程，恢复演练无法确认；不要求给可重建文件索引做 SaaS 每夜备份。 |
| 21 | point-in-time recovery for the day someone runs a delete without a where clause | 不适用 | 本地 SQLite/可重建索引，没有中心数据库连接池或线上业务库 PITR 的当前需求。 |
| 22 | soft deletes, so "I deleted it by accident" is a support ticket and not an incident | 不适用 | 文件动作没有通用删除能力；软件卸载不能用数据库软删除解决。会话保留/删除策略另见 F05/35。 |
| 23 | validation on the server, not just in the form | 适用·已检查 | 本地 Rust 后端承担校验：deny_unknown_fields、范围/身份/确认校验；不能靠前端可信。已跑相关协议、配置测试；不等于所有参数全覆盖。 |
| 24 | timezones stored right, so the 9am reminder doesn't fire at 4am | 部分适用 | 无提醒调度；事件采用 RFC3339/UTC、界面本地显示，未做夏令时和所有区域设置实机矩阵。src/core.rs、src/storage.rs、src/operations.rs。 |
| 25 | the customer with an apostrophe in their last name | 部分适用·已检查局部 | 参数化 SQL、JSON、DOM textContent 降低引号/HTML风险；界面既有恶意名称用例通过。没有逐字段 emoji、组合字符、RTL 全面验收。 |
| 26 | emoji, accents and right-to-left text in every field | 部分适用·已检查局部 | 参数化 SQL、JSON、DOM textContent 降低引号/HTML风险；界面既有恶意名称用例通过。没有逐字段 emoji、组合字符、RTL 全面验收。 |
| 27 | money stored as integers, because 0.1 + 0.2 doesn't equal 0.3 in floating point | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 28 | currency, rounding and tax rules for every country you sell into | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 29 | a data model that survives your second customer asking for "one small change" | 部分适用 | 检查显式数据结构和版本拒绝机制；第二客户需求未知，不能据此预造字段。 |
| 30 | idempotency keys, so a retried payment doesn't charge the card twice | 部分适用·已检查 | 付款不适用；本地副作用有 RequestControl、一轮一次动作、预写回执和未知结果不重放。恢复闭环缺口见 F03。 |
| 31 | deduplication for the user who clicks submit four times | 部分适用·已检查 | 付款不适用；本地副作用有 RequestControl、一轮一次动作、预写回执和未知结果不重放。恢复闭环缺口见 F03。 |
| 32 | pagination that still works on page 400 | 适用·已检查局部 | 索引 names_file 索引、按 row_id/generation 的游标与容量/过期限制已有；搜索没有被证明在 500 万条或第 400 页满足性能要求，本轮未跑大规模负载。src/file_index/store.rs、worker/mod.rs。 |
| 33 | exports, because sooner or later every customer wants their data in a spreadsheet | 部分适用·缺口 | 已有有限诊断复制/导出路径；未找到会话历史导出入口。无需为无表格业务强加 Excel 导出。F05涉及历史生命周期。 |
| 34 | imports that survive the CSV someone saved in 2009 | 不适用 | 没有 CSV 导入或网络上传入口；本地文件被搜索/打开，不等同服务端上传。 |
| 35 | retention rules, and code that deletes what your contract says you'll delete | 适用·缺口 | 动作日志部分有 7 天保留/手动清理且保留未知/Startup证据；工作区没有可用会话删除/归档，100会话/2000消息/8MiB上限见F05。未确认客户合同保留规则。 |
| 36 | secrets out of the code, out of the repo and out of the AI chat history | 适用·发现问题 | 密钥不在 Config 中，Windows 凭据库及端点绑定已检查；文件打开环境继承见F01。未扫描全部 Git 历史、所有私人日志或外部AI聊天，旧泄露无法确认。 |
| 37 | rotating every key the AI pasted into a config file last month | 适用·无法确认 | 无法确认历史泄露/轮换；新增密钥可保存。删除模型明确保留系统凭据，CredDelete只在测试配置编译，生产无本地凭据清理入口（src/credentials.rs:120、desktop/ui/settings.js:148）；服务商端吊销仍须其控制台完成。 |
| 38 | confirming every package the AI installed exists and is the one you meant | 适用·已检查局部 | Cargo.lock/npm lock 已固定依赖；npm现有测试可运行。没有逐包维护者/发布来源真实性审计，不能断言全供应链可信。 |
| 39 | dependency scanning, because the AI pulled in libraries nobody reviewed | 适用·流程缺口 | 见F09：CI未配置漏洞扫描/升级自动化；本轮npm audit为0已知漏洞。Rust锁文件完整漏洞扫描、维护周期与支持期限无法确认。 |
| 40 | input sanitization everywhere a user can type | 部分适用·已检查局部 | DOM数据用textContent、静态模板innerHTML未插入外部变量；SQLite关键写入参数化；Tauri窄IPC能力+CSP。无网络账户服务时传统CSRF登录场景不适用；不宣称完整OWASP/渗透测试通过。 |
| 41 | protection against SQL injection, XSS, CSRF and the rest of the OWASP list | 部分适用·已检查局部 | DOM数据用textContent、静态模板innerHTML未插入外部变量；SQLite关键写入参数化；Tauri窄IPC能力+CSP。无网络账户服务时传统CSRF登录场景不适用；不宣称完整OWASP/渗透测试通过。 |
| 42 | file uploads that get scanned, size-limited and stored where they can't execute | 不适用 | 没有 CSV 导入或网络上传入口；本地文件被搜索/打开，不等同服务端上传。 |
| 43 | CORS rules that aren't "allow everything" | 部分适用·已检查 | 不是对外HTTP后端；Tauri CSP限制connect-src为IPC，默认本地主窗口 capability，无remote配置。未对全部WebView响应头做运行时抓包。desktop/src-tauri/tauri.conf.json、capabilities/main.json。 |
| 44 | security headers on every response | 部分适用·已检查 | 不是对外HTTP后端；Tauri CSP限制connect-src为IPC，默认本地主窗口 capability，无remote配置。未对全部WebView响应头做运行时抓包。desktop/src-tauri/tauri.conf.json、capabilities/main.json。 |
| 45 | encryption at rest and in transit | 适用·已检查局部 | 远程模型HTTPS且禁重定向，密钥Windows凭据库；workspace和普通SQLite是未加密的本地内容。宿主磁盘加密/ACL实机配置无法确认，不把TLS当作静态数据加密证明。 |
| 46 | least-privilege access for everyone, including the AI agent holding production credentials | 适用·发现问题 | 新app_launch使用清理环境的普通用户CreateProcess；旧Shell打开路径见F01。索引SYSTEM服务有SID/交互登录/逐候选模拟调用者检查；未实机执行服务安装/攻击验证。 |
| 47 | logs that record who did what, without recording passwords, tokens or card numbers | 部分适用·已检查局部 | 有结构化动作回执与最小投影；可见用户输入/模型输出/路径会持久化workspace，无通用敏感信息识别。没有SaaS谁查看哪条客户记录的业务；全机日志/崩溃转储泄露无法确认。 |
| 48 | a penetration test before your enterprise customer's security team runs their own | 条件适用·无法确认 | 没有执行正式渗透测试；本轮为代码审查及五项隔离复现，不能替代企业渗透报告。 |
| 49 | answers to the security questionnaire every enterprise buyer sends, with evidence behind each one | 条件适用·无法确认 | 无法从仓库确认客户合同、法律适用范围、数据驻留或组织访问审查；不为清单自动引入企业系统。 |
| 50 | a disclosure process for the day a stranger emails you a vulnerability | 适用·流程缺口 | 未找到仓库SECURITY.md或私密漏洞报告入口说明；CONTRIBUTING引导公开Issues。是否另有私下渠道无法确认。 |
| 51 | a plan for the day a laptop with production access gets stolen | 部分适用·无法确认 | 无自营生产服务器；开发者GitHub/签名/模型账户凭据设备丢失响应不在已审配置中，无法确认。 |
| 52 | SOC 2, which means policies, evidence and an auditor, not a checkbox | 条件不适用 | 当前产品没有声明医疗、银行卡或受审计企业服务用途；没有证据要求这些功能。未来合同/实际数据用途需另审，不作法律合规结论。 |
| 53 | HIPAA, the moment a single patient record touches your system | 条件不适用 | 当前产品没有声明医疗、银行卡或受审计企业服务用途；没有证据要求这些功能。未来合同/实际数据用途需另审，不作法律合规结论。 |
| 54 | a BAA with every vendor that touches that data, including your AI model provider | 条件不适用 | 当前产品没有声明医疗、银行卡或受审计企业服务用途；没有证据要求这些功能。未来合同/实际数据用途需另审，不作法律合规结论。 |
| 55 | audit trails that show who viewed a record, not just who changed it | 部分适用·已检查局部 | 有结构化动作回执与最小投影；可见用户输入/模型输出/路径会持久化workspace，无通用敏感信息识别。没有SaaS谁查看哪条客户记录的业务；全机日志/崩溃转储泄露无法确认。 |
| 56 | PCI scope, and card numbers kept out of your database | 条件不适用 | 当前产品没有声明医疗、银行卡或受审计企业服务用途；没有证据要求这些功能。未来合同/实际数据用途需另审，不作法律合规结论。 |
| 57 | GDPR deletion requests that reach every table, backup and log | 条件适用·无法确认 | 无法从仓库确认客户合同、法律适用范围、数据驻留或组织访问审查；不为清单自动引入企业系统。 |
| 58 | data residency for the customer whose contract says EU only | 条件适用·无法确认 | 无法从仓库确认客户合同、法律适用范围、数据驻留或组织访问审查；不为清单自动引入企业系统。 |
| 59 | quarterly access reviews someone signs off on | 条件适用·无法确认 | 无法从仓库确认客户合同、法律适用范围、数据驻留或组织访问审查；不为清单自动引入企业系统。 |
| 60 | dev, staging and production environments that match each other | 部分适用·已检查局部 | Rust工具链、锁文件、Windows/Ubuntu CI和打包脚本可追踪；没有云IaC需求。干净Windows/WebView2/权限一致性尚未本轮验证。 |
| 61 | infrastructure written as code, so the setup lives somewhere other than one person's memory | 部分适用·已检查局部 | Rust工具链、锁文件、Windows/Ubuntu CI和打包脚本可追踪；没有云IaC需求。干净Windows/WebView2/权限一致性尚未本轮验证。 |
| 62 | a deploy pipeline that runs the tests before anything reaches customers | 适用·流程缺口 | 见F08，CI有测试但本地打包流程不消费测试证明或锁定已通过CI的候选；无法确认外部发布审批。 |
| 63 | one-click rollbacks, because Friday deploys happen | 部分适用·无法确认 | 索引服务有升级/回滚设计和源码入口；桌面安装包/数据完整回滚未执行，不能承诺一键恢复。 |
| 64 | zero-downtime deploys, because your customers don't have maintenance windows | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 65 | feature flags, so you can ship the code without shipping the feature | 部分适用 | 维护能力已有运行时白名单、Debug夹具隔离；无必须引入通用feature-flag平台的依据。发布能力的版本说明不一致见F10。 |
| 66 | autoscaling for the Monday morning spike | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 67 | a CDN, so the app loads fast in Singapore too | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 68 | DNS, SSL certificates, and renewals that don't expire on a Sunday | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 69 | background job queues, so slow work doesn't freeze the page | 部分适用·发现问题 | Rust工作线程避免直接阻塞WebView；但同步WinGet查询没有应用级取消/期限，见F07。无需因此引入云消息队列。 |
| 70 | retries with backoff, so one flaky API call doesn't become a lost order | 适用·已检查局部 | 模型错误后显式提供直接搜索，不自动重放副作用；索引恢复有退避代码。本轮未实测所有提供商故障。 |
| 71 | a dead-letter queue for the jobs that fail every retry | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 72 | cron jobs that don't run twice the day you add a second server | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 73 | caching, and clearing that cache when the data changes | 适用·已检查局部 | 设置保存清执行上下文、观测/应用缓存有期限，旧文件引用需身份复核；缓存失效全组合并未穷举。 |
| 74 | SPF, DKIM and DMARC, so your password resets don't land in spam | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 75 | bounce and complaint handling on every transactional email | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 76 | storage rules, so you stop paying to keep 2021's temp files | 适用·缺口 | 动作日志部分有 7 天保留/手动清理且保留未知/Startup证据；工作区没有可用会话删除/归档，100会话/2000消息/8MiB上限见F05。未确认客户合同保留规则。 |
| 77 | cost alerts, so a runaway agent loop shows up the same day and not on the invoice (we learned this one for $1,382.59) | 适用·缺口 | 按本机用户BYOK成本控制评估，不是多租户限额。已有12次工具/128KiB上下文限制，但不限制输出token或累计费用，见F06。 |
| 78 | a disaster recovery plan, written down and tested once a year | 部分适用·缺口 | 不可重建的 workspace/动作历史应有备份恢复；目前原子写入只保护单次更新。未找到自动历史备份流程，恢复演练无法确认；不要求给可重建文件索引做 SaaS 每夜备份。 |
| 79 | a second region, if your contract promises uptime one region can't deliver | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 80 | structured logs you can search at 3am | 部分适用·已检查局部 | 有结构化动作回执与最小投影；可见用户输入/模型输出/路径会持久化workspace，无通用敏感信息识别。没有SaaS谁查看哪条客户记录的业务；全机日志/崩溃转储泄露无法确认。 |
| 81 | error tracking that groups the same bug instead of sending you 4,000 emails | 部分适用·无法确认 | 本地安全诊断及阶段错误码已有；本轮未查外部告警、遥测、客服值班和事故执行情况。错误码对用户的恢复路径见F03/F07，不要求云SRE整套系统。 |
| 82 | uptime checks from outside your own network | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 83 | alerts that page a human, tuned until they stop paging for nothing | 部分适用·无法确认 | 本地安全诊断及阶段错误码已有；本轮未查外部告警、遥测、客服值班和事故执行情况。错误码对用户的恢复路径见F03/F07，不要求云SRE整套系统。 |
| 84 | dashboards for latency, error rate and throughput | 部分适用·无法确认 | 本地安全诊断及阶段错误码已有；本轮未查外部告警、遥测、客服值班和事故执行情况。错误码对用户的恢复路径见F03/F07，不要求云SRE整套系统。 |
| 85 | tracing for one slow request across every service it touches | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 86 | an on-call rotation, and a person who picks up | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 87 | runbooks, so the engineer on call knows what "queue depth critical" means | 部分适用·无法确认 | 本地安全诊断及阶段错误码已有；本轮未查外部告警、遥测、客服值班和事故执行情况。错误码对用户的恢复路径见F03/F07，不要求云SRE整套系统。 |
| 88 | postmortems that change something | 部分适用·无法确认 | 本地安全诊断及阶段错误码已有；本轮未查外部告警、遥测、客服值班和事故执行情况。错误码对用户的恢复路径见F03/F07，不要求云SRE整套系统。 |
| 89 | a status page customers check before they email you | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 90 | unit tests written by someone who knows what the logic should do | 适用·执行了相关测试 | 本轮75项UI+31项Rust现有测试通过，五项追加隔离探针证实缺陷；仅虚构夹具。不推断全量测试、独立QA角色或发布验收通过。 |
| 91 | integration tests for every third-party API you depend on | 部分适用·无法确认完整集成 | 产品没有注册/结账；模型协议用本机模拟服务，WebView测试主要jsdom；本轮没执行真实模型、WinGet写操作或原生安装/完整端到端验收。 |
| 92 | end-to-end tests for signup, checkout and the other flows that make you money | 部分适用·无法确认完整集成 | 产品没有注册/结账；模型协议用本机模拟服务，WebView测试主要jsdom；本轮没执行真实模型、WinGet写操作或原生安装/完整端到端验收。 |
| 93 | load testing before the launch email goes out | 适用·无法确认 | 本轮未做规模负载、长时间运行或五年旧电脑测试。代码中有预算不等于性能验收。 |
| 94 | test data that looks like production without being production data | 适用·执行了相关测试 | 本轮75项UI+31项Rust现有测试通过，五项追加隔离探针证实缺陷；仅虚构夹具。不推断全量测试、独立QA角色或发布验收通过。 |
| 95 | regression tests, so fixing bug 41 doesn't bring back bug 12 | 适用·执行了相关测试 | 本轮75项UI+31项Rust现有测试通过，五项追加隔离探针证实缺陷；仅虚构夹具。不推断全量测试、独立QA角色或发布验收通过。 |
| 96 | testing on the five-year-old laptop your biggest customer's ops team still uses | 适用·无法确认 | 本轮未做规模负载、长时间运行或五年旧电脑测试。代码中有预算不等于性能验收。 |
| 97 | accessibility compliance, because the enterprise contract requires it | 适用·已检查局部 | UI有aria-label/角色、IMЕ和部分键盘焦点测试；未用读屏器/原生键盘完整验收，不能声称符合无障碍标准。 |
| 98 | subscriptions, upgrades, downgrades and proration | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 99 | failed-payment retries and dunning emails | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 100 | invoices with the customer's PO number on them, because procurement won't pay without it | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 101 | sales tax and VAT | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 102 | refunds, credits and partial refunds | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 103 | usage metering that matches what you bill, to the cent | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 104 | annual contracts with custom terms your billing logic never planned for | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 105 | revenue reports your finance team can reconcile | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 106 | webhooks that verify signatures | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 107 | webhooks that retry, with a log of every failed delivery | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 108 | a versioned API, so you can change it without breaking your customers' integrations | 部分适用·已检查局部 | 没有公开客户API；内部索引协议和schema有版本验证，真实跨版本服务升级兼容未本轮验证。 |
| 109 | API keys customers can create and revoke | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 110 | API rate limits that protect you without blocking real integrations | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 111 | a plan for the day a third-party API changes its response format without telling you | 适用·已检查局部 | 模型响应格式和大小严格校验，状态提示及可配置端点/模型可替换；未确认真实提供商兼容矩阵和弃用迁移流程。 |
| 112 | OAuth token refresh, because tokens expire at the worst possible moment | 不适用 | 当前无产品计费、银行卡、客户 API、Webhook 或 OAuth 登录/集成。模型服务消费另见 77/116。 |
| 113 | a sandbox where customers can test their integration | 部分适用 | 没有客户集成沙箱产品需求；仓库本机假模型/临时夹具可用于开发，已使用。 |
| 114 | protection against prompt injection in every field that reaches the model | 适用·发现问题 | 系统提示将文件名/工具结果定义为不可信，写操作本地确认有测试；仅解释概览仍暴露工具，见F02。未证明提示注入全面免疫。 |
| 115 | customer and patient data kept out of prompts that leave your network | 适用·发现问题 | 不会默认读文件正文，但文件名/路径也是敏感数据，模型外发见F02；本地搜索结果还会进入后续对话上下文（src/session.rs:471）。没有DLP或私有数据自动识别，不能承诺私人信息绝不外发。 |
| 116 | a model spend cap per customer, so one heavy user can't run up your bill | 适用·缺口 | 按本机用户BYOK成本控制评估，不是多租户限额。已有12次工具/128KiB上下文限制，但不限制输出token或累计费用，见F06。 |
| 117 | evals, so you find out when a model update makes your answers worse | 适用·无法确认模型质量 | 固定协议回归不等于真实模型质量eval；未运行跨版本语义、安全/准确率评价集。 |
| 118 | a fallback for the afternoon your model provider goes down | 适用·已检查 | 无模型可文件名搜索、本地概览；请求失败不静默自动执行。既有相关协议测试通过，不等于提供商故障实机演练。 |
| 119 | a migration plan for the day your model gets deprecated | 适用·已检查局部 | 模型响应格式和大小严格校验，状态提示及可配置端点/模型可替换；未确认真实提供商兼容矩阵和弃用迁移流程。 |
| 120 | prompt and output logs for debugging, minus whatever you're not allowed to keep | 部分适用·已检查局部 | 有结构化动作回执与最小投影；可见用户输入/模型输出/路径会持久化workspace，无通用敏感信息识别。没有SaaS谁查看哪条客户记录的业务；全机日志/崩溃转储泄露无法确认。 |
| 121 | a human approval step for anything the AI does that can't be undone | 适用·已检查局部 | 文件移动/模型打开/应用操作/维护操作本地确认；一轮副作用和确认绑定有测试。未知结果恢复见F03，环境传递见F01。 |
| 122 | pages that load on hotel wifi | 部分适用·无法确认 | UI资源本地打包，模型有10秒连接/60秒总超时；酒店网络、弱网模型/安装WebView2体验未测，WinGet查询见F07。 |
| 123 | empty states, loading states and error states on every screen | 适用·已检查局部 | 既有空/错/忙状态测试通过，未逐屏实机覆盖。F03恢复缺口、F05保存额度后的不可恢复提示仍有问题。 |
| 124 | error messages a customer can act on, instead of "something went wrong" | 适用·已检查局部 | 既有空/错/忙状态测试通过，未逐屏实机覆盖。F03恢复缺口、F05保存额度后的不可恢复提示仍有问题。 |
| 125 | forms that keep what people typed when one field fails validation | 适用·已检查局部 | 设置错误和关闭保存失败有保留/回退测试；会话总量超限后原始快照保留，但新历史无法保存，见F05。 |
| 126 | emails that render in every mail client your customers use | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 127 | notifications people can turn off | 部分适用·无法确认 | 未识别出需要订阅管理的营销/推送系统；当前状态/错误回执不能简单关闭，原生通知全面行为未检查。 |
| 128 | undo | 部分适用 | Startup停用有保留/再次确认恢复；移动、强制结束、软件卸载并无通用undo。不要承诺可恢复未保存文档/软件数据；F03影响恢复闭环。 |
| 129 | keyboard navigation and screen reader support | 适用·已检查局部 | UI有aria-label/角色、IMЕ和部分键盘焦点测试；未用读屏器/原生键盘完整验收，不能声称符合无障碍标准。 |
| 130 | translations, with dates and numbers formatted for each locale | 适用·已检查局部 | 英/中显式key有现有测试，语言切换不翻译用户路径；所有地区日期/数字/RTL和CLI双语未完整验证。 |
| 131 | a back button that does what people expect | 部分适用·已检查局部 | 桌面采用标签导航，关闭保留会话并恢复draft有测试；不是浏览器网页返回栈。 |
| 132 | code a human can read | 适用·无法确认全面质量 | 阅读了本文列明的关键模块；生成的WinRT绑定较大不能机械拆分，未做所有行/所有死代码的全库审计，不凭行数列严重缺陷。 |
| 133 | code review on every change | 适用·无法确认执行 | CONTRIBUTING/workflow规定聚焦改动/保留既有工作/版本证据；GitHub保护规则、每次人工review和分支审批未实时检查。F08只描述仓库内打包缺口。 |
| 134 | a branching strategy, so two people can work without overwriting each other | 适用·无法确认执行 | CONTRIBUTING/workflow规定聚焦改动/保留既有工作/版本证据；GitHub保护规则、每次人工review和分支审批未实时检查。F08只描述仓库内打包缺口。 |
| 135 | monthly dependency upgrades, so you're never two major versions behind | 适用·流程缺口 | 见F09：CI未配置漏洞扫描/升级自动化；本轮npm audit为0已知漏洞。Rust锁文件完整漏洞扫描、维护周期与支持期限无法确认。 |
| 136 | framework upgrades before the old version stops getting security patches | 适用·流程缺口 | 见F09：CI未配置漏洞扫描/升级自动化；本轮npm audit为0已知漏洞。Rust锁文件完整漏洞扫描、维护周期与支持期限无法确认。 |
| 137 | deleting the dead code the AI left behind from its first three attempts | 适用·无法确认全面质量 | 阅读了本文列明的关键模块；生成的WinRT绑定较大不能机械拆分，未做所有行/所有死代码的全库审计，不凭行数列严重缺陷。 |
| 138 | splitting up the 2,000-line file the AI kept appending to | 适用·无法确认全面质量 | 阅读了本文列明的关键模块；生成的WinRT绑定较大不能机械拆分，未做所有行/所有死代码的全库审计，不凭行数列严重缺陷。 |
| 139 | architecture decisions written down, with the reasons | 适用·已检查局部 | MVP、workflow、Plans、devlog已有设计和交接资料；代码与入口文档仍存在F10。人员是否实际履职/持续交接无法确认。 |
| 140 | a README that gets a new engineer running locally in under an hour | 适用·已检查局部 | README/DEVELOPMENT提供构建命令；本机已有工具链且验证通过。没有从零计时验证新人一小时启动承诺。 |
| 141 | documentation for whoever inherits this | 适用·已检查局部 | MVP、workflow、Plans、devlog已有设计和交接资料；代码与入口文档仍存在F10。人员是否实际履职/持续交接无法确认。 |
| 142 | an admin panel for your ops team | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 143 | support tooling, so customer success can answer questions without pinging engineering | 部分适用·已检查局部 | 已有安全索引诊断/回执展示；真实服务故障客户支持闭环无法确认，F03就是用户无法自行核对的现有缺口。 |
| 144 | a changelog customers can read | 适用·已检查局部 | 有CHANGELOG.md；是否每个发行产物均对应完整用户变更说明无法确认，未审全部发行记录。 |
| 145 | terms and a privacy policy that match what the code does | 适用·发现文档偏差 | README披露模型会收到名称/路径/元数据；F10的能力范围文档与当前代码不一致。隐私/合同法律充分性无法确认。 |
| 146 | every account, domain and repo in the company's name, not the name of whoever built the prototype | 部分适用·无法确认 | 仓库注明个人维护者与开源许可；没有公司化前提，不要求换成公司账户。账户实际所有权、备份维护者和人员可用性无法确认。 |
| 147 | a bus factor higher than one | 部分适用·无法确认 | 仓库注明个人维护者与开源许可；没有公司化前提，不要求换成公司账户。账户实际所有权、备份维护者和人员可用性无法确认。 |
| 148 | reading every line the AI wrote before it ships (a senior engineer) | 适用·无法确认执行 | CONTRIBUTING/workflow规定聚焦改动/保留既有工作/版本证据；GitHub保护规则、每次人工review和分支审批未实时检查。F08只描述仓库内打包缺口。 |
| 149 | designing the data model before your second customer breaks it (a backend engineer) | 适用·已检查局部 | MVP、workflow、Plans、devlog已有设计和交接资料；代码与入口文档仍存在F10。人员是否实际履职/持续交接无法确认。 |
| 150 | explaining why the AI's fix for one bug created two new ones (a senior engineer, again) | 适用·无法确认人员保障 | 本轮仅验证代码和隔离行为，不证明有人持续承担故障分析、架构或跨机器兼容责任。 |
| 151 | deploys, rollbacks, servers and the cloud bill (a DevOps engineer) | 部分适用 | 桌面打包/索引服务生命周期适用，自营服务器/cloud bill不适用；发布门禁F08，真实运维执行无法确认。 |
| 152 | answering the page at 3am when payments stop (whoever's on call) | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 153 | patching the framework the week a vulnerability goes public (security) | 适用·流程缺口 | 见F09：CI未配置漏洞扫描/升级自动化；本轮npm audit为0已知漏洞。Rust锁文件完整漏洞扫描、维护周期与支持期限无法确认。 |
| 154 | walking an auditor through how patient data moves (security and compliance) | 不适用 | 当前不是自营云 SaaS，没有相应服务、邮件、支付、分布式作业或企业运营承诺。 |
| 155 | testing what the AI says it tested (QA) | 适用·执行了相关测试 | 本轮75项UI+31项Rust现有测试通过，五项追加隔离探针证实缺陷；仅虚构夹具。不推断全量测试、独立QA角色或发布验收通过。 |
| 156 | running a migration on a live database without losing a row (a backend engineer) | 部分适用·无法确认 | 本地SQLite版本/缓存重建适用，不是线上客户主库迁移；真实升级保持所有数据未全面验收。 |
| 157 | saying no to the feature that breaks three others (a tech lead) | 适用·已检查局部 | MVP、workflow、Plans、devlog已有设计和交接资料；代码与入口文档仍存在F10。人员是否实际履职/持续交接无法确认。 |
| 158 | turning "works on my laptop" into "works for every customer at once" (an architect) | 适用·无法确认人员保障 | 本轮仅验证代码和隔离行为，不证明有人持续承担故障分析、架构或跨机器兼容责任。 |
| 159 | debugging the code when the AI that wrote it can't (a senior engineer) | 适用·无法确认人员保障 | 本轮仅验证代码和隔离行为，不证明有人持续承担故障分析、架构或跨机器兼容责任。 |
| 160 | remembering why it's built this way, a year from now (someone who was there) | 适用·已检查局部 | MVP、workflow、Plans、devlog已有设计和交接资料；代码与入口文档仍存在F10。人员是否实际履职/持续交接无法确认。 |
