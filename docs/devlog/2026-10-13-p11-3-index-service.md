# P11.3 — 固定 Windows 索引服务生命周期

范围：P11.3 Coder 实现与自测；主 Agent 在额度中断期间临时完成 runtime/volumes/lock、CLI 与清理逻辑，随后交回同一 Coder。保留 P9/P10/P11.1/P11.2 未提交改动；没有提交、推送、服务安装、UAC 或真实卷 Journal 变更。主 Agent 独占 Plan。

Implementation: IN_PROGRESS（可进入后续集成的代码候选，真实服务生命周期尚未验证）。Independent testing: NOT_RUN。Independent review: NOT_RUN。

最终冻结候选：`target/release/dao-index-worker.exe`，SHA256 `2D38C50973D9212808AAE28E2673D87F4BF2E13CDEB7082D8B59BC08C32AA01B`。这是未提交工作区候选；哈希仅对应本轮 Windows release 可执行文件，不能把旧 P11.2 真实结果归给此构建。

## 行为与边界

- 固定服务名 `DaoShellFileIndex`，LocalSystem、自动启动、SCM dispatcher / stop / shutdown / preshutdown。程序来自 Known Folder ProgramFiles 下 `DaoShellIndexService/<SHA256>/dao-index-worker.exe`；带引号 ImagePath 仅运行 `service-run`。数据在 Known Folder ProgramData 下同名产品目录。没有任意安装路径、服务名、脚本、SQL 或命令入口。
- 安装、升级、修复必须提供提权前冻结的 `--expected-sha256`。只读取运行中 EXE 的身份绑定源句柄，拒绝目录、重解析点、多硬链接、超界文件；内存 bytes 快照核验固定 SHA 后才写受保护版本目录，目标再次核验。不是提权后自行计算 hash 再信任同一来源。正式签名未实施。
- 程序/manifest 仅安装用户 SID 可读/执行，BA/SY 可写；索引目录仅 BA/SY。安装用户取真实 elevated token，并核对同 session 桌面 shell 的 token SID；无 shell 或跨账户代装拒绝。普通客户端不提供 owner SID override。
- manifest 记录 current/previous/pending/recover_to/uninstalled。停止超时禁止切换、删服务或删数据。新版本必须通过实际受认证控制管道、协议/版本/owner 和至少一个卷初始化检查；不把 SCM Running 当成索引健康，也不等全盘首建。失败回滚保留两版及明确恢复目标；新建服务安全配置失败会删除本次服务，初次健康失败先停止再删除服务，保留受保护恢复记录。
- 升级、修复、回滚只读检查独立 SQLite schema header；不迁移 P9/P10。卸载默认删除程序、保留索引及同用户可重装记录；`--delete-index` 仅卸载允许，先验证固定范围、owner/ACL/文件类型及白名单，再逐文件删除。无递归任意路径删除。
- 所有生产服务命令在打开报告文件前拒绝 global `--report`。SCM 入口不输出控制台 JSON。实验 `serve/diagnostics` 的既有报告机制保持独立。
- 服务端以实际管道 impersonation token 核对安装 SID、有效交互 session；游标/取消仍绑定 PID、SID、session、AuthenticationId。普通客户端通过 SCM 固定服务名、LocalSystem 配置、运行 PID、受保护 manifest/EXE hash、进程映像和前后 PID 核验服务端。没有放宽 SYSTEM process/token ACL；SYSTEM 身份依据是 SCM，而非声称普通客户端读取了 SYSTEM token。
- 自动发现启动时在线固定 NTFS 卷，GUID+serial 绑定存储和管道；最多 16 个卷 worker，其余明确未覆盖。卷初始化先核对 DB/source/Journal、建立管道，再标 `worker_started`。卷错误/panic 只更新对应 catalog 的静态 worker_error 并撤去该 pipe；其他卷及健康控制仍可使用。`supported` 只表示采集资格，不代表已发布完整索引。
- 每卷独立取消；控制管道失败或 SCM 停止才取消全部卷。停止统一 join；线程异常不静默当成功。正常工作中的覆盖、代次、更新时间和维护错误仍通过每卷有界 Status 返回。所有卷的数据库、WAL/SHM/journal/诊断日志均按身份排除，正常侧文件消失不误当存储损坏。

## 自测证据（2026-10-13）

使用项目 `scripts/dev.ps1`，普通权限及独立虚构文件；没有运行生产安装入口。

