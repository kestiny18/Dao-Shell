# P11.2 Windows 持续索引辅助进程

日期：2026-10-12。角色：Coder 实现与自测，主 Agent 协调实机实验。范围仅 P11.2；没有安装服务、接桌面、历史数据迁移、提交或推送。

Implementation: IN_PROGRESS（实现级自测完成，等待真实卷建库/续读结果）  
Independent testing: NOT_RUN  
Independent review: NOT_RUN

## 实现边界

- `dao-index-worker serve` 在前台持续维护一个 Windows 卷，普通客户端通过本机命名管道使用 Status/Search/Cancel。SCM、SYSTEM/session 0、自动发现卷属于后续 Task。
- 协议版本 1，消息最多 64 KiB，查询最多 1024 字节，每页 1–50 项，请求期限 50–3000 ms。4 个管道实例，最多 3 个活动查询，为控制请求留位置；未发送请求/未确认回复的连接在 5 秒期限后释放。
- 生产管道 ACL 仅 SYSTEM、管理员、当前用户特定读写权限；普通客户端没有创建服务端实例权限，拒绝远程，首实例独占。实际 pipe token 的 SID/session/PID 决定身份，不使用请求自报身份。客户端核对服务端实际 PID/token/elevation/image。P11.2 只接受同 SID/session。
- 每个候选在调用者 impersonation 下逐段检查目录列举权限，保留祖先句柄且不共享删除，拒绝祖先重解析点，核对最终文件身份。过滤前路径、候选计数和数据库行游标不进入 IPC。分页令牌为服务器保存的随机 UUID，绑定 principal/query/generation，30 秒有效、最多 128 个；多取一个可见结果才产生继续令牌。取消也绑定实际 principal。
- SQLite 写入与只读查询使用不同连接，WAL 保留已发布快照。查询 progress handler 在 SQL VM 内检查 deadline/取消，无命中全表扫描也能被中止。
- 无名称变化时不改代次、不写数据库。内存保存完整协调成功后的观察游标，绑定卷/root/generation；下一有意义事务一并持久化，失败不越过未协调事件，重启允许从持久游标重放。每轮刷新数据库/WAL/SHM/诊断日志的完整文件身份排除；其他卷文件不属于当前源。
- 首次完整性统计加载目录图并缓存父链结果，避免每个文件重复多层 SQL；循环与深度边界保留。Windows DeletePending/零链接返回明确不可用，不伪称存在或猜测删除。
- 已声明的名称采样竞态先按单个身份最多协调 3 次，每次计入预算并检查取消；仍失败则整事务回滚，维护层最多 6 轮退避（250、500、1000、2000、4000、4000 ms），公开 retrying，耗尽后 blocked。未知 source 错误或 Journal 缺口仍明确 error/rebuild_required，需要重启处理；不自动修复离线卷或日志缺口。
- `Status.published_unix_ms` 仅在真实发布事务设置；重启/no-op 保留原时间。实验 schema 2 的 JSON 字段用 serde default 兼容早期实验记录，旧记录时间未知为 null；不迁移 P9/P10 格式。

## 存储与实验 CLI

Volume 模式必须提权，数据库仅允许 Known Folder API 返回的 ProgramData 下 `DaoShellIndexExperiment/<ASCII实验名>/index.db`。不信任环境变量，不修改仓库 ACL。专用目录必须是管理员/SYSTEM owner、受保护的仅 BA/SYSTEM 完全控制 DACL；陌生目录、异常 ACL/owner/重解析点/文件硬链接直接拒绝，不接管。持有祖先/目录/主数据库句柄避免替换。

进程 TokenOwner 设置为 Administrators，使新 SQLite 文件不被普通 split-token 用户凭 owner 身份改 ACL。重启检查目录白名单内数据库/WAL/SHM/journal/`worker-diagnostics.jsonl` 的 owner、ACL 和单一硬链接。日志也纳入同卷身份排除。日志最多写 128 条/次运行，仅在发布或出错时写入，记录阶段耗时、采集计数与详细错误；它受相同目录保护，不由 IPC 返回。

```powershell
& $worker storage-location
& $worker serve --volume E: --db C:\ProgramData\DaoShellIndexExperiment\p11-example\index.db --pipe p11-example --max-resolutions 1000000 --run-for-seconds 900 --report E:\isolated-output\new-report.json
& $worker status --pipe p11-example --server-exe $worker
& $worker search --pipe p11-example --server-exe $worker --query fictional-fixture-token
```

以上是实验入口，不是安装命令。报告必须绝对路径/create_new，不能覆盖已有文件。运行期限仅 CLI 实验使用；省略时持续运行。普通权限 fixture 模式使用独立轮询适配器验证 IPC，不能代替 Windows USN 证据。

