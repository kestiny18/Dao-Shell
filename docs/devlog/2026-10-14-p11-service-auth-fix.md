# P11 实机 finding：普通客户端不能打开 SYSTEM 进程

Implementation: READY（此认证 finding 的实现、自测及普通 CLI 实机连接范围）。Independent testing/review: NOT_RUN。

主 Agent 已获本次用户管理员授权，安装冻结 worker `04B9986FD2A0F625D88EE9A94C5CE8FF6AA4240C86D663EA3534D46C910316DB`，SCM LocalSystem/automatic/Running。实际普通客户端 `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` 返回 ERROR_ACCESS_DENIED，导致原认证失败。证据 `.tools/p11-20261014/live-service-trial/{install-evidence,access-diagnostic}.json`。Coder 未安装/升级/停止服务、未写 ProgramData 或修改进程/token ACL；现存服务继续运行。

## 最小修复

普通客户端不再打开 SYSTEM 进程。固定 SCM 查询仍强制 own-process、Running、非零 PID、LocalSystem、固定带引号 ImagePath；受保护 manifest、安装路径 owner/DACL/无重解析点、固定 EXE SHA 校验保留。现有已连接 pipe 句柄上读取内核 server PID/session，要求 PID 等于固定 SCM 且 session 0，并检查前后 SCM/PID/session 不变。

管道 handle 请求增加 READ_CONTROL（现存固定服务 DACL 已授予）。GetSecurityInfo(SE_KERNEL_OBJECT) 核 owner 为 SY/BA，protected 非 NULL DACL，只有 SY/BA full 和安装用户原固定 specific 0x00120183。MapGenericMask 规范化 generic rights 后比较，不依赖 ACE 排序或字符串表示；拒额外主体、ordinary owner、实例创建/WRITE_DAC/WRITE_OWNER 等额外权限。服务端 ACL 本身不变。验证受保护 EXE 后持有路径/目录/文件句柄至本次认证结束，不把响应中的 SID/build 当身份来源。

依据：Microsoft [Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)、[GetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo)、[SE_OBJECT_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/accctrl/ne-accctrl-se_object_type)。[GetNamedPipeServerSessionId](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserversessionid) 文档句柄描述指向服务端创建句柄，当前主 Agent 和普通夹具已实测客户端句柄能读取 server session；不将文档措辞当成未测试保证。

## 当前证据

- `scripts/dev.ps1 test --locked --lib file_index::worker::service`：22 PASS，包含同 PID 但错误 SCM account/image、前后状态/PID变化、pipe session/PID不符、generic ACL 映射、普通 owner、NULL/非protected/额外权限拒绝、固定 EXE bytes 替换导致 SHA 失败，以及既有生命周期/恢复边界。
- 真实普通权限管道夹具：内核 PID 为测试进程、session 非0，普通 owner 管道被拒绝；不要求 OpenProcess SYSTEM。
- `scripts/dev.ps1 test --locked --lib ordinary_ipc_auth_cursor_incremental_and_restart`：1 PASS；新增 READ_CONTROL 不破坏现有普通实验 IPC。
- 核心 clippy lib/bin/tests `-D warnings`、fmt check PASS。release CLI 构建 PASS。
- 新 CLI `target/release/dao-index-worker.exe` SHA256 `2CFE14924D1180E85C5771A5EE568983A5036C032BB1F1B2CE392657A5C8442B`。Coder 以普通权限执行 `service-health` exit0，认证现存 `04B998…` 服务成功：7卷，C/D/E worker_started，2个无盘符 NTFS journal_unavailable，2个unsupported。没有搜索真实文件名、操作源文件或再次提权。
- 正在构建新 Debug 桌面，刻意继续嵌入旧 `04B998…` payload，以“新 client + 原服务”验证连接；未运行 prepare-index-payload，避免桌面预期版本漂移。这个混合诊断候选不作为最终统一安装包交付。

本 finding 修复不代表全部 P11 实机/性能目标通过。新的统一版本部署由主 Agent 协调。跨账户、服务升级/回滚、重启及真实完整覆盖等按 Plan 保留未验证项。

Debug 普通客户端候选构建完成：`desktop/src-tauri/target/debug/dao-shell-desktop.exe` SHA256 `91C2A3D6904778F00271DA6CB5FC7F6BE83C806EFEA0C6A94070301A7E6E54A0`，嵌入仍为 `04B9986FD2A0F625D88EE9A94C5CE8FF6AA4240C86D663EA3534D46C910316DB`。主 Agent 可冻结并对现存服务做普通 native 连接验证；此处尚不宣称新桌面实际验证完成。CLI 已证明新认证在当前 SYSTEM 实例上通过。
最终桌面 clippy（含 tests，-D warnings）PASS。没有仍运行的构建；停止本 finding 编码，统一版本/安装包由主 Agent 决定。

## 主 Agent 原生复核与 Release 计时候选

主 Agent 对 `91C2…` 新 Debug client + `04B998…` 原服务普通原生实测 PASS，证据 `.tools/p11-20261014/live-service-trial/native-fixed-auth.json`：服务 running、无模型虚构关键词返回 1 条、source=windows_service、Scope 配置未变。初次 UI 669ms 是 Debug/初次值，不作 Release 性能结论。按派发正在构建同样旧 payload 的 Release desktop；没有 prepare、服务升级或优化算法改动。

## 采集瓶颈最小探索建议（仅分析，未实施）

Windows resolver 对每个普通单名称对象至少执行 OpenFileById、info(before)、FileStandardInfo(delete pending)、FSCTL_READ_FILE_USN_DATA、info(after)；多名称硬链接另枚举名称并核父/重开身份。旧 543676 次 metadata / 151.6s 是聚合计时，不能据此判定其中某 API 是唯一瓶颈。首先在独立虚构夹具按单名、多链接和父补查拆分调用次数/耗时，建议32目录/1万对象以及取消预算；只输出计数。暂不并行、不改schema、不加查询缓存。

若逐文件打开确为主因，候选为官方 [GetFileInformationByHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex) 的目录信息类按目录句柄批量读取名称和ID（[FILE_ID_EXTD_DIR_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_extd_dir_info)），与现 resolver 在有界夹具对照。MFT仍提供目录/身份覆盖对照，跨目录硬链接必须汇总全部名称；不可枚举目录、重解析点、未知ID布局、缺失父链不能静默丢弃或判完整。USN从采样开始检查点补读，变化对象仍走现有完整协调，失败旧代次保留。此方案不是原始MFT解析，但全卷接入仍有覆盖/竞态/目录权限风险，需夹具证据后由主 Agent 决定。没有实现批量采集或承诺30秒达标。

Release 普通客户端计时候选已完成（3m09s）：`desktop/src-tauri/target/release/dao-shell-desktop.exe` SHA256 `4BBCEBDFAD3DAF6FE060A77919EE5400EA22FB4DFF25BEB0130192CB0602AECE`；嵌入 payload 仍 `04B9986FD2A0F625D88EE9A94C5CE8FF6AA4240C86D663EA3534D46C910316DB`，同目录 WebView2Loader.dll 存在（160320bytes）。未 prepare、未升级服务；这是新认证客户端配现存服务的计时候选，不是统一新版安装器。交给主 Agent 做 Release端到端查询计时，Coder本轮未测性能或改变采集算法。