| 命令/范围 | 结果 |
|---|---|
| `test --locked --lib file_index` | 41 PASS / 0 FAIL / 1 ignored，5.36s；ignored 崩溃 helper 由恢复用例实际子进程调用 |
| 最后线程隔离改动后的 `test --locked --lib file_index::worker` | 23 PASS / 0 FAIL / 1 ignored，5.28s |
| 新增真实管道身份回归 `file_index::worker::security::service_identity_tests` | 1 PASS；真实 token SID/session/AuthenticationId 匹配，伪造 body SID 不能改变身份，错误安装 SID 被拒绝 |
| `test --locked --bin dao-index-worker` | 1 PASS；管理命令 report 在任何文件创建前拒绝，缺 expected SHA 解析失败 |
| `clippy --locked --lib --bin dao-index-worker --tests -- -D warnings` | PASS |
| `fmt --all -- --check` | PASS |
| `git diff --check` | PASS；只有仓库现有 CRLF 提示 |
| `build --locked --release --bin dao-index-worker` | PASS，最终增量构建 16.76s |

固定 release 普通只读冒烟：`service-status` 返回 `not_installed`；`discover-volumes` 返回 7 个卷，其中 5 个 eligible、0 个 worker_started，仅列出 OS 卷信息，未启动卷采集。`service-run --report <独立未存在夹具路径>` 返回 2 且文件未创建，未进入 SCM dispatcher。没有安装或启动服务。

模拟 SCM 用例覆盖停止超时不切换/删除、健康失败回滚、回滚停止失败留下 pending 后修复、current/previous 正常互换、缺服务修复、未知映像拒绝、新建服务安全配置失败清理，以及首次健康失败删除本次服务。安装文件用例覆盖虚构硬链接源拒绝、恶意路径/恢复版本/SID 拒绝、旧 schema 只读拒绝。卷线程回归真实注入错误与 panic，确认 join、静态错误、不取消其他卷。既有真实普通 IPC/ACL、SQL 取消、重启、崩溃恢复、回执快照绑定继续通过。

## 尚未验证 / 后续必需项

- actual SCM 安装、LocalSystem/session 0 读卷、普通 split-token 客户端验证 SYSTEM 进程映像、程序/数据生产 ACL、required privileges 裁减、自动开机、升级/修复/卸载实机：NOT_VERIFIED。当前候选只配置 `SeImpersonatePrivilege`（SCM 保留其必要系统权限），不宣称最低权限已验证。
- 发现只在服务启动时执行；卷热插拔/离线恢复、Journal 缺口后的显式重建及持续恢复尚未完成，留 P11.5；不会创建或重建真实卷 Journal。1178/1179 仅静态标为 `journal_unavailable`，不猜永久不支持。
- 完整覆盖原因标签仍待 P11.4 协议/界面接入；当前 per-volume complete/phase/error 不代表全部记录可用。首次服务初始化健康只证明至少一卷端点已建立，不代表首建已发布。
- P11.2 v4 于 06:49:39 明确被用户取消 UAC，没有进程或真实 USN 续读证据；不重新弹同一请求。P11.2 的有界重试/跨天真实验证仍 NOT_VERIFIED。
- 已知约 54 万记录首次 228.6s、更新 15.8s、旧 CLI 查询 p95 约 244–311ms；原 30s/2s/200ms 目标不变，仍有差距。本轮未再次全盘重测同算法。
- 桌面启用/模型 fallback/Objects 与 Scope 处理尚未接入，此记录不宣称 P11.4 完成。

## 固定 CLI（实机操作需主 Agent 协调，本轮未执行）

普通只读：`dao-index-worker.exe service-status`、`service-health`、`discover-volumes`。服务安装后，用返回的 GUID+serial key 执行 `service-volume-status --volume-key <key>`、`service-search --volume-key <key> --query <虚构夹具名称>`。API 复用 `service::request_volume`，没有 caller-supplied server EXE/DB。

固定管理员生命周期：`install --expected-sha256 <冻结SHA>`、`upgrade --expected-sha256 <冻结SHA>`、`repair --expected-sha256 <冻结SHA>`、`rollback`、`service-start`、`service-stop`、`uninstall [--delete-index]`。不接收 `--report`；返回码及普通 SCM/health 用于读取结果。默认卸载保留数据；显式 delete-index 不影响源文件。
