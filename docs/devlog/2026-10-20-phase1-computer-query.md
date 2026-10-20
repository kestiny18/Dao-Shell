# 第一期电脑查询原型交付

Date: 2026-10-20 (Asia/Shanghai)
Implementation: READY
Self-test: PASS (local Windows prototype)
Independent testing: NOT_RUN
Independent review: NOT_RUN
Cloud-model natural-language E2E: NOT_VERIFIED

## 版本与范围

- 开发基线：`f36cee668f7e927403054fec1e12d7ed40e5d9fa`，不是设计引用的历史 `c7780ab`。
- 已批准方案与开发授权：`1c8e8c6`。
- 核心策略/观测库：`4537c1c`。
- 最终功能候选：`3a5052fa19e019b472694482ba894a0170f92185`。本记录后的文档提交不改变代码。
- 授权：用户批准第一期开发及小步本地提交；没有推送、合并、发布、安装组件或操作应用的新增授权。本轮未执行这些动作。
- 方案：[第一期设计](../plans/2026-10-20-phase1-computer-query-design.md)。

交付五个桌面只读工具：`resource_snapshot`、`process_list`、`network_snapshot`、`app_list`、`computer_profile`。CPU/内存双榜复用同次采样，磁盘独立查询不等待 CPU；应用普通登记查询不取图标、不采进程，running 视图只确认保守的独占完整路径关联。缺失指标不伪装成零，无匹配应用不伪装成未安装或未运行。

桌面保留三个文件工具，CLI 原有六工具及命令语义保持兼容。模型不能启动/关闭/结束程序、安装软件或调用任意命令。原有概览显式本地应用动作没有扩展到聊天工具。

## 关键边界

共享开关为 ask/enabled/disabled，旧配置缺字段按 ask；与文件访问权限独立。Runtime 在采集前检查，模型说自己已经获得许可不会改变设置。概览解释也检查同一开关并经过投影。设置保存继续使用版本检查与请求互斥，重建模型/候选/观测上下文，保留可见历史。

本地电脑查询无需模型或共享许可。它只产生本地事实；模型需要明确工具调用才能读取已允许共享的白名单数据。共享开启时可传入本地观测目录的 ID、类型及时间作为后续指代线索，而不是自动发送原始事实。

观测库每会话最多 12 组、8 MiB、10 分钟；每组最多保存 2,048 个进程用于分页，超限报告局限。游标绑定会话观测和完整筛选/排序/页大小；原始行引用不随排序/筛选变号。应用目录 60 秒、配置 5 分钟缓存保留原采集时间和原始时钟，不通过复用滑动续期；运行证据和 uptime 不当作静态缓存。

模型投影采用白名单重建，排除 PID/native identity、命令行、环境变量、主机名、IP/MAC、完整程序路径、图标和操作引用；最大 32 KiB，通过完整行裁剪并标记，不截 JSON 字节。外部字符串限制长度/控制字符，UI 用 textContent 渲染。

普通电脑查询不清空文件候选，文件查询不覆盖电脑观测。观测事件与最终 Reply 沿用 session_id/execution_id 归属、按观测及页去重。取消阻止未完成样本的发布和后续模型调用；已经完成的事实卡可以保留。同步 OS 调用仍只有软预算，不能承诺强制中断。

重启只恢复可见文本回执和原采样时间，不恢复原始 JSON、全量清单或活引用。无模型/模型失败不再把整句话静默当文件名搜索；界面保留明确的文件名搜索及五类本地电脑查询入口。

## 自测证据