## 候选版本与实机证据

1. 早期普通权限 IPC 固定 exe SHA256 `07FB31534EC15914BBA2402C6459D8C97ECC5F92382ECA1B421A585FBC598200`。主 Agent 的 `.tools/p11-20261012/live/search-{before,renamed-added,link-deleted,final-link-deleted,restarted}.json`：中文父目录/硬链接改名、新增、删除单链接、删除最后链接、结束后同 DB 重启续读正确，代次 1–5。每次等待约 650 ms 后查询；不是严格延迟统计。错误 server image 被拒绝，随后正常 Status 成功；未知 version 999 零返回关闭，随后正常 Status 成功。
2. 首个受保护 release SHA256 `5DBB4527EFADB0C7CE73D1B99F9273BD7593C900D45332AEA44B84FD46DEFDB6`。真实 UAC 启动后失败 `fixture outside volume`：E 源误把 C 上的存储文件当作同卷排除。未发布快照，不算成功。随后修复为真实 volume serial 判断，普通 C 夹具/E metadata 源回归通过。
3. 修复 release SHA256 `59BB0AB2FD903377FD1C0A88149680946D7D0A1921FD2275DCDF2F87ECCFE03A`。主 Agent 于 20:42:19 启动 PID 7788，900 秒上限。普通客户端核验提权服务端并获得 building；普通 `File.Open` 数据库返回 UnauthorizedAccess（不是文件不存在）。证据 `.tools/p11-20261012/volume-worker-v2/status-initial.json`。20:43:36 后仍 building，首次可查询 30 秒目标明确未达到；此时 CPU 约 60 秒/工作集约 30 MB。建库中 CLI 查询约 70 ms 返回无已发布代次，不能当作查询命中性能或 UI 指标。主 Agent 在建库期间新增三名称硬链接夹具等待 catch-up。全卷发布、续读与端到端耗时待补。

## 自测

v2 至少发布了 generation 1/2，但随后维护进入 `phase:error,error:request_failed,generation:2,complete:false`，CPU 约 152 秒、峰值工作集约 73 MB。不能把这种状态交接为维护成功。新增固定只读诊断命令 `diagnostics --experiment <ASCII名> --report <create_new报告>`，只读取 Known Folder 实验目录中经 owner/ACL/身份核验的固定日志；最多读 1 MiB/1024 条，仅导出白名单数值/布尔值、预定义阶段标签、Win32 数字，不导出原始错误或候选名称。

主 Agent 获取 `.tools/p11-20261012/diagnostic-worker/diagnostics-v2.json`：首次建库 **228611 ms**，枚举 **542928**、metadata lookups **543676**、metadata 总耗时 **151593520 µs**，multi-name files **32862**、unavailable **33**、generation 1/complete false；下一更新 **15769 ms**/generation 2；随后 127 ms 失败明确 `retry_required: incomplete link set`，没有 Win32 code。首次 30 秒、变化 2 秒目标均未满足，保留差距供 P11.5 优化。

依据该证据加入上述有界重试，仅允许链接数量变化、链接集合不完整、重开路径身份变化，以及已有身份句柄后特定名称/父路径查找返回 Win32 2/3。初始 OpenFileById 的 2/3 仍是 Deleted；5/32/87 不被笼统标记可重试。未知 USN、schema、日志缺口不重试。注入回归证明耗尽后名称和内存/持久游标均不变，下一轮完整协调成功才推进。

诊断候选 release SHA256：`A3B3AA08611A5B8DDC15A288BB3178F2F8779D745BA95FA9A7A91E76609B9959`。新增脱敏单测 PASS；该版本 clippy 含 tests、fmt check、release 构建 PASS。

使用 `scripts/dev.ps1` 和独立 tempfile 夹具。覆盖原 P11.1 正确性、旧 schema 保护、事务回滚、硬链接生命周期、跨卷存储身份、WAL 快照、持久发布时间、内存游标失败/重启重放、SQLite 无命中扫描期限/主动取消、真实 IPC 认证/不透明游标/增量删除/重启、拒绝列举父目录隐藏结果及无继续令牌、取消 principal 边界、无请求连接释放。

```powershell
.\scripts\dev.ps1 fmt --all
.\scripts\dev.ps1 test --locked --lib file_index
.\scripts\dev.ps1 clippy -CargoArgs @('--locked','--lib','--bin','dao-index-worker','--tests','--','-D','warnings')
.\scripts\dev.ps1 build --locked --release --bin dao-index-worker
```

最终相关自测 **26 passed, 0 failed, 1 ignored**（5.44 秒）。ignored 是崩溃辅助入口，由实际恢复测试在独立子进程中显式运行：BEGIN IMMEDIATE 修改 names/state 后 exit 23 绕过析构，再启动 worker 恢复原已发布代次/时间/结果；该恢复测试 PASS。clippy 含 `--tests -D warnings`、fmt check、release 构建均 PASS。Cargo.toml 的新增文件末尾空行经 diff check 发现并已移除。

有界重试最终候选 `target/release/dao-index-worker.exe` SHA256：`6501851030375C4F2107B000D3B526395B83B856EDC7E1F19DB14F1B6564FAF3`。包含内存观察游标、诊断 CLI 与阶段限定重试。等待 v2 退出后主 Agent 用同 DB 重启验证真实 USN；不可同时运行两个 writer。

早期失败均保留事实：旧分页测试把 no-op 当发布已修正为真实改名；新增控制通道测试曾保留无人接收的闲置 pipe 实例造成自测超时，修正测试后通过；真实删除短暂 pending 现在明确分类不可用。最终候选另补 no-op 比较前仍核验源身份/名称数上限，建库未发布时 Search 返回 `index_not_ready`；SQL 中断返回 `cancelled`。

## 2026-10-13 续记：receipt 与重启旧快照

候选 `3B2842F697F0C8189256B33425A25A5C0F49DF8E911789A0BD7F6C1DD09236D2`，仍为 `target/release/dao-index-worker.exe`。搜索成功回执的 generation/complete/published_at 来自实际 SQLite 查询快照，而不是开始接收请求时较早的 Shared 状态；运行 phase/error 仍来自运行状态。真实 IPC 回归刻意设置较早 Shared generation/时间，断言返回数据库实际代次/完整性/时间。

重启若有已发布快照，初始化状态为 catching_up 并恢复其 generation/时间，后台续读时仍可查询旧快照。多页结果使用上一页实际 generation 的 SearchCursor，后续事务遇新代次会失败为 stale_cursor；错误分支不暴露已累计的局部 hits。现有核心分页回归覆盖发布后下一页失效。

10-13 对受影响 worker 范围重新验证：**9 passed, 0 failed, 1 ignored**（5.27 秒，ignored 仅崩溃子进程辅助入口）；clippy 含 tests/-D warnings、fmt check、release 构建 PASS。实际产物哈希再次读取一致。此前 v3 `650185...` 的 UAC 未产生进程/报告，真实有界重试验证 **NOT_VERIFIED**，不能将冻结产物当成已实测。

主 Agent 用 v4 复用原受保护数据库尝试跨天 USN 验证。10-13 06:49:39 原生 UAC 明确返回“操作已被用户取消”，证据 `.tools/p11-20261013/volume-worker-v4/launch-error.json`；没有进程运行，不计真实验证通过。本轮不重弹该请求。保留 journal/volume 校验，不因旧日志缺口静默跳游标或自动重建。主 Agent 已记录执行顺序调整：保持 P11.2 实机 NOT_VERIFIED，允许 P11.3 实现与隔离自测先行；不表示 P11.2 完成。

后续结果：v4 RunAs 于 06:49:39 返回系统错误“操作已被用户取消”，没有 helper 运行，也未改动受保护数据库；`.tools/p11-20261013/volume-worker-v4/launch-error.json` 为证据。本轮不重复弹出该提权请求，最新有界重试/真实跨天续读仍 NOT_VERIFIED。为继续已授权且不需提权的工作，主 Agent 在 P11 中记录次序调整并开始 P11.3 实现与隔离自测；此安排不把 P11.2 标记 READY。

## 未验证与限制

- 跨账户、不同会话、SYSTEM 身份、服务安装/升级/卸载：NOT_VERIFIED，后续 Task。
- 初次约 54 万记录 30 秒、稳定变化 p95 2 秒、桌面查询 p95 200 ms：未达成证据，不能用局部 API 或普通夹具代替。
- 本次权限收紧测试是真实普通用户目录 ACL，但不等于跨账户或提权全盘覆盖测试。
- 非预定义竞态的源错误或重试耗尽需重启；未加服务自动恢复。核心 Search 不鉴权，必须经过 worker 过滤，不能直接暴露为公共端点。
- 当前 CLI 分页令牌绑定实际进程 principal；预期桌面长生命周期客户端可复用，分开的 CLI 进程不能接续另一进程令牌。
- 当前公共协议只显示 complete:false/coverage_incomplete，尚未返回有限原因标签；原始覆盖原因仍只在受保护核心状态。主 Agent 将原因标签列为 P11.3/4 的协议完善项，本次冻结不追加该字段。