| 检查 | 结果与适用范围 |
| --- | --- |
| `scripts/dev.ps1 test --locked --all-targets` | PASS；本轮实现的核心、CLI、文件/配置/协议及既有索引回归；既有 ignored 负载/辅助用例未自动启用 |
| 最终 `scripts/dev.ps1 test -CargoArgs @('--locked','--lib','--test','dialogue_protocol','--test','file_sessions')` | PASS：lib 131 passed / 2 ignored，dialogue 17/17，file_sessions 5/5；覆盖最终投影、领域复用与桌面会话改动 |
| `scripts/dev.ps1 clippy ... -D warnings` | PASS；核心 all-targets |
| `scripts/desktop.ps1 test` | PASS，22/22；既有请求互斥、配置竞态、打开确认、动作身份与服务边界 |
| `npm test`（desktop） | PASS，68/68；新增授权卡/概览绕过拒绝、事实事件去重、纯文本注入防护、本地入口、错误卡不请求授权 |
| `scripts/desktop.ps1 clippy -CargoArgs @('--all-targets','--','-D','warnings')` | PASS，最终候选 |
| core/desktop fmt check、`git diff --check`、JS syntax | PASS |
| `scripts/desktop.ps1 build` | PASS，最终 Debug 原型；未打安装包 |
| 隔离真实 WebView + 固定本地 HTTP 模型 | PASS，16 项；最终 `d` 轮，15 次请求全部发往独立本机模拟模型，无云模型/凭据 |

新增确定性检查包括：网络计数回退/新增未知、严格参数/伪造引用、排序筛选后稳定行引用、分页绑定/跨会话拒绝、数量/字节/年龄淘汰、取消前不采集、缓存时间不续期、模型投影预算与身份/路径排除；协议用例覆盖模型伪造授权、拒绝采集、工具后模型错误保留本地结果、自然语言错误不隐式文件搜索。原有同窗双榜及应用精确路径保守关联检查继续通过。

WebView 16 项：首次授权卡、允许后单卡去重、原采样追问、跨文件保留、进程分页、刷新产生新 ID、模型错误保留卡、取消不发布未完成样本、关闭共享清空引用、关闭后五本地工具可用、会话归属、无缺失文案键、只保存文本回执、重启不恢复活卡、无模型保留本地查询、中英文标签。

本地证据在忽略目录 `.tools/phase1-query-smoke-20261020d/evidence.json`，截图为同目录 `computer-cards.png` 与 `computer-cards-zh.png`。这些截图含本机只读采样，未提交到仓库。较早 `b` 轮保留 FAIL：所有业务断言通过后，测试 harness 等待已关闭 WebView 的回包超时；改成先返回再关闭，`c/d` 新轮均通过，不回改旧证据。

## 试用

最终 Debug 文件：`desktop/src-tauri/target/debug/dao-shell-desktop.exe`。

SHA256：`8F8C8B81B22C0318391CD18245B63B602E24A2F8D7BED03CD9A2391D94DBBE50`。

先以新建隔离目录试用固定模型和虚构文件；脚本会自行建立配置/模型/数据目录并关闭自有桌面：

```powershell
node scripts/computer-query-smoke.mjs .tools/my-new-computer-query-trial desktop/src-tauri/target/debug/dao-shell-desktop.exe
```

手动无模型试用可使用另一个新目录：

```powershell
New-Item -ItemType Directory -Path .tools/manual-computer-query -ErrorAction Stop
[IO.File]::WriteAllText((Join-Path $PWD '.tools/manual-computer-query/config.json'), '{"read_roots":[],"computer_query_sharing":"disabled","ui_language":"zh-CN","model":null}', [Text.UTF8Encoding]::new($false))
$env:DAO_SHELL_DESKTOP_CONFIG = (Resolve-Path .tools/manual-computer-query/config.json).Path
& .\desktop\src-tauri\target\debug\dao-shell-desktop.exe
Remove-Item Env:DAO_SHELL_DESKTOP_CONFIG
```

直接用下方“查看电脑状态”查询资源/进程/网络/应用/配置。真实模型试用时在隔离配置中添加自己的连接，启用电脑信息共享后重试问题，例如“内存谁占得最多”→“CPU 呢”→“现在重新看看”。此连接会收到所说明的白名单事实。

## 局限与未验证项

真实云模型的工具选择、十个自然语言问题包和解释质量保持 NOT_VERIFIED。不同 Windows 版本、其他账户/权限、干净机器、长期压力、极大真实应用登记清单以及 OS 同步调用的最坏延迟未验证。现有本机测试和固定模型不能替代这些证据。

注册表应用、可读进程和短采样均有提供者范围；不声称完整列出所有窗口/Store/便携应用、不主动探测联网、不判断根因。第二期应用动作、后台监控和维护功能没有实施。没有启动独立 Tester/Reviewer，本记录仅为 Coder 自测交付。
